# 08-对话流：产出物与文件页签

> 状态：**开工中**（2026-10-18 开页，三条待裁已按推荐案裁定）。上游 = [`specs/06-chat-flow-rendering/spec.md`](../06-chat-flow-rendering/spec.md)
> 的「接线欠账 WF-01…WF-05」+ 你 2026-10-18 追加的三条：「点击图片，右边打开图片」「点击 html，浏览器打开」「点子代理，右栏展开」。
> 对应阶段清单那一行 = `docs/roadmap/README.md` 的「对话流：产出物与文件页签」。

## 目标

对话流里出现的**文件、代码、差异、图片、网页**都能一键进右栏就地看，不再只是死文本。
这是对话流「从能读变成能用」的那一半——06 把行本身做齐了，08 把行上的东西接出去。

## 非目标

写在这里免得越做越大：

- 不做文件树 / 不做全局搜索 / 不做页内查找
- **不做编辑**（右栏只读，改文件仍然走工具）
- 不改后端工具载荷（`edit_replace` 的行号那条欠账仍归后端另议）
- 不动 `MessageRow` 正文排版（06 的边界继续有效）
- 不给图片签加标注 / 裁剪 / 批注（只读 + 另存为之外不处理）
- 不做「预览 ↔ 源码」互相切换的按钮族（html 先只给预览；要看源码你说了再加）

## 现状取证（都是实测，不是推测）

| 入口 | 今天的行为 | 缺什么 |
| --- | --- | --- |
| 工具行文件名（`ToolRow.vue` 的 `#desc` 槽 + `FileIcon`） | 纯文本，点了没反应 | 无点击面；且外层是整行 `<button class="prow">`，**嵌套不了可点元素** |
| 展开体代码块（`ToolDetailView.vue` 的 `code` 块） | 有语言标记与复制钮 | 文件名 / 块头点不开 |
| diff 卡行号（`ToolDetailView.vue` 的 `.tdetail__diff`） | 无点击 | 不能跳到右栏对应行 |
| 缩略图（`ToolRow.vue:70` 与 `MessageRow.vue:73` 的 `viewer.openAsset(a)`） | 开全屏浮层 `ImageViewer.vue` | 右栏没有图片面 |
| 子代理步骤行（`ToolRow.vue` 的整行） | 只有**展开体里**的「查看完整对话」按钮开得了右栏（`ToolDetailView.vue:134` → emits `openTranscript`）；整行点击无反应（`fold: never` 的行连折叠都不响应） | 整行没有点击面 |
| 右栏页签族（`desktop/src/stores/work-panel.ts:20`） | 只有 `"subagent" \| "review" \| "browser"` | 没有 file / image 两类 |
| 后端读文本的入口（`desktop/src-tauri/src/lib.rs` 的 `generate_handler!`） | 只有 `asset_read`（按资产 id 读） | **没有「按路径读文本」的命令** |
| 网页预览 | **基建已在**：`desktop/src/lib/preview-url.ts` 的 `previewUrlFor(absPath)` → `http://lwpreview.localhost/<逐段编码>`，Rust 侧 `browser::preview` 解这个协议（相对引用落回同目录） | 工具行里的 `.html` 没有点进去的入口 |

## 形态申请（AGENTS.md §4 要的四要素）

1. **是什么**：右栏面板新增两种页签——「文件」（Markdown 渲染正文 / 代码高亮带行号）与「图片」；
   `.html` 不再走文件签，而是复用**既有**的浏览器签挂 `lwpreview`。
2. **在哪**：右栏 `WorkPanel.vue` 的页签条与内容区，与既有三类（`subagent` / `review` / `browser`）同族。
3. **为什么既有形态不适合**：`review` 签的契约是「一次 diff 快照」（`work-panel.ts:23` 的 `path` + `diff` 两个字段必填），
   `browser` 签的契约是「一个 URL / 一个 iframe」。文件正文与图片既不是差异也不是网页的源码，
   塞进任一种都会让那一类的契约失真（review 签会被逼出「diff 为空」的假态）。
4. **影响范围**：见「影响范围」那张文件表。含一处**既有形态改动**：`ProcessRow.vue` 的整行由 `<button>` 改成
   `role="button"` 的 `div`（原因见「前端设计」），这是为了能嵌可点文件名；改完 `aria` 与键盘行为要自己对一遍。

