//! `lwpreview://` 预览协议（UI shell 文档 §2.5 浏览器页签，v2.144）：把
//! 工作区（fsRoots）内的本地文件喂给工作面板浏览器页签的 iframe。安全
//! 边界三条：路径必须在 fsRoots 内（canonicalize 后前缀比对，不信 URL
//! 转述）；只 GET；体积钳 20MB。HTML 响应注入 console 桥（`bridge`）。
//!
//! URL 形态：`http://lwpreview.localhost/<逐段编码的绝对路径>`——段内
//! 编码、'/' 做分隔，页面里的相对引用（./game.js、../shared/x.css）才能
//! 落回同目录（整串编码会把相对解析顶到根上，全部 404）。

use std::path::{Path, PathBuf};

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use tauri::http::{header, Request, Response};
use tauri::{Manager, UriSchemeContext};

use super::bridge;
use crate::state::AppState;

/// 响应体上限：单文件 HTML 小游戏/图片的量级远在其下，20MB 拦意外。
const MAX_BYTES: u64 = 20 * 1024 * 1024;

/// 预览协议 origin（Windows/WebView2 下自定义 scheme 落 http + .localhost）。
pub const PREVIEW_ORIGIN: &str = "http://lwpreview.localhost";

/// 段内编码集：controls + URL 结构字符 + Windows 非法文件名字符；保留
/// unreserved（A-Za-z0-9-._~）与分隔符 '/'（由拼接方持有）。
const SEGMENT_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// 绝对路径 → 预览 URL（逐段编码、'/' 连接）。canonicalize 产物带 verbatim
/// 前缀（`\\?\F:\...`），URL 面剥掉（走查实抓：不剥则首段成 "?"，协议侧
/// 判非绝对路径 400，工具开的页签全是错误页）。
pub fn preview_url(path: &str) -> String {
    let normalized = path.trim_start_matches(r"\\?\").replace('\\', "/");
    let encoded: Vec<String> = normalized
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| utf8_percent_encode(s, SEGMENT_SET).to_string())
        .collect();
    format!("{PREVIEW_ORIGIN}/{}", encoded.join("/"))
}

/// URL path → 解码后的段列表（拒绝逃生段与空段）。
fn decode_segments(raw_path: &str) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for seg in raw_path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        let decoded = percent_decode_str(seg).decode_utf8().ok()?.to_string();
        if decoded == ".." {
            out.pop()?;
            continue;
        }
        // 段内不该再出现分隔符（%2F 编码进段里的当字面值收下）
        if decoded.contains('/') || decoded.contains('\\') {
            return None;
        }
        out.push(decoded);
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

fn plain_response(status: u16, message: &str) -> Response<std::borrow::Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .body(std::borrow::Cow::Owned(message.as_bytes().to_vec()))
        .expect("static response builder")
}

/// 扩展名 → MIME（预览可渲染集；未识别落 octet-stream 由浏览器下载/拒显）。
fn mime_for(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "svg" => "image/svg+xml",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "txt" | "md" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

/// fsRoots 围栏：canonicalize 后前缀比对（两边同规范形，\\?\ 前缀一致）。
/// `browser_open` 工具与协议处理器共用同一判定。
pub(crate) fn scope_check(root_paths: &[String], target: &Path) -> Result<PathBuf, u16> {
    if root_paths.is_empty() {
        return Err(404);
    }
    let canonical = target.canonicalize().map_err(|_| 404u16)?;
    for root in root_paths {
        let root_path = PathBuf::from(root);
        if let Ok(root_canon) = root_path.canonicalize() {
            if canonical.starts_with(&root_canon) {
                return Ok(canonical);
            }
        }
    }
    Err(403)
}

/// Builder 注册的协议处理器（`lwpreview`）。
pub fn handle<R: tauri::Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<std::borrow::Cow<'static, [u8]>> {
    if request.method() != "GET" {
        return plain_response(405, "method not allowed");
    }
    let raw_path = request.uri().path().to_string();
    let Some(segments) = decode_segments(&raw_path) else {
        return plain_response(400, "bad path");
    };
    let target = PathBuf::from(segments.join("/"));
    if !target.is_absolute() {
        return plain_response(400, "absolute path required");
    }

    let roots = ctx.app_handle().state::<AppState>().settings.fs_roots();
    let canonical = match scope_check(&roots, &target) {
        Ok(p) => p,
        Err(status) => {
            let msg = if status == 403 {
                "path is outside the configured workspace roots".to_string()
            } else {
                format!("not found: {}", target.display())
            };
            return plain_response(status, &msg);
        }
    };

    let meta = match std::fs::metadata(&canonical) {
        Ok(m) if m.is_file() => m,
        _ => return plain_response(404, "not a file"),
    };
    if meta.len() > MAX_BYTES {
        return plain_response(413, "file too large for preview (20MB cap)");
    }
    let bytes = match std::fs::read(&canonical) {
        Ok(b) => b,
        Err(e) => return plain_response(500, &format!("read failed: {e}")),
    };

    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    let mime = mime_for(ext);
    let (content_type, body) = if mime.starts_with("text/html") {
        let html = String::from_utf8_lossy(&bytes);
        (mime.to_string(), bridge::inject_bridge(&html).into_bytes())
    } else {
        (mime.to_string(), bytes)
    };

    Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(std::borrow::Cow::Owned(body))
        .expect("response builder")
}

#[cfg(test)]
mod tests;
