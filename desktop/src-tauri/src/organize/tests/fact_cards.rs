//! 事实卡端到端（Q3 / MR §5 提案 A）：驱动**生产管道**（`plan_manual` +
//! `run_task`）跑一遍，验四件事——卡按 `type` 落进正确的那一层、开关关掉时
//! **发给记录员的提示词逐字节回到现役形状**、既有 key 清单真的进了提示词、
//! 降级支一个字节都不写。单元面（`organize/facts/tests.rs`）测的是映射与计数，
//! 这里测的是接线：只有走生产路径才能证明"提示词里真加了要求"和"落库真发生在
//! 落树之后而不是之前"。

use async_trait::async_trait;
use memory_tree::{MemoryManagerClient, MemoryManagerUsage};

use super::*;

/// 章节数组 + 轮级总结 + 卡段（形状照 `memory_tree::manager` 的三段契约）。
///
/// 数组覆盖 `good_reply()` 那 6 项材料——本文件的用例都只整理**一次**，理由见
/// `the_organize_prompt_lists_the_keys_that_already_exist` 头注里的 Q3b。
fn good_reply_with_facts(facts: &str) -> String {
    format!(
        "{}
===SUMMARY===
本轮结论是共享 runtime。
===FACTS===
{}",
        good_reply(),
        facts
    )
}

const TWO_CARDS: &str = r#"[{"key":"build.cmd","value":"构建走 pnpm tauri dev","type":"project","tags":"build"},
{"key":"user.pref.pkg","value":"永远用 pnpm 不要用 npm","type":"user","tags":"pref"}]"#;

/// 记账假件：按序返回脚本回复，并**留下每次收到的 (system, user)**——
/// 「关掉开关 = 提示词回到现役形状」这条等价判据只能靠比对发给模型的实参来钉，
/// 现有 `FakeClient` 把参数丢弃了（`_system`/`_user`），所以这里包装一层转发。
struct SpyClient {
    responses: Arc<StdMutex<VecDeque<Result<String, String>>>>,
    seen: Arc<StdMutex<Vec<(String, String)>>>,
}

impl SpyClient {
    fn new(responses: Vec<Result<String, String>>) -> Arc<Self> {
        Arc::new(Self {
            responses: Arc::new(StdMutex::new(responses.into())),
            seen: Arc::new(StdMutex::new(Vec::new())),
        })
    }
}

#[async_trait]
impl MemoryManagerClient for SpyClient {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
    ) -> Result<(String, MemoryManagerUsage), String> {
        self.seen
            .lock()
            .unwrap()
            .push((system.to_string(), user.to_string()));
        let _ = model;
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| "没有脚本回复".to_string())?
            .map(|raw| {
                (
                    raw,
                    MemoryManagerUsage {
                        input: 10,
                        output: 5,
                    },
                )
            })
    }
}

/// 卡片落进 `type` 指定的那一层，且整理主面不受影响（章节照落树）。
#[tokio::test]
async fn recorder_cards_land_on_the_layer_their_type_names() {
    let root = temp_root("cards-layers");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(
        &root,
        settings,
        FakeClient::new(vec![Ok(good_reply_with_facts(TWO_CARDS))]),
    );
    let task = org.plan_manual(&session(), "s-cards").expect("发起成功");
    let ws = task.workspace.clone();
    org.clone().run_task(task).await;

    let store = memory_store(&root);
    let rows = store.list("", 50).unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
    let proj = crate::workspace::scope_key(Some(&ws));
    let build = rows.iter().find(|r| r.key == "build.cmd").expect("项目卡");
    assert_eq!(build.workspace, proj, "project 类型落项目键位");
    assert_eq!(build.source, "organized", "出处必须是 organized");
    let pref = rows
        .iter()
        .find(|r| r.key == "user.pref.pkg")
        .expect("全局卡");
    assert_eq!(pref.workspace, "", "user 类型落全局层（空串键位）");

    // 主面不动：章节与游标与现役同形（卡是副产物，不许改写整理结果）
    let tree = tree_at(&root, org.settings.as_ref(), "s-cards");
    assert_eq!(tree.chapters.len(), 2, "章节照常两份");
    assert_eq!(tree.chaptered_until, 6);
    assert_eq!(tree.history[0].mode, "recorder");
    assert!(tree.summaries.len() == 1, "轮级总结也不许被卡段打断");
    let _ = std::fs::remove_dir_all(&root);
}

