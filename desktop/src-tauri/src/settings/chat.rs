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

/// `chat` 区（settings.json）。字段级缺省：旧文件无此区 → 全部缺省
/// （宽度未设置 / 不默认展开过程 / 回车发送 / 已用口径），零迁移。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatSettings {
    /// 主对话区列宽偏好（§2.7）。None = 未设置（前端按缺省 760 渲染）。
    pub content_max_width: Option<u32>,
    /// 新发轮次的工具行默认展开（§3.8 折叠铁则的唯一例外开关，缺省关；
    /// 用户手动切过的单元当轮内不被默认态覆盖）。
    pub expand_tool_process: bool,
    /// 回车发送（缺省开；关 = Enter 换行、Ctrl+Enter 发送）。
    pub enter_to_send: bool,
    /// 上下文用量读数口径（§3.12 浮层 headline：used / remaining）。
    pub usage_readout: String,
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            content_max_width: None,
            expand_tool_process: false,
            enter_to_send: true,
            usage_readout: USAGE_READOUT_USED.into(),
        }
    }
}

/// IPC 投影（无秘钥，全量展示）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatView {
    pub content_max_width: Option<u32>,
    pub expand_tool_process: bool,
    pub enter_to_send: bool,
    pub usage_readout: String,
}

/// 部分更新（settings_set 入参）。字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatUpdate {
    pub content_max_width: Option<u32>,
    pub expand_tool_process: Option<bool>,
    pub enter_to_send: Option<bool>,
    pub usage_readout: Option<String>,
}

/// 合法性检查（update.rs 校验用）。
pub fn valid_usage_readout(v: &str) -> bool {
    USAGE_READOUTS.contains(&v)
}
