//! 事实卡簇（MR 稿 §5 提案 A · 后审计队列 Q3 · v2.186）：记录员在「章节数组 +
//! 轮级总结」之外顺带产出的 ≤N 张卡，它的**标记、契约文本与解析层钳制**都在这个
//! 文件里。
//!
//! 为什么单独立簇而不是留在 `manager/mod.rs`：本簇只回答「这条回复里的卡段怎么
//! 变成一张可信的卡」，而 `mod.rs` 回答「整段输出怎么变成章节与总结」——两件事的
//! 修改动机不同（改钳制/契约文本不必碰解析主干，反之亦然），且 `parse_drafts` 的
//! 契约变动会牵动整理降级面、卡段契约变动只牵动设置页与提示词尾部。拆分本身零行为
//! 变化：三段内容逐字从 `manager.rs` 搬来，只把 `extract_array` 的可见性抬到
//! `pub(crate)`（跨簇调用），并把 `split_facts_section` 交回 `mod.rs` 使用。
//!
//! ⚠ 跨簇承重只有一条：**卡段必须在总结段之前剥掉**。那条顺序由 `mod.rs` 的
//! `split_summary_section` 调用本文件的 `split_facts_section` 实现，判据在
//! `facts/tests.rs`（有无卡段两向等价 + 反序不容错的实测形状）。

use serde::Deserialize;

use super::extract_array;

/// 事实卡的尾随标记（MR 稿 §5 提案 A · 队列 Q3）：整理收束时记录员在章节 +
/// 轮级总结之外**顺带**产出的 ≤N 张事实卡（这是本刀的全部新增——复用整理
/// 的触发/重试/降级/熔断，零新增调度器）。
///
/// ⚠ **切分顺序是有承重的**：`run_summary` 取的是 `===SUMMARY===` 之后的
/// **全部**文本，卡段若留在总结之后就一起被吞进总结正文（还会被
/// `RUN_SUMMARY_MAX_CHARS` 截掉尾巴）。所以 FACTS 必须在 SUMMARY 之前剥掉，
/// 两处消费（`parse_drafts` 的数组部分、`run_summary` 的总结部分）才同时正确。
const FACTS_MARKER: &str = "===FACTS===";
/// 单张卡 value 的字符上限（稿 §5「≤500 字」与 §6 预算表同源）。
pub const FACT_VALUE_MAX_CHARS: usize = 500;
/// 卡片类型白名单（稿 §3 归层映射的四个 ZCode 类型）：`user|feedback → 全局层`、
/// `project|reference → 当前键位`。**白名单外的整条丢弃**——归层是这唯一的用途，
/// 类型不可信时宁可少一张卡，不许把来历不明的内容塞进全局偏好层。
pub const FACT_TYPES: [&str; 4] = ["user", "feedback", "project", "reference"];
/// 单次整理的卡片硬顶（解析层防线；实际取数由设置 `memory.factCardMax` 决定，
/// 这里只防"模型一次灌回几百张"把整理收束拖爆）。
const FACT_HARD_CAP: usize = 20;

/// 一张事实卡（稿 §5：key + value + type + tags）。
///
/// ⚠ `type` **不落库**：`memories` 表的列是
/// `(workspace, key, value, tags, updated_at, source)`，没有 type 列，加列属
/// schema 迁移（不在本批，零迁移纪律）。它的唯一职责 = **决定归层**（落进
/// 全局层还是当前键位）；卡片的出处由落库时的 `source = "organized"` 记录，
/// 与用户亲手写的 `explicit`、历史遗留的 `extracted` 三档并存。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FactCard {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub tags: String,
    #[serde(rename = "type", default)]
    pub card_type: String,
}

/// 按 `===FACTS===` 切出「前半（数组 + 总结）| 卡段」。无标记 = 无卡段。
///
/// `pub(crate)` 是拆簇带来的一档抬升（拆分前它是 `manager.rs` 的私有项）：父簇的
/// `split_summary_section` 必须**先**调它才能保证总结不被卡段吞掉。除这一处与
/// `extract_array`（反方向：本簇要借父簇的数组定位），两簇之间的可见面没有别的改动。
pub(crate) fn split_facts_section(raw: &str) -> (&str, Option<&str>) {
    match raw.split_once(FACTS_MARKER) {
        Some((before, after)) => (before, Some(after)),
        None => (raw, None),
    }
}

