//! LLM 文本流式通道（⑳，DESIGN §7.2/§12.2）：delta 出口契约与黑盒替身。
//!
//! 内核每个 step 调 LLM 时：`LlmStreamSink` 在册 → 走
//! `LlmService::complete_streaming`（文本 delta 逐段回调）；缺席 → 纯
//! 非流式（`complete`）。delta 只是实时通道——会话日志与投影只认
//! `LlmResponse`（返回值），磁盘镜像/审计不受影响。

/// 文本 delta 出口：组装点注册（桌面端实现直推 `agent://chunk`，
/// runner 抑制该步的整段投影防重复）。
pub trait LlmStreamSink: Send + Sync + 'static {
    /// 返回 true = delta 已被真实消费（直推 UI）；false = 黑盒吞掉
    /// （子内核替身）——内核据此判定 StepOutcome::streamed，被吞掉的
    /// 轮次仍走整段投影（不过子会话本就不投影，双保险）。
    fn on_delta(&self, session_id: Option<&str>, delta: &str) -> bool;

    /// 推理 delta 出口（v2.57，同 on_delta 语义）：供应商 reasoning 逐段
    /// 回调。默认 false（不消费）——推理照常整段落 `SessionEvent::Thinking`
    /// 由投影方投出；消费了则内核以 StepOutcome::reasoned 通知投影方抑制。
    /// 不实现即自动获得"推理只落日志"的保守行为（headless 零变化）。
    fn on_reasoning(&self, _session_id: Option<&str>, _delta: &str) -> bool {
        false
    }

    /// 中止查询（v4.11 停止生成）：组装点实现按会话查中止标记；内核在
    /// LLM 适配器的**每个 delta 回调点**把 `on_delta`/`on_reasoning` 闭包的
    /// 返回值（= 本方法）交给适配器流循环——true = 用户请求停止，适配器
    /// 尽快结束本次调用（已收到的部分正文原样返回，不虚构结尾）。默认
    /// false：headless / 未装配停止面的路径零变化。
    fn should_stop(&self, _session_id: Option<&str>) -> bool {
        false
    }
}

/// 黑盒替身（⑮ 子 Agent 语义保持）：`child_kernel` 把它 scoped 换入压制
/// 全局 sink——子会话内的 delta 不得泄漏到父 UI（子内活动不投影）。
pub struct NoLlmStreamSink;

impl LlmStreamSink for NoLlmStreamSink {
    fn on_delta(&self, _: Option<&str>, _: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests;
