// Stand-in for @earendil-works/pi-tui. Extensions import components and text
// helpers at module load even when the desktop never renders them, so the
// values must exist and stay inert. Rendering belongs to the host (web page),
// not to extension code.

export class Text {
  constructor(text = "", x = 0, y = 0) {
    this.text = String(text);
    this.x = x;
    this.y = y;
  }
  render() {
    return [this.text];
  }
  invalidate() {}
}

export class Markdown {
  constructor(text = "") {
    this.text = String(text);
  }
  render() {
    return [this.text];
  }
  invalidate() {}
}

export class Spacer {
  constructor(lines = 1) {
    this.lines = lines;
  }
  render() {
    return [];
  }
  invalidate() {}
}

export class Container {
  constructor() {
    this.children = [];
  }
  addChild(child) {
    this.children.push(child);
    return this;
  }
  render() {
    return [];
  }
  invalidate() {}
}

export function truncateToWidth(text, width) {
  const value = String(text);
  if (!Number.isFinite(width) || width <= 0 || value.length <= width) return value;
  return value.slice(0, width);
}

export function matchesKey(data, key) {
  return String(data) === String(key);
}

export function visibleWidth(text) {
  return String(text).length;
}

export const Key = {
  escape: "escape",
  ctrlC: "ctrl+c",
  enter: "enter",
};
