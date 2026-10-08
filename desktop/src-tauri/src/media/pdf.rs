//! `pdf_to_images`（批 E，多模态设计 v2.0 §8.2）：PDF 逐页转 PNG 进资产
//! 库，页图片数组随 output 走 §8.1 投影（vision 模型逐页读）。pdfium 动态
//! 库运行时绑定（`resources/pdfium/`，bblanchon/pdfium-binaries 构建
//! chromium/8086）——**库缺席时本工具不进目录**（组装点条件注册，宁缺勿
//! 假）；绑定失败同样如实报错不静默。
//!
//! 页数上限 `max_pages`（缺省 8，硬顶 20）防 token 失控：每页一张图 = 每
//! 页 ≈1024 token 估算（批 D est 锚点），20 页已是一张 4-5MB 截图的量级。

use std::sync::Arc;

use async_trait::async_trait;
use pdfium_render::prelude::*;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{json, Value};

use super::{failed, LimitsFn};
use crate::assets::{validate, AssetStore};

/// 页数硬顶（token 成本闸；20 页 ≈ 20K token 估算量级）。
pub const MAX_PAGES_HARD_CAP: u64 = 20;
/// 缺省渲染页数。
const DEFAULT_PAGES: u64 = 8;
/// 页位图目标宽上限（px）；实际取值再被 multimodal.maxDimension 与页
/// 纵横比收窄（实抓：A4 纵向宽 1600 → 高 2071 撞死 2048 单边上限被
/// put 拒收——目标宽按 cap×宽高比预算，保证长短边都不超）。
const TARGET_WIDTH: u32 = 1600;

/// 渲染目标宽（纯函数，单测覆盖）：宽 ≤ `TARGET_WIDTH` 且按页纵横比
/// 折算的高 ≤ `cap`（cap 0 = 只受 TARGET_WIDTH 约束）。**floor 且留 1px
/// 余量**——pdfium 位图尺寸向上取整（实抓：Letter 612×792 → 目标宽
/// 1583 渲染出 2049 高，2048.06 进位撞死 2048 上限被 put 拒收）。
fn target_width(w_pt: f64, h_pt: f64, cap: u32) -> u32 {
    if w_pt <= 0.0 || h_pt <= 0.0 {
        return TARGET_WIDTH.min(cap.max(1));
    }
    let mut w = TARGET_WIDTH as f64;
    if cap > 0 {
        // 高度约束：w ≤ (cap−1) × (w/h)——1px 余量吸收位图进位
        w = w.min((cap as f64 - 1.0) * w_pt / h_pt);
        // 宽度自身也不得过 cap（极端横版）
        w = w.min((cap as f64 - 1.0).max(1.0));
    }
    (w.floor() as u32).max(1)
}

/// 进程级 pdfium 单例（实抓：`bind_to_library` 全局绑定只许一次，第二次
/// 报 AlreadyInitialized——每调用绑定必死；首次失败同样缓存，同进程不再
/// 反复碰库）。thread_safe feature 缺省开 = Pdfium 跨线程共享。
fn pdfium_instance(lib: &std::path::Path) -> Result<Arc<Pdfium>, String> {
    static PDFIUM: std::sync::OnceLock<Result<Arc<Pdfium>, String>> = std::sync::OnceLock::new();
    PDFIUM
        .get_or_init(|| {
            Pdfium::bind_to_library(lib)
                .map(|b| Arc::new(Pdfium::new(b)))
                .map_err(|e| format!("pdfium 动态库绑定失败（{}）: {e}", lib.display()))
        })
        .clone()
}

fn spec() -> &'static ToolSpec {
    static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "pdf_to_images".into(),
        description: "Convert the first pages of a PDF file into images and attach them \
                      to the conversation so the model can read them visually (vision \
                      models only). `path` must be inside the allowed roots; optional \
                      `max_pages` (default 8, hard cap 20). Pure local read."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "PDF file path (workspace-relative or absolute)" },
                "max_pages": { "type": "number", "description": "pages to convert (default 8, cap 20)" }
            },
            "required": ["path"]
        }),
        side_effect: SideEffect::Pure,
        exposure: Exposure::OnDemand,
    })
}

pub(crate) struct PdfToImagesTool {
    assets: Arc<AssetStore>,
    limits: LimitsFn,
    lib_path: std::path::PathBuf,
}

impl PdfToImagesTool {
    pub(crate) fn new(
        assets: Arc<AssetStore>,
        limits: LimitsFn,
        lib_path: std::path::PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self {
            assets,
            limits,
            lib_path,
        })
    }
}

/// 渲染页数计划（纯函数，单测覆盖）：缺省 8、入参钳 1..=20、再被总页数
/// 截断（宁少勿假——不存在的页不渲染也不报错）。
fn render_plan(max_pages: Option<u64>, total: usize) -> usize {
    let want = max_pages.unwrap_or(DEFAULT_PAGES).clamp(1, MAX_PAGES_HARD_CAP);
    (want as usize).min(total)
}

