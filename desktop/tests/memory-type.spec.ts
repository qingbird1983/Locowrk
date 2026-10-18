import { describe, expect, it } from "vitest";
import { MEMORY_TYPE_LABEL_KEYS, memoryTypeLabelKey } from "../src/lib/memory-type";

/**
 * 管理页那一列的分类映射（S1 刀④c）。
 *
 * 分工说破：值域是**后端**收的档（`memlayout::normalize_type`），所以这里判的不是
 * "什么算合法分类"，而是"六个值各自连到哪个文案键、连不上的那一个是不是未分类"。
 * 键名一旦打错，界面上那一列会显示成空白——那种错在 Rust 侧、在编译期都查不出来。
 */
describe("记忆分类那一列", () => {
  it("四类型各有各的键，互不共用", () => {
    expect(memoryTypeLabelKey("user")).toBe("memory.typeUser");
    expect(memoryTypeLabelKey("feedback")).toBe("memory.typeFeedback");
    expect(memoryTypeLabelKey("project")).toBe("memory.typeProject");
    expect(memoryTypeLabelKey("reference")).toBe("memory.typeReference");
    const keys = new Set(Object.values(MEMORY_TYPE_LABEL_KEYS));
    expect(keys.size).toBe(5);
  });

  it("未分类与任何连不上的值同档（前端不再判一次合法性，只显示）", () => {
    expect(memoryTypeLabelKey("fact")).toBe(MEMORY_TYPE_LABEL_KEYS.fact);
    for (const junk of ["", "FACT", "用户", "reference ", "unknown"]) {
      expect(memoryTypeLabelKey(junk), junk).toBe(MEMORY_TYPE_LABEL_KEYS.fact);
    }
  });
});
