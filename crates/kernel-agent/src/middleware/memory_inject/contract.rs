//! 记忆注入的**契约面**（队列 Q9 起从 `memory_inject.rs` 拆出）：宿主实现要
//! 满足的形状，与配置到生效面的类型。
//!
//! 拆出去的理由 = `check_length`：本体加完学习档要同时管「契约类型」与
//! 「块组装 + 预取 + 监听器」两件事，580 行已越过 400 线（基线只许净降不许净增）。
//! 拆完的边界由判据 S3/S6 认：**契约只在这里定义一次**，本体 `handle` 只消费它。

/// 单条记忆（宿主检索实现返回；只带注入需要的字段）。
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    /// 短标签（memory-sqlite 的 tags 为单串，空串 = 无标签）。
    pub tags: String,
    /// 更新时间（Unix 秒）；当前仅随条目透出，排序由宿主实现裁决。
    pub updated_at: i64,
    /// 层标（㉘·A·fix 双层记忆，Context 文档 §4.6）：`"global"`（用户偏好，
    /// 全项目可见）| `"project"`（本项目记录）；空串 = 不标层（旧行为）。
    pub scope: String,
}

/// 记忆检索源契约：宿主实现（desktop 直读**真源 md**——S1 刀②b-2 起这条通路不再读
/// `memory.sqlite3`；那张影子表连同它的 ALTER 迁移仍归宿主，插件侧的建表与迁移随刀③b
/// 退役）。同步短查询，不跨 await；任何失败返回空表（实现内部 warn，不向上抛——
/// 注入不值得拦聊天）。
///
/// `session_id`（㉘）：本 step 的会话 id——宿主据此把召回限定在该会话
/// 归属的项目内（项目级记忆隔离，Context 文档 §4.6）；内核不解释
/// workspace 概念，只透传 id。`None` = 无会话上下文（如测试装配）。
pub trait MemorySource: Send + Sync + 'static {
    /// 按宿主裁决的相关序（desktop = updated_at 新→旧）取至多 limit 条**正文**条目。
    ///
    /// ⚠ **S3 刀③b 起它在生产注入面上没有调用点了**。刀① 摘掉 `<persistent_memory>`
    /// 正文块时它还有"学习档关掉时的那条回落支"这一个调用者（宿主
    /// `recall_relevant` 内部），那条随学习档一起退役 ⇒ 现在**一个生产调用者都没有**，
    /// 只有宿主的埋点通路 `recall_quiet → memory_inject_log` 还挂在它身上。
    /// 它留下的理由不是"注入面还要它"而是那一格欠账：方案稿 §C 之后"这一轮注入了什么"
    /// 的答案变成了索引指针行，L0 那三个指标（空块率 / 注入产出率 / 注入次数）的口径
    /// 要重定 = specs/10 登记的欠账 ⓠ。**ⓠ 收口之前本方法与 [`MemoryEntry`] 的形状一律
    /// 不动**，也别在这里长出第四个索引段之外的语义。
    fn recall(&self, session_id: Option<&str>, limit: usize) -> Vec<MemoryEntry>;

    /// 会话的键位身份（㉘·A·fix）：预取缓存键 = `(session_id, scope_id)`。
    /// 会话中途改归属 = 换键 = 首查重算（键位不变则逐字节复用，M1 纪律
    /// 不破）。缺省空串 = 不参与键（旧行为，装配测试零负担）。
    fn scope_id(&self, session_id: Option<&str>) -> String {
        let _ = session_id;
        String::new()
    }

    /// 环境块内文（提示词分层设计 §4.2，v2.139 起 `<environment>`）：
    /// 归属会话告知项目根 + 日期/平台。逐会话稳定、同吃预取缓存（日期
    /// 会话锁死）。缺省 `None` = 不带块（未归属/headless）。
    fn context_line(&self, session_id: Option<&str>) -> Option<String> {
        let _ = session_id;
        None
    }

    /// 仓库交接/提醒段（v2.136 项目侧全提醒制）：归属会话的
    /// `<repo>/.Locowrk/` **静态提醒文本**——目录里有什么、在什么节点
    /// 读/更新，正文不再注入，由主模型按需用文件工具查看与维护。宿主
    /// 实现；缺省 `None` = 无提醒段（headless / 未归属 / 开关关），组装
    /// 零变化。文本静态 = 跨轮逐字节稳定（M1 纪律不破）。
    fn repo_context(&self, session_id: Option<&str>) -> Option<String> {
        let _ = session_id;
        None
    }

    /// 全局层记忆**索引**（S3 刀① · 方案稿 §C「索引常驻，正文按需」）：全局层
    /// `MEMORY.md` 的**指针行**（`- [标题](文件.md) — 钩子`），不是事实正文。
    ///
    /// 它是 `<memory_index>` 块的第一段，与 [`MemorySource::project_index`] 同一套规矩：
    /// 宿主给内文、内核包标签、钳制与陈旧标注都在宿主侧。缺省 `None` = 该段缺席
    /// （headless 与测试装配零负担）；**两段都缺席 ⇒ 整块不生成**，不留一对空标签。
    fn global_index(&self, session_id: Option<&str>) -> Option<String> {
        let _ = session_id;
        None
    }

    /// 索引段的**新鲜度指纹**（S3 刀②）：宿主把"注入面读的那几份文件的 mtime"汇成
    /// 一个短串，内核把它并进预取缓存键 ⇒ **人改 `MEMORY.md` = 下一轮生效**，而不是
    /// 整个会话钉死在首查那一份上。缺省空串 = 不参与键（旧行为，测试与 headless 零负担）。
    ///
    /// 为什么由宿主算而不是内核 stat：内核不碰存储（本片一贯的分工），而"哪几份文件
    /// 算注入面的输入"是宿主知道的事——内核只保证"串变了就重算"。
    ///
    /// ⚠ **每 step 都会调它**（取数排在缓存查询之前）⇒ 必须是廉价读数，不许在里面
    /// 读文件内容或做模型往返。
    /// ⚠ 指纹里**不许**放"会话内会变"的东西（如当前轮数），否则 M1 纪律当场破：
    /// 逐轮换字节 = 前缀缓存命中归零。
    fn index_stamp(&self, session_id: Option<&str>) -> String {
        let _ = session_id;
        String::new()
    }

    /// 项目记忆**索引**（MR 稿 §4.1，队列 Q2）：本项目项目层的**指针行**
    /// （`- <key> — <hook>`），不是事实正文——v2.136「项目层事实不自动注入」
    /// 禁的是全文灌进 system，索引给的是"这个项目记了哪些条"，正文仍由模型
    /// `memory_read` 按需取。宿主实现（md 投影优先、sqlite 现合成兜底、预算钳制
    /// 都在宿主侧）。
    ///
    /// 缺省 `None` = 整节缺席（未归属会话没有"本项目"可言 / 项目层空 / headless
    /// 装配），组装零变化。文本随块同吃会话预取缓存 = 会话内字节钉死（M1 纪律）。
    fn project_index(&self, session_id: Option<&str>) -> Option<String> {
        let _ = session_id;
        None
    }
}

/// 注入参数（settings.json `memory` 区；Context 文档 §6.2 的
/// max_inject_count，缺省 5——约束清单 §3"注入条数可配"）。
///
/// **S3 刀③b 起这里只剩两个字段**：原先第三枚 `learn: LearnWeights`（学习档总开关 +
/// 四信号权重）随 `memory_learn/` 一族同刀退役——md 是唯一真源之后，"按 sqlite 命中数
/// 加权排序"是残留的第二真源，且设置页会留一格拨了没反应的开关（裁定见
/// `specs/10-context-injection-s3` 开工三问②）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryInjectConfig {
    pub enabled: bool,
    pub top_k: usize,
}

impl Default for MemoryInjectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            top_k: 5,
        }
    }
}
