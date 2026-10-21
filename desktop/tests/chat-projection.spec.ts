// 台账 T3：`agent/event` 投影的分发表（`src/stores/chat.ts` 的 `onEvent`，21 个事件变体）。
//
// 为什么先写用例而不是先抽纯函数：这条账登记的理由是"事件投影不可单测"，而 Q11（v2.202）
// 已经把 store 层测起来了——`chat-row-semantics.spec.ts` 用 `createPinia + setActivePinia`
// 直调 action，零 Tauri mock。所以"不可测"这句旧论断已不再是事实，剩下的真缺口是
// **这张分发表本身零覆盖**：哪个事件动谁、每条守卫在挡什么，今天只靠真机走查。
// 这一份先把它落成机器证明；如果中途撞到"不起 mock 就动不了"的点，那才是"必须抽纯"的证据
// ——先拿证据再动结构，不反过来（把 1148 行整段搬去一个"纯"文件而收益不明，是本项目禁止的搬家）。
import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useAsksStore } from "../src/stores/asks";
import { useChatStore } from "../src/stores/chat";
import type { ChatItem } from "../src/stores/chat";

const S = "s1";

type Store = ReturnType<typeof useChatStore>;

/** 按 kind 取第一条（投影出来的行只认 kind，不断 id：`nextId` 是模块级自增，
 *  断具体值 = 断测试文件的加载顺序，那种断言换个文件就漂）。 */
const rowOf = <K extends ChatItem["kind"]>(
  chat: Store,
  kind: K,
): Extract<ChatItem, { kind: K }> | undefined =>
  chat.items.find((i) => i.kind === kind) as Extract<ChatItem, { kind: K }> | undefined;

const kinds = (chat: Store) => chat.items.map((i) => i.kind);

/** 换档重跑同一场景。**不是可有可无的糖**：pinia 在同一实例里按 store id 返回同一份
 *  state，第二次 `useChatStore()` 拿到的还是上一个场景写过的行——第一版就是这么红了三条
 *  （断言读到残留），不是生产码的错。要干净档只能 `$reset()` 再设 `replaying`。 */
const fresh = (replaying: boolean): Store => {
  const chat = useChatStore();
  chat.$reset();
  chat.replaying = replaying;
  return chat;
};

beforeEach(() => {
  // pinia 原生档（同 chat-row-semantics.spec.ts）：`@pinia/testing` 的 peer 要
  // pinia >= 4.0.2 而本仓是 ^3.0.0，引它 = 制造版本冲突。
  setActivePinia(createPinia());
});

