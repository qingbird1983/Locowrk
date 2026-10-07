// 代码块语法高亮（UI shell §6.5 v4.16）：shiki github-light/dark 双主题、
// defaultColor:false CSS 变量模式——一次高亮产出 --shiki-light/--shiki-dark
// 双轨 token 色，明暗切换纯 CSS（style.css .shiki 规则），零重渲染。
// highlighter 惰性单例 + 启动预热 + 结果缓存；未就绪 / 语言未收录时返回
// null（调用方降级为无高亮单色），流式重渲自愈。
import { bundledLanguages, createHighlighter } from "shiki";
import type { BundledLanguage } from "shiki";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";

type Highlighter = Awaited<ReturnType<typeof createHighlighter>>;

const DUAL_THEMES = { light: "github-light", dark: "github-dark" } as const;

// 预载常用语言（随 createHighlighter 一次加载）；其余语言走 bundledLanguages
// 按需动态加载（vite 独立 chunk，本地磁盘无网络成本）。
const BASE_LANGS: BundledLanguage[] = [
  "typescript",
  "javascript",
  "tsx",
  "jsx",
  "rust",
  "python",
  "json",
  "shellscript",
  "html",
  "css",
  "scss",
  "vue",
  "markdown",
  "yaml",
  "toml",
  "sql",
  "c",
  "cpp",
  "csharp",
  "java",
  "go",
  "diff",
  "powershell",
  "xml",
  "ini",
];

let instance: Highlighter | null = null;
let bootPromise: Promise<void> | null = null;
const loadedLangs = new Set<string>(BASE_LANGS);
const pendingLangs = new Map<string, Promise<boolean>>();
// 结果缓存：key = lang + NUL + code。流式渲染每 chunk 全文重跑 marked，
// 高亮是其中最贵的一步，缓存把稳态成本压回纯解析。
const cache = new Map<string, string>();
const CACHE_MAX = 300;

async function boot(): Promise<void> {
  instance = await createHighlighter({
    themes: [...Object.values(DUAL_THEMES)],
    langs: BASE_LANGS,
    engine: createJavaScriptRegexEngine(),
  });
}

/** 启动预热（main.ts 调用，fire-and-forget，不阻塞首帧）。失败保持降级态。 */
export function warmupHighlighter(): void {
  if (bootPromise) return;
  bootPromise = boot().catch((e) => {
    console.warn("[highlight] shiki boot failed, codes render plain", e);
    instance = null;
  });
}

async function ensureLang(id: string): Promise<boolean> {
  if (!instance) return false;
  const existing = pendingLangs.get(id);
  if (existing) return existing;
  const loader = (
    bundledLanguages as Record<string, (() => Promise<{ default: unknown }>) | undefined>
  )[id];
  if (!loader) return false;
  const task = loader()
    .then((mod) => instance!.loadLanguage(mod.default as never))
    .then(() => true)
    .catch((e) => {
      console.warn(`[highlight] lang "${id}" load failed`, e);
      return false;
    })
    .then((ok) => {
      if (ok) loadedLangs.add(id);
      return ok;
    });
  pendingLangs.set(id, task);
  task.finally(() => pendingLangs.delete(id));
  return task;
}

/** 同步高亮：返回 code 元素 innerHTML（含双主题 CSS 变量的 span 串）；
 * 未就绪 / 语言未收录 / 无语言时返回 null。未收录语言会触发后台加载，
 * 之后的渲染轮次自愈。 */
export function highlightCode(code: string, lang: string): string | null {
  const h = instance;
  if (!h) return null;
  const id = lang.trim().toLowerCase();
  if (!id || id === "text" || id === "plaintext" || id === "txt") return null;
  const key = `${id}\u0000${code}`;
  const hit = cache.get(key);
  if (hit !== undefined) {
    cache.delete(key);
    cache.set(key, hit);
    return hit;
  }
  if (!loadedLangs.has(id)) {
    void ensureLang(id);
    return null;
  }
  let outer: string;
  try {
    outer = h.codeToHtml(code, {
      lang: id,
      themes: DUAL_THEMES,
      defaultColor: false,
    });
  } catch (e) {
    console.warn(`[highlight] render failed for lang "${id}"`, e);
    return null;
  }
  // shiki 输出整棵 <pre><code>…</code></pre>；我们保留自己的 .code-block
  // pre 结构（padding/换行策略），只取 code 内层注入。
  const inner = /^<pre[^>]*><code[^>]*>([\s\S]*)<\/code><\/pre>$/.exec(outer)?.[1];
  if (!inner) return null;
  cache.set(key, inner);
  if (cache.size > CACHE_MAX) {
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
  return inner;
}