/// 提取事实卡（稿 §5 提案 A）：只吃 `===FACTS===` 之后那个 JSON 数组。
///
/// 三条降级口径都是**逐条丢、不整批硬造**（与章节的"任何一条不满足整批判垃圾
/// 转规则切分"刻意不同——章节铺满区间是树的结构契约，一处破就整批不可信；
/// 卡是可多可少的副产物，多一张少一张不动结构）：
/// 无标记 / 卡段里抽不出数组 / JSON 解析失败 / 数组超限 → **返回空**（整理照常）；
/// 单条 key 或 value 为空、`type` 不在白名单 → **只丢那一条**；
/// value 超 `FACT_VALUE_MAX_CHARS` → 钳制（不丢，截断比丢信息好，且卡本来就不许长）。
/// `limit` = 设置 `memory.factCardMax`；**0 = 一张不取**（关掉开关的第二重防线，
/// 第一重是宿主根本不给卡段要求）。
pub fn fact_cards(raw: &str, limit: usize) -> Vec<FactCard> {
    if limit == 0 {
        return Vec::new();
    }
    let cap = limit.min(FACT_HARD_CAP);
    let Some(after) = split_facts_section(raw).1 else {
        return Vec::new();
    };
    let Some(arr_text) = extract_array(after) else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<Vec<FactCard>>(arr_text) else {
        return Vec::new();
    };
    parsed
        .into_iter()
        .filter(|c| FACT_TYPES.contains(&c.card_type.as_str()))
        .filter_map(|c| {
            let key = collapse_one_line(&c.key);
            let value = clamp_chars(c.value.trim(), FACT_VALUE_MAX_CHARS);
            (!key.is_empty() && !value.is_empty()).then(|| FactCard {
                key,
                value,
                tags: collapse_one_line(&c.tags),
                card_type: c.card_type,
            })
        })
        .take(cap)
        .collect()
}

/// 压成一行并去首尾空白（key / tags 都要落进 md 的 frontmatter 与索引行，
/// 带换行的值会把人类层结构打乱）。
fn collapse_one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clamp_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// 事实卡的**追加要求**文本（宿主把它拼在 user 段之后，见下）。
///
/// 为什么不在四份家族提示词（`include_str!` 的 system 段）里写：《Memory Tree
/// Design v2》§6.5 立的是「角色提示词会话内逐字节稳定、无动态内容」——而
/// 「现有 key 清单」每次整理都不同（正是它给模型查重用），塞进 system 就破了
/// 那条纪律，也会打断记录员自己的前缀缓存。所以：**静态的格式契约归 system
/// （由本函数产出的文本自带标记说明，随材料一起给），动态清单归 user 段**。
/// 开关关掉时宿主**不调用**本函数，user 段逐字节回到现役形状。
pub fn fact_request(existing_keys: &[String], max: usize) -> String {
    let mut s = String::from(
        "\n\n# 事实卡（本次整理顺带产出，最多 \
         ",
    );
    s.push_str(&max.to_string());
    s.push_str(
        " 张）\n\n段落总结之后，另起一行输出标记 `===FACTS===`，再输出一个 JSON 数组\
（不需要就直接给 `[]`）。每张卡四个字段：\n\n\
1. `key`：点分短键，例如 `build.cmd`、`user.pref.pkgmanager`。\n\
2. `value`：一条可独立读懂的事实，500 字以内。写结论与理由，不写过程叙事、不复述章节。\n\
3. `type`：只能取 `user` | `feedback` | `project` | `reference`——\
`user` 是跨项目成立的长期偏好，`feedback` 是用户对协作方式的纠正，\
`project` 是本项目的约束/决定/事实，`reference` 是外部资源指针。\n\
4. `tags`：空格分隔的标签，可留空。\n\n\
只记材料里真实出现过的事实，不为凑数编造；拿不准 type 的不要写。\n",
    );
    if max > 0 {
        s.push_str("下面这些键已经存过，**优先用同名键更新**，不要造近义重复的新键：\n");
        if existing_keys.is_empty() {
            s.push_str("（暂无）\n");
        } else {
            for k in existing_keys {
                s.push_str("- ");
                s.push_str(k);
                s.push('\n');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests;
