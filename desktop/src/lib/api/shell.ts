/** **交给系统 shell 的打开面**（T39 归位）：调用操作系统打开一个目录或一个链接，
 * 家族就这两条——在文件管理器里显示目录、用默认浏览器开外部网页。
 *
 * 这两条原先分居 `sessions.ts`（reveal）与 `llm.ts`（open url）：按命令族算它们同属
 * "交给 OS shell 打开"一族，后端也同一片（`commands/sessions/shell.rs`——fsRoots 白名单
 * 与 https 放行都在那一侧）。拆片口径一片装一族（T38），所以单独成片；
 * 消费点都走 `lib/api` 星号再导出，import 路径一字不改。
 */

import { invoke } from "@tauri-apps/api/core";

/** 在文件管理器中打开项目目录（§2.3 v3.9 项目菜单）：后端校验 fsRoots。 */
export function revealInExplorer(path: string): Promise<void> {
  return invoke("reveal_in_explorer", { path });
}

/** 打开外部网页链接（批 A「如何获取 API Key ↗」）；后端只放行 https。 */
export function openExternalUrl(url: string): Promise<void> {
  return invoke("open_external_url", { url });
}
