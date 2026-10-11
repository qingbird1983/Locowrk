//! 端到端集成测试：真实 dlopen 阶段八③的第一批工具插件
//! （plugins.md：tool-fs 只读三件套 + tool-fetch）。
//!
//! 与 echo_plugin.rs 同款模式：测试自行构建插件产物（LOCOWRK_PLUGIN_VERSION
//! 触发重编）并暂存到 target/locowrk-test/{pid}/，不依赖仓库 plugins/ 目录的
//! 预暂存状态。fetch 的网络路径打本进程 TCP 起的 HTTP 服务，全离线。
//! 装载样板 = `mod common`（本文件用 `stage_and_load`，env 锁用 `common::env_lock`）。

use plugin_api::ToolError;
use serde_json::json;
mod common;
use common::{call, stage_and_load};

#[test]
fn fs_tools_catalog_read_list_grep_roundtrip() {
    let (_registry, source) = stage_and_load("tool-fs", "1.2.1");

    // 目录快照：六个工具，副作用分级符合 plugins.md §二
    let tools = source.list_tools();
    for expected in [
        "fs_read",
        "fs_list",
        "fs_grep",
        "fs_write",
        "fs_delete",
        "fs_restore",
    ] {
        assert!(
            tools.iter().any(|s| s.name == expected),
            "缺少 {expected}: {:?}",
            tools.iter().map(|s| s.name.clone()).collect::<Vec<_>>()
        );
    }
    let side_of = |name: &str| {
        tools
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.side_effect.clone())
    };
    for name in ["fs_read", "fs_list", "fs_grep"] {
        assert!(
            matches!(side_of(name), Some(plugin_api::SideEffect::Pure)),
            "{name} 应为 Pure"
        );
    }
    assert!(matches!(
        side_of("fs_write"),
        Some(plugin_api::SideEffect::Compensable { undo_tool })
            if undo_tool == "fs_restore"
    ));
    for name in ["fs_delete", "fs_restore"] {
        assert!(
            matches!(side_of(name), Some(plugin_api::SideEffect::Irreversible)),
            "{name} 应为 Irreversible"
        );
    }

    // 准备临时内容目录：一个文件 + 一个子目录；它就是本测试的允许根
    let base = std::env::temp_dir().join(format!("locowrk-fs-it-{}", std::process::id()));
    let sub = base.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let hello = base.join("hello.txt");
    std::fs::write(&hello, "alpha line\nBETA gamma\nlast\n").unwrap();
    std::fs::write(sub.join("nested.txt"), "deep beta match\n").unwrap();
    unsafe { std::env::set_var("LOCOWRK_FS_ROOTS", base.display().to_string()) };

    // fs_list：目录在前；文件带大小
    let list = call(
        source.resolve("fs_list").unwrap().as_ref(),
        json!({"path": base}),
    )
    .unwrap();
    let entries = list["entries"].as_array().unwrap();
    assert_eq!(entries[0]["kind"], "dir", "目录应排最前");
    assert_eq!(entries[0]["name"], "sub");
    let hello_entry = entries.iter().find(|e| e["name"] == "hello.txt").unwrap();
    assert_eq!(hello_entry["kind"], "file");
    assert_eq!(hello_entry["size"], 27);

    // fs_read：全量与截断
    let read = call(
        source.resolve("fs_read").unwrap().as_ref(),
        json!({"path": hello}),
    )
    .unwrap();
    assert_eq!(read["truncated"], false);
    assert_eq!(read["content"], "alpha line\nBETA gamma\nlast\n");
    assert_eq!(read["size"], 27);

    let read = call(
        source.resolve("fs_read").unwrap().as_ref(),
        json!({"path": hello, "max_bytes": 10}),
    )
    .unwrap();
    assert_eq!(read["truncated"], true);
    assert_eq!(read["content"].as_str().unwrap().len(), 10);

    // fs_read 目录 → 明确报错（不是静默）
    assert!(
        call(
            source.resolve("fs_read").unwrap().as_ref(),
            json!({"path": base})
        )
        .is_err()
    );

    // fs_grep：大小写不敏感；file 字段可定位
    let grep = call(
        source.resolve("fs_grep").unwrap().as_ref(),
        json!({"path": base, "query": "beta"}),
    )
    .unwrap();
    let matches = grep["matches"].as_array().unwrap();
    assert_eq!(
        matches.len(),
        2,
        "hello.txt 第2行 + nested.txt: {matches:?}"
    );
    assert_eq!(matches[0]["line"], 2);
    assert!(matches[0]["file"].as_str().unwrap().ends_with("hello.txt"));
    assert_eq!(matches[0]["text"], "BETA gamma");

    // fs_grep 结果数上限 → truncated
    let grep = call(
        source.resolve("fs_grep").unwrap().as_ref(),
        json!({"path": base, "query": "beta", "max_results": 1}),
    )
    .unwrap();
    assert_eq!(grep["truncated"], true);
    assert_eq!(grep["matches"].as_array().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(&base);
    unsafe { std::env::remove_var("LOCOWRK_FS_ROOTS") };
}