/// **本批最要紧的一条等价判据**：开关关掉时，①一个字节都不写库，②发给记录员的
/// `user` 段逐字节 = `memory_tree::transcript(items)`（现役形状），③`system`
/// 段本来就不该被本批碰过。只断"没写库"是不够的——那证明不了提示词没被污染
/// （提示词多一段要求 = 每轮整理都在花 token 要一份根本不会落库的东西）。
#[tokio::test]
async fn cards_switch_off_leaves_the_prompt_byte_identical_and_writes_nothing() {
    let off_root = temp_root("cards-off");
    let off_settings = settings_with(
        serde_json::json!({ "memory": { "factCards": false } }),
        &off_root,
    );
    let off_spy = SpyClient::new(vec![Ok(good_reply_with_facts(TWO_CARDS))]);
    let off = organizer(&off_root, off_settings, off_spy.clone());
    let task = off.plan_manual(&session(), "s-off").expect("发起成功");
    let items = task.items.clone();
    off.clone().run_task(task).await;

    let on_root = temp_root("cards-on");
    let on_settings = settings_with(serde_json::json!({}), &on_root); // 缺省开
    let on_spy = SpyClient::new(vec![Ok(good_reply_with_facts(TWO_CARDS))]);
    let on = organizer(&on_root, on_settings, on_spy.clone());
    let task = on.plan_manual(&session(), "s-on").expect("发起成功");
    on.clone().run_task(task).await;

    let (off_system, off_user) = off_spy.seen.lock().unwrap()[0].clone();
    let (on_system, on_user) = on_spy.seen.lock().unwrap()[0].clone();
    let baseline = memory_tree::transcript(&items);
    assert_eq!(
        off_user, baseline,
        "关掉时 user 段必须与现役 transcript 逐字节相同: {off_user}"
    );
    assert!(
        on_user.starts_with(baseline.as_str()),
        "开启时材料部分不动、只在尾部追加要求"
    );
    assert!(
        on_user.contains("===FACTS==="),
        "开启时必须把卡段契约告诉模型"
    );
    assert!(
        on_user.len() > off_user.len(),
        "开启面比关闭面长出的那一段就是要求文本"
    );
    assert_eq!(
        off_system, on_system,
        "system 段（四份家族提示词）不许因本批而变——动态内容只进 user 段"
    );

    let off_rows = memory_store(&off_root).list("", 50).unwrap();
    assert!(off_rows.is_empty(), "关掉时一张不落: {off_rows:?}");
    assert_eq!(memory_store(&on_root).list("", 50).unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(&off_root);
    let _ = std::fs::remove_dir_all(&on_root);
}

/// 查重清单真的进了提示词（稿 §5「提示词注入现有 key 清单，要求更新既有 key
/// 而非造近重复」）：库里已有的行必须**带层标**出现在发给记录员的实参里——
/// 单元面测的是 `existing_keys` 的构造，这里测的是它被拼进了**发给模型的实参**。
///
/// ⚠ 场景 = 「库里先有既有记忆，再整理一次」，不是「整理两次」。换成两次会撞上
/// 一条本批实测抓到的既有缺陷：`plan_manual`/`plan_auto` 传给 `build_items` 的是
/// **全量事件**而不是尾巴切片 ⇒ 第二次整理实测 items=7、`event` 区间 [6,12]、
/// 收束后 `chaptered_until`=13（会话总共只有 7 条事件）。照那个形状写夹具等于把
/// 缺陷当契约钉进测试，所以两次整理的场景整体退到单元面（`facts/tests.rs` 的
/// `the_same_key_upserts_instead_of_piling_up_rows` 钉 upsert）。缺陷登记为队列
/// **Q3b**，不在本批顺手修——它牵动 organize 全族的触发判定与章节区间口径。
#[tokio::test]
async fn the_organize_prompt_lists_the_keys_that_already_exist() {
    let root = temp_root("cards-dedup");
    let memory = memory_store(&root);
    // 写入侧给**原始串**：`write()` 自己归一（v2.186），落点与
    // `existing_keys` 读的归一键位同一个串——夹具照生产构造代码抄，不照想象抄。
    memory
        .write(
            r"F:\ws\demo",
            "arch.store",
            "sqlite 单文件",
            "架构",
            "explicit",
        )
        .unwrap();
    memory
        .write("", "user.lang", "中文直给", "", "explicit")
        .unwrap();
    memory
        .write(
            r"F:\ws\elsewhere",
            "other.proj.key",
            "别的项目的键",
            "",
            "explicit",
        )
        .unwrap();
    let settings = settings_with(serde_json::json!({}), &root);
    let spy = SpyClient::new(vec![Ok(good_reply_with_facts(TWO_CARDS))]);
    let org = organizer(&root, settings, spy.clone());
    let task = org.plan_manual(&session(), "s-dup").expect("发起成功");
    org.clone().run_task(task).await;

    let user = spy.seen.lock().unwrap()[0].1.clone();
    assert!(
        user.contains("arch.store (project)"),
        "本项目既有键必须带 project 层标进实参: {user}"
    );
    assert!(
        user.contains("user.lang (global)"),
        "全局层既有键也要进实参: {user}"
    );
    assert!(
        !user.contains("other.proj.key"),
        "别的项目的同名族键不许混进本项目的清单"
    );
    assert!(!user.contains("（暂无）"), "有货时不许再写「暂无」");

    // 既有 3 行 + 本次两张新键 = 5 行（落库不碰别的项目）
    let rows = memory.list("", 50).unwrap();
    assert_eq!(rows.len(), 5, "{rows:?}");
    let _ = std::fs::remove_dir_all(&root);
}

/// 降级支（规则切分）**一个字节都不写**：稿 §5「规则切分路径产不出事实卡就
/// 产不出，诚实缺省，不硬造」。这条判据盯的是"降级支没有 persist 调用点"这个
/// 形状——如果哪天有人给规则切分硬造卡，这里会红。
#[tokio::test]
async fn rule_split_writes_no_cards() {
    let root = temp_root("cards-rule");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(
        &root,
        settings,
        FakeClient::new(vec![Ok("我不会".into()); 4]),
    );
    let task = org
        .plan_manual(&session(), "s-rule-cards")
        .expect("发起成功");
    org.clone().run_task(task).await;

    let tree = tree_at(&root, org.settings.as_ref(), "s-rule-cards");
    assert_eq!(tree.history[0].mode, "rule", "确实走了降级支");
    assert!(
        memory_store(&root).list("", 50).unwrap().is_empty(),
        "降级支不许写任何记忆行"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 「没写卡段」是**合法输出**而不是失败：模型只回章节 + 总结（v2.185 之前的
/// 全部形状）时，整理照常、卡片零张，且这不算任何异常。
#[tokio::test]
async fn a_reply_without_a_cards_section_is_a_normal_outcome() {
    let root = temp_root("cards-absent");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(
        &root,
        settings,
        FakeClient::new(vec![Ok(good_reply_with_summary())]),
    );
    let task = org.plan_manual(&session(), "s-nocards").expect("发起成功");
    org.clone().run_task(task).await;

    let tree = tree_at(&root, org.settings.as_ref(), "s-nocards");
    assert_eq!(tree.chapters.len(), 2);
    assert_eq!(tree.summaries.len(), 1, "旧形状回复的总结照样收");
    assert!(memory_store(&root).list("", 50).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&root);
}
