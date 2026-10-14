// Q12 主体第一条：**工具行相位与轮头容器**的行语义（`src/stores/chat.ts` 的 onEvent 开关）。
//
// 为什么必须新写而不是改 Q11 那条：Q11 的 chat-row-semantics 断言的是 append-or-create
// 与「重放不带 ts」，它**碰不到** `pending` 这个字段——所以把 `pending:boolean` 换成
// `status` 三相位之后，那 19 条仍然全绿（本仓第 2 条纪律：既有测试对某类改动零证明力
// 时要说破，不许拿绿灯当证据）。这一条补的正是那个空洞。
//
// 事件形状一律照 `desktop/src-tauri/src/events.rs` 的 serde 输出写（snake_case +
// `kind` 标签），不凭想象编字段。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import { useChatStore, type ToolItem } from "../src/stores/chat";
import ToolRow from "../src/components/chat/ToolRow.vue";
import { i18n } from "../src/i18n";
import type { AgentEvent, ToolRowStatus } from "../src/types";

beforeEach(() => {
  setActivePinia(createPinia());
});

const toolCall = (callId: string): AgentEvent =>
  ({
    kind: "tool_call",
    session_id: "s1",
    call_id: callId,
    tool: "edit_replace",
    args: { path: "F:/w/a.rs" },
    side_effect: "compensable",
    plugin_version: "0.1.0",
  }) as unknown as AgentEvent;

const toolStatus = (callId: string, status = "pending_approval"): AgentEvent =>
  ({
    kind: "tool_status",
    session_id: "s1",
    call_id: callId,
    status,
  }) as unknown as AgentEvent;

const toolResult = (callId: string, ok = true): AgentEvent =>
  ({
    kind: "tool_result",
    session_id: "s1",
    call_id: callId,
    ok,
    output: { path: "F:/w/a.rs", additions: 2, deletions: 1 },
    error: null,
  }) as unknown as AgentEvent;

const userMessage = (content = "改一下"): AgentEvent =>
  ({
    kind: "user_message",
    session_id: "s1",
    content,
    pos: 0,
    parts: [],
  }) as unknown as AgentEvent;

const fileChanges = (
  files: Array<Record<string, unknown>>,
  turnId = 7,
): AgentEvent =>
  ({
    kind: "file_changes",
    session_id: "s1",
    turn_id: turnId,
    files,
  }) as unknown as AgentEvent;

function rows(chat: ReturnType<typeof useChatStore>) {
  return chat.items as Array<Record<string, unknown>>;
}

describe("工具行相位（pending:boolean 已退役为 status 三相位）", () => {
  it("tool_call 建行即 running，且**不再带 pending 字段**（防回潮：两份真相必漂）", () => {
    const chat = useChatStore();
    chat.onEvent(toolCall("c1"));
    const row = rows(chat)[0];
    expect(row!.status).toBe("running");
    expect("pending" in row!).toBe(false);
    expect(row!.ok).toBe(null); // 成败仍由结果给，不复制进 status
  });

  it("审批请求点亮等待态；结果已回后乱序到达不许把行退回等待", () => {
    const chat = useChatStore();
    chat.onEvent(toolCall("c1"));
    chat.onEvent(toolStatus("c1"));
    expect(rows(chat)[0]!.status).toBe("awaiting_approval");

    // 乱序：结果先到、等待态后到（后端 emit 顺序无保证，前端不许把已闭合的行改回等待）。
    // ⚠ 必须按 callId 定位——同一个 pinia 实例里 useChatStore() 返回的是**同一个** store，
    // 第二段的行排在 c1 之后，取 [0] 会读到上一段的行（本批判据自纠：红的是用例不是实现）。
    chat.onEvent(toolCall("c2"));
    chat.onEvent(toolResult("c2"));
    chat.onEvent(toolStatus("c2"));
    const second = rows(chat).find((r) => r.callId === "c2");
    expect(second!.status).toBe("settled");
  });

  it("tool_result 收束为 settled，成败读 ok（status 不复制第二份真相）", () => {
    const chat = useChatStore();
    chat.onEvent(toolCall("c1"));
    chat.onEvent(toolResult("c1", false));
    const row = rows(chat)[0];
    expect(row!.status).toBe("settled");
    expect(row!.ok).toBe(false);
  });

  it("重放路径同样给得出相位：等待态不由重放虚构（后端不发，前端不收）", () => {
    const chat = useChatStore();
    chat.replaying = true;
    chat.onEvent(userMessage("重放这条"));
    chat.onEvent(toolCall("c1"));
    chat.onEvent(toolResult("c1"));
    expect(rows(chat).map((r) => r.kind)).toEqual(["user", "tool"]);
    expect(rows(chat)[1]!.status).toBe("settled");
    expect("pending" in rows(chat)[1]!).toBe(false);
  });
});