## 后端：加一个只读命令

```
file_read_text(path: String, max_bytes: Option<u64>) -> Result<FileTextDto, String>
FileTextDto { path: String, bytes: u64, text: String, truncated: bool, binary: bool }
```

- 上限缺省 **256 KiB**（设计稿 §7.2 的既有口径），截断只报事实不回全部内容。
- 含 NUL 字节 ⇒ 判二进制，回 `binary: true` 且 `text` 为空（界面显示「不是文本，不能就地看」+ 打开所在文件夹）。
- 读不到 / 不是文件 ⇒ `Err(人话)`。
- **不做路径白名单**（明知的取舍，写在这里备翻案）：这个应用本来就按工具调用读写任意路径（`fs_read` 工具就在同一台机器上读同一个文件），
  命令只是把已经发生的读取搬到界面上；加白名单会造出「工具读得到、界面看不了」的新裂缝。
  真正的边界是**只读 + 大小上限 + 二进制拒绝**，不是路径。

## 前端设计

**页签数据**（`stores/work-panel.ts`）：

- 加 `kind: "file"`：字段 `filePath: string`、`line?: number`、`endLine?: number`（`title` = 文件名，与 `review` 签同口径取末段）。
- 加 `kind: "image"`：字段 `path: string`、`assetId?: string`、`assetMime?: string`（`title` = 文件名）。
- 复用规则：**同 `filePath` 只开一签**，再次点 = 激活 + 带上新区间滚到那一行；同图同理（`assetId` 为准）。

**扩展名 → 页签类型的唯一判定面**（`desktop/src/lib/artifact-routing.ts`，新建）：

| 判定 | 去哪 | 依据 |
| --- | --- | --- |
| `.md` / `.markdown` / `.mdx` | 文件签（Markdown 渲染正文） | 设计稿 §7.2 |
| `.html` / `.htm` | **浏览器签**（`previewUrlFor` 挂 `lwpreview`） | 你 2026-10-18：「点击 html 浏览器打开」 |
| 图片扩展名（`.png .jpg .jpeg .gif .webp .svg .bmp .avif`） | 图片签（有资产 id 时走 `asset_read`，否则走 `lwpreview`） | 你同日：「点击图片右边打开图片」 |
| 其它已知文本 / 无扩展名 | 文件签（shiki 高亮 + 行号） | 默认档 |
| 二进制（`file_read_text` 回 `binary: true`） | 文件签内显示「不是文本」+ 打开所在文件夹 | 宁缺勿假 |

这张表是**一处判定**：单击入口（工具行文件名、代码块头、打开文件夹）都只调它，不各自 `if` 扩展名。

**新组件**（`desktop/src/components/shell/`）：

- `FileTab.vue`：`.md` 走现成的 `renderMarkdown`（marked + DOMPurify + shiki，`lib/highlight.ts` 已缓存）；
  其它语言走 shiki 高亮 + **行号列**（与 diff 卡同一套等宽口径）；头行 = 文件名 + 复制钮 + 截断提示。
  **不引新依赖**（marked / DOMPurify / shiki 都在）。
- `ImageTab.vue`：`asset_read` 取 data URL，自适应宽度显示；页内给「全屏查看」入口调 `viewer.openAsset`。

**`ProcessRow.vue` 的形态改动（唯一一处既有形态变更）**：
整行 `<button class="prow">` → `<div class="prow" role="button" tabindex="0">` + 手写 Enter/Space，
因为行内要放**可点的文件名**（button 里嵌 button 是非法 HTML，浏览器会把内层剥出去）。
`foldable === false` 的行不给 `tabindex`、不给 `role`，保持「不是开关」的现状。这是 ZCode 的同款做法
（`ToolSummaryRow.tsx` 的注释写明了同一理由）。

## 验收标准（可测：跑什么命令、看到什么结果）

