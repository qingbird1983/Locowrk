//! M1（KV v2 §4/§5）：相邻轮请求前缀一致——真实决策环 + `Spy` 出口记录，
//! 断言两轮 request 的工具目录不被打断。
//!
//! S4 刀③c 之前，这条用例还钉着 `MemoryInject` 的 system 冻结（记忆源逐次
//! 漂移、预取让首查值冻结）。记忆注入整线退役后，`agent/request` 上只剩
//! ToolExposure 会改写载荷，所以断言面收窄到工具目录：目录冻结仍是前缀缓存
//! 的前提，而 system 侧已没有生产者。

use super::*;

// ---- M1（KV v2 §4/§5）：相邻轮请求前缀一致（真实决策环集成）----

struct NoEwma;

impl GlobalEwma for NoEwma {
    fn ewma_scores(&self) -> std::collections::HashMap<String, f64> {
        std::collections::HashMap::new()
    }
}

/// 剧本 LLM：第 1 次 complete 发一个 fs_read 调用，之后纯文本收束。
struct TurnOne {
    calls: Mutex<usize>,
}

#[async_trait]
impl LlmService for TurnOne {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let mut n = self.calls.lock().unwrap();
        let calls = if *n == 0 {
            vec![call("c1", "fs_read")]
        } else {
            vec![]
        };
        *n += 1;
        drop(n);
        Ok(LlmResponse {
            content: String::new(),
            tool_calls: calls,
            usage: None,
        })
    }
}

/// 同一会话连续两轮（共三次 LLM 请求）：tools 逐字节稳定。
/// 第 2 次请求起会话内出现过 fs_read（旧逐轮 Top-N 会把它换进暴露集）——
/// 钉死让暴露集冻结，这是相邻轮前缀一致的前提。
#[tokio::test]
async fn adjacent_turns_keep_request_prefix_stable() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(TurnOne {
        calls: Mutex::new(0),
    }));
    // 决策环（注册次序 = 执行次序）：ToolExposure → Spy
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(ToolExposure::new(
            Arc::new(NoEwma),
            Arc::new(std::sync::RwLock::new(ToolExposureConfig {
                enabled: true,
                max_tools: 2,
                ..ToolExposureConfig::default()
            })),
            None,
        )) as Arc<_>,
    );
    let spy = Arc::new(Spy(Mutex::new(Vec::new())));
    ctx.events
        .on_waterfall(points::REQUEST, Arc::clone(&spy) as Arc<_>);

    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);
    let mut session = Session::new();
    // 轮 1：step 1 发 fs_read 调用（工具执行进会话使用记录），step 2 收束
    session.append(SessionEvent::UserMessage {
        content: "turn 1".into(),
        parts: Vec::new(),
    });
    kernel.step_as(&mut session, "s1").await.expect("step 1");
    kernel.step_as(&mut session, "s1").await.expect("step 2");
    // 轮 2：新用户消息
    session.append(SessionEvent::UserMessage {
        content: "turn 2".into(),
        parts: Vec::new(),
    });
    kernel.step_as(&mut session, "s1").await.expect("turn 2");

    let recorded = spy.0.lock().unwrap().clone();
    assert_eq!(recorded.len(), 3, "三次 LLM 请求各记一份出口 payload");
    let parsed: Vec<crate::RequestPayload> = recorded
        .iter()
        .map(|p| serde_json::from_str(p).expect("payload parses"))
        .collect();

    // tools 逐字节一致（含顺序）：fs_read 用过也不换入
    let tools_json: Vec<String> = parsed
        .iter()
        .map(|p| serde_json::to_string(&p.tools).unwrap())
        .collect();
    assert_eq!(tools_json[0], tools_json[1], "同轮相邻 step tools 冻结");
    assert_eq!(tools_json[1], tools_json[2], "跨轮 tools 冻结");
    let names: Vec<&str> = parsed[0].tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["fs_write", "shell"], "首查选择钉死");
}
