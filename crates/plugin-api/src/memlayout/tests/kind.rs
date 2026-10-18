//! 类型面（S1 刀③a）：`metadata.type` 的落盘与读回、值域归一、由类型算落点，
//! 以及 `feedback` / `project` 缺 `Why:` / `How to apply:` 时的那句提醒。
//!
//! 夹具纪律照本目录头注：**一律经生产渲染落盘**，不手抄 md。唯一允许手抄的是
//! "旧形状"那一档（下面 `an_old_file_without_a_type_line…`），抄的本子 = `render()`
//! 去掉 `  type:` 那一行的历史形状。

use super::*;

/// 不给类型 = 写 `fact`：这一条是"四类型落地之前每个文件的字节形一字不变"的担保。
/// 另一半担保由既有测试承担——`read_back` / `write_delete` 里那些手抄的旧 frontmatter
/// 本子（含 `  type: fact`）在这一刀之后**一个字都没改仍然绿**，说明读侧与写侧同形。
#[test]
fn no_type_and_a_literal_fact_render_the_same_bytes() {
    let unset = entry(r"F:\proj\Kind", "k.a", "value text");
    let explicit = FactEntry {
        ty: "fact",
        ..entry(r"F:\proj\Kind", "k.a", "value text")
    };
    assert_eq!(
        unset.render(),
        explicit.render(),
        "空串与 `fact` 必须同形（否则存量文件会被判成两种东西）"
    );
    assert!(
        unset.render().contains("  type: \"fact\"\n"),
        "未分类落盘就是 fact：{}",
        unset.render()
    );
}

