//! M4 记忆树切流（树 v2 §11.2 阶段 2）：树会话尾切 + 目录追加 / 中途边界推进到下一个
//! User / 收起章节的结果换桩文本 / legacy 会话全量组装零变化 / 目录增长不动 system 前缀。
//! 本簇私有假件：`TextLlm` / `FakeTrees` / `tail_plan` / `echo_kernel` / `OutputShape`
//! （`OutputShape` 只服务本簇的两条桩文本断言 ⇒ 不上收共享区）。

use super::*;

// ---- M4 记忆树切流（树 v2 §11.2 阶段 2）：树会话组装 / legacy 零变化 ----

/// 剧本树组装源：返回固定计划（查无此会话 = legacy）。
struct FakeTrees(Mutex<std::collections::HashMap<String, TreePlan>>);

impl FakeTrees {
    fn with(id: &str, plan: TreePlan) -> Self {
        Self(Mutex::new([(id.to_string(), plan)].into_iter().collect()))
    }
}

#[async_trait]
impl TreeContextSource for FakeTrees {
    async fn plan(&self, session_id: &str) -> Option<TreePlan> {
        self.0.lock().unwrap().get(session_id).cloned()
    }
}

/// 纯文本 LLM。
struct TextLlm;

#[async_trait]
impl LlmService for TextLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            content: "ok".into(),
            tool_calls: vec![],
            usage: None,
        })
    }
}

fn tail_plan(tail_from: usize, directory: &str, collapsed: &[&str]) -> TreePlan {
    TreePlan {
        directory: directory.into(),
        tail_from_event: tail_from,
        collapsed_chapters: collapsed.iter().map(|s| s.to_string()).collect(),
    }
}

/// 组装 kernel：TextLlm + Spy 出口记录。
fn echo_kernel(ctx: &Arc<Context>) -> (Kernel, Arc<Spy>) {
    let spy = Arc::new(Spy(Mutex::new(Vec::new())));
    ctx.provide_dyn::<dyn LlmService>(Arc::new(TextLlm));
    ctx.events
        .on_waterfall(points::REQUEST, Arc::clone(&spy) as Arc<_>);
    (Kernel::new(Arc::clone(ctx), vec![]), spy)
}

#[tokio::test]
async fn tree_session_slices_to_tail_but_no_longer_injects_the_directory() {
    let ctx = Arc::new(Context::new());
    // 只挂 MemoryInject：树目录注入环（TreeDirectoryInject）已随 S3 刀③a 摘除，
    // 这一格要的读数正是"**有**目录文本也不进 system"，所以 FakeTrees 照旧交目录。
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(MemoryInject::new(
            Arc::new(DriftingMemory(std::sync::atomic::AtomicUsize::new(0))),
            Arc::new(std::sync::RwLock::new(MemoryInjectConfig::default())),
        )) as Arc<_>,
    );
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        tail_plan(
            2,
            "[memory tree] Earlier turns are organized below (oldest first); call memory_expand(chapter_id, level=1|2) to read one.\n- b2: 缓存讨论",
            &[],
        ),
    )));
    let (kernel, spy) = echo_kernel(&ctx);

    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "early".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "early reply".into(),
        tool_calls: vec![],
    });
    s.append(SessionEvent::UserMessage {
        content: "tail q".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "tail a".into(),
        tool_calls: vec![],
    });
    kernel.step_as(&mut s, "s1").await.expect("step");

    let recorded = spy.0.lock().unwrap().clone();
    let payload: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
    // 消息 = 尾巴两条；system 里**只有**记忆块——目录文本不再注入（§C 随树退役）。
    assert_eq!(payload.messages.len(), 2);
    assert!(matches!(
        &payload.messages[0],
        session_store::Message::User { content, .. } if content == "tail q"
    ));
    let system = payload.system.expect("system 在场");
    assert!(system.contains("<memory_index>"), "记忆块在场: {system}");
    assert!(
        !system.contains("[memory tree]") && !system.contains("- b2: 缓存讨论"),
        "plan() 交了目录也不许再进上下文: {system}"
    );
    assert!(
        payload.tree_directory.is_some(),
        "但树会话标记仍随 payload 走——它还管消息切片与 tail 起点，本刀只摘注入那一档"
    );
}

#[tokio::test]
async fn tree_session_midturn_boundary_advances_to_next_user() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        // 切点落在助手消息上（记忆管理器切分不硬校验的兜底场景）
        tail_plan(1, "", &[]),
    )));
    let (kernel, spy) = echo_kernel(&ctx);

    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "u1".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "a1".into(),
        tool_calls: vec![],
    });
    s.append(SessionEvent::UserMessage {
        content: "u2".into(),
        parts: Vec::new(),
    });
    kernel.step_as(&mut s, "s1").await.expect("step");

    let recorded = spy.0.lock().unwrap().clone();
    let payload: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
    assert_eq!(payload.messages.len(), 1, "前进到第一条 User：只剩 u2");
    assert!(matches!(
        &payload.messages[0],
        session_store::Message::User { content, .. } if content == "u2"
    ));
    assert_eq!(payload.system, None, "空目录不追加 system");
}

