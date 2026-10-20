/**
 * LLM 轨：供应商与模型目录、探测与测试连接、导入向导、推理档位与子代理模型钉。
 *
 * 由批 F-22（v2.175）从 `desktop/src/types.ts` 目录化拆出：**每个类型声明块逐字节原样搬移**，
 * 只改两处——①按引用面决定 `export` 与否（外部 import 过的与兄弟簇要用的留 `export`，
 * 其余降为模块内部）；②跨簇引用走 `import type`。等价判据见 scripts/../tmp/v180-equiv.mjs。
 */

import type { NetworkProxySettings } from "./settings";

export interface LlmSettingsView {
  /** 活动档 id（空串 = 无档位）。 */
  active: string;
  providers: ProviderView[];
  /** 活动档平面速记（模型 chip / 上下文区 / 嵌入提示直读）。 */
  provider: string;
  model: string;
  baseUrl: string | null;
  apiKeySet: boolean;
  /** 模型上下文窗口（token，S2 迁入）；null = 未设置（回落保守保护，
   * Ollama 可经 /api/show 探测，云端查静态档案或手填）。 */
  windowTokens: number | null;
  /** 网络代理档（批 C §8.5）；null = system（吃环境变量）。 */
  networkProxy: NetworkProxySettings | null;
  /** 子代理主模型钉档（批 F）；null = 跟随会话（缺省）。 */
  subagent: SubagentPin | null;
}

/** 部分更新：字段缺省（undefined）= 保持不变；baseUrl / apiKey 传
 * 空串 = 清除；windowTokens 传 null = 清除窗口（回落保守保护）。 */
export interface LlmUpdate {
  /** 切活动档（v2.58）；id 必须已存在。 */
  active?: string;
  provider?: string;
  model?: string;
  baseUrl?: string;
  apiKey?: string;
  windowTokens?: number | null;
  /** 子代理钉档（批 F）；providerId/model 空串 = 清除（v2.65 约定）。 */
  subagent?: SubagentPin;
}

/** provider 档位投影（v2.58 档位制，DESIGN §13.1；无秘钥字段）。 */
export interface ProviderView {
  id: string;
  label: string;
  /** "openai" | "anthropic" | "ollama"。 */
  kind: string;
  baseUrl: string | null;
  apiKeySet: boolean;
  model: string;
  windowTokens: number | null;
  /** ㉚ 停用档：选择器与 active 回落跳过（配置保留）。 */
  enabled: boolean;
  /** 自定义请求头（批 C §8.1；保留键后端拒收，键大小写原样）。 */
  headers: Record<string, string>;
  /** 该档已配置模型清单（模型配置页右栏真源）。 */
  models: ProviderModelView[];
}

/** 档位增改入参（llm_provider_save；id 缺省 = 新建）。apiKey undefined =
 * 保持已存值（更新时），空串 = 清除；models undefined = 保持已存清单，
 * 传数组 = 整表替换（原 model 不在清单时后端改指首项）；headers
 * undefined = 保持已存值，传对象 = 整表替换（保留键后端拒收）。 */
export interface ProviderInput {
  id?: string;
  label: string;
  kind: string;
  baseUrl?: string | null;
  apiKey?: string;
  model: string;
  windowTokens?: number | null;
  enabled?: boolean;
  models?: ProviderModelView[];
  headers?: Record<string, string>;
}

/** 单档模型清单（llm_list_models 返回组；error = 该档探测失败）。
 * 唯一消费方 = `api.ts` 的 `llmListModels` 预留包装（零消费者，裁定同上）。 */
export interface ProviderModels {
  id: string;
  label: string;
  models: string[];
  error: string | null;
}

/** 档位内一个已配置模型（㉚；批 A 起为目录合成视图：钉死值 ∪ models.dev，
 * 目录部分只进内存——保存时 sources 为 catalog 的派生值由前端剥除）。 */
