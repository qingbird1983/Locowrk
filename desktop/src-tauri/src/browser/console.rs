//! 浏览器页签 console 回传账本（UI shell 文档 §2.5 浏览器页签，v2.144）：
//! 预览页里注入的桥接脚本把 console.error/warn 与页面异常 postMessage 给
//! 前端，前端经 `browser_report_errors` 推回这里——模型侧 `browser_console`
//! 工具读出，验证环（写页面 → 打开 → 看报错）闭环。纯内存环形缓冲：
//! 崩溃即失（页签关了缓冲也没意义，宁缺勿假不落盘）。

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 每页签保留条数（FIFO 溢出删最旧）：一个页面会话的错误量级。
const PER_TAB_CAP: usize = 200;
/// 追踪的页签数上限（最旧页签整体让位）：面板里同时存在的浏览器页签量级。
const TAB_CAP: usize = 8;
/// 单条文本钳制（与注入桥的客户端钳制同宽，双保险）。
const TEXT_CLIP: usize = 2000;

/// 单条 console 记录（wire 形态 camelCase 由 serde rename 承担：ts/level/text
/// 本就全小写）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEntry {
    /// Unix 毫秒（页面侧生成，保事件序）。
    pub ts: i64,
    /// `error` | `warn`（桥只回传这两档）。
    pub level: String,
    /// 序列化后的文本（页签侧已钳 2000 字符，这里再钳一道）。
    pub text: String,
}

/// 前端回传载荷（`browser_report_errors` 命令入口）。
#[derive(Debug, Deserialize)]
pub struct ConsoleReport {
    pub entries: Vec<ConsoleEntry>,
}

/// 账本：tab_id → 环形缓冲 + 页签到达序（供「最近页签」缺省）。
pub struct ConsoleStore {
    tabs: Mutex<BTreeMap<String, VecDeque<ConsoleEntry>>>,
    /// 页签首报到达序，最新在尾；溢出删头（连带缓冲整体让位）。
    order: Mutex<Vec<String>>,
    seq: AtomicU64,
}

fn clip(s: &str) -> String {
    s.chars().take(TEXT_CLIP).collect()
}

impl ConsoleStore {
    pub fn new() -> Self {
        Self {
            tabs: Mutex::new(BTreeMap::new()),
            order: Mutex::new(Vec::new()),
            seq: AtomicU64::new(0),
        }
    }

    /// 新页签 id（打开即占位：页面还没报错也能被 browser_console 点名读）。
    pub fn next_tab_id(&self) -> String {
        let n = self.seq.fetch_add(1, Ordering::Relaxed);
        format!(
            "br-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            n
        )
    }

    /// 回传入账：未知页签先登记（含打开占位补登记），溢出删最旧。
    pub fn register(&self, tab_id: &str) {
        let mut tabs = self.tabs.lock().expect("console tabs poisoned");
        if tabs.contains_key(tab_id) {
            return;
        }
        tabs.insert(tab_id.to_string(), VecDeque::new());
        let mut order = self.order.lock().expect("console order poisoned");
        order.push(tab_id.to_string());
        while order.len() > TAB_CAP {
            let evicted = order.remove(0);
            tabs.remove(&evicted);
        }
    }

    /// 追加一批记录（FIFO 钳每页签上限）。
    pub fn report(&self, tab_id: &str, entries: Vec<ConsoleEntry>) {
        self.register(tab_id);
        let mut tabs = self.tabs.lock().expect("console tabs poisoned");
        if let Some(buf) = tabs.get_mut(tab_id) {
            for e in entries {
                let entry = ConsoleEntry {
                    ts: e.ts,
                    level: e.level,
                    text: clip(&e.text),
                };
                if buf.len() == PER_TAB_CAP {
                    buf.pop_front();
                }
                buf.push_back(entry);
            }
        }
    }

    /// 读某页签（`tab_id` = None 取最近有缓冲的页签；都没有 = None）。
    /// 返回 (tab_id, 条目, truncated)。
    pub fn read(&self, tab_id: Option<&str>) -> Option<(String, Vec<ConsoleEntry>, bool)> {
        let tabs = self.tabs.lock().expect("console tabs poisoned");
        let (id, buf): (String, VecDeque<ConsoleEntry>) = match tab_id {
            Some(id) => (id.to_string(), tabs.get(id)?.clone()),
            None => {
                let order = self.order.lock().expect("console order poisoned");
                // 最近页签优先：到达序取尾往前找第一个还在账本里的
                let id = order.iter().rev().find(|id| tabs.contains_key(*id))?;
                (id.clone(), tabs.get(id)?.clone())
            }
        };
        let truncated = buf.len() >= PER_TAB_CAP;
        Some((id, buf.into_iter().collect(), truncated))
    }
}

#[cfg(test)]
mod tests;
