//! LLM 档位制测试（v2.58，DESIGN §13.1）：旧平面迁移 / 生效投影 /
//! 平面更新物化缺省档 / 切档校验 / 档位命令层语义（save/delete）。
//! 批 F-13 把三面拆成同目录兄弟簇（**用例本体逐字未动**）：档位命令层与子代理钉档见
//! `llm_profile`、provider 钉模与展示视图见 `llm_provider`、网络代理见 `network_proxy`。

use super::*;

// ---- 旧平面 llm 区迁移（open 读入，幂等） ----

#[test]
fn legacy_flat_llm_migrates_to_default_profile() {
    let root = temp_root("llm-migrate");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join(FILE_NAME),
        r#"{"llm":{"provider":"deepseek","model":"deepseek-flash","baseUrl":"https://api.deepseek.com","apiKey":"sk-old","windowTokens":131072}}"#,
    )
    .unwrap();
    let store = SettingsStore::open(&root).unwrap();
    // 生效投影 = 档位原值（平面字段语义不变）
    let v = store.view().llm;
    assert_eq!(v.active, "default");
    assert_eq!(v.providers.len(), 1);
    assert_eq!(v.provider, "deepseek");
    assert_eq!(v.model, "deepseek-flash");
    assert_eq!(v.base_url.as_deref(), Some("https://api.deepseek.com"));
    assert!(v.api_key_set);
    assert_eq!(v.window_tokens, Some(131072));
    // 旧档位无 enabled 字段 = 启用（字段级 serde 缺省；布尔缺省 false 会
    // 把存量档全变停用——30c 走查实抓后修）
    assert!(v.providers[0].enabled);
    // 后端侧（含 key 原值）同源
    assert_eq!(store.llm().api_key.as_deref(), Some("sk-old"));
    // 幂等：再次 open 不二次包裹
    let again = SettingsStore::open(&root).unwrap();
    assert_eq!(again.view().llm.providers.len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

// ---- 生效投影与平面更新 ----

/// **IPC 键名契约**（批 F-6 新立，配合 `settings/llm.rs` → `settings/llm/` 拆分）：
/// `rename_all = "camelCase"` 是 derive 属性——编译器只保证 Rust 字段能编译，
/// 既有 `view()` 断言读的也是 Rust 字段，**没有一条证明前端吃到的键名**。而前端
/// invoke 就是按 `baseUrl` / `windowTokens` / `contextWindow` 取值的，属性一掉
/// 整片设置页静默变空。
///
/// 断言**按层各查各的键集**（顶层 / providers[0] / models[0] / sources /
/// networkProxy / subagent），不递归成一个全集：第一版就是递归收全集，结果
/// 摘掉 `LlmSettingsView` 的 rename_all 后它的 `base_url` 被 `ProviderView` 的
/// `baseUrl` 遮蔽，三条变异全停在同一个无关键上——红是红了，但报不出是谁掉的
/// （v2.156 批 F-6 变异验证实录）。分层后摘哪层属性就缺哪层的键，可定位。
#[test]
fn llm_ipc_json_shape_is_camel_case_end_to_end() {
    let root = temp_root("llm-camel-shape");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join(FILE_NAME),
        r#"{"llm":{"active":"a","providers":[{"id":"a","label":"A","kind":"openai","baseUrl":"https://api.example.com/v1","apiKey":"sk-1","model":"m1","windowTokens":131072,"enabled":true,"headers":{"X-Trace":"1"},"models":[{"id":"m1","alias":"主用","contextWindow":262144,"maxOutput":8192,"availableForSubagents":true}]}],"networkProxy":{"mode":"custom","url":"http://127.0.0.1:7890"},"subagent":{"providerId":"a","model":"m1","thinkingLevel":null}}}"#,
    )
    .unwrap();
    let store = SettingsStore::open(&root).unwrap();
    let json = serde_json::to_value(&store.view().llm).unwrap();

    fn cur_keys(v: &serde_json::Value) -> Vec<String> {
        v.as_object()
            .unwrap_or_else(|| panic!("IPC JSON 该位置不是对象: {v}"))
            .keys()
            .cloned()
            .collect()
    }
    // 路径取到某一层对象，返回它的键集（"0" = 数组首项）
    let keys_at = |path: &[&str]| -> Vec<String> {
        let mut cur = &json;
        let mut walked: Vec<&str> = Vec::new();
        for seg in path {
            walked.push(seg);
            cur = match cur {
                serde_json::Value::Object(m) => m
                    .get(*seg)
                    .unwrap_or_else(|| panic!("IPC JSON 没有路径 {walked:?}")),
                serde_json::Value::Array(a) if *seg == "0" => a.first().expect("空数组"),
                _ => panic!("IPC JSON 路径 {walked:?} 走不通"),
            };
        }
        cur_keys(cur)
    };
    // 逐层双向：正向缺键 = 该层 rename_all 掉了（或字段没了）；反向漏出
    // 下划线 = 同一病根的另一种表现。None 值也要有键（前端读存在性）。
    let check = |layer: &str, path: &[&str], want: &[&str]| {
        let got = keys_at(path);
        for k in want {
            assert!(
                got.iter().any(|x| x == k),
                "{layer} 缺 camelCase 键 {k}；实际 = {got:?}"
            );
        }
        let snake: Vec<&String> = got.iter().filter(|k| k.contains('_')).collect();
        assert!(snake.is_empty(), "{layer} 漏出 snake_case 键 {snake:?}");
    };
    check(
        "LlmSettingsView 顶层",
        &[],
        &[
            "active",
            "providers",
            "provider",
            "model",
            "baseUrl",
            "apiKeySet",
            "windowTokens",
            "networkProxy",
            "subagent",
        ],
    );
    check(
        "ProviderView",
        &["providers", "0"],
        &[
            "id",
            "label",
            "kind",
            "baseUrl",
            "apiKeySet",
            "model",
            "windowTokens",
            "enabled",
            "headers",
            "models",
        ],
    );
    check(
        "ProviderModelView",
        &["providers", "0", "models", "0"],
        &[
            "id",
            "alias",
            "contextWindow",
            "maxOutput",
            "maxOutputSource",
            "supportsImage",
            "availableForSubagents",
            "inputModalities",
            "reasoningLadder",
            "toolCall",
            "pricing",
            "sources",
        ],
    );
    check(
        "SourcesView",
        &["providers", "0", "models", "0", "sources"],
        &["window", "maxOutput", "modalities", "reasoning"],
    );
    check("NetworkProxy", &["networkProxy"], &["mode", "url"]);
    check(
        "SubagentPin",
        &["subagent"],
        &["providerId", "model", "thinkingLevel"],
    );
    std::fs::remove_dir_all(&root).ok();
}
