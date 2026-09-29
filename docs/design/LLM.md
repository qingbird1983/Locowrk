要同时支持 OpenAI、Anthropic 和 Ollama，最直接的方法是**利用它们 API 的高度兼容性，设计一个统一的适配器层**。同时，善用现有 Rust 生态中的 crate，可以省去大量手动处理 HTTP 和 SSE 的工作。

### 🎯 核心理念：一个接口，三种实现

内核不应关心底层是哪个 LLM 提供商，它只与一个统一的 `LlmService` 接口交互。

```rust
// 内核中统一的 LLM 服务接口
#[async_trait]
pub trait LlmService: Send + Sync + 'static {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError>;
    async fn stream(&self, req: LlmRequest) -> Result<BoxStream<'static, Result<StreamChunk, LlmError>>, LlmError>;
}
```

然后，为每个提供商创建一个实现了该接口的插件。你的 `kernel-agent` 只需要通过 `ctx.use_service::<dyn LlmService>()` 来获取服务，完全无需知晓其背后是 OpenAI、Anthropic 还是 Ollama。

### 📊 三巨头 API 关键差异速查

虽然目标统一，但三家的 API 在细节上各有不同。下表总结了在实现适配器时需要特别关注的核心差异：

| 特性维度 | OpenAI (Chat Completions) | Anthropic (Messages) | Ollama (/api/chat) |
| :--- | :--- | :--- | :--- |
| **请求端点** | `POST /v1/chat/completions` | `POST /v1/messages` | `POST /api/chat` |
| **系统提示** | 作为 `messages` 数组中的一条 `role: "system"` 消息 | 独立的顶层 `system` 参数 | 作为 `messages` 数组中的一条 `role: "system"` 消息 |
| **工具定义** | `tools` 数组，函数定义在 `function` 字段内 | `tools` 数组，直接包含 `name`、`description`、`input_schema` | `tools` 数组，格式与 OpenAI 兼容 |
| **流式格式** | **SSE**，数据行以 `data: ` 开头 | **SSE**，使用命名事件 `event: <type>` + `data: <json>` | **NDJSON**，每行一个独立的 JSON 对象 |
| **流式终止符** | `data: [DONE]` | 事件类型 `message_stop` | 最后一个 JSON 对象的 `done` 字段为 `true` |
| **工具调用 (流式)** | `delta.tool_calls` 数组，参数分片传输，需按 `index` 或 `id` 拼接 | 内容块类型为 `tool_use`，`input` 字段为 JSON 对象 | 格式与 OpenAI 类似 |
| **工具结果 (消息)** | `role: "tool"` 的消息，包含 `tool_call_id` 和 `content` | `role: "user"` 的消息，`content` 数组中包含 `type: "tool_result"` 的块 | 格式与 OpenAI 兼容 |

### 🛠️ 实现方案选型

你可以选择引入成熟的 crate 来简化开发，或为了完全的控制而手动实现。

#### 方案一：使用统一的 LLM 客户端库 (推荐)
社区已有一些优秀的库提供了多提供商支持，可以大大减少适配工作量。
*   **`genai`**：支持 OpenAI、Anthropic、Ollama、Gemini 等 **26+ 个提供商**，提供统一的 Rust API。你可以通过模型名（如 `gpt-5.6-luna`）自动路由，也支持自定义 OpenAI 兼容端点。
*   **`multi-llm`**：另一个统一接口的库，支持 OpenAI、Anthropic、Ollama、LM Studio。
*   **`llm`**：支持 OpenAI、Anthropic、Ollama、DeepSeek、xAI 等。

**优势**：开发速度最快，网络层、错误处理、SSE 解析等复杂性都由库处理。
**劣势**：你对底层协议的控制力较弱，可能难以实现某些特定于提供商的优化。

#### 方案二：针对每个提供商手动实现
如果你需要完全的控制权，可以为每个提供商实现独立的适配器。
*   **OpenAI**：使用 `async-openai` crate，功能全面，有清晰的工具调用和流式示例。
*   **Anthropic**：使用 `anthropic-sdk-rs` 或 `async-anthropic`，支持消息和 SSE 流式。
*   **Ollama**：使用 `ollama-sdk` 或 `ollama-rs`，支持流式和工具调用。

**优势**：完全控制，可为每个提供商定制最佳实现。
**劣势**：需要手动处理三家不同的 API 细节，工作量大。

**建议**：**优先选择方案一**。将 `genai` 这样的库封装在你的 `llm-openai` 插件内部，对外只暴露统一的 `LlmService` 接口。这样既获得了开发效率，又保持了内核的干净。

### 🧩 核心实现要点

#### 1. 流式处理与事件发射
`complete` 方法可以简单地等待完整响应。`stream` 方法需要处理三家不同的流式格式，并将其统一为你的 `StreamChunk` 事件，然后通过 Tauri 的 `emit` 推送。

```rust
// 在 stream 方法内部
while let Some(line) = reader.next_line().await? {
    let chunk = match self.provider {
        Provider::OpenAI => parse_openai_sse(&line)?,
        Provider::Anthropic => parse_anthropic_sse(&line)?,
        Provider::Ollama => parse_ollama_ndjson(&line)?,
    };
    match chunk {
        StreamChunk::Text(text) => {
            app_handle.emit("agent://chunk", ChunkPayload { delta: text, .. })?;
        }
        StreamChunk::ToolCall(tool_call) => {
            // 累积工具调用，直到流结束
        }
        StreamChunk::Done => break,
    }
}
```

#### 2. 工具调用 (Function Calling) 的统一适配
*   **定义**：在 `LlmRequest` 中统一使用 OpenAI 风格的 `tools` 字段。适配器负责将其转换为 Anthropic 的格式。
*   **调用**：流式响应中，OpenAI 的 `delta.tool_calls` 和 Anthropic 的 `tool_use` 块都需要被解析并**累积**到一个完整的 `ToolCall` 结构中。
*   **结果**：当 Agent 执行完工具后，你需要根据提供商，将结果打包成正确的格式（OpenAI 的 `role: "tool"` 或 Anthropic 的 `tool_result` 内容块）。

#### 3. 配置与插件注册
`llm-openai` 插件的配置文件可以设计得灵活一些，通过 `provider` 字段来区分不同的后端。

```toml
[llm]
provider = "openai" # 或 "anthropic" | "ollama"
model = "gpt-5.6-luna"
api_key = "sk-..."
base_url = "https://api.openai.com/v1" # 对于 Ollama，通常为 http://localhost:11434
```

在插件启动时（`apply` 方法中），根据配置实例化对应的适配器，并将其注册到 `Context` 中。

```rust
// 在插件启动时
let adapter: Arc<dyn LlmService> = match config.provider.as_str() {
    "openai" => Arc::new(OpenAiAdapter::new(&config)?),
    "anthropic" => Arc::new(AnthropicAdapter::new(&config)?),
    "ollama" => Arc::new(OllamaAdapter::new(&config)?),
    _ => return Err(PluginError::UnsupportedProvider),
};
ctx.provide::<dyn LlmService>(adapter);
```

### 💎 总结与下一步

让 `Locowrk` 同时支持三大模型，核心在于**抽象一个统一的 `LlmService` 接口**，并为每个提供商实现适配器。为了快速启动，强烈建议**集成 `genai` 这类统一客户端库**，在你的插件内部完成转换和封装。

实现这一步后，你的“脚本替身”就可以正式退役，`Locowrk` 将成为一个真正由大模型驱动的 Agent。接好之后，`agent/request` waterfall 插桩点和 `ctx-compact` 上下文压缩中间件的价值就能真正体现出来了。