/// 台账 T11 的对账：磁盘 `manifest.json` 必须与代码 `describe_manifest()` 同源
/// （比对规则与"为什么要有这条"都写在 `plugin_api::meta::manifest_drift`）。
/// `version` 有意不比——它由 `scripts/build_plugins.sh` 的 PLAN 清单注入两侧
/// （代码侧 env 宏 + 产物 JSON 的 sed），源码里那份只是占位。
#[test]
fn manifest_json_and_describe_agree() {
    let described = crate::describe_manifest();
    // 维护通道（不是判据的一部分）：`LOCOWRK_MANIFEST_SYNC=1` 时把清单对齐到代码。
    // 默认路径永远**只报不改**——判据不许顺手改受版本控制的文件，对齐之后还要人看 diff。
    if std::env::var("LOCOWRK_MANIFEST_SYNC").is_ok_and(|v| v == "1") {
        let written = plugin_api::meta::sync_manifest_file(&described, env!("CARGO_MANIFEST_DIR"))
            .expect("同步需要能读写清单文件");
        println!("已把 {written} 对齐到代码 describe，核对 diff 后提交");
        return;
    }
    let drift = plugin_api::meta::drift_of_describe(&described, env!("CARGO_MANIFEST_DIR"));
    assert!(
        drift.is_empty(),
        "磁盘 manifest.json 与代码 describe 漂了（能力字段以代码为真、展示字段两边同步）：\n{}",
        drift.join("\n")
    );
}
