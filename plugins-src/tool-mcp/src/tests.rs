//! tool-mcp 三态用例：正常（发现 / 调用 / 分级 / reload / 懒重连）/
//! 边界（无配置、超时、未知 server / 工具）/ 恶意（坏配置 fail-soft、
//! 坏 server 不拖垮其他、名字清洗）。fake-mcp-server 二进制做替身；
//! 涉 env / 全局 static 的用例持锁串行。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use super::config;
use super::registry::Registry;
use super::{describe_manifest, make_state};
use plugin_api::PluginState;
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::SideEffect;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// fake-mcp-server 产物路径（examples 目标：无哈希、无 test harness，
/// `cargo test` 会构建；`CARGO_BIN_EXE_*` 在 cdylib 单测里不可用）。
fn fake_server_path() -> String {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_fake-mcp-server") {
        return p;
    }
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples")
        .join(format!("fake_mcp_server{}", std::env::consts::EXE_SUFFIX));
    assert!(
        p.is_file(),
        "fake_mcp_server example binary not found at {} (cargo test builds examples first)",
        p.display()
    );
    p.to_str().unwrap().to_string()
}

fn temp_data(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("locowrk-mcp-{tag}-{nanos}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: 全部涉 env 用例经 ENV_LOCK 串行；Rust 2024 起 set_var 为 unsafe
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &dir) };
    dir
}

fn write_config(dir: &Path, servers: Value) {
    std::fs::write(
        dir.join("mcp.json"),
        json!({ "servers": servers }).to_string(),
    )
    .unwrap();
}

fn fake_cfg() -> Value {
    json!({
        "command": fake_server_path(),
        "args": [],
        "side_effect": "pure",
        "tool_effects": { "boom": "irreversible" },
        "timeout_ms": 5000
    })
}

#[test]
fn discover_call_grades_reload_and_lazy_reconnect() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data("e2e");
    write_config(&dir, json!({ "fake": fake_cfg() }));
    let reg = Registry::discover();

    // 状态：已连接、3 个工具
    let rows = reg.status();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["connected"], json!(true));
    assert_eq!(rows[0]["tools"], json!(3));
    assert_eq!(rows[0]["error"], Value::Null);

    // 规格：echo 继承 server 级 pure，boom 被 tool_effects 收紧为 irreversible
    let specs = reg.tool_specs();
    let echo = specs.iter().find(|s| s.name == "mcp.fake.echo").unwrap();
    assert!(matches!(echo.side_effect, SideEffect::Pure));
    let boom = specs.iter().find(|s| s.name == "mcp.fake.boom").unwrap();
    assert!(matches!(boom.side_effect, SideEffect::Irreversible));

    // 调用：文本回显 + 工具级失败作为数据返回（tool-fetch 惯例）
    let out = reg.call("fake", "echo", json!({ "text": "hi" })).unwrap();
    assert_eq!(out["content"], json!("echo: hi"));
    assert_eq!(out["is_error"], json!(false));
    let out = reg.call("fake", "boom", json!({})).unwrap();
    assert_eq!(out["is_error"], json!(true));
    assert_eq!(out["content"], json!("boom!"));

    // reload：重连成功，工具缓存不变
    reg.reload();
    assert_eq!(reg.status()[0]["connected"], json!(true));
    let out = reg
        .call("fake", "echo", json!({ "text": "after-reload" }))
        .unwrap();
    assert_eq!(out["content"], json!("echo: after-reload"));

    // 懒重连：强制断开后调用自动恢复
    reg.servers
        .lock()
        .unwrap()
        .get("fake")
        .unwrap()
        .lock()
        .unwrap()
        .conn = None;
    let out = reg.call("fake", "echo", json!({ "text": "lazy" })).unwrap();
    assert_eq!(out["content"], json!("echo: lazy"));

    // 未知 server / 工具
    assert!(
        reg.call("nope", "x", json!({}))
            .unwrap_err()
            .contains("unknown server")
    );
    assert!(
        reg.call("fake", "absent", json!({}))
            .unwrap_err()
            .contains("no tool")
    );
}

