//! BaseSystemInject（提示词分层设计 §4.1）：`agent/request` waterfall
//! 监听器——把**静态不可变层**（身份总纲 / 工具策略 / 安全边界 / 输出
//! 纪律 / system-reminder 合法性声明）写进 system 前缀。
//!
//! 病根（设计稿 §1）：此前聊天请求的行为指令只有三个「主动去做」的
//! 记忆/项目注入块（读 handoff、维护记忆、list 项目文件）——模型开场
//! 就查记忆、读项目文件、盘问项目阶段。本件补上「你是谁、什么优先、
//! 背景怎么对待」的总纲：**用户当前请求 > 会话上下文 > 指令文件 >
//! 背景 > 一切主动机制**；「背景不是议程」。
//!
//! **字节前缀纪律（M1 加强）**：六节全部 `include_str!` 编译期内嵌
//! （memory-tree prompts 同款），拼接结果跨进程、跨会话逐字节一致；
//! 注册在 waterfall 最前（ToolExposure 之后、Memory-Inject 之前）=
//! system 的字节前缀，**跨会话也吃供应商前缀缓存**——比旧行为更强：
//! 此前每会话 system 开头就是动态记忆块。其后注册的监听器
//! （Memory-Inject → 思考语言 → 树目录殿后）只做追加不改前缀。
//!
//! kernel 每 step 重建 payload，`system` 恒从 None 起算，无重复注入。

use std::sync::LazyLock;

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use serde_json::Value;

use crate::waterfall::{RequestPayload, points};

const SECTION_INTRO: &str = include_str!("../prompts/base/01-intro.md");
const SECTION_IDENTITY: &str = include_str!("../prompts/base/02-identity.md");
const SECTION_TOOLS: &str = include_str!("../prompts/base/03-tools.md");
const SECTION_SAFETY: &str = include_str!("../prompts/base/04-safety.md");
const SECTION_OUTPUT: &str = include_str!("../prompts/base/05-output.md");
const SECTION_CONTEXT: &str = include_str!("../prompts/base/06-context.md");

/// 静态层全文（`\n\n` 分节；进程内拼一次，后续零成本引用）。
static BASE_SYSTEM: LazyLock<String> = LazyLock::new(|| {
    [
        SECTION_INTRO,
        SECTION_IDENTITY,
        SECTION_TOOLS,
        SECTION_SAFETY,
        SECTION_OUTPUT,
        SECTION_CONTEXT,
    ]
    .join("\n\n")
});

/// 静态层全文（测试/诊断锚：跨会话字节稳定断言用）。
pub fn base_system() -> &'static str {
    &BASE_SYSTEM
}

/// 静态注入监听器（无状态——所有会话共享同一份编译期内嵌文本）。
pub struct BaseSystemInject;

#[async_trait]
impl WaterfallListener for BaseSystemInject {
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let Ok(mut req) = serde_json::from_value::<RequestPayload>(payload.clone()) else {
            return WaterfallDecision::Continue;
        };
        req.system = match req.system.take() {
            Some(prev) => Some(format!("{prev}\n\n{}", base_system())),
            None => Some(base_system().to_string()),
        };
        // to_value 失败按 fail-open 放行（与 Memory-Inject 同纪律）
        super::replace_decision(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use session_store::Message;

    fn payload() -> Value {
        serde_json::to_value(RequestPayload {
            messages: vec![Message::user("hi")],
            model: None,
            params: Value::Object(Default::default()),
            tools: Vec::new(),
            compacted: None,
            system: None,
            session_id: None,
            tree_directory: None,
        })
        .unwrap()
    }

    #[tokio::test]
    async fn injects_static_layer_as_whole_system() {
        let out = match BaseSystemInject.handle(points::REQUEST, &payload()).await {
            WaterfallDecision::Replace(v) => {
                let req: RequestPayload = serde_json::from_value(v).unwrap();
                req.system.unwrap()
            }
            _ => panic!("expected replace"),
        };
        assert!(out.starts_with("You are Locowrk"), "静态段 = system 前缀");
        assert!(out.contains("# First principles"));
        assert!(out.contains("never a task list of its own"));
        assert!(out.contains("<system-reminder>"), "合法性声明在场");
    }

    /// 字节稳定（M1 锚）：两次调用（= 两会话两轮）逐字节一致——拼接
    /// 结果只由编译期内嵌常量决定，无任何运行时输入。
    #[tokio::test]
    async fn byte_stable_across_calls() {
        let run = || async {
            match BaseSystemInject.handle(points::REQUEST, &payload()).await {
                WaterfallDecision::Replace(v) => {
                    serde_json::from_value::<RequestPayload>(v).unwrap().system
                }
                _ => panic!("expected replace"),
            }
        };
        assert_eq!(run().await, run().await);
        assert_eq!(base_system(), base_system());
    }

    /// 已有 system（其他决策方先写）= 追加不覆盖，静态段仍在最前
    /// （前缀纪律：静态段必须排字节前缀）。
    #[tokio::test]
    async fn appends_after_existing_system() {
        let mut v = payload();
        v["system"] = json!("prior section");
        match BaseSystemInject.handle(points::REQUEST, &v).await {
            WaterfallDecision::Replace(out) => {
                let req: RequestPayload = serde_json::from_value(out).unwrap();
                let system = req.system.unwrap();
                assert!(system.starts_with("prior section"));
                assert!(system.contains("You are Locowrk"));
            }
            _ => panic!("expected replace"),
        }
    }

    #[tokio::test]
    async fn non_request_key_is_noop() {
        assert_eq!(
            BaseSystemInject.handle(points::PRE_STEP, &payload()).await,
            WaterfallDecision::Continue
        );
    }
}