#[async_trait]
impl Tool for PdfToImagesTool {
    fn spec(&self) -> &ToolSpec {
        spec()
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
            .ok_or_else(|| failed("pdf_to_images", "missing non-empty string field `path`"))?
            .to_string();
        let max_pages = match args.get("max_pages") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_f64()
                    .filter(|n| *n >= 1.0 && n.fract() == 0.0)
                    .map(|n| n as u64)
                    .ok_or_else(|| {
                        failed("pdf_to_images", "field `max_pages` must be a >=1 integer")
                    })?,
            ),
        };
        let limits = (self.limits)();
        let lib = self.lib_path.clone();
        let assets = Arc::clone(&self.assets);
        let rendered = tokio::task::spawn_blocking(move || -> Result<PdfOutcome, String> {
            let roots = plugin_api::fsutil::allowed_roots()?;
            let resolved =
                plugin_api::fsutil::resolve_existing(std::path::Path::new(&raw), &roots)?;
            let bytes = std::fs::read(&resolved)
                .map_err(|e| format!("`{}`: {e}", resolved.display()))?;
            // 先验后转：非 PDF / 超限在 pdfium 介入前拒绝
            let sniffed =
                validate::validate(&bytes, "", &limits).map_err(|e| e.to_string())?;
            if sniffed.ext != "pdf" {
                return Err(format!(
                    "pdf_to_images 只吃 PDF 文件（`{}` 实为 .{}）",
                    resolved.display(),
                    sniffed.ext
                ));
            }
            let stem = resolved
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("pdf")
                .to_string();
            let pdfium = pdfium_instance(&lib)?;
            let document = pdfium
                .load_pdf_from_byte_vec(bytes, None)
                .map_err(|e| format!("PDF 打开失败: {e}"))?;
            // pdfium-render 页序号是 i32（PdfPageIndex）；本crate内先归一
            // usize 做计划，回取时再转回（render_plan 上限 20，无溢出面）
            let total = document.pages().len().max(0) as usize;
            let n = render_plan(max_pages, total);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let page = document
                    .pages()
                    .get(i as i32)
                    .map_err(|e| format!("第 {} 页读取失败: {e}", i + 1))?;
                let width = target_width(
                    page.width().value as f64,
                    page.height().value as f64,
                    limits.max_dimension,
                );
                let config = PdfRenderConfig::new().set_target_width(width as i32);
                let png = page
                    .render_with_config(&config)
                    .and_then(|b| b.as_image())
                    .map_err(|e| format!("第 {} 页渲染失败: {e}", i + 1))?;
                let mut buf = std::io::Cursor::new(Vec::new());
                png.write_to(&mut buf, image::ImageFormat::Png)
                    .map_err(|e| format!("第 {} 页 PNG 编码失败: {e}", i + 1))?;
                let png = buf.into_inner();
                let name = format!("{stem}-p{:02}.png", i + 1);
                let asset = assets
                    .put(&png, "image/png", &name, &limits)
                    .map_err(|e| e.to_string())?;
                out.push(asset);
            }
            Ok(PdfOutcome { total, assets: out })
        })
        .await
        .map_err(|e| failed("pdf_to_images", format!("转换任务失败: {e}")))?
        .map_err(|e| failed("pdf_to_images", e))?;
        let n = rendered.assets.len();
        Ok(json!({
            "assets": rendered.assets,
            "text": format!(
                "converted {n} of {} pages to images (page images attached in order)",
                rendered.total
            ),
            "pages_total": rendered.total,
            "pages_rendered": n,
        }))
    }
}

struct PdfOutcome {
    total: usize,
    assets: Vec<session_store::AssetRef>,
}

#[cfg(test)]
mod tests {
    use super::{render_plan, target_width};

    #[test]
    fn render_plan_clamps_and_truncates() {
        assert_eq!(render_plan(None, 100), 8, "缺省 8");
        assert_eq!(render_plan(Some(3), 100), 3);
        assert_eq!(render_plan(Some(0), 100), 1, "下限 1");
        assert_eq!(render_plan(Some(999), 100), 20, "硬顶 20");
        assert_eq!(render_plan(Some(10), 4), 4, "总页数截断");
        assert_eq!(render_plan(Some(5), 0), 0, "空文档零页");
    }

    #[test]
    fn target_width_respects_dimension_cap_by_aspect() {
        // A4 纵向 595×842pt：宽 1600 → 高 2071 曾撞死 2048 上限
        let w = target_width(595.0, 842.0, 2048);
        assert!(w <= 2048 * 595 / 842 + 1, "width={w}");
        let h = 842.0 * (w as f64) / 595.0;
        assert!(h <= 2048.0, "height={h}");
        // 横版：宽自身不得超 cap
        let wl = target_width(842.0, 595.0, 2048);
        assert!(wl <= 2048);
        // cap 0 = 只受 TARGET_WIDTH 约束
        assert_eq!(target_width(595.0, 842.0, 0), 1600);
    }
}
