//! 工具入口共用的**真源侧读件**（S1 刀③b：这几枚工具不再开库，读的就是
//! `<数据根>/memory/` 下那些 md）。
//!
//! 四件：时间戳 `now` · 一条已存事实的 JSON 投影 `hit_json` · 单层/两层读取
//! [`read_preferred`] 与 [`both_layers`]。它们都只有几行，再拆就是为行数造结构
//! （T31 那把 `parse_usage` 的课）。
//!
//! ⚠ 这一片存在的核心理由是 [`Hit::ws`] 那个字段：`StoredFact.workspace` 在**人自己
//! 新建、没写 frontmatter 的文件**上只是目录名兜底（`memlayout::LayerDir::hint` 的
//! 陷阱，specs/05 刀②c-1 登记的第三处），拿它当 undo 的还原目标会把记忆写进一个
//! 不存在的键位。所以从磁盘读到内存的第一站就把「我按键位问了哪一层」带在身上。

use std::path::{Path, PathBuf};

use plugin_api::memlayout::{self, StoredFact};
use plugin_api::serde_json::{Value, json};

/// 一条事实 + 它的**权威键位**（`''` = 全局层）。
pub(crate) struct Hit {
    pub ws: String,
    pub fact: StoredFact,
}

impl Hit {
    /// 工具返回体里的那一行。字段名与退役掉的那条 SQL 投影逐字同形
    /// （`key` / `value` / `tags` / `updated_at` / `scope` / `workspace`），
    /// 再加 `type` —— 名字与参数面不动，提示词与审批面才不会跟着漂。
    ///
    /// `workspace` 用 [`Hit::ws`]（读它时用的那个键位），**不是** `fact.workspace`。
    pub(crate) fn json(&self) -> Value {
        json!({
            "key": self.fact.key,
            "value": self.fact.value,
            "tags": self.fact.tags,
            "updated_at": self.fact.updated_at,
            "type": self.fact.ty,
            "workspace": self.ws,
            "scope": if self.ws.is_empty() { "global" } else { "project" },
        })
    }
}

/// 数据根下的真源根（每次调用现读 env，与宿主侧同一个算式）。
pub(crate) fn root() -> Result<PathBuf, String> {
    memlayout::memory_root()
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 按键位取一条（不存在 = `Ok(None)`，读不动 = `Err`——"没有"与"读不动"必须是两个
/// 答案，这是 S1 刀②c-1 在宿主侧立的同一纪律）。
pub(crate) fn read_one(r: &Path, ws: &str, key: &str) -> Result<Option<Hit>, String> {
    Ok(memlayout::read_fact(r, ws, key)?.map(|fact| Hit {
        ws: ws.to_string(),
        fact,
    }))
}

/// 双层读取优先级（Context 文档 §4.6）：先当前项目键位，未命中回落全局偏好层。
/// 与退役掉的那条 SQL `WHERE key = ? AND workspace IN (?1, '') ORDER BY
/// (workspace = ?1) DESC LIMIT 1` 同形；headless / 未归属的当前键位本就是 `''`，
/// 两次问的是同一层 ⇒ 由第一次命中直接返回。
pub(crate) fn read_preferred(r: &Path, project_ws: &str, key: &str) -> Result<Option<Hit>, String> {
    if !project_ws.is_empty() {
        if let Some(h) = read_one(r, project_ws, key)? {
            return Ok(Some(h));
        }
    }
    read_one(r, "", key)
}

/// 两层并集（列举与搜索的数据源）：当前项目层 ∪ 全局层。
///
/// **同名 key 两条都保留、各带层标**——那是退役掉的那条 SQL
/// `WHERE (workspace = ?1 OR workspace = '')` 的行为（`total` 也按两条算），
/// 不是去重后的"项目优先"：优先只发生在按键位取一条那一路（[`read_preferred`]）。
/// 这里去重会让管理口径与工具口径分叉。
pub(crate) fn both_layers(r: &Path, project_ws: &str) -> Result<Vec<Hit>, String> {
    let mut out: Vec<Hit> = Vec::new();
    if !project_ws.is_empty() {
        out.extend(
            memlayout::facts_of(r, project_ws)?
                .into_iter()
                .map(|fact| Hit {
                    ws: project_ws.to_string(),
                    fact,
                }),
        );
    }
    out.extend(memlayout::facts_of(r, "")?.into_iter().map(|fact| Hit {
        ws: String::new(),
        fact,
    }));
    Ok(out)
}

/// 写入键位裁决（Context 文档 §4.6 双层记忆，**本片判断逻辑自 S1 刀③b 一个字没动**——
/// 换掉的只是它下面接的存储）：显式 `"global"` = 全局偏好层（`''`）；
/// 没给 scope 时**由类型算缺省层**（刀③a，设计 §2.1 分级规则：`user` / `feedback`
/// 落全局，`project` / `reference` 落当前项目）；未归属会话缺省写**拒绝**（没有项目
/// 的"项目记忆"不许落地污染全局层——先归属，或显式升格为偏好）；
/// headless 无会话上下文 = `''` 单一宇宙（旧行为）。
///
/// `ty` 传空串或值域外的值 ⇒ [`memlayout::layer_for`] 给 `Project` ⇒ 与四类型落地之前
/// **逐字同形**（红证 = `tests/type_layer.rs::an_entry_with_no_type_lands_where_it_always_landed`）。
/// 值域外的 `scope` 串一律 Err（那是既有契约，不由类型缺省兜底——两档判据不许互相掩盖）。
pub(crate) fn write_scope(
    ctx: &super::WsCtx,
    scope_arg: Option<&Value>,
    ty: &str,
) -> Result<String, String> {
    let want_global = match scope_arg {
        None | Some(Value::Null) => memlayout::layer_for(ty, None) == memlayout::Layer::Global,
        Some(v) => match v.as_str() {
            Some("project") => false,
            Some("global") => true,
            other => {
                return Err(format!(
                    "memory_write: `scope` must be \"project\" or \"global\", got {other:?}"
                ));
            }
        },
    };
    Ok(match ctx {
        super::WsCtx::Headless => String::new(),
        super::WsCtx::Project(_) if want_global => String::new(),
        super::WsCtx::Project(ws) => ws.clone(),
        super::WsCtx::Unattributed if want_global => String::new(),
        super::WsCtx::Unattributed => {
            return Err(
                "memory_write: this session has no project — ask the user to associate one \
                 first, or pass scope=\"global\" to save a user preference that applies in \
                 every project"
                    .into(),
            );
        }
    })
}
