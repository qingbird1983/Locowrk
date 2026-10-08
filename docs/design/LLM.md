# LLM 接入设计（阶段八②参考）

> 版本行：v3.4（2026-10-08）——批 D 多模态地基（ContentPart 消息结构 /
> wire content 数组编码 / AssetStore 内容寻址 / 上传链路 / 双道门控 /
> 估算平铺）。v3.3 批 C 连接与安全（keyring / 自定义请求头 / 端点扫描 /
> 逐模型测试 / 网络代理）。v3.2 批 B 推理档位（方言即数据）+ 输出预检钳
> 制（D5 窄修订）+ G1 length 闸；v3.1 批 A 目录；v3.0（2026-10-05）
> **方案一（genai 统一库）退役**，改为自建 OpenAI 兼容统一客户端。v2.x =
> genai 三合一（历史见 CHANGELOG）。

## 核心理念：一个接口，一条线格式

内核只与统一的 `LlmService` 接口交互（`LlmRequest`/`LlmResponse` 中立投影，
消息复用 session-store 投影），模型适配层不特殊、也只是服务（§7.2）。

**线上协议只有一种：OpenAI Chat Completions（`POST {base}/chat/completions`
+ SSE）。** 不再按 provider 分派原生协议适配器——业界"几大兼容接口"事实上
已收敛到 OpenAI 形态：

| provider kind | 鉴权 | 端点归一 | 覆盖 |
| :--- | :--- | :--- | :--- |
| `openai` | `Authorization: Bearer` | `join_version`（缺版本段补 `/v1`，已带 `/v<数字>` 原样——智谱 /v4、方舟 /v3） | OpenAI、DeepSeek、智谱、Z.AI、Kimi、MiniMax、硅基流动、火山方舟、通义（compatible-mode）、OpenRouter、Groq、xAI、Mistral、Together、Fireworks、NVIDIA、Baseten、Cerebras、HuggingFace、vLLM 等自建网关 |
| `anthropic` | `x-api-key` + `anthropic-version` | 同上（走官方 OpenAI 兼容层 `/v1/chat/completions`） | Anthropic |
| `ollama` | 无 | 同上（`{base}/v1/chat/completions`） | Ollama / LM Studio 等本地 |

kind 只决定**鉴权头与端点归一**，不再决定请求/响应形状。设置面板 22 个预设
全部由此一条线覆盖；自定义端点（CUSTOM_SERVICE）同样。

## 请求装配

- `messages`：中立投影直转——user / assistant（正文 + `tool_calls` 数组）/
  `role:"tool"`（`tool_call_id` + 内容；协议没有独立错误位，错误作为 JSON
  内容回传，模型可自行处理）。
- `system`：请求级系统提示 → `messages` 首条 `role:"system"`（⑪ 记忆注入走
  这里）。
- `tools`：OpenAI 风格 `{type:"function", function:{name, description,
  parameters}}`；空目录不携带字段。
- `temperature`：仅当决策环 params 显式携带才发。
- **推理档位（批 B，方言即数据）**：`params["thinking_level"]`（语义档）→
  `llm/dialect.rs` 按 baseUrl 定位预设方言写字段——openai/anthropic =
  `reasoning_effort`、GLM = `thinking:{type}`、Qwen = `enable_thinking`、
  DeepSeek = `thinking:{type}` + `reasoning_effort` 双字段（官方归一
  minimal→low、medium/high→high、xhigh/ultra→max）。映射不了的档位不发。
  档位来源 = 会话旁车（composer 循环钮，clamp 就近降档/剥除）或辅助调用
  标记（`params["aux"]=true` → 方言最低档，title/handoff/extract/记录员）。
  无方言预设（openrouter 等）不发任何字段。
- **max_tokens（D5 窄修订，v2.75「不发派生限额」的窄开口）**：仅当用户在
  模型行钉死「最大输出」才发送，且发送前预检钳制
  `min(钉死, ctx − estimate_tokens − 1000)`（下限 1024，估用吃满不钳负——
  留给上下文溢出语义）；目录值只展示永不发送；400 报文点名 max_tokens 时
  剥字段重发一次（stream_options 同款先例）。
- 缺 Key 先行报错（友好文案指引设置页）；openai/anthropic 保留环境变量逃生舱
  （`OPENAI_API_KEY`/`ANTHROPIC_API_KEY`）。

## 流式（SSE）

- 解析：`data:` 行为单位，多行 data 拼接，`[DONE]` 终止；keep-alive/注释行
  跳过。**唯一线路**：`complete` 非流式 = 内部同一条流式管道聚合（一套线上
  代码，无第二形状）。
