//! 图片与附件的发送面（批 D / 批 E，多模态稿 §8.1）：`check_modalities` = 发送前
//! 能力门控（未知放行，不发明目录没有的能力），`project_tool_assets` = 工具结果里的
//! 图片从 tool 消息挪出、合成一条只带图的尾部 user 消息（媒体投影），加它的四条
//! 纪律与 note 文案。改写是**位置无关的纯函数**：note 跨请求逐字节稳定（M1 前缀缓存纪律），
//! 历史轮的图片不重发（重发 = token 永久膨胀）。

use serde_json::Value;

use session_store::{ContentPart, Message};

use kernel_agent::LlmRequest;

/// 发送前能力门控（供应商稿 §7 第二道防线；第一道在批 D 前端上传时）：
/// 请求带图片 part 且生效模型图像输入三态解析为 `Some(false)` → 拒发带
/// 指引。**未知（None）放行**——目录没有的能力不发明（宁缺勿假）。
pub fn check_modalities(req: &LlmRequest, image_allowed: Option<bool>) -> Result<(), String> {
    let has_image = req.messages.iter().any(|m| {
        matches!(m, Message::User { parts, .. } if parts
            .iter()
            .any(|p| matches!(p, ContentPart::Image { .. })))
    });
    if has_image && image_allowed == Some(false) {
        return Err("当前模型不支持图片输入：请在 设置 → 模型 里换用视觉模型，\
             或去掉图片附件后重发"
            .into());
    }
    Ok(())
}

/// 工具结果图片投影（批 E，多模态稿 §8.1；裁定 D4 按推荐落档 = 投影）。
/// 工具 output 里的 `asset`/`assets`（mime `image/*`）从 tool 消息挪出——
/// tool 消息只吃文本，图片随消息列尾部合成的一条 user 消息发送（agent 循环
/// 里工具结果就在尾部，「下一条 user 消息」多数时候不存在，合成一条只带
/// 图片的 user 消息 = ZCode 媒体投影同思路；设计稿「无下一条 user 保持
/// 占位」的形态记档偏离——占位会让模型永远看不到图，主流程不可用）。
///
/// 三条纪律：
/// * **工具消息改写是位置无关的纯函数**——note 文本跨请求逐字节稳定
///   （M1 前缀缓存纪律）；历史轮里已有后续 user 消息的工具图片**不重发**
///   （每请求重发 = token 永久膨胀；模型在工具结果当轮已看过一次）。
/// * 模型不支持图片（`Some(false)`）→ 不投影不报错：引用号留原地 + note
///   指引（工具文本输出照常可用；整轮 Fatal 不成比例）。
/// * 调用点在 `AppLlm::prepare`（image_allowed 已在场）——maxOutput 钳制
///   估算吃的是投影后消息，设计稿记档的估算缺口在调用点闭合。
pub fn project_tool_assets(messages: Vec<Message>, image_allowed: Option<bool>) -> Vec<Message> {
    let has_images = messages.iter().any(|m| match m {
        Message::ToolResult { output, error, .. } => {
            error.is_none() && !tool_image_assets(output).is_empty()
        }
        _ => false,
    });
    if !has_images {
        return messages;
    }
    let mut tail_assets: Vec<session_store::AssetRef> = Vec::new();
    let mut user_seen = false; // 从尾往前扫：更晚的 user 消息是否出现过
    let mut out: Vec<Message> = Vec::new();
    for m in messages.into_iter().rev() {
        let m = match m {
            Message::ToolResult {
                call_id,
                tool,
                args,
                output,
                error,
            } if error.is_none() => {
                let assets = tool_image_assets(&output);
                if assets.is_empty() {
                    Message::ToolResult {
                        call_id,
                        tool,
                        args,
                        output,
                        error,
                    }
                } else if image_allowed == Some(false) {
                    // 引用号留原地（设计稿「保持文本占位 + asset 引用号」）
                    Message::ToolResult {
                        call_id,
                        tool,
                        args,
                        output: with_image_note(&output, false, unsupported_note()),
                        error,
                    }
                } else {
                    if !user_seen {
                        // 逆序遍历：结果间与结果内都先翻，末尾一次 reverse
                        // 双双还原（实抓：单结果 assets 数组被整体翻面，
                        // 模型看到的页序颠倒）
                        tail_assets.extend(assets.iter().rev().cloned());
                    }
                    Message::ToolResult {
                        call_id,
                        tool,
                        args,
                        output: with_image_note(&output, true, attached_note(&assets)),
                        error,
                    }
                }
            }
            other => other,
        };
        if matches!(m, Message::User { .. }) {
            user_seen = true;
        }
        out.push(m);
    }
    out.reverse();
    if !tail_assets.is_empty() {
        // 逆序收集，落位前翻回消息序
        tail_assets.reverse();
        out.push(Message::User {
            content: TOOL_IMAGE_PREFIX.into(),
            parts: tail_assets
                .into_iter()
                .map(|asset| ContentPart::Image { asset })
                .collect(),
        });
    }
    out
}

/// 合成图片 user 消息的正文前缀（content 数组 text 在前 = 稳定前缀）。
const TOOL_IMAGE_PREFIX: &str = "[工具产生的图片]";

/// 工具 output 里的 image 资产（§8.1 约定：`asset` 单数 / `assets` 数组；
/// mime `image/*` 才算——PDF 引用不走图片投影，原地保留）。
fn tool_image_assets(output: &Value) -> Vec<session_store::AssetRef> {
    let parse = |v: &Value| -> Option<session_store::AssetRef> {
        let id = v.get("id")?.as_str()?.to_string();
        let mime = v.get("mime")?.as_str()?.to_string();
        let name = v.get("name")?.as_str()?.to_string();
        mime.starts_with("image/")
            .then_some(session_store::AssetRef { id, mime, name })
    };
    let mut out = Vec::new();
    if let Some(a) = output.get("asset") {
        if let Some(r) = parse(a) {
            out.push(r);
        }
    }
    if let Some(list) = output.get("assets").and_then(Value::as_array) {
        for a in list {
            if let Some(r) = parse(a) {
                out.push(r);
            }
        }
    }
    out
}

/// 已投影的 note（历史轮与尾部轮同文案 = 字节稳定；指过去发生的事实）。
fn attached_note(assets: &[session_store::AssetRef]) -> String {
    let names: Vec<&str> = assets.iter().map(|a| a.name.as_str()).collect();
    format!("[图片已附于本工具结果之后: {}]", names.join(", "))
}

/// 模型不支持图片时的 note（引用号留原地）。
fn unsupported_note() -> String {
    "[图片未随请求发送：当前模型不支持图片输入]".into()
}

/// output 克隆改写：`strip` 时剥 `asset`/`assets` 键（投影成功后引用号是
/// 噪声；不支持时保留），统一加 `image_note`。
fn with_image_note(output: &Value, strip: bool, note: String) -> Value {
    let Some(mut obj) = output.as_object().cloned() else {
        return output.clone();
    };
    if strip {
        obj.remove("asset");
        obj.remove("assets");
    }
    obj.insert("image_note".into(), Value::String(note));
    Value::Object(obj)
}
