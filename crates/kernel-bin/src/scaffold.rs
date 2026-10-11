//! `locowrk new plugin <name>`——插件脚手架（packaging.md §8）。
//!
//! 在指定目录生成最小可编译的 C ABI 工具插件：
//! `<name>/{Cargo.toml, src/lib.rs, manifest.json, assets/icon.svg}`。
//! manifest 按 packaging.md v2 全量字段（多语言展示名/描述/图标/作者），
//! 工具级 `display_name` 与 manifest 级 `reversible` 一并写出——当前宿主
//! 解析时忽略未知字段（前向兼容），未来版本演进时模板无需改动。
//! `origin` 不写：缺省一律 community（§2.4 安全默认——官方标识必须
//! 显式声明，防止第三方包伪造"官方内置"徽章）。

use std::path::{Path, PathBuf};

/// 生成插件脚手架，返回插件目录。`parent` 为生成位置（一般为当前目录）。
pub fn new_plugin(name: &str, parent: &Path) -> Result<PathBuf, String> {
    let id = sanitize_id(name)
        .ok_or_else(|| format!("invalid plugin name `{name}`: 需要 ASCII 小写字母/数字/连字符"))?;
    let dir = parent.join(&id);
    if dir.exists() {
        return Err(format!("{} 已存在，拒绝覆盖", dir.display()));
    }
    std::fs::create_dir_all(dir.join("src"))
        .map_err(|e| format!("create_dir_all {}/src: {e}", dir.display()))?;
    std::fs::create_dir_all(dir.join("assets"))
        .map_err(|e| format!("create_dir_all {}/assets: {e}", dir.display()))?;

    let api_path = plugin_api_dep(&dir);
    let tool = id.replace('-', "_");
    let filled = |tpl: &str| {
        tpl.replace("__PLUGIN_ID__", &id)
            .replace("__TOOL_NAME__", &tool)
            .replace("__DLL_STEM__", &tool)
            .replace("__STRUCT_NAME__", &struct_name(&id))
            .replace("__API_PATH__", &api_path)
    };
    std::fs::write(dir.join("Cargo.toml"), filled(CARGO_TOML))
        .map_err(|e| format!("write Cargo.toml: {e}"))?;
    std::fs::write(dir.join("manifest.json"), filled(MANIFEST_JSON))
        .map_err(|e| format!("write manifest.json: {e}"))?;
    std::fs::write(dir.join("src").join("lib.rs"), filled(LIB_RS))
        .map_err(|e| format!("write src/lib.rs: {e}"))?;
    std::fs::write(dir.join("assets").join("icon.svg"), ICON_SVG)
        .map_err(|e| format!("write assets/icon.svg: {e}"))?;
    Ok(dir)
}

