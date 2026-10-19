//! 记忆注入区（⑪，Context and Memory.md §6.2）设置测试：缺省值 /
//! 旧文件兼容 / 部分更新语义 / 校验拒绝 / 刀④b 两枚行为开关。
//! 树与压缩那两组键（M2 整理三键 / M4 尾巴策略 + collapse）自 S1 刀④b 起住兄弟簇
//! `memory_tree.rs`：本文件被它们顶到 487 行，而两组键与注入区本体各自都还在长。

use super::*;

// ---- 记忆注入区（⑪，Context and Memory.md §6.2） ----

#[test]
fn memory_defaults_legacy_compat_and_validation() {
    let root = temp_root("memory");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省：开 / 注入 5 条
    let v = store.view().memory;
    assert!(v.enabled);
    assert_eq!(v.top_k, 5);
    // memory_inject_config 桥接（MemoryInject 共享锁内容物）
    let cfg = store.memory_inject_config();
    assert!(cfg.enabled);
    assert_eq!(cfg.top_k, 5);
    // 旧 settings.json 无 memory 字段 → 字段级缺省，不解析失败
    std::fs::write(root.join(FILE_NAME), r#"{"locale":"zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().memory;
    assert!(v.enabled);
    assert_eq!(v.top_k, 5);
    // 更新 + 磁盘往返
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                enabled: Some(false),
                top_k: Some(3),
                organize_pacing: None,
                organize_chunk_tokens: None,
                memory_manager_model: None,
                memory_manager_provider: None,
                tail_strategy: None,
                tail_window_turns: None,
                collapse_enabled: None,
                collapse_threshold_pct: None,
                files: None,
                repo_context: None,
                global_layer: None,
                project_layer: None,
                instruction_files: None,
                instruction_global_kib: None,
                instruction_project_kib: None,
                fact_cards: None,
                fact_card_max: None,
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert!(!store.view().memory.enabled);
    assert_eq!(store.view().memory.top_k, 3);
    let cfg = store.memory_inject_config();
    assert!(!cfg.enabled);
    assert_eq!(cfg.top_k, 3);
    assert_eq!(SettingsStore::open(&root).unwrap().view().memory.top_k, 3);
    // 部分更新：只给 top_k，enabled 保持
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                top_k: Some(8),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert!(!store.view().memory.enabled);
    assert_eq!(store.view().memory.top_k, 8);
    // 校验：top_k 出界拒绝且不污染内存态
    for bad in [0, 21] {
        assert!(store
            .update(
                None,
                None,
                None,
                None,
                None,
                None,
                Some(MemoryUpdate {
                    top_k: Some(bad),
                    ..Default::default()
                }),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .is_err());
    }
    assert_eq!(store.view().memory.top_k, 8);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn retired_keys_are_phantom() {
    let root = temp_root("m5keys");
    SettingsStore::open(&root).unwrap(); // 建目录与缺省文件，再覆盖夹具
                                         // 幻影键（v2.136 退役三键 + v2.137 defaultOrganizer）：存量
                                         // settings.json 带着旧键照常解析，投影不出现、非退役键照常生效
    std::fs::write(
        root.join(super::FILE_NAME),
        r#"{"memory":{"autoExtract":false,"handoffUpdate":false,"experiences":true,"defaultOrganizer":"legacy","topK":7}}"#,
    )
    .unwrap();
    let v = SettingsStore::open(&root).unwrap().view().memory;
    assert_eq!(v.top_k, 7, "非退役键照常生效");
    let json = serde_json::to_value(&v)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    for key in [
        "autoExtract",
        "handoffUpdate",
        "experiences",
        "defaultOrganizer",
    ] {
        assert!(!json.contains_key(key), "退役键 {key} 不进投影");
    }
    // 本批新增的两个键必须在投影里（管理页读的是 view 面，缺一个就是界面空白）
    for key in ["factCards", "factCardMax"] {
        assert!(json.contains_key(key), "新键 {key} 必须进投影");
    }
    std::fs::remove_dir_all(&root).ok();
}

// ---- S1 刀④b 两枚行为开关（ADR-0007 §18）：缺省 / 存量兼容 / 往返 ----

/// 这里只判**设置面**那三件事，它们各自会漂且没有别的判据兜着；关档之后的运行时
/// 效果在 `memory/tests/layer_switches.rs`。
///
/// ① 缺省 = 开（= 加开关之前唯一的形状）。这一条是那枚常量的**唯一守卫**：变异实测
///    （`tmp/mut-rust.mjs` CASE 1）把它改成 `false` 时既有注入用例**一条都不红**——它们
///    构造的是 `MemoryStore::new` 里那枚未接线缺省，与 settings 不同源。⇒ 两处缺省要同向，
///    办法是各有一枚守卫，不是"其中一处红了另一处自然跟着对"。
/// ② 存量 `settings.json` 没有这两个键 ⇒ 走字段级缺省，不解析失败：老用户升级之后看到的
///    必须是两枚**开着**的开关，否则"我什么都没动，记忆怎么不进了"无从解释。
/// ③ 保存 → 内存态 → 重开读回，三处都得是 false；且部分更新（只给一枚）不许把另一枚顶回
///    缺省——那是 `Option` 语义在这两枚上的同一条纪律。
///
/// 走 `set_memory`（本目录 `mod.rs` 的夹具）而不是手抄那 13 个位置参数：那道"同一个动作
/// 不许有两个出处"的判据就是为这个姿势立的（Q9 抄过一次，第五道闸当场报双向两条红）。
#[test]
fn layer_switches_default_on_and_roundtrip() {
    let root = temp_root("memory-layers");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().memory;
    assert!(v.global_layer, "全局层缺省 = 开（= 加开关之前唯一的形状）");
    assert!(v.project_layer, "项目层缺省 = 开（同上）");
    // ② 存量兼容：只带一枚的旧文件照常解析，缺的那一枚走字段级缺省
    std::fs::write(root.join(FILE_NAME), r#"{"memory":{"globalLayer":true}}"#).unwrap();
    let legacy = SettingsStore::open(&root).unwrap().view().memory;
    assert!(legacy.global_layer);
    assert!(legacy.project_layer, "缺键不许被当成 false");
    // ③ 两枚一起关 → 内存态 → 重开读回（读回 = 永久保存的硬证明）
    set_memory(
        &store,
        MemoryUpdate {
            global_layer: Some(false),
            project_layer: Some(false),
            ..Default::default()
        },
    )
    .unwrap();
    let v = store.view().memory;
    assert!(!v.global_layer && !v.project_layer, "两枚都要落进去");
    drop(store);
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().memory;
    assert!(!v.global_layer && !v.project_layer, "重开读回同样两枚关着");
    // 部分更新：只开项目层 ⇒ 全局层保持关（两枚各关各的，不是同一枚的两个名字）
    set_memory(
        &store,
        MemoryUpdate {
            project_layer: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    let v = store.view().memory;
    assert!(!v.global_layer, "另一枚不许被顶回缺省");
    assert!(v.project_layer);
    // 投影键名 = 前端取值的那两个名字：漂一个字母编译不红，界面直接空白
    let json = serde_json::to_value(&v).unwrap();
    for key in ["globalLayer", "projectLayer"] {
        assert!(json.get(key).is_some(), "投影必须含 {key}（层区按名读）");
    }
    std::fs::remove_dir_all(&root).ok();
}
