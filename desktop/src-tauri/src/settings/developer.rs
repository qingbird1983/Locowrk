//! 开发者选项区（UI shell 文档 §3.2 / §七 规则 16 可配置原则②「禁止项」
//! 的容器）：**默认隐藏**的设置分区——`unlocked` 存解锁态（侧栏版本号
//! 连点 7 次置真，安卓式；分区内可锁回），其余全是危险开关（界面上写死
//! 的禁止/限制的解锁面），缺省全关——禁止项保持写死语义，解锁是显式的
//! 开发者行为。普通用户偏好不进本区（那是各常规设置分区的职责）。

use serde::{Deserialize, Serialize};

/// 缓存命中率告警阈值缺省（%）：会话累计命中率跌破此值时提醒（M0 观测，
/// KV v2 §7.4）。
pub const CACHE_HIT_ALERT_PERCENT_DEFAULT: u8 = 80;

// ---- M2 记忆树整理（树 v2 §15 开发者层级可调项；Memory-KV Roadmap）----

/// 反弹熔断 k 缺省（§5.5：整理后 k 个用户轮内再触发 = 反弹）。
pub const ORGANIZE_REBOUND_K_DEFAULT: u32 = 3;
/// 反弹熔断 m 缺省（连续 m 次反弹 → 暂停本会话自动整理）。
pub const ORGANIZE_REBOUND_M_DEFAULT: u32 = 3;
/// 熔断 k/m 合法区间（1–10）。
pub const ORGANIZE_REBOUND_RANGE: std::ops::RangeInclusive<u32> = 1..=10;
/// 记忆管理器单次调用超时缺省（秒）。
/// ⚠ **队列 Q6（CB §7.2）同调改的缺省**：60 → 30。这条与下面那条**必须一起改**——
/// 稿 §11 点名的坑就是「只调超时不调重试」：60s×(1+3)=240s 的在途盲区若只把超时
/// 砍半，仍留着 4 次尝试 = 120s，盲区没变小一半。终值 = 30×(1+2)=**最坏 90s**
/// （不变量判据 `defaults_keep_the_in_flight_window_at_90_secs`）。
/// 这不是"顺手改了存量用户的东西"：本项早已是可调项，改的是**缺省**，且方向是
/// 队列 Q6 排定并经理批准的（用户设过的值不受影响，字段级缺省只作用在未设过的键上）。
pub const MANAGER_TIMEOUT_SECS_DEFAULT: u64 = 30;
pub const MANAGER_TIMEOUT_SECS_RANGE: std::ops::RangeInclusive<u64> = 10..=600;
/// 记忆管理器重试次数缺省（仍失败 → 规则切分降级，§5.4）。
/// Q6 同调：3 → 2。**为什么不是 1**（稿 §7.2 原话）：慢网络 + 限流下少一次重试会把
/// 本来能成功的整理打成降级规则切分，而降级目录的标题质量明显不如模型产出
/// （降级支标题取自「首条用户消息前 20 字」）。
pub const MANAGER_MAX_RETRIES_DEFAULT: u32 = 2;
pub const MANAGER_MAX_RETRIES_RANGE: std::ops::RangeInclusive<u32> = 0..=10;
/// 在途收束后的连续补跑上限缺省（CB §12.2 要求的那本账 · 队列 Q6）。
/// 0 = 关掉补跑（只记标记、等下一轮自然触发）——按可配置原则②，这条限制有开关。
pub const ORGANIZE_MAX_RERUN_DEFAULT: u32 = 3;
pub const ORGANIZE_MAX_RERUN_RANGE: std::ops::RangeInclusive<u32> = 0..=10;
/// 规则切分块缺省与合法区间（§15：5K–50K 缺省 20K）。
pub const RULE_CHUNK_TOKENS_DEFAULT: u64 = 20_000;
pub const RULE_CHUNK_TOKENS_RANGE: std::ops::RangeInclusive<u64> = 5_000..=50_000;

