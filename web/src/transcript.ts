import type { Message } from "./mygo";

// TranscriptView keeps the full transcript in memory but mounts only the rows
// near the viewport. The page owns no business state; it renders what Go sends
// and keeps scroll position stable while heights change during streaming.

const OVERSCAN = 800;
const MESSAGE_ESTIMATE = 78;
const TOOL_ESTIMATE = 160;

type ToolState = "running" | "done" | "failed";

interface MessageItem {
  kind: "message";
  key: string;
  id: string;
  role: string;
  text: string;
  thinking: string;
  error: string;
}

interface ToolItem {
  kind: "tool";
  key: string;
  id: string;
  name: string;
  args: unknown;
  result: string;
  state: ToolState;
  durationMs?: number;
}

type Item = MessageItem | ToolItem;

interface MountedRow {
  el: HTMLDivElement;
  body?: HTMLElement;
  thinking?: HTMLElement;
  result?: HTMLElement;
  state?: HTMLElement;
}

export interface TranscriptStats {
  rows: number;
  mounted: number;
  totalHeight: number;
}

function messageKey(id: string): string {
  return `m:${id}`;
}

function toolKey(id: string): string {
  return `t:${id}`;
}

function element<K extends keyof HTMLElementTagNameMap>(tag: K, className?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  return node;
}

export class TranscriptView {
  private scroller: HTMLElement;
  private topSpacer: HTMLDivElement;
  private list: HTMLDivElement;
  private bottomSpacer: HTMLDivElement;

  private items: Item[] = [];
  private positions = new Map<string, number>();
  private heights: number[] = [];
  private mounted = new Map<string, MountedRow>();
  private dirty = new Set<string>();

  private start = 0;
  private end = 0;
  private follow = true;
  private frame = 0;
  private onFollowChange?: (follow: boolean) => void;

  constructor(scroller: HTMLElement, onFollowChange?: (follow: boolean) => void) {
    this.scroller = scroller;
    this.onFollowChange = onFollowChange;
    this.topSpacer = element("div", "vspace");
    this.list = element("div", "vlist");
    this.bottomSpacer = element("div", "vspace");
    scroller.replaceChildren(this.topSpacer, this.list, this.bottomSpacer);
    scroller.addEventListener("scroll", () => this.handleScroll(), { passive: true });
    new ResizeObserver(() => this.schedule()).observe(scroller);
  }

  setMessages(messages: Message[]): void {
    this.items = [];
    this.positions.clear();
    this.heights = [];
    this.mounted.clear();
    this.dirty.clear();
    this.list.replaceChildren();
    this.start = this.end = 0;

    for (const message of messages) {
      if (message.role === "system") continue;
      if (message.role === "toolResult") {
        this.attachToolResult(message.text, Boolean(message.error));
        continue;
      }
      this.appendItem(this.messageItem(message));
      for (const call of message.toolCalls ?? []) {
        this.appendItem({
          kind: "tool",
          key: toolKey(call.id),
          id: call.id,
          name: call.name,
          args: call.arguments,
          result: "",
          state: "done",
        });
      }
    }
    this.follow = true;
    this.notifyFollow();
    this.flush();
  }

  startMessage(id: string, role: string, text = ""): void {
    if (role === "system" || role === "toolResult") return;
    const existing = this.find(messageKey(id));
    if (existing) {
      if (text) this.setMessageText(existing as MessageItem, text);
      return;
    }
    this.appendItem({
      kind: "message",
      key: messageKey(id),
      id,
      role,
      text,
      thinking: "",
      error: "",
    });
    this.schedule();
  }

  appendDelta(id: string, block: "text" | "thinking", delta: string): void {
    if (!delta) return;
    let item = this.find(messageKey(id)) as MessageItem | undefined;
    if (!item) {
      this.startMessage(id, "assistant");
      item = this.find(messageKey(id)) as MessageItem | undefined;
    }
    if (!item) return;
    if (block === "thinking") item.thinking += delta;
    else item.text += delta;
    this.markDirty(item);
    this.schedule();
  }

  endMessage(id: string, role: string, text: string, thinking: string, error: string): void {
    if (role === "system" || role === "toolResult") return;
    let item = this.find(messageKey(id)) as MessageItem | undefined;
    if (!item) {
      this.startMessage(id, role, text);
      item = this.find(messageKey(id)) as MessageItem | undefined;
      if (!item) return;
    }
    item.text = text;
    item.thinking = thinking;
    item.error = error;
    this.markDirty(item);
    this.schedule();
  }

