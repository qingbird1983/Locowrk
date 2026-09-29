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
构建产物（已 gitignore）。当前官方内置五件（plugins.md）：

| 插件 | 版本 | 工具 | 副作用 |
|---|---|---|---|
| `tool-echo` | 1.2.0 / 1.3.0 | echo | Pure（多版本并存/回退演示） |
| `tool-fs` | 1.1.0 | fs.read / fs.list / fs.grep / fs.write / fs.delete / fs.restore | 读=Pure；写=Compensable（备份）；delete/restore=Irreversible |
| `tool-fetch` | 1.0.0 | fetch | Pure（GET；SSRF 防线） |
| `tool-shell` | 1.0.0 | shell | Irreversible（白名单 + denylist） |
| `tool-edit` | 1.0.0 | edit.replace / edit.lines / edit.diff | Compensable（undo 走 fs.restore） |

```bash
bash scripts/build_plugins.sh    # 按上表各版本构建并暂存
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

安装态是 `~/.Locowrk/`（经 PluginService 从市场安装，阶段七落地）；
本目录只服务内核开发与测试，不经市场与 lockfile。装进 lockfile 的插件
由 PluginService 投影展示元数据（读安装目录的 manifest.json），开发态
插件同样走该投影——registry 记录了装载目录。
