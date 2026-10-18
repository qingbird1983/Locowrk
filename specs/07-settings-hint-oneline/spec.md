# 07-设置页小字一行化

- 状态：待确认
- 负责人：青鸟（定取舍） / Qoder（实现）
- 日期：2026-10-18
- 出处 / 上游：[`docs/design/authority/Front-end Design Guidelines.md`](../../docs/design/authority/Front-end%20Design%20Guidelines.md)
  §二②（用户 2026-10-06 裁定「一行封顶 ≤32 汉字」）。这条规矩一直在库里，缺的是判据、闸与存量清理，
  **不是新形态**。

## 目标

设置页每行标题下的小字说明，早就有「一行封顶 ≤32 汉字」的裁定，但没有闸兜着，两年没人执法：
实测存量 80 条越线（zh 38 / en 42），最长一条 zh 160 字 / en 448 字符，界面上摊成 2–3 行。
要让它**在写的时候就过不去**，而不是靠下次想起来再清一遍。

## 验收标准（必须可测：跑什么命令、看到什么结果）

- [ ] AC-1 **闸能判**：`node scripts/check-i18n.mjs` 退出码 0，输出 `zh=955 en=955`，无「超长原文」报错；
      把任一条 `*Hint` 手工改回 100 字再跑，**必须报红**（判据不是摆设）。
- [ ] AC-2 **存量清完**：`settings.*` 与 `developer.*` 两个命名域的全部 `*Hint`，中文 ≤32 字、英文 ≤80 字符。
- [ ] AC-3 **渲染兜底**：`SettingsRow.vue` 的 hint 段落挂 `line-clamp-1`，闸失效时也不会长回去。
- [ ] AC-4 `make check` 全绿。

## 非目标

- 不改文案含义，只删解释性从句（工程原理、括号补充、第二句）；**数值、范围、缺省值必须保留**
- 不做 `help` + `ⓘ` 第二落点 —— 放不下的信息第二批再安置
- 不动 `panel.browserEmptyHint` 一类空态文案（不属设置说明，归 §二⑩）

## 设计要点

判据取「中文 32 字 / 英文 80 字符」，英文按 2.5× 折算：同义英文本来就比中文长，等比换算会把英文全判红。
闸的判错域限定 `settings.*` / `developer.*`，因为只有这两处是「设置说明」，其余 `*Hint` 是别类形态。

## 计划

1. 标准补判据与闸 → 2. 清存量 → 3. 补渲染兜底 → 4. 跑闸收口

## 任务

- [ ] 标准与闸
- [ ] 存量
- [ ] 渲染兜底
- [ ] `make check` 全绿

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `docs/design/authority/Front-end Design Guidelines.md` | §二② 补判据、闸与实测读数 |
| `scripts/check-i18n.mjs` | 新增 hint 长度与换行判据 |
| `desktop/src/components/settings/SettingsRow.vue` | hint 段落挂 `line-clamp-1` |
| `desktop/src/locales/{zh-CN,en}/{settings,developer}.ts` | 80 条压到一行 |

## 风险与回滚

- 风险：删过头把必要信息删掉 —— 判据是「数值与范围必须保留」，删的是解释不是参数
- 挂账 / 遗留：放不下的完整说明暂时没有落点（等 `help` + `ⓘ` 第二批）
- 回滚：单文件可回退，不动数据
