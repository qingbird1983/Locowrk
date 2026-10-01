//! SSRF 防线单测（恶意路径三态）：host/port 提取矩阵、私网判定矩阵、
//! localhost 拒绝与显式后门。涉 env 用例串行。

use std::sync::Mutex;

use super::{guard, host_port, is_private};
use std::net::IpAddr;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn host_port_extracts_authority() {
    let cases = [
        ("http://example.com", ("example.com", 80)),
        ("https://example.com/x?y=1#z", ("example.com", 443)),
        ("http://example.com:8080/a", ("example.com", 8080)),
        ("http://user:pw@example.com:81/", ("example.com", 81)),
        ("http://[::1]:8080/x", ("::1", 8080)),
        ("http://[::1]/x", ("::1", 80)),
        ("https://EXAMPLE.com", ("example.com", 443)),
    ];
    for (url, (host, port)) in cases {
        assert_eq!(
            host_port(url).unwrap(),
            (host.to_owned(), port),
            "url: {url}"
        );
    }
    assert!(host_port("http:///path").is_err(), "empty host");
    assert!(host_port("http://h:notaport/").is_err(), "bad port");
    assert!(host_port("no-scheme").is_err());
}

#[test]
fn private_ip_matrix() {
    let private = [
        "127.0.0.1",
        "127.8.8.8",
        "10.1.2.3",
        "172.16.0.1",
        "172.31.255.255",
        "192.168.1.1",
        "169.254.1.1",
        "0.0.0.0",
        "::1",
        "fe80::1",
        "fd00::1",
        "::",
    ];
    for s in private {
        assert!(
            is_private(s.parse::<IpAddr>().unwrap()),
            "{s} must be private"
        );
    }
    let public = ["8.8.8.8", "1.1.1.1", "172.32.0.1", "2606:4700::1111"];
    for s in public {
        assert!(!is_private(s.parse::<IpAddr>().unwrap()), "{s} is public");
    }
}

#[test]
fn guard_denies_localhost_and_ip_literals() {
    // env 是进程级全局：与设 LOCOWRK_NOTIFY_ALLOW_PRIVATE 的用例串行，
    // 否则后门开着时本用例必挂（并行竞态）
    let _env_lock = ENV_LOCK.lock().unwrap();
    // IP 字面量不需要 DNS
    let err = guard("http://127.0.0.1:6379/").unwrap_err();
    assert!(err.contains("SSRF guard"), "got: {err}");
    assert!(guard("http://192.168.1.10/admin").is_err());
    assert!(guard("http://[::1]:9/").is_err());
    // localhost 靠解析（本机必有）
    assert!(guard("http://localhost:6379/").is_err());
}

#[test]
fn allow_private_env_overrides() {
    let _guard_lock = ENV_LOCK.lock().unwrap();
    unsafe { std::env::set_var("LOCOWRK_NOTIFY_ALLOW_PRIVATE", "1") };
    assert!(guard("http://127.0.0.1:1/").is_ok(), "显式后门放行本机");
    unsafe { std::env::set_var("LOCOWRK_NOTIFY_ALLOW_PRIVATE", "0") };
    assert!(guard("http://127.0.0.1:1/").is_err());
    unsafe { std::env::remove_var("LOCOWRK_NOTIFY_ALLOW_PRIVATE") };
}
