// Q12·第二刀（v2.205）：**轮头读数**的行语义与组件渲染。
//
// 补的是什么空洞：`turn_header` 是新增事件（状态四态 + 扣过等待的工作时长），
// store 与组件两侧在上一批都没有对应断言 ⇒ 光靠 vue-tsc 只保证字段名对得上，
// 保证不了「拿不到 active_ms 时不许当 0」这条语义。这一族钉三件事：
//   ① 事件挂到该轮的 user 行（与 files / endTs 同容器，不另造行种）；
//   ② `active_ms` 缺省 = 不写字段（旧日志的如实降级），写成 0 就是谎报；
//   ③ 计时行收束读数优先用宿主的 active_ms，缺省才回落墙钟差。
//
// 事件形状照 `desktop/src-tauri/src/events.rs` 的 serde 输出写（`kind` 标签 +
// snake_case + `state` 四值），不凭想象编字段（本仓第 150 条同族纪律）。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import { useChatStore } from "../src/stores/chat";
import TurnTimerLine from "../src/components/chat/TurnTimerLine.vue";
import { i18n } from "../src/i18n";
import type { AgentEvent } from "../src/types";

const START = 1_700_000_000_000;
/** 墙钟 550 秒 = 9 分 10 秒。 */
const END = START + 550_000;
/** 其中等用户点审批 480 秒 ⇒ 真正干活 70 秒 = 1 分 10 秒。 */
const ACTIVE = 70_000;

beforeEach(() => {
  setActivePinia(createPinia());
  // 读数格式按 locale 分叉（zh「N 分 N 秒」/ en「Nm Ns」），用例钉中文格式
  (i18n.global.locale as unknown as { value: string }).value = "zh-CN";
});

const userMessage = (content: string): AgentEvent =>
  ({
    kind: "user_message",
    session_id: "s1",
    content,
    pos: 0,
    parts: [],
  }) as unknown as AgentEvent;

const turnHeader = (state: string, activeMs?: number | null): AgentEvent =>
  ({
    kind: "turn_header",
    session_id: "s1",
    turn_id: 3,
    state,
    ...(activeMs === undefined ? {} : { active_ms: activeMs }),
  }) as unknown as AgentEvent;

function rows(chat: ReturnType<typeof useChatStore>) {
  return chat.items as Array<Record<string, unknown>>;
}

describe("轮头事件的行语义（turn_header → user 行）", () => {
  it("状态与扣过等待的时长挂在**最近一条 user 行**，不另造行种", () => {
    const chat = useChatStore();
    chat.onEvent(userMessage("第一问"));
    chat.onEvent(turnHeader("interrupted", ACTIVE));
    expect(rows(chat).length).toBe(1);
    const row = rows(chat)[0]!;
    expect(row.turnState).toBe("interrupted");
    expect(row.activeMs).toBe(ACTIVE);
  });

  it("active_ms 缺省时只写状态，**不得把\"读不出\"写成 0**", () => {
    const chat = useChatStore();
    chat.onEvent(userMessage("旧会话"));
    chat.onEvent(turnHeader("completed", null));
    const row = rows(chat)[0]!;
    expect(row.turnState).toBe("completed");
    expect("activeMs" in row).toBe(false);
  });

  it("调度注入轮没有 user 行 ⇒ 不虚构归属（不建行、不暂存死数据）", () => {
    const chat = useChatStore();
    chat.onEvent(turnHeader("completed", 1_000));
    expect(rows(chat).length).toBe(0);
  });
});

describe("轮计时行的读数来源（宿主 active_ms 优先，缺省回落墙钟）", () => {
  // props 类型显式写出来（`running` 是必填项）：写成 `Record<string, unknown>` 时
  // vue-tsc 直接把这条用例判红（TS2322），组件契约不许在测试里松一档。
  const mounted = async (props: {
    startTs?: number;
    endTs?: number;
    activeMs?: number;
    running: boolean;
  }) => {
    // 断言只锁**读数结构**、不锁中文文案：钉中文串在 en 档必漂（q11 判据 S5 就是管这个），
    // 而这条用例要钉的是「用的是哪个数」，不是「用哪种语言说它」。
    i18n.global.locale.value = "en";
    const wrapper = mount(TurnTimerLine, { props, global: { plugins: [i18n] } });
    await flushPromises();
    return wrapper;
  };

  it("收束 + activeMs 在场：显示扣过等待的那个数，而不是墙钟那个数", async () => {
    const wrapper = await mounted({
      startTs: START,
      endTs: END,
      activeMs: ACTIVE,
      running: false,
    });
    expect(wrapper.text()).toContain("1m 10s");
    expect(wrapper.text()).not.toContain("9m 10s");
    wrapper.unmount();
  });

  it("收束 + activeMs 缺省：如实回落到 endTs − startTs 的墙钟读数", async () => {
    const wrapper = await mounted({ startTs: START, endTs: END, running: false });
    expect(wrapper.text()).toContain("9m 10s");
    wrapper.unmount();
  });

  it("轮还在跑时不许采用宿主给的 activeMs（那还没有本轮读数）", async () => {
    const wrapper = await mounted({
      startTs: START,
      endTs: END,
      activeMs: ACTIVE,
      running: true,
    });
    expect(wrapper.text()).not.toContain("1m 10s");
    wrapper.unmount();
  });

  it("无起点 = 无从起算 ⇒ 只剩发丝线，不虚构时序", async () => {
    const wrapper = await mounted({ running: false, activeMs: ACTIVE });
    expect(wrapper.text()).toBe("");
    wrapper.unmount();
  });
});
