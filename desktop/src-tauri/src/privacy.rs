//! 隐私门（㉘·C，DESIGN §7.2 决策表②）：任何权限档位都拦的"最危险"清单——
//! 用户裁定"完全访问基本零弹窗，除非是最危险的触及用户隐私的或本地隐私的"。
//!
//! 形态 = 纯函数扫描（args → 类别键），住桌面端决策表里、压过模式放行与
//! 豁免，强制弹卡且只许一次。参照对象 ZCode 的同位置设计只有 `alwaysAsk`
//! 工具钉（无凭据/危险命令清单——其 path-policy 自述有意不做路径围栏）；
//! 本清单是侦察报告"值得补的桌面端最小集"建议的保守子集。
//!
//! 纪律：**启发式宁漏勿错杀**——漏报由档位兜底（`询问` 档非 Pure 本来就
//! 卡；`完全` 档是用户显式选择的自担面），错杀会让 `完全` 档弹窗不断、
//! 直接违背本门存在的目的。`.env` 明确不拦（开发日常读写，ZCode 同款不拦；
//! 未来可配开关）。清单只增不删须走设计文档。

use serde_json::Value;

/// 命中类别键（前端 i18n `approval.sensitive.<key>`，未知键裸显）。
/// 与 `desktop/src/locales/{zh-CN,en}.ts` 成对维护。
pub(crate) type SensitiveKey = &'static str;

/// 判定一次工具调用是否触及隐私/高危目标。扫 args 里所有字符串候选
/// （`content`/`patch` 这类"文件体"字段与超长串跳过——写一个内容里*提到*
/// `.ssh` 的文件不等于碰凭据，错杀不可接受）。
pub(crate) fn check(_tool: &str, args: &Value) -> Option<SensitiveKey> {
    let mut hit = None;
    walk_at(args, 6, &mut |s| {
        if hit.is_none() {
            hit = scan_string(s);
        }
    });
    hit
}

/// 深度优先收集字符串（限深限长；文件体字段跳过）。
fn walk_at(v: &Value, depth: usize, f: &mut dyn FnMut(&str)) {
    if depth == 0 {
        return;
    }
    match v {
        Value::String(s) => {
            if s.len() <= 512 {
                f(s);
            }
        }
        Value::Array(items) => {
            for it in items {
                walk_at(it, depth - 1, f);
            }
        }
        Value::Object(map) => {
            for (k, val) in map {
                if !matches!(k.as_str(), "content" | "patch" | "text" | "body") {
                    walk_at(val, depth - 1, f);
                }
            }
        }
        _ => {}
    }
}

/// 单串扫描：先命令后路径（命令类更具体）。命令串只小写（`/s` `/q` 这类
/// 旗标不能被分隔符归一毁掉）；路径串才做 `/`→`\` 归一。
fn scan_string(s: &str) -> Option<SensitiveKey> {
    let lower = s.to_ascii_lowercase();
    command_hit(&lower).or_else(|| path_hit(&lower.replace('/', "\\")))
}

// ---- 命令类（凭据库 / 转储 / 毁灭性）----------------------------------------

fn command_hit(cmd: &str) -> Option<SensitiveKey> {
    // Windows 凭据库枚举 / 转储工具
    if ["vaultcmd", "mimikatz", "sekurlsa"]
        .iter()
        .any(|m| cmd.contains(m))
    {
        return Some("os_store");
    }
    let lsass_dump = (cmd.contains("lsass") && cmd.contains("dmp"))
        || (cmd.contains("comsvcs") && cmd.contains("minidump"))
        || (cmd.contains("procdump") && cmd.contains("lsass"));
    if lsass_dump {
        return Some("cred_dump");
    }
    // 毁灭性：盘格 / 分区 / 阴影副本 / 引导存储 / 全盘擦写 / fork bomb / 整盘 dd
    let catastrophic = [
        "mkfs",
        "diskpart",
        "cipher /w",
        "vssadmin delete",
        "bcdedit",
        ":(){",
        " of=/dev/",
    ]
    .iter()
    .any(|m| cmd.contains(m))
        || format_targets_drive(cmd)
        || recursive_force_delete_targets_root(cmd);
    catastrophic.then_some("catastrophic")
}

/// `format x:`（带空格分隔，避免误伤 "format json" 这类普通词组）。
fn format_targets_drive(cmd: &str) -> bool {
    cmd.split_whitespace()
        .zip(cmd.split_whitespace().skip(1))
        .any(|(a, b)| {
            a == "format" && {
                let b = b.trim_end_matches(['\\', '/']);
                b.len() == 2 && b.as_bytes()[1] == b':' && b.as_bytes()[0].is_ascii_lowercase()
            }
        })
}

/// 递归强删打在盘根 / 家目录：`rm -rf /`、`rm -rf ~`、`rd /s /q x:\`。
/// 只认这几种无歧义形态；变体漏报交给档位兜底（宁漏勿错杀）。
fn recursive_force_delete_targets_root(cmd: &str) -> bool {
    let rm_root = ["rm -rf /", "rm -fr /", "rm -r -f /", "rm -rf ~", "rm -fr ~"]
        .iter()
        .any(|m| cmd.contains(m));
    let rd_root = ["rd /s /q", "rmdir /s /q"]
        .iter()
        .any(|m| match cmd.find(m) {
            Some(pos) => cmd[pos..]
                .split_whitespace()
                .nth(3)
                .map(|t| {
                    let t = t.trim_end_matches(['\\', '/']);
                    t.len() == 2 && t.as_bytes()[1] == b':'
                })
                .unwrap_or(false),
            None => false,
        });
    rm_root || rd_root
}

// ---- 路径类（凭据目录 / 文件 / 浏览器凭据库）----------------------------------

fn path_hit(norm: &str) -> Option<SensitiveKey> {
    // 整目录：组件边界——`x.ssh` 不算；`\.ssh\` 或以 `\.ssh` 结尾（列目录）才算
    for (marker, key) in [
        ("\\.ssh\\", "ssh"),
        ("\\.aws\\", "aws"),
        ("\\.gnupg\\", "gpg"),
        ("\\.kube\\", "kube"),
    ] {
        if norm.contains(marker) || norm.ends_with(marker.trim_end_matches('\\')) {
            return Some(key);
        }
    }
    // 散文件名（主目录凭据老三样）
    if norm.contains("\\.git-credentials") || norm.contains("\\.netrc") {
        return Some("creds_file");
    }
    if norm.contains("\\.docker\\") && norm.contains("config.json") {
        return Some("docker");
    }
    browser_store(norm)
}

/// 浏览器凭据库：凭据库文件名 × 厂商目录限定（裸 `cookies` 不算——项目里
/// 可能真有叫 cookies 的文件；限定词大幅压误报）。
fn browser_store(norm: &str) -> Option<SensitiveKey> {
    const VENDORS: [&str; 8] = [
        "chrome", "chromium", "edge", "firefox", "brave", "opera", "vivaldi", "browser",
    ];
    const STORES: [&str; 5] = [
        "\\login data",
        "\\cookies",
        "\\logins.json",
        "\\key4.db",
        "\\local state",
    ];
    if !VENDORS.iter().any(|v| norm.contains(v)) {
        return None;
    }
    STORES
        .iter()
        .any(|s| norm.contains(s) || norm.ends_with(s.trim_start_matches('\\')))
        .then_some("browser")
}

#[cfg(test)]
mod tests;
