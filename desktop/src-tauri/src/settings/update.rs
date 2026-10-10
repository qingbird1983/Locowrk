//! settings_set 的校验与应用（settings.json 各分区一一对应；从
//! settings.rs 拆出只为本体守住 400 行纪律——逻辑不变）。
//!
//! 顺序纪律：校验全部通过 → 应用到副本 → 落盘成功才提交内存。任何
//! 校验失败提前返回 Err，不污染内存态（测试 `rejects_*` 系列锁定）。

use super::{
    AppearanceUpdate, ChatUpdate, ContextUpdate, DeveloperUpdate, EmbeddingUpdate, LlmUpdate,
    MemoryUpdate, MultimodalUpdate, PluginsUpdate, SettingsStore, SettingsView, ThinkingUpdate,
    ToolsUpdate, FILE_NAME, LOCALES, THINKING_LANGUAGE_VALUES,
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
            // v2.137 整理阈值（绝对 token，100K–800K）：折算键
            // threshold_pct / strategy / stability_rounds 随 ctx-compact
            // 桌面链退役（settings 里为幻影键）。
            if let Some(n) = u.threshold_tokens {
                if !super::context::valid_organize_threshold(n) {
                    return Err(format!(
                        "整理阈值 {n} 超出范围（{}–{} token）",
                        super::context::ORGANIZE_THRESHOLD_RANGE.start(),
                        super::context::ORGANIZE_THRESHOLD_RANGE.end()
                    ));
                }
                next.context.threshold_tokens = n;
            }
            if let Some(v) = u.usage_bar {
                next.context.usage_bar = v;
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
            // M2 记忆树整理三键（树 v2 §4.3/§5.2/§15）：节奏白名单 +
            // 增量块区间 + 记忆管理器模型（空串按清除处理，回落同主模型）
            if let Some(p) = u.organize_pacing {
                if !super::memory::ORGANIZE_PACING_VALUES.contains(&p.as_str()) {
                    return Err(format!("整理节奏 {p:?} 非法（可选 batch / incremental）"));
                }
                next.memory.organize_pacing = p;
            }
            if let Some(n) = u.organize_chunk_tokens {
                if !super::memory::ORGANIZE_CHUNK_TOKENS_RANGE.contains(&n) {
                    return Err(format!(
                        "增量块 {n} 超出范围（{}–{} token）",
                        super::memory::ORGANIZE_CHUNK_TOKENS_RANGE.start(),
                        super::memory::ORGANIZE_CHUNK_TOKENS_RANGE.end()
                    ));
                }
                next.memory.organize_chunk_tokens = n;
            }
            // 记忆管理器模型（空串 = 清除，回落同主模型；与 base_url/api_key
            // 同约定——双层 Option 的 null 在 JSON 里无法表达清除）
            if let Some(m) = u.memory_manager_model {
                next.memory.memory_manager_model = (!m.trim().is_empty()).then_some(m);
            }
            if let Some(p) = u.memory_manager_provider {
                next.memory.memory_manager_provider = (!p.trim().is_empty()).then_some(p);
            }
            // M4 尾巴策略 + collapse（树 v2 §7.2/§8.3/§15）：策略白名单 +
            // 窗口轮数区间 + 收起阈值区间
            if let Some(p) = u.tail_strategy {
                if !super::memory::TAIL_STRATEGY_VALUES.contains(&p.as_str()) {
                    return Err(format!("尾巴策略 {p:?} 非法（可选 boundary / window）"));
                }
                next.memory.tail_strategy = p;
            }
            if let Some(n) = u.tail_window_turns {
                if !super::memory::TAIL_WINDOW_TURNS_RANGE.contains(&n) {
                    return Err(format!(
                        "固定窗口 {n} 超出范围（{}–{} 轮）",
                        super::memory::TAIL_WINDOW_TURNS_RANGE.start(),
                        super::memory::TAIL_WINDOW_TURNS_RANGE.end()
                    ));
                }
                next.memory.tail_window_turns = n;
            }
            if let Some(e) = u.collapse_enabled {
                next.memory.collapse_enabled = e;
            }
            if let Some(p) = u.collapse_threshold_pct {
                if !super::memory::COLLAPSE_THRESHOLD_PCT_RANGE.contains(&p) {
                    return Err(format!(
                        "自动收起阈值 {p}% 超出范围（{}–{}%）",
                        super::memory::COLLAPSE_THRESHOLD_PCT_RANGE.start(),
                        super::memory::COLLAPSE_THRESHOLD_PCT_RANGE.end()
                    ));
                }
                next.memory.collapse_threshold_pct = p;
            }
            if let Some(e) = u.files {
                next.memory.files = e;
            }
            if let Some(e) = u.repo_context {
                next.memory.repo_context = e;
            }
            if let Some(e) = u.instruction_files {
                next.memory.instruction_files = e;
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
            if let Some(secs) = u.ask_timeout_secs {
                // v4.11 §3.39①：ask_user 守夜时长可调（0 = 不限等待）
                if !super::tools::ASK_TIMEOUT_RANGE.contains(&secs) {
                    return Err(format!(
                        "选择等待时长 {secs}s 非法（0 = 不限，或 {}–{} 秒）",
                        super::tools::ASK_TIMEOUT_RANGE.start(),
                        super::tools::ASK_TIMEOUT_RANGE.end()
                    ));
                }
                next.tools.ask_timeout_secs = secs;
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
            super::chat::apply_update(&mut next.chat, u)?;
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
            // M6.1 前缀断裂诊断开关（缺省关）
            if let Some(e) = u.cache_diagnostics {
                next.developer.cache_diagnostics = e;
            }
            if let Some(p) = u.cache_hit_alert_percent {
                if !super::developer::valid_alert_percent(p) {
                    return Err(format!("缓存命中率告警阈值 {p}% 超出范围（1–100）"));
                }
                next.developer.cache_hit_alert_percent = p;
            }
            // M2 记忆树整理（树 v2 §15 开发者层级）：熔断 k/m / 超时 /
            // 重试 / 规则切分块
            if let Some(k) = u.organize_rebound_k {
                if !super::developer::ORGANIZE_REBOUND_RANGE.contains(&k) {
                    return Err(format!("反弹熔断 k {k} 超出范围（1–10）"));
                }
                next.developer.organize_rebound_k = k;
            }
            if let Some(m) = u.organize_rebound_m {
                if !super::developer::ORGANIZE_REBOUND_RANGE.contains(&m) {
                    return Err(format!("反弹熔断 m {m} 超出范围（1–10）"));
                }
                next.developer.organize_rebound_m = m;
            }
            if let Some(secs) = u.memory_manager_timeout_secs {
                if !super::developer::MANAGER_TIMEOUT_SECS_RANGE.contains(&secs) {
                    return Err(format!(
                        "记忆管理器超时 {secs}s 超出范围（{}–{} 秒）",
                        super::developer::MANAGER_TIMEOUT_SECS_RANGE.start(),
                        super::developer::MANAGER_TIMEOUT_SECS_RANGE.end()
                    ));
                }
                next.developer.memory_manager_timeout_secs = secs;
            }
            if let Some(n) = u.memory_manager_max_retries {
                if !super::developer::MANAGER_MAX_RETRIES_RANGE.contains(&n) {
                    return Err(format!(
                        "记忆管理器重试次数 {n} 超出范围（{}–{}）",
                        super::developer::MANAGER_MAX_RETRIES_RANGE.start(),
                        super::developer::MANAGER_MAX_RETRIES_RANGE.end()
                    ));
                }
                next.developer.memory_manager_max_retries = n;
            }
            if let Some(n) = u.rule_chunk_tokens {
                if !super::developer::RULE_CHUNK_TOKENS_RANGE.contains(&n) {
                    return Err(format!(
                        "规则切分块 {n} 超出范围（{}–{} token）",
                        super::developer::RULE_CHUNK_TOKENS_RANGE.start(),
                        super::developer::RULE_CHUNK_TOKENS_RANGE.end()
                    ));
                }
                next.developer.rule_chunk_tokens = n;
            }
        }
        if let Some(u) = thinking {
            // 思考语言（docs/design/NEW/language of thought.md）：白名单
            // 校验。会话内锁死由注入侧缓存保证——中途改这里不影响已开
            // 会话的 system 前缀，新会话才吃新值。
            if !THINKING_LANGUAGE_VALUES.contains(&u.language.as_str()) {
                return Err(format!(
                    "思考语言 {:?} 非法（可选 follow_ui / zh-CN / en-US / none）",
                    u.language
                ));
            }
            next.thinking.language = u.language;
        }
        if let Some(u) = multimodal {
            // 多模态区（批 D）：数值界 + 白名单格式校验在
            // MultimodalSettings::validate；失败不污染内存态
            if let Some(v) = u.max_dimension {
                next.multimodal.max_dimension = v;
            }
            if let Some(v) = u.max_size_mb {
                next.multimodal.max_size_mb = v;
            }
            if let Some(v) = u.image_formats {
                next.multimodal.image_formats = v.iter().map(|s| s.trim().to_lowercase()).collect();
            }
            if let Some(v) = u.file_formats {
                next.multimodal.file_formats = v.iter().map(|s| s.trim().to_lowercase()).collect();
            }
            if let Some(v) = u.auto_gc {
                next.multimodal.auto_gc = v;
            }
            if let Some(v) = u.gc_threshold_days {
                next.multimodal.gc_threshold_days = v;
            }
            next.multimodal.validate()?;
        }
        let bytes = serde_json::to_vec_pretty(&next).expect("settings serialize");
        std::fs::write(self.root.join(FILE_NAME), bytes)
            .map_err(|e| format!("settings.json 写入失败: {e}"))?;
        *self.inner.lock().expect("settings poisoned") = next;
        Ok(self.view())
    }
}
