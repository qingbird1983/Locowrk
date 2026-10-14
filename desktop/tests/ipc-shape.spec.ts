// Q11 第一条：IPC 包装面（`src/lib/api.ts`）——**它发的东西长什么样**必须有机器证明。
// 为什么这条最该先立：`api.ts` 是 Tauri command 的薄适配层（AGENTS 架构边界「command 只做
// 参数转换，不写业务逻辑」的前端那一半），它把 JS 的驼峰键转成后端 serde 认的形状、
// 把 `undefined` 归一成 `null`、把可选支拼成两种 payload。这三件事全都不是"业务"，
// 一旦漂了**运行时不会报错**——后端按 `Option<T>` 收下 `null`，只是行为变了。
// Q12（CF 稿 §10 · C1）要往这条面上加新命令，没有它就没有"重放回归"的第一层。
//
// ⚠ 断言的对象 = `api.ts` 里 `invoke(cmd, args)` 的第二参**字面量**；命令名 = 后端
// `#[tauri::command]` 的名字（两侧由 `check-commands.mjs` 三向闭合，本用例不重抄那道闸）。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import { askRespond, memoryInjectView, memoryList, memoryTreeStream, setSettings } from "../src/lib/api";
import { invokeCalls, onCommand, resetIpc } from "./tauri-stub";

beforeEach(() => {
  resetIpc();
  // 本组用例只判「发出去的形状」，返回值不参与 ⇒ 预置成 resolve(undefined)。
  // ⚠ 这里**不给假 payload**：编一个后端不会返回的对象，等于让夹具冒充契约。
  for (const cmd of [
    "memory_inject_view",
    "memory_list",
    "ask_respond",
    "settings_set",
    "memory_tree_stream",
  ]) {
    onCommand(cmd, undefined);
  }
});

describe("api 包装层的命令名与参数形状", () => {
  it("无参命令不发第二参（memory_inject_view 的调用形状 = 命令名 + undefined）", async () => {
    await memoryInjectView();
    expect(invokeCalls()).toEqual([{ cmd: "memory_inject_view", args: undefined }]);
  });

  it("缺省参数归一成 null 而不是省略键（memory_list 的 query/limit 两支）", async () => {
    await memoryList();
    await memoryList("docker", 3);
    const calls = invokeCalls();
    expect(calls[0]).toEqual({ cmd: "memory_list", args: { query: null, limit: null } });
    expect(calls[1]).toEqual({ cmd: "memory_list", args: { query: "docker", limit: 3 } });
  });

  it("ask_respond 的两支各是一种后端形状：answer 的 note 缺省归一为空串，skip 带 reason:null", async () => {
    await askRespond("r1", { kind: "answer", labels: ["A", "B"] });
    await askRespond("r2", { kind: "answer", labels: [], note: "补充" });
    await askRespond("r3", { kind: "skip" });
    const calls = invokeCalls();
    expect(calls[0]).toEqual({
      cmd: "ask_respond",
      args: { requestId: "r1", answer: { kind: "answer", labels: ["A", "B"], note: "" } },
    });
    expect(calls[1]).toEqual({
      cmd: "ask_respond",
      args: { requestId: "r2", answer: { kind: "answer", labels: [], note: "补充" } },
    });
    // skip 支的 `reason: null` 是后端判别式里的字段，前端省略它 = 少一个可辨识分支
    expect(calls[2]).toEqual({
      cmd: "ask_respond",
      args: { requestId: "r3", answer: { kind: "skip", reason: null } },
    });
  });

  it("settings_set 是「全键 + null 占位」的固定形状：只传 locale 时其余十一键仍在场", async () => {
    await setSettings({ locale: "en" });
    const [call] = invokeCalls();
    expect(call?.cmd).toBe("settings_set");
    expect(call?.args).toEqual({
      locale: "en",
      appearance: null,
      shortcut: null,
      llm: null,
      embedding: null,
      context: null,
      memory: null,
      tools: null,
      plugins: null,
      chat: null,
      developer: null,
      thinking: null,
    });
    // 键数也是口径：少一键 = 包装忘了转（Q12 加区时这条会红，正是想要的）
    expect(Object.keys(call?.args ?? {})).toHaveLength(12);
  });

  it("memory_tree_stream 的组键原样上送，不在前端拆成两个数（队列 Q14：组键的身份是字符串）", async () => {
    await memoryTreeStream("s1", "F:\\ws\\demo", "0:5");
    await memoryTreeStream("s1", "F:\\ws\\demo", "orphan");
    const calls = invokeCalls();
    // 参数键 = 后端命令形参（sessionId / workspace / group）；组键**逐字**上送，
    // 前端一旦在这里 parse 或改名，后端那条「区间查无 = 拒」的判据就永远打不到。
    expect(calls[0]).toEqual({
      cmd: "memory_tree_stream",
      args: { sessionId: "s1", workspace: "F:\\ws\\demo", group: "0:5" },
    });
    expect(calls[1]).toEqual({
      cmd: "memory_tree_stream",
      args: { sessionId: "s1", workspace: "F:\\ws\\demo", group: "orphan" },
    });
  });
});