- [ ] **AC-1** 点工具行文件名 → 右栏开「文件」签：`.md` 出渲染正文；`.ts` 出高亮 + 行号。
- [ ] **AC-2** 点展开体代码块 → 复用同签并滚到该块首行，那一行有高亮底。
- [ ] **AC-3** 点 diff 卡的行号 → 同签滚到该行并高亮（含旧行号 → 新行号的映射；映射不出就只滚到该 hunk 头）。
- [ ] **AC-4** 点图片（工具行缩略图 / 消息里的图）→ 右栏开「图片」签；签内可进全屏查看器。
- [ ] **AC-5** 同文件 / 同图再次点 = 复用并激活，`wp.tabs.length` 不变（不重复开签）。
- [ ] **AC-6** 边界：>256 KiB 显示前 256 KiB + 一行「已截断，共 N」；二进制显示「不是文本」+ 打开所在文件夹；文件不存在显示人话错误。
- [ ] **AC-7** `file_read_text` 有单元测试三条（截断 / 二进制 / 不存在）。
- [ ] **AC-8** 点 `.html` 文件 → 右栏开浏览器签挂 `lwpreview`，页面真能跑（页面里的相对引用 `./a.js` 也活着，
      实测口径 = 打开 `f:\warehouse\tity\breakout.html` 那种带外部引用的页）。
- [ ] **AC-9** 形态唯一：`scripts/check-style.mjs` 0 报；新页签的落点写进标准 §七 登记。
- [ ] **AC-10** `sh scripts/tasks.sh gates` + `check-docs` + 前端四闸（vue-tsc / stylelint / check-style / vitest）全绿。
- [ ] **AC-11** 真实窗口逐个目视，交付说明写「打开哪、点哪、该看到什么」。
- [ ] **AC-12** 点子代理步骤行的**整行** → 右栏开子代理签（`wp.tabs` 里 `kind: "subagent"` 且被激活）；
      展开体里的「查看完整对话」按钮保留（两条路都通，不互相顶掉）。

## 任务（分批，每批一个提交）

- [ ] 批 1 后端 `file_read_text` + 三条单测
- [ ] 批 2 页签骨架：两个 kind + `FileTab.vue` / `ImageTab.vue` + `WorkPanel.vue` 分派与页签图标 + `artifact-routing.ts`
- [ ] 批 3 接线 WF-01 / WF-02 / WF-05 / WF-04（子代理整行可点 → 右栏；含 `ProcessRow.vue` 去 button 化）
- [ ] 批 4 接线图片（缩略图 → 右栏签）与 `.html`（→ 浏览器签）；全屏入口保留
- [ ] 批 5 收口：标准 §七 登记新形态、i18n 中英成对、闸全绿、真实窗口目视

## 影响范围（文件表）

| 文件 | 改动 |
| --- | --- |
| `desktop/src-tauri/src/lib.rs` | `generate_handler!` 加一行 |
| `desktop/src-tauri/src/commands/` | 新命令 + 单测（并入 `assets.rs` 或新开 `files.rs`） |
| `desktop/src/lib/api.ts` | 加 `fileReadText()` 封装 |
| `desktop/src/lib/artifact-routing.ts` | 新建：扩展名 → 页签类型唯一判定 |
| `desktop/src/stores/work-panel.ts` | 两个 kind + 字段 + `openFileTab()` / `openImageTab()`（含同路径复用） |
| `desktop/src/components/shell/WorkPanel.vue` | 分派两个新 kind + 页签图标 |
| `desktop/src/components/shell/FileTab.vue` | 新建 |
| `desktop/src/components/shell/ImageTab.vue` | 新建 |
| `desktop/src/components/chat/ProcessRow.vue` | 整行去 `<button>`，支持内嵌可点元素 |
| `desktop/src/components/chat/ToolRow.vue` | 文件名可点；缩略图改开右栏签；子代理整行 → 右栏子代理签 |
| `desktop/src/components/chat/MessageRow.vue` | 消息图改开右栏签 |
| `desktop/src/components/chat/ToolDetailView.vue` | 代码块头、diff 行号可点（emits 上抛） |
| `desktop/src/components/ChatTranscript.vue` | 接新 emits → store |
| `desktop/src/locales/{zh-CN,en}/panel.ts` | 页签名 / 截断 / 非文本 / 错误 四条成对 |
| `docs/design/authority/Front-end Design Guidelines.md` | §七 豁免清单登记新形态（若要） |

## 已裁（2026-10-18 你定的，三条都按推荐案）

1. **缩略图单击** = 开右栏图片签（全屏查看器保留为图片签内的「放大」入口）。
2. **文件签** = 按路径读**磁盘当前内容**（不是工具 output 快照）。
3. **图片签** = 关签即销毁（不学浏览器签常驻保活）。
