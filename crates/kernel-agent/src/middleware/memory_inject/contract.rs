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
    /// 按宿主裁决的相关序（desktop = updated_at 新→旧）取至多 limit 条。
    fn recall(&self, session_id: Option<&str>, limit: usize) -> Vec<MemoryEntry>;

    /// 队列 Q9（ML 稿 §3.3/§3.4 = L2+L3 合一批）：带**相关性词表 + 四信号权重**的召回。
    ///
    /// ⚠ **S3 刀① 起本方法在生产面没有调用点了**：唯一那个调用点住在
    /// `<persistent_memory>` 正文块的组装处，那块已并入 `<memory_index>` 索引段。
    /// 方法与宿主实现**暂时都留着**（去留按 `specs/10-context-injection-s3` 的裁定
    /// = 刀③ 同刀退役），本条文档描述的是"它还连着时"的契约形状。
    ///
    /// **缺省实现 = 忽略 `query` 回落 [`MemorySource::recall`]，这是等价性装置而不是便利**：
    /// ① 五个测试 fake 与 headless 装配零改动；② [`LearnWeights::default`] 的
    /// `enabled = false` + 本缺省支 = 「关掉学习档必须逐字节回到现役」这条要求直接由
    /// 类型给出（收口判据 = 既有记忆面用例一字未改仍然绿）。生产实现（宿主
    /// `MemoryStore`）覆写它 = 四信号现算加权排序 + 命中写回强化计数。
    /// `terms` 为空 = 相关性档不贡献分数（其余三信号仍生效）。
    fn recall_relevant(
        &self,
        session_id: Option<&str>,
        limit: usize,
        query: &RecallQuery,
    ) -> Vec<MemoryEntry> {
        let _ = query;
        self.recall(session_id, limit)
    }

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryInjectConfig {
    pub enabled: bool,
    pub top_k: usize,
    /// 队列 Q9 学习档（总开关 + 四信号权重）：形状见 [`LearnWeights`]，
    /// **权重只在这一处存**——设置面、注入调用点、召回契约共用同一个类型，
    /// 不在别处再摊平一份四字段（同一份权重两个出处 = 下一刀必漂）。
    pub learn: LearnWeights,
}

/// 学习档 = 一个布尔 + 四个千分比权重（队列 Q9 · ML 稿 §3.3 排序、§3.4 强化、
/// §5.4 配置面）。它同时是**内核递给宿主的召回契约载荷**（见
/// [`MemorySource::recall_relevant`]）与**设置页的可调项形状**，两副担子用
/// 同一个类型挑，是为了让「配置改了但没传下去」这类漂移在类型上就不成立。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LearnWeights {
    /// **false = 排序与写回一起关**，注入字节逐字节回到现役
    /// （`ORDER BY updated_at DESC, key`）。稿 §5.4 只要求排序回退，本批把写回
    /// 也纳入同一档：对照组若仍在累积 `hit_count`，A/B 比的就不是同一份库。
    pub enabled: bool,
    /// 四信号权重，**千分比整数**（和 = 1000，校验在 `settings/memory.rs`）。
    /// ⚠ 对稿 §5.4 的实现层订正：稿写的是 float，换成整数是因为配置面靠
    /// `PartialEq` 判「变更即失效重算」，而 float 既不进 `Eq`、四权重相加也不闭合
    /// ⇒ 「权重和为 1」这条无法机器判。
    pub relevance: u16,
    pub strength: u16,
    pub freshness: u16,
    pub recency: u16,
}

/// 权重缺省（稿 §3.3 表的 0.40 / 0.25 / 0.20 / 0.15 ⇒ 千分比）。
/// 四档之和 = 1000 由这个数组一处给定，别处不重抄数字。
pub const LEARN_WEIGHT_DEFAULT: [u16; 4] = [400, 250, 200, 150];

impl Default for LearnWeights {
    /// **`enabled = false`**：这是契约载荷的保守缺省，不是产品缺省——未覆写
    /// [`MemorySource::recall_relevant`] 的装配（五个测试 fake、headless）拿到的
    /// 就是现役行为。产品缺省（开）在 [`MemoryInjectConfig::default`] 里显式写出。
    fn default() -> Self {
        Self {
            enabled: false,
            relevance: LEARN_WEIGHT_DEFAULT[0],
            strength: LEARN_WEIGHT_DEFAULT[1],
            freshness: LEARN_WEIGHT_DEFAULT[2],
            recency: LEARN_WEIGHT_DEFAULT[3],
        }
    }
}

/// 一次召回的请求载荷（队列 Q9）：相关性词表 + 权重。
///
/// 为什么把两者捆成一个类型而不是两个参数：**「learn 关掉」这件事必须只有一个
/// 表达方式**。分两个参数时调用点可以传 `terms = 非空, weights = 关`，宿主实现
/// 就得再判一次"到底算不算开"；捆起来后这种半开状态在构造时就不成立——
/// `RecallQuery::default()` 是空词表 + 关档 = 现役。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecallQuery {
    /// 最近用户消息里取到的关键词表（空 = 相关性档不贡献分数，其余三信号仍生效）。
    pub terms: Vec<String>,
    pub weights: LearnWeights,
}

impl Default for MemoryInjectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            top_k: 5,
            learn: LearnWeights {
                enabled: true,
                ..LearnWeights::default()
            },
        }
    }
}
