# Locowrk 多模态模型支持设计

> 配套项目：Locowrk 版本：**2.0**（v1.0 用户初稿 + PI/ZCode 双家取参修订；修订摘要见 §0） 前置文档：《Agent 内核设计》《LLM 客户端与调度层设计》（[LLM.md](http://LLM.md) v3.0）、《模型供应商配置与能力探测设计 v2.0》（能力数据上游） 目标：从"纯文本"升级为"**输入侧多模态**"——发图片/文件给模型 参照系：PI-Desktop（模态门控 / 能力投影）、ZCode（发送前门控 / 媒体投影 projectMessagesForInputFormat / 预检钳制联动）

---

## 0\. v2.0 修订摘要（相对 v1.0 初稿）

| # | v1.0 初稿 | v2.0 裁定 | 理由 |
|---|---|---|---|
| 1 | ContentPart 含 ToolUse / ToolResult（Anthropic 式 content 内嵌工具） | 退役。ContentPart 收敛为 Text/Image/Audio/File 四种；工具协议维持内核中立事件（Message::Assistant{tool_calls} + Message::ToolResult） | 单线 chat completions 里 tool_calls 本就是独立字段；内嵌式要把整个工具循环重写一遍，纯成本零收益 |
| 2 | 工具结果图片直接进 ToolResult.content | 投影策略：工具图片 → asset 存储 + 工具文本留 \[图片: asset-id\] 占位；请求组装时把图片 part 挪进下一条 user 消息（挪不动 = 文本占位） | chat completions 的 tool 角色消息官方只吃文本；ZCode 的媒体投影是同思路（projectMessagesForInputFormat） |
| 3 | 模型输出图片 ✅ 第一版 | 降级记档。第一版 = 输入侧多模态 | 单线下主流聊天模型不回图片（图像生成走独立 API，Gemini 原生才回 inline_data）；与 v1.0 自己的目标句"发图片给模型"一致 |
| 4 | 「一次全改，不做两套并存」 | 限定内存路径。落盘格式必须 serde(default) 增量演进：SessionEvent::UserMessage +parts、Message::User +parts，老 jsonl 缺字段 = 纯文本；快照走 schema_version | 家法：Thinking/LlmUsage/ContextUsage 三先例全是"旧日志无此变体 serde 天然兼容"；重写格式 = 毁重放 |
| 5 | \~/.locowrk/assets | <数据根>/assets/（LOCOWRK_DATA_DIR 纪律）；GC 引用扫描面 = sessions jsonl + 快照 + sub-transcripts 三处 | 走查隔离依赖数据根可重定向；v1.0 只扫 session 漏了快照与子转录 |
| 6 | 四套 Provider 编码器（ProviderAdapter trait） | 退役。单线兼容层方案：parts → content 数组 image_url（data URL），一处编码 | 修订摘要 #3 同源；Anthropic/Google/Ollama 官方兼容层都吃 image_url |
| 7 | 图像 detail 三档用户旋钮（§7.3/§11） | P2 记档。v1 恒 auto（兼容层大多忽略 detail；估算按平铺值） | 先跑通主链路，旋钮随批 E 评估 |
| 8 | — | 新增：穿透面四处补全（jsonl 重放/快照/fork+rewind+undo/树偏移投影）、token 估算与 maxOutput 预检钳制联动、上传时+发送前双道门控落点 | v1.0 影响面表漏项 |
| 9 | （v1.0 时点 DeepSeek 尚无公开视觉能力） | DeepSeek 官方确认 deepseek-flash 视觉走**同一 chat completions 兼容层**（api-docs/guides/vision）——§5.1 矩阵、估算锚点 1024、detail 支撑、上传限制已按官方文档更新 | D2「输入侧单线可通」从推断升级为厂商实证 |

---

## 一、设计目标

| 目标 | 含义 |
|---|---|
| 消息结构支持多模态 | 用户消息 content: String → 增加可选 parts: Vec<ContentPart> |
| 单线适配 | 只走 OpenAI 兼容层（v2.75 纪律），图片编码一处完成 |
| 存储策略清晰 | 内容寻址 asset 存储，消息里只存引用 |
| 前端可交互 | 粘贴、拖拽、选择文件、预览、渲染、查看大图 |
| 工具返回多模态 | 截图/图表等工具产物能进入对话流（投影策略） |
| 不破坏前缀缓存 | 内容寻址 → 同图每轮编码出相同 base64，字节前缀稳定 |
| 降级可用 | 模型不支持某模态：上传时提示 + 发送前拦截，优雅处理 |

### 非目标（第一版）

* 不做模型输出图片（图像生成另批，见修订摘要 #3）
* 不做实时语音对话（音频文件上传属 P2）
* 不做视频
* 不做图像编辑

---

## 二、现在的问题（影响面 v2.0 补全）

### 2.1 消息结构太窄

```rust
// crates/session-store/src/session.rs:26
pub enum Message {
    User { content: String },          // ← 只能装文本
    Assistant { content: String, tool_calls: Vec<ToolCallMsg> },
    ToolResult { call_id, tool, args, output: Value, error: Option<String> },
}
```

内核 `LlmRequest.messages` 直接复用这份中立投影（kernel-agent [services.rs:21](http://services.rs:21)），桌面 [wire.rs](http://wire.rs) 再映射 OpenAI 格式——**改这一处，全链路跟着走**，这是好消息。

### 2.2 穿透面清单（v2.0 修订：比 v1.0 §2.2 多四处，全部必须过）

| 层 | 现状 | 改动 |
|---|---|---|
| 落盘事件 | SessionEvent::UserMessage{content}（event.rs:24，jsonl 一行一事件） | +parts: Vec<ContentPart>（serde default） |
| 中立投影 | Message::User{content} | +parts（serde default）；老快照/老 jsonl 缺字段 = 纯文本 |
| 快照 | SessionState{messages} 带 schema_version（§9.5） | schema_version 递增；读旧快照补空 parts |
| fork / rewind / undo | Session::fork(cursor())（子代理前缀克隆）、rewind_to+fork+.undo.bak 重写 | parts 随消息自然携带，逐一过测试 |
| 树偏移投影 | TreeContextSource::derive_messages_from（带偏移，M4） | 切点语义不变（User 之前）；parts 随消息走 |
| 内核估算 | kernel_agent::estimate_tokens（ContextUsage 数据源） | 图片平铺估算（§7） |
| 压缩 | ctx-compact 读文本摘要 | parts 不参与摘要；图片只进估算（§10） |
| 桌面 wire | wire.rs 消息映射 | parts → content 数组编码（§5） |
| 投影事件 | agent://user-message 等 | UserMessage 投影带 parts（前端渲染） |
| 前端 | MessageRow / TranscriptFlow / chat store | 图片缩略图渲染 + 上传链路（§9） |
| 记忆 | extract 转录构建 | parts 折叠为 \[图片\] 占位文本进转录（记忆只记文本事实） |

---

## 三、消息结构重构

### 3.1 新结构

```rust
// crates/session-store/src/content.rs（新）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    Image { asset: AssetRef },
    File  { asset: AssetRef },                    // 第一版只收 PDF
    Audio { asset: AssetRef },                    // P2
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetRef {
    pub id: String,        // sha256 hex
    pub mime: String,
    pub name: String,      // 原文件名（展示用）
}

// session.rs：消息增字段（serde default = 老 jsonl/快照零迁移）
User {
    content: String,
    #[serde(default)]
    pub parts: Vec<ContentPart>,   // 空 = 纯文本消息（content 为正文）
}
```

**明确不进 ContentPart 的（v2.0 裁定）**：

* `ToolUse` / `ToolResult` —— 工具协议维持内核中立事件，wire 层 OpenAI tool_calls 形状不变（修订摘要 #1）
* `detail` 字段 —— P2 记档（修订摘要 #7），v1 恒 auto
* URL 引用形态 —— v1 只有 Asset 引用（用户上传与工具产物都先落 asset 存储）；`Base64`/`Path`/`Url` 内联形态不设（宁缺勿假，引用统一走 AssetRef）

### 3.2 兼容性（内存一次改对，落盘增量演进）

* **内存路径一次全改**：`Message::text()` 便捷构造保留；`user_with_parts()` 新增；调用方逐步过
* **落盘/重放必须增量**：老 jsonl 的 `UserMessage{content}` 读入 = `parts: \[\]`；新事件多一个字段老代码照读（serde default）；快照 schema_version 递增
* **回归底线**：v2.47 的「重启后历史会话双气泡完整恢复」测试必须继续绿（老会话逐字节不动的重放）

### 3.3 便捷构造

```rust
impl Message {
    pub fn text(role_text: &str, content: impl Into<String>) -> Self { /* … */ }
    pub fn user_with_parts(text: String, parts: Vec<ContentPart>) -> Self { /* … */ }
}
```

---

## 四、支持的内容类型

### 4.1 第一版（输入侧）

| 类型 | 输入 | 输出 | 说明 |
|---|---|---|---|
| 文本 | ✅ | ✅ | 已有 |
| 图片 | ✅ png/jpg/webp/gif | ❌ | 核心场景，data URL 编码 |
| 文件 | ✅ PDF | ❌ | 兼容层映射（Anthropic 官方兼容层支持 document；Google 兼容层支持；不支持的中转站由门控拦截） |
| 工具调用/结果 | ✅ | ✅ | 已有；工具产物图片走投影（§8） |

### 4.2 第二版（P2）

| 类型 | 说明 |
|---|---|
| 音频 | 语音文件上传（mp3/wav/m4a）；音频输入在 chat completions 兼容层覆盖差，按预设/目录能力门控 |
| detail 旋钮 / 图像输出 | 图像生成另批；detail 随批 E 评估 |

---

## 五、Provider 适配（单线方案）

### 5.1 各家兼容层对 `image_url` 的支持（全部走现有 kind 路由，**零新增线**）

| 供应商（kind） | 兼容端点 | 图片输入 | 备注 |
|---|---|---|---|
| OpenAI（openai） | 官方 | ✅ content 数组 image_url（data URL） | 原生形态 |
| Anthropic（anthropic） | 官方 OpenAI 兼容层 | ✅ image_url（base64 data URL） | 兼容层映射为 source.base64 |
| Google（openai） | …/v1beta/openai/ | ✅ image_url | 兼容层映射 inline_data |
| Ollama（ollama） | /v1 兼容端点 | ✅ image_url | 视觉模型（llava/qwen2-vl 等） |
| DeepSeek（openai） | 官方（同 base_url） | ✅ **deepseek-flash**（视觉）；deepseek-v4-pro 纯文本 | 官方即走 chat completions content 数组（api-docs/guides/vision，2026-10 核；deepseek-chat/reasoner 旧名已停用，能力按现役模型归档）；**图片仅限 user 消息**——system/assistant/tool 携带 → 400，故 §8.1 投影策略是硬要求而非优化 |

### 5.2 编码落点（一处完成）

```rust
// desktop/src-tauri/src/llm/wire.rs 消息映射扩展
// Message::User { content, parts }：
//   parts 为空 → 现状字符串 content（字节前缀与现网完全一致，M1 缓存零扰动）
//   parts 非空 → content = [
//     {"type":"text","text": content},                       // 正文在前（稳定前缀）
//     {"type":"image_url","image_url":{"url":"data:mime;base64,…"}},  // 按 parts 序
//     {"type":"file","file":{…}} / 兼容层语义映射
//   ]
// asset 字节从 AssetStore 读（桌面适配器持有数据根），加载带内存 LRU
```

**前缀缓存纪律自查**：同一图片 = 同一 asset = 同一 base64 字节 → 前缀稳定；图片只追加在消息尾部 → 追加式变更，符合纪律 3（稳定头 + 追加清单）。

### 5.3 能力检测与降级（双道门控）

| 道 | 时机 | 落点 | 行为 |
|---|---|---|---|
| 上传检查 | 用户贴图/选文件时 | 前端（chat store 吃 ProviderModelView.input_modalities） | 当前模型不支持 → 提示卡（§9.6）：切换建议 / 仅发送文字 |
| 发送拦截 | 组装 payload 后 | wire.rs（目录 ∪ 用户钉死模态） | UnsupportedModality 错误 + hint（供应商设计 v2.0 §7） |

默认策略 = **拒绝发送 + 提示切换**（v1.0 §5.5 保持）；「视觉备用模型」自动切换不做（记档，避免静默换模型烧钱）。

---

## 六、存储策略（Asset 存储）

### 6.1 内容寻址

```
<数据根>/assets/
├── images/<sha256>.png
├── files/<sha256>.pdf
└── audio/<sha256>.mp3        （P2）
```

* **Asset ID = 内容 sha256**：天然去重；消息只存 `AssetRef{id,mime,name}`
* 上传即落盘（`asset_put` 命令：bytes+mime → id），发送时由 wire 层读取编码
* **上传时即时校验**：尺寸上限（默认 2048px，超出缩边）、体积上限（默认 5MB，超出拒绝）、格式白名单（png/jpg/jpeg/webp/gif + pdf）

### 6.2 生命周期

| 时机 | 行为 |
|---|---|
| 上传/工具产出 | hash → 存入 → 返回 AssetRef |
| 会话中 | 消息只存引用 |
| 发送 | AssetStore 读 → wire 编码（内存 LRU） |
| 会话删除 | 引用扫描（GC 兜底，不即时删） |
| GC | 每日后台 + 启动：扫描引用面 → 孤儿且超 7 天 → 删除 |

### 6.3 GC 引用扫描面（v2.0 修订：三处全扫）

1. `<数据根>/sessions/\*.jsonl`（含 parts 里的 AssetRef）
2. `<数据根>/sessions/\*.snapshot`（快照里的 SessionState.messages）
3. `<数据根>/sub-transcripts/\*.jsonl`（子代理转录）

扫描为全量文本粗筛（sha256 hex 子串匹配）+ 命中者豁免——不做精确索引（成本 > 收益，7 天宽限兜底误删）。

---

## 七、Token 成本

### 7.1 估算（进 kernel `estimate_tokens`，ContextUsage/预检共用）

```rust
ContentPart::Image => 1024,  // 平铺粗估，锚点=DeepSeek 官方单图 token 上限 1024；真实计数以 LlmUsage 为准——宁缺勿假原则不变
ContentPart::File  => 1000,  // PDF 粗估
ContentPart::Audio => 500,   // P2
```

* 图片只进估算不进真实计数（供应商 usage 自报，现有 LlmUsage 六字段不动）
* 估算变大 → 上下文进度环/阈值更早触发压缩 = 正确行为

### 7.2 与 maxOutput 预检钳制联动（ZCode 公式）

```
max_tokens = min(用户钉死值, ctx_window − estimate_tokens(req) − 1000)
```

图片推高 estimate → 钳制自动收窄输出预算，防"图大 + 输出顶格"双挤爆。落点见供应商设计 v2.0 §6。

### 7.3 detail 旋钮 —— P2 记档（修订摘要 #7）

DeepSeek 官方原生支持 `detail: low | high | original`（auto=original；low 缩到 512² 省 token），OpenAI 同名字段——P2 落旋钮时有双家原生支撑，兼容层接受度风险低。

**传图方式记档**：DeepSeek 另支持外部图片 URL（60s 下载窗）与 Files API file_id（单图 64MiB）——Locowrk v1 恒走 base64 data URL（本地文件为主；URL 有时效窗、file_id 需独立 Files API 线），两者记档不做。上传校验按官方限制对齐：JPEG/PNG/GIF/WebP、单图 ≤32MiB、请求体 ≤48MiB、单边 ≤8192px（本地上限 5MB/2048px 远严于官方，留兼容层余量）。

---

## 八、与工具系统的集成

### 8.1 工具结果图片的投影策略（v2.0 核心裁定）

```
工具返回图片（如 screenshot）：
  1. 工具侧：bytes → asset_put → ToolResult.output 带 {"asset": {id,mime,name}, "text": …}
     （ToolResult 结构不动，output: Value 里携带——协议零破坏）
  2. 转录 UI：直接从 output.asset 渲染预览（不依赖 parts）
  3. 请求组装（桌面 wire 层）：ToolResult 中的图片 part
     → 挪到下一条 user 消息的 parts 尾部（ZCode 媒体投影同思路）
     → 工具消息文本留 "[图片已附于下条消息: name]" 占位
  4. 挪不动（无下一条 user / 模型不支持）→ 保持文本占位 + asset 引用号
```

**kernel 零改动**：投影发生在桌面 [wire.rs](http://wire.rs) 消息映射层；内核 estimate 对被挪图片的估算缺口记档（方向 = 高估风险低，接受）。

### 8.2 多模态工具（P1，随批 E）

| 工具 | 返回 | SideEffect |
|---|---|---|
| screenshot | Image | Pure（截自己屏幕） |
| read_image | Image（工作区文件 → asset） | Pure |
| pdf_to_images | Vec![]() | Pure |

### 8.3 工具产物的渲染

```
⚙ screenshot  ✓ 成功
   [截图预览缩略]
   [查看大图] [保存到工作区]
```

---

## 九、UI/UX

### 9.1 输入（扩展现有 `AttachButton.vue` v1，不另起组件）

| 方式 | 交互 |
|---|---|
| 粘贴 | Ctrl+V 图片/文件 → 预览条 |
| 拖拽 | 拖入 composer → 预览条 |
| 选择文件 | 附件按钮（现 pick_files + @路径 语义保留，图片/PDF 走新链路） |
| 截图 | P1（随 screenshot 工具批） |

### 9.2 附件预览条

```
┌──────────────────────────────────────┐
│  [🖼 img1.png ×] [📄 doc.pdf ×]      │ ← 上传中=进度、失败=红框可重试
│  输入消息...                          │
│  [+] [📷]              [deepseek ▾] [↑] │
└──────────────────────────────────────┘
```

### 9.3 消息渲染

* 用户消息：正文 + parts 缩略图（`asset://` 协议或 convertFileSrc；缩略最大边 320）
* 点击缩略图 → 查看大图（批 E：查看器含左右切换/下载/Esc；批 D 先用系统打开器顶位）
* 老会话重放：无 parts = 现状渲染，逐字节不变

### 9.4 模型选择器能力徽章

```
[deepseek-chat ▾]
  ● deepseek-v4-pro      📝
  ● deepseek-flash       📝 👁
  ○ claude-sonnet        📝 👁 📄
```

* 徽章数据 = `ProviderModelView.input_modalities`（批 A 投影）
* 推理档徽章（✨）与档位循环钮见供应商设计 §5.3

### 9.5 上传时门控提示（§5.3 第一道）

```
⚠️ 当前模型不支持图片
deepseek-v4-pro 只能处理文本。可以：
  [切换到 deepseek-flash]  ← 同供应商视觉模型优先建议
  [打开模型选择器]
  [仅发送文字部分]
```

### 9.6 发送失败如实呈现

`UnsupportedModality` 错误条：文案带 hint + 「打开模型选择器」动作（复用 ChatSurface 错误条模式）。

---

## 十、上下文压缩与多模态

### 10.1 第一版（批 D）：图片不参与压缩，只参与估算

* ctx-compact 摘要只吃文本（parts 不进摘要器）
* 图片推高 estimate → 压缩更早触发 → 摘要后**图片仍保留**（窗口仍不够时靠 maxOutput 钳制 + 用户清上下文）——如实呈现，不做静默丢图

### 10.2 第二版（批 E，P2）：挂捆边界的降级链

| 策略 | 说明 |
|---|---|
| 保留原图 | 最近的图片 |
| 降 detail | P2（detail 旋钮同批） |
| 替换为描述 | 视觉模型生成一句话描述替代原图（辅助调用降档语义复用） |
| 丢弃 | 捆边界之外最旧的 |

与记忆树捆边界对齐：捆与捆之间全部降级为文本描述。**前缀缓存注意**：降级/替换改动历史字节 = 计划内缓存失效，与 ctx-compact 稳定期同款语义（接受一次、随整理边界走）。

---

## 十一、配置项（settings.json `multimodal` 区，字段级缺省零迁移）

```jsonc
{
  "multimodal": {
    "maxDimension": 2048,        // px，超出缩边
    "maxSizeMb": 5,              // 上传体积上限
    "imageFormats": ["png","jpg","jpeg","webp","gif"],
    "fileFormats": ["pdf"],
    "autoGc": true,              // 孤儿 asset 清理
    "gcThresholdDays": 7
  }
}
```

> v1.0 §11.2 的 TOML 形态退役——家法是 settings.json；存储位置不开放配置（固定数据根，走查隔离依赖）。

---

## 十二、落地路径（映射到阶段计划）

| 阶段（本稿） | 内容 | 批次 | 优先级 |
|---|---|---|---|
| 1 | ContentPart + Message/SessionEvent parts（serde default）+ 快照版本 | D | P0 |
| 2 | wire.rs parts 编码（单线）+ 发送前门控 | D | P0 |
| 3 | AssetStore + GC（三扫描面）+ asset_put 命令 | D | P0 |
| 4 | 输入链路（粘贴/拖拽/选择 + 预览条 + 上传门控） | D | P0 |
| 5 | 消息渲染（缩略图）+ 投影事件带 parts | D | P0 |
| 6 | estimate_tokens 扩展 + 钳制联动 | D | P0 |
| 7 | 图片查看器 | E | P1 |
| 8 | 工具结果投影 + screenshot/read_image 工具 | E | P1 |
| 9 | 降级链（挂捆边界）+ detail 旋钮 | E | P2 |
| 10 | 音频 | E+ | P2 |
| — | 模型能力数据（模态/梯子） | A/B 前置 | — |

---

## 十三、常见坑（v2.0 更新）

| 坑 | 原因 | 解法 |
|---|---|---|
| 老会话重放丢消息 | 落盘格式重写 | serde(default) parts；v2.47 双气泡回归必须绿 |
| 图片撑爆上下文 | 无限制 | 上传缩边 + 体积上限 + 估算进钳制公式 |
| 图片格式不支持 | 兼容层差异 | 门控拦截 + 格式白名单；转码不做（记档） |
| 图片重复存储 | 无去重 | sha256 内容寻址 |
| Asset 泄漏 | 引用没扫全 | GC 三扫描面 + 7 天宽限 |
| 前缀缓存失效 | 图片字节每轮变 | 内容寻址 = 字节稳定；正文在 parts 前（稳定头） |
| 模型不支持图像 | 选错模型 | 上传时提示 + 发送前拦截（双道） |
| 压缩丢图 | 摘要器吃图 | v1 图片不进压缩只进估算；降级链 P2 挂捆边界 |
| 工具图片无处放 | tool 消息只吃文本 | 投影到下一条 user parts（§8.1） |
| 渲染闪烁 | 重复读盘 | AssetStore 内存 LRU + 缩略缓存 |
| 快照与 jsonl 不一致 | 只改了一边 | Message 与 SessionEvent 同批改 + 往返测试 |

---

## 十四、一句话

**多模态是"把消息从一维文本升级为多维内容"——但升级的落盘格式必须向后兼容（serde default 增量），工具图片必须投影（tool 消息只吃文本），第一版只做输入侧（输出侧单线走不通）。**

---

*文档版本：v2.0（2026-10-07）* *配套项目：Locowrk* *关联文档：《LLM 客户端与调度层设计》《模型供应商配置与能力探测设计 v2.0》《Model Capability and Multimodal Phase Plan》*