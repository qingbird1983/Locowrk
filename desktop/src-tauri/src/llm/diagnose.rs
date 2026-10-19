//! 前缀缓存断裂诊断（M6.1，KV v2 §7.4——ZCode 未做的差异化能力）：
//! 每 step 在 AppLlm 出口对**最终请求**做分块 hash 链，与会话上轮链比对，
//! 第一个不一致的块即断裂点。比全量 token diff 便宜一个量级，定位到块
//! 已够诊断（「system 变了 / 工具集变了 / 第 k 条消息变了」），无需
//! token 级精度。
//!
//! 分块两级：**粗块** = system / tools / 逐消息；**system 子块**按稳定
//! 协议标记切片（environment / repo_context / memory_index /
//! memory_guidance / [memory tree] 目录段；v2.139
//! workspace_context 升级为 environment——提示词分层设计 §4.2；S3 刀① 把
//! persistent_memory 与 project_memory_index 两块合成一个 memory_index）——
//! 记忆块与树目录的变化可以
//! 定位到段，不混在「system 变了」里。开关缺省关（开发者分区
//! `developer.cacheDiagnostics`），开启后
//! 每轮仅多几次 hash 计算，无额外请求。无会话身份的请求（记录员/摘要
//! 等一次性调用，纪律 6 本就不写缓存）不记录。

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use crate::timeutil::now_ms;
use kernel_agent::LlmRequest;
use plugin_api::ToolSpec;
use serde::Serialize;
use session_store::Message;

/// system 内稳定标记（按出现序切片；前一个标记起 = 后一个标记止）。
///
/// Q2（MR §4.1）加 `<project_memory_index>`：项目记忆索引是**会话内钉死**的段，
/// 但它中途变化（M2 落库后 reconcile / 管理页编辑触发重算）就是前缀断裂，
/// 必须能定位到段而不是混进"system 变了"。
///
/// ⚠ **S3 刀① 换表**：`<persistent_memory>` 与 `<project_memory_index>` 两块合并成
/// 一个 `<memory_index>`（两层索引段），所以这张表从 6 枚变 5 枚、顺序跟着组装序走
/// （environment → repo_context → memory_index → memory_guidance → 目录段）。
/// 这一处是**观测通道**不是装饰：漏改它，索引段中途变化会被报成"system 变了"，
/// 缓存诊断从此看不见这一段的断裂——而它正是刀② 要把 mtime 接进缓存键之前
/// 最需要盯住的那一格。
const SYSTEM_MARKERS: [&str; 5] = [
    "<environment>",
    "<repo_context>",
    "<memory_index>",
    "<memory_guidance>",
    "[memory tree]",
];

fn hash_str(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn hash_tools(tools: &[ToolSpec]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for t in tools {
        t.name.hash(&mut h);
        t.description.hash(&mut h);
        t.schema.to_string().hash(&mut h);
    }
    h.finish()
}

fn message_text(m: &Message) -> String {
    // 序列化整条做 hash：Assistant 的 tool_calls / ToolResult 的 output
    // 变化同样是前缀字节变化，不能只看 content。
    serde_json::to_string(m).unwrap_or_else(|_| "<unserializable>".into())
}

/// 单 step 的 hash 链。
#[derive(Debug, Clone)]
pub struct StepChain {
    /// system 整块 + 子块（标记名, hash）——子块按标记出现序。
    pub system_sub: Vec<(String, u64)>,
    pub tools: u64,
    pub messages: Vec<u64>,
}

/// system 按稳定标记切片（不要求闭合标签配对——guidance/树目录本就
/// 无闭合；块 = 标记起到下一标记止，头部剩余 = "base"）。
fn slice_system(system: &str) -> Vec<(String, u64)> {
    let mut marks: Vec<(usize, &str)> = SYSTEM_MARKERS
        .iter()
        .filter_map(|m| system.find(m).map(|i| (i, *m)))
        .collect();
    marks.sort_by_key(|(i, _)| *i);
    let mut out = Vec::new();
    let base_end = marks.first().map(|(i, _)| *i).unwrap_or(system.len());
    out.push(("base".to_string(), hash_str(&system[..base_end])));
    for (idx, (start, name)) in marks.iter().enumerate() {
        let end = marks.get(idx + 1).map(|(i, _)| *i).unwrap_or(system.len());
        out.push((name.to_string(), hash_str(&system[*start..end])));
    }
    out
}

impl StepChain {
    pub fn capture(req: &LlmRequest) -> Self {
        Self {
            system_sub: req.system.as_deref().map(slice_system).unwrap_or_default(),
            tools: hash_tools(&req.tools),
            messages: req
                .messages
                .iter()
                .map(|m| hash_str(&message_text(m)))
                .collect(),
        }
    }
}

/// 一次 step 的诊断（面板数据源；breaks 空 = 前缀健康纯追加）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakReport {
    /// 会话内 step 序号（从 1 起）。
    pub step: u64,
    /// 断裂块名（首个在前）：base/system 子块名 / "tools" /
    /// "message#k" / "messages-truncated"。
    pub breaks: Vec<String>,
    pub prev_messages: usize,
    pub messages: usize,
    pub ts_ms: i64,
}

