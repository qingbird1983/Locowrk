//! 阶段四 demo 的进程内工具：工具不一定要来自 dlopen 插件——任何
//! `dyn Tool` 都能注册进工具目录（"一切皆插件"的反面同样成立：
//! 宿主原生工具与插件工具在主循环眼里无差别）。

use std::sync::atomic::{AtomicU32, Ordering};

use async_trait::async_trait;
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};

static HOST_VERSION: semver::Version = semver::Version::new(0, 0, 0);

/// 不可逆工具（SideEffect::Irreversible）：触发 Kernel 审批门。
pub struct DangerTool {
    spec: ToolSpec,
    calls: AtomicU32,
}

impl DangerTool {
    pub fn new() -> Self {
        Self {
            spec: ToolSpec {
                name: "danger".into(),
                description: "删除目标文件（不可逆，触发审批）".into(),
                schema: json!({"type":"object","properties":{"target":{"type":"string"}}}),
                side_effect: SideEffect::Irreversible,
            },
            calls: AtomicU32::new(0),
        }
    }

    pub fn calls(&self) -> u32 {
        self.calls.load(Ordering::Relaxed)
    }
}

impl Default for DangerTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DangerTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn plugin_version(&self) -> &semver::Version {
        &HOST_VERSION
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(json!({
            "status": "deleted",
            "target": args.get("target").cloned().unwrap_or(Value::Null)
        }))
    }
}

/// 作用域 B 的同名 echo 工具：证明同名工具在不同作用域指向不同实现。
pub struct StubEchoTool {
    spec: ToolSpec,
}

impl StubEchoTool {
    pub fn new() -> Self {
        Self {
            spec: ToolSpec {
                name: "echo".into(),
                description: "scope-B 进程内 stub（与插件版同名）".into(),
                schema: json!({}),
                side_effect: SideEffect::Pure,
            },
        }
    }
}

impl Default for StubEchoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for StubEchoTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn plugin_version(&self) -> &semver::Version {
        &HOST_VERSION
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("(empty)");
        Ok(json!({ "echoed": text, "from": "scope-b-stub" }))
    }
}
