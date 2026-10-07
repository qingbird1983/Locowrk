//! 对话行为设置（UI shell 文档 §3.2 `ChatSection` / §2.7 主区列宽）。纯
//! 前端呈现偏好——后端只存取与校验，不解释；列宽偏好走 settings 不走
//! localStorage（§2.7：随设置同步与备份）。

use serde::{Deserialize, Serialize};

/// 主区列宽下限（§2.7 拖拽 / 键盘下限 560）。缺省 760 与 `--lw-content-max`
/// 的唯一真源在前端 `lib/chat-width.ts`，此处只做存取钳制。
pub const CONTENT_MAX_WIDTH_MIN: u32 = 560;
/// 列宽存取上限（§2.7 实际上限 = 主区宽 − 48，由前端 ResizeObserver 实时
/// 钳制；这里只防显然离谱的值）。
pub const CONTENT_MAX_WIDTH_MAX: u32 = 4_000;
/// 上下文用量读数口径：已用（缺省，§3.12 浮层 headline）。
pub const USAGE_READOUT_USED: &str = "used";
/// 上下文用量读数口径：剩余。
pub const USAGE_READOUT_REMAINING: &str = "remaining";
const USAGE_READOUTS: [&str; 2] = [USAGE_READOUT_USED, USAGE_READOUT_REMAINING];
/// 缓存命中单价缺省（元/百万 tokens，deepseek-flash 空闲价；KV v2 §7.2）。
pub const CACHE_HIT_PRICE_DEFAULT: f64 = 0.02;
/// 缓存未命中单价缺省（元/百万 tokens）。价差 = 省钱估算的来源（命中约
/// 为未命中的 1/50）。
pub const CACHE_MISS_PRICE_DEFAULT: f64 = 1.0;
/// 单价存取上限（元/百万 tokens）：只防显然离谱的值，真实价格远低于此。
pub const PRICE_MAX: f64 = 10_000.0;
/// 长消息折叠行数下限（组件规格 §六：3 行）。
pub const FOLD_LINES_MIN: u32 = 3;
/// 长消息折叠行数上限（组件规格 §六：20 行）。
pub const FOLD_LINES_MAX: u32 = 20;

/// `chat` 区（settings.json）。字段级缺省：旧文件无此区 → 全部缺省
/// （宽度未设置 / 过程组展开仅明细收起 / 回车发送 / 已用口径 / 自动标题开），零迁移。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatSettings {
    /// 主对话区列宽偏好（§2.7）。None = 未设置（前端按缺省 760 渲染）。
    pub content_max_width: Option<u32>,
    /// 折叠工具执行过程（v4.20 改名+语义反转，缺省关 = 过程组默认展开、
    /// 唯一被默认折叠的层是工具明细；开 = 过程组折叠为一行摘要 = 完整折叠
    /// 三层。旧 `expandToolProcess` 键退役 = 幻影键，字段级缺省零迁移）。
    pub collapse_tool_process: bool,
    /// 回车发送（缺省开；关 = Enter 换行、Ctrl+Enter 发送）。
    pub enter_to_send: bool,
    /// 上下文用量读数口径（§3.12 浮层 headline：used / remaining）。
    pub usage_readout: String,
    /// 自动会话标题（v2.57，缺省开）：首轮收束后旁车无标题时一次 LLM
    /// 调用生成 ≤25 字标题（替代首条截断）；用户手动重命名过（旁车有
    /// title 字段）永不覆盖。关 = 恢复首条截断。
    pub auto_title: bool,
    /// 缓存命中率/节省显示（M0 观测，KV v2 §7，缺省开）：用量浮层
    /// 「会话累计」行的显隐。
    pub cache_stats: bool,
    /// 缓存命中单价（元/百万 tokens）：节省估算 = 命中 × (未命中价 − 命中价)。
    pub cache_hit_price: f64,
    /// 缓存未命中单价（元/百万 tokens）。
    pub cache_miss_price: f64,
    /// 长消息折叠（组件规格「Line Clamp with Fade Mask」，缺省开）：用户
    /// 消息与助手长文超过 `fold_lines` 行自动收起（渐变遮罩 + 圆形展开钮）。
    pub fold_long_messages: bool,
    /// 长消息折叠行数（3–20，缺省 5）。
    pub fold_lines: u32,
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            content_max_width: None,
            collapse_tool_process: false,
            enter_to_send: true,
            usage_readout: USAGE_READOUT_USED.into(),
            auto_title: true,
            cache_stats: true,
            cache_hit_price: CACHE_HIT_PRICE_DEFAULT,
            cache_miss_price: CACHE_MISS_PRICE_DEFAULT,
            fold_long_messages: true,
            fold_lines: 5,
        }
    }
}

