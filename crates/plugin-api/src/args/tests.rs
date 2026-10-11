//! `arg_u64` 的等价测试：把**被删掉的六份原文**当参考实现，跑同一批输入逐字段
//! （含错误报文字节）对账。先例 = 批 F-1 `now_ms` / 批 F-2 `clip`：参考实现必须是
//! 被删的那份，不是我另写一个"看起来对"的。
//! 六份原文实测只有两种写法：表达式形（memory-sqlite / tool-git / tool-shell）与
//! `let … ?` 形（tool-fetch / tool-fs / tool-notify），两形都原样抄在下面。
use super::arg_u64;
use serde_json::{Map, Value, json};

fn ref_expr(args: &Value, key: &str, default: u64, max: u64) -> Result<u64, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .map(|n| n.min(max))
            .ok_or_else(|| format!("field `{key}` must be a non-negative number")),
    }
}

fn ref_let(args: &Value, key: &str, default: u64, max: u64) -> Result<u64, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => {
            let n = v
                .as_u64()
                .ok_or_else(|| format!("field `{key}` must be a non-negative number"))?;
            Ok(n.min(max))
        }
    }
}

/// 输入矩阵：缺省 / 显式 null / 正常值 / 越界截断 / 零不是缺省 / 负数 / 小数 /
/// 字符串数字 / 布尔 / `max = u64::MAX` 的不截断用法 / 容器不是对象 / 键不同名。
/// 三个短名常量 + `o(k, v)`（造 `{k: v}` 对象）只为把每例压进一行：把键名写两遍会
/// 顶破 rustfmt 的 100 列上限，而列宽是这张表唯一的可读性来源。
const K: &str = "timeout_secs";
const OFF: &str = "offset";
const LIM: &str = "limit";
const MAXU: u64 = u64::MAX;

fn o(k: &str, v: Value) -> Value {
    let mut m = Map::new();
    m.insert(k.to_owned(), v);
    Value::Object(m)
}

fn cases() -> Vec<(&'static str, &'static str, Value, u64, u64)> {
    vec![
        ("键不存在", K, json!({}), 20, 120),
        ("显式 null", K, o(K, Value::Null), 20, 120),
        ("正常值", K, o(K, json!(30)), 20, 120),
        ("越界截断", K, o(K, json!(999)), 20, 120),
        ("零不是缺省", K, o(K, json!(0)), 20, 120),
        ("负数", K, o(K, json!(-1)), 20, 120),
        ("小数", K, o(K, json!(1.5)), 20, 120),
        ("字符串数字", K, o(K, json!("30")), 20, 120),
        ("字符串", K, o(K, json!("x")), 20, 120),
        ("布尔", K, o(K, json!(true)), 20, 120),
        ("max 不设限", OFF, o(OFF, json!(MAXU)), 0, MAXU),
        ("容器不是对象", K, json!(42), 20, 120),
        ("另一键名", LIM, o(LIM, json!(5)), 50, 200),
    ]
}

#[test]
fn arg_u64_matches_both_deleted_shapes_on_every_case() {
    for (label, key, args, default, max) in cases() {
        let got = arg_u64(&args, key, default, max);
        // 两个参考实现是不同的 fn item，数组字面量要显式收到 fn 指针上
        // （`E0308 expected fn item, found a different fn item` 实测撞过一次）。
        type Ref = fn(&Value, &str, u64, u64) -> Result<u64, String>;
        let refs: [(&str, Ref); 2] = [("表达式形", ref_expr), ("let-? 形", ref_let)];
        for (who, reference) in refs {
            assert_eq!(
                reference(&args, key, default, max),
                got,
                "[{label}] 基座 {got:?} 与{who}参考实现不等"
            );
        }
    }
}

#[test]
fn arg_u64_error_message_names_the_field_and_matches_both_shapes_byte_for_byte() {
    // 报文是这一族最容易改漂的东西（六份原文一模一样 ⇒ 上收后只剩一个出处；
    // 批 F-5/F-6 的 F12/F12b 报错教学以后要动它，也只能动一处）。
    for key in [LIM, K, "max_bytes"] {
        let bad = o(key, json!("x"));
        let msg = arg_u64(&bad, key, 1, 9).unwrap_err();
        let want = format!("field `{key}` must be a non-negative number");
        assert_eq!(msg, want, "{key} 的报文变了");
        assert_eq!(msg, ref_let(&bad, key, 1, 9).unwrap_err(), "let-? 形不等");
        assert_eq!(msg, ref_expr(&bad, key, 1, 9).unwrap_err(), "表达式形不等");
    }
}
