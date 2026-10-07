//! 方言测试：每方言 patch 形状锁死（字段名/值结构）；DeepSeek 官方归一表；
//! 映射不了 = None；clamp 就近降档 / 剥除；aux 最低档；host → 方言。

use super::*;
use serde_json::json;

#[test]
fn openai_effort_patches_reasoning_effort() {
    let d = Dialect::OpenAiEffort;
    assert_eq!(d.ladder(), ["minimal", "low", "medium", "high"]);
    assert_eq!(
        d.patch("low"),
        Some(vec![("reasoning_effort", json!("low"))])
    );
    assert_eq!(d.patch("disabled"), None, "openai_effort 无关闭档");
    assert_eq!(d.patch("max"), None);
}

#[test]
fn anthropic_effort_patches_reasoning_effort() {
    let d = Dialect::AnthropicEffort;
    assert_eq!(d.ladder(), ["low", "medium", "high"]);
    assert_eq!(
        d.patch("high"),
        Some(vec![("reasoning_effort", json!("high"))])
    );
    assert_eq!(d.patch("enabled"), None);
}

#[test]
fn glm_thinking_patches_type_object() {
    let d = Dialect::GlmThinking;
    assert_eq!(d.ladder(), ["disabled", "enabled"]);
    assert_eq!(
        d.patch("disabled"),
        Some(vec![("thinking", json!({"type": "disabled"}))])
    );
    assert_eq!(
        d.patch("enabled"),
        Some(vec![("thinking", json!({"type": "enabled"}))])
    );
    assert_eq!(d.patch("low"), None);
}

#[test]
fn qwen_enable_patches_bool() {
    let d = Dialect::QwenEnable;
    assert_eq!(
        d.patch("enabled"),
        Some(vec![("enable_thinking", json!(true))])
    );
    assert_eq!(
        d.patch("disabled"),
        Some(vec![("enable_thinking", json!(false))])
    );
}

#[test]
fn deepseek_dual_field_with_official_normalization() {
    let d = Dialect::DeepseekThinking;
    assert_eq!(d.ladder(), ["disabled", "low", "high", "max"]);
    assert_eq!(
        d.patch("disabled"),
        Some(vec![("thinking", json!({"type": "disabled"}))])
    );
    // low/high/max：thinking enabled + reasoning_effort 双字段
    let (fields, effort) = {
        let p = d.patch("low").unwrap();
        (p[0].0, p[1].1.clone())
    };
    assert_eq!(fields, "thinking");
    assert_eq!(effort, json!("low"));
    let p = d.patch("max").unwrap();
    assert_eq!(p[1].1, json!("max"));
    assert_eq!(p[1].0, "reasoning_effort");
    // 官方归一：minimal→low、medium→high、xhigh/ultra→max
    assert_eq!(d.patch("minimal").unwrap()[1].1, json!("low"));
    assert_eq!(d.patch("medium").unwrap()[1].1, json!("high"));
    assert_eq!(d.patch("xhigh").unwrap()[1].1, json!("max"));
}

#[test]
fn none_dialect_has_no_ladder_and_no_patch() {
    assert_eq!(Dialect::None.ladder(), &[] as &[&str]);
    assert_eq!(Dialect::None.patch("low"), None);
    assert_eq!(Dialect::None.aux_level(), None);
    assert_eq!(Dialect::None.clamp("low"), None);
}

#[test]
fn clamp_exact_nearest_lower_or_strip() {
    let d = Dialect::DeepseekThinking;
    assert_eq!(d.clamp("low"), Some("low"));
    // 不在梯子：就近降档（语义序取最后一个低于请求档的梯子档）
    assert_eq!(
        d.clamp("minimal"),
        Some("disabled"),
        "minimal 之下只有 disabled"
    );
    assert_eq!(
        d.clamp("medium"),
        Some("low"),
        "medium 降 = low（high 不可达）"
    );
    assert_eq!(d.clamp("xhigh"), Some("high"));
    assert_eq!(
        d.clamp("enabled"),
        Some("max"),
        "enabled 语义序最后 → 降到 max"
    );
    // 没有更低档 = 剥除
    assert_eq!(
        Dialect::AnthropicEffort.clamp("disabled"),
        None,
        "anthropic 梯子无 disabled 之下的档 → 剥除"
    );
}

#[test]
fn aux_level_is_lowest_rung() {
    assert_eq!(Dialect::DeepseekThinking.aux_level(), Some("disabled"));
    assert_eq!(Dialect::GlmThinking.aux_level(), Some("disabled"));
    assert_eq!(Dialect::OpenAiEffort.aux_level(), Some("minimal"));
    assert_eq!(Dialect::AnthropicEffort.aux_level(), Some("low"));
}

#[test]
fn dialect_of_resolves_preset_hosts() {
    assert_eq!(
        dialect_of(Some("https://api.deepseek.com")),
        Dialect::DeepseekThinking
    );
    assert_eq!(
        dialect_of(Some("https://open.bigmodel.cn/api/paas/v4")),
        Dialect::GlmThinking
    );
    assert_eq!(
        dialect_of(Some("https://dashscope.aliyuncs.com/compatible-mode/v1")),
        Dialect::QwenEnable
    );
    assert_eq!(
        dialect_of(Some("https://api.openai.com/v1")),
        Dialect::OpenAiEffort
    );
    // openrouter 在表但方言空 = None（宁缺勿假）
    assert_eq!(
        dialect_of(Some("https://openrouter.ai/api/v1")),
        Dialect::None
    );
    // 不在表 / 空 / 缺失 = None
    assert_eq!(
        dialect_of(Some("https://relay.example.com/v1")),
        Dialect::None
    );
    assert_eq!(dialect_of(None), Dialect::None);
}
