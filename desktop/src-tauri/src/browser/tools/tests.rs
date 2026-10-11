//! browser_open / browser_console 工具单测：参数裁决（二选一/绝对路径/
//! 扩展名门/URL 形状）、fsRoots 围栏、事件载荷、console 缺省读。

use super::*;
use serde_json::json;
use std::path::Path;

/// 事件记录账（channel, payload）列表句柄。
type EmitLog = std::sync::Arc<std::sync::Mutex<Vec<(String, Value)>>>;

fn emit_log() -> (EmitFn, EmitLog) {
    let log: EmitLog = std::sync::Arc::default();
    let sink = log.clone();
    let f: EmitFn = Arc::new(move |channel, payload| {
        sink.lock().unwrap().push((channel.to_string(), payload));
    });
    (f, log)
}

fn settings_with_roots(roots: Vec<String>) -> Arc<SettingsStore> {
    // settings 存储走真实存储（临时目录），tools 区 fsRoots 打进去
    let dir = crate::testutil::unique_dir("lw-browser-tools");
    let store = SettingsStore::open(&dir).expect("settings open");
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(crate::settings::ToolsUpdate {
                fs_roots: Some(roots),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
        )
        .expect("fs_roots update");
    Arc::new(store)
}

#[tokio::test]
async fn open_requires_exactly_one_of_path_or_url() {
    let (emit, log) = emit_log();
    let console = Arc::new(ConsoleStore::new());
    let tool = BrowserOpenTool::new(settings_with_roots(vec![]), console, emit);
    let err = tool
        .call(&CallCtx::new(), json!({}))
        .await
        .expect_err("空参数拒");
    assert!(err.to_string().contains("exactly one"));
    let err = tool
        .call(
            &CallCtx::new(),
            json!({"path":"F:\\a.html","url":"https://x.dev"}),
        )
        .await
        .expect_err("双参数拒");
    assert!(err.to_string().contains("exactly one"));
    assert!(log.lock().unwrap().is_empty(), "拒绝路径零事件");
}

#[tokio::test]
async fn open_rejects_relative_path_and_bad_extension() {
    let (emit, log) = emit_log();
    let tool = BrowserOpenTool::new(
        settings_with_roots(vec![]),
        Arc::new(ConsoleStore::new()),
        emit,
    );
    let err = tool
        .call(&CallCtx::new(), json!({"path":"page.html"}))
        .await
        .expect_err("相对路径拒");
    assert!(err.to_string().contains("absolute"));
    let err = tool
        .call(&CallCtx::new(), json!({"path":"F:\\dir\\main.rs"}))
        .await
        .expect_err("非入口扩展名拒");
    assert!(err.to_string().contains("entry file"), "{err}");
    assert!(log.lock().unwrap().is_empty());
}

#[tokio::test]
async fn open_rejects_outside_roots() {
    let (emit, _) = emit_log();
    // fsRoots 写入带存在性校验：根必须是真实存在的目录，"外部"文件也须实存
    let base = crate::testutil::unique_dir("lw-browser-out");
    let root = base.join("ws");
    std::fs::create_dir_all(&root).unwrap();
    let outside = base.join("outside.html");
    std::fs::write(&outside, b"x").unwrap();
    let tool = BrowserOpenTool::new(
        settings_with_roots(vec![root.to_string_lossy().to_string()]),
        Arc::new(ConsoleStore::new()),
        emit,
    );
    let err = tool
        .call(&CallCtx::new(), json!({"path": outside.to_string_lossy()}))
        .await
        .expect_err("根外拒");
    assert!(err.to_string().contains("workspace roots"), "{err}");
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn open_file_emits_event_and_returns_tab() {
    let base = crate::testutil::unique_dir("lw-browser-open");
    let root = base.join("ws");
    std::fs::create_dir_all(&root).unwrap();
    let page = root.join("game.html");
    std::fs::write(&page, "<p>hi</p>").unwrap();

    let (emit, log) = emit_log();
    let console = Arc::new(ConsoleStore::new());
    let tool = BrowserOpenTool::new(
        settings_with_roots(vec![root.to_string_lossy().to_string()]),
        console.clone(),
        emit,
    );
    let out = tool
        .call(&CallCtx::new(), json!({"path": page.to_string_lossy()}))
        .await
        .expect("根内 html 放行");
    assert_eq!(out["kind"], "file");
    let tab_id = out["tabId"].as_str().unwrap();
    assert!(tab_id.starts_with("br-"));
    assert!(
        out["url"]
            .as_str()
            .unwrap()
            .starts_with(preview::PREVIEW_ORIGIN),
        "{out}"
    );
    assert_eq!(out["title"], "game.html");
    // 事件恰一条，通道与载荷一致
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].0, CHANNEL_OPEN);
    assert_eq!(log[0].1["tabId"], out["tabId"]);
    // 打开即占位：console 可点名读空表
    let (got, entries, _) = console.read(Some(tab_id)).expect("tab registered");
    assert_eq!(got, tab_id);
    assert!(entries.is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn open_url_validates_shape() {
    let (emit, _) = emit_log();
    let console = Arc::new(ConsoleStore::new());
    let tool = BrowserOpenTool::new(settings_with_roots(vec![]), console, emit);
    for bad in ["ftp://x", "https://a b", "javascript:alert(1)"] {
        let err = tool
            .call(&CallCtx::new(), json!({"url": bad}))
            .await
            .expect_err(bad);
        assert!(!err.to_string().is_empty());
    }
}

#[tokio::test]
async fn console_read_without_tabs_reports_honestly() {
    let tool = BrowserConsoleTool::new(Arc::new(ConsoleStore::new()));
    let out = tool.call(&CallCtx::new(), json!({})).await.unwrap();
    assert_eq!(out["entries"].as_array().unwrap().len(), 0);
    assert!(out["note"].as_str().unwrap().contains("browser_open"));
}

#[test]
fn entry_gate_matches_renderable_set() {
    assert!(ENTRY_EXTENSIONS.contains(&"html"));
    assert!(!ENTRY_EXTENSIONS.contains(&"exe"));
    assert!(Path::new("F:\\a\\b.html").extension().is_some());
}
