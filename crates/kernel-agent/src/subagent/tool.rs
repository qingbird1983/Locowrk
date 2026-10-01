//! `subagent_run` 工具面（⑮）：LLM 可调用的子 Agent 入口。宿主/内核侧
//! 内置工具（同 search_tools 形态，组装点注册）；副作用标 **Irreversible**
//! （宁严勿松：一次派生 = LLM 花销 + 白名单工具执行，闸门交给人）。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};

use super::run::{self, SubagentOutcome};
use super::{SubagentGate, filter};
use crate::kernel::Kernel;

/// 组装点固定的版本身份（内置工具无插件版本，占位 0.0.0）。
static TOOL_VERSION: semver::Version = semver::Version::new(0, 0, 0);

/// 子 Agent 派生工具。持有内核 Arc（Kernel → Context → ToolSource → 本
/// 工具 → Kernel 是组装期有意成环：内核与工具目录都是进程级单例）。
pub struct SubagentTool {
    kernel: Arc<Kernel>,
    gate: Arc<SubagentGate>,
    spec: ToolSpec,
}

impl SubagentTool {
    pub fn new(kernel: Arc<Kernel>, gate: Arc<SubagentGate>) -> Self {
        Self {
            kernel,
            gate,
            spec: build_spec(),
        }
    }
}

fn build_spec() -> ToolSpec {
    ToolSpec {
        name: filter::SELF_TOOL.to_string(),
        description: "在子 Agent（sub-agent）里执行一个独立任务：它继承当前对话的上下文，\
            独立循环调用工具直到完成，只把最终摘要返回给当前对话。\
            参数：prompt（必填，交给子 Agent 的完整任务描述）；\
            tools（可选，允许子 Agent 使用的工具名白名单，缺省继承全部可用工具）；\
            max_turns（可选，子 Agent 最大步数，缺省 8、上限 32）；\
            timeout_secs（可选，超时秒数，缺省 300、上限 1800，0 = 不限）。\
            预算耗尽/超时会如实返回部分结果（status 标明），不会虚构完成。"
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "交给子 Agent 的完整任务描述" },
                "tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "子 Agent 可用工具名白名单；缺省继承全部"
                },
                "max_turns": { "type": "integer", "minimum": 1 },
                "timeout_secs": { "type": "integer", "minimum": 0 }
            },
            "required": ["prompt"]
        }),
        side_effect: SideEffect::Irreversible,
        exposure: Exposure::Always,
    }
}

#[async_trait]
impl Tool for SubagentTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn plugin_version(&self) -> &semver::Version {
        &TOOL_VERSION
    }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        // 会话身份必需（⑭ step_as 填充）：没有它就没有父转录可 fork，
        // 也没有审批标识可携带。
        let Some(parent_session_id) = ctx.session_id.as_deref() else {
            return Err(tool_err(
                "缺少会话身份（session_id）——子 Agent 只能在会话内派生",
            ));
        };

        let parsed = parse_args(args).map_err(tool_err)?;

        let outcome: SubagentOutcome = run::run_child(
            &self.kernel,
            &self.gate,
            parent_session_id,
            parsed.prompt,
            parsed.allow,
            parsed.max_turns,
            parsed.timeout,
        )
        .await;
        Ok(outcome.to_json())
    }
}

fn tool_err(message: impl Into<String>) -> ToolError {
    ToolError::Failed {
        tool: filter::SELF_TOOL.into(),
        message: message.into(),
    }
}

struct ParsedArgs {
    prompt: String,
    allow: Vec<String>,
    max_turns: u32,
    timeout: Option<Duration>,
}

fn parse_args(args: Value) -> Result<ParsedArgs, String> {
    let obj = args
        .as_object()
        .ok_or("参数必须是对象 {prompt, tools?, max_turns?, timeout_secs?}")?;

    let prompt = obj
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("prompt 必填且不能为空")?
        .to_string();

    let allow = match obj.get("tools") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) if list.is_empty() => Vec::new(),
        Some(Value::Array(list)) => list
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| "tools 数组元素必须是工具名字符串".to_string())
            })
            .collect::<Result<Vec<String>, String>>()?,
        Some(other) => return Err(format!("tools 必须是字符串数组，得到 {other}")),
    };

    let max_turns = match obj.get("max_turns") {
        None | Some(Value::Null) => run::DEFAULT_MAX_TURNS,
        Some(v) => v
            .as_u64()
            .ok_or("max_turns 必须是正整数")?
            .clamp(1, run::MAX_TURNS_CAP as u64) as u32,
    };

    let timeout = match obj.get("timeout_secs") {
        None | Some(Value::Null) => Some(Duration::from_secs(run::DEFAULT_TIMEOUT_SECS)),
        Some(v) => {
            let secs = v
                .as_u64()
                .ok_or("timeout_secs 必须是非负整数")?
                .min(run::TIMEOUT_CAP_SECS);
            // 0 = 不限（spec 声明的语义）
            if secs == 0 {
                None
            } else {
                Some(Duration::from_secs(secs))
            }
        }
    };

    Ok(ParsedArgs {
        prompt,
        allow,
        max_turns,
        timeout,
    })
}
