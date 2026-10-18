// specs/08 批 2 的两个产物页签：文件页签的三种落点（markdown / 代码行号 / 读不到）
// 与图片页签的两条数据面（资产 id / 只有路径）。
//
// 命令名与参数键照生产抄（`desktop/src/lib/api/files.ts` 的 `file_read_text`、
// `assets.ts` 的 `asset_read`）——tauri-stub 的约定是**没有预置就 reject**，
// 所以「读不到」那条用例不需要额外编错误。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import FileTab from "../src/components/shell/FileTab.vue";
import ImageTab from "../src/components/shell/ImageTab.vue";
import ToolRow from "../src/components/chat/ToolRow.vue";
import { i18n } from "../src/i18n";
import { resetRowFolds } from "../src/lib/chat/fold-state";
import { useWorkPanelStore } from "../src/stores/work-panel";
import type { ToolItem } from "../src/stores/chat";
import { onCommand, resetIpc } from "./tauri-stub";

const fileDto = (over: Record<string, unknown> = {}) => ({
  path: "/w/a.ts",
  bytes: 24,
  text: "line one\nline two\n",
  truncated: false,
  binary: false,
  ...over,
});

beforeEach(() => {
  setActivePinia(createPinia());
  resetIpc();
});

describe("FileTab", () => {
  it("renders markdown through the existing renderer", async () => {
    onCommand("file_read_text", fileDto({ path: "/w/notes.md", text: "# 标题\n\n正文" }));
    const w = mount(FileTab, {
      props: { path: "/w/notes.md" },
      global: { plugins: [i18n] },
    });
    await flushPromises();
    expect(w.find(".md").exists()).toBe(true);
    expect(w.text()).toContain("标题");
    expect(w.find(".ftab__line").exists()).toBe(false);
  });

  it("numbers lines and marks the target range", async () => {
    onCommand("file_read_text", fileDto({ text: "a\nb\nc\nd" }));
    const w = mount(FileTab, {
      props: { path: "/w/a.ts", line: 3, endLine: 3 },
      global: { plugins: [i18n] },
    });
    await flushPromises();
    const lines = w.findAll(".ftab__line");
    expect(lines).toHaveLength(4);
    expect(lines[2]?.classes()).toContain("ftab__line--target");
    expect(lines[0]?.find(".ftab__no").text()).toBe("1");
  });

  it("shows a note instead of blank when the read fails", async () => {
    const w = mount(FileTab, { props: { path: "/w/gone.ts" }, global: { plugins: [i18n] } });
    await flushPromises();
    expect(w.text()).toContain("unstubbed command");
    expect(w.find(".ftab__code").exists()).toBe(false);
  });
});

describe("ImageTab", () => {
  it("loads an asset by id and offers fullscreen", async () => {
    onCommand("asset_read", "data:image/png;base64,AAAA");
    const w = mount(ImageTab, {
      props: { path: "/w/shot.png", assetId: "a1", assetMime: "image/png" },
      global: { plugins: [i18n] },
    });
    await flushPromises();
    expect(w.find("img.itab__img").attributes("src")).toBe("data:image/png;base64,AAAA");
    expect(w.find(".itab__full").exists()).toBe(true);
  });

  it("falls back to the lwpreview protocol without an asset id", async () => {
    const w = mount(ImageTab, { props: { path: "/w/shot.png" }, global: { plugins: [i18n] } });
    await flushPromises();
    expect(w.find("img.itab__img").attributes("src")).toContain("lwpreview.localhost");
    expect(w.find(".itab__full").exists()).toBe(false);
  });
});

describe("ToolRow 的产物入口", () => {
  const toolItem = (over: Partial<ToolItem> = {}): ToolItem => ({
    id: 7,
    kind: "tool",
    callId: "call-7",
    tool: "fs_read",
    args: { path: "/w/a.ts" },
    status: "settled",
    ok: true,
    sideEffect: "read",
    pluginVersion: "0.1.0",
    output: { path: "/w/a.ts", bytes: 2 },
    ts: 1,
    doneTs: 2,
    ...over,
  });

  const mountRow = (item: ToolItem) => {
    const pinia = createPinia();
    setActivePinia(pinia);
    return {
      wp: useWorkPanelStore(),
      w: mount(ToolRow, { props: { item }, global: { plugins: [pinia, i18n] } }),
    };
  };

  beforeEach(() => {
    resetRowFolds();
  });

  it("routes an .html file name to the browser tab", async () => {
    const { wp, w } = mountRow(toolItem({ args: { path: "/w/page.html" } }));
    await w.find(".tool__file").trigger("click");
    expect(wp.tabs).toHaveLength(1);
    expect(wp.tabs[0]?.kind).toBe("browser");
    expect(wp.tabs[0]?.target).toBe("/w/page.html");
  });

  it("整行开子代理转录，三角仍管展开", async () => {
    const { w } = mountRow(
      toolItem({
        tool: "subagent_run",
        args: { prompt: "查一下" },
        output: { child_session_id: "c1" },
      }),
    );
    await w.find(".prow").trigger("click");
    expect(w.emitted("openTranscript")?.[0]).toEqual(["c1", "查一下"]);
    expect(w.find(".tool__sub").exists()).toBe(false);
    await w.find(".prow__chev").trigger("click");
    expect(w.find(".tool__sub").exists()).toBe(true);
  });
});
