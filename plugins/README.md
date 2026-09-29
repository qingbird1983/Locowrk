# plugins/ — 插件安装目录

多版本并存是回退的基础（DESIGN.md §5.7）。布局：

```
plugins/
  {plugin-id}/
    {semver}/
      lib{plugin_id}.dll        # Windows 下是 .dll；设计文档示例中的 .so 为 Linux/macOS 命名
      manifest.json             # PluginManifest（abi_version 必须等于 PLUGIN_ABI_VERSION）
```

版本回退 = 把 `active` 指针切回旧版本目录，不需要重新下载。
旧版本永远保留，直到用户主动清理（DESIGN.md §11.5）。
