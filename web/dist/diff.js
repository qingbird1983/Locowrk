// node_modules/mygo-runtime/dist/index.js
function runtime() {
  const rt = globalThis.mygo;
  if (!rt)
    throw new Error("mygo: runtime not found; is this page running inside a MyGo window?");
  return rt;
}
function call(method, ...args) {
  try {
    return runtime().call(method, ...args);
  } catch (err) {
    return Promise.reject(err);
  }
}
function on(name, listener) {
  return runtime().on(name, listener);
}
function once(name, listener) {
  return runtime().once(name, listener);
}
function event(name) {
  return {
    name,
    on: (listener) => on(name, listener),
    once: (listener) => once(name, listener)
  };
}

// src/mygo.ts
var Diff = {
  /** CopyPath copies one file path to the host clipboard. */
  copyPath(tabID2, path) {
    return call("Diff.CopyPath", tabID2, path);
  },
  /**
   * Load returns the diff of a tab's workspace. It is called by the diff page
   * after it loads; expected git failures are states in DiffDoc, not errors.
   */
  load(tabID2) {
    return call("Diff.Load", tabID2);
  }
};
var events = {
  /** Event carries one project event to the page. */
  harnessEvent: event("harness-event"),
  /**
   * DiffReload asks a diff page to reload its document without losing the
   * current view mode or scroll anchor.
   */
  diffReload: event("diff-reload")
};

// src/diff/model.ts
function fileKey(file) {
  return "f:" + file.path;
}
function hunkKey(file, index) {
  return "h:" + file.path + ":" + index;
}
function buildRows(doc2, mode2, folded2) {
  const rows2 = [];
  for (const file of doc2.files) {
    rows2.push({ kind: "file", key: fileKey(file), file });
    if (folded2.has(fileKey(file))) continue;
    file.hunks.forEach((hunk, index) => {
      rows2.push({ kind: "hunk", key: hunkKey(file, index), file, hunk, hunkIndex: index });
      if (folded2.has(hunkKey(file, index))) return;
      if (mode2 === "unified") {
        pushUnified(rows2, file, hunk);
      } else {
        pushSide(rows2, file, hunk);
      }
    });
  }
  return rows2;
}
function pushUnified(rows2, file, hunk) {
  hunk.lines.forEach((line, index) => {
    rows2.push({ kind: "line", key: `${file.path}:${hunk.oldStart}:${index}`, file, hunk, line });
  });
}
function pushSide(rows2, file, hunk) {
  const lines = hunk.lines;
  let i = 0;
  let seq = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line) break;
    if (line.kind === "del") {
      const dels = [];
      while (i < lines.length && lines[i]?.kind === "del") dels.push(lines[i++]);
      const adds = [];
      while (i < lines.length && lines[i]?.kind === "add") adds.push(lines[i++]);
      const n = Math.max(dels.length, adds.length);
      for (let j = 0; j < n; j++) {
        rows2.push({
          kind: "line",
          key: `${file.path}:${hunk.oldStart}:s${seq++}`,
          file,
          hunk,
          left: dels[j],
          right: adds[j]
        });
      }
      continue;
    }
    if (line.kind === "add") {
      while (i < lines.length && lines[i]?.kind === "add") {
        rows2.push({
          kind: "line",
          key: `${file.path}:${hunk.oldStart}:s${seq++}`,
          file,
          hunk,
          right: lines[i]
        });
        i++;
      }
      continue;
    }
    if (line.kind === "context") {
      rows2.push({
        kind: "line",
        key: `${file.path}:${hunk.oldStart}:s${seq++}`,
        file,
        hunk,
        left: line,
        right: line
      });
      i++;
      continue;
    }
    rows2.push({ kind: "meta", key: `${file.path}:${hunk.oldStart}:m${seq++}`, text: line.text });
    i++;
  }
}
function fileDelta(file) {
  let additions = 0;
  let deletions = 0;
  for (const hunk of file.hunks) {
    for (const line of hunk.lines) {
      if (line.kind === "add") additions++;
      else if (line.kind === "del") deletions++;
    }
  }
  return { additions, deletions };
}
function maxLineChars(rows2) {
  let max = 0;
  for (const row of rows2) {
    if (row.kind !== "line") continue;
    for (const line of [row.left, row.right, row.line]) {
      if (line) max = Math.max(max, line.text.length);
    }
  }
  return max;
}
function statusLabel(status) {
  switch (status) {
    case "added":
      return "\u65B0\u589E";
    case "deleted":
      return "\u5220\u9664";
    case "renamed":
      return "\u91CD\u547D\u540D";
    case "binary":
      return "\u4E8C\u8FDB\u5236";
    case "mode":
      return "\u6743\u9650";
    default:
      return "\u4FEE\u6539";
  }
}

