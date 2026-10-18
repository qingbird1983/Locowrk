//! 写路径的**字符级上限**（台账 T35）：`key` 与 `value` 的字节上限——key 参与文件名
//! 与主键所以不能无限长，value 防 runaway 写。
//!
//! 两个 const 单独成立一片是刻意的：它们是"能被写进来的东西有多大"这一件事，
//! 而"库/目录长什么样"是另一件事（后者自 S1 刀③b 起住在 `memlayout` 的布局面）。
//! 两者都被工具入口读 ⇒ 抬 `pub(super)`，只在 `store` 子树内可见，插件外部面一个字没变。
//!
//! 随刀③b 退役的还有第三个 const `ESC`（LIKE 的通配符转义符）：文件侧的子串匹配是
//! `memlayout::fact_matches`，没有需要转义的语法层。
pub(super) const KEY_MAX: usize = 512;
pub(super) const VALUE_MAX: usize = 256 * 1024;
