//! 四类型与落点层（S1 刀③a）：模型只报 `type`，**落点由 `memlayout::layer_for` 算**；
//! `Why:` / `How to apply:` 只提醒不拦写；值域外的值在落盘之前收进未分类。
//!
//! 这一簇有一半的用例是在钉"没给类型时行为一字不变"：四类型是**加档**不是改档，
//! `an_entry_with_no_type_lands_where_it_always_landed` 与
//! `an_out_of_range_type_is_folded_before_it_reaches_the_file` 两条就是那条担保的红证。

use super::*;
use plugin_api::memlayout;
use plugin_api::serde_json::json;

/// 某键位某条事实在 md 里的落点（一律经 `fact_rel_path`，测试不自拼目录形状）。
fn md_path(dir: &std::path::Path, ws: &str, key: &str) -> std::path::PathBuf {
    dir.join("memory").join(memlayout::fact_rel_path(ws, key))
}

#[test]
fn an_entry_with_no_type_lands_where_it_always_landed() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("type-legacy");
    let ws = key_of(r"F:\proj\TypeLegacy");
    store::memory_write(&proj(&ws), &json!({ "key": "k.plain", "value": "v" })).unwrap();
    // 没给 type ⇒ 落当前项目键位（四类型之前的唯一行为），文件里那一行是未分类
    let text = std::fs::read_to_string(md_path(&dir, &ws, "k.plain")).expect("落在项目层");
    assert!(text.contains("  type: \"fact\""), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn user_and_feedback_types_take_the_global_layer_without_being_told_to() {
    for ty in ["user", "feedback"] {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = temp_data_dir(&format!("type-{ty}"));
        let ws = key_of(r"F:\proj\TypeGlobal");
        let out = store::memory_write(
            &proj(&ws),
            &json!({ "key": "k.pref", "value": "永远用 pnpm", "type": ty }),
        )
        .unwrap();
        assert_eq!(out["scope"], json!("global"), "{ty} 该落全局层：{out}");
        assert!(
            md_path(&dir, "", "k.pref").exists(),
            "{ty} 的文件在 global/"
        );
        assert!(
            !md_path(&dir, &ws, "k.pref").exists(),
            "不许同时在项目层也留一份（同一条记忆落两层 = 下次改一条漏一条）"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn an_explicit_scope_still_wins_over_the_type_default() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("type-scope");
    let ws = key_of(r"F:\proj\TypeScope");
    // type=user 但显式要项目层
    let a = store::memory_write(
        &proj(&ws),
        &json!({ "key": "k.a", "value": "v", "type": "user", "scope": "project" }),
    )
    .unwrap();
    assert_eq!(a["scope"], json!("project"), "显式 scope 覆盖类型缺省：{a}");
    // type=project 但显式升格为全局偏好
    let b = store::memory_write(
        &proj(&ws),
        &json!({ "key": "k.b", "value": "v", "type": "project", "scope": "global" }),
    )
    .unwrap();
    assert_eq!(b["scope"], json!("global"), "{b}");
    assert!(md_path(&dir, &ws, "k.a").exists(), "k.a 在项目层");
    assert!(md_path(&dir, "", "k.b").exists(), "k.b 在 global/");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_type_opens_the_global_layer_for_an_unattributed_session() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("type-unattr");
    // 现状（Context 文档 §4.6）：未归属会话写"项目记忆"被拒——没有项目的记忆不许污染全局层
    let refused = store::memory_write(&WsCtx::Unattributed, &json!({ "key": "k.x", "value": "v" }));
    assert!(refused.is_err(), "无 type 无 scope ⇒ 照旧拒绝：{refused:?}");
    // 刀③a 之后多了一条**正当**出路：报 `type="user"` 说的是"这是全局偏好"，
    // 而不是"一条没处放的项目记忆"——所以它不需要再假装成 scope 参数
    let ok = store::memory_write(
        &WsCtx::Unattributed,
        &json!({ "key": "k.x", "value": "v", "type": "user" }),
    )
    .unwrap();
    assert_eq!(ok["scope"], json!("global"), "{ok}");
    assert!(md_path(&dir, "", "k.x").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_why_hint_names_what_is_missing_without_blocking_the_write() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("type-hint");
    let ws = key_of(r"F:\proj\TypeHint");
    // 用 `project` 而不是 `feedback` 跑这条：feedback 的缺省层是全局（上面那用例已经测了），
    // 这一条要看的是"提醒与落盘互不干涉"，落点留在项目层最好读。第一版我在这里写了
    // feedback 却断言项目层的文件在场，抓到一个"提醒不落盘"的假缺陷——
    // 而 assert 失败时线程正持有 `ENV_LOCK`，于是同批后面那条拿到的是 `PoisonError`
    // 而不是它自己的结论（这把夹具的连锁污染已登记在 specs/05 挂账）。
    let out = store::memory_write(
        &proj(&ws),
        &json!({ "key": "k.pb", "value": "别再直接改 sqlite", "type": "project" }),
    )
    .unwrap();
    let hint = out["hint"]
        .as_str()
        .expect("两段都缺 ⇒ 带回一句提醒")
        .to_string();
    assert!(
        hint.contains("Why:") && hint.contains("How to apply:"),
        "{hint}"
    );
    let md = md_path(&dir, &ws, "k.pb");
    assert!(md.exists(), "提醒不拦写：条目照样落盘");
    assert!(
        std::fs::read_to_string(&md)
            .unwrap()
            .contains("  type: \"project\"")
    );
    // 两段齐 ⇒ 不提醒（每次都提醒会变成噪声，模型很快学会忽略那个字段）
    let full = store::memory_write(
        &proj(&ws),
        &json!({
            "key": "k.pb2",
            "value": "Why: 上次同步把人的编辑冲掉了\nHow to apply: 写前先比对 mtime",
            "type": "project",
        }),
    )
    .unwrap();
    assert!(full.get("hint").is_none(), "两段都在场就不该再报：{full}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_undo_restores_the_value_but_keeps_the_type_that_is_in_the_file() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("type-undo");
    let ws = key_of(r"F:\proj\TypeUndo");
    store::memory_write(
        &proj(&ws),
        &json!({ "key": "k.u", "value": "第一版", "type": "project" }),
    )
    .unwrap();
    let second = store::memory_write(
        &proj(&ws),
        &json!({ "key": "k.u", "value": "第二版", "type": "project" }),
    )
    .unwrap();
    // undo 的载荷取自**库里那一行**，而库里没有 `type` 列 ⇒ `sync_write` 拿到的是 `None`。
    // 那一档的语义是"沿用现存文件里的分类"，不是"当成没分类"：后者会让一次撤销
    // 把人或模型定的类型抹平成 `fact`（值回滚与分类是两件事）。
    store::memory_undo(
        &proj(&ws),
        &contract("call-2", json!({ "key": "k.u" }), second),
    )
    .unwrap();
    let text = std::fs::read_to_string(md_path(&dir, &ws, "k.u")).unwrap();
    assert!(text.contains("第一版"), "值照旧回滚：{text}");
    assert!(
        text.contains("  type: \"project\""),
        "撤销不许把分类抹平：{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_out_of_range_type_is_folded_before_it_reaches_the_file() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = temp_data_dir("type-odd");
    let ws = key_of(r"F:\proj\TypeOdd");
    let out = store::memory_write(
        &proj(&ws),
        &json!({ "key": "k.odd", "value": "v", "type": "Preference" }),
    )
    .unwrap();
    assert_eq!(out["type"], json!("fact"), "回显的是收档之后的值：{out}");
    assert_eq!(out["scope"], json!("project"), "未分类按项目走 = 现状行为");
    assert!(
        std::fs::read_to_string(md_path(&dir, &ws, "k.odd"))
            .unwrap()
            .contains("  type: \"fact\""),
        "值域外的串不许落进真源文件"
    );
    // 值域外的 **scope** 仍然直接报错，不由类型缺省兜底——两档判据不许互相掩盖
    let bad = store::memory_write(
        &proj(&ws),
        &json!({ "key": "k.odd2", "value": "v", "scope": "worldwide" }),
    );
    assert!(bad.is_err(), "拼错的 scope 不许被静默忽略：{bad:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
