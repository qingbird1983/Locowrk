//! `netsafe` 的等价测试：把**被上收掉的那份原文**（`tool-fetch/src/ssrf.rs`，改前
//! 109 行）原样抄成参考实现，跑同一批 URL 输入逐字节对账（含 Err 报文）。
//! 先例 = 批 F-1 `now_ms` / 批 F-2 `clip` / 批 F-21a `arg_u64`：参考实现必须是被删的
//! 那份，不是我另写一个"看起来对"的。
//!
//! `tool-notify` 那份不另抄：门禁 `ssrf_shapes` 直接拿 `git show HEAD:` 的两份原文
//! 做归一化比较，机器证明**两者只差报文前缀与后门名两处字面量**——那一条成立时，
//! 与 fetch 等值的基座必然也与 notify 等值（两个参数位分别注入即可）。
//!
//! env 是进程级全局：会改它的用例之间必须串行（`ENV_LOCK`），并在结尾清干净，
//! 否则后门开着时同一测试二进制里的其它用例会莫名变绿（历批踩过同族竞态）。
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use super::{Ssrf, is_private};

const FETCH_ENV: &str = "LOCOWRK_FETCH_ALLOW_PRIVATE";
const NOTIFY_ENV: &str = "LOCOWRK_NOTIFY_ALLOW_PRIVATE";

/// 上收前的原文（`tool-fetch/src/ssrf.rs` 第 13–106 行，除可见性外一字未改）。
mod ref_fetch {
    use super::{IpAddr, SocketAddr, ToSocketAddrs};

    const ENV_ALLOW: &str = "LOCOWRK_FETCH_ALLOW_PRIVATE";

    pub(super) fn guard(url: &str) -> Result<(), String> {
        if std::env::var(ENV_ALLOW).map(|v| v == "1").unwrap_or(false) {
            return Ok(());
        }
        let (host, port) = host_port(url)?;
        if let Ok(ip) = host.parse::<IpAddr>() {
            return if is_private(ip) {
                Err(deny(&ip))
            } else {
                Ok(())
            };
        }
        let addrs: Vec<SocketAddr> = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|e| format!("fetch: cannot resolve `{host}`: {e}"))?
            .collect();
        if addrs.is_empty() {
            return Err(format!("fetch: `{host}` resolved to no addresses"));
        }
        for addr in &addrs {
            if is_private(addr.ip()) {
                return Err(deny(&addr.ip()));
            }
        }
        Ok(())
    }

    fn deny(ip: &IpAddr) -> String {
        format!(
            "fetch refused: `{ip}` is a private/loopback/link-local address \
             (SSRF guard; set LOCOWRK_FETCH_ALLOW_PRIVATE=1 to override)"
        )
    }

    pub(super) fn host_port(url: &str) -> Result<(String, u16), String> {
        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| "fetch: missing scheme".to_string())?;
        let default_port = if scheme.eq_ignore_ascii_case("https") {
            443
        } else {
            80
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        let authority = authority.rsplit('@').next().unwrap_or(authority);
        let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
            let (h, tail) = inner
                .split_once(']')
                .ok_or_else(|| "fetch: malformed IPv6 authority".to_string())?;
            let port = tail
                .strip_prefix(':')
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(default_port);
            (h.to_owned(), port)
        } else {
            match authority.rsplit_once(':') {
                Some((h, p)) => {
                    let port = p
                        .parse::<u16>()
                        .map_err(|_| format!("fetch: bad port in `{authority}`"))?;
                    (h.to_owned(), port)
                }
                None => (authority.to_owned(), default_port),
            }
        };
        if host.is_empty() {
            return Err("fetch: empty host".into());
        }
        Ok((host.to_ascii_lowercase(), port))
    }

    pub(super) fn is_private(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                v4.is_loopback()          // 127/8
                    || v4.is_private()    // 10/8, 172.16/12, 192.168/16
                    || v4.is_link_local() // 169.254/16
                    || v4.is_unspecified() // 0.0.0.0
                    || v4.is_broadcast()
                    || v4.is_multicast()
            }
            IpAddr::V6(v6) => {
                v6.is_loopback() // ::1
                    || v6.is_unspecified()
                    || v6.is_multicast()
                    || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 unique local
                    || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 link local
            }
        }
    }
}

const FETCH: Ssrf = Ssrf {
    label: "fetch",
    env_allow: FETCH_ENV,
};

const NOTIFY: Ssrf = Ssrf {
    label: "notify",
    env_allow: NOTIFY_ENV,
};

