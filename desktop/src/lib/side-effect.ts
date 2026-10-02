// 工具副作用分级（UI shell doc §3.4 副作用族 / §3.16：工具行折叠态带副作用
// 徽章）。映射表 = 插件源码逐一对出（plugins-src/*/src/lib.rs 的
// side_effect 字段 + desktop 内置 scheduler / search_tools / subagent），
// 不是前端自造；表外工具（第三方插件）返回 null = 不渲染徽章（诚实：
// 宁缺勿假，分级未知就不标）。
export type SideEffect = "pure" | "compensable" | "irreversible";

const TABLE: Record<string, SideEffect> = {
  // tool-shell（shell=Irreversible / shell_read=Pure）
  shell: "irreversible",
  shell_read: "pure",
  // tool-fs
  fs_read: "pure",
  fs_list: "pure",
  fs_grep: "pure",
  fs_write: "compensable",
  fs_delete: "irreversible",
  fs_restore: "irreversible",
  // tool-edit（三个编辑工具全部 Compensable）
  edit_replace: "compensable",
  edit_lines: "compensable",
  edit_diff: "compensable",
  // tool-git
  git_status: "pure",
  git_diff: "pure",
  git_log: "pure",
  git_commit: "compensable",
  git_push: "irreversible",
  git_undo_commit: "irreversible",
  // memory-sqlite
  memory_write: "compensable",
  memory_read: "pure",
  memory_list: "pure",
  memory_search: "pure",
  memory_delete: "compensable",
  memory_undo: "irreversible",
  // tool-echo / tool-fetch / tool-mcp / tool-shout
  echo: "pure",
  fetch: "pure",
  mcp_status: "pure",
  mcp_reload: "pure",
  shout: "pure",
  // tool-sandbox / tool-notify
  sandbox_run: "irreversible",
  notify: "irreversible",
  // desktop 宿主内置
  search_tools: "pure",
  schedule_create: "irreversible",
  schedule_list: "pure",
  schedule_delete: "pure",
  subagent_run: "irreversible",
};

/** 表外工具返回 null（不渲染徽章，不猜分级）。 */
export function toolSideEffect(tool: string): SideEffect | null {
  return TABLE[tool] ?? null;
}
