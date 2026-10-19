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
async fn tree_session_slices_to_tail_and_appends_directory() {
    let ctx = Arc::new(Context::new());
    // Memory-Inject 在前 → TreeDirectoryInject 在后（目录在记忆块之后）
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(MemoryInject::new(
            Arc::new(DriftingMemory(std::sync::atomic::AtomicUsize::new(0))),
            Arc::new(std::sync::RwLock::new(MemoryInjectConfig::default())),
        )) as Arc<_>,
    );
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(crate::TreeDirectoryInject) as Arc<_>,
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
    // 消息 = 尾巴两条；system = 记忆块 + 目录（次序 = 树 v2 §7.1）
    assert_eq!(payload.messages.len(), 2);
    assert!(matches!(
        &payload.messages[0],
        session_store::Message::User { content, .. } if content == "tail q"
    ));
    let system = payload.system.expect("system 在场");
    let mem_at = system.find("<memory_index>").expect("记忆块在前");
    let tree_at = system.find("[memory tree]").expect("目录在后");
    assert!(mem_at < tree_at, "目录必须排在记忆块之后（§7.1 [2]→[3]）");
    assert!(
        system.ends_with("- b2: 缓存讨论"),
        "目录挂 system 尾部（清单行收尾，无闭合标签——闭合标签会随追加移动破坏前缀）"
    );
    assert!(payload.tree_directory.is_some(), "树会话标记随 payload 走");
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

#[tokio::test]
async fn directory_growth_keeps_system_prefix() {
    // 只增不改（纪律 3）：章节 b3 追加后，旧 system 必须是新 system 的字节前缀
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        tail_plan(
            1,
            "[memory tree] Earlier turns are organized below.\n- b2: 旧章节",
            &[],
        ),
    )));
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(crate::TreeDirectoryInject) as Arc<_>,
    );
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
    kernel.step_as(&mut s, "s1").await.expect("step 1");

    // b3 出现：目录追加一行
    ctx.provide_dyn::<dyn TreeContextSource>(Arc::new(FakeTrees::with(
        "s1",
        tail_plan(
            1,
            "[memory tree] Earlier turns are organized below.\n- b2: 旧章节\n- b3: 新章节",
            &[],
        ),
    )));
    s.append(SessionEvent::UserMessage {
        content: "u2".into(),
        parts: Vec::new(),
    });
    kernel.step_as(&mut s, "s1").await.expect("step 2");

    let recorded = spy.0.lock().unwrap().clone();
    let v1: crate::RequestPayload = serde_json::from_str(&recorded[0]).unwrap();
    let v2: crate::RequestPayload = serde_json::from_str(&recorded[1]).unwrap();
    let s1 = v1.system.unwrap();
    let s2 = v2.system.unwrap();
    assert!(
        s2.starts_with(&s1),
        "目录只增不改：旧前缀逐字节保留 (s1={s1:?} s2={s2:?})"
    );
}

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