- 正文：`choices[].delta.content` 逐段回调。
- 推理：`choices[].delta.reasoning_content`（DeepSeek/Qwen/GLM 等事实标准）
  逐段回调 `on_reasoning`，不进正文。
- 工具调用：`delta.tool_calls[]` 按 `index` 累积（id / function.name /
  arguments 分片拼接）；空 call_id 按序补号（本地模型常见）。
- 用量：请求带 `stream_options:{include_usage:true}`（不识别的网关忽略即可；
  400 且 body 指名 stream_options 时修复性重试一次去掉该字段）。映射
  `LlmUsage{input, output, cache_read, cache_write, reasoning, total}`：
  `prompt_tokens`/`completion_tokens`/`total_tokens`；缓存读 =
  `prompt_tokens_details.cached_tokens` 或 DeepSeek
  `prompt_cache_hit_tokens`；缓存写 = `cache_creation_tokens`（Anthropic 形
  态）有则取；推理 = `completion_tokens_details.reasoning_tokens`。全 0 =
  None（宁缺勿假）。
- 停止生成（§3.39④）：回调返回 `true` = 用户请求停止——立即停止拉流，以已
  累积的部分正文原样收束（Ok、无工具调用、无用量，不虚构结尾）。
- 流意外结束（连接断 / 无 `[DONE]` / 空收束）：按可重试的流失败处理（见下），
  不当成功。
- **G1 length 闸（批 B，可靠性稿 §13）**：`finish_reason=length` = 输出在
  max_tokens 处被截断——按 Fatal 如实报错（本轮未完成收束），部分
  tool_calls 一律不执行，截断消息不入史；文案指引调大「最大输出」。

## 传输韧性（PI-Desktop 同款，掐断问题的根治）

目标：**瞬时故障对用户不可见**——限流、网关抖动、断流自动重试到成功为止；
只有密钥错误这类真问题才报错。重试在适配器内（覆盖聊天/记录员/标题/提取/
交接全部调用点），内核决策环（`agent/request-error`）留给插件。

错误分类：

| 信号 | 分类 | 处置 |
| :--- | :--- | :--- |
| 401 / 403 | 鉴权失败 | **立即报错**（重试无意义） |
| 429 | 供应商限流 | 重试，退避见下 |
| 408 / 5xx / 网络错误 / 超时 / 断流 | 瞬时故障 | 重试，退避见下 |
| 400 / 413 且报文含上下文超限语义 | 上下文超限 | 如实报错（压缩由阈值机制提前预防） |
| 其他 4xx | 请求错误 | 如实报错 |

退避（**重试不设次数上限**——用户裁定"半次失败都不可接受"：瞬时故障一直
重试到成功，或用户点停止；单次等待始终封顶，不存在无限等待）：

- 限流：2s 起步 ×2 指数 + 25% 抖动，单次封顶 30s；**服务器 `Retry-After`/
  `retry-after-ms` 优先**（ms → 秒 → HTTP-date 次序），同样封顶 30s。
- 瞬时：1s/2s/4s/8s 翻倍后维持 8s；服务器声明延迟同样优先。
- **流空闲看门狗：180s 零事件** = 视为断流（可重试），防止挂在半死的连接上
  永远等；正常的长输出只要持续吐字就不限总时长。
- 重试期间用户点停止：立即中止重试，以已有部分正文收束。

**流式重置**：已吐给前端的 delta 不因重试自动消失——重试前发
`agent://llm-retry`（sessionId/attempt/延迟），前端清空本步未完成的流式缓冲
并显示"自动重试中"状态行；重试轮的 delta 从零重建同一气泡。重放路径不受影
响（投影只认返回值整段）。非流式调用点（记录员/标题/提取/交接）重试天然不
可见，无需重置。

## 用量与缓存口径

沿用 v2.58：`LlmUsage` 为供应商报告的真实计数；命中率分母 = input（含缓存
读，M1 前缀缓存纪律同源）。重试轮的用量只计成功那次（失败请求供应商不计量）。

## 模型能力目录（批 A，v3.1 增补）

**能力数据的燃料 = models.dev 快照**（供应商稿 v2.0 §4；D1 裁定：快照路线）。

- **三源两优先级**：随包快照（`desktop/src-tauri/resources/models.dev/api.json`
  ≈5.3MB，`bundle.resources` 打包，惰性加载不进启动路径）→ 在线刷新
  （设置页按钮，10s 超时，**只进内存永不落盘**）；用户钉死值（settings
  `llm.providers[].models[]` 的 `maxOutput`/`supportsImage`/`contextWindow`）
  恒最高。合并优先级：用户钉死 > 在线刷新 > 内置快照 > 缺省（未知如实）。
