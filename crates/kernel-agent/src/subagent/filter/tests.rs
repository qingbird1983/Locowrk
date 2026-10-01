//! FilterToolSource 单测：空白名单继承、严格子集、自我剔除三态。
//! （单元测试拿不到 tests/common 的 mock，这里自带最小替身。）

use std::sync::Arc;

use async_trait::async_trait;
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{Value, json};

use super::FilterToolSource;
use crate::kernel::ToolSource;

struct FixedTool;

#[async_trait]
impl Tool for FixedTool {
    fn spec(&self) -> &ToolSpec {
        unreachable!("本测试只断言 list/resolve 的名字集")
    }
    fn plugin_version(&self) -> &semver::Version {
        unreachable!()
    }
    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        unreachable!()
    }
}

struct FixedSource(Vec<&'static str>);

#[async_trait]
impl ToolSource for FixedSource {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        if self.0.contains(&name) {
            Ok(Arc::new(FixedTool))
        } else {
            Err(ToolError::NotAvailable(name.to_string()))
        }
    }
    async fn list(&self) -> Vec<ToolSpec> {
        // FixedTool.spec 是 unreachable!，这里手搓 spec 只填名字
        self.0
            .iter()
            .map(|n| ToolSpec {
                name: (*n).to_string(),
                description: String::new(),
                schema: json!({}),
                side_effect: SideEffect::Pure,
                exposure: Default::default(),
            })
            .collect()
    }
}

#[tokio::test]
async fn empty_allow_inherits_all_except_self() {
    let f = FilterToolSource::new(
        Arc::new(FixedSource(vec!["echo", "send_email", "subagent_run"])),
        vec![],
    );
    let names: Vec<String> = f.list().await.into_iter().map(|s| s.name).collect();
    assert_eq!(names, vec!["echo".to_owned(), "send_email".to_owned()]);
    assert!(f.resolve("subagent_run").await.is_err(), "单层硬底线");
    assert!(f.resolve("echo").await.is_ok());
}

#[tokio::test]
async fn explicit_allow_is_strict_subset() {
    let f = FilterToolSource::new(
        Arc::new(FixedSource(vec!["echo", "send_email", "subagent_run"])),
        vec!["echo".into(), "subagent_run".into()],
    );
    let names: Vec<String> = f.list().await.into_iter().map(|s| s.name).collect();
    assert_eq!(names, vec!["echo".to_owned()]);
    assert!(f.resolve("send_email").await.is_err());
}

#[tokio::test]
async fn unknown_allow_name_resolves_not_available() {
    let f = FilterToolSource::new(Arc::new(FixedSource(vec!["echo"])), vec!["ghost".into()]);
    assert!(matches!(
        f.resolve("ghost").await,
        Err(ToolError::NotAvailable(_))
    ));
    assert!(f.list().await.is_empty());
}
