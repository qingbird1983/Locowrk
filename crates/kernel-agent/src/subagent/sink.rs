//! 子会话转录接收器（§3.16，v2.52）：收束时把子**自身段**交给宿主持久化
//! ——「子会话也落盘」通路的宿主侧出口。黑盒语义原样：事件只写给宿主
//! （桌面写 `<数据根>/sub-transcripts/`），不投影进父转录；headless 不注入
//! = 维持纯黑盒，行为与 ⑮ 完全一致。

use session_store::Session;

pub trait SubTranscriptSink: Send + Sync {
    /// 子 Agent 收束（无论成败）调用一次：`own_from..` 为子自身段（任务
    /// prompt 起），fork 来的父前缀不包含。实现方自行决定去留。
    fn close(&self, child_session_id: &str, own_from: usize, child: &Session);
}
