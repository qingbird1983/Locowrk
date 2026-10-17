//! settings_set 的校验与应用（**编排层**：分区一律转发，两个根字段就地处理）。
//!
//! 台账 T15 收的形状：本文件原来 381 行，其中约 310 行是十个分区"就地铺开"的
//! 校验与落值——而批 Q6 早就把 `developer` 那一段收进了分区自己。同一件事两种
//! 写法，且写法更丑的那个正在长新字段。现在每个分区自己持有
//! `apply_update(消费型值) -> Result<(), String>`，本文件按 settings.json 的字段
//! 顺序转发一遍：`appearance` / `embedding` / `context` / `memory` / `tools` /
//! `plugins` / `chat` / `developer` / `thinking` / `multimodal`（`llm` 自 v2.58
//! 就是这个形状）。两个根字段 `locale` / `shortcut` 不属于任何分区结构、各自只有
//! 3–6 行校验，为它们再造一层是凭空加机制 ⇒ 留在这里。
//!
//! 搬移的等价性不靠人读：十条变换只有"去一层缩进"与"next.<分区>. → self."
//! （`memory/update.rs` 多一条 `super::memory:: → super::`），行集合差由脚本核
//! （脚本与读数在提交正文）；既有 `settings/tests/*` 一字未改仍然绿是第二重证据。
//!
//! 顺序纪律（未改）：校验全部通过 → 应用到副本 → 落盘成功才提交内存。任何
//! 校验失败提前返回 Err，不污染内存态（测试 `rejects_*` 系列锁定）。

use super::{
    AppearanceUpdate, ChatUpdate, ContextUpdate, DeveloperUpdate, EmbeddingUpdate, LlmUpdate,
    MemoryUpdate, MultimodalUpdate, PluginsUpdate, SettingsStore, SettingsView, ThinkingUpdate,
    ToolsUpdate, FILE_NAME, LOCALES,
};
use tauri_plugin_global_shortcut::Shortcut;

impl SettingsStore {
    /// 校验 → 应用到副本 → 落盘成功才提交内存。返回最新投影。
    // 参数与 settings_set 同形：settings.json 各分区一一对应。
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &self,
        locale: Option<String>,
        appearance: Option<AppearanceUpdate>,
        shortcut: Option<String>,
        llm: Option<LlmUpdate>,
        embedding: Option<EmbeddingUpdate>,
        context: Option<ContextUpdate>,
        memory: Option<MemoryUpdate>,
        tools: Option<ToolsUpdate>,
        plugins: Option<PluginsUpdate>,
        chat: Option<ChatUpdate>,
        developer: Option<DeveloperUpdate>,
        thinking: Option<ThinkingUpdate>,
        multimodal: Option<MultimodalUpdate>,
    ) -> Result<SettingsView, String> {
        let mut next = self.inner.lock().expect("settings poisoned").clone();
        // ---- 根字段（不属于任何分区结构，校验就三行，不另造一层）----
        if let Some(loc) = locale {
            if !LOCALES.contains(&loc.as_str()) {
                return Err(format!("不支持的语言: {loc}（可选 system / zh-CN / en）"));
            }
            next.locale = loc;
        }
        // ---- 以下每个分区一行转发：校验与落值在该分区自己的文件里 ----
        if let Some(u) = appearance {
            next.appearance.apply_update(u)?;
        }
        if let Some(sc) = shortcut {
            // 校验用全局快捷键解析器本身（与 tray.rs 注册同一套语义）；
            // 空串 / 无法识别都拒绝。注册在命令层做（需要 AppHandle）。
            let sc = sc.trim().to_string();
            if sc.is_empty() {
                return Err("快捷键不能为空（示例 Ctrl+Shift+L）".into());
            }
            if sc.parse::<Shortcut>().is_err() {
                return Err(format!("无法识别的快捷键: {sc}（示例 Ctrl+Shift+L）"));
            }
            next.shortcut = sc;
        }
        if let Some(u) = llm {
            // v2.58 档位制：校验与应用在 LlmConfig::apply_update（平面
            // 字段映射到目标档，active 切档；失败不污染内存态）
            next.llm.apply_update(&u)?;
        }
        if let Some(u) = embedding {
            next.embedding.apply_update(u)?;
        }
        if let Some(u) = context {
            next.context.apply_update(u)?;
        }
        if let Some(u) = memory {
            next.memory.apply_update(u)?;
        }
        if let Some(u) = tools {
            next.tools.apply_update(u)?;
        }
        if let Some(u) = plugins {
            next.plugins.apply_update(u)?;
        }
        if let Some(u) = chat {
            next.chat.apply_update(u)?;
        }
        if let Some(u) = developer {
            // 分区内的校验与 apply 段（约 40 行）不铺开在这里写：那个文件本批加完
            // `organizeMaxRerun` 后到 401 行，被 `check_length` 判成新增超限件。职责归位
            // 到 `DeveloperSettings::apply_update`（搬移是机械的，等价性见那里的注释）。
            next.developer.apply_update(&u)?;
        }
        if let Some(u) = thinking {
            next.thinking.apply_update(u)?;
        }
        if let Some(u) = multimodal {
            next.multimodal.apply_update(u)?;
        }
        let bytes = serde_json::to_vec_pretty(&next).expect("settings serialize");
        std::fs::write(self.root.join(FILE_NAME), bytes)
            .map_err(|e| format!("settings.json 写入失败: {e}"))?;
        *self.inner.lock().expect("settings poisoned") = next;
        Ok(self.view())
    }
}
