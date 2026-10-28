import "./runtime.js";
// Stand-in for @earendil-works/pi-tui: enough for headless tool registration.
// Rendering is never exercised by the spike, so components are inert.

export class Text {
  constructor(text) {
    this.text = text;
  }
}

export class Markdown {
  constructor(text) {
    this.text = text;
  }
}

export class Spacer {
  constructor(lines) {
    this.lines = lines;
  }
}

export class Container {
  constructor() {
    this.children = [];
  }
  addChild(child) {
    this.children.push(child);
    return this;
  }
}

export function truncateToWidth(text) {
  return String(text);
}

export function matchesKey(data, key) {
  return String(data) === String(key);
}

export function visibleWidth(text) {
  return String(text).length;
}

