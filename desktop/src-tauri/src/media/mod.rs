//! 多模态工具面（批 E，多模态设计 v2.0 §8.2）：宿主内置三件——
//! `screenshot`（截屏）/ `read_image`（工作区图片 → 资产）/ `pdf_to_images`
//! （PDF 逐页转图，pdfium 动态库随包）。全部 Pure（本机读与截自己屏幕，
//! 不写不改），经 `KernelToolSource::with_builtin` 进目录。
//!
//! 返回形态 = §8.1 投影约定：output 带 `asset`（单图）或 `assets`（页数组）
//! ——wire 层 [`crate::llm::wire::project_tool_assets`] 把图片挪进消息列
//! 尾部的合成 user 消息，tool 消息文本留 note。转录 UI 直接吃 output 渲染
//! 预览（ToolRow），不依赖消息 parts。
//!
//! 围栏：宿主内置工具不走插件 vtable 的调用期根——read_image /
//! pdf_to_images 用 `fsutil::allowed_roots()`（env 全局 = 数据根 + 设置
//! fsRoots，⑱ 起启动与设置变更即注入，㉘·C 同源语义）。

pub mod pdf;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{json, Value};

use crate::assets::{validate, AssetStore};

/// 上限读取器（settings `multimodal` 区 → validate::Limits；调用期读 =
/// 设置保存即热换，与 asset_put 命令同口径）。
pub(crate) type LimitsFn = Arc<dyn Fn() -> validate::Limits + Send + Sync>;

/// pdfium 动态库文件名（按平台；随包 resources/pdfium/，跨平台打包名单
/// 记档——当前只随 Windows dll）。
pub const PDFIUM_LIB_FILE: &str = if cfg!(target_os = "windows") {
    "pdfium.dll"
} else if cfg!(target_os = "macos") {
    "libpdfium.dylib"
} else {
    "libpdfium.so"
};

/// 随包 pdfium 解析：资源目录存在性判定；缺席 = None（pdf_to_images 不进
/// 目录——宁缺勿假，注册一个必然失败的工具是谎报）。
pub fn resolve_pdfium_lib(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    use tauri::Manager;
    let path = app
        .path()
        .resolve(
            format!("resources/pdfium/{PDFIUM_LIB_FILE}"),
            tauri::path::BaseDirectory::Resource,
        )
        .ok()?;
    tracing::debug!(path = %path.display(), "pdfium 资源解析");
    path.exists().then_some(path)
}

fn failed(tool: &str, message: impl Into<String>) -> ToolError {
    ToolError::Failed {
        tool: tool.to_string(),
        message: message.into(),
    }
}

// ---- screenshot ----

fn screenshot_spec() -> &'static ToolSpec {
    static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "screenshot".into(),
        description: "Capture the screen and attach the image to the conversation so the \
                      model can see it (vision models only; text models get a reference \
                      instead). Optional `display` index (0-based, default 0 = primary). \
                      Pure local read."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "display": { "type": "number",
                             "description": "0-based display index (default 0)" }
            }
        }),
        side_effect: SideEffect::Pure,
        // Always（氛围能力）：目录常驻，模型无需检索即知可以看屏幕
        exposure: Exposure::Always,
    })
}

pub(crate) struct ScreenshotTool {
    assets: Arc<AssetStore>,
    limits: LimitsFn,
}

impl ScreenshotTool {
    pub(crate) fn new(assets: Arc<AssetStore>, limits: LimitsFn) -> Arc<Self> {
        Arc::new(Self { assets, limits })
    }
}

