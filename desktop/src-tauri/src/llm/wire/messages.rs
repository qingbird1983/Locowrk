//! 中立消息 → 线上消息（一条线路）：正文与工具调用合成一条 assistant，tool 结果的
//! 错误作为 JSON 内容回传（协议没有独立错误位），**错误支也过预算**——只截成功支等于
//! 留一个绕开闸门的侧门；User 带 parts 时 content 出数组（正文 text 在前 = 稳定前缀）。
//! 空 parts 回落字符串 content，字节与现网逐字节一致。

use serde_json::{json, Value};

use session_store::{ContentPart, Message};

use super::payload::project_tool_output;

/// 中立投影 → 线上消息。Assistant 的正文 + 工具调用合成一条消息（协议要求
/// tool 结果挂在发起调用的同一条 assistant 消息上）；ToolResult 的错误作为
/// JSON 内容回传（协议没有独立错误位，模型可自行处理）。
/// User 带 parts（批 D）→ content 数组：正文 text 在前（稳定前缀，M1 缓存
/// 纪律——同图同 asset 同字节 + 图片只追加在尾部 = 追加式变更）+
/// `image_url` data URL / `file`（PDF）按 parts 序。**空 parts = 字符串
/// content（字节与现网逐字节一致）**。
///
/// `spill`（S2 刀②）= `(落盘句柄, 会话 id)` 两个条件都齐时才有值；尺寸判定本身
/// 不在这里，见 `payload::project_tool_output`（一条判定、两种后果）。
pub(super) fn message_value(
    m: &Message,
    assets: Option<&crate::assets::AssetStore>,
    budget: Option<usize>,
    spill: Option<(&crate::spill::SpillStore, &str)>,
) -> Result<Value, String> {
    match m {
        Message::User { content, parts } => {
            if parts.is_empty() {
                return Ok(json!({ "role": "user", "content": content }));
            }
            let mut arr: Vec<Value> = Vec::with_capacity(parts.len() + 1);
            if !content.is_empty() {
                arr.push(json!({ "type": "text", "text": content }));
            }
            for p in parts {
                match p {
                    ContentPart::Text { text } => {
                        if !text.is_empty() {
                            arr.push(json!({ "type": "text", "text": text }));
                        }
                    }
                    ContentPart::Image { asset } => {
                        let url = asset_data_url(assets, asset)?;
                        arr.push(json!({ "type": "image_url", "image_url": { "url": url } }));
                    }
                    // 兼容层 file part 形态（OpenAI：file_data data URL +
                    // filename；Anthropic/Google 兼容层各自映射）
                    ContentPart::File { asset } => {
                        let url = asset_data_url(assets, asset)?;
                        arr.push(json!({
                            "type": "file",
                            "file": { "filename": asset.name, "file_data": url },
                        }));
                    }
                }
            }
            Ok(json!({ "role": "user", "content": arr }))
        }
        Message::Assistant {
            content,
            tool_calls,
        } => {
            if tool_calls.is_empty() {
                Ok(json!({ "role": "assistant", "content": content }))
            } else {
                Ok(json!({
                    "role": "assistant",
                    "content": if content.is_empty() { Value::Null } else { json!(content) },
                    "tool_calls": tool_calls.iter().map(|tc| json!({
                        "id": tc.id.0,
                        "type": "function",
                        "function": {
                            "name": tc.tool,
                            "arguments": tc.args.to_string(),
                        },
                    })).collect::<Vec<_>>(),
                }))
            }
        }
        Message::ToolResult {
            call_id,
            output,
            error,
            ..
        } => {
            // 错误支也过预算：错误正文可能带一整段 stderr/堆栈，形状与成功支
            // 同属"一条 tool content"，只截成功支等于留一个绕开闸门的侧门。
            let content = match error {
                Some(e) => project_tool_output(json!({ "error": e }).to_string(), budget, spill),
                None => project_tool_output(output.to_string(), budget, spill),
            };
            Ok(json!({ "role": "tool", "tool_call_id": call_id.0, "content": content }))
        }
    }
}

/// asset → data URL（LRU 缓存在 AssetStore 内）。注入缺席 = Fatal（组装
/// 点没接资产存储却收到带附件的请求——测试路径错误配置，如实报错不静默丢）。
fn asset_data_url(
    assets: Option<&crate::assets::AssetStore>,
    asset: &session_store::AssetRef,
) -> Result<String, String> {
    let store =
        assets.ok_or_else(|| format!("资产存储未接入，附件 {} 无法随请求发送", asset.name))?;
    store.data_url(&asset.id, &asset.mime)
}
