// IPC 载荷夹具：**逐字段照 `src/types/memory.ts` 的 `InjectView` 抄**，而那份类型
// 的生产真源是 `desktop/src-tauri/src/inject_view.rs`（snake_case 与既有 IPC 投影同形）。
// ⚠ 数字取**自洽**的一组，由 `tests/inject-fixture.spec.ts` 重算校验，避免夹具本身讲一个
//   后端不会讲的故事：weights 千分比 [400,250,200,150] × signals 千分比 [900,500,300,100]
//   ⇒ `total = Σ wᵢ·sᵢ`（**千分比×千分比，不除 1000**，同 `memory_learn/signals.rs::score`）
//   = 360000+125000+60000+15000 = **560000**；
//   `shareᵢ = wᵢ·sᵢ·1000/total` 截断后 [642,223,107,26]（和 998），
//   余量 2 归最大一档（`explain.rs::share_permille` 的口径）⇒ **[644,223,107,26]**，相加 = 1000。
//   ⚠ 本行注释在第一版写的是「加权和 = 360+125+60+15 = 560」——那把它除以了 1000，
//   于是夹具里的 `total` 少三个零，被自己立的自检用例当场判红（本批判据自纠第 143 条）。
// `ts` 取"相对现在的 60 秒前"而不是写死绝对值——`relativeTimeSec` 与生产同一把尺
// （历批第 117 条：夹具时间基准与生产不同源 = 判据判的是夹具）。
import type { InjectView } from "../src/types";

export const VIEW_WITH_ROW: InjectView = {
  overview: {
    injections: 3,
    injected_keys: 5,
    yielded_keys: 4,
    store_empty_ratio: 0.2,
    injection_yield: 0.8,
  },
  explain: {
    present: true,
    unreadable: false,
    ts: Math.floor(Date.now() / 1000) - 60,
    session_key: "desktop-fixture",
    scope_id: "f:\\warehouse\\demo",
    weights: [400, 250, 200, 150],
    rows: [{ key: "user-role", share: [644, 223, 107, 26], total: 560000 }],
  },
};

/** 关档那次注入的形状：整块解释不在场（列是 NULL），面板该说"还没有可解释的注入"。 */
export const VIEW_NOT_PRESENT: InjectView = {
  ...VIEW_WITH_ROW,
  explain: {
    present: false,
    unreadable: false,
    ts: 0,
    session_key: "",
    scope_id: "",
    weights: [],
    rows: [],
  },
};

/** 有回执但字节读不出来：与"没有"是**两个**结论，界面文案不共用。 */
export const VIEW_UNREADABLE: InjectView = {
  ...VIEW_NOT_PRESENT,
  explain: { ...VIEW_NOT_PRESENT.explain, present: true, unreadable: true },
};
