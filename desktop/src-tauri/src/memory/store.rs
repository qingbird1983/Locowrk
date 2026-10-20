//! `MemoryStore` 的**结构与装配面**（台账 T33）：字段、构造、两条接线（召回观测 /
//! 交接注入）、作用域解析，外加所有分片共用的三件底层件——`open_conn`（开连接 + 建表 +
//! 两侧迁移都挂在这一条线上）、行装配 `row`、人类层句柄 `files` 与它的两条落盘动作
//! `write_md` / `delete_md`。
//!
//! S1 刀②c-1 起**人类层不是"接线"而是结构**：`files` 不再是 `Option`，也不再问
//! settings `memory.files` ——它和 `memory.sqlite3` 一样由数据根算出（`<根>/memory`），
//! 构造时自算 ⇒ 没有"未接线 = 退回纯 sqlite"那一档可退。`with_files` 因此删除，
//! `state/memory_stack.rs` 那一处调用同步消失（句柄本身仍在 `AppState` 里另持一份，
//! 给 reconcile 与「打开文件夹」两枚命令用）。
//!
//! 本片对同类型的其余分片（`crud` / `reconcile` / `recall` / `source`）给出四个
//! `pub(super)` 入口：`open_conn`、`row`、`write_md`、`delete_md`；跨片读到的四个字段
//! `warned` / `repo_on` / `repo_ensured` / `recall_log` 同口径抬升，其余字段保持私有
//! （只有本片碰得到）。抬升只到 `memory` 子树，出不了模块 ⇒ 模块外的公开面与此前逐字相同。
//!
//! 末尾那条合上的花括号是**本片自补的语法外壳**：`impl MemoryStore` 按面分进四个文件，
//! 原文那一对花括号只属于"开到这里"的这一片，剩下的由本片自己收。

use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;

use crate::workspace::{scope_key, WorkspaceResolver};

use super::schema::{ensure_learning_schema, ensure_workspace_schema};
use super::shapes::{MemoryRow, CREATE_TABLE};

pub struct MemoryStore {
    dir: PathBuf,
    /// 会话 → 项目根解析（workspace.rs 单一真源；recall 按键位过滤用）。
    workspace_of: WorkspaceResolver,
    /// 检索失败只告警一次（每 step 都 recall，别刷日志）。
    pub(super) warned: AtomicBool,
    /// **真源所在**（《Memory File-ization Design v1.1》）：`<dir>/memory/` 下的一事实
    /// 一文件。刀②c-1 起它是结构而不是可选项——`MemoryFiles` 只是路径持有者
    /// （构造不碰盘、克隆很便宜），所以这里连 `Arc` 都不需要共享，自己建一份即可。
    /// ⚠ settings `memory.files` 这一档**管不到它**：真源没有可退回的第二处，
    /// "关掉文件化"在宿主读写面上已经没有任何语义（该项剩下的作用面与收口见
    /// `specs/05` 刀②c 挂账）。
    files: crate::memory_files::MemoryFiles,
    /// F2 交接注入开关（settings `memory.repoContext` 闭包；缺省恒关）。
    pub(super) repo_on: Arc<dyn Fn() -> bool + Send + Sync>,
    /// 全局记忆层开关（S1 刀④b · settings `memory.globalLayer` 闭包）。
    /// ⚠ 缺省**恒开**，与上面那枚 `repo_on` 的"缺省恒关"相反——不是随手写的两个
    /// 布尔：交接段是 v2.136 才加的注入，而全局层记忆从这套机制存在第一天起就在注入，
    /// 所以后者关掉"未接线"这一档才等于改前形状（未接线 = 单测与降级启动）。
    pub(super) global_on: Arc<dyn Fn() -> bool + Send + Sync>,
    /// 项目记忆层开关（S1 刀④b · settings `memory.projectLayer` 闭包；缺省恒开，同上）。
    pub(super) project_on: Arc<dyn Fn() -> bool + Send + Sync>,
    /// 已引导过的仓库（每进程每仓库一次 ensure_layout，幂等也兜底防抖）。
    pub(super) repo_ensured: Mutex<std::collections::HashSet<PathBuf>>,
    /// Q1a（L0 注入侧可观测性）：埋点落库句柄（`usage.sqlite3` 旁挂的
    /// `memory_inject_log`）。**未接线（单测、降级启动）= 不记**，召回行为逐字不变。
    /// 句柄类型是 `recall_log::RecallLog` 而不是 `usage::UsageRecorder`：统计写入器
    /// 不该兼管记忆观测（L0 稿 §3.2「命中与工具热度混起来会互相污染」）。
    pub(super) recall_log: Option<Arc<crate::recall_log::RecallLog>>,
}