#[test]
fn fetch_rejects_non_http_schemes() {
    let (_registry, source) = stage_and_load("tool-fetch", "1.0.0");
    let err = call(
        source.resolve("fetch").unwrap().as_ref(),
        json!({"url": "file:///etc/hosts"}),
    )
    .expect_err("file: 协议必须拒绝");
    let msg = match err {
        ToolError::Failed { message, .. } => message,
        other => other.to_string(),
    };
    assert!(msg.contains("http"), "{msg}");
}

/// 本进程 TCP 起一个只应答一次的 HTTP 服务（全离线）。
fn serve_once(status_line: &'static str, body: &'static str) -> (u16, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf); // 请求头不解析，够触发应答即可
        let resp = format!(
            "{status_line}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
    });
    (port, handle)
}

#[test]
fn fetch_local_http_200_returns_capped_body() {
    let _env = common::env_lock().lock().unwrap();
    let (_registry, source) = stage_and_load("tool-fetch", "1.0.0");
    // SSRF 防线默认拒绝 127.0.0.1——本机测试显式走后门
    unsafe { std::env::set_var("LOCOWRK_FETCH_ALLOW_PRIVATE", "1") };
    let (port, server) = serve_once("HTTP/1.1 200 OK", "hello-fetch-body");

    let out = call(
        source.resolve("fetch").unwrap().as_ref(),
        json!({"url": format!("http://127.0.0.1:{port}/data"), "max_bytes": 5}),
    )
    .expect("HTTP 200 应成功");
    server.join().unwrap();

    assert_eq!(out["status"], 200);
    assert_eq!(out["content_type"], "text/plain");
    assert_eq!(out["truncated"], true);
    assert_eq!(out["body"], "hello");
    assert!(
        out["url"].as_str().unwrap().contains(&port.to_string()),
        "url 应是重定向后的最终地址: {}",
        out["url"]
    );
}

#[test]
fn fetch_http_404_is_data_not_error() {
    let _env = common::env_lock().lock().unwrap();
    let (_registry, source) = stage_and_load("tool-fetch", "1.0.0");
    unsafe { std::env::set_var("LOCOWRK_FETCH_ALLOW_PRIVATE", "1") };
    let (port, server) = serve_once("HTTP/1.1 404 Not Found", "no such page");

    let out = call(
        source.resolve("fetch").unwrap().as_ref(),
        json!({"url": format!("http://127.0.0.1:{port}/missing")}),
    )
    .expect("4xx 按数据返回，不是 Err");
    server.join().unwrap();

    assert_eq!(out["status"], 404);
    assert_eq!(out["body"], "no such page");
}

/// 恶意路径（安全底线）：未设后门时，环回地址在 guard 处被拒，
/// 请求到不了网络（没有任何 server 应答）。
#[test]
fn fetch_ssrf_guard_refuses_loopback() {
    let _env = common::env_lock().lock().unwrap();
    let (_registry, source) = stage_and_load("tool-fetch", "1.0.0");
    unsafe { std::env::remove_var("LOCOWRK_FETCH_ALLOW_PRIVATE") };

    let err = call(
        source.resolve("fetch").unwrap().as_ref(),
        json!({"url": "http://127.0.0.1:1/"}),
    )
    .expect_err("环回地址必须被 SSRF 防线拒绝");
    let msg = match err {
        ToolError::Failed { message, .. } => message,
        other => other.to_string(),
    };
    assert!(msg.contains("SSRF guard"), "{msg}");
}
