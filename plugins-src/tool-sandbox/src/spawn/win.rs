//! Windows 受限 spawn（⑯ 方案定稿 = 候选①受限令牌 + JobObject 限额）。
//!
//! 流程：双管道（读端去继承，归宿主读取线程）→ NUL stdin → 受限令牌
//! （`CreateRestrictedToken`：DISABLE_MAX_PRIVILEGE 剥全部特权 + deny
//! BUILTIN\Administrators 不可 UAC 提升；自派生受限令牌走
//! `CreateProcessAsUserW` 无需特权——失败降级普通令牌，isolation 如实
//! 标注 job-only，不静默）→ `CREATE_SUSPENDED` 起进程 → 挂 Job（内存/
//! 进程数限额 + 可选 CPU 硬顶 + 剪贴板 UI 限制 + `KILL_ON_JOB_CLOSE`
//! 树治理；不开 BREAKAWAY，子进程不可脱离）→ Resume。**Job 建立失败 =
//! 拒绝执行**——沙箱的核心承诺就是限额，与 tool-shell"治理缺席降级
//! 照跑"的语义有意不同（宁拒勿猜）。

use std::ffi::c_void;
use std::fs::File;
use std::os::windows::io::FromRawHandle;
use std::path::Path;

use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GetLastError, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
    SetHandleInformation, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::{
    AllocateAndInitializeSid, CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, FreeSid, PSID,
    SECURITY_ATTRIBUTES, SID_AND_ATTRIBUTES, SID_IDENTIFIER_AUTHORITY, TOKEN_ASSIGN_PRIMARY,
    TOKEN_DUPLICATE, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_CPU_RATE_CONTROL_ENABLE,
    JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
    JOB_OBJECT_UILIMIT_READCLIPBOARD, JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
    JOBOBJECT_BASIC_UI_RESTRICTIONS, JOBOBJECT_CPU_RATE_CONTROL_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicUIRestrictions,
    JobObjectCpuRateControlInformation, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::SystemServices::SE_GROUP_USE_FOR_DENY_ONLY;
use windows_sys::Win32::System::Threading::{
    CREATE_SUSPENDED, CreateProcessAsUserW, CreateProcessW, GetCurrentProcess, GetExitCodeProcess,
    OpenProcessToken, PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOW,
    TerminateProcess, WaitForSingleObject,
};

use crate::spawn::{RunLimits, Spawned};

/// 挂起→挂 Job→恢复的起跑序列里**父侧临时**句柄（管道写端/NUL stdin/
/// 降级时未用的受限令牌），成功与失败路径都要收掉。**Job 句柄不在此**——
/// 它带 KILL_ON_JOB_CLOSE：父侧一关，整棵进程树立刻被终止，必须移交
/// `Imp` 持有到执行结束（教训：起跑序列里误关它 = 子进程秒死、输出全空）。
struct TempHandles {
    token: Option<HANDLE>,
    stdin_nul: HANDLE,
    wr_out: HANDLE,
    wr_err: HANDLE,
}

impl TempHandles {
    /// SAFETY: 字段都是本函数链路新建的有效句柄，各至多关一次
    unsafe fn close_all(&mut self) {
        // SAFETY: unsafe fn 内显式块（edition 2024 deny-by-default lint）
        unsafe {
            CloseHandle(self.stdin_nul);
            CloseHandle(self.wr_out);
            CloseHandle(self.wr_err);
            if let Some(tok) = self.token {
                CloseHandle(tok);
            }
        }
    }
}

pub(crate) struct Imp {
    process: HANDLE,
    job: HANDLE,
}

impl Imp {
    pub(crate) fn poll_wait(&mut self) -> Result<Option<Option<i32>>, String> {
        // SAFETY: 句柄在本对象生命周期内有效
        match unsafe { WaitForSingleObject(self.process, 0) } {
            WAIT_OBJECT_0 => {
                let mut code: u32 = 0;
                // SAFETY: 同上
                if unsafe { GetExitCodeProcess(self.process, &mut code) } == 0 {
                    return Err("GetExitCodeProcess failed".into());
                }
                Ok(Some(Some(code as i32)))
            }
            WAIT_TIMEOUT => Ok(None),
            _ => Err("WaitForSingleObject failed".into()),
        }
    }

    pub(crate) fn kill_tree(&self) {
        // SAFETY: 同上；已全部退出的 job 上调用是 no-op（幂等）
        unsafe { TerminateJobObject(self.job, 1) };
    }

    pub(crate) fn reap(&mut self) {
        // SAFETY: 同上；收尾清残留孙进程（握管道会让读取线程等不到 EOF）
        unsafe {
            TerminateJobObject(self.job, 1);
            let _ = WaitForSingleObject(self.process, 5_000);
        }
    }
}

impl Drop for Imp {
    fn drop(&mut self) {
        // SAFETY: 句柄在本对象生命周期内有效；KILL_ON_JOB_CLOSE 在关
        // job 句柄时收掉一切残留进程
        unsafe {
            CloseHandle(self.process);
            CloseHandle(self.job);
        }
    }
}

pub(crate) fn spawn_process(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    limits: &RunLimits,
) -> Result<Spawned, String> {
    let (rd_out, wr_out) = inheritable_pipe()?;
    let (rd_err, wr_err) = inheritable_pipe()?;
    // SAFETY: 新建句柄；后续所有失败路径经 handles.close_all() 清理
    let stdin_nul = unsafe {
        let name: Vec<u16> = "\\\\.\\NUL".encode_utf16().chain([0]).collect();
        let sa = inherit_sa();
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &sa,
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if stdin_nul == INVALID_HANDLE_VALUE {
        return Err("open NUL device failed".into());
    }

    let token = build_restricted_token();
    let job = make_job(limits)?;
    let mut temps = TempHandles {
        token,
        stdin_nul,
        wr_out,
        wr_err,
    };

    let mut cmd_line = quote_arg(program);
    for a in args {
        cmd_line.push(' ');
        cmd_line.push_str(&quote_arg(a));
    }
    let mut cmd_w: Vec<u16> = cmd_line.encode_utf16().chain([0]).collect();
    let cwd_w: Option<Vec<u16>> = cwd.map(|p| {
        p.as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain([0])
            .collect()
    });

    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdInput = stdin_nul;
    si.hStdOutput = wr_out;
    si.hStdError = wr_err;
    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    let (spawn_ok, used_restricted, restrict_err) = {
        let cwd_ptr = match &cwd_w {
            Some(w) => w.as_ptr(),
            None => std::ptr::null(),
        };
        let mut used = false;
        let mut fail: Option<u32> = None;
        // SAFETY: si/pi 均为本栈帧有效结构；命令行缓冲存活至调用返回
        let ok = unsafe {
            let mut ok = false;
            if let Some(tok) = token {
                ok = CreateProcessAsUserW(
                    tok,
                    std::ptr::null(),
                    cmd_w.as_mut_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    1,
                    CREATE_SUSPENDED,
                    std::ptr::null(),
                    cwd_ptr,
                    &si,
                    &mut pi,
                ) != 0;
                if !ok {
                    fail = Some(GetLastError());
                }
                used = ok;
            }
            if !ok {
                // 受限令牌缺席或其 spawn 失败：普通令牌降级（Job 限额
                // 仍然成立），isolation 字符串如实标注，不静默
                ok = CreateProcessW(
                    std::ptr::null(),
                    cmd_w.as_mut_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    1,
                    CREATE_SUSPENDED,
                    std::ptr::null(),
                    cwd_ptr,
                    &si,
                    &mut pi,
                ) != 0;
            }
            ok
        };
        (ok, used, fail)
    };
    if !spawn_ok {
        // SAFETY: 句柄都还是本函数新建、未移交的（进程没起来，Job 一并关）
        unsafe {
            CloseHandle(job);
            temps.close_all();
        }
        return Err(format!("CreateProcess failed (error {})", unsafe {
            GetLastError()
        }));
    }

    // 挂 Job 必须在 Resume 之前（否则有起跑竞态窗口）；失败即终止并拒绝
    // SAFETY: pi 句柄是 spawn 刚交付的有效值
    if unsafe { AssignProcessToJobObject(job, pi.hProcess) } == 0 {
        let err = unsafe { GetLastError() };
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
            CloseHandle(job);
            temps.close_all();
        }
        return Err(format!("AssignProcessToJobObject failed (error {err})"));
    }
    unsafe {
        ResumeThread(pi.hThread);
        CloseHandle(pi.hThread);
        temps.close_all();
    }

    let isolation = if used_restricted {
        "restricted-token+job".to_string()
    } else {
        match restrict_err {
            Some(code) => {
                format!("job-only (restricted token unavailable: error {code})")
            }
            None => "job-only (restricted token unavailable: token build failed)".into(),
        }
    };
    Ok(Spawned {
        imp: Imp {
            process: pi.hProcess,
            job,
        },
        stdout: Some(unsafe { File::from_raw_handle(rd_out) }),
        stderr: Some(unsafe { File::from_raw_handle(rd_err) }),
        isolation,
    })
}

/// 继承用 SECURITY_ATTRIBUTES（管道写端 / NUL stdin 要能被子进程继承）。
fn inherit_sa() -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    }
}

