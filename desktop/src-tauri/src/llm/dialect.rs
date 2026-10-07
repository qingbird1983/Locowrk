//! 推理档位方言（批 B，供应商稿 §5）：档位梯子到各家请求字段的映射是
//! **数据选表 + 每方言一个纯函数**——新增一家等于加一个枚举值加一个分支
//! 加预设表一行，不碰请求管线。映射不了的组合宁缺勿假（返回 None 不发）。
//! 梯子取方言自带档位集（可发送口径）；目录 `reasoning_options` 只作展示
//! 参考，目录档位在方言表里没有写法的一律不渲染不发送（§5.1）。

use serde_json::{json, Value};

use super::catalog_match;

/// 推理档位方言（供应商稿 §3.2）。`None` = 无梯子：选择器隐藏、不发字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// `reasoning_effort: "minimal|low|medium|high"`（OpenAI gpt-5/o 系）。
    OpenAiEffort,
    /// `reasoning_effort`（Anthropic 官方兼容层映射；批 B 走查核实）。
    AnthropicEffort,
    /// `thinking: {type: "enabled"|"disabled"}`（GLM 系）。
    GlmThinking,
    /// `enable_thinking: true|false`（Qwen3 系）。
    QwenEnable,
    /// `thinking:{type}` + `reasoning_effort:low|high|max` 双字段
    /// （DeepSeek 官方 thinking_mode 指南，2026-10 核）。
    DeepseekThinking,
    None,
}

/// 预设 baseUrl host → 方言（与 catalog_match 别名表同源一张表）。
pub fn dialect_of(base_url: Option<&str>) -> Dialect {
    let Some(url) = base_url.map(str::trim).filter(|s| !s.is_empty()) else {
        return Dialect::None;
    };
    let host = catalog_match::host_of(url);
    match catalog_match::dialect_of_host(&host) {
        Some("openai_effort") => Dialect::OpenAiEffort,
        Some("anthropic_effort") => Dialect::AnthropicEffort,
        Some("glm_thinking") => Dialect::GlmThinking,
        Some("qwen_enable") => Dialect::QwenEnable,
        Some("deepseek_thinking") => Dialect::DeepseekThinking,
        _ => Dialect::None,
    }
}

impl Dialect {
    /// 方言自带梯子（可发送口径；None = 空 = 选择器隐藏）。
    pub fn ladder(self) -> &'static [&'static str] {
        match self {
            Dialect::OpenAiEffort => &["minimal", "low", "medium", "high"],
            Dialect::AnthropicEffort => &["low", "medium", "high"],
            Dialect::GlmThinking | Dialect::QwenEnable => &["disabled", "enabled"],
            Dialect::DeepseekThinking => &["disabled", "low", "high", "max"],
            Dialect::None => &[],
        }
    }

    /// 辅助调用降档（§5.4）：梯子最低档（思考可关直达 disabled；纯 effort
    /// 型取最低 effort）。None = 无梯子（辅助调用不发字段）。
    pub fn aux_level(self) -> Option<&'static str> {
        self.ladder().first().copied()
    }

    /// 语义档位 → 请求 body patch（字段名 → 值）。映射不了 = None（调用方
    /// 不发该字段，宁缺勿假）。DeepSeek 官方归一表：minimal→low、
    /// medium/high→high、xhigh/ultra→max（thinking 模式忽略 temperature/
    /// presence/frequency_penalty——我们不发，无冲突）。
    pub fn patch(self, level: &str) -> Option<Vec<(&'static str, Value)>> {
        let lvl = level.trim().to_lowercase();
        if lvl.is_empty() {
            return None;
        }
        match self {
            Dialect::OpenAiEffort => {
                if self.ladder().contains(&lvl.as_str()) {
                    Some(vec![("reasoning_effort", json!(lvl))])
                } else {
                    None
                }
            }
            Dialect::AnthropicEffort => {
                if self.ladder().contains(&lvl.as_str()) {
                    Some(vec![("reasoning_effort", json!(lvl))])
                } else {
                    None
                }
            }
            Dialect::GlmThinking => match lvl.as_str() {
                "disabled" => Some(vec![("thinking", json!({"type": "disabled"}))]),
                "enabled" => Some(vec![("thinking", json!({"type": "enabled"}))]),
                _ => None,
            },
            Dialect::QwenEnable => match lvl.as_str() {
                "disabled" => Some(vec![("enable_thinking", json!(false))]),
                "enabled" => Some(vec![("enable_thinking", json!(true))]),
                _ => None,
            },
            Dialect::DeepseekThinking => {
                let effort = match lvl.as_str() {
                    "minimal" => "low",
                    "medium" | "high" => "high",
                    "xhigh" | "ultra" => "max",
                    other => other,
                };
                match effort {
                    "disabled" => Some(vec![("thinking", json!({"type": "disabled"}))]),
                    "low" | "high" | "max" => Some(vec![
                        ("thinking", json!({"type": "enabled"})),
                        ("reasoning_effort", json!(effort)),
                    ]),
                    _ => None,
                }
            }
            Dialect::None => None,
        }
    }

    /// clamp（§5.2 解析链）：档位在梯子里原样；不在 = 就近降档；梯子里
    /// 没有更低档 = None（剥除，不猜）。梯子空（无方言）恒 None。
    pub fn clamp(self, level: &str) -> Option<&'static str> {
        let ladder = self.ladder();
        if ladder.is_empty() {
            return None;
        }
        let lvl = level.trim().to_lowercase();
        if let Some(i) = ladder.iter().position(|r| *r == lvl) {
            return Some(ladder[i]);
        }
        // 就近降档：梯子里最后一个低于请求档的语义位（语义序即梯子序）
        let req = semantic_index(&lvl)?;
        let mut best: Option<&'static str> = None;
        for r in ladder {
            if semantic_index(r).is_some_and(|ri| ri < req) {
                best = Some(r);
            }
        }
        best
    }
}

/// 语义档位的规范序（clamp 就近降档的比较基准）。
fn semantic_index(s: &str) -> Option<usize> {
    const ORDER: [&str; 8] = [
        "disabled", "minimal", "low", "medium", "high", "xhigh", "max", "enabled",
    ];
    ORDER.iter().position(|o| *o == s)
}

#[cfg(test)]
mod tests;
