//! organize 单测：影子编排全链路——记忆管理器成功 / 垃圾输出降级规则切分 /
//! 增量触发 / 反弹熔断 / 崩溃重投（会话在盘）/ 会话已删放弃。设置经临时
//! 目录 settings.json 直写（serde 缺省补齐其余字段，绕开 settings_set
//! 校验以便用极小触发阈值）。

//! 拆分说明（v2.158 批 F-8）：本模块只留**共享夹具**（假记忆管理器 / 临时数据根 /
//! settings 直写 / 会话与材料构造 / 投影读取），19 条用例按职责分派到 6 个子文件——
//! `pipeline`（管道与触发决策）· `recovery`（崩溃重投）· `overflow`（窗口溢出 + 自动收起）·
//! `views`（观测与编辑面）· `tools`（检索与历史工具）· `summary`（轮级总结）。
//! 夹具是 tests 的私有项，子簇经 `use super::*;` 取用（形状照 `settings/tests/` 先例，
//! **可见性零改动**）；用例本体逐字搬运，一条断言未增删。

mod fact_cards;
mod overflow;
mod pipeline;
mod recovery;
mod summary;
mod tail;
mod tools;
mod views;

use std::collections::VecDeque;

use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;

use memory_tree::{MemoryManagerClient, MemoryManagerUsage, OrganizeReason, TreeStore};

use session_store::{Session, SessionEvent};

use super::Organizer;

use crate::settings::SettingsStore;

/// 脚本化假记忆管理器：按序返回原始回复（Ok 文本 / Err 借口）。
struct FakeClient {
    responses: StdMutex<VecDeque<Result<String, String>>>,
}

impl FakeClient {
    fn new(responses: Vec<Result<String, String>>) -> Arc<Self> {
        Arc::new(Self {
            responses: StdMutex::new(responses.into()),
        })
    }
}

#[async_trait]
impl MemoryManagerClient for FakeClient {
    async fn complete(
        &self,
        _system: &str,
        _user: &str,
        _model: Option<&str>,
    ) -> Result<(String, MemoryManagerUsage), String> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| "没有脚本回复".to_string())?
            .map(|raw| {
                (
                    raw,
                    MemoryManagerUsage {
                        input: 100,
                        output: 40,
                    },
                )
            })
    }
}

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = crate::testutil::work_dir(&format!("orgz-{}", tag));
    dir
}

/// 直写 settings.json（camelCase；其余字段 serde default 补齐）。
fn settings_with(json: serde_json::Value, root: &std::path::Path) -> Arc<SettingsStore> {
    std::fs::write(root.join("settings.json"), json.to_string()).unwrap();
    Arc::new(SettingsStore::open(root).unwrap())
}

fn resolver() -> crate::workspace::WorkspaceResolver {
    Arc::new(|_sid: &str| Some(r"F:\ws\demo".to_string()))
}

/// 六事件会话：三轮问答（3 User + 3 Assistant）。
fn session() -> Session {
    let mut s = Session::new();
    for i in 0..3 {
        s.append(SessionEvent::UserMessage {
            content: format!("第{i}轮问题"),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("第{i}轮回答"),
            tool_calls: vec![],
        });
    }
    s
}

/// 合法铺满 6 项材料的记忆管理器输出（两章节）。
fn good_reply() -> String {
    r#"[{"title":"话题一","summary":"摘要一。","event_range":[0,1]},{"title":"话题二","summary":"摘要二。","event_range":[2,5]}]"#.into()
}

fn organizer(
    root: &std::path::Path,
    settings: Arc<SettingsStore>,
    client: Arc<dyn MemoryManagerClient>,
) -> Arc<Organizer> {
    Arc::new(Organizer::new(
        settings,
        root.join("sessions"),
        client,
        resolver(),
        root.to_path_buf(),
        Arc::new(crate::model_override::ModelOverrideStore::new(
            root.join("sessions"),
        )),
        // 目录缺席 = 空快照（测试不依赖目录能力面）
        Arc::new(crate::llm::catalog::CatalogStore::new(
            crate::testutil::unique_dir("orgz-catalog-absent"),
        )),
        std::sync::Arc::new(|_, _| {}), // 哑通知（不触 emit）
        None,                           // 碎片表缺席（本批不测置位面）
        // 事实卡落库句柄（Q3）：与测试侧 `memory_store` 同一个数据根 ⇒ 同一个
        // sqlite 文件（两个实例、一份盘）。md 面未接线（`files: None`）= 卡片
        // 只落 sqlite，人类层文件不该在整理测试目录里凭空长出来。
        memory_store(root),
    ))
}

/// 记忆层夹具（Q3 事实卡）：`MemoryStore::new` 只做懒连接、不碰盘 ⇒ 不用卡片
/// 的既有用例零副作用；断言侧另开一个实例读同一个文件即可。
pub(super) fn memory_store(root: &std::path::Path) -> Arc<crate::memory::MemoryStore> {
    Arc::new(crate::memory::MemoryStore::new(root, resolver()))
}

fn tree_at(
    root: &std::path::Path,
    _settings: &SettingsStore,
    session_id: &str,
) -> memory_tree::MemoryTree {
    TreeStore::new(root)
        .load(r"F:\ws\demo", session_id)
        .unwrap_or_else(|| memory_tree::MemoryTree::new(r"F:\ws\demo", session_id))
}

/// 树模式编排器（历史上溢出/收起要 Tree 档；v2.137 后与 organizer 同构）。
fn tree_organizer(
    root: &std::path::Path,
    settings: Arc<SettingsStore>,
    client: Arc<dyn MemoryManagerClient>,
) -> Arc<Organizer> {
    organizer(root, settings, client)
}

/// 十二轮会话（溢出窗口的孤儿轮素材）。
fn long_session() -> Session {
    let mut s = Session::new();
    for i in 0..12 {
        s.append(SessionEvent::UserMessage {
            content: format!("第{i}轮问题"),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("第{i}轮回答"),
            tool_calls: vec![],
        });
    }
    s
}

/// 合法铺满 6 项材料 + 尾随总结段的记忆管理器输出。
fn good_reply_with_summary() -> String {
    format!(
        "{}\n===SUMMARY===\n本轮完成了插件加载排查，结论是共享 runtime；仍未决：缓存命中率观测。",
        good_reply()
    )
}