#[test]
fn slow_tool_times_out_but_connection_survives() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data("timeout");
    write_config(
        &dir,
        json!({ "fake": { "command": fake_server_path(), "args": [], "timeout_ms": 300 } }),
    );
    let reg = Registry::discover();
    let err = reg.call("fake", "slow", json!({})).unwrap_err();
    assert!(err.contains("timed out"), "got: {err}");
    // server 级超时对每条连接上的每次调用生效：slow 睡 900ms 期间同一
    // server 的后续调用也会等到它忙完——所以"超时后连接是否还可用"要
    // reload 重连之后看（懒重连路径在 e2e 用例另有覆盖）
    reg.reload();
    let out = reg
        .call("fake", "echo", json!({ "text": "still-alive" }))
        .unwrap();
    assert_eq!(out["content"], json!("echo: still-alive"));
}

#[test]
fn broken_server_is_skipped_and_good_one_still_works() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data("broken");
    write_config(
        &dir,
        json!({
            "fake": fake_cfg(),
            "ghost": { "command": "definitely-not-a-real-binary-xyz", "args": [] }
        }),
    );
    // 整体发现不得失败
    let reg = Registry::discover();
    let rows = reg.status();
    assert_eq!(rows.len(), 2);
    let ghost = rows.iter().find(|r| r["name"] == "ghost").unwrap();
    assert_eq!(ghost["connected"], json!(false));
    assert!(ghost["error"].as_str().is_some());
    let out = reg.call("fake", "echo", json!({ "text": "ok" })).unwrap();
    assert_eq!(out["content"], json!("echo: ok"));
}

#[test]
fn missing_config_loads_empty_and_config_errors_fail_soft() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data("noconfig");
    let reg = Registry::discover();
    assert!(reg.status().is_empty());
    assert!(reg.tool_specs().is_empty());
    assert!(reg.cfg_errors.is_empty());

    // 坏配置：三条 server 级错误 + 一个合法 server；整体不崩
    write_config(
        &dir,
        json!({
            "good": fake_cfg(),
            "Bad Name": { "command": "x" },
            "nocmd": {},
            "badgrade": { "command": "x", "side_effect": "whatever" }
        }),
    );
    let reg = Registry::discover();
    assert_eq!(reg.cfg_errors.len(), 3);
    assert_eq!(reg.status().len(), 1);
    assert!(reg.call("good", "echo", json!({ "text": "1" })).is_ok());
}

#[test]
fn compensable_grade_is_tightened_to_irreversible() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data("grade");
    write_config(
        &dir,
        json!({ "c": { "command": fake_server_path(), "args": [], "side_effect": "compensable" } }),
    );
    let reg = Registry::discover();
    // 远端工具没有机械 undo：宁严勿松
    assert!(
        reg.tool_specs()
            .iter()
            .all(|s| matches!(s.side_effect, SideEffect::Irreversible))
    );
}

#[test]
fn tool_name_sanitization() {
    assert_eq!(
        config::sanitize_tool_name("weird.name"),
        Some("weird_name".to_string())
    );
    assert_eq!(
        config::sanitize_tool_name("ok-name_2"),
        Some("ok-name_2".to_string())
    );
    assert_eq!(config::sanitize_tool_name("///"), None);
    assert_eq!(config::sanitize_tool_name("___"), None);
}

#[test]
fn describe_and_make_state_share_discovery() {
    let _g = ENV_LOCK.lock().unwrap();
    super::registry::reset();
    let dir = temp_data("describe");
    write_config(&dir, json!({ "fake": fake_cfg() }));

    // describe：管理工具 + 3 个远端工具，全部进 manifest
    let raw = describe_manifest();
    let text = raw.to_str().unwrap().trim_end_matches('\0');
    let manifest: Value = plugin_api::serde_json::from_str(text).unwrap();
    let names: Vec<&str> = manifest["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 5);
    assert!(names.contains(&"mcp.status"));
    assert!(names.contains(&"mcp.reload"));
    assert!(names.contains(&"mcp.fake.echo"));

    // make_state 接管 static 里的连接（不重复 spawn），host 指针 null 安全
    let state = make_state(std::ptr::null());
    let out =
        PluginState::call(state.as_ref(), "mcp.fake.echo", json!({ "text": "flow" })).unwrap();
    assert_eq!(out["content"], json!("echo: flow"));
    // 管理工具走同一入口
    let st = PluginState::call(state.as_ref(), "mcp.status", json!({})).unwrap();
    assert_eq!(st["servers"][0]["name"], json!("fake"));

    drop(state);
    super::registry::reset();
}
