//go:build windows

package adapter

import (
	"os/exec"
	"strconv"
	"syscall"
	"unsafe"
)

// processTree owns the Windows job that holds the plugin and every process it
// spawns. A job with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE keeps grandchildren
// from outliving the plugin when it crashes.
type processTree struct {
	handle syscall.Handle
	pid    int
}

const (
	jobObjectExtendedLimitInformation = 9
	jobObjectLimitKillOnJobClose      = 0x2000
	processSetQuota                   = 0x0100
	processTerminate                  = 0x0001
	processSynchronize                = 0x00100000
)

var (
	kernel32                     = syscall.NewLazyDLL("kernel32.dll")
	procCreateJobObjectW         = kernel32.NewProc("CreateJobObjectW")
	procSetInformationJobObject  = kernel32.NewProc("SetInformationJobObject")
	procAssignProcessToJobObject = kernel32.NewProc("AssignProcessToJobObject")
	procTerminateJobObject       = kernel32.NewProc("TerminateJobObject")
	procOpenProcess              = kernel32.NewProc("OpenProcess")
	procWaitForSingleObject      = kernel32.NewProc("WaitForSingleObject")
	procCloseHandle              = kernel32.NewProc("CloseHandle")
)

// The layouts mirror the Windows SDK; Go's alignment matches the SDK's on
// amd64 and arm64.
type jobBasicLimitInformation struct {
	PerProcessUserTimeLimit int64
	PerJobUserTimeLimit     int64
	LimitFlags              uint32
	MinimumWorkingSetSize   uintptr
	MaximumWorkingSetSize   uintptr
	ActiveProcessLimit      uint32
	Affinity                uintptr
	PriorityClass           uint32
	SchedulingClass         uint32
}

type ioCounters struct {
	ReadOperationCount  uint64
	WriteOperationCount uint64
	OtherOperationCount uint64
	ReadTransferCount   uint64
	WriteTransferCount  uint64
	OtherTransferCount  uint64
}

type jobExtendedLimitInformation struct {
	BasicLimitInformation jobBasicLimitInformation
	IoInfo                ioCounters
	ProcessMemoryLimit    uintptr
	JobMemoryLimit        uintptr
	PeakProcessMemoryUsed uintptr
	PeakJobMemoryUsed     uintptr
}

// prepareCommand is a no-op on Windows; the process tree uses a job object.
func prepareCommand(_ *exec.Cmd) {}

// attachProcessTree puts the started plugin into a kill-on-close job.
func attachProcessTree(cmd *exec.Cmd) *processTree {
	if cmd == nil || cmd.Process == nil {
		return nil
	}
	job, _, _ := procCreateJobObjectW.Call(0, 0)
	if job == 0 {
		return nil
	}
	info := jobExtendedLimitInformation{}
	info.BasicLimitInformation.LimitFlags = jobObjectLimitKillOnJobClose
	procSetInformationJobObject.Call(job, jobObjectExtendedLimitInformation, uintptr(unsafe.Pointer(&info)), unsafe.Sizeof(info))

	proc, _, _ := procOpenProcess.Call(processSetQuota|processTerminate, 0, uintptr(cmd.Process.Pid))
	if proc == 0 {
		procCloseHandle.Call(job)
		return nil
	}
	ok, _, _ := procAssignProcessToJobObject.Call(job, proc)
	procCloseHandle.Call(proc)
	if ok == 0 {
		procCloseHandle.Call(job)
		return nil
	}
	return &processTree{handle: syscall.Handle(job), pid: cmd.Process.Pid}
}

// kill terminates every process in the job and waits for the plugin to go, so a
// caller that stops an extension can delete its files immediately.
func (t *processTree) kill() {
	if t == nil || t.handle == 0 {
		return
	}
	procTerminateJobObject.Call(uintptr(t.handle), 1)
	if t.pid <= 0 {
		return
	}
	proc, _, _ := procOpenProcess.Call(processSynchronize, 0, uintptr(t.pid))
	if proc == 0 {
		return
	}
	procWaitForSingleObject.Call(proc, 2000)
	procCloseHandle.Call(proc)
}

// close releases the job; the kill-on-close limit takes any survivor with it.
func (t *processTree) close() {
	if t == nil || t.handle == 0 {
		return
	}
	procCloseHandle.Call(uintptr(t.handle))
	t.handle = 0
}

// killTree is the best-effort fallback when no job could be created.
func killTree(pid int) {
	cmd := exec.Command("taskkill", "/T", "/F", "/PID", strconv.Itoa(pid))
	_ = cmd.Run()
}
