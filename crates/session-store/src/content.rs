//! 多模态内容部件（批 D，多模态设计 v2.0 §3）：用户消息从一维文本升级为
//! 「正文 + parts」。ToolUse/ToolResult **不进** ContentPart——工具协议维持
//! 内核中立事件（修订摘要 #1）；detail 旋钮 P2 记档；URL 内联形态不设
//! （引用统一走 AssetRef，宁缺勿假）。
//!
//! 落盘纪律：所有新字段 serde 字段级缺省——旧 jsonl/旧快照缺 `parts` 读入
//! = 纯文本消息，零迁移（Thinking/LlmUsage 先例同款）。

use serde::{Deserialize, Serialize};

/// 资产引用：内容寻址存储（`<数据根>/assets/`）里的一个文件。`id` =
/// 内容 sha256 hex（天然去重）；消息只存引用不存字节。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRef {
    pub id: String,
    pub mime: String,
    /// 原文件名（展示用；不入线上协议——file part 另带 filename）。
    pub name: String,
}

/// 消息内容部件。`tag = "type"` + snake_case 与会话事件同风格，日志即
/// schema。第一版输入侧只有文本/图片/PDF（音频 P2 记档，不设变体）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    Image {
        asset: AssetRef,
    },
    /// 第一版只收 PDF（兼容层映射 file part；不支持的中转站由门控拦截）。
    File {
        asset: AssetRef,
    },
}

impl ContentPart {
    /// 图片/文件部件的资产引用（Text = None）。
    pub fn asset(&self) -> Option<&AssetRef> {
        match self {
            ContentPart::Text { .. } => None,
            ContentPart::Image { asset } | ContentPart::File { asset } => Some(asset),
        }
    }

    /// 估算平铺值（多模态稿 §7.1）：图片 1024（锚点 = DeepSeek 官方单图
    /// token 上限）、文件 1000；真实计数以供应商 LlmUsage 为准——宁缺勿假
    /// 原则不变。Text 不平铺（正文本身已按字节估算）。
    pub fn est_tokens(&self) -> u64 {
        match self {
            ContentPart::Text { .. } => 0,
            ContentPart::Image { .. } => 1024,
            ContentPart::File { .. } => 1000,
        }
    }

    /// 转录占位（记忆只记文本事实）：`[图片: name]` / `[文件: name]`。
    pub fn placeholder(&self) -> Option<String> {
        match self {
            ContentPart::Text { .. } => None,
            ContentPart::Image { asset } => Some(format!("[图片: {}]", asset.name)),
            ContentPart::File { asset } => Some(format!("[文件: {}]", asset.name)),
        }
    }
}

/// 转录折叠（多模态稿 §2.2 记忆行）：正文 + parts 占位串接（占位在正文
/// 后，空格分隔；Text 部件不占位——正文即真相）。extract 转录 / 导出共用。
pub fn fold_parts(content: &str, parts: &[ContentPart]) -> String {
    let mut out = content.to_string();
    for p in parts {
        if let Some(ph) = p.placeholder() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&ph);
        }
    }
    out
}

#[cfg(test)]
mod tests;