#[tokio::test]
async fn tree_session_collapsed_expand_results_become_stubs() {
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        tail_plan(0, "", &["b2"]),
    )));
    let (kernel, spy) = echo_kernel(&ctx);

    let mut s = Session::new();
    s.append(SessionEvent::UserMessage {
        content: "q".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: String::new(),
        tool_calls: vec![CallId("c1".into()), CallId("c2".into())],
    });
    for (cid, chapter) in [("c1", "b1"), ("c2", "b2")] {
        s.append(SessionEvent::ToolCall {
            call_id: CallId(cid.into()),
            tool: "memory_expand".into(),
            args: json!({ "chapter_id": chapter, "level": 1 }),
            plugin_id: None,
            plugin_version: Version::new(0, 1, 0),
            side_effect: SideEffect::Pure,
        });
        s.append(SessionEvent::ToolResult {
            call_id: CallId(cid.into()),
            output: json!({ "summary": "很长很长的原文".to_string() }),
            error: None,
        });
    }
    kernel.step_as(&mut s, "s1").await.expect("step");

    let recorded = spy.0.lock().unwrap().clone();
    let payload: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
    let mut stubbed = 0;
    for m in &payload.messages {
        if let session_store::Message::ToolResult {
            tool, args, output, ..
        } = m
        {
            assert_eq!(tool, "memory_expand");
            let id = args["chapter_id"].as_str().unwrap();
            if id == "b2" {
                assert!(
                    output.as_str().unwrap().contains("collapsed"),
                    "收起章节的结果换成桩文本"
                );
                stubbed += 1;
            } else {
                assert!(output.is_object(), "未收起章节原样保留");
            }
        }
    }
    assert_eq!(stubbed, 1, "只桩 b2");
    assert_eq!(payload.messages.len(), 4, "tool_calls ↔ 结果配对不拆");
}

#[tokio::test]
async fn legacy_sessions_keep_full_assembly() {
    // ① 组装源未注册（headless）② 计划返回 None（legacy 会话）：全量投影
    for with_source in [false, true] {
        let ctx = Arc::new(Context::new());
        if with_source {
            ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees(Mutex::new(
                std::collections::HashMap::new(),
            ))));
        }
        let (kernel, spy) = echo_kernel(&ctx);
        let mut s = Session::new();
        s.append(SessionEvent::UserMessage {
            content: "u1".into(),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: "a1".into(),
            tool_calls: vec![],
        });
        s.append(SessionEvent::UserMessage {
            content: "u2".into(),
            parts: Vec::new(),
        });
        kernel.step_as(&mut s, "s1").await.expect("step");
        let recorded = spy.0.lock().unwrap().clone();
        let payload: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
        assert_eq!(payload.messages.len(), 3);
        assert_eq!(payload.system, None);
        assert_eq!(payload.tree_directory, None);
    }
}

// 原先这里有一条 `directory_growth_keeps_system_prefix`（**S3 刀③a 删除**）：它拿
// TreeDirectoryInject 造"目录长一行"的前后两份 system，证旧前缀逐字节保留。注入环
// 摘除之后目录文本根本不再进 system（上面那条用例正面证的就是这件事），这条断言
// 失去对象——删它不是为过闸删测试。"动态段不改写静态前缀"这条纪律的继承者 =
// `memory_inject/tests/prefetch.rs` 与 `project_index.rs`（逐字节复用 + 取数次数 +
// 新鲜度档变了才重算）。

#[test]
fn tail_user_start_and_collapse_helpers() {
    let events = vec![
        SessionEvent::UserMessage {
            content: "u".into(),
            parts: Vec::new(),
        },
        SessionEvent::AssistantMessage {
            content: "a".into(),
            tool_calls: vec![],
        },
        SessionEvent::UserMessage {
            content: "u2".into(),
            parts: Vec::new(),
        },
    ];
    assert_eq!(tail_user_start(&events, 0), Some(0));
    assert_eq!(tail_user_start(&events, 1), Some(2), "半轮起点前进到 User");
    assert_eq!(tail_user_start(&events, 3), None);
    assert_eq!(tail_user_start(&events, 99), None, "越界安全");

    let msgs = vec![session_store::Message::ToolResult {
        call_id: CallId("c1".into()),
        tool: "memory_expand".into(),
        args: json!({ "chapter_id": "b9" }),
        output: json!({ "summary": "x" }),
        error: None,
    }];
    let out = collapse_expanded(msgs.clone(), &["b2".into()]);
    assert!(out[0].output_is_object(), "不在收起集 = 原样保留");
    let out = collapse_expanded(msgs, &["b9".into()]);
    assert!(out[0].output_contains_stub(), "收起集命中 = 桩文本");
}

/// 测试小工具：ToolResult output 形态断言。
trait OutputShape {
    fn output_is_object(&self) -> bool;
    fn output_contains_stub(&self) -> bool;
}

impl OutputShape for session_store::Message {
    fn output_is_object(&self) -> bool {
        matches!(self, session_store::Message::ToolResult { output, .. } if output.is_object())
    }
    fn output_contains_stub(&self) -> bool {
        matches!(self, session_store::Message::ToolResult { output, .. }
            if output.as_str().is_some_and(|s| s.contains("collapsed")))
    }
}
