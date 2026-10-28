// A small, dependency-free tokenizer for the diff's language hinting. It is
// deliberately not a parser: strings, comments, numbers and keywords are
// enough to make a diff readable, and unknown languages stay plain text.
export type TokenKind = "plain" | "comment" | "string" | "number" | "keyword";

export interface Token {
  text: string;
  kind: TokenKind;
}

interface Lang {
  lineComment?: string;
  block?: [string, string];
  quotes: string[];
  keywords: Set<string>;
}

const common = new Set([
  "if", "else", "for", "while", "return", "break", "continue", "switch", "case",
  "default", "class", "interface", "type", "func", "function", "var", "let",
  "const", "new", "delete", "try", "catch", "finally", "throw", "import",
  "export", "from", "as", "package", "struct", "map", "chan", "go", "defer",
  "select", "range", "true", "false", "nil", "null", "undefined", "this",
  "self", "async", "await", "yield", "public", "private", "protected", "static",
  "final", "void", "int", "string", "bool", "byte", "float", "double", "long",
  "pub", "use", "mod", "impl", "fn", "match", "enum", "trait", "where", "loop",
  "and", "or", "not", "in", "is", "with", "lambda", "pass", "raise", "def",
  "elif", "except", "None", "True", "False", "echo", "then", "fi", "do", "done",
]);

const langs: Record<string, Lang> = {
  go: { lineComment: "//", block: ["/*", "*/"], quotes: ['"', "'", "`"], keywords: common },
  ts: { lineComment: "//", block: ["/*", "*/"], quotes: ['"', "'", "`"], keywords: common },
  js: { lineComment: "//", block: ["/*", "*/"], quotes: ['"', "'", "`"], keywords: common },
  json: { lineComment: undefined, quotes: ['"'], keywords: new Set(["true", "false", "null"]) },
  css: { block: ["/*", "*/"], quotes: ['"', "'"], keywords: new Set(["import", "media", "supports", "var"]) },
  html: { block: ["<!--", "-->"], quotes: ['"', "'"], keywords: new Set(["html", "head", "body", "div", "span", "script", "style"]) },
  sh: { lineComment: "#", quotes: ['"', "'"], keywords: common },
  py: { lineComment: "#", quotes: ['"', "'"], keywords: common },
  rs: { lineComment: "//", block: ["/*", "*/"], quotes: ['"', "'"], keywords: common },
  plain: { quotes: ['"', "'"], keywords: common },
};

const extLang: Record<string, string> = {
  go: "go", ts: "ts", tsx: "ts", js: "js", jsx: "js", mjs: "js", cjs: "js",
  json: "json", css: "css", html: "html", htm: "html", sh: "sh", bash: "sh",
  zsh: "sh", py: "py", rs: "rs",
};

// languageFor picks a tokenizer from a file path; empty means plain.
export function languageFor(path: string): string {
  const ext = path.toLowerCase().split(".").pop() ?? "";
  return extLang[ext] ?? "plain";
}

// tokenize splits one line. Lines longer than 500 characters stay plain to
// bound the page's work.
export function tokenize(text: string, lang: string): Token[] {
  if (text.length > 500) return [{ text, kind: "plain" }];
  const cfg = langs[lang] ?? langs.plain ?? { quotes: ['"', "'"], keywords: new Set<string>() };
  const out: Token[] = [];
  let plain = "";
  let i = 0;

  const flush = (): void => {
    if (plain) {
      out.push({ text: plain, kind: "plain" });
      plain = "";
    }
  };
  const push = (kind: TokenKind, s: string): void => {
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
