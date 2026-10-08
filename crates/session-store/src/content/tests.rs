use super::*;

#[test]
fn part_serde_roundtrip_tagged() {
    let img = ContentPart::Image {
        asset: AssetRef {
            id: "abc123".into(),
            mime: "image/png".into(),
            name: "shot.png".into(),
        },
    };
    let v = serde_json::to_value(&img).unwrap();
    assert_eq!(v["type"], "image");
    assert_eq!(v["asset"]["id"], "abc123");
    let back: ContentPart = serde_json::from_value(v).unwrap();
    assert_eq!(back, img);
}

#[test]
fn est_tokens_flat_values() {
    let mk = |p: &ContentPart| p.est_tokens();
    assert_eq!(
        mk(&ContentPart::Image {
            asset: AssetRef {
                id: String::new(),
                mime: String::new(),
                name: String::new()
            }
        }),
        1024
    );
    assert_eq!(
        mk(&ContentPart::File {
            asset: AssetRef {
                id: String::new(),
                mime: String::new(),
                name: String::new()
            }
        }),
        1000
    );
    assert_eq!(mk(&ContentPart::Text { text: "hi".into() }), 0);
}

#[test]
fn fold_parts_appends_placeholders() {
    let parts = vec![
        ContentPart::Image {
            asset: AssetRef {
                id: "1".into(),
                mime: "image/png".into(),
                name: "a.png".into(),
            },
        },
        ContentPart::Text {
            text: "ignored".into(),
        },
        ContentPart::File {
            asset: AssetRef {
                id: "2".into(),
                mime: "application/pdf".into(),
                name: "b.pdf".into(),
            },
        },
    ];
    assert_eq!(
        fold_parts("看这个", &parts),
        "看这个 [图片: a.png] [文件: b.pdf]"
    );
    assert_eq!(fold_parts("", &parts), "[图片: a.png] [文件: b.pdf]");
    assert_eq!(fold_parts("纯文本", &[]), "纯文本");
}