describe("轮头容器：本轮文件改动不作独立行", () => {
  it("file_changes 挂到最近一条用户行（与 endTs 同一容器做法）", () => {
    const chat = useChatStore();
    chat.onEvent(userMessage("写两个文件"));
    chat.onEvent(toolCall("c1"));
    chat.onEvent(
      fileChanges([
        { path: "F:/w/a.rs", kind: "modified", additions: 2, deletions: 1 },
        { path: "F:/w/new.rs", kind: "created", additions: 5, deletions: 0 },
        { path: "F:/w/b.rs", kind: "overwritten" }, // NULL 档：算不出 diff
      ]),
    );
    const head = rows(chat)[0]!;
    const files = head.files as Array<Record<string, unknown>>;
    expect(files).toHaveLength(3);
    expect(files[0].additions).toBe(2);
    expect(files[2].additions).toBeUndefined(); // null → 不渲染数字，也不渲染 +0
    // 关键：文件清单**不新增行**（过程组不被它撑长）
    expect(chat.items).toHaveLength(2);
  });

  it("没有用户行可挂时不虚构归属（调度轮 = 计时行本就无从起算的同一缺口）", () => {
    const chat = useChatStore();
    chat.onEvent(fileChanges([{ path: "F:/w/c.rs", kind: "modified" }], 9));
    expect(chat.items).toHaveLength(0);
  });
});

// ---- 第三条族：组件层的**极性**（本批抓到的一处真错，见下）----
// 为什么 store 用例不够：`pending:boolean` → `status` 三相位是「一维布尔 → 多维枚举」的
// 迁移，`x` 与 `!x` 两侧都变成合法比较，**vue-tsc 全瞎**。本批把 `ToolRow.vue` 里
// `pending ? [] : assets` 抄成了 `status === "settled" ? [] : assets`（取反丢了），
// 表现 = 工具结果一回来，±N 与图片缩略当场消失——19+6 条用例与类型检查都没抓到它。
// 这一族钉的就是「settled 才出数」这个方向（累计第 152 条）。
describe("ToolRow 的 ±N 只在结果收束后出数（相位极性）", () => {
  const mounted = async (status: ToolRowStatus) => {
    const wrapper = mount(ToolRow, {
      props: {
        item: {
          id: 1,
          kind: "tool",
          callId: "c1",
          tool: "edit_replace",
          args: { path: "F:/w/a.rs" },
          sideEffect: "compensable",
          pluginVersion: "0.1.0",
          status,
          ok: status === "settled" ? true : null,
          output: status === "settled" ? { path: "F:/w/a.rs", additions: 2, deletions: 1 } : undefined,
        } satisfies ToolItem,
      },
      global: { plugins: [i18n] },
    });
    await flushPromises();
    return wrapper;
  };

  it("settled：±N 上屏，数字来自插件上报的 additions/deletions", async () => {
    const wrapper = await mounted("settled");
    expect(wrapper.find(".tool__diff").exists()).toBe(true);
    expect(wrapper.find(".tool__add").text()).toBe("+2");
    expect(wrapper.find(".tool__del").text()).toBe("−1"); // U+2212，与界面同字符
    wrapper.unmount();
  });

  it("running / awaiting_approval：未收束不许出数（取反丢了就长在这里）", async () => {
    for (const st of ["running", "awaiting_approval"] as ToolRowStatus[]) {
      const wrapper = await mounted(st);
      expect(wrapper.find(".tool__diff").exists()).toBe(false);
      expect(wrapper.find(".tool__assets").exists()).toBe(false);
      wrapper.unmount();
    }
  });
});