describe("分发表：每个事件动谁", () => {
  it("chunk 走 appendAssistant（相邻同 kind 并进同一行，不各造气泡）", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "chunk", session_id: S, delta: "你好" });
    chat.onEvent({ kind: "chunk", session_id: S, delta: "，世界" });
    expect(rowOf(chat, "assistant")?.text).toBe("你好，世界");
    expect(kinds(chat)).toEqual(["assistant"]);
  });

  it("thinking 走 appendThinking，且正文 chunk 到达时先收口思考行", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "thinking", session_id: S, text: "先看目录" });
    chat.onEvent({ kind: "chunk", session_id: S, delta: "答案" });
    const th = rowOf(chat, "thinking")!;
    expect(th.text).toBe("先看目录");
    // settleThinking 只在实时档写 doneTs（重放条目无 ts，天然空转）
    expect(th.doneTs).toBeTypeOf("number");
    expect(kinds(chat)).toEqual(["thinking", "assistant"]);
  });

  it("user_message 恢复用户气泡并带 pos（编辑重发的定位键）", () => {
    const chat = useChatStore();
    chat.replaying = true;
    chat.onEvent({ kind: "user_message", session_id: S, content: "重发我", pos: 7 });
    const u = rowOf(chat, "user")!;
    expect(u.text).toBe("重发我");
    expect(u.pos).toBe(7);
  });

  it("tool_call 起一条 running 行、ok=null（成败由 tool_result 自带）", () => {
    const chat = useChatStore();
    chat.onEvent({
      kind: "tool_call",
      session_id: S,
      call_id: "c1",
      tool: "fs_write",
      args: { path: "a.txt" },
      side_effect: "irreversible",
      plugin_version: "0.1.6",
    });
    const t = rowOf(chat, "tool")!;
    expect([t.status, t.ok, t.callId]).toEqual(["running", null, "c1"]);
  });

  it("tool_result 命中宿主行才写，且写满三样（status/ok/output）", () => {
    const chat = useChatStore();
    chat.onEvent({
      kind: "tool_call",
      session_id: S,
      call_id: "c1",
      tool: "fs_write",
      args: null,
      side_effect: "irreversible",
      plugin_version: "0.1.6",
    });
    chat.onEvent({
      kind: "tool_result",
      session_id: S,
      call_id: "c1",
      ok: false,
      output: null,
      error: "boom",
    });
    const t = rowOf(chat, "tool")!;
    expect([t.status, t.ok, t.error]).toEqual(["settled", false, "boom"]);
  });

  it("approval_resolved 落一条 decision 记录行（决策记录的唯一来源）", () => {
    const chat = useChatStore();
    chat.onEvent({
      kind: "approval_resolved",
      session_id: S,
      call_id: "c1",
      tool: "fs_delete",
      allowed: false,
    });
    const d = rowOf(chat, "decision")!;
    expect([d.tool, d.allowed]).toEqual(["fs_delete", false]);
  });

  it("context_compacted 落系统行并把摘要挂在该行的 id 上（[查看详情] 数据源）", () => {
    const chat = useChatStore();
    chat.onEvent({
      kind: "context_compacted",
      session_id: S,
      from_tokens: 3000,
      to_tokens: 1000,
      strategy: "layered",
      summary_preview: "摘要预览",
    });
    const sys = rowOf(chat, "system")!;
    expect(sys.actions).toEqual(["details", "undo"]);
    expect(chat.noticeDetails[sys.id]).toBe("摘要预览");
    // 差值 3000-1000=2000 是投影算的；`≈2K` 那个 ≈ 是 format.ts 的档位口径
    // （1953 不是整千）——本条只证差值没算反，格式化归它自己的用例。
    expect((sys.params as { saved: string }).saved).toBe("≈2K");
  });
});

