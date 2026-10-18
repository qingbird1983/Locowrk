//! 记忆类型面（设计 §2.1：四类型 + 分级规则 + `Why:` / `How to apply:` 纪律）。
//!
//! 三件事，都是**纯函数**（不碰盘、不算路径）：①`metadata.type` 的值域与归一，
//! ②由类型算落点层（模型只报 type，目录由 [`layer_for`] 算——记忆工具有宿主与插件
//! 两条通路，放任何一侧都算不出另一侧的落点，所以它住在 `plugin-api`），
//! ③`feedback` / `project` 缺 `Why:` / `How to apply:` 时的提醒判据（**只提醒、不拦写**）。
//!
//! 为什么 type 不进 sqlite 影子：真源 = 文件之后，字段只往真源加；影子表在刀③ 之后
//! 只剩学习量两列，给一个即将退役的内容表加列 = 为它做一次 schema 迁移（specs/05 刀③ 登记）。

/// 四类型（值域硬编码在官方 agent-sdk 提示词里，设计 §2.1 勘误表第 16 行订正过旧的
/// `fact|preference|decision|handoff`）。
pub const MEMORY_TYPES: [&str; 4] = ["user", "feedback", "project", "reference"];

/// 未给类型 / 旧文件 / 写了值域外任何东西时的落盘值 = **未分类**。
/// 存量不重写：四类型落地之前落的文件本来就写着 `fact`，读回它不代表任何分类决定。
pub const UNCLASSIFIED_TYPE: &str = "fact";

/// 渲染前把调用方给的 type 收进值域：四类型原样（大小写与首尾空白不敏感），
/// 其余（空串、`fact`、人乱写的）= [`UNCLASSIFIED_TYPE`]。
///
/// 这一档是**写侧的唯一入口**：工具参数面允许模型写 `Feedback` 或带空格，
/// 落盘必须是同一个小写形，否则 `layer_for` 与索引都会按两种串漂。
pub fn normalize_type(ty: &str) -> String {
    let flat = ty.trim().to_ascii_lowercase();
    if MEMORY_TYPES.contains(&flat.as_str()) {
        flat
    } else {
        UNCLASSIFIED_TYPE.to_string()
    }
}

/// 落点层（只说"全局还是项目"，具体目录由 `paths.rs` 算）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// `memory/global/` = 键位 `''`。
    Global,
    /// `memory/projects/<可读名-哈希>/facts/` = 当前项目键位。
    Project,
}

/// 一条类型 + 工具显式 `scope` 参数 → 该落哪一层。
///
/// 优先级 = **显式覆盖 > 类型缺省**（设计 §2.1 分级规则）：
/// `scope="global"` / `"project"` 说了算；没给时 `user` / `feedback` 落全局
/// （用户偏好与对人的反馈天然跨项目），`project` / `reference` 落项目。
/// 未分类（含旧 `fact`）按**项目**走——那是现状行为：今天每条记忆都落当前项目键位，
/// "读不回类型"不许被解释成"它其实是全局偏好"。
/// 值域外的 `scope` 串（拼错 / 大小写漂）一律**忽略覆盖**、回落到类型缺省。
pub fn layer_for(ty: &str, explicit_scope: Option<&str>) -> Layer {
    match explicit_scope.map(|s| s.trim().to_ascii_lowercase()) {
        Some(s) if s == "global" => return Layer::Global,
        Some(s) if s == "project" => return Layer::Project,
        _ => {}
    }
    match normalize_type(ty).as_str() {
        "user" | "feedback" => Layer::Global,
        _ => Layer::Project,
    }
}

/// `feedback` / `project` 的正文纪律：缺 `Why:` 或 `How to apply:` 时给一句提醒。
///
/// `None` = 不需要提醒（未分类 / `user` / `reference` 都不带这两段，或两段都在场）。
/// 这是**提醒不是拒绝**：拦写会把"格式不齐"变成"记忆没记上"，而缺 Why 的记忆仍然
/// 比没有记忆强（设计 §2.1；AC-S1-3 的验收只看落位，不看这两段在不在）。
/// 判据宽松到"子串命中"：`- Why:`、`**Why:**`、行中 `why:` 都算在场——
/// 宁可少提醒一条，也不把人的正常写法判成缺。
pub fn missing_guidance(ty: &str, body: &str) -> Option<&'static str> {
    let t = normalize_type(ty);
    if t != "feedback" && t != "project" {
        return None;
    }
    let low = body.to_ascii_lowercase();
    let mut missing: Vec<&str> = Vec::new();
    if !low.contains("why:") {
        missing.push("Why:");
    }
    if !low.contains("how to apply:") {
        missing.push("How to apply:");
    }
    if missing.is_empty() {
        return None;
    }
    Some(match missing.as_slice() {
        ["Why:"] => {
            "memory written, but a `feedback`/`project` entry should carry a `Why:` line \
                    (knowing the reason is what makes edge cases judgeable) — add it on the next edit"
        }
        ["How to apply:"] => {
            "memory written, but a `feedback`/`project` entry should carry a \
                             `How to apply:` line (how to act on it) — add it on the next edit"
        }
        _ => {
            "memory written, but a `feedback`/`project` entry should carry `Why:` and \
             `How to apply:` lines (reason + how to act) — add them on the next edit"
        }
    })
}