/// 四类型原样往返：模型报什么类型，读回来就是那一个（值域内不加工、不翻译）。
#[test]
fn each_known_type_round_trips_through_the_frontmatter() {
    for ty in MEMORY_TYPES {
        let root = tmp_root(&format!("kind-{ty}"));
        let ws = r"F:\proj\Kind";
        write_fact(
            &root,
            &FactEntry {
                ty,
                ..entry(ws, "k.round", "正文")
            },
        )
        .unwrap();
        let back = read_fact(&root, ws, "k.round").unwrap().expect("读得回来");
        assert_eq!(back.ty, ty, "{ty} 往返不变");
        // 落盘那一行确实是渲染出来的，不是读侧凭默认值兜出来的
        let text = std::fs::read_to_string(&back.path).unwrap();
        assert!(text.contains(&format!("  type: \"{ty}\"")), "{text}");
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// 值域外（拼错 / 大小写漂 / 人自己写了个别的）一律收成未分类，**不拦写也不报错**：
/// 真源是可被人改写的文件，读侧不许因为一行frontmatter 把整条记忆判没了。
#[test]
fn an_out_of_range_type_is_folded_to_unclassified_on_both_sides() {
    let root = tmp_root("kind-unknown");
    let ws = r"F:\proj\Kind";
    write_fact(
        &root,
        &FactEntry {
            ty: "Preference",
            ..entry(ws, "k.odd", "正文")
        },
    )
    .unwrap();
    let text = std::fs::read_to_string(root.join(fact_rel_path(ws, "k.odd"))).unwrap();
    assert!(
        text.contains("  type: \"fact\""),
        "写侧就收进值域，落盘不留 `Preference`：{text}"
    );
    assert_eq!(read_fact(&root, ws, "k.odd").unwrap().unwrap().ty, "fact");
    // 大小写与首尾空白不敏感（同一类型不许落两种串）
    assert_eq!(normalize_type(" Feedback "), "feedback");
    assert_eq!(normalize_type(""), UNCLASSIFIED_TYPE);
    let _ = std::fs::remove_dir_all(&root);
}

/// 旧文件根本没写 `  type:` 那一行 ⇒ 读成未分类，**不回填、不重写**（设计 §2.1）。
/// 这一条必须手抄文件：它测的就是"生产再也没写过这个形状"的那批存量。
#[test]
fn an_old_file_without_a_type_line_reads_as_unclassified_and_stays_untouched() {
    let root = tmp_root("kind-legacy");
    let dir = root.join(GLOBAL_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("k.old.md");
    let before = "---\nname: \"k.old\"\ndescription: \"d\"\nmetadata:\n  \
                  source: \"explicit\"\n  workspace: \"\"\n  updated: 1700000000\n---\n\n旧内容\n";
    std::fs::write(&path, before).unwrap();
    let back = read_fact(&root, "", "k.old").unwrap().expect("读得回来");
    assert_eq!(back.ty, UNCLASSIFIED_TYPE, "没那行 = 未分类，不是故障");
    assert_eq!(back.value, "旧内容", "正文照旧读得出");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        before,
        "读一次不许改写文件（回填存量 = 摧毁人的 mtime 保护）"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 落点层的缺省与覆盖（纯函数，不碰盘）。
#[test]
fn layer_for_lets_the_explicit_scope_win_and_the_type_decide_the_rest() {
    // 类型缺省：user / feedback → 全局；project / reference → 项目
    assert_eq!(layer_for("user", None), Layer::Global);
    assert_eq!(layer_for("feedback", None), Layer::Global);
    assert_eq!(layer_for("project", None), Layer::Project);
    assert_eq!(layer_for("reference", None), Layer::Project);
    // 未分类（含旧 fact 与空串）走**项目** = 现状行为：今天每条记忆都落当前项目键位，
    // "读不回类型"不许被解释成"它其实是全局偏好"
    assert_eq!(layer_for("fact", None), Layer::Project);
    assert_eq!(layer_for("", None), Layer::Project);
    // 显式 scope 覆盖类型缺省（两个方向都要能覆盖）
    assert_eq!(layer_for("user", Some("project")), Layer::Project);
    assert_eq!(layer_for("project", Some("global")), Layer::Global);
    // 大小写 / 空白不敏感；值域外的 scope 忽略覆盖、回落类型缺省（不许猜成全局）
    assert_eq!(layer_for("USER", None), Layer::Global);
    assert_eq!(layer_for("project", Some(" Global ")), Layer::Global);
    assert_eq!(layer_for("user", Some("world")), Layer::Global);
    assert_eq!(layer_for("project", Some("world")), Layer::Project);
}

/// 提醒判据只管两件事：哪两个类型要提醒、缺哪一段。它**不是拒绝**（写不写都落盘）。
#[test]
fn guidance_asks_the_two_disciplined_types_and_nobody_else() {
    // 未分类 / user / reference 都不带这两段纪律
    assert!(missing_guidance("fact", "一句话").is_none());
    assert!(missing_guidance("user", "永远用 pnpm").is_none());
    assert!(missing_guidance("reference", "接口在 docs/adr").is_none());
    // feedback / project 缺两段 ⇒ 提醒，且文案点名缺的是哪一段（三种缺法各一条文案，
    // 数"文案里出现了哪个字段名"就是在数"它报的是哪一段"）。变量按**缺的那一段**命名，
    // 第一版我按"在场的那一段"起名，把两条断言写反、当场被自己的用例抓住。
    let both = missing_guidance("feedback", "别再往 store 里塞全局状态").unwrap();
    assert!(
        both.contains("Why:") && both.contains("How to apply:"),
        "{both}"
    );
    let missing_how = missing_guidance("project", "Why: 因为并发写会撕裂").unwrap();
    assert!(
        missing_how.contains("`How to apply:`") && !missing_how.contains("a `Why:`"),
        "只缺 How 就不该报 Why：{missing_how}"
    );
    let missing_why = missing_guidance("project", "How to apply: 改前先加锁").unwrap();
    assert!(
        missing_why.contains("`Why:`") && !missing_why.contains("How to apply"),
        "只缺 Why 就不该报 How：{missing_why}"
    );
    // 两段都在场 ⇒ 不提醒；写法宽松（前缀符号、大小写、句中）都算在场
    assert!(
        missing_guidance("FEEDBACK", "- **Why:** 踩过\n- How to apply: 先写测试").is_none(),
        "两段齐 ⇒ 闭嘴"
    );
}
