//! 真源侧存储实现（S1 刀③b）：本插件那六枚工具读写的就是 `<数据根>/memory/` 下的 md
//! ——布局、渲染、解析、索引刷新全部经 `plugin_api::memlayout`（宿主与插件**同一份
//! 实现**，同一键位必落同一文件）。
//!
//! 这一刀退役掉的东西一起写在这里，免得下一轮又被"怎么没了"问一遍：`rusqlite` 依赖 ·
//! `store/schema.rs`（建表串 + 两份旧库迁移 + `open`）· `store/rows.rs` 的 SQL 读侧件
//! （`row_json` / `row_scoped_json` / `row_with_workspace_json` / `exists` / `escape_like`）
//! · `mdsync` 那层"sqlite 成功后同步 md"与它带回的 `md_warning` 字段 · 本插件对 env
//! `LOCOWRK_MEMORY_FILES` 的读取。理由只有一句：**真源 = 文件之后，"两份存储、谁跟谁
//! 同步"这个问题本身不存在了**（《Memory File-ization Design v1.1》§8 决策点 1 撤销的
//! 就是那套双层双写）。
//!
//! ⚠ `memory.sqlite3` **仍然存在**，但住在宿主那一侧：它是影子（由
//! `desktop/src-tauri/src/memory_reconcile.rs` 从真源单向重建）兼学习量两列的家
//! （`hit_count` / `last_hit_at`；那一格等拍板的口径见 specs/05 刀②c-2 第 2 件）。
//! 存量库里没进过 md 的行按作者 2026-10-18 裁定**不回填**——"免得污染，我到时会清理掉的"。
//!
//! 项目隔离与键位归一的纪律一个字没动：主键形状 `(workspace, key)` 现在由
//! `memlayout::fact_rel_path` 编码成"目录 + 文件名"，归一仍只在 `WsCtx::from_parts` 里的
//! `wskey::normalize` 做一次（写侧 `write_scope`、读侧 `scope()`、md 目录名三处取的
//! 都是同一个串 ⇒ 一处归一、三处同形；宿主侧 `workspace::scope_key` 调的是同一个函数）。
//!
//! 台账第三节 · 第二批第二刀那次拆面的历史（五个面沉进 `store/`）仍然有效，只是
//! 五片换成了 `facts`（真源读件 + 键位裁决）· `limits`（两个字符级上限）·
//! `tools`（五枚工具入口）· `undo`（补偿入口）——`schema` 与 `mdsync` 两片随本刀消失，
//! 原先"整块留在父文件没提成本级文件"那条权衡也跟着作废（那块东西没了）。
//! 上面那条 `pub(crate) use crate::WsCtx;` 是给各片的 `super::WsCtx` 写法接根的：
//! 切片后各片的 `super` 是 `store` 而不是 crate root。

pub(crate) use crate::WsCtx;

mod facts;
mod limits;
mod tools;
mod undo;

pub(crate) use tools::*;
pub(crate) use undo::*;
