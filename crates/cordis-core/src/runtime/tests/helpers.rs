//! 单测公共件：测试服务、可编排的 TestPlugin、生命周期事件记录器。

use std::any::TypeId;
use std::sync::{Arc, Mutex};

use crate::context::Context;
use crate::plugin::{Plugin, PluginError};

/// 测试服务：具体类型键（§3.1）。
pub(super) struct Counter(pub(super) u32);

/// 测试服务：trait 对象键（§3.1）。
pub(super) trait Greeter: Send + Sync + 'static {
    fn greet(&self) -> String;
}

pub(super) struct En;

impl Greeter for En {
    fn greet(&self) -> String {
        "hello".into()
    }
}

pub(super) type Log = Arc<Mutex<Vec<String>>>;

pub(super) fn new_log() -> Log {
    Arc::new(Mutex::new(Vec::new()))
}

pub(super) fn push(log: &Log, tag: String) {
    log.lock().expect("log lock poisoned").push(tag);
}

/// 通用测试插件：可声明依赖、可选提供 Counter/Greeter、可在 apply/dispose 注入失败。
pub(super) struct TestPlugin {
    name: &'static str,
    log: Log,
    deps: Vec<TypeId>,
    provide_counter: Option<u32>,
    provide_greeter: bool,
    fail_apply: bool,
    fail_dispose: bool,
}

impl TestPlugin {
    pub(super) fn new(name: &'static str, log: Log) -> Self {
        Self {
            name,
            log,
            deps: Vec::new(),
            provide_counter: None,
            provide_greeter: false,
            fail_apply: false,
            fail_dispose: false,
        }
    }

    pub(super) fn depends_on_counter(mut self) -> Self {
        self.deps.push(TypeId::of::<Counter>());
        self
    }

    pub(super) fn depends_on_greeter(mut self) -> Self {
        self.deps.push(TypeId::of::<dyn Greeter>());
        self
    }

    pub(super) fn provides_counter(mut self, n: u32) -> Self {
        self.provide_counter = Some(n);
        self
    }

    pub(super) fn provides_greeter(mut self) -> Self {
        self.provide_greeter = true;
        self
    }

    pub(super) fn fails_apply(mut self) -> Self {
        self.fail_apply = true;
        self
    }

    pub(super) fn fails_dispose(mut self) -> Self {
        self.fail_dispose = true;
        self
    }
}

#[async_trait::async_trait]
impl Plugin for TestPlugin {
    fn inject(&self) -> Vec<TypeId> {
        self.deps.clone()
    }

    async fn apply(&self, ctx: &Context) -> Result<(), PluginError> {
        push(&self.log, format!("{}:apply", self.name));
        if self.fail_apply {
            return Err(PluginError::Apply("boom".into()));
        }
        if let Some(n) = self.provide_counter {
            ctx.provide(Counter(n));
        }
        if self.provide_greeter {
            ctx.provide_dyn::<dyn Greeter>(Arc::new(En));
        }
        Ok(())
    }

    async fn dispose(&self, _ctx: &Context) -> Result<(), PluginError> {
        push(&self.log, format!("{}:dispose", self.name));
        if self.fail_dispose {
            return Err(PluginError::Dispose("cleanup broke".into()));
        }
        Ok(())
    }
}

/// 生命周期事件记录器（§3.3 事件驱动验证用）。
pub(super) struct EventRecorder(pub(super) Log);

#[async_trait::async_trait]
impl crate::event_bus::Listener for EventRecorder {
    async fn on_event(&self, key: &str, _payload: &serde_json::Value) {
        push(&self.0, key.to_string());
    }
}
