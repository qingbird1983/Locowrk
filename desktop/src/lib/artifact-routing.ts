/** **产物 → 右栏页签的唯一判定面**（specs/08 批 2；标准 §二⑱）。
 *
 * 只认扩展名，不认工具名：工具行给的路径、展开体代码块头、diff 行号三处入口都调这里，
 * 任何组件里再写一遍 `if (ext === "md")` 都是第二套判定（标准 §二⑱ 判错）。
 *
 * `.html` 走**浏览器**页签（不是文件页签）：它要跑起来才有意义，`preview-url.ts` 的
 * `lwpreview` 协议本来就为本地页面准备（用户 2026-10-18：「点击 html，浏览器打开」）。
 * 二进制文件仍然进**文件**页签——是不是文本由后端 `file_read_text` 判，前端不猜。 */

export type ArtifactRoute = "file" | "image" | "browser";

/** 图片扩展名（小写、不带点）。 */
const IMAGE_EXT = new Set([
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "svg",
  "bmp",
  "avif",
]);

const HTML_EXT = new Set(["html", "htm"]);

/** 取扩展名（小写、不带点；无扩展名回空串）。 */
export function extOf(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** 路径 → 该开哪种页签。 */
export function routeArtifact(path: string): ArtifactRoute {
  const ext = extOf(path);
  if (HTML_EXT.has(ext)) return "browser";
  if (IMAGE_EXT.has(ext)) return "image";
  return "file";
}
