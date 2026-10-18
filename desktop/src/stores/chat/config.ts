// stores/chat/config.ts —— 会话级配置的写穿（模型 / 推理档位 / 权限档 / 悬空裁决）。
// 三条同形：**先改本地 → 写穿后端 → 失败回滚本地**（本地显示必须如实，不许"显示已切、
// 后端还在旧档"）。唯一的例外 = resolveUncertain，它不预改本地，等事件回流投影。
import {
  sessionResolveUncertain,
  sessionSetModel,
  sessionSetPermissionMode,
  sessionSetThinking,
} from "../../lib/api";
import { useSettingsStore } from "../settings";
import { refreshSessions } from "./session";
import type { ChatState } from "./state";

/** v2.58 §3.30 选择模型：会话内 = 写会话级覆盖（设置不动）；首页
 * 空态 = 写设置缺省（切活动档 + 该档模型——"接下来聊什么模型"）。 */
export async function setModel(
  st: ChatState,
  providerId: string,
  model: string,
) {
  const inSession = st.sessions.some((s) => s.id === st.sessionId);
  if (!inSession) {
    await useSettingsStore().saveLlm({ active: providerId, model });
    return;
  }
  const prev = st.modelOverride;
  st.modelOverride = { providerId, model };
  try {
    await sessionSetModel(st.sessionId, providerId, model);
  } catch (err) {
    // 写穿失败回滚本地（后端仍按旧模型跑——本地显示必须如实）
    st.modelOverride = prev;
    throw err;
  }
}

/** v2.58 清除会话模型覆盖（选择器「跟随默认」行）。 */
export async function clearModelOverride(st: ChatState) {
  const prev = st.modelOverride;
  st.modelOverride = null;
  try {
    await sessionSetModel(st.sessionId, "", "");
  } catch (err) {
    st.modelOverride = prev;
    throw err;
  }
}

/** 批 B §5.2 设置会话推理档位（循环钮）：写旁车 + 缓存，下一请求即
 * 生效；level null = 清除（模型走自家缺省）。失败回滚本地。 */
export async function setThinking(st: ChatState, level: string | null) {
  const prev = st.thinkingLevel;
  st.thinkingLevel = level;
  try {
    await sessionSetThinking(st.sessionId, level ?? "");
  } catch (err) {
    st.thinkingLevel = prev;
    throw err;
  }
}

/** ㉘·B 设置会话权限档（§3.33 切换器）：写穿后端旁车 + 本地即时
 * 生效（下一笔工具调用即按新档裁决，运行中可切）。 */
export async function setPermissionMode(
  st: ChatState,
  mode: "plan" | "ask" | "acceptEdits" | "auto",
) {
  st.permissionMode = mode;
  try {
    await sessionSetPermissionMode(st.sessionId, mode);
  } catch (err) {
    // 写穿失败回滚本地（后端仍按旧档裁决——本地显示必须如实）
    await refreshSessions(st);
    st.permissionMode =
      st.sessions.find((s) => s.id === st.sessionId)?.permissionMode ?? null;
    throw err;
  }
}

/** G2 崩溃恢复裁决：落日志并实时回推（uncertain_resolved + 合成
 * tool_result 由投影同一开关收束卡与工具行，本地不预改）。 */
export async function resolveUncertain(
  st: ChatState,
  callId: string,
  decision: "completed" | "not_executed" | "inspected",
  note = "",
) {
  await sessionResolveUncertain(st.sessionId, callId, decision, note);
}