#[async_trait]
impl Tool for ScreenshotTool {
    fn spec(&self) -> &ToolSpec {
        screenshot_spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        static V: std::sync::LazyLock<semver::Version> =
            std::sync::LazyLock::new(|| semver::Version::new(1, 0, 0));
        &V
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let display = match args.get("display") {
            None | Some(Value::Null) => 0usize,
            Some(v) => v
                .as_f64()
                .filter(|n| *n >= 0.0 && n.fract() == 0.0)
                .map(|n| n as usize)
                .ok_or_else(|| failed("screenshot", "field `display` must be a >=0 integer"))?,
        };
        let max_dimension = (self.limits)().max_dimension;
        let png = tokio::task::spawn_blocking(move || -> Result<(u32, u32, Vec<u8>), String> {
            let screens = screenshots::Screen::all().map_err(|e| format!("枚举显示器失败: {e}"))?;
            let screen = screens.get(display).ok_or_else(|| {
                format!(
                    "display {display} 不存在（本机共 {} 个显示器，序号从 0 起）",
                    screens.len()
                )
            })?;
            let shot = screen.capture().map_err(|e| format!("截屏失败: {e}"))?;
            let (w, h) = shot.dimensions();
            // 截屏按 multimodal.maxDimension 缩边落库（0 = 不限）——机器产物
            // 的采集参数，不是用户文件转码（read_image 的拒收语义不适用；
            // 实抓：真屏 3440×1440 撞死 2048 上限，工具必死）。缩小同时压
            // 住 PNG 体积与 wire data URL 载荷。
            let png = if max_dimension > 0 && w.max(h) > max_dimension {
                let img = image::RgbaImage::from_raw(w, h, shot.into_raw())
                    .ok_or_else(|| "pixel buffer size mismatch".to_string())?;
                let scale = max_dimension as f64 / w.max(h) as f64;
                let (nw, nh) = (
                    ((w as f64 * scale).round() as u32).max(1),
                    ((h as f64 * scale).round() as u32).max(1),
                );
                let resized = image::imageops::resize(
                    &img,
                    nw,
                    nh,
                    image::imageops::FilterType::Triangle,
                );
                let mut out = std::io::Cursor::new(Vec::new());
                image::DynamicImage::ImageRgba8(resized)
                    .write_to(&mut out, image::ImageFormat::Png)
                    .map_err(|e| format!("PNG 编码失败: {e}"))?;
                (nw, nh, out.into_inner())
            } else {
                // 像素缓冲换底（into_raw 不依赖对方 image 版本类型一致）
                let png = rgba_to_png(w, h, shot.into_raw())
                    .map_err(|e| format!("PNG 编码失败: {e}"))?;
                (w, h, png)
            };
            Ok(png)
        })
        .await
        .map_err(|e| failed("screenshot", format!("截屏任务失败: {e}")))?
        .map_err(|e| failed("screenshot", e))?;
        let (w, h, png) = png;
        let name = format!("screenshot-{}.png", now_ms());
        let asset = self
            .assets
            .put(&png, "image/png", &name, &(self.limits)())
            .map_err(|e| failed("screenshot", e))?;
        Ok(json!({
            "asset": asset,
            "text": format!("screenshot captured: {w}x{h} (display {display})"),
            "width": w,
            "height": h,
        }))
    }
}

// ---- read_image ----

fn read_image_spec() -> &'static ToolSpec {
    static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "read_image".into(),
        description: "Read an image file from the workspace and attach it to the \
                      conversation so the model can see it (vision models only). `path` \
                      must be inside the allowed roots (data root + workspace roots). \
                      For PDFs use pdf_to_images; for text use fs tools. Pure local read."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "image file path (workspace-relative or absolute)" }
            },
            "required": ["path"]
        }),
        side_effect: SideEffect::Pure,
        exposure: Exposure::OnDemand,
    })
}

pub(crate) struct ReadImageTool {
    assets: Arc<AssetStore>,
    limits: LimitsFn,
}

impl ReadImageTool {
    pub(crate) fn new(assets: Arc<AssetStore>, limits: LimitsFn) -> Arc<Self> {
        Arc::new(Self { assets, limits })
    }
}

#[async_trait]
impl Tool for ReadImageTool {
    fn spec(&self) -> &ToolSpec {
        read_image_spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        static V: std::sync::LazyLock<semver::Version> =
            std::sync::LazyLock::new(|| semver::Version::new(1, 0, 0));
        &V
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let raw = args
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| failed("read_image", "missing non-empty string field `path`"))?
            .to_string();
        let limits = (self.limits)();
        let read = tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, String), String> {
            let roots = plugin_api::fsutil::allowed_roots()?;
            let resolved = plugin_api::fsutil::resolve_existing(std::path::Path::new(&raw), &roots)?;
            let bytes = std::fs::read(&resolved)
                .map_err(|e| format!("`{}`: {e}", resolved.display()))?;
            let name = resolved
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("image")
                .to_string();
            Ok((bytes, name))
        })
        .await
        .map_err(|e| failed("read_image", format!("读取任务失败: {e}")))?
        .map_err(|e| failed("read_image", e))?;
        let (bytes, name) = read;
        // 先验后写：非图片在落盘前拒绝（写出去再报错 = 泄漏副作用）
        let sniffed = validate::validate(&bytes, "", &limits)
            .map_err(|e| failed("read_image", e))?;
        if sniffed.dir != "images" {
            return Err(failed(
                "read_image",
                "read_image 只读图片文件；PDF 请用 pdf_to_images，其它文件用 fs 工具读取",
            ));
        }
        let dims = validate::image_dimensions(&bytes, sniffed.mime);
        let asset = self
            .assets
            .put(&bytes, sniffed.mime, &name, &limits)
            .map_err(|e| failed("read_image", e))?;
        let dims_text = dims
            .map(|(w, h)| format!(", {w}x{h}"))
            .unwrap_or_default();
        Ok(json!({
            "asset": asset,
            "text": format!("image loaded: {name} ({}, {} KB{})", sniffed.mime, bytes.len() / 1024, dims_text),
        }))
    }
}

// ---- 共享小件 ----

/// RGBA 像素缓冲 → PNG 字节（自持 image crate 编码，不依赖截图/PDF 侧的
/// image 类型一致；from_raw 尺寸不符 = Err 不 panic）。
pub(crate) fn rgba_to_png(w: u32, h: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    use std::io::Write as _;
    let img = image::RgbaImage::from_raw(w, h, rgba)
        .ok_or_else(|| format!("pixel buffer size mismatch: {w}x{h}"))?;
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::BufWriter::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
