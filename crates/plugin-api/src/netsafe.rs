//! SSRF 防线基座（审计 F7 · 批 F-21b 上收）：内网 / 环回 / 链路本地地址一律拒绝，
//! `http://localhost:6379`、`http://192.168.x.x` 这类请求到不了网络。
//!
//! 上收前的实况（逐份读原文实测，不是照抄审计登记）：`tool-fetch/src/ssrf.rs` 与
//! `tool-notify/src/ssrf.rs` 两份**只差两处字面量**——报文前缀（`fetch` / `notify`）与
//! 后门环境变量名（`LOCOWRK_FETCH_ALLOW_PRIVATE` / `LOCOWRK_NOTIFY_ALLOW_PRIVATE`），
//! 逻辑、分支、判定面逐字相同 ⇒ 参数位只有两个，实现只留一处（`label` + `env_allow`）。
//!
//! 纪律（历批定下的语义，改动必须逐条对得上）：
//! - IP 字面量直接判；域名先解析出**全部**地址再逐个判（宁拒勿漏）
//! - 解析失败 = 拒绝（fail-closed，反正请求也发不出去）
//! - `{env_allow}=1` 是显式后门（本机开发/测试用；打本进程服务的测试必须设它）
//! - 重定向在**调用侧**关闭：302 指向内网是绕过本防线的最短路径
//! - 残余风险（v1 接受）：DNS rebinding 的 TOCTOU——guard 解析与实际请求是两次解析
//!
//! 不进 ABI 面（与 `args`/`fsutil`/`memlayout` 同栏）：插件源码级依赖，不跨 C ABI。
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

/// 本防线的参数位：报文前缀 + 显式后门的环境变量名。
/// 两个字段都是 `&'static str` ⇒ 可以写成 `const`，插件侧零运行时开销。
pub struct Ssrf {
    pub label: &'static str,
    pub env_allow: &'static str,
}

impl Ssrf {
    /// 放行 = `Ok(())`；拒绝 = 带报文。报文里只有插件名与后门名随参数位变，
    /// 其余字面与上收前的两份原文逐字节相同（等价证明在 `netsafe/tests.rs`）。
    pub fn guard(&self, url: &str) -> Result<(), String> {
        if std::env::var(self.env_allow)
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            return Ok(());
        }
        let (host, port) = self.host_port(url)?;
        if let Ok(ip) = host.parse::<IpAddr>() {
            return if is_private(ip) {
                Err(self.deny(&ip))
            } else {
                Ok(())
            };
        }
        let addrs: Vec<SocketAddr> = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|e| format!("{}: cannot resolve `{host}`: {e}", self.label))?
            .collect();
        if addrs.is_empty() {
            return Err(format!("{}: `{host}` resolved to no addresses", self.label));
        }
        for addr in &addrs {
            if is_private(addr.ip()) {
                return Err(self.deny(&addr.ip()));
            }
        }
        Ok(())
    }

    /// 从 URL 提取 (host, port)。scheme 只允许 http/https（上游已校验）；
    /// 去 userinfo；IPv6 字面量必须带方括号。host 归一小写。
    pub fn host_port(&self, url: &str) -> Result<(String, u16), String> {
        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| format!("{}: missing scheme", self.label))?;
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
                .ok_or_else(|| format!("{}: malformed IPv6 authority", self.label))?;
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
                        .map_err(|_| format!("{}: bad port in `{authority}`", self.label))?;
                    (h.to_owned(), port)
                }
                None => (authority.to_owned(), default_port),
            }
        };
        if host.is_empty() {
            return Err(format!("{}: empty host", self.label));
        }
        Ok((host.to_ascii_lowercase(), port))
    }

    fn deny(&self, ip: &IpAddr) -> String {
        format!(
            "{} refused: `{ip}` is a private/loopback/link-local address \
             (SSRF guard; set {}=1 to override)",
            self.label, self.env_allow
        )
    }
}

/// 私网 / 环回 / 链路本地 / 未指定 / 组播判定（IPv4 与 IPv6 各一套）。
/// 与上收前两份原文逐字相同；`fc00::/7` 用段首掩码判，`fe80::/10` 同理。
pub fn is_private(ip: IpAddr) -> bool {
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
