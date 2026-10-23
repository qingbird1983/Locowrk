//! `AppLlm::compaction_view`（specs/13 挂账 ⑤ 收账）：压缩台从主对话台**派生**——
//! 同一批底层句柄共享同一柄（断 `Arc::ptr_eq`，断的是柄不是值：柄不同 = "主轮能用
//! ≠ 摘要能用"那类 bug 的根，值相同而柄不同照样算漂），三枚主对话专属装饰摘掉
//! （重试通知 / 拒收读数 / 共享诊断账本 ⇒ 换独立实例）。
//!
//! 挂在本模块（`app_llm`）而不是 `llm/tests/`：断言要读私有字段（柄同一性只有
//! 字段能证），字段可见性 = 定义模块及其子孙；`llm/tests` 是兄弟不是子孙。
//! 构造面照生产链抄（`state/llm_stack.rs::build`）：with_overrides →
//! with_retry_surface → with_diagnoser → with_multimodal → with_spill →
//! with_refusals，一枚不缺——派生读数的对照面缺一档就少证一柄。

use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::assets::AssetStore;
use crate::compact::Overflow;
use crate::model_override::ModelOverrideStore;
use crate::spill::SpillStore;
use crate::stop::StopFlags;

#[test]
fn compaction_view_shares_handles_and_drops_main_decorations() {
    let root = crate::llm::tests::temp_root("compaction-view");
    let settings = Arc::new(SettingsStore::open(&root).unwrap());
    let overrides = Arc::new(ModelOverrideStore::new(root.clone()));
    let stop = Arc::new(StopFlags::default());
    let notify: RetryNotify = Arc::new(|_, _, _| {});
    let diagnoser = Arc::new(crate::llm::diagnose::Diagnoser::default());
    let catalog = Arc::new(crate::llm::catalog::CatalogStore::new(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/models.dev/api.json"),
    ));
    let assets = Arc::new(AssetStore::new(root.join("assets")));
    let spill = Arc::new(SpillStore::new(&root));
    let refusals = Arc::new(Overflow::default());

    let main = AppLlm::with_overrides(Arc::clone(&settings), Arc::clone(&overrides))
        .with_retry_surface(Arc::clone(&stop), notify)
        .with_diagnoser(Arc::clone(&diagnoser))
        .with_multimodal(Arc::clone(&assets), Arc::clone(&catalog))
        .with_spill(Arc::clone(&spill))
        .with_refusals(Arc::clone(&refusals));

    let view = main.compaction_view();

    // 同柄：settings + 五枚注入件，派生共享的是**同一个 Arc**。
    assert!(Arc::ptr_eq(&view.settings, &settings), "settings 同柄");
    assert!(
        view.overrides
            .as_ref()
            .is_some_and(|o| Arc::ptr_eq(o, &overrides)),
        "会话级覆盖同柄"
    );
    assert!(
        view.stop.as_ref().is_some_and(|s| Arc::ptr_eq(s, &stop)),
        "停止标记同柄"
    );
    assert!(
        view.assets
            .as_ref()
            .is_some_and(|a| Arc::ptr_eq(a, &assets)),
        "资产存储同柄"
    );
    assert!(
        view.catalog
            .as_ref()
            .is_some_and(|c| Arc::ptr_eq(c, &catalog)),
        "能力目录同柄"
    );
    assert!(
        view.spill.as_ref().is_some_and(|s| Arc::ptr_eq(s, &spill)),
        "落盘句柄同柄（改前是压缩侧自己 new 的第二份，SpillStore 无状态 ⇒ 行为等价）"
    );

    // 摘掉的三枚装饰：通知与拒收读数归 None；诊断账本换独立实例（不与主对话那台共享）。
    assert!(view.notify_retry.is_none(), "压缩重试不许推 llm-retry");
    assert!(view.refusals.is_none(), "压缩请求被拒不记会话占用读数");
    assert!(
        !Arc::ptr_eq(&view.diagnoser, &diagnoser),
        "诊断账本独立（M6.1 那台记的是主对话 step 边界的分块 hash 链）"
    );

    // 主对话那台自身不被派生改动（派生是纯读）。
    assert!(main.notify_retry.is_some(), "派生不夺走主对话的通知");
    assert!(main.refusals.is_some(), "派生不夺走主对话的拒收读数");
}
