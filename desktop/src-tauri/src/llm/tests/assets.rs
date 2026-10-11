//! 工具结果图片投影（批 E §8.1）：图片进尾随 user 消息 / 历史图不重发但提示稳定 /
//! 不支持模态保留资源引用 / 非图片资源与纯文本输出原样不动。

use super::*;

// ---- 批 E：工具结果图片投影（§8.1，D4 按推荐落档） ----

#[test]
fn tool_image_projects_into_tail_user_message() {
    // agent 循环常态：工具结果在消息列尾部（其后无 user）→ 合成一条
    // 只带图片的 user 消息追加，tool 消息剥引用 + note
    let out = super::wire::project_tool_assets(
        vec![
            Message::User {
                content: "截个屏".into(),
                parts: Vec::new(),
            },
            tool_msg(asset_output()),
        ],
        Some(true),
    );
    assert_eq!(out.len(), 3);
    match &out[1] {
        Message::ToolResult { output, .. } => {
            assert!(output.get("asset").is_none(), "引用号已剥");
            assert_eq!(output["image_note"], "[图片已附于本工具结果之后: shot.png]");
            assert_eq!(output["text"], "screenshot 1920x1080 captured");
        }
        other => panic!("expected tool result, got {other:?}"),
    }
    match &out[2] {
        Message::User { content, parts } => {
            assert_eq!(content, "[工具产生的图片]");
            assert_eq!(parts.len(), 1);
        }
        other => panic!("expected synthetic user, got {other:?}"),
    }
}

#[test]
fn historical_tool_image_not_resent_but_note_stable() {
    // 历史轮：工具结果之后已有 user 消息 → 图片不重发（token 不膨胀），
    // 但 note 文案与尾部轮逐字节一致（M1 前缀缓存纪律）
    let tail = super::wire::project_tool_assets(vec![tool_msg(asset_output())], Some(true));
    let tail_note = match &tail[0] {
        Message::ToolResult { output, .. } => output["image_note"].clone(),
        other => panic!("expected tool result, got {other:?}"),
    };
    let grown = super::wire::project_tool_assets(
        vec![
            tool_msg(asset_output()),
            Message::Assistant {
                content: "屏幕显示…".into(),
                tool_calls: Vec::new(),
            },
            Message::User {
                content: "再来一张".into(),
                parts: Vec::new(),
            },
        ],
        Some(true),
    );
    assert_eq!(grown.len(), 3, "历史轮不合成 user 消息");
    match &grown[0] {
        Message::ToolResult { output, .. } => {
            assert_eq!(output["image_note"], tail_note, "note 跨请求字节稳定");
            assert!(output.get("asset").is_none());
        }
        other => panic!("expected tool result, got {other:?}"),
    }
    // 尾部合成消息只出现一次且在末尾
    let tail2 = super::wire::project_tool_assets(
        vec![
            tool_msg(asset_output()),
            Message::User {
                content: "hi".into(),
                parts: Vec::new(),
            },
        ],
        Some(true),
    );
    assert_eq!(tail2.len(), 2);
    assert!(matches!(tail2[1], Message::User { ref parts, .. } if parts.is_empty()));
}

#[test]
fn unsupported_model_keeps_asset_ref_with_note() {
    let out = super::wire::project_tool_assets(vec![tool_msg(asset_output())], Some(false));
    assert_eq!(out.len(), 1, "不支持 = 不投影不合成");
    match &out[0] {
        Message::ToolResult { output, .. } => {
            assert_eq!(output["asset"]["id"], "abc123", "引用号留原地");
            assert_eq!(
                output["image_note"],
                "[图片未随请求发送：当前模型不支持图片输入]"
            );
        }
        other => panic!("expected tool result, got {other:?}"),
    }
}

#[test]
fn non_image_assets_and_plain_output_untouched() {
    // PDF 引用不走图片投影（原地保留，零改写）
    let pdf_out = json!({
        "text": "readme.pdf stored",
        "asset": { "id": "pdf1", "mime": "application/pdf", "name": "readme.pdf" }
    });
    let out = super::wire::project_tool_assets(vec![tool_msg(pdf_out)], Some(true));
    match &out[0] {
        Message::ToolResult { output, .. } => assert_eq!(output["asset"]["id"], "pdf1"),
        other => panic!("expected tool result, got {other:?}"),
    }
    // 纯文本工具结果与多资产（assets 数组）保序
    let multi = json!({
        "text": "3 pages",
        "assets": [
            { "id": "p1", "mime": "image/png", "name": "page-1.png" },
            { "id": "p2", "mime": "image/png", "name": "page-2.png" }
        ]
    });
    let out = super::wire::project_tool_assets(vec![tool_msg(multi)], None);
    assert_eq!(out.len(), 2, "None = 放行投影");
    match &out[1] {
        Message::User { parts, .. } => {
            let ids: Vec<&str> = parts
                .iter()
                .map(|p| match p {
                    session_store::ContentPart::Image { asset } => asset.id.as_str(),
                    _ => "",
                })
                .collect();
            assert_eq!(ids, vec!["p1", "p2"], "多资产保序（页序不翻面）");
        }
        other => panic!("expected synthetic user, got {other:?}"),
    }
    // 跨结果也保序：A[p1,p2] 在前、B[b1] 在后 → 合成消息 [p1,p2,b1]
    let two = vec![
        Message::User {
            content: "hi".into(),
            parts: Vec::new(),
        },
        tool_msg(json!({"assets": [
            { "id": "a1", "mime": "image/png", "name": "a1.png" },
            { "id": "a2", "mime": "image/png", "name": "a2.png" }
        ]})),
        tool_msg(json!({ "asset": { "id": "b1", "mime": "image/png", "name": "b1.png" }})),
    ];
    let out = super::wire::project_tool_assets(two, None);
    match out.last() {
        Some(Message::User { parts, .. }) => {
            let ids: Vec<&str> = parts
                .iter()
                .map(|p| match p {
                    session_store::ContentPart::Image { asset } => asset.id.as_str(),
                    _ => "",
                })
                .collect();
            assert_eq!(ids, vec!["a1", "a2", "b1"], "跨结果保序");
        }
        other => panic!("expected synthetic user, got {other:?}"),
    }
}
