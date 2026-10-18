// 思考行收起态的两个分岔（用户 2026-10-18 按 ZCode 参考图裁定）：
// **实时轮不许露思考正文**，只给「思考 · 12 秒」；重放轮没有时间戳可算，
// 退回首句摘要（`SessionEvent::Thinking` 的投影不带 ts，见
// `src/stores/chat/types.ts:75` 的口径）。这两条都只靠真机目检会来回漂，
// 钉在这里：改坏了要么正文漏进实时行，要么历史行变成一个光秃秃的「思考」。
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ThinkingRow from "../src/components/chat/ThinkingRow.vue";
import { formatDuration } from "../src/lib/format";
import { i18n } from "../src/i18n";

const mounted = (props: { text: string; ts?: number; doneTs?: number }) =>
  mount(ThinkingRow, { props, global: { plugins: [i18n] } });

describe("思考行收起态", () => {
  it("实时轮：元信息段出时长，描述段不存在（不露正文）", () => {
    const w = mounted({ text: "用户问「怎么了」——先看看上下文再答。", ts: 1000, doneTs: 13000 });
    expect(w.find(".prow__meta").text()).toContain(formatDuration(12000));
    expect(w.find(".prow__desc").exists()).toBe(false);
    w.unmount();
  });

  it("重放轮（无 ts）：不虚构时长，退回首句摘要", () => {
    const w = mounted({ text: "用户问「怎么了」——先看看上下文再答。" });
    expect(w.find(".prow__meta").exists()).toBe(false);
    expect(w.find(".prow__desc").text()).toContain("用户问");
    w.unmount();
  });
});
