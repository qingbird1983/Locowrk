//! `MEMORY.md` 索引的生成面（一文件一行 + 设计 §2.1 的**写侧预算钳**）。
//!
//! 为什么单独一片：索引要做的是"扫整个键位目录 + 算预算 + 落一个文件"，而
//! `mod.rs` 那侧做的是"一条事实的布局与渲染"——两件事，两种改法。分片后对外
//! 路径仍由 `super` re-export（`memlayout::refresh_index` 一字未动），消费点
//! （宿主 `memory_files` / `memory_reconcile` 与本模块 `write_fact`）零改动。

use std::path::Path;

use super::{one_line, scope_dirs, sync};

/// 从 frontmatter 读回 description（索引 hook 用；无 = None）。只服务索引，
/// 所以住在这片；整块 frontmatter 的读回面在 `sync::parse_fact`（那边字段全）。
fn parse_description(text: &str) -> Option<String> {
    for line in text.lines().skip(1) {
        if line.trim_end() == "---" {
            break;
        }
        if let Some(v) = line.strip_prefix("description: ") {
            return Some(sync::yaml_unquote(v));
        }
    }
    None
}

/// 索引硬顶：指针行数。**这条预算数字的全仓唯一出处就在这里**——写侧（本文件
/// `refresh_index` 落盘）与注入侧（宿主 `memory_index`）共用同一份，两份各写
/// 一份迟早漂成"文件里 210 行、注入只给看 200 行"，而人打开文件夹看到的那份
/// 就不再是模型看到的那份（判据 = `check-single-source` 的 SS10 / SS10b）。
pub const MAX_INDEX_LINES: usize = 200;
/// 索引硬顶：正文字符数（与行数任一先撞线就截，长 key 会先撞字符预算）。
pub const MAX_INDEX_CHARS: usize = 25_000;
/// 截断必须自描述：末行留一条指路，否则人打开索引会以为"本项目就记了这些"。
pub const INDEX_TRUNCATED_NOTE: &str =
    "… index truncated — open this folder to see every file, or use memory_search";

/// 写侧预算钳：行数与字符数**任一先撞线即截**，且只丢整行不写半行。
/// 返回 `(保留的指针行, 是否发生截断)`。头部与每行行尾换行都算进字符预算——
/// 头部不算就会被它悄悄撑爆，行尾不算则 200 行实际比预算多出 200 字符。
fn clamp_index(lines: Vec<String>) -> (Vec<String>, bool) {
    let head = "# 记忆索引\n\n";
    let mut kept: Vec<String> = Vec::new();
    let mut used = head.chars().count();
    for line in lines {
        let cost = line.chars().count() + 1;
        if kept.len() >= MAX_INDEX_LINES || used + cost > MAX_INDEX_CHARS {
            return (kept, true);
        }
        used += cost;
        kept.push(line);
    }
    (kept, false)
}

/// 重建一个键位的 MEMORY.md 索引（按键位算目录 ⇒ 写侧用这条）。
pub fn refresh_index(root: &Path, workspace: &str) -> Result<(), String> {
    let (dir, index_path) = scope_dirs(root, workspace)?;
    refresh_layer(&dir, &index_path)
}

/// 重建**已知某一层**的索引（拿事实目录 + 索引路径，不按键位算）。
///
/// 为什么要有这一条而不是都走 [`refresh_index`]：项目层里"这个文件属于哪个权威
/// workspace 串"只有按键位算的那条路能回答，而读侧与清空侧手里只有磁盘在场
/// （[`super::layers`] 给的正是目录，不是键位）。硬要反推就会拿目录名再去拼一次
/// 路径 ⇒ 指到别的目录，删不掉也不报错。
/// facts 目录不存在时**照旧创建**（与改前的 `refresh_index` 逐字节同形：项目层的
/// 索引坐在 `facts/` 的上一层，只建父目录会让"空 facts 自描述"这个既有形状变掉）。
pub(crate) fn refresh_layer(facts_dir: &Path, index_path: &Path) -> Result<(), String> {
    let mut lines: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(facts_dir) {
        for f in entries.flatten() {
            let name = f.file_name();
            let Some(stem) = name.to_str().and_then(|s| s.strip_suffix(".md")) else {
                continue;
            };
            if stem == "MEMORY" {
                continue;
            }
            let desc = std::fs::read(f.path())
                .ok()
                .and_then(|b| {
                    parse_description(&String::from_utf8_lossy(&b)).map(|d| one_line(&d, 80))
                })
                .unwrap_or_default();
            lines.push(format!("- [{stem}]({stem}.md) — {desc}"));
        }
    }
    lines.sort();
    let (kept, truncated) = clamp_index(lines);
    let mut body = format!("# 记忆索引\n\n{}", kept.join("\n"));
    if truncated {
        body.push_str(&format!("\n{INDEX_TRUNCATED_NOTE}"));
    }
    std::fs::create_dir_all(facts_dir)
        .map_err(|e| format!("memory 目录创建失败 {}: {e}", facts_dir.display()))?;
    sync::atomic_write(index_path, &format!("{body}\n"))
        .map_err(|e| format!("memory 索引写入失败 {}: {e}", index_path.display()))
}
