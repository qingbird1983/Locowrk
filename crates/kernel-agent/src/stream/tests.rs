//! 子内核 delta 压制单元测试（child_kernel 是 pub(crate)，集成测试够不到）：
//! scoped 空实现命中 → 全局 sink 收不到子 delta（⑮ 黑盒语义在 ⑳ 保持）。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use std::sync::Arc as StdArc;

use async_trait::async_trait;
use cordis_core::Context;
use plugin_api::{ToolError, ToolSpec};
use session_store::{Session, SessionEvent};

use super::{LlmStreamSink, NoLlmStreamSink};
use crate::kernel::{
    Kernel, LlmError, LlmRequest, LlmResponse, LlmService, StepOutcome, ToolSource,
};

struct EmptySource;

#[async_trait]
impl ToolSource for EmptySource {
    async fn resolve(
        &self,
        _session_id: Option<&str>,
        name: &str,
    ) -> Result<StdArc<dyn plugin_api::Tool>, ToolError> {
        Err(ToolError::NotAvailable(name.into()))
    }
    async fn list(&self, _session_id: Option<&str>) -> Vec<ToolSpec> {
        Vec::new()
    }
}

struct CountingLlm {
    streaming_calls: AtomicUsize,
}

#[async_trait]
impl LlmService for CountingLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        unreachable!("sink 在册应走 complete_streaming")
    }

    async fn complete_streaming(
        &self,
        _req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.streaming_calls.fetch_add(1, Ordering::Relaxed);
        on_delta("子内 delta");
        Ok(LlmResponse {
            content: "子内正文".into(),
            tool_calls: vec![],
            usage: None,
        })
    }
}

struct RecordedSink(Mutex<Vec<String>>);

impl LlmStreamSink for RecordedSink {
    fn on_delta(&self, _: Option<&str>, delta: &str) -> bool {
        self.0.lock().unwrap().push(delta.into());
        true
    }
}

#[test]
fn child_kernel_shadows_global_sink() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let ctx = Arc::new(Context::new());
        ctx.provide_dyn::<dyn LlmService>(Arc::new(CountingLlm {
            streaming_calls: AtomicUsize::new(0),
        }));
        let sink = Arc::new(RecordedSink(Mutex::new(Vec::new())));
        ctx.provide_dyn::<dyn LlmStreamSink>(sink.clone() as Arc<dyn LlmStreamSink>);
        ctx.provide_dyn::<dyn ToolSource>(Arc::new(EmptySource));

        let parent = Kernel::new(Arc::clone(&ctx), vec![]);
        let child = parent.child_kernel(vec![]).unwrap();
        let mut session = Session::new();
        session.append(SessionEvent::UserMessage {
            content: "任务".into(),
        });
        let outcome = child.step_as(&mut session, "p1-sub-1").await.unwrap();

        assert_eq!(
            outcome,
            StepOutcome::Continue {
                streamed: false,
                reasoned: false
            }
        );
        assert!(sink.0.lock().unwrap().is_empty(), "父 UI 不得收到子 delta");
    });
    // NoLlmStreamSink 本体是空实现（直接锁定契约）
    NoLlmStreamSink.on_delta(Some("x"), "ignored");
}
