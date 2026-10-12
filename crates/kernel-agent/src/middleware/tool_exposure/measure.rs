//! 工具目录的 JSON 字节计量（审计 M-24c）：只回答"一个 `ToolSpec` 序列化成 JSON
//! 有多长、这些长度怎么拼成整个目录的长度、折合多少 token"。
//!
//! 为什么单住一处：本簇与暴露策略（打分 / 钉死 / 裁剪）无关，改计量口径（比如把
//! `chars/4` 换成真 tokenizer）不该牵动中间件文件；`tool_exposure.rs` 因此只留
//! 监听器本体与配置面（AGENTS「单文件建议不超过 400 行」的三问自检：本簇独立成
//! 模块后两侧各自"只做一件事"）。
//!
//! 病根（M-24c）：旧写法每请求把全目录（含 schema）整体序列化**两次**只为求一个
//! 长度——`to_string(&catalog).len()` 与 `to_string(&kept).len()`，两次都分配整串
//! 再丢掉。现改成逐项 `to_writer` 进计数 sink（**零分配**）算一遍，子集长度由下标
//! 求和得出。等值证明 = `../tests/scoring.rs` 的
//! `json_len_matches_whole_vector_serialization`（拿被删的旧实现当参考实现逐格对账，
//! 带"字节数 > 字符数"与"全目录比单项长"两条阳性对照）。

use plugin_api::ToolSpec;

/// `ToolSpec` 序列化成 JSON 后的**字节数**（`to_writer` 写进计数 sink，
/// 不分配整串）。失败按 0 计，与旧 `to_string(…).map_or(0, …)` 同口径。
pub(super) fn spec_json_len(spec: &ToolSpec) -> u64 {
    let mut sink = ByteCounter(0);
    serde_json::to_writer(&mut sink, spec).map_or(0, |_| sink.0)
}

/// 计数 sink：只累加写过的字节数（`write` 永不失败）。
struct ByteCounter(u64);

impl std::io::Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// `serde_json::to_string(&Vec<_>)` 的字节数，由**逐项长度**重构：紧凑输出无空格，
/// 故 n ≥ 1 时 = `[` + 各项 + (n−1) 个逗号 + `]` = Σ + n + 1；n = 0 时 = `[]` = 2。
/// 结果与整体序列化**逐字节等长**（等价测试见模块头注）。
pub(super) fn array_json_len(item_lens: &[u64]) -> u64 {
    if item_lens.is_empty() {
        return 2;
    }
    item_lens.iter().sum::<u64>() + item_lens.len() as u64 + 1
}

/// token 估算 = JSON 字节数 / 4（§9.6 的"约"，与 `estimate_tokens` 同一把尺）。
/// 取整**在求差之前**分别做——与旧 `spec_tokens(a).saturating_sub(spec_tokens(b))`
/// 同口径；先减后除会差最多 1（`tests/scoring.rs` 里这条口径由参考实现钉住）。
pub(super) fn tokens_of(json_len: u64) -> u64 {
    json_len / 4
}
