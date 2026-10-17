# 工具规格：edit_replace（按字符串替换）

> **落地状态（2026-10-06，v2.114）**：`edit_replace` 并非新工具——`plugins-src/tool-edit` 早有精确匹配版（0.1.2，v2.66 起围栏跟会话）。本批按本文档升级为 **0.2.0**（大改版经用户确认）：容错匹配（尾随空白/CRLF/BOM）+ 结构化报错（多重命中列行号）+ `match_type`/`byte_range` 如实回报 + 原子写 + **exposure 翻 always**（编辑首选；配套 `fs_read` 0.1.3 同升 always，保「重读 → 逐字复制」闭环）。初稿三处修订：§3.2 行首缩进放宽**不做**（缩进敏感语言误改风险）、§四 类型 3 **删除**（与模糊匹配定义自相矛盾——命中即替换，"近似但有差异"在禁止编辑距离前提下就是找不到）、§8.2 备份**维持数据根**既有机制（`fsutil::backup_original` + `fs_restore` 撤销链），不进用户项目目录。

## 一、为什么需要这个工具

现有 `edit_diff` 要求 AI 提供完整的 unified diff（含 `@@` header、精确行号、逐字符一致的上下文行）。
**AI 做不到。** 它脑子里有大概意图，但不是逐字节准确。

结果：AI 每次编辑要"先读再写再核对"，还是常常失败。

`edit_replace` 把编辑简化为一件事：**给我一段旧文本，我给你换成新文本。**

AI 不需要知道：
- 行号
- diff 格式
- 精确的空白字符

它只需要知道：**"把这段换成那段"。**

---

## 二、工具定义

```json
{
  "name": "edit_replace",
  "description": "把文件中的一段文本替换成另一段文本。优先使用这个工具编辑文件，只在需要结构性大改时才用 edit_diff。",
  "schema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "要编辑的文件路径"
      },
      "old_string": {
        "type": "string",
        "description": "要被替换的原文。必须从文件中复制，逐字符准确（缩进、换行都要对）"
      },
      "new_string": {
        "type": "string",
        "description": "替换后的新文本"
      },
      "replace_all": {
        "type": "boolean",
        "default": false,
        "description": "如果 old_string 在文件中出现多次，是否全部替换。默认 false（只在唯一匹配时替换）"
      }
    },
    "required": ["path", "old_string", "new_string"]
  },
  "side_effect": "Compensable",
  "exposure": "always"
}
```

---

## 三、匹配规则

### 3.1 精确匹配优先

先用 `old_string` **逐字符**在文件里查找。

找到了：
- 出现 1 次 → 替换
- 出现多次 → 看 `replace_all`
  - `true` → 全部替换
  - `false` → **报错**（要求 AI 提供更长的、唯一的 `old_string`）

找不到：进入模糊匹配。

### 3.2 模糊匹配（fuzz）

精确匹配失败时，按以下规则再试一遍：

| 规则 | 说明 |
|---|---|
| **忽略行尾空白** | `"foo  "` 匹配 `"foo"`；命中的尾随空格属于替换区间一并换掉 |
| **忽略行尾换行符差异** | `\n` 和 `\r\n` 等价（模型打不出 `\r`——这是「照抄也失败」的主因） |
| **忽略 BOM** | 文件开头或 old_string 开头的 BOM 不参与匹配 |

**不做的事**：
- 不忽略行首空白差异（缩进敏感语言——Python/YAML——误改风险；二期再议）
- 不忽略单词内部的空格
- 不忽略大小写
- 不做编辑距离匹配（太危险）

**实现**（0.2.0 实装）：原文与 old_string 各自归一化成字节流（只删 ASCII 字节，多字节字符不动），归一化流上普通子串搜索，命中区间经 per-line kept 映射表映射回原缓冲字节区间后整体替换；`new_string` 的行尾随命中区域行尾风格适配（CRLF 文件插 LF 会造成混合行尾）。