describe("守卫：缺数据时不许虚构", () => {
  it("turn_header 的 active_ms=null ⇒ 不写 activeMs（缺 ≠ 一秒没干活）", () => {
    const chat = useChatStore();
    chat.replaying = true;
    chat.onEvent({ kind: "user_message", session_id: S, content: "问", pos: 1 });
    chat.onEvent({
      kind: "turn_header",
      session_id: S,
      state: "completed",
      active_ms: null,
    });
    const u = rowOf(chat, "user")!;
    expect(u.turnState).toBe("completed");
    expect("activeMs" in u).toBe(false);
  });

  it("turn_header 带读数时写入原值（0 是真 0，与上一条的缺档不同断言）", () => {
    const chat = useChatStore();
    chat.replaying = true;
    chat.onEvent({ kind: "user_message", session_id: S, content: "问", pos: 1 });
    chat.onEvent({ kind: "turn_header", session_id: S, state: "failed", active_ms: 0 });
    expect(rowOf(chat, "user")!.activeMs).toBe(0);
  });

  it("没有用户行的调度轮：turn_header / file_changes 都不虚构归属", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "turn_header", session_id: S, state: "completed" });
    chat.onEvent({
      kind: "file_changes",
      session_id: S,
      files: [{ path: "a.ts", kind: "created" }],
    });
    expect(chat.items).toHaveLength(0);
  });

  it("file_changes 挂在 user 行上，不作为独立行（同类只许一种形态）", () => {
    const chat = useChatStore();
    chat.replaying = true;
    chat.onEvent({ kind: "user_message", session_id: S, content: "问", pos: 1 });
    chat.onEvent({
      kind: "file_changes",
      session_id: S,
      files: [{ path: "a.ts", kind: "created", additions: 3, deletions: null }],
    });
    expect(chat.items).toHaveLength(1);
    expect(rowOf(chat, "user")!.files).toHaveLength(1);
  });

  it("tool_result 找不到宿主行 ⇒ 静默，不补一条假行", () => {
    const chat = useChatStore();
    chat.onEvent({
      kind: "tool_result",
      session_id: S,
      call_id: "幽灵",
      ok: true,
      output: 1,
      error: null,
    });
    expect(chat.items).toHaveLength(0);
  });

  it("tool_status 只认 pending_approval，且已收束的行不许退回等待态", () => {
    const chat = useChatStore();
    const call = {
      kind: "tool_call" as const,
      session_id: S,
      call_id: "c1",
      tool: "shell",
      args: null,
      side_effect: "irreversible",
      plugin_version: "0.1.8",
    };
    chat.onEvent(call);
    chat.onEvent({ kind: "tool_status", session_id: S, call_id: "c1", status: "pending_approval" });
    expect(rowOf(chat, "tool")!.status).toBe("awaiting_approval");

    // 乱序/竞态：结果已回之后再收到点亮信号，保持事实态
    chat.onEvent({
      kind: "tool_result",
      session_id: S,
      call_id: "c1",
      ok: true,
      output: "done",
      error: null,
    });
    chat.onEvent({ kind: "tool_status", session_id: S, call_id: "c1", status: "pending_approval" });
    expect(rowOf(chat, "tool")!.status).toBe("settled");
  });

  it("turn_done 不带 steps 时保留旧读数（TurnEnd 重放投影本就不带，宁缺勿假）", () => {
    const chat = useChatStore();
    chat.steps = 5;
    chat.onEvent({ kind: "turn_done", session_id: S });
    expect(chat.steps).toBe(5);
    expect(chat.running).toBe(false);
  });

  it("turn_done 的 endTs 只写第一次（已有不覆盖）", () => {
    const chat = useChatStore();
    chat.replaying = true;
    chat.onEvent({ kind: "user_message", session_id: S, content: "问", pos: 1, ts: 100 });
    chat.onEvent({ kind: "turn_done", session_id: S, ts: 900 });
    chat.onEvent({ kind: "turn_done", session_id: S, ts: 1500 });
    expect(rowOf(chat, "user")!.endTs).toBe(900);
  });

  it("llm_usage 的 duration_ms=0 ⇒ speed 保持 null（除零不许伪装成 0 tokens/s）", () => {
    const chat = useChatStore();
    chat.onEvent({
      kind: "llm_usage",
      session_id: S,
      input: 100,
      output: 50,
      cache_read: 0,
      cache_write: 0,
      reasoning: 0,
      total: 150,
      duration_ms: 0,
    });
    expect(chat.turnUsage!.speed).toBeNull();
    expect(chat.turnUsage!.steps).toBe(1);
  });
});

describe("实时与重放同形", () => {
  it("实时给 user 行打本地 ts，重放缺 ts 就不虚构", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "user_message", session_id: S, content: "实时", pos: 1 });
    expect(rowOf(chat, "user")!.ts).toBeTypeOf("number");

    const replay = fresh(true);
    replay.onEvent({ kind: "user_message", session_id: S, content: "重放", pos: 1 });
    expect("ts" in replay.items[0]!).toBe(false);
  });

  it("user_message 开新轮 ⇒ 轮级用量重置（重放同序重建）", () => {
    const chat = useChatStore();
    chat.onEvent({
      kind: "llm_usage",
      session_id: S,
      input: 10,
      output: 5,
      cache_read: 0,
      cache_write: 0,
      reasoning: 0,
      total: 15,
      duration_ms: 10,
    });
    expect(chat.turnUsage).not.toBeNull();
    chat.replaying = true;
    chat.onEvent({ kind: "user_message", session_id: S, content: "下一轮", pos: 2 });
    expect(chat.turnUsage).toBeNull();
  });

  it("llm_retry 清掉本步未完成的流式正文；告知行只实时发、重放不发", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "chunk", session_id: S, delta: "半截" });
    chat.onEvent({ kind: "llm_retry", session_id: S, attempt: 2, delay_ms: 500 });
    expect(rowOf(chat, "assistant")!.text).toBe("");
    expect(rowOf(chat, "system")?.key).toBe("notice.llmRetried");

    const replay = fresh(true);
    replay.onEvent({ kind: "chunk", session_id: S, delta: "半截" });
    replay.onEvent({ kind: "llm_retry", session_id: S, attempt: 2, delay_ms: 500 });
    expect(kinds(replay)).toEqual(["assistant"]);
  });

  it("tool_uncertain 复位与建卡；uncertain_resolved 两侧都找不到时静默（重放幂等）", () => {
    const chat = useChatStore();
    chat.onEvent({
      kind: "tool_uncertain",
      session_id: S,
      call_id: "c9",
      tool: "fs_delete",
      args: null,
    });
    expect(rowOf(chat, "uncertain")?.callId).toBe("c9");

    chat.onEvent({
      kind: "uncertain_resolved",
      session_id: S,
      call_id: "c9",
      tool: "fs_delete",
      decision: "not_executed",
      note: "人工核对",
    });
    expect(rowOf(chat, "uncertain")!.resolved).toEqual({
      decision: "not_executed",
      note: "人工核对",
    });

    const orphan = fresh(true);
    orphan.onEvent({
      kind: "uncertain_resolved",
      session_id: S,
      call_id: "没有宿主",
      tool: "fs_delete",
      decision: "inspected",
      note: "",
    });
    expect(orphan.items).toHaveLength(0);
  });
});