impl MemoryStore {
    pub fn new(dir: &Path, workspace_of: WorkspaceResolver) -> Self {
        Self {
            dir: dir.to_path_buf(),
            workspace_of,
            warned: AtomicBool::new(false),
            files: crate::memory_files::MemoryFiles::new(dir),
            repo_on: Arc::new(|| false),
            global_on: Arc::new(|| true),
            project_on: Arc::new(|| true),
            repo_ensured: Mutex::new(std::collections::HashSet::new()),
            recall_log: None,
        }
    }

    /// 接线召回观测（Q1 / L0，state 组装点调用）：注入侧每次交付记一行
    /// `memory_inject_log`。**只记不改**——召回的 SQL、条数、字节全部与改前同源。
    pub fn with_recall_log(mut self, log: Arc<crate::recall_log::RecallLog>) -> Self {
        self.recall_log = Some(log);
        self
    }

    /// 接线交接注入（F2，state 组装点调用）：settings `memory.repoContext`
    /// 闭包——归属会话注入 `<repo>/.Locowrk/` 交接段 + 引导建目录。
    pub fn with_repo_context(mut self, on: Arc<dyn Fn() -> bool + Send + Sync>) -> Self {
        self.repo_on = on;
        self
    }

    /// 接线两枚**行为开关**（S1 刀④b · ADR-0007 §18 的前两枚；第三枚就是上面那枚
    /// `repoContext`，它早已存在，所以这里不另立一份"交接层开关"）。
    /// 吃闭包而不是 `bool`：设置页保存即生效，不重启（`files_on` / `repo_on` 同一模式）。
    /// 判的是"哪一层进不进上下文"，不是"数据存在吗"——关掉之后文件一字不动、
    /// 管理页照旧全量、模型主动查仍读得到（边界写在 `settings::MemorySettings::global_layer`）。
    pub fn with_layer_switches(
        mut self,
        global_on: Arc<dyn Fn() -> bool + Send + Sync>,
        project_on: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        self.global_on = global_on;
        self.project_on = project_on;
        self
    }

    /// 会话的仓库根（归属路径 = 交接面落点）。未归属 / 非目录 = None。
    /// v2.139 pub：指令文件层（instruction_files.rs）同一真源找项目根
    /// AGENTS.md。
    pub fn repo_root(&self, session_id: Option<&str>) -> Option<PathBuf> {
        let sid = session_id?;
        let ws = (self.workspace_of)(sid)?;
        if ws.is_empty() {
            return None;
        }
        let repo = PathBuf::from(&ws);
        repo.is_dir().then_some(repo)
    }

    /// 人类层句柄（**真源的唯一取用档**）。
    ///
    /// 为什么现在不再是 `Option`：这句判断此前散在三处（`md_write` / `md_delete` /
    /// `md_index_body` 各写一份 `(Some(files), true)` 或 `!(files_on)()`），刀②b 的读侧
    /// 是第四个消费者 ⇒ 收成一处之后，刀②c-1 把它变成"根本没有第二档"。
    pub(super) fn files(&self) -> &crate::memory_files::MemoryFiles {
        &self.files
    }

    /// 真源根（`<数据根>/memory`）。
    ///
    /// ⚠ S4 刀③b 起**生产面无调用点**：原来唯一的子树外读点 `organize/facts.rs`
    /// 随整理面删除；现在只给测试夹具用，随 `memory/` 整面在刀③c 一起删。
    #[cfg(test)]
    pub fn md_root(&self) -> &std::path::Path {
        self.files.root()
    }

    /// 落一条事实到真源。**失败 = 上抛**（刀②c-1 的语义反转）：旧形状是"sqlite 已落，
    /// md 只是同步、失败只 warn"，那句"调用方保证 sqlite 已是真源"现在反过来由
    /// `crud.rs` 的四条写路径兑现——先落这里，成功了才去碰影子。
    pub(super) fn write_md(
        &self,
        ws: &str,
        key: &str,
        value: &str,
        tags: &str,
        source: &str,
        at: i64,
    ) -> Result<(), String> {
        self.files
            .write_fact(&plugin_api::memlayout::FactEntry {
                workspace: ws,
                key,
                value,
                tags,
                source,
                updated_at: at,
                // 宿主这四条写路径（管理页新建 / 编辑 / 整理回写 / 迁移）今天都不带分类：
                // 四类型的**生产者**是记忆工具的 `type` 参数（S1 刀③），管理页那半在刀④。
                // 空串 = 未分类 ⇒ 落盘字节形与四类型落地之前一字不差。
                ty: "",
            })
            .map(|_| ())
            // 报错必须**点名是哪条记忆**：这一句现在会一路走到管理页的错误条与整理管线的
            // 日志，只给路径的话用户看不出是自己刚写的那条失败了（旧 warn 文案就带 key，
            // 换成 Err 之后这一维不许丢）。
            .map_err(|e| format!("记忆 `{key}` 落盘失败: {e}"))
    }

