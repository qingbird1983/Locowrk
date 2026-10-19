/**
 * 文件类型图标：把文件路径认成一种语言/类型，取 `public/file-icons/<name>.svg`。
 *
 * 素材 = material-icon-theme 官方 SVG（MIT，登记见仓库根 `THIRD-PARTY-NOTICES.md`）；
 * 认名三层回退移植自 ZCode `packages/ui/src/lib/fileDisplayHelpers.ts` 的
 * `resolveIconName()`：整名 alias（`.gitignore` → git）→ 去扩展名后逐段回退
 * （`vitest.config.ts` → vitest）→ 扩展名 alias（`tsx` → react_ts）→ 兜底
 * document。**表外扩展名直接拿扩展名当图标名**，所以往 `public/file-icons/`
 * 丢一个新素材就自动生效，这里不用改。
 */

const DEFAULT_ICON = "document";

/** 整个文件名（含点开头的隐藏文件）优先于扩展名命中。 */
const FILE_NAME_ALIASES: Record<string, string> = {
  ".editorconfig": "editorconfig",
  ".env": "settings",
  ".gitattributes": "git",
  ".gitignore": "git",
  ".npmrc": "npm",
  ".nvmrc": "nodejs_alt",
  ".prettierrc": "prettier",
  ".yarnrc": "yarn",
  "babel.config": "babel",
  bun: "lock",
  "bun.lock": "lock",
  cargo: "rust",
  "cargo.lock": "lock",
  dockerfile: "docker",
  eslint: "eslint",
  "eslint.config": "eslint",
  gemfile: "gemfile",
  jest: "jest",
  "jest.config": "jest",
  makefile: "makefile",
  "package-lock": "lock",
  "pnpm-lock": "lock",
  readme: "readme",
  tsconfig: "tsconfig",
  vitest: "vitest",
  "vitest.config": "vitest",
  yarn: "yarn",
};

/** 素材名与扩展名对不上的那些（`tsx` 的素材叫 `react_ts`）。 */
const EXTENSION_ALIASES: Record<string, string> = {
  backup: "document",
  bash: "console",
  cjs: "javascript",
  cts: "typescript",
  css: "css",
  doc: "word",
  docx: "word",
  go: "go",
  html: "html",
  java: "java",
  jpeg: "image",
  jpg: "image",
  js: "javascript",
  jsx: "react",
  json: "json",
  jsonl: "json",
  mjs: "javascript",
  md: "markdown",
  m4a: "audio",
  m4v: "video",
  flac: "audio",
  mov: "video",
  mp3: "audio",
  mp4: "video",
  ogg: "audio",
  opus: "audio",
  mts: "typescript",
  pdf: "pdf",
  php: "php",
  png: "image",
  pptx: "powerpoint",
  py: "python",
  responses: "json",
  rs: "rust",
  sb: "storybook",
  sql: "database",
  sh: "console",
  snap: "snapcraft",
  svg: "svg",
  toml: "toml",
  ts: "typescript",
  tsx: "react_ts",
  txt: "document",
  wav: "audio",
  weba: "audio",
  webm: "video",
  xlsx: "table",
  yaml: "yaml",
  yml: "yaml",
  zsh: "console",
  // Locowrk 补充：上游表是按 TS 项目收紧的，这些是 Rust / Vue 仓库常碰到的
  rb: "ruby",
  ps1: "powershell",
  cs: "csharp",
  kt: "kotlin",
  kts: "kotlin",
  cpp: "cpp",
  cc: "cpp",
  cxx: "cpp",
  hpp: "cpp",
  h: "c",
  bat: "console",
  cmd: "console",
  ini: "settings",
  cfg: "settings",
  conf: "settings",
  zip: "zip",
  gz: "zip",
  tar: "zip",
  log: "document",
};

/** 取路径末段（`\` 与 `/` 都认）。 */
function leafOf(path: string): string {
  const norm = path.replace(/\\/g, "/");
  return norm.split("/").filter(Boolean).pop() ?? norm;
}

/** 文件路径 → 图标名；认不出给 `document`（该素材恒在，不会破图）。 */
export function resolveFileIcon(path: string): string {
  const leaf = leafOf(path).toLowerCase();
  const dot = leaf.lastIndexOf(".");

  // 先整名、再逐段回退 stem：`vitest.config.ts` → `vitest.config` → `vitest`
  // （配置文件名语义优先于扩展名，否则 tsconfig.base.json 会退成 json 图标）
  const candidates = [leaf];
  if (dot !== -1) {
    let stem = leaf.slice(0, dot);
    candidates.push(stem);
    while (stem.includes(".")) {
      stem = stem.slice(0, stem.lastIndexOf("."));
      if (stem) candidates.push(stem);
    }
  }
  for (const key of candidates) {
    const hit = FILE_NAME_ALIASES[key];
    if (hit) return hit;
  }

  if (dot === -1) return DEFAULT_ICON;
  const ext = leaf.slice(dot + 1);
  return EXTENSION_ALIASES[ext] ?? (ext || DEFAULT_ICON);
}

/** 图标名 → public 资源路径（vite `publicDir` = `desktop/public`）。 */
export function fileIconSrc(name: string): string {
  return `/file-icons/${name}.svg`;
}
