//! M1（KV v2 §4/§5）：相邻轮请求前缀一致——真实决策环 + `DriftingMemory`（每次 recall
//! 变一点）+ `Spy` 出口记录，断言两轮 request 的公共前缀不被打断。

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

/// 同一会话连续两轮（共三次 LLM 请求）：tools 与 system 逐字节稳定。
/// 第 2 次请求起会话内出现过 fs_read（旧逐轮 Top-N 会把它换进暴露集）、
/// 记忆源逐次漂移（旧逐轮检索会改写 system）——钉死 + 预取让两者冻结，
/// 这是 M4 树模式 A/B 数据可信的前提。
#[tokio::test]
async fn adjacent_turns_keep_request_prefix_stable() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(ProbeSource(specs())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(TurnOne {
        calls: Mutex::new(0),
    }));
    // 决策环（注册次序 = 执行次序）：ToolExposure → Memory-Inject → Spy
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
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(MemoryInject::new(
            Arc::new(DriftingMemory(std::sync::atomic::AtomicUsize::new(0))),
            Arc::new(std::sync::RwLock::new(MemoryInjectConfig::default())),
        )),
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

    // system（记忆块）逐字节一致：三次都是首查内容
    let systems: Vec<Option<String>> = parsed.iter().map(|p| p.system.clone()).collect();
    assert!(systems[0].is_some(), "记忆块已注入");
    assert_eq!(systems[0], systems[1], "相邻 step system 冻结");
    assert_eq!(systems[1], systems[2], "跨轮 system 冻结");
    assert!(
        systems[0]
            .as_ref()
            .unwrap()
            .contains("- [drift](drift.md) — 0"),
        "块内容 = 首查值（预取后不再漂移）"
    );
}
