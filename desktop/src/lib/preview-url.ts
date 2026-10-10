// lwpreview 预览 URL 构造（UI shell 文档 §2.5 浏览器页签，v2.144）：
// 绝对路径 → http://lwpreview.localhost/<逐段编码>。段内编码、'/' 做分隔
// ——页面里的相对引用（./game.js、../a.css）才能落回同目录；与 Rust 侧
// browser::preview 的解码规则互为镜像（各自实现，解码宽容）。

export const PREVIEW_ORIGIN = "http://lwpreview.localhost";

/** 判定输入是不是 http(s) 地址（手动输入分流用）。 */
export function isHttpUrl(input: string): boolean {
  return /^https?:\/\//i.test(input.trim());
}

/** 绝对路径 → 预览 URL（Windows 反斜杠归一为 '/'）。 */
export function previewUrlFor(absPath: string): string {
  const segments = absPath
    .replace(/\\/g, "/")
    .split("/")
    .filter((s) => s.length > 0);
  return `${PREVIEW_ORIGIN}/${segments.map(encodeURIComponent).join("/")}`;
}
