import { Diff } from "../mygo";
import { events } from "../mygo";
import type { DiffDoc, DiffFile, DiffLine } from "../mygo";
import {
  buildRows,
  fileDelta,
  fileKey,
  hunkKey,
  maxLineChars,
  statusLabel,
  type Mode,
  type Row,
} from "./model";
import { languageFor, tokenize } from "./tokens";

const ROW_H = 18;
const OVERSCAN = 40;
const startedAt = performance.now();

const params = new URLSearchParams(location.search);
const tabID = params.get("tab") ?? "";
const storageKey = "diff-mode:" + tabID;

const app = document.getElementById("app");
if (!app) throw new Error("missing #app");

let doc: DiffDoc | null = null;
let mode: Mode = sessionStorage.getItem(storageKey) === "unified" ? "unified" : "side";
const folded = new Set<string>();
let rows: Row[] = [];
let mountedStart = -1;
let mountedEnd = -1;
let raf = 0;
let focusDone = false;

const header = document.createElement("header");
header.className = "diff-head";
const notices = document.createElement("div");
notices.className = "notices";
const scroller = document.createElement("div");
scroller.className = "diff-scroll";
const body = document.createElement("div");
body.className = "diff-body";
scroller.append(body);
app.append(header, notices, scroller);
scroller.addEventListener("scroll", schedule, { passive: true });
new ResizeObserver(() => {
  mountedStart = -1;
  drawWindow();
}).observe(scroller);

void load();
events.diffReload.on(() => {
  void load();
});

async function load(): Promise<void> {
  if (!tabID) {
    fatal("缺少标签页参数，无法加载变更。");
    return;
  }
  const top = scroller.scrollTop;
  setNotices(["加载中…"]);
  try {
    doc = await Diff.load(tabID);
  } catch (err) {
    fatal("读取变更失败：" + String(err));
    return;
  }
  drawHeader();
  drawNotices();
  if (doc.state !== "ok") {
    rows = [];
    body.replaceChildren();
    body.style.height = "0";
    mountedStart = -1;
    markReady();
    return;
  }
  const wasLoaded = focusDone;
  rebuildRows(false);
  if (!focusDone && doc.focus) {
    focusDone = true;
    const index = rows.findIndex((r) => r.kind === "file" && r.file?.path === doc?.focus);
    if (index >= 0) scroller.scrollTop = index * ROW_H;
  }
  focusDone = true;
  if (wasLoaded) {
    scroller.scrollTop = Math.min(top, Math.max(0, rows.length * ROW_H - scroller.clientHeight));
  }
  mountedStart = -1;
  drawWindow();
  markReady();
}

function markReady(): void {
  if (!doc) return;
  document.documentElement.dataset.diffState = doc.state;
  document.documentElement.dataset.diffFiles = String(doc.files.length);
  document.documentElement.dataset.diffLines = String(doc.stats.lines);
  document.documentElement.dataset.diffMs = String(Math.round(performance.now() - startedAt));
}

function drawHeader(): void {
  header.replaceChildren();
  if (!doc) return;
  const main = document.createElement("div");
  main.className = "head-main";
  const name = document.createElement("strong");
  name.textContent = doc.workspace || "工作区";
  main.append(name);
  if (doc.revision) {
    const rev = document.createElement("span");
    rev.className = "muted mono";
    rev.textContent = doc.revision;
    main.append(rev);
  }
  const stats = document.createElement("span");
  stats.className = "stats";
  stats.textContent = `${doc.stats.files} 个文件 +${doc.stats.additions} −${doc.stats.deletions}`;
  main.append(stats);

  const actions = document.createElement("div");
  actions.className = "head-actions";
  const seg = document.createElement("div");
  seg.className = "segmented";
  for (const [value, label] of [["side", "并排"], ["unified", "统一"]] as const) {
    const b = document.createElement("button");
    b.type = "button";
    b.textContent = label;
    b.setAttribute("aria-pressed", String(mode === value));
    b.addEventListener("click", () => {
      if (mode === value) return;
      mode = value;
      sessionStorage.setItem(storageKey, mode);
      rebuildRows(true);
      drawHeader();
    });
    seg.append(b);
  }
  const refresh = document.createElement("button");
  refresh.type = "button";
  refresh.textContent = "刷新";
  refresh.addEventListener("click", () => void load());
  actions.append(seg, refresh);
  header.append(main, actions);
}

