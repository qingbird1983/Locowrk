/**
 * 内置服务商预设（UI shell 文档 §3.30 模型配置页，参考 PI-Desktop
 * NAMED_ENDPOINT_PRESETS 收敛到本项目三种 kind）。只收「OpenAI 兼容
 * /v1 形态 + Anthropic + Ollama」三种本项目后端真能路由的服务商——
 * 不 invent 路由不了的瓦片（约束最小化 + 如实呈现）。baseUrl 允许带
 * 任意 /v<数字> 版本段（后端 join_version 归一：缺段补 /v1）。
 *
 * 全部为纯函数 + 静态表，不发请求；搜索用 haystack 子串匹配（名称/id/
 * 别名/域名都能命中，"智谱""bigmodel""open.bigmodel.cn" 落同一条）。
 */

export type PresetKind = "openai" | "anthropic" | "ollama";

export interface ProviderPreset {
  id: string;
  name: string;
  baseUrl: string;
  kind: PresetKind;
  aliases?: string[];
}

export const CUSTOM_SERVICE = "custom";

export const PROVIDER_PRESETS: readonly ProviderPreset[] = [
  { id: "deepseek", name: "DeepSeek", baseUrl: "https://api.deepseek.com", kind: "openai", aliases: ["深度求索"] },
  { id: "zhipu", name: "智谱 AI", baseUrl: "https://open.bigmodel.cn/api/paas/v4", kind: "openai", aliases: ["zhipu", "bigmodel", "glm"] },
  { id: "zai", name: "Z.AI", baseUrl: "https://api.z.ai/api/paas/v4", kind: "openai", aliases: ["zai", "智谱国际"] },
  { id: "moonshot", name: "月之暗面 Kimi", baseUrl: "https://api.moonshot.cn/v1", kind: "openai", aliases: ["moonshot", "kimi"] },
  { id: "minimax", name: "MiniMax", baseUrl: "https://api.minimaxi.com/v1", kind: "openai", aliases: ["minimax"] },
  { id: "siliconflow", name: "硅基流动", baseUrl: "https://api.siliconflow.cn/v1", kind: "openai", aliases: ["siliconflow"] },
  { id: "volcengine", name: "火山方舟", baseUrl: "https://ark.cn-beijing.volces.com/api/v3", kind: "openai", aliases: ["volcengine", "doubao", "豆包", "ark"] },
  { id: "dashscope", name: "通义千问", baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1", kind: "openai", aliases: ["dashscope", "qwen", "阿里云百炼"] },
  { id: "xiaomi", name: "小米 MiMo", baseUrl: "https://api.xiaomimimo.com/v1", kind: "openai", aliases: ["xiaomi", "mimo"] },
  { id: "openai", name: "OpenAI", baseUrl: "https://api.openai.com/v1", kind: "openai" },
  { id: "anthropic", name: "Anthropic", baseUrl: "https://api.anthropic.com/v1", kind: "anthropic", aliases: ["claude"] },
  { id: "openrouter", name: "OpenRouter", baseUrl: "https://openrouter.ai/api/v1", kind: "openai" },
  { id: "groq", name: "Groq", baseUrl: "https://api.groq.com/openai/v1", kind: "openai" },
  { id: "xai", name: "xAI", baseUrl: "https://api.x.ai/v1", kind: "openai", aliases: ["grok"] },
  { id: "mistral", name: "Mistral", baseUrl: "https://api.mistral.ai/v1", kind: "openai" },
  { id: "together", name: "Together AI", baseUrl: "https://api.together.xyz/v1", kind: "openai", aliases: ["together"] },
  { id: "fireworks", name: "Fireworks", baseUrl: "https://api.fireworks.ai/inference/v1", kind: "openai", aliases: ["fireworks"] },
  { id: "nvidia", name: "NVIDIA", baseUrl: "https://integrate.api.nvidia.com/v1", kind: "openai", aliases: ["nim"] },
  { id: "baseten", name: "Baseten", baseUrl: "https://inference.baseten.co/v1", kind: "openai" },
  { id: "cerebras", name: "Cerebras", baseUrl: "https://api.cerebras.ai/v1", kind: "openai" },
  { id: "huggingface", name: "Hugging Face", baseUrl: "https://router.huggingface.co/v1", kind: "openai", aliases: ["hf"] },
  { id: "ollama-local", name: "Ollama 本地", baseUrl: "http://localhost:11434", kind: "ollama", aliases: ["ollama", "本地"] },
];

/** 选服务瓦片格的一行选项（含自定义端点，居首由渲染方排）。 */
export interface ServiceOption {
  id: string;
  name: string;
  /** 域名/路径 detail（自定义端点为空串）。 */
  endpoint: string;
  kind?: PresetKind;
  baseUrl?: string;
  haystack: string;
}

/** URL 的 host + 去尾斜杠路径（瓦片 detail 与行 meta 用）。 */
export function endpointLabel(url: string): string {
  try {
    const parsed = new URL(url);
    return parsed.host + parsed.pathname.replace(/\/+$/, "");
  } catch {
    return url;
  }
}

/** baseUrl 的 host 部分（服务行 meta 第一段；解析失败原样返回）。 */
export function hostOf(url: string | null | undefined): string {
  if (!url) return "";
  try {
    return new URL(url).host || url;
  } catch {
    return url.replace(/^https?:\/\//, "").split("/")[0] || url;
  }
}

/** 名称首字母/数字大写（字母瓦片；无字母数字返回空串回落 server 字形）。 */
export function monogramLetter(name: string): string {
  const m = name.match(/[\p{L}\p{N}]/u);
  return m ? m[0].toLocaleUpperCase() : "";
}

function optionOf(p: ProviderPreset): ServiceOption {
  const endpoint = endpointLabel(p.baseUrl);
  return {
    id: p.id,
    name: p.name,
    endpoint,
    kind: p.kind,
    baseUrl: p.baseUrl,
    haystack: `${p.name} ${p.id} ${(p.aliases ?? []).join(" ")} ${endpoint}`.toLowerCase(),
  };
}

export function customOption(): ServiceOption {
  return {
    id: CUSTOM_SERVICE,
    name: "",
    endpoint: "",
    haystack: "custom endpoint 自定义 端点 openai anthropic 兼容",
  };
}

/** 预设选项表（自定义端点不计入——渲染方独立置首）。 */
export function presetOptions(): ServiceOption[] {
  return PROVIDER_PRESETS.map(optionOf);
}

/** 大小写不敏感子串过滤；空 query 全量保留。 */
export function filterOptions<T extends { haystack: string }>(options: readonly T[], query: string): T[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [...options];
  return options.filter((o) => o.haystack.includes(needle));
}

/** 上下文窗口预设档（§3.30 高级展开；点击写 token 原值）。 */
export const WINDOW_PRESETS: ReadonlyArray<{ label: string; tokens: number }> = [
  { label: "32K", tokens: 32_768 },
  { label: "128K", tokens: 131_072 },
  { label: "256K", tokens: 262_144 },
  { label: "512K", tokens: 524_288 },
  { label: "1M", tokens: 1_048_576 },
  { label: "2M", tokens: 2_097_152 },
];

/** 限额徽章短格式（§3.30「1M · 384K」形态）；null/0 = 空串不渲染。
 * 去尾零：1048576 → "1M"（不是 "1.0M"），1536 → "2K"。 */
export function formatLimitShort(n: number | null | undefined): string {
  if (!n) return "";
  const trim = (x: number) => {
    const r = Math.round(x * 10) / 10;
    return r % 1 === 0 ? String(Math.round(r)) : String(r);
  };
  if (n >= 1_000_000) return `${trim(n / 1_000_000)}M`;
  if (n >= 1000) return `${trim(n / 1000)}K`;
  return String(n);
}
