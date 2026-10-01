//! Windows 命令行转义单测（MSVCRT 规则）——spawn 全链由 exec 层 e2e 覆盖
//! （真跑 `git --version`，含受限令牌/Job 挂载路径与降级上报）。

use super::quote_arg;

#[test]
fn plain_tokens_stay_bare() {
    assert_eq!(quote_arg("git"), "git");
    assert_eq!(quote_arg("--version"), "--version");
}

#[test]
fn spaces_are_quoted() {
    assert_eq!(quote_arg("a b"), "\"a b\"");
    assert_eq!(quote_arg(""), "\"\"");
    assert_eq!(quote_arg("tab\there"), "\"tab\there\"");
}

#[test]
fn quotes_and_backslashes_fold_correctly() {
    // "hello" → \"hello\"
    assert_eq!(quote_arg("\"hello\""), "\"\\\"hello\\\"\"");
    // 路径尾反斜杠在闭引号前翻倍：C:\a b\ → "C:\a b\\"
    assert_eq!(quote_arg("C:\\a b\\"), "\"C:\\a b\\\\\"");
    // 引号前的反斜杠：a\"b → "a\\\"b"
    assert_eq!(quote_arg("a\\\"b"), "\"a\\\\\\\"b\"");
}
