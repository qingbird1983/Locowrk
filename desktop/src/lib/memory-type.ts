/**
 * 记忆**分类**的展示映射（S1 刀④c · ADR-0007 §18 里管理页那一列）。
 *
 * 值域固定六个，而且**后端已经收过档**：`memlayout::normalize_type` 把四类型之外的一切
 * （旧文件里的 `fact`、人手写的任意串、根本没这行）一律读成 `UNCLASSIFIED_TYPE` =
 * `"fact"` ⇒ 这一层不必再判合法性，只按六个键出文案（`MemoryRow.type` 的注释同口径）。
 *
 * 未知值也回落"未分类"那一档而不是原样打印：界面上一个空徽章或一串没人看的英文，
 * 都比"这一条没分类"这句事实更难读。这里改的只有**显示**，判据仍在后端那一份。
 */
export const MEMORY_TYPE_LABEL_KEYS = {
  user: "memory.typeUser",
  feedback: "memory.typeFeedback",
  project: "memory.typeProject",
  reference: "memory.typeReference",
  fact: "memory.typeUnclassified",
} as const;

/** 一条记忆的分类 → i18n 键。 */
export function memoryTypeLabelKey(type: string): string {
  return MEMORY_TYPE_LABEL_KEYS[type as keyof typeof MEMORY_TYPE_LABEL_KEYS]
    ?? MEMORY_TYPE_LABEL_KEYS.fact;
}
