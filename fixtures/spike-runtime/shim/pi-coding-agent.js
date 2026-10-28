import "./runtime.js";
// Stand-in for @earendil-works/pi-coding-agent during the runtime spike.
// Values are re-exports the shim can answer without pi's own runtime.

export const CONFIG_DIR_NAME = ".pi";

export function getAgentDir() {
  return globalThis.__locowrkAgentDir || ".pi/agent";
}

export function getMarkdownTheme() {
  return { fg: (_c, s) => s, bold: (s) => s, italic: (s) => s };
}

export function withFileMutationQueue(_path, fn) {
  return fn();
}

export function formatReadLineRange(offset, limit) {
  if (!offset && !limit) return "";
  return `:${offset || 1}${limit ? `-${offset + limit - 1}` : ""}`;
}

export function parseFrontmatter(text) {
  return { frontmatter: {}, body: String(text) };
}

export class CustomEditor {
  constructor() {}
  handleInput() {}
  render() {
    return [];
  }
}

export class CustomMessage {
  constructor(entry) {
    this.entry = entry;
  }
}

