use super::*;

/// Fallback 矩阵 §7"列表崩"行：已装目录里 manifest.json 损坏 → 该行仍列出，
/// 展示投影降级为缺省，列表整体不崩。
#[tokio::test]
async fn list_installed_survives_broken_manifest_on_disk() {
    let data = temp_dir("broken-manifest");
    let market_root = temp_dir("broken-market");

    // 手写 lockfile 条目 + 只含坏 manifest 的安装目录（模拟磁盘残留损坏态）
    let mut lf = LockFile {
        schema_version: crate::lockfile::LOCKFILE_SCHEMA_VERSION,
        ..LockFile::default()
    };
    lf.add(
        "tool-broken",
        Version::new(1, 0, 0),
        1,
        crate::lockfile::InstallSource::Market,
    );
    lf.save(&data.join("lockfile.json")).unwrap();
    let ver_dir = data.join("plugins").join("tool-broken").join("1.0.0");
    std::fs::create_dir_all(&ver_dir).unwrap();
    std::fs::write(ver_dir.join("manifest.json"), b"{not json").unwrap();

    let svc = service_in(data, market_root);
    let listed = svc.list_installed();

    assert_eq!(listed.len(), 1, "坏 manifest 不应把整张列表打崩");
    assert_eq!(listed[0].id, "tool-broken");
    assert_eq!(listed[0].version, "1.0.0");
    // 投影降级为缺省：展示字段全 None + community + 无副作用徽章
    assert!(listed[0].display.display_name.is_none());
    assert!(listed[0].display.description.is_none());
    assert!(listed[0].display.icon.is_none());
    assert_eq!(
        listed[0].display.origin,
        plugin_api::manifest::PluginOrigin::Community
    );
    assert!(listed[0].display.side_effects.is_empty());
}

/// 展示投影跟随 active 指针（§10.7）：安装 → 更新到新版 → 补丁修改新版
/// 目录的 display_name（模拟新版元数据）→ 投影切新；回退 → 投影切回旧版。
/// 两个版本目录并存（前端版本下拉的数据源）。
#[tokio::test]
async fn display_projection_follows_active_version_pointer() {
    let vers = staged_versions();
    if vers.len() < 2 {
        eprintln!("skip: market/tool-shout 需至少两个暂存版本（先跑 scripts/build_market.sh）");
        return;
    }
    let old = &vers[0];
    let new = &vers[vers.len() - 1];
    let data = temp_dir("disp-active");
    let market_root = temp_dir("disp-market");
    fake_market_from_staged(&market_root, &vers);
    let svc = service_in(data.clone(), market_root);

    svc.install("tool-shout", Some(&old.to_string()))
        .await
        .unwrap();
    let baseline = svc.list_installed()[0]
        .display
        .display_name
        .clone()
        .expect("暂存 manifest 自带 display_name");

    // 更新到 latest：active 切到新版目录
    let got = svc.update("tool-shout").await.unwrap();
    assert_eq!(got.to_string(), new.to_string());

    // 补丁改新版安装目录的 display_name → 投影立即切新（证明读的是
    // active 目录，不是旧版目录）
    let new_manifest = data
        .join("plugins")
        .join("tool-shout")
        .join(new.to_string())
        .join("manifest.json");
    let mut patched: plugin_api::serde_json::Value =
        plugin_api::serde_json::from_str(&std::fs::read_to_string(&new_manifest).unwrap()).unwrap();
    patched["display_name"]["en-US"] = plugin_api::serde_json::json!("Patched v2 Name");
    std::fs::write(&new_manifest, patched.to_string()).unwrap();
    let listed = svc.list_installed();
    assert_eq!(
        listed[0]
            .display
            .display_name
            .as_ref()
            .unwrap()
            .get("en-US")
            .map(String::as_str),
        Some("Patched v2 Name"),
        "投影应取 active（新版）目录的 manifest"
    );

    // 回退：只切指针，无需下载 → 投影切回旧版目录
    svc.rollback("tool-shout", &old.to_string()).await.unwrap();
    let listed = svc.list_installed();
    assert_eq!(listed[0].active_version, Some(old.to_string()));
    assert_eq!(
        listed[0].display.display_name,
        Some(baseline),
        "回退后投影切回旧版目录"
    );
    // 两个版本目录并存（前端版本下拉的数据源）
    assert!(
        data.join("plugins")
            .join("tool-shout")
            .join(new.to_string())
            .is_dir()
    );
    assert!(
        data.join("plugins")
            .join("tool-shout")
            .join(old.to_string())
            .is_dir()
    );
}