function drawNotices(): void {
  const list: string[] = [];
  if (!doc) {
    setNotices(list);
    return;
  }
  if (doc.state === "clean") list.push("工作区没有未暂存修改。");
  else if (doc.state === "notRepo") list.push(doc.message || "当前目录不是 git 仓库。");
  else if (doc.state === "noGit") list.push(doc.message || "找不到 git。");
  else if (doc.state === "timeout") list.push(doc.message || "读取超时，请重试。");
  else if (doc.state === "error") list.push(doc.message || "git 读取失败。");
  if (doc.truncated) list.push("已截断：" + (doc.reason || "内容超过限制") + "。");
  if (doc.hidden) list.push(`已隐藏 ${doc.hidden} 个敏感文件。`);
  if (doc.skipped) list.push(`${doc.skipped} 个文件无法解析，已跳过。`);
  if (doc.state === "ok") list.push("只显示已跟踪文件的未暂存修改。");
  if (doc.state === "ok" && doc.files.length === 0) list.push("没有可显示的变更。");
  setNotices(list);
}

function setNotices(list: string[]): void {
  notices.replaceChildren();
  for (const text of list) {
    const line = document.createElement("div");
    line.className = "notice";
    line.textContent = text;
    notices.append(line);
  }
  notices.hidden = list.length === 0;
}

function fatal(message: string): void {
  rows = [];
  body.replaceChildren();
  body.style.height = "0";
  header.replaceChildren();
  const line = document.createElement("div");
  line.className = "notice error";
  line.textContent = message;
  notices.replaceChildren(line);
  notices.hidden = false;
}

function rebuildRows(keepScroll: boolean): void {
  if (!doc || doc.state !== "ok") return;
  const top = scroller.scrollTop;
  rows = buildRows(doc, mode, folded);
  body.style.height = `${rows.length * ROW_H}px`;
  body.style.width = `${bodyWidth()}px`;
  mountedStart = -1;
  if (keepScroll) scroller.scrollTop = top;
  drawWindow();
}

function bodyWidth(): number {
  const chars = maxLineChars(rows);
  const code = Math.max(320, chars * 8 + 40);
  const total = mode === "side" ? code * 2 + 120 : code + 120;
  return Math.max(scroller.clientWidth, total);
}

function schedule(): void {
  if (raf) return;
  raf = requestAnimationFrame(() => {
    raf = 0;
    drawWindow();
  });
}

function drawWindow(): void {
  if (rows.length === 0) {
    body.replaceChildren();
    mountedStart = -1;
    return;
  }
  const viewport = scroller.clientHeight || 600;
  const first = Math.max(0, Math.floor(scroller.scrollTop / ROW_H) - OVERSCAN);
  const last = Math.min(rows.length, Math.ceil((scroller.scrollTop + viewport) / ROW_H) + OVERSCAN);
  if (first === mountedStart && last === mountedEnd) return;
  mountedStart = first;
  mountedEnd = last;
  const frag = document.createDocumentFragment();
  for (let i = first; i < last; i++) {
    const row = rows[i];
    if (row) frag.append(rowElement(row, i));
  }
  body.replaceChildren(frag);
}

function rowElement(row: Row, index: number): HTMLElement {
  const el = document.createElement("div");
  el.className = "row " + row.kind;
  el.style.top = `${index * ROW_H}px`;
  if (row.kind === "file" && row.file) fileRow(el, row.file);
  else if (row.kind === "hunk" && row.hunk && row.file && row.hunkIndex !== undefined) {
    hunkRow(el, row.file, row.hunkIndex);
  } else if (row.kind === "line") lineRow(el, row);
  else if (row.kind === "meta") {
    const text = document.createElement("span");
    text.className = "meta-text";
    text.textContent = row.text ?? "";
    el.append(text);
  }
  return el;
}