/// 匿名管道；读端去继承（归宿主读取线程），写端可继承（进子进程）。
fn inheritable_pipe() -> Result<(HANDLE, HANDLE), String> {
    let sa = inherit_sa();
    let mut rd: HANDLE = std::ptr::null_mut();
    let mut wr: HANDLE = std::ptr::null_mut();
    // SAFETY: 两个新句柄；失败时 CreatePipe 不产出句柄
    if unsafe { CreatePipe(&mut rd, &mut wr, &sa, 0) } == 0 {
        return Err(format!("CreatePipe failed (error {})", unsafe {
            GetLastError()
        }));
    }
    // SAFETY: rd 是 CreatePipe 刚交付的有效句柄
    unsafe { SetHandleInformation(rd, HANDLE_FLAG_INHERIT, 0) };
    Ok((rd, wr))
}

/// 建带限额的 Job：内存/进程数上限 + 可选 CPU 硬顶 + 剪贴板 UI 限制 +
/// `KILL_ON_JOB_CLOSE` 树治理。任何一步失败都 CloseHandle 后返回 Err
/// （限额承诺不可打折，见模块注释）。
fn make_job(limits: &RunLimits) -> Result<HANDLE, String> {
    // SAFETY: 新句柄；Err 路径立即 CloseHandle
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err("CreateJobObjectW failed".into());
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        info.ProcessMemoryLimit = (limits.mem_mb * 1024 * 1024) as usize;
        info.BasicLimitInformation.ActiveProcessLimit = limits.max_procs as u32;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
        {
            CloseHandle(job);
            return Err("set job memory/process limits failed".into());
        }
        if let Some(rate) = limits.cpu_rate {
            let mut cpu: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION = std::mem::zeroed();
            cpu.ControlFlags =
                JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP;
            // union 写入（外层 unsafe 块已覆盖）：百分比×100（50% = 5000）
            cpu.Anonymous.CpuRate = (rate * 100) as u32;
            if SetInformationJobObject(
                job,
                JobObjectCpuRateControlInformation,
                &cpu as *const _ as *const c_void,
                std::mem::size_of::<JOBOBJECT_CPU_RATE_CONTROL_INFORMATION>() as u32,
            ) == 0
            {
                CloseHandle(job);
                return Err("set job cpu cap failed".into());
            }
        }
        let ui = JOBOBJECT_BASIC_UI_RESTRICTIONS {
            UIRestrictionsClass: JOB_OBJECT_UILIMIT_READCLIPBOARD
                | JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
        };
        if SetInformationJobObject(
            job,
            JobObjectBasicUIRestrictions,
            &ui as *const _ as *const c_void,
            std::mem::size_of::<JOBOBJECT_BASIC_UI_RESTRICTIONS>() as u32,
        ) == 0
        {
            CloseHandle(job);
            return Err("set job ui limits failed".into());
        }
        Ok(job)
    }
}

