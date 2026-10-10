//! `subagent_run` 工具面（⑮）：LLM 可调用的子 Agent 入口。宿主/内核侧
//! 内置工具（同 search_tools 形态，组装点注册）；副作用标 **Pure**
//! （v2.145 用户裁定推翻 v2.88 的 Irreversible：派生本身不动用户世界，
//! 子工具各有其门——ZCode Agent 工具 `readOnly/needsApproval:false`
//! 同口径；副产品 = Plan 档可派生只读研究子代理，子目录被 Plan 围栏
//! Pure-only，天然安全）。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};

use super::run::{self, SubagentOutcome};
use super::{SubTranscriptSink, SubagentGate, filter};
use crate::kernel::Kernel;

/// 组装点固定的版本身份（内置工具无插件版本，占位 0.0.0）。
static TOOL_VERSION: semver::Version = semver::Version::new(0, 0, 0);

/// 子 Agent 派生工具。持有内核 Arc（Kernel → Context → ToolSource → 本
/// 工具 → Kernel 是组装期有意成环：内核与工具目录都是进程级单例）。
pub struct SubagentTool {
    kernel: Arc<Kernel>,
    gate: Arc<SubagentGate>,
    spec: ToolSpec,
    /// §3.16 子会话转录接收器：宿主可选注入（桌面写 sub-transcripts/）；
    /// None = 纯黑盒（headless / 未接组装点），行为与 ⑮ 一致。
    transcript_sink: Option<Arc<dyn SubTranscriptSink>>,
    /// 批 F 子代理模型面工厂：每子 run 造「带回退链的 LlmService + 报告」
    /// 换入子内核作用域；None = 子回退全局服务（⑮ 以来行为不变）。
    child_model: Option<super::ChildModelFactory>,
}

impl SubagentTool {
    pub fn new(kernel: Arc<Kernel>, gate: Arc<SubagentGate>) -> Self {
        Self {
            kernel,
            gate,
            spec: build_spec(),
            transcript_sink: None,
            child_model: None,
        }
    }

    /// 注入子转录接收器（组装点专用；§8.2「子会话落盘」通路）。
    pub fn with_transcript_sink(mut self, sink: Arc<dyn SubTranscriptSink>) -> Self {
        self.transcript_sink = Some(sink);
        self
    }

    /// 注入子代理模型面工厂（批 F：子代理模型档 + 终端错误回退链）。
    pub fn with_child_model_factory(mut self, factory: super::ChildModelFactory) -> Self {
        self.child_model = Some(factory);
        self
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
            预算耗尽/超时会如实返回部分结果（status 标明），不会虚构完成。\
            重要任务收束后可用检索工具找 subagent_verify 对结果做独立验收\
            （子代理自述完成 ≠ 完成）。"
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
        side_effect: SideEffect::Pure,
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

        let outcome: SubagentOutcome = run::run_child(run::ChildRunParams {
            parent: &self.kernel,
            gate: &self.gate,
            parent_session_id,
            prompt: parsed.prompt,
            allow: parsed.allow,
            max_turns: parsed.max_turns,
            timeout: parsed.timeout,
            transcript_sink: self.transcript_sink.as_deref(),
            child_model: self.child_model.as_ref(),
        })
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
