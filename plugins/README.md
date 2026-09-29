# plugins/ — 开发态插件安装目录

多版本并存是回退的基础（DESIGN.md §5.7、§11.3）。布局：

```
plugins/
  {plugin-id}/
    {semver}/
      lib{plugin_id}.dll        # Windows 下是 .dll；设计文档示例中的 .so 为 Linux/macOS 命名
```

版本回退 = 把 `active` 指针切回旧版本目录，不需要重新下载。
旧版本永远保留，直到用户主动清理（DESIGN.md §11.5）。

## 插件源码

插件源码在仓库的 `plugins-src/`（workspace 成员），`plugins/` 只放
构建产物（已 gitignore）。以 tool-echo 为例：

```bash
bash scripts/build_plugins.sh    # 以 1.2.0 / 1.3.0 各构建一次并暂存
cargo run -p kernel-bin -- demo  # 阶段三验收：升级 → 回退，全程不重启
```

manifest 版本号来自编译期环境变量 `LOCOWRK_PLUGIN_VERSION`（见
`plugins-src/tool-echo/build.rs`），目录名与产物 manifest 必须一致，
registry 加载时会校验（不一致拒载）。

## 与安装态的区别

安装态是 `~/.agent-kernel/`（经 PluginService 从市场安装，阶段七落地）；
本目录只服务内核开发与测试，不经市场与 lockfile。