  startTool(id: string, name: string, args: unknown): void {
    const existing = this.find(toolKey(id)) as ToolItem | undefined;
    if (existing) {
      existing.name = name;
      existing.args = args;
      existing.state = "running";
      this.markDirty(existing);
      this.schedule();
      return;
    }
    this.appendItem({
      kind: "tool",
      key: toolKey(id),
      id,
      name,
      args,
      result: "",
      state: "running",
    });
    this.schedule();
  }

  appendTool(id: string, text: string): void {
    const item = this.find(toolKey(id)) as ToolItem | undefined;
    if (!item || !text) return;
    item.result += text;
    this.markDirty(item);
    this.schedule();
  }

  endTool(id: string, text: string, isError: boolean, durationMs?: number): void {
    const item = this.find(toolKey(id)) as ToolItem | undefined;
    if (!item) return;
    item.result = text;
    item.state = isError ? "failed" : "done";
    item.durationMs = durationMs;
    this.markDirty(item);
    this.schedule();
  }

  scrollToBottom(): void {
    this.follow = true;
    this.notifyFollow();
    this.scroller.scrollTop = this.scroller.scrollHeight;
    this.schedule();
  }

  scrollToTop(): void {
    this.follow = false;
    this.notifyFollow();
    this.scroller.scrollTop = 0;
    this.schedule();
  }

  lastAssistantText(): string {
    for (let i = this.items.length - 1; i >= 0; i--) {
      const item = this.items[i];
      if (!item) continue;
      if (item.kind === "message" && item.role === "assistant" && item.text) return item.text;
    }
    return "";
  }

  stats(): TranscriptStats {
    return { rows: this.items.length, mounted: this.mounted.size, totalHeight: this.totalHeight() };
  }

  private attachToolResult(text: string, failed: boolean): void {
    for (const item of this.items) {
      if (item.kind !== "tool" || item.result !== "") continue;
      item.result = text;
      item.state = failed ? "failed" : "done";
      this.markDirty(item);
      return;
    }
  }

  private messageItem(message: Message): MessageItem {
    return {
      kind: "message",
      key: messageKey(message.id),
      id: message.id,
      role: message.role,
      text: message.text,
      thinking: message.thinking ?? "",
      error: message.error ?? "",
    };
  }

  private appendItem(item: Item): void {
    this.positions.set(item.key, this.items.length);
    this.items.push(item);
    this.heights.push(item.kind === "tool" ? TOOL_ESTIMATE : MESSAGE_ESTIMATE);
  }

  private find(key: string): Item | undefined {
    const index = this.positions.get(key);
    return index === undefined ? undefined : this.items[index];
  }

  private setMessageText(item: MessageItem, text: string): void {
    item.text = text;
    this.markDirty(item);
    this.schedule();
  }

  private markDirty(item: Item): void {
    this.dirty.add(item.key);
  }

  private handleScroll(): void {
    const nearBottom = this.distanceFromBottom() < 80;
    if (nearBottom !== this.follow) {
      this.follow = nearBottom;
      this.notifyFollow();
    }
    this.schedule();
  }

  private notifyFollow(): void {
    this.onFollowChange?.(this.follow);
  }

  private distanceFromBottom(): number {
    return this.scroller.scrollHeight - this.scroller.scrollTop - this.scroller.clientHeight;
  }

