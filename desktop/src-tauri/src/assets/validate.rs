//! 上传校验（批 D §6.1）：格式白名单（魔数嗅探，不信客户端 mime 申报）+
//! 体积上限 + 图片单边上限。**超限拒绝不转码**（图片转码记档不做，阶段
//! 计划批 D「明确不做」）——上限阈值 settings `multimodal` 区可调。

/// 校验参数（settings multimodal 区投影；命令层读设置传参——本模块不依
/// 赖设置存储，纯函数可测）。
#[derive(Debug, Clone)]
pub struct Limits {
    /// 图片单边上限（px）。0 = 不限。
    pub max_dimension: u32,
    /// 体积上限（MB）。0 = 不限。
    pub max_size_mb: u64,
    /// 图片格式白名单（扩展名小写；空 = 全拒）。
    pub image_formats: Vec<String>,
    /// 文件格式白名单。
    pub file_formats: Vec<String>,
}

/// 嗅探结论：mime + 扩展名 + 类别目录（images/files）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sniffed {
    pub mime: &'static str,
    pub ext: &'static str,
    pub dir: &'static str,
}

/// 校验入口：返回嗅探出的规范 mime（与申报 mime 解耦——落盘mime 以魔数
/// 为准）。失败 = Err 面向用户的文案。
pub fn validate(bytes: &[u8], _declared_mime: &str, limits: &Limits) -> Result<Sniffed, String> {
    let sniffed = sniff(bytes).ok_or_else(|| {
        "不支持的文件格式（仅支持 PNG/JPG/WebP/GIF 图片与 PDF）".to_string()
    })?;
    let allowed: &Vec<String> = if sniffed.dir == "images" {
        &limits.image_formats
    } else {
        &limits.file_formats
    };
    if !allowed.iter().any(|f| f == sniffed.ext) {
        return Err(format!(
            "格式 .{} 未启用（设置 multimodal 区可调整白名单）",
            sniffed.ext
        ));
    }
    if limits.max_size_mb > 0 {
        let mb = bytes.len() as u64 / (1024 * 1024);
        let over = bytes.len() as u64 > limits.max_size_mb * 1024 * 1024;
        if over {
            return Err(format!(
                "文件 {mb}MB 超过上限 {lim}MB",
                lim = limits.max_size_mb
            ));
        }
    }
    if sniffed.dir == "images" && limits.max_dimension > 0 {
        if let Some((w, h)) = image_dimensions(bytes, sniffed.mime) {
            let m = limits.max_dimension;
            if w > m || h > m {
                return Err(format!(
                    "图片 {w}×{h} 超过单边上限 {m}px（先自行缩小后再上传）"
                ));
            }
        }
        // 尺寸读不出的合法图（罕见变体）放行——体积与格式两道已过
    }
    Ok(sniffed)
}

/// 魔数嗅探（宁缺勿假：认不出的格式一律拒绝，哪怕 mime 申报合法）。
fn sniff(b: &[u8]) -> Option<Sniffed> {
    let has = |pre: &[u8]| b.len() >= pre.len() && &b[..pre.len()] == pre;
    if has(b"\x89PNG\r\n\x1a\n") {
        return Some(Sniffed { mime: "image/png", ext: "png", dir: "images" });
    }
    if has(b"\xff\xd8\xff") {
        return Some(Sniffed { mime: "image/jpeg", ext: "jpg", dir: "images" });
    }
    if has(b"GIF87a") || has(b"GIF89a") {
        return Some(Sniffed { mime: "image/gif", ext: "gif", dir: "images" });
    }
    if b.len() > 12 && has(b"RIFF") && &b[8..12] == b"WEBP" {
        return Some(Sniffed { mime: "image/webp", ext: "webp", dir: "images" });
    }
    if has(b"%PDF-") {
        return Some(Sniffed { mime: "application/pdf", ext: "pdf", dir: "files" });
    }
    None
}

/// 图片头解析单边尺寸（PNG IHDR / JPEG SOF / GIF 逻辑屏幕 / WebP VP8X、
/// VP8 /VP8L）。读不出 = None（放行，交由体积/格式两道把关）。
pub fn image_dimensions(b: &[u8], mime: &str) -> Option<(u32, u32)> {
    match mime {
        "image/png" => {
            // IHDR 在固定偏移 16：宽高各 4 字节大端
            if b.len() < 24 {
                return None;
            }
            Some((
                u32::from_be_bytes(b[16..20].try_into().ok()?),
                u32::from_be_bytes(b[20..24].try_into().ok()?),
            ))
        }
        "image/gif" => {
            if b.len() < 10 {
                return None;
            }
            Some((
                u16::from_le_bytes(b[6..8].try_into().ok()?) as u32,
                u16::from_le_bytes(b[8..10].try_into().ok()?) as u32,
            ))
        }
        "image/jpeg" => jpeg_dimensions(b),
        "image/webp" => webp_dimensions(b),
        _ => None,
    }
}

/// JPEG：扫段标记找 SOF0–SOF15（跳过 D8/D01 填充与 RSTn），段长 2 字节
/// 大端；SOF 段内 偏移5=高 2字节、偏移7=宽。
fn jpeg_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2usize;
    while i + 9 <= b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = b[i + 1];
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        if i + 4 > b.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes(b[i + 2..i + 4].try_into().ok()?) as usize;
        let is_sof = (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker);
        if is_sof {
            if i + 9 > b.len() {
                return None;
            }
            let h = u16::from_be_bytes(b[i + 5..i + 7].try_into().ok()?) as u32;
            let w = u16::from_be_bytes(b[i + 7..i + 9].try_into().ok()?) as u32;
            return Some((w, h));
        }
        i += 2 + seg_len;
    }
    None
}

/// WebP：VP8X（canvas 尺寸 24bit−1）/ 有损 VP8 / 无损 VP8L 三种容器。
fn webp_dimensions(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 30 {
        return None;
    }
    let fourcc = &b[12..16];
    if fourcc == b"VP8X" {
        let read24 = |s: usize| -> u32 {
            b[s] as u32 | (b[s + 1] as u32) << 8 | (b[s + 2] as u32) << 16
        };
        return Some((read24(24) + 1, read24(27) + 1));
    }
    if fourcc == b"VP8 " {
        // 关键帧起始：3 字节帧标签后 6 字节起 = 宽（14bit LE）/高
        if b[23] != 0x9D || b[24] != 0x01 || b[25] != 0x2A {
            return None;
        }
        let w = (u16::from_le_bytes(b[26..28].try_into().ok()?) & 0x3FFF) as u32;
        let h = (u16::from_le_bytes(b[28..30].try_into().ok()?) & 0x3FFF) as u32;
        return Some((w, h));
    }
    if fourcc == b"VP8L" {
        // 签名 0x2F 后 14+14 bit 尺寸（LSB 风格位包）
        if b[20] != 0x2F {
            return None;
        }
        let bits = u32::from_le_bytes(b[21..25].try_into().ok()?);
        return Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1));
    }
    None
}

#[cfg(test)]
mod tests;
