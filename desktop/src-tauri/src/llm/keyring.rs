//! API Key 系统凭据库（批 C，供应商稿 §8.4，LLM.md v3.3）：service =
//! `"Locowrk"`，account = provider id（Windows = Credential Manager）。
//! 约定三条：**惰性迁移**（settings 加载时明文 Key → 凭据库 → 换标记，
//! 幂等）；**双读回落**（存量明文优先，标记在位走 `keyring.get`）；
//! **失败回落文件**（凭据库不可用 / 写失败 → 明文留在 settings + warn，
//! 如实降级不装加密）。IPC 面不变——api_key 永不过线，只露
//! `api_key_set` 布尔（`ProviderProfile::has_key` 两态都点亮）。
//!
//! 纯逻辑与凭据库访问分离（`*_with` 闭包注入）：单元测试不碰真实
//! Credential Manager。

use crate::settings::{LlmConfig, ProviderProfile};

/// 凭据库标记（`key_storage` 唯一合法值；None = 文件存储）。
pub const KEYRING_MARK: &str = "keyring";
/// 凭据库 service 名（测试构建不接凭据库，此常量随之仅生产构建使用）。
#[cfg_attr(test, allow(dead_code))]
const SERVICE: &str = "Locowrk";

fn entry(provider_id: &str) -> Result<keyring::Entry, String> {
    // 单元测试构建不碰真实凭据库（SettingsStore::open 的惰性迁移会在
    // 测试里把夹具明文 Key 写进开发机 Credential Manager = 副作用）；
    // 测试构建一律按"凭据库不可用"回落文件存储路径（该路径本身有测试
    // 与生产语义）。生产构建正常接入。
    #[cfg(test)]
    {
        let _ = provider_id;
        Err("测试构建不接凭据库".into())
    }
    #[cfg(not(test))]
    keyring::Entry::new(SERVICE, provider_id).map_err(|e| format!("凭据库不可用: {e}"))
}

/// 读：明文优先 → 标记在位走凭据库（读失败 = warn + None，如实降级）。
pub fn resolve(p: &ProviderProfile) -> Option<String> {
    resolve_with(p, &get_password)
}

fn resolve_with(
    p: &ProviderProfile,
    get: &dyn Fn(&str) -> Result<String, String>,
) -> Option<String> {
    if let Some(k) = p
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        return Some(k.to_string());
    }
    if p.key_storage.as_deref() == Some(KEYRING_MARK) {
        match get(&p.id) {
            Ok(k) if !k.trim().is_empty() => return Some(k),
            Ok(_) => tracing::warn!(provider = %p.id, "凭据库里的 Key 为空，按未配置处理"),
            Err(e) => tracing::warn!(provider = %p.id, "凭据库读取失败: {e}"),
        }
    }
    None
}

fn get_password(provider_id: &str) -> Result<String, String> {
    entry(provider_id)?
        .get_password()
        .map_err(|e| format!("凭据读取失败: {e}"))
}

/// 写：成功 = true；失败（无凭据库等）= false，调用方回落文件存储。
pub fn store(provider_id: &str, key: &str) -> bool {
    store_with(provider_id, key, &set_password)
}

fn store_with(
    provider_id: &str,
    key: &str,
    set: &dyn Fn(&str, &str) -> Result<(), String>,
) -> bool {
    match set(provider_id, key) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(provider = %provider_id, "凭据库写入失败（回落文件存储）: {e}");
            false
        }
    }
}

fn set_password(provider_id: &str, key: &str) -> Result<(), String> {
    entry(provider_id)?
        .set_password(key)
        .map_err(|e| format!("凭据写入失败: {e}"))
}

/// 删（档位删除 / Key 清除时随行；best-effort——本来就不在 = 成功语义）。
pub fn delete(provider_id: &str) {
    if let Ok(e) = entry(provider_id) {
        if let Err(err) = e.delete_credential() {
            // NotFound = 已不在，其余（平台错误）才值得记
            if !matches!(err, keyring::Error::NoEntry) {
                tracing::warn!(provider = %provider_id, "凭据删除失败: {err}");
            }
        }
    }
}

