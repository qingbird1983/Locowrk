//! SSRF 防线（用户 2026-09-29 指定）：内网 / 环回 / 链路本地地址一律拒绝，
//! `http://localhost:6379`、`http://192.168.x.x` 这类请求到不了网络。
//!
//! - IP 字面量直接判；域名先解析出**全部**地址再逐个判（宁拒勿漏）
//! - 解析失败 = 拒绝（fail-closed，反正请求也发不出去）
//! - `LOCOWRK_FETCH_ALLOW_PRIVATE=1` 是显式后门（本机开发/测试用；集成
//!   测试的 fetch 用例打本进程 TCP 服务，必须设它）
//! - 重定向在 fetch 侧关闭：302 指向内网是绕过本防线的最短路径
//! - 残余风险（v1 接受）：DNS rebinding 的 TOCTOU——guard 解析与 ureq
//!   实际请求是两次解析
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

const ENV_ALLOW: &str = "LOCOWRK_FETCH_ALLOW_PRIVATE";

pub(crate) fn guard(url: &str) -> Result<(), String> {
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

/// 从 URL 提取 (host, port)。scheme 只允许 http/https（上游已校验）；
/// 去 userinfo；IPv6 字面量必须带方括号。host 归一小写。
fn host_port(url: &str) -> Result<(String, u16), String> {
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

fn is_private(ip: IpAddr) -> bool {
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

#[cfg(test)]
mod tests;