export interface ProviderModelView {
  id: string;
  alias: string | null;
  contextWindow: number | null;
  /** 用户钉死最大输出（token）；null = 跟随目录 / 不发。 */
  maxOutput: number | null;
  maxOutputSource: string | null;
  /** 用户钉死图像输入（tri-state：null = 随目录 / 纯文本缺省）。 */
  supportsImage: boolean | null;
  /** 可用于子代理（批 F：用户钉死；null = 不参与）。 */
  availableForSubagents: boolean | null;
  /** 展示用有效模态（未知 = ["text"]）。 */
  inputModalities: string[];
  /** 归一化推理梯子（空 = 隐藏档位选择器）。 */
  reasoningLadder: string[];
  toolCall: boolean | null;
  pricing: ProviderModelPricing | null;
  /** 后端投影恒带对象；null = 客户端新建行、待目录合并（ModelPanes 哨兵）。 */
  sources: ProviderModelSources | null;
}

/** 每百万 token 单价（USD 原值如实；目录缺分量 = null 不冒充 0）。 */
export interface ProviderModelPricing {
  input: number | null;
  output: number | null;
  cacheRead: number | null;
}

/** 字段来源标记（批 A，供应商稿 §4.5 SourcesView）。 */
export interface ProviderModelSources {
  window: ModelFieldSource;
  maxOutput: ModelFieldSource;
  modalities: ModelFieldSource;
  reasoning: ModelFieldSource;
}

/** 字段来源：`"user"` = 用户钉死（编辑即落 settings，刷新不覆盖）；
 * `"catalog"` = 跟随 models.dev；null = 未知（如实标注）。 */
type ModelFieldSource = "user" | "catalog" | null;

/** 模型能力目录状态（llm_catalog_status；快照 bundled / 在线刷新 refreshed）。 */
export interface CatalogStatus {
  source: "bundled" | "refreshed";
  updatedMs: number;
  providers: number;
  models: number;
}

/** 单模型目录查询结果（llm_lookup_model 命中时）。 */
export interface CatalogModelInfo {
  providerKey: string;
  modelId: string;
  displayName: string | null;
  contextWindow: number | null;
  maxOutput: number | null;
  inputModalities: string[];
  reasoningOptions: string[];
  toolCall: boolean | null;
  pricing: ProviderModelPricing | null;
}

/** 表单探测结果（§8.2）：清单或错误 + 端点扫描自动补全的 base_url
 * （与传入不同时非空，表单字段未再编辑时自动采纳）。 */
export interface ProbeOutcome {
  models: string[];
  error: string | null;
  effectiveBaseUrl: string | null;
  /** 推荐三选（批 F：目录数据驱动 ≤3；空 = 前端回落旧全选行为）。 */
  recommended: string[];
}

/** 逐模型连通测试结果（§8.3；五分类 category + 如实报文）。 */
export interface ModelTestResult {
  ok: boolean;
  category:
    | "auth"
    | "model_not_found"
    | "rate_limit"
    | "network"
    | "server"
    | "unknown"
    | null;
  message: string | null;
  latencyMs: number | null;
}

/** 导入面板候选（批 F；密钥不出后端——hasKey 布尔代替）。 */
export interface ImportCandidate {
  source: "claude-code" | "codex" | "opencode" | "cc-switch";
  id: string;
  label: string;
  kind: string;
  baseUrl: string | null;
  model: string;
  models: ImportCandidateModel[];
  hasKey: boolean;
  notes: string[];
}

interface ImportCandidateModel {
  id: string;
  contextWindow: number | null;
}

export interface ImportScanOutcome {
  candidates: ImportCandidate[];
  errors: string[];
}

export interface ImportApplyReport {
  imported: string[];
  skipped: string[];
}

/** 子代理主模型钉档（批 F）：thinkingLevel null = 跟随会话档位。 */
export interface SubagentPin {
  providerId: string;
  model: string;
  thinkingLevel: string | null;
}

/** embedding 模型区投影（⑬ 智能层共享基建）：无秘钥字段。
 * S4 刀② 从 `types/memory.ts` 迁来——记忆面退役后它只剩工具语义检索
 * （`search_tools`）一个消费者，属本簇。 */
export interface EmbeddingView {
  /** 设置原值；null = 未填（后端按 provider 兜底：ollama → 本机 11434）。 */
  baseUrl: string | null;
  model: string | null;
  apiKeySet: boolean;
}

/** embedding 区部分更新：字段缺省 = 保持不变；传空串 = 清除。 */
export interface EmbeddingUpdate {
  baseUrl?: string;
  model?: string;
  apiKey?: string;
}
