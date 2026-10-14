// `@tauri-apps/api/core` 的 `invoke` 替身（Q11 运行面的第一件）。
//
// ⚠ **命令名与参数键一律照生产抄，不照想象编**（历批第 117/140 条那一族：夹具按
// 想象写 ⇒ 测试全绿而真跑恒为空）：命令名 = `desktop/src-tauri` 侧的
// `#[tauri::command]` 名字（由 `check-commands.mjs` 的三向闭合名单给），参数键 =
// `src/lib/api.ts` 里那一句 `invoke(cmd, args)` 的第二参字面量。本文件只记录
// 「谁调了什么」，**不猜测后端会返回什么**——每个 payload 都由用例显式喂。
import { vi } from "vitest";

export type InvokeCall = { cmd: string; args?: Record<string, unknown> };

const calls: InvokeCall[] = [];
const responses = new Map<string, unknown>();

/** 替身本体：记录调用 + 按命令名返回预置值。**没有预置 = reject**（不是 resolve
 *  undefined）——静默成功会让"忘了喂数据"的用例长得像通过。 */
export const invoke = vi.fn((cmd: string, args?: Record<string, unknown>) => {
  calls.push({ cmd, args });
  if (!responses.has(cmd)) {
    return Promise.reject(new Error(`unstubbed command: ${cmd}`));
  }
  return Promise.resolve(responses.get(cmd));
});

export function onCommand(cmd: string, payload: unknown): void {
  responses.set(cmd, payload);
}

export function invokeCalls(): InvokeCall[] {
  return calls.slice();
}

/** 用例隔离用：清调用记录与预置值（不清 mock 实现本身）。 */
export function resetIpc(): void {
  calls.length = 0;
  responses.clear();
  invoke.mockClear();
}
