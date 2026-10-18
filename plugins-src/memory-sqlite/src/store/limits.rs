//! 这一侧写路径的三个**字符级上限**（台账 T35）：`key` 与 `value` 的字节上限
//! （key 是主键不能无限长，value 防 runaway 写）与 LIKE 的通配符转义符。
//! 三个 const 一共 5 行，单独成立一片是刻意的：它们是"能被写进来的东西有多大"这一件事，
//! 而表形状（`CREATE_TABLE`）是"库长什么样"另一件事——后者跟着开库路径在 `schema.rs`。
//! 三者都被工具入口读 ⇒ 抬 `pub(super)`，只在 `store` 子树内可见，插件外部面一个字没变。
/// key / value 上限（字节）。key 是主键不能无限长；value 防 runaway 写。
pub(super) const KEY_MAX: usize = 512;
pub(super) const VALUE_MAX: usize = 256 * 1024;
/// LIKE 通配符转义符（SQL 语法层面）。
pub(super) const ESC: char = '\\';