/// 受限令牌：基于当前进程令牌 `CreateRestrictedToken`——DISABLE_MAX_PRIVILEGE
/// 剥全部特权 + deny BUILTIN\Administrators（S-1-5-32-544，不可再 UAC
/// 提升）。任一步失败返回 None，调用方降级普通令牌并如实标注。
fn build_restricted_token() -> Option<HANDLE> {
    // SAFETY: 令牌/SID 生命周期都在本函数内收口；out 句柄移交调用方
    unsafe {
        let mut tok: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_QUERY,
            &mut tok,
        ) == 0
        {
            return None;
        }
        let nta = SID_IDENTIFIER_AUTHORITY {
            Value: [0, 0, 0, 0, 0, 5],
        };
        let mut sid: PSID = std::ptr::null_mut();
        let have_sid = AllocateAndInitializeSid(&nta, 2, 32, 544, 0, 0, 0, 0, 0, 0, &mut sid) != 0;
        let restricted = if have_sid {
            let deny = SID_AND_ATTRIBUTES {
                Sid: sid,
                Attributes: SE_GROUP_USE_FOR_DENY_ONLY as u32,
            };
            let mut out: HANDLE = std::ptr::null_mut();
            let ok = CreateRestrictedToken(
                tok,
                DISABLE_MAX_PRIVILEGE,
                1,
                &deny,
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                &mut out,
            ) != 0;
            if ok { Some(out) } else { None }
        } else {
            None
        };
        if !sid.is_null() {
            FreeSid(sid);
        }
        CloseHandle(tok);
        restricted
    }
}

/// Windows 命令行转义（MSVCRT 规则）：含空格/tab/引号的参数加引号；
/// 反斜杠"位于引号前翻倍、闭引号前多补一个"。
fn quote_arg(s: &str) -> String {
    if !s.is_empty() && !s.chars().any(|c| c == ' ' || c == '\t' || c == '"') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    let mut slashes = 0usize;
    for c in s.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
                slashes = 0;
                out.push('"');
            }
            _ => {
                out.push_str(&"\\".repeat(slashes));
                slashes = 0;
                out.push(c);
            }
        }
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}

#[cfg(test)]
mod tests;
