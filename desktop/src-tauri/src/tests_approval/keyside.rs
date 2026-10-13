//! 队列 Q7d 主体（v2.199）：**授权规则库的键位合一**——走生产链路的用例。
//!
//! 成因 = 规则文件名吃的是会话旁车里的**原样串**，同一个项目的四种写法落成
//! **四个规则文件** ⇒ 审批卡上点了「本项目始终允许」，换一种写法打开同一个项目
//! 就不认了：allow 形同没批过，手改的 deny 形同虚设（取证稿《Approval Keyside
//! Alignment and Rules Store Migration v1》§4）。本簇钉三件事：
//! ① 换写法打开同一个项目 = 同一条规则仍然生效（免卡直行）；
//! ② 磁盘上只有一个规则文件，名字 = **归一键**的 fnv（反面：原样串那颗键位
//!   不再被生产出来）；
//! ③ 别的项目不许被合并进来（那是越权不是合一）。
//!
//! 期望一律按 `scope_key` 现算、平台两支各写：折叠大小写是 Windows 专属语义，
//! POSIX 上 `/tmp/GhostProj` 与 `/tmp/ghostproj` 是两个真不同的目录（同
//! `workspace/tests.rs` 的口径——把串形写死会得到一条 Linux 绿、Windows 红的假判据）。

use super::*;

use std::path::Path;

/// 归属替身：`s1*` 归给指定写法的工作区串，其余未归属。
fn ctx_giving(ws: &'static str) -> crate::workspace::SessionContextResolver {
    Arc::new(move |sid: &str| CallContext {
        session_id: Some(sid.to_string()),
        workspace: sid.starts_with("s1").then(|| ws.to_string()),
        fs_roots: Vec::new(),
    })
}

/// 规则目录里的文件名清单（②的观察面）。
fn listed_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path().file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[tokio::test]
async fn same_project_written_differently_shares_one_allow_rule() {
    #[cfg(windows)]
    let (first, again, elsewhere) = (
        r"F:\Warehouse\GhostProj",
        r"f:/warehouse/GHOSTPROJ/", // 大小写 + 分隔符 + 尾分隔符三种漂移同时踩上
        r"F:\Warehouse\GhostProj-Other",
    );
    #[cfg(not(windows))]
    let (first, again, elsewhere) = (
        "/tmp/GhostProj",
        "/tmp/GhostProj/", // POSIX 只认尾斜杠这一种漂移
        "/tmp/ghostproj",  // 大小写不同 = 另一个项目，不许合
    );

    let root = tmp_root("keyside");
    let rules = Arc::new(PermissionRuleStore::new(&root.join("data")));
    let modes = Arc::new(PermissionModeStore::new(
        root.join("sessions"),
        PermissionMode::Ask,
    ));
    // 同一份规则库、同一个档位，只换 ctx 下发的写法 = 换一种写法打开项目
    let hub_on = |ws: &'static str| {
        Arc::new(
            ApprovalHub::with_timeout(
                Arc::new(Collector::default()),
                std::time::Duration::from_millis(80),
            )
            .with_modes(Arc::clone(&modes))
            .with_workspace(ctx_giving(ws))
            .with_rules(Arc::clone(&rules)),
        )
    };

    // 第一次（原样写法）：Ask 档弹卡 → 后台按「本项目始终允许」决议
    let hub = hub_on(first);
    let decider = Arc::clone(&hub);
    let worker = tokio::spawn(async move {
        loop {
            if let Some(req) = decider.pending().first().cloned() {
                decider
                    .decide(&req.request_id, true, None, Some("project"))
                    .expect("decision accepted");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    });
    let d = hub
        .resolve(&shell_req("s1", "npm test"), &shell_spec())
        .await;
    worker.await.unwrap();
    assert!(d.allowed, "决策后 step 醒来放行");

    // ② 磁盘面：一个项目一个文件，文件名 = 归一键
    let dir = root.join("data").join("permissions");
    let names = listed_files(&dir);
    assert_eq!(
        names,
        vec![rules_file_of(first)],
        "规则文件名必须吃归一键（同串才能同文件）"
    );
    #[cfg(windows)]
    assert!(
        !names.contains(&format!("{:08x}.json", fnv1a64(first.as_bytes()))),
        "文件名仍在吃原样串 = 归一没发生"
    );

    // ① 换写法打开同一个项目：同一条规则仍生效，免卡直行
    let d = hub_on(again)
        .resolve(&shell_req("s1", "npm test"), &shell_spec())
        .await;
    assert!(
        d.allowed,
        "写法「{again}」下批过的规则必须仍然生效（改前这里是漏批）"
    );
    assert_eq!(d.reason.as_deref(), Some("rule:project-allow"));

    // ③ 另一个项目不许命中（弹卡 → 守夜 80ms 自动拒绝），且不许造出第二个文件
    let d = hub_on(elsewhere)
        .resolve(&shell_req("s1", "npm test"), &shell_spec())
        .await;
    assert!(
        !d.allowed,
        "归一不许把别的项目的授权合过来（那是越权不是合一）"
    );
    assert_eq!(listed_files(&dir), names, "别的项目不许多出一个规则文件");
    let _ = std::fs::remove_dir_all(&root);
}