/// 开发者选项设置（settings.json `developer` 区，字段级缺省零迁移）。
/// Default = 全关是语义要求：分区隐藏 + 禁止项保持写死语义（阈值是唯一
/// 带非零缺省的字段，只在告警开启后有意义）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct DeveloperSettings {
    /// 分区解锁态：真 = 导航轨 / ⌘K / 面板三处可见。
    pub unlocked: bool,
    /// 允许删除运行中的会话（后端忙锁守卫放行）。运行轮的余段会重建日志
    /// 文件（append 语义），开关 hint 如实说明；调度最近目标同步剔除，
    /// 不向已删会话注入触发轮。
    pub allow_delete_running_session: bool,
    /// 允许切走正在运行的会话（前端守卫放行）。半截轮次会掺入新会话转录
    /// （事件不分会话过滤），收束后切回可补全。
    pub allow_switch_running_session: bool,
    /// 缓存命中率告警（缺省关）：会话累计命中率跌破阈值时 toast 一次，
    /// 每会话至多一次；≥3 个 LLM 步才判——冷启动首请求无缓存可命，
    /// 首轮低命中率不是断裂。
    pub cache_hit_alert: bool,
    /// 前缀断裂诊断（M6.1，KV v2 §7.4 缺省关）：开启后每 step 在 AppLlm
    /// 出口算分块 hash 链与上轮比对（几次 hash 计算，无额外请求），
    /// `llm_cache_diagnose` 命令读最近一步诊断。
    pub cache_diagnostics: bool,
    /// 告警阈值（%）。
    pub cache_hit_alert_percent: u8,
    /// 记忆树反弹熔断 k（§5.5，1–10 缺省 3）。
    pub organize_rebound_k: u32,
    /// 记忆树反弹熔断 m（连续 m 次 → 暂停自动整理，1–10 缺省 3）。
    pub organize_rebound_m: u32,
    /// 记忆管理器单次调用超时（秒，10–600 缺省 60）；重试次数（0–10 缺省
    /// 3，仍失败 → 规则切分）。**持久化键沿用历史名 recorderTimeoutSecs/
    /// recorderMaxRetries**（设置文件兼容；术语注见《Memory Tree Design v2》）。
    #[serde(rename = "recorderTimeoutSecs")]
    pub memory_manager_timeout_secs: u64,
    #[serde(rename = "recorderMaxRetries")]
    pub memory_manager_max_retries: u32,
    /// 规则切分块（token，5K–50K 缺省 20K）。
    pub rule_chunk_tokens: u64,
    /// 在途收束后允许**连续补跑**几次（CB §12.2 · 队列 Q6，0–10 缺省 3）。
    /// 语义是"整理追不上对话节奏时最多再自己追三步"，不是"一共跑几次"：
    /// 任何一次新的自然触发（`after_turn` 真发起了一轮整理）都把这本账清零
    /// （`Organizer::clear_rerun_streak`）。0 = 关掉补跑，标记照记、等下一轮。
    pub organize_max_rerun: u32,
}

impl Default for DeveloperSettings {
    fn default() -> Self {
        Self {
            unlocked: false,
            allow_delete_running_session: false,
            allow_switch_running_session: false,
            cache_hit_alert: false,
            cache_diagnostics: false,
            cache_hit_alert_percent: CACHE_HIT_ALERT_PERCENT_DEFAULT,
            organize_rebound_k: ORGANIZE_REBOUND_K_DEFAULT,
            organize_rebound_m: ORGANIZE_REBOUND_M_DEFAULT,
            memory_manager_timeout_secs: MANAGER_TIMEOUT_SECS_DEFAULT,
            memory_manager_max_retries: MANAGER_MAX_RETRIES_DEFAULT,
            rule_chunk_tokens: RULE_CHUNK_TOKENS_DEFAULT,
            organize_max_rerun: ORGANIZE_MAX_RERUN_DEFAULT,
        }
    }
}

/// IPC 投影（结构即设置原值）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperView {
    pub unlocked: bool,
    pub allow_delete_running_session: bool,
    pub allow_switch_running_session: bool,
    pub cache_hit_alert: bool,
    pub cache_diagnostics: bool,
    pub cache_hit_alert_percent: u8,
    pub organize_rebound_k: u32,
    pub organize_rebound_m: u32,
    #[serde(rename = "recorderTimeoutSecs")]
    pub memory_manager_timeout_secs: u64,
    #[serde(rename = "recorderMaxRetries")]
    pub memory_manager_max_retries: u32,
    pub rule_chunk_tokens: u64,
    pub organize_max_rerun: u32,
}

/// 部分更新（settings_set 入参）：字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DeveloperUpdate {
    pub unlocked: Option<bool>,
    pub allow_delete_running_session: Option<bool>,
    pub allow_switch_running_session: Option<bool>,
    pub cache_hit_alert: Option<bool>,
    pub cache_diagnostics: Option<bool>,
    pub cache_hit_alert_percent: Option<u8>,
    pub organize_rebound_k: Option<u32>,
    pub organize_rebound_m: Option<u32>,
    #[serde(rename = "recorderTimeoutSecs")]
    pub memory_manager_timeout_secs: Option<u64>,
    #[serde(rename = "recorderMaxRetries")]
    pub memory_manager_max_retries: Option<u32>,
    pub rule_chunk_tokens: Option<u64>,
    pub organize_max_rerun: Option<u32>,
}

/// 合法性检查（update.rs 校验用）：阈值 1–100。
pub fn valid_alert_percent(v: u8) -> bool {
    (1..=100).contains(&v)
}