### 3.3 唯一性检查

**精确匹配和模糊匹配都必须保证唯一性。**

如果模糊匹配后出现多个候选：
- `replace_all = true` → 全部替换
- `replace_all = false` → **报错**，并在错误信息里列出所有候选位置

**唯一性是安全底线。** 不唯一就不能改——否则可能改错地方。

---

## 四、错误处理

### 4.1 错误信息要告诉 AI 下一步做什么

**不要写黑话。** 错误信息里要包含：
1. 发生了什么
2. 可能的原因
3. 建议的下一步动作

### 4.2 错误类型

**类型 1：找不到 old_string**

```
错误：在文件 <path> 中找不到你要替换的文本。

可能原因：
- old_string 和文件中的实际内容有差异（空格、缩进、换行）
- 文件可能已经被修改

建议：
1. 用 fs_read 重新读取这个文件
2. 从读取结果里逐字符复制要替换的部分
3. 再次调用 edit_replace
```

**类型 2：找到多处但 replace_all = false**

```
错误：old_string 在文件中出现 N 次，但 replace_all = false。

出现位置（行号）：
  第 42 行: fn foo() {
  第 87 行: fn foo() {
  第 134 行: fn foo() {

建议（选一个）：
- 把 old_string 扩展得更长，让它唯一（比如包含上一行或下一行）
- 设置 replace_all = true 替换所有
```

**类型 3：容错也未命中**（初稿的「模糊命中但内容有差异 → 拒绝」已删除——模糊匹配按定义就是「有差异仍命中」，命中不替换则永远不生效；"近似但内容不同" 在禁止编辑距离的前提下就是找不到，归入本类）

```
错误：在文件 <path> 中找不到你要替换的文本
（精确匹配与忽略行尾空白/换行符差异的容错搜索都失败了）。

可能原因：
- old_string 和文件实际内容差异超出容错范围（缩进、用词）
- 文件可能已经被修改

建议：
1. 用 fs_read 重新读取这个文件
2. 从读取结果里逐字符复制要替换的部分
3. 再次调用 edit_replace
```

（实际错误文案为英文，随工具描述同语言；上图为语义示意。）

---

## 五、成功返回

```json
{
  "path": "src/main.rs",
  "bytes": 1088,
  "edits": 1,
  "match_type": "exact",
  "byte_range": [1024, 1088],
  "existed": true,
  "backup": "<数据根>/backups/tool-edit/<nanos>-<pid>/<文件名>"
}
```

**返回 `match_type`**：`exact` / `fuzzy`——模糊命中是「你给的原文与实际有差异」的信号，AI 应该意识到下次更仔细。

**返回 `backup`**：Compensable 的撤销锚点——备份落在**数据根**（`fsutil::backup_original`），配合 `fs_restore` 撤销（0.1.2 起既有机制，本批未改动）。写入为原子写（临时文件 + 同目录 rename）。

---

## 六、AI 使用指南

**Locowrk 没有可编辑的主模型系统提示词**（system 只由中间件块组成），优先级指引的载体是**工具 description**——本批已落：

- `edit_replace` description：PREFERRED editing tool for most edits…fall back to edit_diff only for structural changes…（含容错语义与 match_type 说明）
- `edit_diff` description：Prefer edit_replace for most edits; use this only for structural changes across many places…
- `fs_read` description：Read before edit_replace so old_string can be copied character-for-character.

要点（对模型可见的引导全在 description 里）：

- old_string 要足够长，保证唯一（一般包含 3-5 行上下文）
- old_string 要从 fs_read 结果里复制，不要凭记忆写
- 如果替换失败，先 fs_read 核对，不要连续重试同样的参数

---

## 七、和现有工具的关系

| 工具 | 适用场景 | 优先级 |
|---|---|---|
| **edit_replace** | 90% 的编辑场景 | **首选** |
| edit_diff | 结构性大改 | 次选 |
| edit_lines | 明确知道行号时 | 少见 |

