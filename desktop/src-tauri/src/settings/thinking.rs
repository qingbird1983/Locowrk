//! 思考语言设置区（docs/design/archive/language of thought.md）：控制模型
//! reasoning（思考过程）用什么语言。定位是**设置项**——宿主在组装
//! system prompt 时注入一段提示词，非插件、非内核中间件、不跨 ABI。
//!
//! 落盘 settings.json `thinking` 区（文档 §7 的 [thinking] 同构，容器
//! 随本项目惯例为 settings.json 而非独立 toml）。缺省 `follow_ui`：
//! 界面中文 → 思考中文、界面英文 → 思考英文，零配置即合理结果。
//!
//! **会话内锁死（前缀缓存红线，文档 §5）**：本区只存配置；锁死语义由
//! 注入侧实现（`crate::thinking_language` 按会话首请求解析并缓存）——
//! 中途改这里，当前会话不变，新会话生效。

use serde::{Deserialize, Serialize};

/// 缺省档：跟随界面语言（settings.json `locale`，`system` 档经
/// sys-locale 解析 OS 语言）。
pub const THINKING_FOLLOW_UI: &str = "follow_ui";
/// 不指定：不注入任何提示词（模型用自己的缺省）。
pub const THINKING_NONE: &str = "none";

/// 白名单（文档 §7：follow_ui | zh-CN | en-US | none）。en-US 对应界面
/// `en` 档（提示词模板文件名跟 locale 包规范走）。
pub const THINKING_LANGUAGE_VALUES: [&str; 4] =
    [THINKING_FOLLOW_UI, "zh-CN", "en-US", THINKING_NONE];

/// settings.json `thinking` 区。AppSettings 级 `#[serde(default)]`：
/// 旧 settings.json 无此区零迁移。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinkingSettings {
    pub language: String,
}

impl Default for ThinkingSettings {
    fn default() -> Self {
        Self {
            language: THINKING_FOLLOW_UI.into(),
        }
    }
}

/// IPC 投影（SettingsView.thinking）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingView {
    pub language: String,
}

/// settings_set 的 `thinking` 分区载荷：单字段全量（与顶层 `locale`
/// 同款就地保存语义，SettingsSelect 每次提交完整值）。
#[derive(Debug, Clone, Deserialize)]
pub struct ThinkingUpdate {
    pub language: String,
}

impl ThinkingSettings {
    /// 校验 + 应用（台账 T15 从 `settings/update.rs` **机械搬移**：两条变换 =
    /// `next.thinking.` → `self.`、缩进减一层，其余逐字节不动；签名吃消费型值，
    /// 理由与等价性证据同 `plugins.rs` 的注释）。
    pub(crate) fn apply_update(&mut self, u: ThinkingUpdate) -> Result<(), String> {
        // 思考语言（docs/design/archive/language of thought.md）：白名单
        // 校验。会话内锁死由注入侧缓存保证——中途改这里不影响已开
        // 会话的 system 前缀，新会话才吃新值。
        if !THINKING_LANGUAGE_VALUES.contains(&u.language.as_str()) {
            return Err(format!(
                "思考语言 {:?} 非法（可选 follow_ui / zh-CN / en-US / none）",
                u.language
            ));
        }
        self.language = u.language;
        Ok(())
    }
}
