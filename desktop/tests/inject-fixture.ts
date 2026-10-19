// IPC 载荷夹具：**逐字段照 `src/types/memory.ts` 的 `InjectView` 抄**，而那份类型
// 的生产真源是 `desktop/src-tauri/src/inject_view.rs`（snake_case 与既有 IPC 投影同形）。
//
// ⚠ 第一版这里还有三行 `explain` 夹具（四信号占比 / 权重读数 / 回执三态），
// 那批数字的生产口径 = `memory_learn/signals.rs` 的千分比加权与
// `explain.rs::share_permille` 的余量归最大一档。学习档在 S3 刀③b 整族退役 ⇒
// **夹具与它自证的用例同批瘦身**（见 `inject-fixture.spec.ts` 的头注）。
//
// 现在剩下的是 L0 那三个派生读数。数字取**自洽**的一组，由
// `tests/inject-fixture.spec.ts` 重算校验，避免夹具本身讲一个后端不会讲的故事：
// `injections = 5`、`store_empty_ratio = 0.2` ⇒ 空注入次数 = 1（必须是整数次，
// 那个读数背后是计数而不是随手写的分数）；`injected_keys = 5`、`yielded_keys = 4`
// ⇒ `injection_yield = 0.8`（= 4/5，两个都是**按 key 去重的集合**，不是次数）。
import type { InjectView } from "../src/types";

/** 观测表里有账的形状（跑过几轮注入，其中一次交付为空）。 */
export const VIEW: InjectView = {
  overview: {
    injections: 5,
    injected_keys: 5,
    yielded_keys: 4,
    store_empty_ratio: 0.2,
    injection_yield: 0.8,
  },
};

/** 一次都没注入过的形状：两个比率是 **null**，不是 0（"没注入过"与"注入了但都空"
 *  是两个结论，界面上不许折成同一个数）。 */
export const VIEW_EMPTY: InjectView = {
  overview: {
    injections: 0,
    injected_keys: 0,
    yielded_keys: 0,
    store_empty_ratio: null,
    injection_yield: null,
  },
};