**工具目录按名字序钉死（M1 前缀纪律），「排最前」做不到**——排序话语权在 exposure（edit_replace = always 直通暴露）与 description（互指优先级）手里，本批以这两者落地。

---

## 八、实现要点

### 8.1 搜索策略

```rust
fn find_occurrences(content: &str, old: &str) -> Vec<usize> {
    // 1. 精确匹配
    let exact = content.match_indices(old).map(|(i, _)| i).collect::<Vec<_>>();
    if !exact.is_empty() { return exact; }

    // 2. 归一化匹配（去掉行尾空白后再找）
    let normalized_content = normalize_whitespace(content);
    let normalized_old = normalize_whitespace(old);
    // 找到位置后，映射回原始位置
    // ...
}
```

### 8.2 备份

**0.1.2 起既有机制，本批未改动**：每次替换前 `fsutil::backup_original` 把原文件复制到**数据根** `backups/tool-edit/<nanos>-<pid>/`，结果字段 `backup` 即撤销锚点，`fs_restore`（tool-fs）按它恢复。
- 不落用户项目目录（`.locowrk/backups` 初稿方案废弃：大小写与仓库 `.Locowrk/` 惯例不符，且污染用户 git 状态）
- 备份清理（保留 N 份/N 天）暂未做——数据根自持目录，量小；需要时随批次补

### 8.3 原子性

- **本批已落**：临时文件（唯一名，同目录）+ rename（Windows rename 覆盖既有目标），写一半崩溃不留损坏文件
- helper 暂为 tool-edit 私有（错误文案 edit 语境）；第二个消费方出现时上收 `fsutil`

---

## 九、验收标准

- [x] 精确匹配时能正确替换
- [x] 精确匹配出现多次且 replace_all = false 时报错，并列出所有位置（行号）
- [x] 精确匹配失败时能尝试模糊匹配（尾随空白 / CRLF / BOM；缩进不放宽）
- [x] 模糊匹配命中多个候选时报错（列行号）
- [x] 错误信息包含"建议的下一步动作"
- [x] 每次替换前自动备份（既有机制）
- [x] 成功返回 match_type 和 backup（另加 byte_range）
- [x] 支持 CRLF/LF 混合文件（new_string 行尾随命中区域适配）
- [x] 支持 UTF-8 中文内容（归一化只删 ASCII 字节，多字节字符不动）
- [x] exposure always（edit_replace + fs_read 配套）

测试：`cargo test -p tool-edit`（replace_matrix / replace_fuzzy_matrix / lines_matrix + diff 三例）。

---

## 十、完成后告诉我

1. **匹配算法**：归一化字节流（只删 ASCII：BOM/\r/行尾空白）+ 普通子串搜索 + per-line kept 映射回原缓冲区间整体替换；精确优先，零命中才进容错；唯一性两轮都强制。
2. **容错边界**：行尾空白、CRLF/LF、BOM 三条；**不做**行首缩进、大小写、编辑距离；`new_string` 行尾随命中区域行尾风格适配。
3. **错误文案**（英文，三型）：未命中（精确+容错都失败，教 fs_read 重读逐字复制）/ 多重命中（列候选行号，exact 与 fuzzy 同构，fuzzy 标注）/ 容错归一化后为空白（nothing to match）。
4. **备份策略**：既有 `fsutil::backup_original` → 数据根 `backups/tool-edit/<nanos>-<pid>/`，`fs_restore` 撤销；无保留期清理（需要时随批补）。
5. **打地鼠实测**：待实机（重启载入 0.2.0 产物后对 `F:\Warehouse\Timo\whack-a-mole.html` 真实轮验证）。
```

---

**核心思想**：

**不要让 AI 学"怎么写精确的 patch"，要让工具学会"容忍 AI 的不精确"。**

**AI 的强项是"理解意图"，弱项是"记住精确细节"。工具应该配合它的强项，而不是考验它的弱项。**