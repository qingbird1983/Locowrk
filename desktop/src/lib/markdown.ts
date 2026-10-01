// 助手正文 markdown 渲染（UI shell doc §6.5）：marked 解析 + DOMPurify 消毒
// + 代码块增强（语言标签头行 + 复制按钮，原型 .code-block 形态）。
// 表格 / 列表 / 行内代码的样式由组件层的 .md 规则承担。
import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({ gfm: true, breaks: true });

const COPY_ICON =
  '<svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="6" y="6" width="7.4" height="7.4" rx="1.6"/><path d="M10.2 3.2H4.4a1.4 1.4 0 0 0-1.4 1.4v5.8"/></svg>';

/** 解析 + 消毒 + 代码块增强，返回可直接 v-html 的 HTML。labels = 复制按钮
 * 的 title/aria 文案（调用方从 i18n 传入）。 */
export function renderMarkdown(
  text: string,
  labels?: { copyCode?: string },
): string {
  const raw = marked.parse(text, { async: false }) as string;
  const clean = DOMPurify.sanitize(raw);
  const tpl = document.createElement("template");
  tpl.innerHTML = clean;
  for (const pre of Array.from(tpl.content.querySelectorAll("pre"))) {
    const code = pre.querySelector("code");
    if (!code) continue;
    const lang = (
      Array.from(code.classList).find((c) => c.startsWith("language-")) ?? ""
    )
      .slice("language-".length)
      .trim();
    const fig = document.createElement("figure");
    fig.className = "code-block";
    const head = document.createElement("figcaption");
    head.className = "code-block__head";
    const label = document.createElement("span");
    label.textContent = lang || "text";
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "copy-btn";
    btn.title = labels?.copyCode ?? "复制代码";
    btn.setAttribute("aria-label", labels?.copyCode ?? "复制代码");
    btn.innerHTML = COPY_ICON;
    head.append(label, btn);
    pre.replaceWith(fig);
    fig.append(head, pre);
  }
  return tpl.innerHTML;
}
