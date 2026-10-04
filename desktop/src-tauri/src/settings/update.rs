//! settings_set 的校验与应用（settings.json 各分区一一对应；从
//! settings.rs 拆出只为本体守住 400 行纪律——逻辑不变）。
//!
//! 顺序纪律：校验全部通过 → 应用到副本 → 落盘成功才提交内存。任何
//! 校验失败提前返回 Err，不污染内存态（测试 `rejects_*` 系列锁定）。

use super::{
    AppearanceUpdate, ChatUpdate, ContextUpdate, DeveloperUpdate, EmbeddingUpdate, LlmUpdate,
    MemoryUpdate, PluginsUpdate, SettingsStore, SettingsView, ShellUpdate, ToolsUpdate, FILE_NAME,
    LOCALES, STABILITY_ROUNDS_RANGE,
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
        shell: Option<ShellUpdate>,
        plugins: Option<PluginsUpdate>,
        chat: Option<ChatUpdate>,
        developer: Option<DeveloperUpdate>,
    ) -> Result<SettingsView, String> {
        let mut next = self.inner.lock().expect("settings poisoned").clone();
        if let Some(loc) = locale {
            if !LOCALES.contains(&loc.as_str()) {
                return Err(format!("不支持的语言: {loc}（可选 system / zh-CN / en）"));
            }
            next.locale = loc;
        }
        if let Some(u) = appearance {
            if let Some(theme) = &u.theme {
                if !super::appearance::valid_theme(theme) {
                    return Err(format!(
                        "不支持的主题: {theme}（可选 dark / light / system）"
                    ));
                }
                next.appearance.theme = theme.clone();
            }
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
            if let Some(b) = u.base_url {
                next.embedding.base_url = (!b.is_empty()).then_some(b);
            }
            if let Some(m) = u.model {
                next.embedding.model = (!m.is_empty()).then_some(m);
            }
            if let Some(k) = u.api_key {
                next.embedding.api_key = (!k.is_empty()).then_some(k);
            }
        }
        if let Some(u) = context {
            if let Some(pct) = u.threshold_pct {
                if !(10..=90).contains(&pct) {
                    return Err(format!("压缩阈值 {pct}% 超出范围（10–90，10% 一级）"));
                }
                next.context.threshold_pct = pct;
            }
            if let Some(s) = u.strategy {
                next.context.strategy = s;
            }
            if let Some(n) = u.stability_rounds {
                if !STABILITY_ROUNDS_RANGE.contains(&n) {
                    return Err(format!(
                        "稳定期轮数 {n} 超出范围（{}–{}）",
                        STABILITY_ROUNDS_RANGE.start(),
                        STABILITY_ROUNDS_RANGE.end()
                    ));
                }
                next.context.stability_rounds = n;
            }
        }
        if let Some(u) = memory {
            if let Some(e) = u.enabled {
                next.memory.enabled = e;
            }
            if let Some(k) = u.top_k {
                if !(1..=20).contains(&k) {
                    return Err(format!("记忆注入条数 {k} 超出范围（1–20）"));
                }
                next.memory.top_k = k;
            }
            if let Some(a) = u.auto_extract {
                next.memory.auto_extract = a;
            }
        }
        if let Some(u) = tools {
            if let Some(e) = u.enabled {
                next.tools.enabled = e;
            }
            if let Some(n) = u.max_per_request {
                if !(1..=200).contains(&n) {
                    return Err(format!("每次请求最大工具数 {n} 超出范围（1–200）"));
                }
                next.tools.max_per_request = n;
            }
            if let Some(secs) = u.call_timeout_secs {
                if !super::tools::CALL_TIMEOUT_RANGE.contains(&secs) {
                    return Err(format!(
                        "单次工具调用超时 {secs}s 超出范围（{}–{} 秒）",
                        super::tools::CALL_TIMEOUT_RANGE.start(),
                        super::tools::CALL_TIMEOUT_RANGE.end()
                    ));
                }
                next.tools.call_timeout_secs = secs;
            }
            if let Some(steps) = u.max_turn_steps {
                if !super::tools::MAX_TURN_STEPS_RANGE.contains(&steps) {
                    return Err(format!(
                        "单轮步数上限 {steps} 非法（{}–{}）",
                        super::tools::MAX_TURN_STEPS_RANGE.start(),
                        super::tools::MAX_TURN_STEPS_RANGE.end()
                    ));
                }
                next.tools.max_turn_steps = steps;
            }
            if let Some(roots) = u.fs_roots {
                next.tools.fs_roots = super::tools::normalize_fs_roots(&roots)?;
            }
            if let Some(secs) = u.approval_timeout_secs {
                // v2.49 可配置原则：审批守夜阈值可调（0 = 不限等待）
                if !super::tools::APPROVAL_TIMEOUT_RANGE.contains(&secs) {
                    return Err(format!(
                        "审批等待时长 {secs}s 非法（0 = 不限，或 {}–{} 秒）",
                        super::tools::APPROVAL_TIMEOUT_RANGE.start(),
                        super::tools::APPROVAL_TIMEOUT_RANGE.end()
                    ));
                }
                next.tools.approval_timeout_secs = secs;
            }
            if let Some(mode) = u.default_permission_mode {
                // ㉘·B 新会话权限档缺省：只收四档合法值（会话级覆盖存旁车）
                if crate::permission_mode::PermissionMode::parse(&mode).is_none() {
                    return Err(format!(
                        "权限档缺省 {mode:?} 非法（plan / ask / acceptEdits / auto）"
                    ));
                }
                next.tools.default_permission_mode = mode;
            }
            if let Some(pin) = u.pin_session {
                // KV v2 §4.2 工具集会话钉死（无范围校验，布尔开关）
                next.tools.pin_session = pin;
            }
        }
        if let Some(u) = shell {
            if let Some(e) = u.unlock_risky {
                next.shell.unlock_risky = e;
            }
        }
        if let Some(u) = plugins {
            if let Some(days) = u.auto_disable_idle_days {
                if !super::plugins::valid_days(days) {
                    return Err(format!(
                        "自动停用天数 {days} 非法（0 = 关闭，或 {}–{} 天）",
                        super::plugins::AUTO_DISABLE_MIN_DAYS,
                        super::plugins::AUTO_DISABLE_MAX_DAYS
                    ));
                }
                next.plugins.auto_disable_idle_days = days;
            }
        }
        if let Some(u) = chat {
            if let Some(w) = u.content_max_width {
                if !(super::chat::CONTENT_MAX_WIDTH_MIN..=super::chat::CONTENT_MAX_WIDTH_MAX)
                    .contains(&w)
                {
                    return Err(format!(
                        "对话宽度 {w}px 超出范围（{}–{}）",
                        super::chat::CONTENT_MAX_WIDTH_MIN,
                        super::chat::CONTENT_MAX_WIDTH_MAX
                    ));
                }
                next.chat.content_max_width = Some(w);
            }
            if let Some(e) = u.expand_tool_process {
                next.chat.expand_tool_process = e;
            }
            if let Some(e) = u.enter_to_send {
                next.chat.enter_to_send = e;
            }
            if let Some(r) = u.usage_readout {
                if !super::chat::valid_usage_readout(&r) {
                    return Err(format!(
                        "不支持的用量读数口径: {r}（可选 used / remaining）"
                    ));
                }
                next.chat.usage_readout = r;
            }
            if let Some(a) = u.auto_title {
                next.chat.auto_title = a;
            }
            // M0 观测（KV v2 §7）：命中率显示开关 + 节省估算单价
            if let Some(e) = u.cache_stats {
                next.chat.cache_stats = e;
            }
            if let Some(p) = u.cache_hit_price {
                if !super::chat::valid_price(p) {
                    return Err(format!(
                        "缓存命中单价 {p} 超出范围（0–{}）",
                        super::chat::PRICE_MAX
                    ));
                }
                next.chat.cache_hit_price = p;
            }
            if let Some(p) = u.cache_miss_price {
                if !super::chat::valid_price(p) {
                    return Err(format!(
                        "缓存未命中单价 {p} 超出范围（0–{}）",
                        super::chat::PRICE_MAX
                    ));
                }
                next.chat.cache_miss_price = p;
            }
        }
        if let Some(u) = developer {
            // v2.49 开发者选项：布尔开关无范围校验；危险开关缺省全关由
            // Default 保证，这里只做显式覆盖
            if let Some(e) = u.unlocked {
                next.developer.unlocked = e;
            }
            if let Some(e) = u.allow_delete_running_session {
                next.developer.allow_delete_running_session = e;
            }
            if let Some(e) = u.allow_switch_running_session {
                next.developer.allow_switch_running_session = e;
            }
            // M0 观测：命中率告警开关 + 阈值
            if let Some(e) = u.cache_hit_alert {
                next.developer.cache_hit_alert = e;
            }
            if let Some(p) = u.cache_hit_alert_percent {
                if !super::developer::valid_alert_percent(p) {
                    return Err(format!("缓存命中率告警阈值 {p}% 超出范围（1–100）"));
                }
                next.developer.cache_hit_alert_percent = p;
            }
        }
        let bytes = serde_json::to_vec_pretty(&next).expect("settings serialize");
        std::fs::write(self.root.join(FILE_NAME), bytes)
            .map_err(|e| format!("settings.json 写入失败: {e}"))?;
        *self.inner.lock().expect("settings poisoned") = next;
        Ok(self.view())
    }
}
