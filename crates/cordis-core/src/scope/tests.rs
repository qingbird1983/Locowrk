//! ScopeKey 身份语义与 ScopedLayers 分层行为（§3.4 / §9.3）。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dashmap::DashMap;

use super::{ScopeKey, ScopedLayers};

#[test]
fn scope_key_identity_is_pointer_based() {
    let a = ScopeKey::new();
    let a_clone = a.clone();
    let b = ScopeKey::new();

    assert_eq!(a, a_clone, "同一来源的克隆必须相等");
    assert_ne!(a, b, "不同来源的键即使内容同型也不相等");
    assert_ne!(a, b);
}

#[test]
fn scope_key_hash_matches_equality() {
    use std::collections::HashSet;

    let a = ScopeKey::new();
    let b = ScopeKey::new();
    let mut set = HashSet::new();
    set.insert(a.clone());
    set.insert(a.clone());
    set.insert(b.clone());
    // 三个键 → 两个桶；若 derive(Hash)（对内层 () 求值）会挤成 1 个
    assert_eq!(set.len(), 2);
    assert!(set.contains(&a.clone()));
}

#[test]
fn scoped_layers_lazy_creation_and_lookup() {
    let layers: ScopedLayers<DashMap<u32, String>> = ScopedLayers::new();
    let key = ScopeKey::new();

    assert!(layers.scoped_is_empty(), "初始没有任何 scoped 层");
    assert!(
        layers
            .with_scoped(&key, |l| l.get(&1).map(|r| r.value().clone()))
            .is_none()
    );

    layers.layer_mut(&key).insert(1, "one".into());

    assert!(!layers.scoped_is_empty(), "首次写入后层已惰性创建");
    assert_eq!(
        layers.with_scoped(&key, |l| l.get(&1).map(|r| r.value().clone())),
        Some("one".to_string())
    );

    let other = ScopeKey::new();
    assert!(
        layers
            .with_scoped(&other, |l| l.get(&1).map(|r| r.value().clone()))
            .is_none(),
        "其他作用域看不到这层"
    );
    assert_eq!(layers.global().len(), 0, "global 层不受影响");
}

/// 模拟 Context 的用法：scoped 命中优先，未命中回退 global（§3.4 use_scoped）。
#[test]
fn scoped_lookup_falls_back_to_global() {
    let layers: ScopedLayers<DashMap<u32, Arc<AtomicUsize>>> = ScopedLayers::new();
    let key = ScopeKey::new();
    layers.global().insert(1, Arc::new(AtomicUsize::new(7)));

    let lookup = |k: &ScopeKey| -> Option<Arc<AtomicUsize>> {
        if !layers.scoped_is_empty() {
            if let Some(v) = layers.with_scoped(k, |l| l.get(&1).map(|r| r.value().clone())) {
                return Some(v);
            }
        }
        layers.global().get(&1).map(|e| e.value().clone())
    };

    // 快路径：没有任何 scoped 层时直接走 global（§9.3）
    assert_eq!(lookup(&key).map(|a| a.load(Ordering::Relaxed)), Some(7));

    // 层存在但键缺失 → 仍回退 global
    layers.layer_mut(&key);
    assert_eq!(lookup(&key).map(|a| a.load(Ordering::Relaxed)), Some(7));

    // 层内命中 → 遮蔽 global
    layers
        .layer_mut(&key)
        .insert(1, Arc::new(AtomicUsize::new(9)));
    assert_eq!(lookup(&key).map(|a| a.load(Ordering::Relaxed)), Some(9));
}