    /// 从真源删一条。返回值 = **文件原本在不在**（刀②c-1 起这是 no-op 的唯一判据；
    /// 旧判据是"库里删掉了几行"，真源翻转后那个数说的已经不是"这条记忆存在过"）。
    /// 刀④a 把底下的落点换成 `soft_delete_fact`（搬进 `obsolete/`，不是 `remove_file`），
    /// 签名与这两条判据都没动——"没搬走"与"本来就没有"仍然是同一档，"层读不动"才是 Err。
    pub(super) fn delete_md(&self, ws: &str, key: &str) -> Result<bool, String> {
        self.files
            .soft_delete_fact(ws, key)
            .map_err(|e| format!("记忆 `{key}` 删除失败: {e}"))
    }

    /// 会话（可缺）→ 键位（未归属 = ""）。context_line / scope_id 共用。
    pub fn scope_of_session(&self, session_id: Option<&str>) -> String {
        scope_key(session_id.and_then(|s| (self.workspace_of)(s)).as_deref())
    }

    /// 项目 `MEMORY.md` 投影全文（Q2 索引注入的**唯一档**，人可编辑的那份）。
    ///
    /// `None` = 项目键位为空 / 文件读不到。注意"文件在场但只有 `# 记忆索引` 标题"
    /// 也归入"用不上"——那是本机实测的常态（见 `memory_index` 头注），判断在
    /// `memory_index::from_md` 里做。
    pub(crate) fn md_index_body(&self, workspace: &str) -> Option<String> {
        if workspace.is_empty() {
            return None;
        }
        // 家目录经 memlayout 单点算式（Q15 收口：这里此前自己拼过一份
        // `projects/<可读名-哈希>`，那是同一句话的第三个出处）。`.ok()?`
        // 而不是 unwrap：算不出来 = 这一节缺席，按同函数头注的降级口径走。
        let path = plugin_api::memlayout::project_dir(self.files.root(), workspace)
            .ok()?
            .join("MEMORY.md");
        std::fs::read_to_string(path).ok()
    }

    /// 按调用开连接、用完即弃；建表 + 旧库迁移幂等。
    /// ⚠ 这个"同款"原本是照着插件写的（两侧各开同一个库），自 S1 刀③b 起
    /// **只有宿主开它**了——连接策略本身不变（宿主自己就是多线的）。
    pub(super) fn open_conn(&self) -> Result<Connection, String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("memory db dir `{}`: {e}", self.dir.display()))?;
        let conn = Connection::open(self.dir.join("memory.sqlite3"))
            .map_err(|e| format!("memory db open: {e}"))?;
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(|e| format!("memory db busy_timeout: {e}"))?;
        conn.execute_batch(CREATE_TABLE)
            .map_err(|e| format!("memory db init: {e}"))?;
        ensure_workspace_schema(&conn)?;
        ensure_learning_schema(&conn)?;
        Ok(conn)
    }

    pub(super) fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRow> {
        Ok(MemoryRow {
            workspace: r.get(0)?,
            key: r.get(1)?,
            value: r.get(2)?,
            tags: r.get(3)?,
            // ⚠ 影子表**没有** `type` 这一列（`CREATE_TABLE` 八列里没有它，分类只住 md）。
            // 所以库里读回来一律是未分类，这不是"丢了值"而是"那一侧从来就没有这一维"。
            // 现在不会出事：本函数的两个消费者（`reconcile.rs` 的行比对）都不比 type。
            // 哪天要让对账比分类，得先给影子表加列并过 `check-schema-parity.mjs` 的
            // H1–H7，不能就这么把一个常量当成数据用。
            ty: plugin_api::memlayout::UNCLASSIFIED_TYPE.into(),
            source: r.get(4)?,
            updated_at: r.get(5)?,
        })
    }
}
