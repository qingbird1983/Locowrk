//! 目录匹配（批 A，供应商稿 §4.3，取 PI 打分制弃正则层）：供应商定位
//! （预设 host 别名表 +100 / 快照 api host 精确或后缀 +80）→ 归一化
//! 模型 id 精确命中。**无供应商定位 = 无匹配**——中转站跨发布商「借池」
//! 不做（记档），宁缺勿假。模型 id 归一化剥 `-latest`/日期戳/`@region`。

use std::collections::HashMap;

use super::catalog::{CatalogModel, CatalogSnapshot};

/// 预设 host → (models.dev 供应商键, 推理方言)（供应商稿 §4.3「预设别名
/// 表」+ §3.2 方言接线）。键 = 预设 baseUrl 的 host[:port]（与
/// lib/provider-presets.ts 对齐，测试锁两边）。快照里没有的供应商（百川/
/// 零一）不进供应商列——匹配回落 None 如实未知；方言列 `""` = 无梯子
/// （openrouter 等「目录补」预设宁缺勿假，批 B 不发明兼容写法）。
const PRESET_HOST_PROVIDER: &[(&str, &str, &str)] = &[
    ("api.deepseek.com", "deepseek", "deepseek_thinking"),
    ("open.bigmodel.cn", "zhipuai", "glm_thinking"),
    ("api.z.ai", "zai", "glm_thinking"),
    ("api.moonshot.cn", "moonshotai", ""),
    ("api.minimaxi.com", "minimax", ""),
    ("api.siliconflow.cn", "siliconflow-cn", ""),
    ("ark.cn-beijing.volces.com", "volcengine", ""),
    ("dashscope.aliyuncs.com", "alibaba-cn", "qwen_enable"),
    ("api.xiaomimimo.com", "xiaomi", ""),
    ("api.openai.com", "openai", "openai_effort"),
    ("api.anthropic.com", "anthropic", "anthropic_effort"),
    ("openrouter.ai", "openrouter", ""),
    ("api.groq.com", "groq", ""),
    ("api.x.ai", "xai", ""),
    ("api.mistral.ai", "mistral", ""),
    ("api.together.xyz", "togetherai", ""),
    ("api.fireworks.ai", "fireworks-ai", ""),
    ("integrate.api.nvidia.com", "nvidia", ""),
    ("inference.baseten.co", "baseten", ""),
    ("api.cerebras.ai", "cerebras", ""),
    ("router.huggingface.co", "huggingface", ""),
    ("generativelanguage.googleapis.com", "google", ""),
    ("api.stepfun.com", "stepfun", ""),
    ("localhost:1234", "lmstudio", ""),
    ("127.0.0.1:1234", "lmstudio", ""),
];

/// host → 预设方言（批 B 推理档位接线；None = 不在预设表 = 无梯子）。
pub fn dialect_of_host(host: &str) -> Option<&'static str> {
    PRESET_HOST_PROVIDER
        .iter()
        .find(|(h, _, _)| *h == host)
        .map(|(_, _, d)| *d)
        .filter(|d| !d.is_empty())
}

/// 供应商定位打分：`(score, host_exact)`，score 0 = 未定位。
fn provider_score(
    base_host: &str,
    provider_key: &str,
    api_index: &HashMap<&str, &str>,
) -> (u32, bool) {
    for (host, key, _) in PRESET_HOST_PROVIDER {
        if *key == provider_key && *host == base_host {
            return (100, true);
        }
    }
    if let Some(api) = api_index.get(provider_key) {
        let api_host = host_of(api);
        if api_host == base_host {
            return (80, true);
        }
        // 后缀命中（子域互属，如同一根域的不同区域端点）
        if base_host.ends_with(&format!(".{api_host}"))
            || api_host.ends_with(&format!(".{base_host}"))
        {
            return (80, false);
        }
    }
    (0, false)
}

/// URL → host[:port]（小写；解析失败空串）。保留端口：本地端点
/// localhost:1234 与 127.0.0.1:1234 靠端口参与区分。
pub fn host_of(url: &str) -> String {
    let s = url.trim();
    let s = s.split("://").nth(1).unwrap_or(s);
    let host = s.split(['/', '?', '#']).next().unwrap_or("");
    host.trim_end_matches('/').to_lowercase()
}

