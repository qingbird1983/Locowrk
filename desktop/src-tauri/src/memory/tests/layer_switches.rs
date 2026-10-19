//! 两枚**行为开关**（S1 刀④b · ADR-0007 §18「全局记忆 / 项目记忆 / 项目交接层各关各的」）
//! 的宿主侧判据：缺省恒开、关档只出不进、两枚各管各的层。
//!
//! 为什么单独一簇：这一批同时动了三处（settings 字段 / `MemoryStore` 闭包 / 两个注入
//! 出口），每一处漂了都有人喊——**只有"未接线 = 恒开"这一档没人喊**：单测与降级启动都走
//! 这条，它漂成恒关的表现是"注入面用例全绿但一条都不注入"，因为每条召回用例都只断言
//! "给了什么"，没有一条断言"没接线时它给不给"。缺省漂了也没人喊，就得自己造个会喊的人。
//!
//! ⚠ 这套东西里有**两处**缺省，它们必须同向、各自都要有守卫：settings 面那枚
//! （`GLOBAL_LAYER_DEFAULT`）由 `settings/tests/memory.rs::layer_switches_default_on_and_roundtrip`
//! 守，未接线那枚（`MemoryStore::new` 里的 `Arc::new(|| true)`）由本簇第一条守。
//! 原先这里写的是"缺省漂了会红在既有注入用例上"——变异实测（`tmp/mut-rust.mjs` CASE 1）
//! 那些用例**一条都不红**，因为它们拿的是未接线那枚，与 settings 常量不同源。
//!
//! 界面那半（设置没加载完时三枚显示为开）不在本簇，写在 `components/memory/LayerSwitches.vue`。

use super::*;
use crate::recall_log::RecallLog;
use kernel_agent::{LearnWeights, RecallQuery};
use std::sync::atomic::{AtomicBool, Ordering};

/// 一枚**可现场扳动**的开关闭包。为什么用 `AtomicBool` 而不是各建一个 store：
/// 生产里这两枚是闭包（保存即生效，不重建 store），各建一个只证了"构造时读对"，
/// 证不了"每轮都现读"——后者才是当初把 `files_on` 做成闭包的理由。
fn flag() -> (Arc<AtomicBool>, Arc<dyn Fn() -> bool + Send + Sync>) {
    let f = Arc::new(AtomicBool::new(true));
    let read = {
        let f = Arc::clone(&f);
        Arc::new(move || f.load(Ordering::Relaxed)) as Arc<dyn Fn() -> bool + Send + Sync>
    };
    (f, read)
}

/// 学习档的入参：只关心 `weights.enabled = true` 这一维（其余权重取内核缺省，
/// 排序对不对由 `recall_learn.rs` 判）。
fn learn_query() -> RecallQuery {
    RecallQuery {
        terms: vec!["docker".into()],
        weights: LearnWeights {
            enabled: true,
            ..Default::default()
        },
    }
}