describe("跨 store 转发：对话流零气泡", () => {
  it("ask_user 进 composer 坞的 asks store，对话流一条都不加", () => {
    const chat = useChatStore();
    const asks = useAsksStore();
    chat.onEvent({
      kind: "ask_user",
      request_id: "q1",
      question: "选哪个",
      options: [{ label: "A" }],
      multi_select: false,
      timeout_secs: 0,
    });
    expect(chat.items).toHaveLength(0);
    expect(asks.pending.map((p) => p.request_id)).toEqual(["q1"]);
  });

  it("ask_user 重复投递按 request_id 去重（一个事实一个出处）", () => {
    const chat = useChatStore();
    const asks = useAsksStore();
    const ev = {
      kind: "ask_user" as const,
      request_id: "q1",
      question: "选哪个",
      options: [{ label: "A" }],
      multi_select: false,
      timeout_secs: 0,
    };
    chat.onEvent(ev);
    chat.onEvent(ev);
    expect(asks.pending).toHaveLength(1);
  });

  it("ask_resolved 摘卡（应答/跳过/超时/停止四路同源走这里）", () => {
    const chat = useChatStore();
    const asks = useAsksStore();
    chat.onEvent({
      kind: "ask_user",
      request_id: "q1",
      question: "选哪个",
      options: [{ label: "A" }],
      multi_select: false,
      timeout_secs: 0,
    });
    chat.onEvent({ kind: "ask_resolved", request_id: "q1" });
    expect(asks.pending).toHaveLength(0);
  });

  it("schedule_fired / turn_capped 各落一条告知行并把读数放进 params（不拼死文案）", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "schedule_fired", session_id: S, schedule_id: "t1", title: "晨报" });
    chat.onEvent({ kind: "turn_capped", session_id: S, steps: 40 });
    const notices = chat.items.filter((i) => i.kind === "system") as Extract<
      ChatItem,
      { kind: "system" }
    >[];
    expect(notices.map((n) => n.key)).toEqual(["notice.scheduleFired", "notice.turnCapped"]);
    expect(notices[0]!.params).toEqual({ title: "晨报" });
    expect(notices[1]!.params).toEqual({ steps: 40 });
  });
});

describe("聚合读数：一份输入两处产出", () => {
  it("llm_usage 同时累加轮级与会话级，且 speed 取最后一步", () => {
    const chat = useChatStore();
    const usage = (input: number, output: number, duration_ms: number) =>
      ({
        kind: "llm_usage" as const,
        session_id: S,
        input,
        output,
        cache_read: 8,
        cache_write: 2,
        reasoning: 1,
        total: input + output,
        duration_ms,
      }) as const;
    chat.onEvent(usage(100, 50, 50));
    chat.onEvent(usage(100, 100, 100));
    const u = chat.turnUsage!;
    expect([u.input, u.output, u.total, u.steps]).toEqual([200, 150, 350, 2]);
    expect(u.speed).toBe(1000);
    const su = chat.sessionUsage!;
    expect([su.input, su.output, su.cacheRead, su.steps]).toEqual([200, 150, 16, 2]);
  });

  it("context_usage 原样带四档（S4 起不再有扣减链三字段）", () => {
    const chat = useChatStore();
    chat.onEvent({
      kind: "context_usage",
      session_id: S,
      used: 1200,
      threshold: 2000,
      window_tokens: 8000,
      tools_est_tokens: 0,
    });
    expect(chat.contextUsage).toEqual({
      used: 1200,
      threshold: 2000,
      windowTokens: 8000,
      toolsEstTokens: 0,
    });
  });
});