// src/diff/tokens.ts
var common = /* @__PURE__ */ new Set([
  "if",
  "else",
  "for",
  "while",
  "return",
  "break",
  "continue",
  "switch",
  "case",
  "default",
  "class",
  "interface",
  "type",
  "func",
  "function",
  "var",
  "let",
  "const",
  "new",
  "delete",
  "try",
  "catch",
  "finally",
  "throw",
  "import",
  "export",
  "from",
  "as",
  "package",
  "struct",
  "map",
  "chan",
  "go",
  "defer",
  "select",
  "range",
  "true",
  "false",
  "nil",
  "null",
  "undefined",
  "this",
  "self",
  "async",
  "await",
  "yield",
  "public",
  "private",
  "protected",
  "static",
  "final",
  "void",
  "int",
  "string",
  "bool",
  "byte",
  "float",
  "double",
  "long",
  "pub",
  "use",
  "mod",
  "impl",
  "fn",
  "match",
  "enum",
  "trait",
  "where",
  "loop",
  "and",
  "or",
  "not",
  "in",
  "is",
  "with",
  "lambda",
  "pass",
  "raise",
  "def",
  "elif",
  "except",
  "None",
  "True",
  "False",
  "echo",
  "then",
  "fi",
  "do",
  "done"
]);
var langs = {
  go: { lineComment: "//", block: ["/*", "*/"], quotes: ['"', "'", "`"], keywords: common },
  ts: { lineComment: "//", block: ["/*", "*/"], quotes: ['"', "'", "`"], keywords: common },
  js: { lineComment: "//", block: ["/*", "*/"], quotes: ['"', "'", "`"], keywords: common },
  json: { lineComment: void 0, quotes: ['"'], keywords: /* @__PURE__ */ new Set(["true", "false", "null"]) },
  css: { block: ["/*", "*/"], quotes: ['"', "'"], keywords: /* @__PURE__ */ new Set(["import", "media", "supports", "var"]) },
  html: { block: ["<!--", "-->"], quotes: ['"', "'"], keywords: /* @__PURE__ */ new Set(["html", "head", "body", "div", "span", "script", "style"]) },
  sh: { lineComment: "#", quotes: ['"', "'"], keywords: common },
  py: { lineComment: "#", quotes: ['"', "'"], keywords: common },
  rs: { lineComment: "//", block: ["/*", "*/"], quotes: ['"', "'"], keywords: common },
  plain: { quotes: ['"', "'"], keywords: common }
};
var extLang = {
  go: "go",
  ts: "ts",
  tsx: "ts",
  js: "js",
  jsx: "js",
  mjs: "js",
  cjs: "js",
  json: "json",
  css: "css",
  html: "html",
  htm: "html",
  sh: "sh",
  bash: "sh",
  zsh: "sh",
  py: "py",
  rs: "rs"
};
function languageFor(path) {
  const ext = path.toLowerCase().split(".").pop() ?? "";
  return extLang[ext] ?? "plain";
}
function tokenize(text, lang) {
  if (text.length > 500) return [{ text, kind: "plain" }];
  const cfg = langs[lang] ?? langs.plain ?? { quotes: ['"', "'"], keywords: /* @__PURE__ */ new Set() };
  const out = [];
  let plain = "";
  let i = 0;
  const flush = () => {
    if (plain) {
      out.push({ text: plain, kind: "plain" });
      plain = "";
    }
  };
  const push = (kind, s) => {
    flush();
    out.push({ text: s, kind });
  };
  while (i < text.length) {
    const rest = text.slice(i);
    if (cfg.lineComment && rest.startsWith(cfg.lineComment)) {
      push("comment", rest);
      break;
    }
    if (cfg.block && rest.startsWith(cfg.block[0])) {
      const end = text.indexOf(cfg.block[1], i + cfg.block[0].length);
      const stop = end < 0 ? text.length : end + cfg.block[1].length;
      push("comment", text.slice(i, stop));
      i = stop;
      continue;
    }
    const ch = text[i] ?? "";
    if (cfg.quotes.includes(ch)) {
      let j = i + 1;
      while (j < text.length) {
        if (text[j] === "\\") {
          j += 2;
          continue;
        }
        if (text[j] === ch) {
          j++;
          break;
        }
        j++;
      }
      push("string", text.slice(i, j));
      i = j;
      continue;
    }
    if (/[0-9]/.test(ch)) {
      let j = i + 1;
      while (j < text.length && /[0-9a-fA-FxX._]/.test(text[j] ?? "")) j++;
      push("number", text.slice(i, j));
      i = j;
      continue;
    }
    if (/[A-Za-z_$]/.test(ch)) {
      let j = i + 1;
      while (j < text.length && /[A-Za-z0-9_$]/.test(text[j] ?? "")) j++;
      const word = text.slice(i, j);
      if (cfg.keywords.has(word)) push("keyword", word);
      else plain += word;
      i = j;
      continue;
    }
    plain += ch;
    i++;
  }
  flush();
  return out;
}

