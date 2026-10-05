//! SSE 帧解析（LLM.md v3.0「流式」节）：按行缓冲，产出完整 `data:` 载荷。
//! 规范子集：事件以空行分隔；`data:` 行（可多行）拼接为一条载荷；`:` 开头
//! 注释 / keep-alive 行跳过；其余字段（event:/id:/retry:）OpenAI 兼容流不
//! 使用，忽略。`data: [DONE]` 是终止哨兵。

/// 一条凑齐的 SSE 载荷。`Done` = `[DONE]` 哨兵。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseEvent {
    Data(String),
    Done,
}

#[derive(Default)]
pub struct SseParser {
    /// 行缓冲（网络分块不保证按行对齐）。
    line: String,
    /// 当前事件已累积的 data 拼接（多行 data 之间以 \n 连接）。
    data: Option<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂一段网络字节（兼容端点均为 UTF-8 JSON SSE）。产出本次凑齐的完整
    /// 事件；不完整的行留在缓冲。
    pub fn feed(&mut self, chunk: &str, out: &mut Vec<SseEvent>) {
        for c in chunk.chars() {
            if c != '\n' {
                self.line.push(c);
                continue;
            }
            let line = std::mem::take(&mut self.line);
            self.feed_line(&line, out);
        }
    }

    /// 流结束时冲刷残留（缺尾空行的收束事件）。
    pub fn finish(&mut self, out: &mut Vec<SseEvent>) {
        let line = std::mem::take(&mut self.line);
        self.feed_line(&line, out);
        if let Some(data) = self.data.take() {
            self.dispatch(&data, out);
        }
    }

    fn feed_line(&mut self, line: &str, out: &mut Vec<SseEvent>) {
        if line.is_empty() {
            // 空行 = 事件边界：结算已累积的 data
            if let Some(data) = self.data.take() {
                self.dispatch(&data, out);
            }
            return;
        }
        if line.starts_with(':') {
            return; // 注释 / keep-alive
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            // 终止哨兵立即派发——不等到空行（供应商可能不发尾空行）
            if rest.trim() == "[DONE]" {
                self.data = None;
                out.push(SseEvent::Done);
                return;
            }
            match &mut self.data {
                Some(acc) => {
                    acc.push('\n');
                    acc.push_str(rest);
                }
                None => self.data = Some(rest.to_string()),
            }
        }
        // event:/id:/retry: 等字段忽略
    }

    fn dispatch(&self, data: &str, out: &mut Vec<SseEvent>) {
        if data.trim() == "[DONE]" {
            out.push(SseEvent::Done);
        } else {
            out.push(SseEvent::Data(data.to_string()));
        }
    }
}
