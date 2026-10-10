//! 项目粘性授权规则（《Permission Model Redesign (ZCode Alignment) v1》
//! §2.3，v2.145）：「本项目始终允许」的持久化面。
//!
//! - **存储**：数据根 `permissions/<fnv1a-hex8>.json`，一工作区一文件，
//!   形状对齐 ZCode PermissionRuleset：`{version, workspace, allow:[{tool,
//!   content?}], deny:[…]}`。content 缺省 = 工具级；`前缀:*` = token 边界
//!   前缀匹配（`subject == prefix || subject.starts_with(prefix + " ")`，
//!   ZCode matchesRuleContent 同款）；其余精确串。每次查询直读文件（规则
//!   文件 KB 级，非 Pure 调用才查；无缓存 = 人改文件立即生效，可作撤销
//!   授权的手工出口）。
//! - **写入面**（v2.145）：审批卡「本项目始终允许」——`shell` 落精确
//!   命令规则（v1 无 CLI 注册表不做智能前缀，记档设计稿 §3），其余工具
//!   落工具级规则。UI 不产 deny 规则（ZCode 同款：拒绝不持久化），deny
//!   清单供手改 JSON 使用。
//! - **读侧**：坏 JSON / 版本不识别 = 当空（宁缺勿假，与树投影同口径）；
//!   空规则集清除时整文件删除。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use plugin_api::memlayout::{atomic_write, fnv1a64};
use serde::{Deserialize, Serialize};

/// 单条规则。`content = None` 为工具级（该工具一切调用放行/拒绝）；
/// `Some` 仅对带命令文本的工具（`shell`）有意义。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Ruleset {
    #[serde(default)]
    version: u8,
    /// 人读字段（文件名是哈希，原值留档）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workspace: Option<String>,
    #[serde(default)]
    allow: Vec<PermissionRule>,
    #[serde(default)]
    deny: Vec<PermissionRule>,
}

const VERSION: u8 = 1;

/// 规则匹配（ZCode `matchesRuleContent` 同款语义）。`subject` = 命令文本
/// （shell）——None 时只有工具级规则命中。
pub(crate) fn rule_matches(rule: &PermissionRule, tool: &str, subject: Option<&str>) -> bool {
    if rule.tool != tool {
        return false;
    }
    let Some(content) = &rule.content else {
        return true;
    };
    let Some(subject) = subject else {
        return false;
    };
    match content.strip_suffix(":*") {
        Some(prefix) => subject == prefix || subject.starts_with(&format!("{prefix} ")),
        None => subject == content,
    }
}

/// 项目规则库：`<数据根>/permissions/` 下每工作区一文件。无进程内缓存
/// （每次查询直读，人改即生效）。
pub struct PermissionRuleStore {
    dir: PathBuf,
    /// 读写互斥（同一文件的读-改-写竞态；查询共享无妨但一把锁简单对）
    io: Mutex<()>,
}

impl PermissionRuleStore {
    pub fn new(data_root: &Path) -> Self {
        Self {
            dir: data_root.join("permissions"),
            io: Mutex::new(()),
        }
    }

    fn file_of(&self, workspace: &str) -> PathBuf {
        self.dir
            .join(format!("{:08x}.json", fnv1a64(workspace.as_bytes())))
    }

    fn read(&self, workspace: &str) -> Ruleset {
        let Ok(text) = std::fs::read_to_string(self.file_of(workspace)) else {
            return Ruleset::default();
        };
        match serde_json::from_str::<Ruleset>(&text) {
            // 版本不识别 = 当空（宁缺勿假）
            Ok(rs) if rs.version == VERSION => rs,
            _ => Ruleset::default(),
        }
    }

    fn write(&self, workspace: &str, rs: &Ruleset) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("create permissions dir failed: {e}"))?;
        let text = serde_json::to_string_pretty(rs).map_err(|e| e.to_string())?;
        atomic_write(&self.file_of(workspace), &text)
    }

    /// 审批决策表第 ③ 级：项目 deny 规则命中？（v2.145 UI 不产 deny，
    /// 手改 JSON 生效；先于模式放行 = 用户明示拒绝连 Auto 也拦）
    pub fn denies(&self, workspace: &str, tool: &str, subject: Option<&str>) -> bool {
        let _g = self.io.lock().expect("permission rules io poisoned");
        self.read(workspace)
            .deny
            .iter()
            .any(|r| rule_matches(r, tool, subject))
    }

    /// 审批决策表第 ⑤ 级：项目 allow 规则命中？
    pub fn allows(&self, workspace: &str, tool: &str, subject: Option<&str>) -> bool {
        let _g = self.io.lock().expect("permission rules io poisoned");
        self.read(workspace)
            .allow
            .iter()
            .any(|r| rule_matches(r, tool, subject))
    }

    /// 「本项目始终允许」：追加一条 allow 规则并落盘（已存在同条 = 幂等）。
    pub fn add_allow(
        &self,
        workspace: &str,
        tool: &str,
        content: Option<String>,
    ) -> Result<(), String> {
        let _g = self.io.lock().expect("permission rules io poisoned");
        let mut rs = self.read(workspace);
        let rule = PermissionRule {
            tool: tool.to_string(),
            content,
        };
        if !rs.allow.contains(&rule) {
            rs.allow.push(rule);
        }
        rs.version = VERSION;
        rs.workspace = Some(workspace.to_string());
        self.write(workspace, &rs)
    }

    /// 设置管理行：清除本项目全部规则（文件删除）。
    pub fn clear(&self, workspace: &str) -> Result<(), String> {
        let _g = self.io.lock().expect("permission rules io poisoned");
        let path = self.file_of(workspace);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("remove rules file failed: {e}"))?;
        }
        Ok(())
    }

    /// 打开规则目录（设置行「打开文件夹」出口）。
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 设置管理行：全部工作区的规则概览（扫描目录逐文件读）。
    pub fn list_all(&self) -> Vec<ProjectRulesView> {
        let _g = self.io.lock().expect("permission rules io poisoned");
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for e in entries.filter_map(|e| e.ok()).map(|e| e.path()) {
            if e.extension().is_none_or(|x| x != "json") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&e) else {
                continue;
            };
            let Ok(rs) = serde_json::from_str::<Ruleset>(&text) else {
                continue;
            };
            if rs.version != VERSION {
                continue;
            }
            out.push(ProjectRulesView {
                workspace: rs.workspace.unwrap_or_default(),
                allow: rs.allow.len(),
                deny: rs.deny.len(),
            });
        }
        out.sort_by(|a, b| a.workspace.cmp(&b.workspace));
        out
    }
}

/// 设置管理行的投影（IPC 面）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectRulesView {
    pub workspace: String,
    pub allow: usize,
    pub deny: usize,
}

#[cfg(test)]
mod tests;