// src/diff/main.ts
var ROW_H = 18;
var OVERSCAN = 40;
var startedAt = performance.now();
var params = new URLSearchParams(location.search);
var tabID = params.get("tab") ?? "";
var storageKey = "diff-mode:" + tabID;
var app = document.getElementById("app");
if (!app) throw new Error("missing #app");
var doc = null;
var mode = sessionStorage.getItem(storageKey) === "unified" ? "unified" : "side";
var folded = /* @__PURE__ */ new Set();
var rows = [];
var mountedStart = -1;
var mountedEnd = -1;
var raf = 0;
var focusDone = false;
var header = document.createElement("header");
header.className = "diff-head";
var notices = document.createElement("div");
notices.className = "notices";
var scroller = document.createElement("div");
scroller.className = "diff-scroll";
var body = document.createElement("div");
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
async function load() {
  if (!tabID) {
    fatal("\u7F3A\u5C11\u6807\u7B7E\u9875\u53C2\u6570\uFF0C\u65E0\u6CD5\u52A0\u8F7D\u53D8\u66F4\u3002");
    return;
  }
  const top = scroller.scrollTop;
  setNotices(["\u52A0\u8F7D\u4E2D\u2026"]);
  try {
    doc = await Diff.load(tabID);
  } catch (err) {
    fatal("\u8BFB\u53D6\u53D8\u66F4\u5931\u8D25\uFF1A" + String(err));
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
function markReady() {
  if (!doc) return;
  document.documentElement.dataset.diffState = doc.state;
  document.documentElement.dataset.diffFiles = String(doc.files.length);
  document.documentElement.dataset.diffLines = String(doc.stats.lines);
  document.documentElement.dataset.diffMs = String(Math.round(performance.now() - startedAt));
}
function drawHeader() {
  header.replaceChildren();
  if (!doc) return;
  const main = document.createElement("div");
  main.className = "head-main";
  const name = document.createElement("strong");
  name.textContent = doc.workspace || "\u5DE5\u4F5C\u533A";
  main.append(name);
  if (doc.revision) {
    const rev = document.createElement("span");
    rev.className = "muted mono";
    rev.textContent = doc.revision;
    main.append(rev);
  }
  const stats = document.createElement("span");
  stats.className = "stats";
  stats.textContent = `${doc.stats.files} \u4E2A\u6587\u4EF6 +${doc.stats.additions} \u2212${doc.stats.deletions}`;
  main.append(stats);
  const actions = document.createElement("div");
  actions.className = "head-actions";
  const seg = document.createElement("div");
  seg.className = "segmented";
  for (const [value, label] of [["side", "\u5E76\u6392"], ["unified", "\u7EDF\u4E00"]]) {
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
  refresh.textContent = "\u5237\u65B0";
  refresh.addEventListener("click", () => void load());
  actions.append(seg, refresh);
  header.append(main, actions);
}
function drawNotices() {
  const list = [];
  if (!doc) {
    setNotices(list);
    return;
  }
  if (doc.state === "clean") list.push("\u5DE5\u4F5C\u533A\u6CA1\u6709\u672A\u6682\u5B58\u4FEE\u6539\u3002");
  else if (doc.state === "notRepo") list.push(doc.message || "\u5F53\u524D\u76EE\u5F55\u4E0D\u662F git \u4ED3\u5E93\u3002");
  else if (doc.state === "noGit") list.push(doc.message || "\u627E\u4E0D\u5230 git\u3002");
  else if (doc.state === "timeout") list.push(doc.message || "\u8BFB\u53D6\u8D85\u65F6\uFF0C\u8BF7\u91CD\u8BD5\u3002");
  else if (doc.state === "error") list.push(doc.message || "git \u8BFB\u53D6\u5931\u8D25\u3002");
  if (doc.truncated) list.push("\u5DF2\u622A\u65AD\uFF1A" + (doc.reason || "\u5185\u5BB9\u8D85\u8FC7\u9650\u5236") + "\u3002");
  if (doc.hidden) list.push(`\u5DF2\u9690\u85CF ${doc.hidden} \u4E2A\u654F\u611F\u6587\u4EF6\u3002`);
  if (doc.skipped) list.push(`${doc.skipped} \u4E2A\u6587\u4EF6\u65E0\u6CD5\u89E3\u6790\uFF0C\u5DF2\u8DF3\u8FC7\u3002`);
  if (doc.state === "ok") list.push("\u53EA\u663E\u793A\u5DF2\u8DDF\u8E2A\u6587\u4EF6\u7684\u672A\u6682\u5B58\u4FEE\u6539\u3002");
  if (doc.state === "ok" && doc.files.length === 0) list.push("\u6CA1\u6709\u53EF\u663E\u793A\u7684\u53D8\u66F4\u3002");
  setNotices(list);
}
function setNotices(list) {
  notices.replaceChildren();
  for (const text of list) {
    const line = document.createElement("div");
    line.className = "notice";
    line.textContent = text;
    notices.append(line);
  }
  notices.hidden = list.length === 0;
}
function fatal(message) {
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
function rebuildRows(keepScroll) {
  if (!doc || doc.state !== "ok") return;
  const top = scroller.scrollTop;
  rows = buildRows(doc, mode, folded);
  body.style.height = `${rows.length * ROW_H}px`;
  body.style.width = `${bodyWidth()}px`;
  mountedStart = -1;
  if (keepScroll) scroller.scrollTop = top;
  drawWindow();
}
function bodyWidth() {
  const chars = maxLineChars(rows);
  const code2 = Math.max(320, chars * 8 + 40);
  const total = mode === "side" ? code2 * 2 + 120 : code2 + 120;
  return Math.max(scroller.clientWidth, total);
}
function schedule() {
  if (raf) return;
  raf = requestAnimationFrame(() => {
    raf = 0;
    drawWindow();
  });
}
function drawWindow() {
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
function rowElement(row, index) {
  const el = document.createElement("div");
  el.className = "row " + row.kind;
  el.style.top = `${index * ROW_H}px`;
  if (row.kind === "file" && row.file) fileRow(el, row.file);
  else if (row.kind === "hunk" && row.hunk && row.file && row.hunkIndex !== void 0) {
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
function fileRow(el, file) {
  const fold = document.createElement("button");
  fold.type = "button";
  fold.className = "fold";
  fold.textContent = folded.has(fileKey(file)) ? "\u25B8" : "\u25BE";
  fold.title = "\u6298\u53E0\u6587\u4EF6";
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
    bin.textContent = "\u4E8C\u8FDB\u5236\u6587\u4EF6\uFF0C\u4E0D\u663E\u793A\u5185\u5BB9";
    el.append(bin);
  } else {
    const delta = fileDelta(file);
    const stats = document.createElement("span");
    stats.className = "delta";
    stats.textContent = `+${delta.additions} \u2212${delta.deletions}`;
    el.append(stats);
  }
  const copy = document.createElement("button");
  copy.type = "button";
  copy.className = "copy";
  copy.textContent = "\u590D\u5236\u8DEF\u5F84";
  copy.addEventListener("click", (ev) => {
    ev.stopPropagation();
    void Diff.copyPath(tabID, file.path).catch((err) => {
      showInlineError("\u590D\u5236\u5931\u8D25\uFF1A" + String(err));
    });
  });
  el.append(copy);
  el.addEventListener("click", (ev) => {
    if (ev.target === fold || ev.target === copy) return;
    toggleFold(fileKey(file));
  });
}
function hunkRow(el, file, index) {
  const key = hunkKey(file, index);
  const fold = document.createElement("button");
  fold.type = "button";
  fold.className = "fold";
  fold.textContent = folded.has(key) ? "\u25B8" : "\u25BE";
  fold.title = "\u6298\u53E0\u7247\u6BB5";
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
function rowHeader(file, index) {
  return file.hunks[index]?.header ?? "@@";
}
function lineRow(el, row) {
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
function num(n) {
  const el = document.createElement("span");
  el.className = "ln";
  el.textContent = n && n > 0 ? String(n) : "";
  return el;
}
function sign(kind) {
  const el = document.createElement("span");
  el.className = "sign";
  el.textContent = kind === "add" ? "+" : kind === "del" ? "\u2212" : " ";
  return el;
}
function code(text, lang) {
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
function toggleFold(key) {
  if (folded.has(key)) folded.delete(key);
  else folded.add(key);
  rebuildRows(true);
  mountedStart = -1;
  drawWindow();
  drawHeader();
}
function showInlineError(message) {
  const line = document.createElement("div");
  line.className = "notice error";
  line.textContent = message;
  notices.append(line);
  notices.hidden = false;
}
