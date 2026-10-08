//! media 模块测试（全部无网络、无屏幕、无 pdfium）：PNG 编码 × AssetStore
//! 落盘往返 + 页数计划钳制（pdf.rs 内）。真实截屏/渲染走实机走查——单测
//! 不依赖显示器与动态库（CI/无头环境可跑）。

use std::sync::Arc;

use serde_json::json;
use plugin_api::Tool;

use super::{rgba_to_png, ScreenshotTool};
use crate::assets::{validate, AssetStore};

fn test_limits() -> validate::Limits {
    validate::Limits {
        max_dimension: 0,
        max_size_mb: 10,
        image_formats: vec!["png".into(), "jpg".into(), "webp".into(), "gif".into()],
        file_formats: vec!["pdf".into()],
    }
}

fn temp_store(tag: &str) -> (std::path::PathBuf, Arc<AssetStore>) {
    let dir = std::env::temp_dir().join(format!("locowrk-media-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    (dir.clone(), Arc::new(AssetStore::new(dir.join("assets"))))
}

#[test]
fn rgba_png_roundtrip_through_asset_store() {
    // 4×2 红蓝像素 → PNG → AssetStore 落盘 → 读回解码，尺寸与首像素一致
    let (_dir, store) = temp_store("png-roundtrip");
    let mut rgba = Vec::with_capacity(4 * 2 * 4);
    for x in 0..4u32 {
        for _y in 0..2 {
            rgba.extend_from_slice(&[if x == 0 { 255 } else { 0 }, 0, 0, 255]);
        }
    }
    let png = rgba_to_png(4, 2, rgba).expect("encode");
    let asset = store.put(&png, "image/png", "t.png", &test_limits()).expect("put");
    assert_eq!(asset.mime, "image/png");
    let path = store.path_of(&asset.id, &asset.mime).expect("path");
    let bytes = std::fs::read(&path).expect("read back");
    let img = image::load_from_memory(&bytes).expect("decode");
    let rgba = img.to_rgba8();
    assert_eq!((img.width(), img.height()), (4, 2));
    assert_eq!(rgba.get_pixel(0, 0).0, [255, 0, 0, 255]);
    // 同字节再 put = 幂等（内容寻址同 id）
    let again = store.put(&png, "image/png", "t.png", &test_limits()).expect("put2");
    assert_eq!(again.id, asset.id);
}

#[test]
fn rgba_png_rejects_size_mismatch() {
    assert!(rgba_to_png(4, 2, vec![0; 3 * 4]).is_err(), "字节数不符 = Err 不 panic");
}

#[test]
fn screenshot_spec_shape() {
    // 目录声明面锁形状：Pure + Always + 参数 schema（走查与暴露文档对表）
    let store = AssetStore::new(std::env::temp_dir().join("locowrk-media-spec"));
    let limits: super::LimitsFn = Arc::new(test_limits);
    let tool = ScreenshotTool::new(Arc::new(store), limits);
    let spec = tool.spec();
    assert_eq!(spec.name, "screenshot");
    assert!(matches!(spec.side_effect, plugin_api::SideEffect::Pure));
    assert!(matches!(spec.exposure, plugin_api::Exposure::Always));
    assert!(spec.schema["properties"]["display"].is_object());
    // schema 是合法 JSON object（ToolSpec 契约）
    assert!(json!(spec.schema).is_object());
}