- **匹配**（`llm/catalog_match.rs`，PI 打分制无正则）：预设 host 别名表
  +100 → 快照 `api` host 精确/后缀 +80 → 归一化模型 id 精确命中（剥
  `-latest`/日期戳/`@region`/路径前缀）。**无供应商定位 = 无匹配**——中转站
  跨发布商"借池"不做（记档），宁缺勿假。别名表 × 随包快照有守卫测试
  （models.dev 改名即红）。
- **合成投影**（`settings/llm.rs::synthesize_view`）：IPC 的
  `ProviderModelView` = settings 钉死面 ∪ 目录（`sources` 逐字段标记
  user/catalog/None）；settings 只存钉死值，**目录数据永不落 settings**——
  前端保存时剥除 sources=catalog 的派生值（ProviderSetupDialog.save）。
- **命令三件**：`llm_catalog_status` / `llm_catalog_refresh` /
  `llm_lookup_model`（自定义模型添加时就地升级）。
- **梯子归一化**：目录 `reasoning_options` → 语义档（disabled/minimal/low/
  medium/high/xhigh/max/enabled；toggle+effort 并存时 enabled 冗余不出现
  ——deepseek 梯子 = disabled/low/high/max；toggle 独占才有 enabled——GLM）。
  批 A 只投影展示，批 B 接预设方言后成为档位选择器的数据源。
- **§5.5 思考回传复测结论（2026-10-08 实机）**：DeepSeek 官方文档要求带
  tools 时回传历史 `reasoning_content`（否则 400）——deepseek-flash 真轮
  8 次工具调用不回传**零 400**，义务未被供应商执行，**维持现状不实现回传**；
  若将来收紧，配对来源现成（Thinking 事件紧贴 AssistantMessage），PI 占位
  回退 `[reasoning not retained]` 记档。
- **价格**：目录 USD 原值如实展示（选择器行缀 / 详情行），只作参考不进任何
  计算；与会话累计估算（`chat.cacheHitPrice/cacheMissPrice`，CNY 手填）两套
  口径并存互不替换。

## 连接与安全（批 C，v3.3 增补）

**keyring（§8.4）**：`keyring` crate（service=`"Locowrk"`，account=provider
id；Windows = Credential Manager）。`ProviderProfile +key_storage`（缺省缺
字段 = 文件存储，旧文件零迁移）：保存非空 Key → `keyring.set` 成功后
`api_key=None + key_storage="keyring"`；失败回落文件存储 + warn（便携环境 /
无凭据库，如实记档）。读取双路：存量明文优先，否则标记在位走 `keyring.get`。
settings 加载时**惰性迁移**（明文 → 凭据库 → 立即落盘换标记，幂等）。IPC 面
不变——api_key 永不过线，只露 `api_key_set` 布尔（keyring 在位同样点亮）。

**自定义请求头（§8.1）**：`ProviderProfile +headers`（BTreeMap，serde
default）。保存时校验：≤32 条、键为合法 token（可见 ASCII 无冒号）、值无控
制字符；**保留键拒收**（authorization / x-api-key / api-key /
proxy-authorization / host / content-type / content-length / cookie /
set-cookie / connection / transfer-encoding / te / trailer / upgrade——鉴权
与逐跳头不可碰）。注入 = `client.rs` 构造 HeaderMap：鉴权头先入、用户头
**最后写入者覆盖**（可覆盖 anthropic-version / User-Agent，保留键在保存侧
已挡死）。

**端点扫描（§8.2，PI 同款补中转站）**：`llm_probe_models` 失败时对自定义
base_url 候选扫描：现值（join_version 归一）→ `{origin}/v1` →
`{origin}/api/v1` → `{origin}/api`（≤4 去重，每个 3s 超时；无 scheme 的裸
域名补全定位——本地形态 localhost/`127.*`/`::1`/无点主机补 http://，其余补
https://）。胜出候选随结果回报 `effective_base_url`，表单在
字段未再编辑时自动采纳 + 提示。**选择器路径（llm_list_models）不扫描**——
那里无处采纳，扫描成功反而谎报（聊天仍走原端点必败），宁缺勿假。
候选只变路径不换 host（凭据不发去陌生主机）。