  private schedule(): void {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.flush();
    });
  }

  private flush(): void {
    const total = this.totalHeight();
    if (this.items.length === 0) {
      this.list.replaceChildren();
      this.mounted.clear();
      this.start = this.end = 0;
      this.topSpacer.style.height = "0px";
      this.bottomSpacer.style.height = "0px";
      return;
    }

    const viewport = this.scroller.clientHeight || 600;
    const anchor = this.follow ? Math.max(0, total - viewport) : this.scroller.scrollTop;
    const start = this.indexAt(anchor - OVERSCAN);
    const end = Math.min(this.items.length, this.indexAt(anchor + viewport + OVERSCAN) + 1);
    if (start !== this.start || end !== this.end) {
      this.renderWindow(start, end);
    } else {
      this.refreshMounted();
      this.measure();
    }
    this.updateSpacers();
    if (this.follow) this.stickToBottom();
  }

  private stickToBottom(): void {
    const target = this.scroller.scrollHeight - this.scroller.clientHeight;
    if (target < 0) return;
    if (Math.abs(this.scroller.scrollTop - target) > 1) this.scroller.scrollTop = target;
  }

  private renderWindow(start: number, end: number): void {
    const beforeTop = this.topSpacer.offsetHeight;
    const keep = this.scroller.scrollTop;
    for (const [key, row] of this.mounted) {
      const index = this.positions.get(key);
      if (index === undefined || index < start || index >= end) {
        row.el.remove();
        this.mounted.delete(key);
      }
    }
    for (let i = start; i < end; i++) {
      const item = this.items[i];
      if (!item) continue;
      let row = this.mounted.get(item.key);
      if (!row) {
        row = this.createRow(item);
        this.mounted.set(item.key, row);
      }
      this.updateRow(row, item);
      this.list.append(row.el);
    }
    this.start = start;
    this.end = end;
    this.dirty.clear();
    this.measure();
    this.updateSpacers();
    const shift = this.topSpacer.offsetHeight - beforeTop;
    if (!this.follow && shift !== 0) this.scroller.scrollTop = keep + shift;
  }

  private refreshMounted(): void {
    if (this.dirty.size === 0) return;
    for (const key of this.dirty) {
      const index = this.positions.get(key);
      const row = this.mounted.get(key);
      const item = index === undefined ? undefined : this.items[index];
      if (!row || !item) continue;
      this.updateRow(row, item);
    }
    this.dirty.clear();
  }

  private measure(): void {
    for (let i = this.start; i < this.end; i++) {
      const item = this.items[i];
      if (!item) continue;
      const row = this.mounted.get(item.key);
      if (!row) continue;
      const height = row.el.offsetHeight;
      if (height > 0 && Math.abs((this.heights[i] ?? 0) - height) > 0.5) this.heights[i] = height;
    }
  }

  private updateSpacers(): void {
    let top = 0;
    for (let i = 0; i < this.start; i++) top += this.heights[i] ?? 0;
    let rest = 0;
    for (let i = this.end; i < this.items.length; i++) rest += this.heights[i] ?? 0;
    this.topSpacer.style.height = `${Math.round(top)}px`;
    this.bottomSpacer.style.height = `${Math.round(rest)}px`;
  }

  private totalHeight(): number {
    let total = 0;
    for (const height of this.heights) total += height;
    return total;
  }

  private indexAt(y: number): number {
    if (this.items.length === 0) return 0;
    let acc = 0;
    for (let i = 0; i < this.items.length; i++) {
      acc += this.heights[i] ?? 0;
      if (y < acc) return i;
    }
    return this.items.length - 1;
  }

  private createRow(item: Item): MountedRow {
    const el = element("div", "trow");
    if (item.kind === "tool") {
      const details = element("details", "tool");
      details.open = true;
      const summary = element("summary");
      const name = element("span", "tool-name");
      const state = element("span", "tool-state");
      summary.append(name, state);
      const args = element("pre", "args");
      const result = element("pre", "result");
      details.append(summary, args, result);
      el.append(details);
      return { el, result, state };
    }
    const root = element("div", `msg ${item.role}`);
    const role = element("div", "role");
    role.textContent = item.role === "toolResult" ? "tool result" : item.role;
    const body = element("div", "body");
    const thinking = element("div", "thinking");
    root.append(role, body);
    if (item.role === "assistant") root.append(thinking);
    el.append(root);
    return { el, body, thinking };
  }

  private updateRow(row: MountedRow, item: Item): void {
    if (item.kind === "tool") {
      const name = row.el.querySelector(".tool-name");
      if (name) name.textContent = item.name;
      if (row.state) {
        const suffix = item.durationMs === undefined ? "" : ` (${item.durationMs} ms)`;
        row.state.textContent = item.state === "running" ? "运行中…" : item.state === "failed" ? `失败${suffix}` : `完成${suffix}`;
      }
      const args = row.el.querySelector(".args");
      if (args) args.textContent = JSON.stringify(item.args ?? {}, null, 2);
      if (row.result) row.result.textContent = item.result;
      row.el.classList.toggle("failed", item.state === "failed");
      return;
    }
    if (row.body) {
      row.body.textContent = item.error ? `${item.text}\n${item.error}`.trim() : item.text;
    }
    if (row.thinking) row.thinking.textContent = item.thinking;
    row.el.classList.toggle("error", Boolean(item.error));
  }
}