impl BreakReport {
    pub fn healthy(&self) -> bool {
        self.breaks.is_empty()
    }
}

/// 会话诊断账本（AppLlm 持有；FIFO 128 会话，id 不复用残留无害）。
#[derive(Default)]
pub struct Diagnoser {
    prev: Mutex<HashMap<String, (StepChain, u64)>>,
    last: Mutex<HashMap<String, BreakReport>>,
}

impl Diagnoser {
    fn fifo_push(map: &mut HashMap<String, (StepChain, u64)>, key: String, val: (StepChain, u64)) {
        if map.len() >= 128 && !map.contains_key(&key) {
            let oldest = map.keys().next().cloned().expect("len checked non-empty");
            map.remove(&oldest);
        }
        map.insert(key, val);
    }

    /// 记录一步并产出诊断。无会话身份 = None（一次性请求不记）。
    pub fn record(&self, req: &LlmRequest) -> Option<BreakReport> {
        let session_id = req.session_id.as_deref().filter(|s| !s.is_empty())?;
        let chain = StepChain::capture(req);
        let mut prev = self.prev.lock().expect("diagnose prev poisoned");
        let step = prev.get(session_id).map(|(_, s)| s + 1).unwrap_or(1);
        let mut breaks = Vec::new();
        if let Some((p, _)) = prev.get(session_id) {
            // system 子块：按名字配对比（块出现/消失也算断）
            let cur: HashMap<&str, u64> = chain
                .system_sub
                .iter()
                .map(|(n, h)| (n.as_str(), *h))
                .collect();
            let old: HashMap<&str, u64> =
                p.system_sub.iter().map(|(n, h)| (n.as_str(), *h)).collect();
            for (name, h) in &cur {
                if old.get(name).is_some_and(|ph| ph != h) {
                    breaks.push((*name).to_string());
                }
            }
            for name in old.keys() {
                if !cur.contains_key(name) {
                    breaks.push(format!("{name} (removed)"));
                }
            }
            if p.tools != chain.tools {
                breaks.push("tools".to_string());
            }
            // 逐消息：第一个不一致位即断点（前缀缓存只看字节前缀）
            for (i, h) in chain.messages.iter().enumerate() {
                match p.messages.get(i) {
                    Some(ph) if ph == h => {}
                    Some(_) => {
                        breaks.push(format!("message#{i}"));
                        break;
                    }
                    None => break, // 纯追加，健康
                }
            }
            if chain.messages.len() < p.messages.len() {
                breaks.push("messages-truncated".to_string());
            }
        }
        let report = BreakReport {
            step,
            breaks,
            prev_messages: prev
                .get(session_id)
                .map(|(c, _)| c.messages.len())
                .unwrap_or(0),
            messages: chain.messages.len(),
            ts_ms: now_ms(),
        };
        Self::fifo_push(&mut prev, session_id.to_string(), (chain, step));
        drop(prev);
        self.last
            .lock()
            .expect("diagnose last poisoned")
            .insert(session_id.to_string(), report.clone());
        Some(report)
    }

    /// 面板拉取：会话最近一步诊断（没记过 = None）。
    pub fn last_of(&self, session_id: &str) -> Option<BreakReport> {
        self.last
            .lock()
            .expect("diagnose last poisoned")
            .get(session_id)
            .cloned()
    }
}

#[cfg(test)]
mod tests;