/// IPC 投影（无秘钥，全量展示）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatView {
    pub content_max_width: Option<u32>,
    pub collapse_tool_process: bool,
    pub enter_to_send: bool,
    pub usage_readout: String,
    pub auto_title: bool,
    pub cache_stats: bool,
    pub cache_hit_price: f64,
    pub cache_miss_price: f64,
    pub fold_long_messages: bool,
    pub fold_lines: u32,
}

/// 部分更新（settings_set 入参）。字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatUpdate {
    pub content_max_width: Option<u32>,
    pub collapse_tool_process: Option<bool>,
    pub enter_to_send: Option<bool>,
    pub usage_readout: Option<String>,
    pub auto_title: Option<bool>,
    pub cache_stats: Option<bool>,
    pub cache_hit_price: Option<f64>,
    pub cache_miss_price: Option<f64>,
    pub fold_long_messages: Option<bool>,
    pub fold_lines: Option<u32>,
}

/// 合法性检查（update.rs 校验用）。
pub fn valid_usage_readout(v: &str) -> bool {
    USAGE_READOUTS.contains(&v)
}

/// 单价合法性：有限且在 [0, PRICE_MAX]。
pub fn valid_price(v: f64) -> bool {
    v.is_finite() && (0.0..=PRICE_MAX).contains(&v)
}

/// 折叠行数合法性（组件规格 §六：3–20 行）。
pub fn valid_fold_lines(v: u32) -> bool {
    (FOLD_LINES_MIN..=FOLD_LINES_MAX).contains(&v)
}

/// chat 区部分更新（update.rs 编排调用）：本区校验与落值收敛在区模块内，
/// 编排层只留一行转发。
pub fn apply_update(next: &mut ChatSettings, u: ChatUpdate) -> Result<(), String> {
    if let Some(w) = u.content_max_width {
        if !(CONTENT_MAX_WIDTH_MIN..=CONTENT_MAX_WIDTH_MAX).contains(&w) {
            return Err(format!(
                "对话宽度 {w}px 超出范围（{CONTENT_MAX_WIDTH_MIN}–{CONTENT_MAX_WIDTH_MAX}）"
            ));
        }
        next.content_max_width = Some(w);
    }
    if let Some(e) = u.collapse_tool_process {
        next.collapse_tool_process = e;
    }
    if let Some(e) = u.enter_to_send {
        next.enter_to_send = e;
    }
    if let Some(r) = u.usage_readout {
        if !valid_usage_readout(&r) {
            return Err(format!(
                "不支持的用量读数口径: {r}（可选 used / remaining）"
            ));
        }
        next.usage_readout = r;
    }
    if let Some(a) = u.auto_title {
        next.auto_title = a;
    }
    // M0 观测（KV v2 §7）：命中率显示开关 + 节省估算单价
    if let Some(e) = u.cache_stats {
        next.cache_stats = e;
    }
    if let Some(p) = u.cache_hit_price {
        if !valid_price(p) {
            return Err(format!("缓存命中单价 {p} 超出范围（0–{PRICE_MAX}）"));
        }
        next.cache_hit_price = p;
    }
    if let Some(p) = u.cache_miss_price {
        if !valid_price(p) {
            return Err(format!("缓存未命中单价 {p} 超出范围（0–{PRICE_MAX}）"));
        }
        next.cache_miss_price = p;
    }
    // 长消息折叠（组件规格）：开关 + 行数（3–20）
    if let Some(e) = u.fold_long_messages {
        next.fold_long_messages = e;
    }
    if let Some(n) = u.fold_lines {
        if !valid_fold_lines(n) {
            return Err(format!(
                "折叠行数 {n} 超出范围（{FOLD_LINES_MIN}–{FOLD_LINES_MAX}）"
            ));
        }
        next.fold_lines = n;
    }
    Ok(())
}
