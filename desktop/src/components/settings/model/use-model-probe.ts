/**
 * 表单模型清单探测 composable（§3.30 添加/编辑服务模态）：请求序号防旧
 * 覆新（慢响应不覆盖新键入的探测）；状态 idle/loading/ready/error +
 * 可用清单 + 错误摘要。防抖计时留在组件（依赖表单字段组合）。
 */
import { ref } from "vue";
import { llmProbeModels } from "../../../lib/api";

export function useModelProbe() {
  const probeState = ref<"idle" | "loading" | "ready" | "error">("idle");
  const available = ref<string[]>([]);
  const probeError = ref("");
  let seq = 0;

  async function run(
    kind: string,
    baseUrl: string,
    apiKey: string | null,
    profileId: string | null,
    emptyHint: string,
  ): Promise<void> {
    const mine = ++seq;
    if (!baseUrl) {
      probeState.value = "idle";
      available.value = [];
      return;
    }
    probeState.value = "loading";
    try {
      const list = await llmProbeModels(kind, baseUrl, apiKey, profileId);
      if (mine !== seq) return;
      available.value = list;
      probeState.value = list.length > 0 ? "ready" : "error";
      probeError.value = list.length > 0 ? "" : emptyHint;
    } catch (e) {
      if (mine !== seq) return;
      probeState.value = "error";
      available.value = [];
      probeError.value = String(e);
    }
  }

  function reset(): void {
    seq += 1;
    probeState.value = "idle";
    available.value = [];
    probeError.value = "";
  }

  return { probeState, available, probeError, run, reset };
}
