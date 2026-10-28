// Minimal node:path stand-in for the neutral-platform bundle. Path work is
// pure string handling, so it does not need host capabilities.

export const sep = "/";

export function join(...parts) {
  return parts
    .filter((p) => p !== "")
    .join("/")
    .replace(/\/+/g, "/");
}

export function resolve(...parts) {
  return join(...parts);
}

export function dirname(p) {
  const s = String(p);
  const i = s.lastIndexOf("/");
  return i <= 0 ? "/" : s.slice(0, i);
}

export function basename(p, ext) {
  const s = String(p);
  const base = s.slice(s.lastIndexOf("/") + 1);
  return ext && base.endsWith(ext) ? base.slice(0, -ext.length) : base;
}

export function extname(p) {
  const base = basename(p);
  const i = base.lastIndexOf(".");
  return i <= 0 ? "" : base.slice(i);
}

export function isAbsolute(p) {
  return String(p).startsWith("/");
}
