//! 阶段六演示工具：内置 `send_email`（`SideEffect::Irreversible`），用于在
//! 桌面端验收审批弹窗（§13.2 / 纪律 9）。真实工具应是插件——与 DemoLlm
//! 同理，组装点的内置替身只是过渡形态，后续由插件替换，内核无感知。

use std::sync::OnceLock;

use async_trait::async_trait;
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{json, Value};

pub struct SendEmailTool;

fn spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "send_email".into(),
        description: "发送一封邮件（演示用：不可逆副作用，调用前必须审批）".into(),
        schema: json!({
            "type": "object",
            "properties": {
                "to": { "type": "string", "description": "收件人地址" },
                "subject": { "type": "string", "description": "主题" },
                "body": { "type": "string", "description": "正文" }
            },
            "required": ["to", "subject", "body"]
        }),
        side_effect: SideEffect::Irreversible,
        exposure: Default::default(),
    })
}

#[async_trait]
impl Tool for SendEmailTool {
    fn spec(&self) -> &ToolSpec {
        spec()
    }

    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(0, 1, 0);
        &V
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let to = args.get("to").and_then(Value::as_str).unwrap_or_default();
        Ok(json!({ "sent": true, "to": to }))
    }
}
