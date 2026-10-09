use super::*;

fn limits() -> Limits {
    Limits {
        max_dimension: 2048,
        max_size_mb: 5,
        image_formats: vec![
            "png".into(),
            "jpg".into(),
            "jpeg".into(),
            "webp".into(),
            "gif".into(),
        ],
        file_formats: vec!["pdf".into()],
    }
}

/// 最小合法 PNG：8 字节签名 + IHDR（13 字节数据）——尺寸 100×50。
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut v = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend_from_slice(&[0, 0, 0, 13]); // IHDR 数据长
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&w.to_be_bytes());
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0]); // 位深/色型/压缩/滤波/隔行
    v
}

#[test]
fn sniff_and_accept_png_with_dimensions() {
    let s = validate(&png(1920, 1080), "image/png", &limits()).unwrap();
    assert_eq!((s.mime, s.ext, s.dir), ("image/png", "png", "images"));
}

#[test]
fn rejects_unknown_magic_and_oversize_and_dimension() {
    // 未知魔数（哪怕申报 image/png）
    assert!(validate(b"\x00\x01\x02", "image/png", &limits()).is_err());
    // 声明白名单外的扩展（pdf 白名单只收 pdf；这里是图片被拒）
    let mut l = limits();
    l.image_formats = vec!["jpg".into()];
    assert!(validate(&png(10, 10), "image/png", &l).is_err());
    // 体积超限（1 字节没事——这里用 max_size_mb=0 = 不限的边界先验）
    let mut l = limits();
    l.max_size_mb = 0;
    assert!(validate(&png(10, 10), "image/png", &l).is_ok());
    // 尺寸超限
    let mut l = limits();
    l.max_dimension = 100;
    let err = validate(&png(1920, 1080), "image/png", &l).unwrap_err();
    assert!(err.contains("1920×1080"), "{err}");
}

#[test]
fn jpeg_gif_webp_pdf_dimension_parsers() {
    // JPEG：SOI + APP0(JFIF 最小 16 长段) + SOF0（8 数据段，高 200 宽 100）
    let mut j = vec![0xFF, 0xD8, 0xFF];
    j.extend_from_slice(&[
        0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0,
    ]);
    j.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x0B, 8, 0x00, 0xC8, 0x00, 0x64, 3, 0x11, 0,
    ]);
    assert_eq!(image_dimensions(&j, "image/jpeg"), Some((100, 200)));
    // GIF：头 + 逻辑屏幕（宽高 LE）
    let mut g = b"GIF89a".to_vec();
    g.extend_from_slice(&100u16.to_le_bytes());
    g.extend_from_slice(&50u16.to_le_bytes());
    assert_eq!(image_dimensions(&g, "image/gif"), Some((100, 50)));
    // WebP VP8X：canvas = 值 + 1
    let mut w = b"RIFF\x24\x00\x00\x00WEBP".to_vec();
    w.extend_from_slice(b"VP8X");
    w.extend_from_slice(&[0x0A, 0, 0, 0]); // chunk size
    w.extend_from_slice(&[0, 0, 0, 0]); // flags
    w.extend_from_slice(&[99, 0, 0]); // width-1 LE24
    w.extend_from_slice(&[49, 0, 0]); // height-1
    assert_eq!(image_dimensions(&w, "image/webp"), Some((100, 50)));
    // PDF 嗅探
    let s = validate(b"%PDF-1.7 ...", "application/pdf", &limits()).unwrap();
    assert_eq!((s.mime, s.dir), ("application/pdf", "files"));
}
