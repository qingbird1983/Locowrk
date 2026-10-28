# spike-mygo-ui：Phase 4 混合渲染夹具

验证“原生外框 + 内嵌 WebView”这条路，MyGo v0.7.1，独立 module，窗口隐藏。

```bash
go run . -headless   # 无窗口：100 行 + 500 事件计数，120 帧耗时
go run .             # 原生窗口 + 内嵌页面：页面→Go、Go→页面事件，打印各阶段毫秒
```

两条命令都输出一行 JSON；`ok` 为 true 表示通过。

2026-10-28（Windows 11）读数：

| 检查 | 结果 |
|---|---|
| 原生窗口 / WebView | 74.6 ms / 93.3 ms（进程启动起算） |
| 页面→Go | 679.5 ms |
| Go→页面事件→Go | 684.4 ms |
| 无窗口 120 帧 | 平均 0.18 ms，最慢 0.65 ms |

夹具不是产品代码，也不在根 module 内；构建产物不入库。
