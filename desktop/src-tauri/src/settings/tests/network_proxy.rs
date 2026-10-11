//! 网络代理测试（自 `llm.rs` 批 F-13 拆出，用例本体逐字未改）：system/direct/custom 三态
//! 校验、custom 缺 url 与非 http 拒绝、未知模式拒绝、url 去 blanks 归一。

use super::*;

// ---- 网络代理（批 C §8.5） ----

#[test]
fn network_proxy_validation_and_normalization() {
    assert!(NetworkProxy {
        mode: "system".into(),
        url: None
    }
    .validate()
    .is_ok());
    assert!(NetworkProxy {
        mode: "direct".into(),
        url: None
    }
    .validate()
    .is_ok());
    let custom = NetworkProxy {
        mode: "custom".into(),
        url: Some(" http://127.0.0.1:7890 ".into()),
    };
    assert!(custom.validate().is_ok());
    // custom 缺 url / 非 http 形态拒绝
    assert!(NetworkProxy {
        mode: "custom".into(),
        url: None
    }
    .validate()
    .is_err());
    assert!(NetworkProxy {
        mode: "custom".into(),
        url: Some("socks5://x".into())
    }
    .validate()
    .is_err());
    // 未知模式拒绝
    assert!(NetworkProxy {
        mode: "proxy".into(),
        url: None
    }
    .validate()
    .is_err());
    // 归一：system/direct 不带 url；custom 去 blanks
    assert_eq!(
        NetworkProxy {
            mode: "direct".into(),
            url: Some("http://x".into())
        }
        .normalized()
        .url,
        None
    );
    assert_eq!(
        NetworkProxy {
            mode: "custom".into(),
            url: Some(" http://a:1 ".into())
        }
        .normalized()
        .url
        .as_deref(),
        Some("http://a:1")
    );
}