impl DeveloperSettings {
    /// 校验 + 应用（队列 Q6 从 `settings/update.rs` **机械搬移**而来）。
    ///
    /// 为什么搬：`update.rs` 加完本批新字段后 401 行被 `check_length` 判红（新增超限件）。
    /// 就地删注释压回 400 是躲闸，搬到这里是职责归位——一个分区的字段应用本来就该待在
    /// 该分区的文件里，`update.rs` 只做编排（批 Q4 抽 `check_instruction_kib` 是同族先例）。
    /// 搬移只做了两条变换：`next.developer.` → `self.`、缩进减一层，其余逐字节不动；
    /// 等价性证据 = 既有 developer 测试面一字未改仍然绿 + 脚本自检（归一后字符序列相等）。
    pub(crate) fn apply_update(&mut self, u: &DeveloperUpdate) -> Result<(), String> {
        // v2.49 开发者选项：布尔开关无范围校验；危险开关缺省全关由
        // Default 保证，这里只做显式覆盖
        if let Some(e) = u.unlocked {
            self.unlocked = e;
        }
        if let Some(e) = u.allow_delete_running_session {
            self.allow_delete_running_session = e;
        }
        if let Some(e) = u.allow_switch_running_session {
            self.allow_switch_running_session = e;
        }
        // M0 观测：命中率告警开关 + 阈值
        if let Some(e) = u.cache_hit_alert {
            self.cache_hit_alert = e;
        }
        // M6.1 前缀断裂诊断开关（缺省关）
        if let Some(e) = u.cache_diagnostics {
            self.cache_diagnostics = e;
        }
        if let Some(p) = u.cache_hit_alert_percent {
            if !super::developer::valid_alert_percent(p) {
                return Err(format!("缓存命中率告警阈值 {p}% 超出范围（1–100）"));
            }
            self.cache_hit_alert_percent = p;
        }
        // M2 记忆树整理（树 v2 §15 开发者层级）：熔断 k/m / 超时 /
        // 重试 / 规则切分块
        if let Some(k) = u.organize_rebound_k {
            if !super::developer::ORGANIZE_REBOUND_RANGE.contains(&k) {
                return Err(format!("反弹熔断 k {k} 超出范围（1–10）"));
            }
            self.organize_rebound_k = k;
        }
        if let Some(m) = u.organize_rebound_m {
            if !super::developer::ORGANIZE_REBOUND_RANGE.contains(&m) {
                return Err(format!("反弹熔断 m {m} 超出范围（1–10）"));
            }
            self.organize_rebound_m = m;
        }
        if let Some(secs) = u.memory_manager_timeout_secs {
            if !super::developer::MANAGER_TIMEOUT_SECS_RANGE.contains(&secs) {
                return Err(format!(
                    "记忆管理器超时 {secs}s 超出范围（{}–{} 秒）",
                    super::developer::MANAGER_TIMEOUT_SECS_RANGE.start(),
                    super::developer::MANAGER_TIMEOUT_SECS_RANGE.end()
                ));
            }
            self.memory_manager_timeout_secs = secs;
        }
        if let Some(n) = u.memory_manager_max_retries {
            if !super::developer::MANAGER_MAX_RETRIES_RANGE.contains(&n) {
                return Err(format!(
                    "记忆管理器重试次数 {n} 超出范围（{}–{}）",
                    super::developer::MANAGER_MAX_RETRIES_RANGE.start(),
                    super::developer::MANAGER_MAX_RETRIES_RANGE.end()
                ));
            }
            self.memory_manager_max_retries = n;
        }
        if let Some(n) = u.rule_chunk_tokens {
            if !super::developer::RULE_CHUNK_TOKENS_RANGE.contains(&n) {
                return Err(format!(
                    "规则切分块 {n} 超出范围（{}–{} token）",
                    super::developer::RULE_CHUNK_TOKENS_RANGE.start(),
                    super::developer::RULE_CHUNK_TOKENS_RANGE.end()
                ));
            }
            self.rule_chunk_tokens = n;
        }
        if let Some(n) = u.organize_max_rerun {
            // 出界文案的数字由 `check_max_rerun` 现算（区间只有一个出处，批 Q4 立的
            // 抽函数同族：本文件已经 399 行，就地再写一遍完整文案就是新增超限件）。
            super::developer::check_max_rerun(n)?;
            self.organize_max_rerun = n;
        }
        Ok(())
    }
}

/// 连续补跑上限的出界文案（队列 Q6）。**抽在这里而不是在 `update.rs` 里就地写**：
/// 那个文件已经 399/400 行，把八行校验再塞进去 = 新增超限件（批 Q4 同一条闸抓到同
/// 一种形状，处置也一样——调用点只给数字，区间端点由 `RANGE` 现算，不留第二个出处）。
pub fn check_max_rerun(n: u32) -> Result<(), String> {
    if ORGANIZE_MAX_RERUN_RANGE.contains(&n) {
        return Ok(());
    }
    Err(format!(
        "连续补跑上限 {n} 超出范围（{}–{}）",
        ORGANIZE_MAX_RERUN_RANGE.start(),
        ORGANIZE_MAX_RERUN_RANGE.end()
    ))
}