// specs/12 刀②：自动压缩的界面反馈（`agent://compact-status` 的四个相位）。
//
// 形状与这一份里其它用例同款（store 级直调 `onEvent`），但这一族抓的是**一根
// 线**的形状：自动压缩在每个 step 边界都会再问一次要不要压 ⇒ 一轮里可能连着
// 失败几次。按"改写最后一根同 tag 的线"实现，才能保证界面上一轮只留一根
// （按 key 改写会攒出一串同义的线：正在压缩 / 没压成 / 正在压缩 / 没压成）。
describe("自动压缩的过程反馈：四相共用一根发丝线", () => {
  /** 系统条那几行的 key（顺序 = 落列顺序）。 */
  const sysKeys = (chat: ReturnType<typeof useChatStore>) =>
    chat.items.filter((i) => i.kind === "system").map((i) => (i as { key?: string }).key);

  it("开始 ⇒ 没成：同一根线就地改写，不攒出第二根", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "compact_status", session_id: S, phase: "running" });
    expect(sysKeys(chat)).toEqual(["notice.compactRunning"]);

    chat.onEvent({
      kind: "compact_status",
      session_id: S,
      phase: "failed",
      reason: "provider-error",
    });
    // 一根线（不是两根）：「没压成」把「正在压缩」**就地改写**掉了。
    expect(sysKeys(chat)).toEqual(["notice.compactProviderError"]);
    const line = chat.items.find((i) => i.kind === "system") as {
      tone?: string;
      tag?: string;
    };
    expect(line.tone, "仍是发丝线（v2.143 那一根形态）").toBe("hairline");
    expect(line.tag).toBe("compact-auto");
  });

  it("连着失败两次也只留一根线（末态就是真相）", () => {
    const chat = useChatStore();
    for (const reason of ["provider-error", "budget"]) {
      chat.onEvent({ kind: "compact_status", session_id: S, phase: "running" });
      chat.onEvent({ kind: "compact_status", session_id: S, phase: "failed", reason });
    }
    expect(sysKeys(chat)).toEqual(["notice.compactBudget"]);
    expect(chat.items.filter((i) => i.kind === "system")).toHaveLength(1);
  });

  it("摘要正文流式到达 ⇒ 累加到那根线的「查看详情」上（转录里不另起一块）", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "compact_status", session_id: S, phase: "running" });
    chat.onEvent({
      kind: "compact_status",
      session_id: S,
      phase: "delta",
      text: "## Objective\n- ",
    });
    chat.onEvent({
      kind: "compact_status",
      session_id: S,
      phase: "delta",
      text: "做完那件事\n",
    });
    // delta 不另起线：那根发丝线仍是同一根（形态只有一种）。
    expect(sysKeys(chat)).toEqual(["notice.compactRunning"]);
    const line = chat.items.find((i) => i.kind === "system") as {
      id: number;
      actions?: string[];
    };
    expect(line.actions, "在途那根线带一个「查看详情」").toEqual(["details"]);
    expect(chat.noticeDetails[line.id]).toBe("## Objective\n- 做完那件事\n");
  });

  it("熔断 ⇒ 「已暂停」那句（走相位，不走判据名那张表）", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "compact_status", session_id: S, phase: "running" });
    chat.onEvent({ kind: "compact_status", session_id: S, phase: "paused" });
    expect(sysKeys(chat)).toEqual(["notice.compactPaused"]);
  });

  it("压成了 ⇒ 这根线收走（断层条接手说省了多少）", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "compact_status", session_id: S, phase: "running" });
    chat.onEvent({ kind: "compact_status", session_id: S, phase: "done" });
    expect(sysKeys(chat)).toEqual([]);
  });

  it("认不出的判据名走兜底那条，并把名字原样带出（绝不悄悄说成已压缩）", () => {
    const chat = useChatStore();
    chat.onEvent({
      kind: "compact_status",
      session_id: S,
      phase: "failed",
      reason: "brand-new-reason",
    });
    const line = chat.items.find((i) => i.kind === "system") as {
      key?: string;
      params?: Record<string, unknown>;
    };
    expect(line.key).toBe("notice.compactUnknown");
    expect(line.params?.reason).toBe("brand-new-reason");
  });
});
