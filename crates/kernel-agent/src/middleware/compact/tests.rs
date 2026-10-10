//! 批 C 摘掉 `CtxCompact` 中间件后，压缩面只剩两件活代码，这里各锁一道：
//! [`estimate_tokens`] 的尺子口径（全仓用量/预检/树尾巴估算都吃它）与
//! [`CompactionOutcome`] 的历史事件载荷形状（旧会话日志重放面，字段不能漂）。

use super::{CompactionOutcome, estimate_tokens};
use session_store::{AssetRef, ContentPart, Message};

fn asset() -> AssetRef {
    AssetRef {
        id: "abc123".into(),
        mime: "image/png".into(),
        name: "shot.png".into(),
    }
}

#[test]
fn estimate_is_serialized_bytes_over_four() {
    let empty: Vec<Message> = Vec::new();
    assert_eq!(
        estimate_tokens(&empty),
        0,
        "空列表 = 0（不是 None 也不是猜）"
    );

    let msg = Message::user("hello world");
    let expected = serde_json::to_string(&msg).unwrap().len() as u64 / 4;
    assert_eq!(
        estimate_tokens(std::slice::from_ref(&msg)),
        expected,
        "口径 = 序列化字节 / 4，改公式必须过这条"
    );
    assert!(
        estimate_tokens(&[msg.clone(), msg]) > expected,
        "消息条数增加估算必须增加"
    );
}

#[test]
fn media_parts_add_their_flat_values() {
    // 平铺值锚点本身（图片 1024 = DeepSeek 官方单图上限；文件 1000；Text
    // 不平铺）——一把尺的刻度先钉死。
    let img = ContentPart::Image { asset: asset() };
    let file = ContentPart::File { asset: asset() };
    let text = ContentPart::Text { text: "x".into() };
    assert_eq!(img.est_tokens(), 1024, "图片平铺 1024");
    assert_eq!(file.est_tokens(), 1000, "文件平铺 1000");
    assert_eq!(text.est_tokens(), 0, "Text 部件不吃平铺额度");

    // 集成面：加一个部件 = 平铺值 **加上部件自身的 JSON 字节**。实测差值
    // 1046（1024 + 部件序列化后 ÷4 的 22），所以只能断「≥ 平铺值、且不超出
    // 一个量级」——把差值写死成 1024 是假的（首轮即被自己抓红）。
    let base = Message::user("同一正文");
    let text_only = estimate_tokens(&[base]);
    let flat_check = |part: ContentPart, flat: u64| {
        let with = estimate_tokens(&[Message::user_with_parts("同一正文", vec![part])]);
        assert!(
            with - text_only >= flat,
            "至少贡献自己的平铺值 {flat}，got {}",
            with - text_only
        );
        assert!(
            with - text_only < flat + 64,
            "多出的只有部件那几个字节的 JSON，got {}",
            with - text_only
        );
    };
    flat_check(
        ContentPart::Image { asset: asset() },
        ContentPart::Image { asset: asset() }.est_tokens(),
    );
    flat_check(
        ContentPart::File { asset: asset() },
        ContentPart::File { asset: asset() }.est_tokens(),
    );

    // Text 部件只添几个字节，绝不得爬到图片那量级
    let with_text_part = estimate_tokens(&[Message::user_with_parts("同一正文", vec![text])]);
    assert!(with_text_part > text_only, "多一个部件 = 多几个字节");
    assert!(
        with_text_part - text_only < 100,
        "Text 部件不得吃到图片那量级的平铺值"
    );
}

#[test]
fn cjk_is_underestimated_by_design() {
    // 记档行为的反面锁：CJK 每字符 3 字节 / 4 后仍**低于**字符数——这把尺
    // 对中文是低估（宁早压不爆仓的前提是它被真实锚点接管，v2.138；供应商
    // 不报用量时才回落到这里）。将来有人改成按字符数算，这条会亮红提醒
    // 口径变了。
    let han = "汉".repeat(4_000);
    let est = estimate_tokens(&[Message::user(han.clone())]);
    assert!(
        est < han.chars().count() as u64,
        "中文按字节 /4 必须落在字符数以下，got {est}"
    );
}

#[test]
fn outcome_reads_old_log_shape() {
    // 旧会话日志里 ContextCompacted 事件的载荷字面形（重放面：runner 投影
    // → 前端通知/转录）。字段名漂一个，历史事件就读不回来。
    let logged = r#"{
        "from_tokens": 41200,
        "to_tokens": 8100,
        "strategy": "layered",
        "summary": "讨论了切点纪律。",
        "kept_messages": 6
    }"#;
    let back: CompactionOutcome = serde_json::from_str(logged).expect("历史形状必须读得进");
    assert_eq!(back.strategy, "layered");
    assert_eq!(back.kept_messages, 6);
    let again = serde_json::to_value(&back).unwrap();
    assert_eq!(again["from_tokens"], 41200, "序列化字段名与日志同形");
    assert_eq!(again["summary"], "讨论了切点纪律。");
}