/// URL 面：字面量（公网 / 各段私网 / IPv6 带不带端口）、userinfo、大写归一、
/// 缺 scheme、空 host、坏端口、DNS 路径（`localhost` 本机必解析得出，不打外网）。
fn urls() -> Vec<&'static str> {
    vec![
        "http://8.8.8.8",
        "http://8.8.8.8:53/x",
        "https://1.1.1.1",
        "http://127.0.0.1:6379/",
        "http://127.8.8.8",
        "http://10.1.2.3",
        "http://172.16.0.1",
        "http://172.32.0.1",
        "http://192.168.1.10/admin",
        "http://169.254.1.1",
        "http://0.0.0.0",
        "http://[::1]:8080/x",
        "http://[::1]/x",
        "http://[fe80::1]",
        "http://[fd00::1]",
        "http://user:pw@localhost:81/",
        "http://LOCALHOST",
        "https://localhost/x?y=1#z",
        "http://localhost:6379/",
        "http:///path",
        "http://h:notaport/",
        "no-scheme",
        "http://[::1",
        "",
    ]
}

#[test]
fn guard_matches_the_uplifted_original_byte_for_byte() {
    let _env = ENV_LOCK.lock().unwrap();
    clear_env(FETCH_ENV);
    for url in urls() {
        let got = FETCH.guard(url);
        assert_eq!(ref_fetch::guard(url), got, "[guard] url: {url:?}");
        // 报文若被改漂（哪怕一个词），上面这条等值立刻红——这正是上收最怕的形态
        if let Err(msg) = &got {
            assert!(
                msg.starts_with("fetch") || msg.is_empty(),
                "[报文前缀] url: {url:?} got: {msg}"
            );
        }
    }
}

#[test]
fn host_port_and_is_private_match_the_original() {
    for url in urls() {
        assert_eq!(
            ref_fetch::host_port(url),
            FETCH.host_port(url),
            "[host_port] url: {url:?}"
        );
    }
    let ips = [
        "127.0.0.1",
        "127.8.8.8",
        "10.1.2.3",
        "172.16.0.1",
        "172.31.255.255",
        "172.32.0.1",
        "192.168.1.1",
        "169.254.1.1",
        "224.0.0.1",
        "255.255.255.255",
        "0.0.0.0",
        "8.8.8.8",
        "::1",
        "::",
        "fe80::1",
        "fd00::1",
        "2606:4700::1111",
        "ff02::1",
    ];
    for s in ips {
        let ip = s.parse::<IpAddr>().unwrap();
        assert_eq!(
            ref_fetch::is_private(ip),
            is_private(ip),
            "[is_private] {s}"
        );
    }
}

#[test]
fn notify_params_reproduce_its_own_env_and_prefix() {
    // 参数位注入必须两两独立：notify 的后门不能放行 fetch，反之亦然。
    let _env = ENV_LOCK.lock().unwrap();
    clear_env(FETCH_ENV);
    set_env(NOTIFY_ENV, "1");
    assert!(
        NOTIFY.guard("http://127.0.0.1:1/").is_ok(),
        "notify 后门开 = 放行"
    );
    assert!(
        FETCH.guard("http://127.0.0.1:1/").is_err(),
        "notify 后门不该影响 fetch"
    );
    clear_env(NOTIFY_ENV);
    set_env(FETCH_ENV, "1");
    assert!(FETCH.guard("http://127.0.0.1:1/").is_ok());
    assert!(
        NOTIFY.guard("http://127.0.0.1:1/").is_err(),
        "fetch 后门不该影响 notify"
    );
    clear_env(FETCH_ENV);
    // 后门只认 "1"：写 0 或别的值都必须仍拦（上收前就是 `== "1"` 字面判）
    for v in ["0", "", "true", "2"] {
        set_env(FETCH_ENV, v);
        assert!(
            FETCH.guard("http://127.0.0.1:1/").is_err(),
            "env={v} 必须仍拦"
        );
        assert!(
            NOTIFY.guard("http://127.0.0.1:1/").is_err(),
            "env={v} 必须仍拦"
        );
    }
    clear_env(FETCH_ENV);
    // 报文逐字节：两个参数位各自的后门名都要出现在提示里
    assert_eq!(
        FETCH.guard("http://10.0.0.1").unwrap_err(),
        format!(
            "fetch refused: `10.0.0.1` is a private/loopback/link-local address \
             (SSRF guard; set {FETCH_ENV}=1 to override)"
        )
    );
    assert_eq!(
        NOTIFY.guard("http://10.0.0.1").unwrap_err(),
        format!(
            "notify refused: `10.0.0.1` is a private/loopback/link-local address \
             (SSRF guard; set {NOTIFY_ENV}=1 to override)"
        )
    );
}

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// edition 2024 下 `set_var`/`remove_var` 是 unsafe（并行改 env 会与正在读的线程竞态）。
/// SAFETY = 每个调用点都持有 `ENV_LOCK`，改 env 的用例之间全串行；断言完即清除，
/// 不留后门状态给同一二进制里的其它用例。
fn set_env(k: &str, v: &str) {
    unsafe { std::env::set_var(k, v) };
}

fn clear_env(k: &str) {
    unsafe { std::env::remove_var(k) };
}