/// 未接线 = 恒开。这一条测的是 `MemoryStore::new` 里那两枚 `Arc::new(|| true)`：
/// 它同时是下面几条的**分母**——没有它，"关掉之后为空"可能只是因为夹具压根没接上。
#[test]
fn both_switches_default_to_open_on_an_unwired_store() {
    let dir = temp_dir("switch-unwired");
    let store = MemoryStore::new(&dir, test_resolver());
    let ws = keysid();
    store
        .write("", "pref.lang", "中文", "语言", "explicit")
        .unwrap();
    store
        .write(&ws, "build.cmd", "pnpm tauri dev", "构建", "explicit")
        .unwrap();
    assert_eq!(
        store.recall(None, 10).len(),
        1,
        "未接线的 store 必须照常注入全局层（漂成恒关 = 全套注入用例失去意义）"
    );
    assert!(
        store.project_index(Some("ws-session")).is_some(),
        "未接线的 store 必须照常注入项目索引"
    );
    assert!(
        store.global_index(Some("ws-session")).is_some(),
        "未接线的 store 必须照常注入全局索引（S3 刀① 新增的那个出口）"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 关掉全局层 ⇒ 两条召回通路都空、**一次埋点都不记**，而管理页照旧全量、写路径照旧
/// 落盘、项目层不受牵连。
///
/// "不记埋点"是这条用例的名字所在：门如果装在 `recall_once` 的 filter 里（而不是
/// `recall_quiet` 顶部早退），交付同样是空表，但账本上会留下一行"候选 0 条、注入 0 条"
/// ——那个形状与"这层本来就没记过东西"一模一样，指标会把"用户关了开关"读成"记忆是空的"。
#[test]
fn closing_the_global_layer_stops_injection_without_touching_the_layer() {
    let dir = temp_dir("switch-global-off");
    let log = RecallLog::at(dir.join("usage.sqlite3"));
    let (global, global_on) = flag();
    let (_keep, project_on) = flag();
    let store = wired(&dir)
        .with_recall_log(log.clone())
        .with_layer_switches(global_on, project_on);
    let ws = keysid();
    store
        .write("", "pref.lang", "中文", "语言", "explicit")
        .unwrap();
    store.write(&ws, "build.cmd", "v", "", "explicit").unwrap();

    // 前提（开关开着）：一条注入 + 一行账 + 项目层那节在场 + 管理页两层全量。
    // ⚠ 埋点那一维**每轮换一个 session id**：`memory_inject_log` 的唯一键含 session_id
    // （inject_row.rs 那两条就靠它数行数），全用 `None` 的话同一会话只留一行，
    // "关档那轮多记了一行"根本数不出来——本批第一版就是这么打的空（CASE 3 绿），
    // 那句"不记埋点"当时是断言不是证据。
    assert_eq!(
        store.recall(Some("s-open"), 10).len(),
        1,
        "开关开着：全局层照常进"
    );
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "前提：埋点通路确实接上了（一轮一行）"
    );
    assert!(store.project_index(Some("ws-session")).is_some());
    assert_eq!(store.list("", 200).unwrap().len(), 2, "两层都在列表里");

    // 扳到关（同一个 store，不重建 ⇒ 证的是"每轮现读"而不是"构造时读一次"）
    global.store(false, Ordering::Relaxed);
    assert!(
        store.recall(Some("s-closed"), 10).is_empty(),
        "关 = 全局层不出内容（S3 刀① 之后它的注入出口是 `global_index`，见下面那条）"
    );
    assert!(
        store
            .recall_relevant(Some("s-closed-rel"), 10, &learn_query())
            .is_empty(),
        "关掉全局层不等于换一条通路继续注入"
    );
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "关档那两轮都不许记账（早退，而不是留一行'候选 0 条、注入 0 条'）"
    );
    // 只出不进：列表仍两层全量。这一句同时是"文件没被动过"的证据——列举的读源就是 md。
    assert_eq!(store.list("", 200).unwrap().len(), 2, "管理页照旧全量");
    // 写路径不吃开关：关着照样写得动，写完就看得见
    store
        .write("", "pref.new", "新写的", "", "explicit")
        .expect("关掉注入不拦写入");
    assert_eq!(
        store.list("", 200).unwrap().len(),
        3,
        "关档期间新落的那条照样可见"
    );
    // 另一枚各管各的：全局层关了，项目层那节照常
    assert!(
        store.project_index(Some("ws-session")).is_some(),
        "两枚开关不许互相牵连"
    );
    // 再扳回开：恢复即生效，不需要重启（闭包档的存在理由）。
    // 这一轮换 session id 并**要求记账**，是上面那个"仍等于 1"的正对照：
    // 没有它，"一个字节都没记"可能只是因为埋点通路整条没接上。
    global.store(true, Ordering::Relaxed);
    assert_eq!(
        store.recall(Some("s-back"), 10).len(),
        2,
        "开回来就恢复注入（含关档期间新写的那条）"
    );
    assert_eq!(
        log.stats().unwrap().injections,
        2,
        "正对照：通路还活着，前面那个 1 不是空集里的真话"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 关掉项目层 ⇒ `<project_memory_index>` 整节缺席，走的是"未归属会话"那同一个出口
/// （`None` = 块不生成，不是生成一个空块）；全局层召回不受牵连。
#[test]
fn closing_the_project_layer_removes_only_the_index_section() {
    let dir = temp_dir("switch-project-off");
    let (_keep, global_on) = flag();
    let (project, project_on) = flag();
    let store = wired(&dir).with_layer_switches(global_on, project_on);
    let ws = keysid();
    store
        .write("", "pref.lang", "中文", "", "explicit")
        .unwrap();
    store
        .write(&ws, "build.cmd", "pnpm tauri dev", "构建", "explicit")
        .unwrap();

    let out = store
        .project_index(Some("ws-session"))
        .expect("前提：项目层有货");
    assert!(out.contains("build.cmd"), "开着时指针行原样进块：{out}");
    project.store(false, Ordering::Relaxed);
    assert_eq!(
        store.project_index(Some("ws-session")),
        None,
        "关 = 整节缺席，与未归属会话走同一个出口"
    );
    assert_eq!(store.recall(None, 10).len(), 1, "全局层那枚不受牵连");
    assert_eq!(
        store.list("build.cmd", 200).unwrap().len(),
        1,
        "管理页仍列得项目层那一条"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// S3 刀① 新增的注入出口 = **全局层索引段**。它吃的是同一枚 `global_on`：关掉全局层
/// 之后若只有 `recall` 早退而索引段照出，这枚开关就成了"少注一块正文"的装饰——
/// §E 那条「关 = 该层不读、不更新、不注入」要的是整层从上下文里消失。
///
/// 三条断语各挡一种漂法：① 指针行原样进段（挡"合成了一份新的"）；② 导语必须是
/// **全局那一档**（拿 `LEAD_IN_GLOBAL` 常量本身比，不抄字面量 = 不留第二出处；
/// 挡"两层的导语漂成同一条"，那样全局段会对模型谎称"THIS project"）；
/// ③ 关档 ⇒ `None` 而不是空块，且项目段不受牵连。
#[test]
fn the_global_layer_injects_its_index_section_under_the_same_switch() {
    let dir = temp_dir("switch-global-index");
    let (global, global_on) = flag();
    let (_keep, project_on) = flag();
    let store = wired(&dir).with_layer_switches(global_on, project_on);
    let ws = keysid();
    store
        .write("", "pref.lang", "中文", "语言偏好", "explicit")
        .unwrap();
    store.write(&ws, "build.cmd", "v", "", "explicit").unwrap();

    let out = store
        .global_index(Some("ws-session"))
        .expect("前提：全局层有货就该出段");
    assert!(
        out.contains("- [pref.lang](pref.lang.md)"),
        "指针行原样进段: {out}"
    );
    assert!(
        out.starts_with(crate::memory_index::LEAD_IN_GLOBAL),
        "全局段吃全局导语: {out}"
    );
    assert!(
        !out.contains("THIS project"),
        "不许拿项目导语糊全局段: {out}"
    );
    assert!(
        !out.contains("中文"),
        "索引段只有指针，正文一个字都不进: {out}"
    );

    global.store(false, Ordering::Relaxed);
    assert_eq!(
        store.global_index(Some("ws-session")),
        None,
        "关 = 整段缺席（与「这层没记过」同一个出口，不留空段头）"
    );
    assert!(
        store.project_index(Some("ws-session")).is_some(),
        "两枚开关不许互相牵连"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 学习档（`recall_relevant` 那条还按影子库寻址的支路）吃**同一枚**门。
///
/// 单列一条而不是并进上面那条：它的早退在 `source.rs` 里位于权重判断**之前**，把门挪到
/// 判断之后（"先决定走哪条通路、再在各自通路里早退"）会让这条红而上面那条绿——两档共用
/// 一枚门这件事必须有两条独立红证。关档连库都不开是这半档的附带读法：`memory_learn`
/// 那条路是宿主最后一处按库寻址的读路（specs/05 刀②c-2 的挂账）。
#[test]
fn the_global_switch_covers_the_relevance_branch() {
    let dir = temp_dir("switch-learn-branch");
    let (global, global_on) = flag();
    let (_keep, project_on) = flag();
    let store = wired(&dir).with_layer_switches(global_on, project_on);
    // 学习档读的是影子库 ⇒ 用生产里唯一还写内容列的入口造行（`md_read.rs` 同款手法）
    store
        .reconcile_upsert("", "pref.db", "docker compose", "ops", "explicit", 1)
        .unwrap();

    global.store(false, Ordering::Relaxed);
    assert!(
        store.recall_relevant(None, 10, &learn_query()).is_empty(),
        "关掉全局层 ⇒ 学习档同样一条不给"
    );
    global.store(true, Ordering::Relaxed);
    assert_eq!(
        store.recall_relevant(None, 10, &learn_query()).len(),
        1,
        "正对照：开回来就有货（否则上面那个空是空集里的真话）"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
