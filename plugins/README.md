# plugins/ — 开发态插件安装目录

多版本并存是回退的基础（DESIGN.md §5.7、§11.3）。布局（packaging.md §3.1，
每个版本目录自包含）：

```
plugins/
  {plugin-id}/
    {semver}/
      {plugin_id}.dll      # Windows 下是 .dll；设计文档示例中的 .so 为 Linux/macOS 命名
      manifest.json        # packaging.md v2 全量 manifest（展示元数据真源）
      assets/
        icon.svg           # manifest.icon 相对路径指向的图标
```

版本回退 = 把 `active` 指针切回旧版本目录，不需要重新下载。
旧版本永远保留，直到用户主动清理（DESIGN.md §11.5）。

## 插件源码

插件源码在仓库的 `plugins-src/`（workspace 成员），`plugins/` 只放
构建产物（已 gitignore）。

> **⚠ v2.149 批 E 订正（本表原写「官方内置八件」，且版本/工具名/副作用四轴
> 全与代码脱节）**：件数 = **9 个 id**（`plugins-src/` 实有 10 个目录——
> `tool-shout` 是**市场验收夹具**，只进 `market/` 不进本目录）；`tool-sandbox`
> 已随 v2.145 权限模型重设计整目录退役；工具名的点号分隔（`fs.read`）早在
> v2.25 就因 OpenAI 兼容接口拒收点号改为下划线；版本基准 2026-10-04 用户
> 裁定全表重置 0.1.0 起步（旧 1.x 系历史档位）；`fetch` 曾标 Pure，实为
> Irreversible。**版本真源 = `scripts/build_plugins.sh` 里的 `stage` 调用
> 清单**（本表由它抄录，改版本只改那一份清单，不在这里二次维护）。

| 插件 | 最新暂存版本 | 工具 | 副作用 |
|---|---|---|---|
| `tool-echo` | 0.1.0 / 0.2.0（双档） | `echo` | Pure（多版本并存/回退演示专用） |
| `tool-fs` | 0.1.5 | `fs_read` / `fs_list` / `fs_grep` / `fs_write` / `fs_delete` / `fs_restore` | 读=Pure；`fs_write`=Compensable（`undo_tool=fs_restore`）；delete/restore=Irreversible |
| `tool-fetch` | 0.1.2 | `fetch` | **Irreversible**（GET 也对外发起请求：服务端留痕、URL 可能带 token，每次调用弹审批卡） |
| `tool-shell` | 0.1.6 | `shell` / `shell_read` | `shell`=Irreversible（v2.145 起**无命令级禁令**：结构性拒绝之外一切放行进审批卡）；`shell_read`=Pure（`proof.rs` 只读证明 + EXACT_ARGV 免审面） |
| `tool-edit` | 0.2.1 | `edit_replace` / `edit_lines` / `edit_diff` | Compensable（`undo_tool=fs_restore`） |
| `tool-git` | 0.1.3 | `git_status` / `git_diff` / `git_log` / `git_commit` / `git_push` / `git_undo_commit` | 读=Pure；`git_commit`=Compensable（`undo_tool=git_undo_commit`）；push/undo=Irreversible |
| `memory-sqlite` | 0.1.5 | `memory_write` / `memory_read` / `memory_list` / `memory_search` / `memory_delete` / `memory_undo` | 读=Pure；write/delete=Compensable（`undo_tool=memory_undo`）；`memory_undo`=Irreversible |
| `tool-mcp` | 0.1.0 | `mcp_status` / `mcp_reload` + `mcp_<server>_<tool>`（装载时从 `<数据根>/mcp.json` 发现并冻结） | 管理工具=Pure；远端工具按配置分级，**缺省 Irreversible** |
| `tool-notify` | 0.1.0 | `notify` | Irreversible（webhook 外发 / 桌面系统通知都触及会话之外） |

同一 id 的多个版本目录**都在**（回退演示与历史留档），registry 按
`resolve_latest` 取最新档；表里只列最新暂存版本。

**本表不含宿主内置工具**：`search_tools` / `memory_expand` /
`memory_collapse` / `memory_tree_*` / `memory_project_history` /
`subagent_run` / `subagent_verify` / `browser_open` / `browser_console` /
`pick_directory` 一类由 `desktop/src-tauri` 组装点直接注册（`with_builtin`），
不经 `plugins/` 目录与动态库装载，因此也不在这里的 manifest 面上。

```bash
bash scripts/build_plugins.sh    # 按脚本内 stage 清单逐个构建并暂存
cargo run -p kernel-bin -- demo  # 阶段三验收：升级 → 回退，全程不重启
```

manifest 版本号来自编译期环境变量 `LOCOWRK_PLUGIN_VERSION`（见
`plugins-src/tool-echo/build.rs`），目录名与产物 manifest 必须一致，
registry 加载时会校验（不一致拒载）。暂存脚本把 `plugins-src/{id}/`
的 manifest.json（sed 替换版本号）与 `assets/` 一并拷进版本目录。

## 动态库发现规则

约定名 `{id_underscored}.dll` 优先；缺席时扫描版本目录里**唯一的**
平台动态库（packaging.md §3.1："dll 名字由作者决定，宿主不假设文件名"）。
目录里出现多个动态库视为歧义，拒绝装载（宁可漏载也不猜）。

## 与安装态的区别

安装态是数据根（缺省 `~/.Locowrk/`，`LOCOWRK_DATA_DIR` 覆盖——
`desktop/src-tauri/src/data_root.rs` 是唯一解析入口）下的
`plugins/` + `lockfile.json`，经 PluginService 从市场安装（阶段七落地）；
本目录只服务内核开发与测试，不经市场与 lockfile。装进 lockfile 的插件
由 PluginService 投影展示元数据（读安装目录的 manifest.json），开发态
插件同样走该投影——registry 记录了装载目录。