/// 模型 id 归一化（供应商稿 §4.3）：小写；取最后一个 '/' 后段；剥
/// `@region` 后缀；循环剥 `-latest` 与 `-YYYY-MM-DD` / `-YYYYMMDD` 日期
/// 戳；修剪尾 `-`。无正则（家法：不加依赖）。
pub fn normalize_id(id: &str) -> String {
    let mut s = id.trim().to_lowercase();
    if let Some(idx) = s.rfind('/') {
        s = s[idx + 1..].to_string();
    }
    if let Some(idx) = s.find('@') {
        s.truncate(idx);
    }
    loop {
        let before = s.clone();
        if let Some(stripped) = s.strip_suffix("-latest") {
            s = stripped.to_string();
        }
        // -YYYY-MM-DD（尾 10 位成日期样，含连字符位）
        if s.len() > 11 {
            let t = s.as_bytes();
            let tail = &t[t.len() - 10..];
            let date_like = tail[0..4].iter().all(u8::is_ascii_digit)
                && tail[4] == b'-'
                && tail[5..7].iter().all(u8::is_ascii_digit)
                && tail[7] == b'-'
                && tail[8..10].iter().all(u8::is_ascii_digit);
            if date_like {
                s.truncate(s.len() - 11);
            }
        }
        // -YYYYMMDD（尾 8 位全数字且前一个字符是 '-'）
        if s.len() > 9 {
            let t = s.as_bytes();
            if t[t.len() - 9] == b'-' && t[t.len() - 8..].iter().all(u8::is_ascii_digit) {
                s.truncate(s.len() - 9);
            }
        }
        while s.ends_with('-') {
            s.pop();
        }
        if s == before {
            break;
        }
    }
    s
}

/// 完整查找：供应商定位 → 归一化 id 精确命中（先原样后归一化）。ollama
/// 本地目录不入快照，直接 None。
pub fn lookup<'a>(
    snapshot: &'a CatalogSnapshot,
    kind: &str,
    base_url: Option<&str>,
    model: &str,
) -> Option<&'a CatalogModel> {
    if kind == "ollama" {
        return None;
    }
    let base = base_url.map(str::trim).filter(|s| !s.is_empty())?;
    let base_host = host_of(base);
    if base_host.is_empty() {
        return None;
    }
    // 快照 api 字段索引（provider_key → api url；快照顶层 provider 里
    // 少数带 api，匹配期借视图零拷贝）。
    let api_index = snapshot.api_index();
    // 供应商定位：分最高者胜；并列取 host 精确者；全并列取键名字典序
    // （providers 已排序，首见胜出 = 确定性，不吃 HashMap 序）。
    let mut providers: Vec<&String> = snapshot.models.keys().map(|(p, _)| p).collect();
    providers.sort();
    providers.dedup();
    let mut best: Option<(&str, (u32, bool))> = None;
    for pkey in providers {
        let score = provider_score(&base_host, pkey, &api_index);
        if score.0 == 0 {
            continue;
        }
        let better = match best {
            None => true,
            Some((_, (s, e))) => score.0 > s || (score.0 == s && score.1 && !e),
        };
        if better {
            best = Some((pkey, score));
        }
    }
    let (pkey, _) = best?;
    // 两遍扫：原样 id 精确命中优先，其次归一化命中（同一轮 find 会吃
    // HashMap 序——带日期戳的旧条目可能抢在裸名前面）。
    let in_provider = |m: &&'a CatalogModel| m.provider_key == *pkey;
    let model_norm = normalize_id(model);
    let candidates: Vec<&'a CatalogModel> = snapshot.models.values().filter(in_provider).collect();
    // 三级裁决（全部确定性，不吃 HashMap 序）：原样 id 精确命中 → 归一化
    // 命中里裸名（自身即归一形）优先 → 归一化命中里 model_id 字典序最小
    // （带日期戳/后缀的多变体间稳定取胜者；单轮 find 会在变体间随机）。
    if let Some(m) = candidates.iter().find(|m| m.model_id == model) {
        return Some(m);
    }
    let norm_hits: Vec<&CatalogModel> = candidates
        .iter()
        .filter(|m| normalize_id(&m.model_id) == model_norm)
        .copied()
        .collect();
    norm_hits
        .iter()
        .find(|m| m.model_id == model_norm)
        .or_else(|| norm_hits.iter().min_by_key(|m| m.model_id.as_str()))
        .copied()
}

impl CatalogSnapshot {
    /// provider_key → api url 的借用视图（匹配期零拷贝）。
    fn api_index(&self) -> HashMap<&str, &str> {
        self.api_index
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }
}

#[cfg(test)]
mod tests;