/// 惰性迁移：所有带明文 Key 的档位 → 凭据库 → `api_key=None` +
/// `key_storage="keyring"`。幂等（明文不在 = 跳过）。返回是否有变更
///（settings open 据此决定立即落盘——安全改进不留明文在盘上等下一次保存）。
pub fn migrate(cfg: &mut LlmConfig) -> bool {
    migrate_with(cfg, &mut |id, key| store_with(id, key, &set_password))
}

fn migrate_with(cfg: &mut LlmConfig, set: &mut dyn FnMut(&str, &str) -> bool) -> bool {
    let mut changed = false;
    for p in &mut cfg.providers {
        let Some(key) = p
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        if set(&p.id, &key) {
            p.api_key = None;
            p.key_storage = Some(KEYRING_MARK.to_string());
            changed = true;
        }
        // 写失败：明文原样保留（回落文件存储语义），下轮启动再试
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn profile(id: &str, api_key: Option<&str>, storage: Option<&str>) -> ProviderProfile {
        ProviderProfile {
            id: id.into(),
            api_key: api_key.map(str::to_string),
            key_storage: storage.map(str::to_string),
            headers: BTreeMap::new(),
            ..Default::default()
        }
    }

    #[test]
    fn resolve_prefers_plaintext_then_keyring() {
        // 明文在场：不碰凭据库
        let p = profile("p1", Some("sk-plain"), Some(KEYRING_MARK));
        assert_eq!(
            resolve_with(&p, &|_| panic!("不应读凭据库")),
            Some("sk-plain".into())
        );
        // 标记在位 + 明文缺：走凭据库
        let p = profile("p2", None, Some(KEYRING_MARK));
        assert_eq!(
            resolve_with(&p, &|_| Ok("sk-vault".into())).as_deref(),
            Some("sk-vault")
        );
        // 无标记无明文：None
        let p = profile("p3", None, None);
        assert_eq!(resolve_with(&p, &|_| Ok("x".into())), None);
    }

    #[test]
    fn resolve_degrades_on_keyring_read_failure() {
        let p = profile("p4", None, Some(KEYRING_MARK));
        // 读失败 = None + 不 panic（回落语义，调用方按未配置走）
        assert_eq!(resolve_with(&p, &|_| Err("凭据库锁死".into())), None);
        // 库里空串 = 按未配置
        assert_eq!(resolve_with(&p, &|_| Ok("  ".into())), None);
    }

    #[test]
    fn migrate_moves_plaintext_and_marks_idempotent() {
        let mut cfg = LlmConfig {
            active: "a".into(),
            providers: vec![
                profile("a", Some("sk-1"), None),
                profile("b", None, Some(KEYRING_MARK)), // 已迁移：跳过
                profile("c", Some("  "), None),         // 空白 Key：跳过
            ],
            network_proxy: None,
            subagent: None,
        };
        let mut written: Vec<(String, String)> = Vec::new();
        let changed = migrate_with(&mut cfg, &mut |id, key| {
            written.push((id.into(), key.into()));
            true
        });
        assert!(changed);
        assert_eq!(written, vec![("a".into(), "sk-1".into())]);
        assert_eq!(cfg.profile("a").unwrap().api_key, None);
        assert_eq!(
            cfg.profile("a").unwrap().key_storage.as_deref(),
            Some(KEYRING_MARK)
        );
        // 幂等：再跑一轮零写入零变更
        let mut written2: Vec<(String, String)> = Vec::new();
        let changed2 = migrate_with(&mut cfg, &mut |id, key| {
            written2.push((id.into(), key.into()));
            true
        });
        assert!(!changed2);
        assert!(written2.is_empty());
    }

    #[test]
    fn migrate_write_failure_keeps_plaintext() {
        let mut cfg = LlmConfig {
            active: String::new(),
            providers: vec![profile("a", Some("sk-1"), None)],
            network_proxy: None,
            subagent: None,
        };
        let changed = migrate_with(&mut cfg, &mut |_, _| false);
        assert!(!changed);
        // 明文原样保留（回落文件存储），无标记
        assert_eq!(cfg.profile("a").unwrap().api_key.as_deref(), Some("sk-1"));
        assert_eq!(cfg.profile("a").unwrap().key_storage, None);
    }
}
