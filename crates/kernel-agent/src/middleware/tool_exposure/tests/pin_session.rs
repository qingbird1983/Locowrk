//! M1 会话钉死（KV v2 §4.2）（自 `tests.rs` 批 F-16 拆出，用例本体逐字未改）：钉死后
//! 暴露集跨轮逐字节冻结、关闭恢复每轮 Top-N / 钉死集与收紧后的目录取交集 / 目录放宽时
//! 新增 always 工具追加尾部（M4）/ 无会话身份不钉死。

use super::*;

/// 钉死关闭（KV v2 §4.2 实验对比档）。
fn cfg_lock_unpinned(enabled: bool, max_tools: usize) -> Arc<RwLock<ToolExposureConfig>> {
    Arc::new(RwLock::new(ToolExposureConfig {
        enabled,
        max_tools,
        pin_session: false,
    }))
}

// ---- M1 会话钉死（KV v2 §4.2）----

/// 钉死后暴露集跨轮冻结：第 2 轮会话内用过了未入选工具（旧行为会换它
/// 上位），tools 块仍与第 1 轮逐字节一致；关掉开关恢复每轮 Top-N。
#[tokio::test]
async fn pin_freezes_exposed_set_across_rounds() {
    let tools = vec![
        spec("a1", Exposure::OnDemand),
        spec("a2", Exposure::OnDemand),
        spec("a3", Exposure::OnDemand),
    ];
    let ewma: HashMap<String, f64> = [("a1".into(), 5.0), ("a2".into(), 1.0)].into();
    let l = listener(ewma, cfg_lock(true, 2));

    // 第 1 轮：无会话使用，按全局热度入选 a1、a2
    let (_, out1) = run_session(&l, tools.clone(), Vec::new(), Some("s1")).await;
    let names1: Vec<&str> = out1.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names1, vec!["a1", "a2"]);

    // 第 2 轮：会话内用过了 a3——钉死 keeps a1、a2（字节也一致）
    let (_, out2) = run_session(
        &l,
        tools.clone(),
        messages_with_session_use("a3"),
        Some("s1"),
    )
    .await;
    let names2: Vec<&str> = out2.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names2, vec!["a1", "a2"]);
    assert_eq!(
        serde_json::to_string(&out1.tools).unwrap(),
        serde_json::to_string(&out2.tools).unwrap(),
        "钉死会话 tools 块必须逐字节稳定（前缀缓存不灭）"
    );

    // 对照：钉死关闭 = 恢复每轮 Top-N，a3 靠会话热度上位（对照监听器
    // 无全局热度表，a1/a2 得分归零，a3 的会话 1.0 排最前）
    let l2 = ToolExposure::new(
        Arc::new(Fixed(HashMap::new())),
        cfg_lock_unpinned(true, 2),
        None,
    );
    let (_, _) = run_session(&l2, tools.clone(), Vec::new(), Some("s1")).await;
    let (_, legacy) = run_session(&l2, tools, messages_with_session_use("a3"), Some("s1")).await;
    let names_legacy: Vec<&str> = legacy.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names_legacy, vec!["a3", "a1"], "关闭钉死恢复旧行为");
}

/// 钉死集与当前目录取交集：目录被围栏收紧（计划模式）不复活已出目录的
/// 工具——显式变更接受失效，恢复后钉死序不变。
#[tokio::test]
async fn pinned_set_intersects_shrunk_catalog() {
    let tools = vec![
        spec("a1", Exposure::OnDemand),
        spec("a2", Exposure::OnDemand),
        spec("a3", Exposure::OnDemand),
    ];
    let ewma: HashMap<String, f64> = [("a1".into(), 5.0)].into();
    let l = listener(ewma, cfg_lock(true, 2));
    let (_, out1) = run_session(&l, tools, Vec::new(), Some("s1")).await;
    assert_eq!(
        out1.tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        vec!["a1", "a2"]
    );

    // 围栏收紧：目录只剩 a1、a3 → 钉死集 ∩ 目录 = [a1]
    let fenced = vec![
        spec("a1", Exposure::OnDemand),
        spec("a3", Exposure::OnDemand),
    ];
    let (_, out2) = run_session(&l, fenced, Vec::new(), Some("s1")).await;
    let names: Vec<&str> = out2.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["a1"], "出目录的工具不复活");
}

/// M4 补口（走查实抓）：目录**放宽**时新增的 always 工具（会话中途切
/// 记忆树模式，memory_expand 进目录）无条件保留、追加在尾部——旧集
/// 逐字节保留（前缀缓存不灭），always 直通不受钉死名单约束。
#[tokio::test]
async fn pinned_set_appends_new_always_tools_on_catalog_growth() {
    let tools = vec![
        spec("a1", Exposure::OnDemand),
        spec("a2", Exposure::OnDemand),
    ];
    let ewma: HashMap<String, f64> = [("a1".into(), 5.0)].into();
    let l = listener(ewma, cfg_lock(true, 2));
    let (_, out1) = run_session(&l, tools, Vec::new(), Some("s1")).await;
    assert_eq!(
        out1.tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        vec!["a1", "a2"]
    );

    // 目录放宽：新增 always 的 expand（切树模式）→ [a1, a2, expand]
    let grown = vec![
        spec("a1", Exposure::OnDemand),
        spec("a2", Exposure::OnDemand),
        spec("expand", Exposure::Always),
    ];
    let (_, out2) = run_session(&l, grown, Vec::new(), Some("s1")).await;
    let names: Vec<&str> = out2.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["a1", "a2", "expand"], "always 新工具追加尾部");
    assert!(
        serde_json::to_string(&out2.tools[..2]).unwrap()
            == serde_json::to_string(&out1.tools).unwrap(),
        "旧集逐字节保留（尾部追加不破前缀）"
    );
}

/// 无会话身份（headless 一次性进程）不钉死：保持每轮 Top-N 旧行为。
#[tokio::test]
async fn sessionless_requests_keep_legacy_per_round() {
    let tools = vec![
        spec("a1", Exposure::OnDemand),
        spec("a2", Exposure::OnDemand),
        spec("a3", Exposure::OnDemand),
    ];
    let l = listener(HashMap::new(), cfg_lock(true, 2));
    let (_, r1) = run(&l, tools.clone(), Vec::new()).await;
    let (_, r2) = run(&l, tools, messages_with_session_use("a3")).await;
    assert_eq!(
        r1.tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        vec!["a1", "a2"]
    );
    assert_eq!(
        r2.tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        vec!["a3", "a1"],
        "无身份 = 逐轮重算（与钉死前行为一致）"
    );
}