**逐模型连通测试（§8.3）**：`llm_test_model {profile_id, model}` 最小请求
（单条 user "ping"，非流式、无工具、无 max_tokens、15s 总超时），返回延迟
ms 或五分类错误：`auth`(401/403) / `model_not_found`(404 带模型名与报文) /
`rate_limit`(429) / `network`(超时/DNS/连接) / `server`(5xx)；其余 unknown
带截断报文。`llm_probe_models` 保留"拉列表"语义，两者并存。

**网络代理（§8.5，裁定 D6）**：`llm +network_proxy {mode, url}`
（system = reqwest 缺省吃环境变量 / direct = 强制直连 / custom = 指定
url）。HTTP 客户端逐请求新建（现状），代理随 cfg 读取——设置保存即对下一次
请求生效，天然热换。probe / 扫描 / 测试同吃。动机：本机常驻 clash 类代理时
显式可控（v2.45 走查记录）。

## 多模态输入侧（批 D，v3.4 增补）

**消息结构**：`session-store/src/content.rs` 新 `ContentPart`（`text` /
`image` / `file` 三变体；`AssetRef{id, mime, name}` 引用内容寻址存储；
ToolUse/ToolResult **不进** ContentPart——工具协议维持内核中立事件，修订
摘要 #1 裁定）。`Message::User` / `SessionEvent::UserMessage` 各
`+parts: Vec<ContentPart>`（serde default，旧 jsonl/旧快照缺字段 = 纯文本
零迁移；快照 schema_version 1→2 带 v1→v2 upcaster）。内存路径一次改对，
落盘增量演进；v2.47 双气泡重放回归锁老字节。

**wire 编码（一处完成）**：`Message::User.parts` 为空 = 现状字符串 content
（**字节前缀与现网完全一致，M1 前缀缓存零扰动**——空 parts 是常态路径）；
非空 = content 数组，`{"type":"text","text":content}` 正文在前（稳定头）+
`image_url`（data URL base64）按 parts 序 + `file`（`{filename, file_data}`
PDF）。asset 字节从 AssetStore 读（AppLlm 组装点注入），data URL 带 LRU
缓存（同图同字节 = 前缀稳定，纪律 3 追加式变更）。读失败 = Fatal 如实报错。

**双道门控**：①上传时（前端，`ProviderModelView.supportsImage`/
`inputModalities`）提示卡：切换建议 / 仅发文字；②发送前
（`wire::check_modalities`）：请求带图片 part 且生效模型三态解析为
`Some(false)`（用户钉死 = 否，或钉死缺席时目录 `input_modalities` 明确无
image）→ Fatal 拒发带指引；**未知（None）放行**——目录没有的能力不发明。
「视觉备用自动切换」不做（记档：避免静默换模型烧钱）。

**AssetStore（内容寻址）**：`<数据根>/assets/{images,files}/<sha256>.<ext>`，
消息只存 `AssetRef`。`asset_put`（base64 + mime + name）上传即落盘：校验 =
格式白名单（png/jpg/jpeg/webp/gif + pdf）+ 体积 ≤ maxSizeMb（缺省 5）+
图片单边 ≤ maxDimension（缺省 2048px；**超限拒绝不转码**——图片转码记档
不做，阶段计划批 D「明确不做」）；`asset_read`（缩略图渲染，IPC base64）；
`asset_gc`（手动触发）：引用面三处全扫（sessions/*.jsonl +
sessions/*.snapshot + sub-transcripts/*.jsonl，sha256 hex 子串粗筛）→ 孤儿
且 mtime 超 gcDays（缺省 7）→ 删除；autoGc 开时启动 + 每 24h 后台各跑一遍。
settings `multimodal` 区（字段级缺省零迁移），存储位置不开放配置（走查隔
离依赖 LOCOWRK_DATA_DIR）。

**估算与钳制联动**：`estimate_tokens` 对 parts 平铺——Image 1024（锚点 =
DeepSeek 官方单图 token 上限）+ File 1000；图片只进估算不进真实计数
（LlmUsage 供应商自报不动）。估算推高 → ctx-compact 更早触发（摘要器不吃
parts——转录投影剥 parts 只留正文）+ maxOutput 预检钳制自动收窄输出预算
（同一把尺）。压缩不丢图：摘要后图片仍保留（§10.1 宁可如实溢出不静默丢）。

**记忆与转录**：extract 转录 / 记忆树材料 / 标题 / handoff 全部只吃正文文
本——parts 在转录里折叠为 `[图片: name]` 占位（记忆只记文本事实）；导出
Markdown 同款占位行。工具结果图片投影到下一条 user parts 属批 E（§8.1）。
