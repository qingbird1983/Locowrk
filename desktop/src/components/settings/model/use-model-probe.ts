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
  /** 推荐三选（批 F：后端目录数据驱动 ≤3；探测失败 = 空）。 */
  const recommended = ref<string[]>([]);
  const probeError = ref("");
  /** 端点扫描胜出的 base_url（§8.2；与传入不同时非空，表单采纳）。 */
  const effectiveBaseUrl = ref<string | null>(null);
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
    effectiveBaseUrl.value = null;
    try {
      const outcome = await llmProbeModels(kind, baseUrl, apiKey, profileId);
      if (mine !== seq) return;
      available.value = outcome.models;
      recommended.value = outcome.recommended ?? [];
      effectiveBaseUrl.value = outcome.effectiveBaseUrl;
      probeState.value = outcome.models.length > 0 ? "ready" : "error";
      probeError.value = outcome.error ?? (outcome.models.length > 0 ? "" : emptyHint);
    } catch (e) {
      if (mine !== seq) return;
      probeState.value = "error";
      available.value = [];
      recommended.value = [];
      probeError.value = String(e);
    }
  }

  function reset(): void {
    seq += 1;
    probeState.value = "idle";
    available.value = [];
    recommended.value = [];
    probeError.value = "";
    effectiveBaseUrl.value = null;
  }

  return { probeState, available, recommended, probeError, effectiveBaseUrl, run, reset };
}
