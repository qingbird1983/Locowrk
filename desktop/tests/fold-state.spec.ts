// 层 3（单行展开体）展开记忆（设计稿 §8，台账 AF-12）：记忆必须活在组件外，
// 否则过程行一重投影（`v-for` 重建）用户刚展开的明细就自己收起来。
import { beforeEach, describe, expect, it } from "vitest";
import {
  isRowExpanded,
  isRowMounted,
  resetRowFolds,
  toggleRow,
} from "../src/lib/chat/fold-state";

describe("层 3 展开记忆", () => {
  beforeEach(() => resetRowFolds());

  it("一个 id 同时管「展开着」与「挂载过」", () => {
    expect(isRowMounted(7)).toBe(false);
    toggleRow(7);
    expect(isRowExpanded(7)).toBe(true);
    expect(isRowMounted(7)).toBe(true);
    toggleRow(7);
    expect(isRowExpanded(7)).toBe(false);
    // 收起不等于没挂载过 —— 懒挂载语义（保 DOM）靠这一位
    expect(isRowMounted(7)).toBe(true);
  });

  it("切会话清空：层 3 记忆不跨会话", () => {
    toggleRow(11);
    resetRowFolds();
    expect(isRowExpanded(11)).toBe(false);
    expect(isRowMounted(11)).toBe(false);
  });

  it("不同行互不串味", () => {
    toggleRow(1);
    expect(isRowExpanded(2)).toBe(false);
    expect(isRowMounted(2)).toBe(false);
  });
});
