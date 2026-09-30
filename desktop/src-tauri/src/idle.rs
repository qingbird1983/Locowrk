//! 插件闲置分级（⑩，exposure 文档 §8.3–§8.7）：后端只产出结构化
//! IdleInfo（level + value + unit），不产人类文案——"多久没用"的措辞
//! 由前端按 locale 渲染（§11）。时间计算全部在这里，前端零解读。

use serde::{Deserialize, Serialize};

/// 六级闲置分类（§8.3 表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdleLevel {
    /// ≤7 天用过（含试用期保护，§8.7）。
    Fresh,
    /// 7–30 天。
    Normal,
    /// 30–90 天。
    Idle,
    /// 90–180 天。
    Stale,
    /// 180 天以上。
    Abandoned,
    /// 装了 ≥14 天从未用（与"长时间未用"严格区分，§8.7）。
    NeverUsed,
}

/// 人类尺度的时长档位（§8.5/§8.6/§8.7）。值本身是档位标签而非裸天数：
/// month/year 携带倍数，never_used 的 value 是安装月数档（None=两周内 /
/// 1=一个多月 / 3=三个多月 / 6=半年多），文案分档由前端表完成。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdleUnit {
    Today,
    ThisWeek,
    LastWeek,
    LastMonth,
    Month,
    HalfYear,
    Year,
    NeverUsed,
    /// 刚装上（<14 天未用）——试用期保护，前端不渲染徽章。
    JustInstalled,
}

/// 结构化闲置信息（§8.5）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdleInfo {
    pub level: IdleLevel,
    pub value: Option<u32>,
    pub unit: Option<IdleUnit>,
}

const DAY: i64 = 86_400;

/// 闲置分级（§8.6 完整文案表 + §8.7 从未使用表）。`now`/`installed_at`/
/// `last_used_at` 均为 Unix 秒；从未用过时 last_used_at 传 None。
pub fn classify(now: i64, installed_at: i64, last_used_at: Option<i64>) -> IdleInfo {
    let days = |t: i64| (now - t) / DAY;
    if let Some(d) = last_used_at.map(days) {
        let (level, value, unit) = match d {
            ..=1 => (IdleLevel::Fresh, None, Some(IdleUnit::Today)),
            2..=6 => (IdleLevel::Fresh, None, Some(IdleUnit::ThisWeek)),
            7..=13 => (IdleLevel::Fresh, None, Some(IdleUnit::LastWeek)),
            14..=29 => (IdleLevel::Normal, None, Some(IdleUnit::LastMonth)),
            30..=59 => (IdleLevel::Idle, Some(1), Some(IdleUnit::Month)),
            60..=89 => (IdleLevel::Idle, Some(2), Some(IdleUnit::Month)),
            90..=179 => (IdleLevel::Stale, Some(3), Some(IdleUnit::Month)),
            180..=364 => (IdleLevel::Abandoned, None, Some(IdleUnit::HalfYear)),
            _ => (
                IdleLevel::Abandoned,
                Some((d / 365).max(1) as u32),
                Some(IdleUnit::Year),
            ),
        };
        return IdleInfo { level, value, unit };
    }
    // 从未使用：按安装后多久分级（§8.7）
    let age = days(installed_at);
    let (level, value, unit) = match age {
        ..=13 => (IdleLevel::Fresh, None, Some(IdleUnit::JustInstalled)),
        14..=30 => (IdleLevel::NeverUsed, None, Some(IdleUnit::NeverUsed)),
        31..=90 => (IdleLevel::NeverUsed, Some(1), Some(IdleUnit::NeverUsed)),
        91..=180 => (IdleLevel::NeverUsed, Some(3), Some(IdleUnit::NeverUsed)),
        _ => (IdleLevel::NeverUsed, Some(6), Some(IdleUnit::NeverUsed)),
    };
    IdleInfo { level, value, unit }
}

/// 筛选档位键（§9.2 chips 的后端裁决，嵌套包含：gt3m ⊂ gt1m ⊂ recent）。
/// 运行中/已停用两个 chips 走 enabled/active 状态，不经这里。
pub fn filter_key(info: &IdleInfo) -> &'static str {
    let months = info.value.unwrap_or(0);
    match info.unit {
        Some(IdleUnit::Month) => match months {
            1..=2 => "gt1m",
            _ => "gt3m",
        },
        Some(IdleUnit::HalfYear) | Some(IdleUnit::Year) => "gt6m",
        Some(IdleUnit::NeverUsed) => match months {
            0 => "recent",
            1..=2 => "gt1m",
            3..=5 => "gt3m",
            _ => "gt6m",
        },
        _ => "recent",
    }
}

#[cfg(test)]
mod tests;