function fileRow(el: HTMLElement, file: DiffFile): void {
  const fold = document.createElement("button");
  fold.type = "button";
  fold.className = "fold";
  fold.textContent = folded.has(fileKey(file)) ? "▸" : "▾";
  fold.title = "折叠文件";
  fold.addEventListener("click", () => toggleFold(fileKey(file)));
  el.append(fold);

  const pathEl = document.createElement("span");
  pathEl.className = "path";
  pathEl.textContent = file.path;
  el.append(pathEl);

  const status = document.createElement("span");
  status.className = "badge";
  status.textContent = statusLabel(file.status);
  el.append(status);

  if (file.status === "binary") {
    const bin = document.createElement("span");
    bin.className = "muted";
    bin.textContent = "二进制文件，不显示内容";
    el.append(bin);
  } else {
    const delta = fileDelta(file);
    const stats = document.createElement("span");
    stats.className = "delta";
    stats.textContent = `+${delta.additions} −${delta.deletions}`;
    el.append(stats);
  }

  const copy = document.createElement("button");
  copy.type = "button";
  copy.className = "copy";
  copy.textContent = "复制路径";
  copy.addEventListener("click", (ev) => {
    ev.stopPropagation();
    void Diff.copyPath(tabID, file.path).catch((err) => {
      showInlineError("复制失败：" + String(err));
    });
  });
  el.append(copy);

  el.addEventListener("click", (ev) => {
    if (ev.target === fold || ev.target === copy) return;
    toggleFold(fileKey(file));
  });
}

function hunkRow(el: HTMLElement, file: DiffFile, index: number): void {
  const key = hunkKey(file, index);
  const fold = document.createElement("button");
  fold.type = "button";
  fold.className = "fold";
  fold.textContent = folded.has(key) ? "▸" : "▾";
  fold.title = "折叠片段";
  fold.addEventListener("click", (ev) => {
    ev.stopPropagation();
    toggleFold(key);
  });
  const text = document.createElement("span");
  text.className = "hunk-head";
  text.textContent = rowHeader(file, index);
  el.append(fold, text);
  el.addEventListener("click", (ev) => {
    if (ev.target === fold) return;
    toggleFold(key);
  });
}

function rowHeader(file: DiffFile, index: number): string {
  return file.hunks[index]?.header ?? "@@";
}

function lineRow(el: HTMLElement, row: Row): void {
  const file = row.file;
  const lang = file ? languageFor(file.path) : "plain";
  if (row.line) {
    const line = row.line;
    el.classList.add(line.kind);
    el.append(num(line.oldNo), num(line.newNo), sign(line.kind), code(line.text, lang));
    return;
  }
  el.classList.add("side");
  const left = row.left;
  const right = row.right;
  if (left?.kind === "del") el.classList.add("del");
  if (right?.kind === "add") el.classList.add("add");
  el.append(num(left?.oldNo ?? 0));
  const leftCode = code(left?.text ?? "", lang);
  leftCode.classList.add("left-code");
  if (!left) leftCode.classList.add("empty");
  el.append(leftCode);
  el.append(num(right?.newNo ?? 0));
  const rightCode = code(right?.text ?? "", lang);
  rightCode.classList.add("right-code");
  if (!right) rightCode.classList.add("empty");
  el.append(rightCode);
}

function num(n: number | undefined): HTMLElement {
  const el = document.createElement("span");
  el.className = "ln";
  el.textContent = n && n > 0 ? String(n) : "";
  return el;
}

function sign(kind: string): HTMLElement {
  const el = document.createElement("span");
  el.className = "sign";
  el.textContent = kind === "add" ? "+" : kind === "del" ? "−" : " ";
  return el;
}

function code(text: string, lang: string): HTMLElement {
  const el = document.createElement("span");
  el.className = "code";
  for (const token of tokenize(text, lang)) {
    if (token.kind === "plain") {
      el.append(document.createTextNode(token.text));
      continue;
    }
    const span = document.createElement("span");
    span.className = "t-" + token.kind;
    span.textContent = token.text;
    el.append(span);
  }
  return el;
}

function toggleFold(key: string): void {
  if (folded.has(key)) folded.delete(key);
  else folded.add(key);
  rebuildRows(true);
  mountedStart = -1;
  drawWindow();
  drawHeader();
}

function showInlineError(message: string): void {
  const line = document.createElement("div");
  line.className = "notice error";
  line.textContent = message;
  notices.append(line);
  notices.hidden = false;
}
