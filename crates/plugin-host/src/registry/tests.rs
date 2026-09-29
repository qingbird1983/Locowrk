use super::*;

#[test]
fn activate_rollback_and_deactivate_bookkeeping() {
    let reg = PluginRegistry::new();
    let v12: Version = "1.2.0".parse().unwrap();

    // 空表：activate / rollback / current 都按错误路径走（§15.4 不静默降级）
    assert!(matches!(
        reg.activate("tool-x", &v12),
        Err(LoadError::NotLoaded(_))
    ));
    assert!(matches!(
        reg.rollback("tool-x"),
        Err(LoadError::NotLoaded(_))
    ));
    assert!(reg.current("tool-x").is_none());
    assert!(!reg.has_version("tool-x", &v12));
    assert_eq!(reg.graveyard_len(), 0);

    // 未加载时的 deactivate 是无操作（幂等）
    reg.deactivate("tool-x");
    assert_eq!(reg.graveyard_len(), 0);
    assert!(reg.list_states().is_empty());
    assert!(reg.active_instances().is_empty());

    // 未激活过的插件 rollback 报 NotLoaded（有 active 但无更早版本的
    // NoPreviousVersion 路径需要真实实例，由集成测试覆盖）
    assert!(matches!(
        reg.rollback("tool-y"),
        Err(LoadError::NotLoaded(_))
    ));
}
