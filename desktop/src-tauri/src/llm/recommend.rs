//! 探测清单推荐三选（批 F，替代「自动全选 ≤8」）：目录能力数据驱动的
//! 确定性预选——toolCall 必备、族分散、超贵降权（阶段计划批 F ③）。
//! 纯函数 + 目录命中面由调用方（probe 命令）注入。

/// 推荐上限（「三选」）。
pub const RECOMMEND_MAX: usize = 3;

/// 一个候选模型的推荐输入（目录命中面；未命中 = 全 None）。
#[derive(Debug, Clone, Default)]
pub struct RecCandidate {
    pub id: String,
    pub tool_call: Option<bool>,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
    /// models.dev `family`；None = id 自成族（族分散不排除未知族样本）。
    pub family: Option<String>,
}

/// 确定性推荐 ≤[`RECOMMEND_MAX`] 个：
/// ① 目录 tool_call 有 `Some(true)` 在场 → 剔 `Some(false)`（未知 None
///   留——目录没说的能力不发明，但已知不能调工具的模型不该当主力）；
/// ② 价格升序（in+out 之和；未知最后，同价 id 字典序）；③ 族分散：同
/// family 至多取 1；④ 取前 [`RECOMMEND_MAX`]。目录全未命中 = 列表序
/// 前 N 保底（推荐面不可用也不让用户空手）。
pub fn recommend(candidates: &[RecCandidate]) -> Vec<String> {
    if candidates.is_empty() {
        return Vec::new();
    }
    let mut pool: Vec<&RecCandidate> = candidates.iter().collect();
    if pool.iter().any(|c| c.tool_call == Some(true)) {
        pool.retain(|c| c.tool_call != Some(false));
    }
    let price = |c: &RecCandidate| match (c.input_price, c.output_price) {
        (Some(i), Some(o)) => Some(i + o),
        (Some(i), None) | (None, Some(i)) => Some(i),
        (None, None) => None,
    };
    pool.sort_by(|a, b| {
        let (pa, pb) = (price(a), price(b));
        match (pa, pb) {
            // 同价 id 字典序（确定性）；双未知 = 保持列表序（保底口径）
            (Some(x), Some(y)) => x.total_cmp(&y).then_with(|| a.id.cmp(&b.id)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
    let mut picked_families: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for c in pool {
        if out.len() >= RECOMMEND_MAX {
            break;
        }
        if let Some(f) = c.family.as_deref() {
            if picked_families.contains(&f) {
                continue;
            }
            picked_families.push(f);
        }
        out.push(c.id.clone());
    }
    out
}

#[cfg(test)]
mod tests;