/// `locowrk new plugin <name>` 入口：在当前目录生成脚手架。
pub fn run_new(args: &[String]) -> std::process::ExitCode {
    match (args.first().map(String::as_str), args.get(1)) {
        (Some("plugin"), Some(name)) => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match new_plugin(name, &cwd) {
                Ok(dir) => {
                    println!("插件脚手架已生成：{}", dir.display());
                    println!("  {}/Cargo.toml", dir.display());
                    println!(
                        "  {}/manifest.json   # packaging.md v2 展示字段",
                        dir.display()
                    );
                    println!("  {}/src/lib.rs", dir.display());
                    println!("  {}/assets/icon.svg", dir.display());
                    println!("下一步：cargo build 后把动态库交给宿主装载（见 plugins/README.md）");
                    std::process::ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("locowrk new plugin failed: {e}");
                    std::process::ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage: locowrk new plugin <name>");
            std::process::ExitCode::FAILURE
        }
    }
}

/// 清洗插件名（§2.2：ASCII 小写/数字/连字符）。空格/下划线归一为连字符，
/// 其余字符丢弃；清洗后为空则拒绝。
fn sanitize_id(name: &str) -> Option<String> {
    let mut out = String::new();
    let mut prev_dash = true; // 顺带去掉开头连字符
    for ch in name.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            out.push(ch);
            prev_dash = false;
        } else if (ch == '-' || ch == '_' || ch == ' ') && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Rust 结构体名：`my-plugin` → `MyPlugin`。
fn struct_name(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|seg| !seg.is_empty())
        .map(|seg| {
            let mut chars = seg.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// plugin-api 依赖路径：从生成位置向上找 Locowrk 仓库根（含
/// crates/plugin-api），命中则写相对路径依赖；仓库外写占位符，
/// 由使用者手工替换。
fn plugin_api_dep(dir: &Path) -> String {
    let mut cur = dir.to_path_buf();
    loop {
        if cur
            .join("crates")
            .join("plugin-api")
            .join("Cargo.toml")
            .is_file()
        {
            let depth = dir
                .strip_prefix(&cur)
                .map(|rel| rel.components().count())
                .unwrap_or(0);
            let mut path = std::path::PathBuf::new();
            for _ in 0..depth {
                path.push("..");
            }
            path.push("crates");
            path.push("plugin-api");
            return path.to_string_lossy().replace('\\', "/");
        }
        match cur.parent() {
            Some(parent) => cur = parent.to_path_buf(),
            None => return "REPLACE_WITH_LOCOWRK_PATH/crates/plugin-api".into(),
        }
    }
}

const CARGO_TOML: &str = r#"[package]
name = "__PLUGIN_ID__"
version = "0.1.0"
edition = "2024"
publish = false

# 插件产物是动态库（Windows: __DLL_STEM__.dll，见 plugins/README.md）。
[lib]
crate-type = ["cdylib"]

# 插件只依赖冻结契约层：serde_json / semver 经 plugin-api re-export。
[dependencies]
plugin-api = { path = "__API_PATH__" }

# 独立 crate：不挂在 Locowrk workspace 下也能单独 cargo build。
[workspace]
"#;

/// manifest 模板 = packaging.md §8.1 示例（id 按用户输入替换）。
const MANIFEST_JSON: &str = r#"{
  "id": "__PLUGIN_ID__",
  "version": "0.1.0",
  "abi_version": 1,
  "kind": "tool",

  "display_name": {
    "zh-CN": "我的插件",
    "en-US": "My Plugin"
  },
  "description": {
    "zh-CN": "描述这个插件做什么",
    "en-US": "Describe what this plugin does"
  },
  "icon": "assets/icon.svg",

  "author": {
    "name": "Your Name",
    "url": "https://github.com/your-name"
  },
  "license": "MIT",

  "tools": [
    {
      "name": "__TOOL_NAME__",
      "display_name": {
        "zh-CN": "我的工具",
        "en-US": "My Tool"
      },
      "description": {
        "zh-CN": "工具描述",
        "en-US": "Tool description"
      },
      "schema": {
        "type": "object",
        "properties": {
          "input": { "type": "string", "description": "输入" }
        },
        "required": ["input"]
      },
      "side_effect": "Pure"
    }
  ],

  "reversible": { "kind": "Pure" }
}
"#;

const LIB_RS: &str = r#"//! __PLUGIN_ID__：`locowrk new plugin` 生成的最小工具插件骨架（packaging.md §8）。
//!
//! 一个工具 `__TOOL_NAME__`（SideEffect::Pure）：`{"input": "..."}` → 原样回显。
//! 开发流程：改 manifest.json 的展示字段与工具 schema，改本文件的 call 逻辑，
//! `cargo build` 后把动态库交给宿主装载（或经市场目录安装，见 plugins/README.md）。

use std::ffi::CString;

use plugin_api::manifest::{Author, LocalizedOrString, PluginKind, PluginManifest, PluginOrigin};
use plugin_api::meta::zh_en;
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

const PLUGIN_ID: &str = "__PLUGIN_ID__";
const PLUGIN_VERSION: &str = "0.1.0";
const TOOL_NAME: &str = "__TOOL_NAME__";

/// describe 符号的数据源：manifest JSON（NUL 结尾，约定见 plugin-api::ffi）。
fn describe_manifest() -> CString {
    let manifest = PluginManifest {
        id: PLUGIN_ID.into(),
        version: PLUGIN_VERSION.into(),
        abi_version: plugin_api::PLUGIN_ABI_VERSION,
        kind: PluginKind::Tool,
        tools: vec![ToolSpec {
            name: TOOL_NAME.into(),
            description: "echoes the input back unchanged".into(),
            schema: json!({
                "type": "object",
                "properties": { "input": { "type": "string" } },
                "required": ["input"]
            }),
            side_effect: SideEffect::Pure,
            exposure: Default::default(),
        }],
        // v2 展示元数据（packaging.md §2.2）。origin 缺省 community（§2.4
        // 安全默认）；确属官方插件时改为 PluginOrigin::Official。
        // 这里的七个字段刻意**手写展开**而不调 `meta::official_meta`：那个基座
        // 是给仓库自带的官方插件收样板的（会硬写 `Official` 徽章与仓库作者），
        // 第三方插件用它就等于伪造官方身份。
        display_name: Some(zh_en("我的插件", "My Plugin")),
        description: Some(LocalizedOrString::Localized(zh_en(
            "描述这个插件做什么",
            "Describe what this plugin does",
        ))),
        icon: Some("assets/icon.svg".into()),
        author: Some(Author {
            name: "Your Name".into(),
            url: None,
        }),
        origin: PluginOrigin::Community,
        homepage: None,
        license: Some("MIT".into()),
    };
    manifest.to_describe_json()
}

struct __STRUCT_NAME__State;

impl PluginState for __STRUCT_NAME__State {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        if tool != TOOL_NAME {
            return Err(format!("unknown tool `{tool}`"));
        }
        let input = args
            .get("input")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing string field `input`".to_string())?;
        Ok(json!({ "output": input, "plugin_version": PLUGIN_VERSION }))
    }
}

fn make_state(_host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(__STRUCT_NAME__State)
}

plugin_api::declare_plugin!(describe_manifest, make_state);
"#;

/// 默认占位图标 = packaging.md §8.2。
const ICON_SVG: &str = r#"<svg viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg" fill="none">
  <rect x="8" y="8" width="48" height="48" rx="12"
        stroke="currentColor" stroke-width="3"/>
  <circle cx="32" cy="32" r="6" fill="currentColor"/>
  <path d="M32 14 V26 M32 38 V50 M14 32 H26 M38 32 H50"
        stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
</svg>
"#;

#[cfg(test)]
mod tests;
