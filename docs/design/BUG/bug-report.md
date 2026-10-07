# Locowrk 工具 bug 上报（plugin 0.2.0）

> **状态：Bug 1 / Bug 2 已修复 —— 转为历史存档（用户告知，未经独立复测）；Bug 3 为本轮实测新增、待上报。**
> 用户确认 Bug 1、Bug 2 均已修复，不再作为待提交的上报材料，保留作排查与回归复测的参考。
> **Bug 3（报错不指路 / 指路覆盖不一致）为新增待上报项，见文末。**
> 若要独立确认 Bug 1 / Bug 2，按文末「附」节重建最小仓库逐条跑一遍（复测方法：一次一个变体）。

Bug 1 / Bug 2 为两处实测确认、已修复的历史问题（均含最小复现）；Bug 3 为新增待上报项。环境：Windows 主机，`shell` 走 Unix coreutils（git-bash 风格）。

---

## Bug 1 — `git_push` 失败时错误信息被截断，关键原因行丢失

**状态**：✅ 已修复（用户告知，未经独立复测）

**严重度**：中（影响可诊断性；用户/模型都拿不到失败原因）

**现象**：仓库未配置 remote 时调用 `git_push`，返回的错误只剩 git stderr 的最后半句，构成不了完整句子。

**复现**：
1. 建一个无 remote 的仓库（`git remote -v` 为空）；
2. 调用 `git_push{repo, branch:"main"}`。

**实际返回**：
```json
{"error":"tool `git_push` failed: git push failed: and the repository exists."}
```
注意 `"and the repository exists."` 明显是半句话。

**期望**：完整保留 git 的原始 stderr，即：
```
fatal: 'origin' does not appear to be a git repository
fatal: Could not read from remote repository.

Please make sure you have the correct access rights
and the repository exists.
```

**推测原因**：拼接错误信息时只取了 stderr 的**最后一行**（或按行尾截断），而 git 这里的关键信息在**前面**几行。建议改为完整透传 stderr（或至少保留首行 + 末行）。

---

## Bug 2 — `git_undo_commit` 无法撤销仓库的第一个（根）提交

**状态**：✅ 已修复（用户告知，未经独立复测）

**严重度**：中（补偿链对根提交失效；且短哈希时报错具有误导性）

**现象**：`git_undo_commit` 对**非根提交**工作完全正常；对**根提交**（无父提交）必失败，且失败信息是 git 的用法提示，看不出病因。

**复现与对照实验**（同一仓库，两个提交，唯一变量=目标提交有无父）：

| 目标提交 | 有父？ | `output.commit` | 结果 |
|---|---|---|---|
| `4872516…`（第 2 个） | 有 | 全 40 位哈希 | ✅ `{"commit":"48725166…","undone":true}`，HEAD 回退一格，改动保留在 staged（`git status` 该文件 `x:"M",y:" "`，`git diff --staged` 有内容） |
| `20b1d6c…`（根提交） | 无 | 全 40 位哈希 | ❌ `{"error":"tool git_undo_commit failed: git reset failed: 'git <command> [<revision>...] -- [<file>...]'"}` |

**推测原因**：实现是「软重置到目标提交的父提交」（等价 `git reset --soft <commit>^`）。根提交没有父，`<commit>^` 解析为空，构造出的 `git reset` 命令缺了 revision 参数 → git 打印用法提示。错误信息未做「根提交不支持」的专门处理。

**附带问题（同一工具）**：传**短哈希**时，工具会先在 HEAD 比对处失败并报
```
git_undo_commit: HEAD has moved (20b1d6caf…e4e != 20b1d6c); manual intervention required
```
即 `output.commit` **必须是全 40 位哈希**。但这句 `HEAD has moved` 与实际病因（哈希长度不符，HEAD 并未移动）不符，容易误导排查方向。建议：①在文档/参数说明里显式要求全哈希；②对短哈希给出针对性报错。

**修复建议**：
- 根提交场景返回明确错误，如「无法撤销仓库的第一个提交（无父可回退）」；
- 或退化为 `git update-ref -d HEAD` / `git reset --soft` 到空树等可行实现；
- 短哈希与「HEAD 移动」两类错误分开报。

---

## 附：可复现 Bug 1/Bug 2 的最小仓库构造

本机 `git init` 不在 shell 白名单，但可手工造 `.git` 骨架复现（`fs_write` + `create_dirs`）：

```
<repo>/.git/HEAD                → "ref: refs/heads/main\n"
<repo>/.git/config              → [core] repositoryformatversion=0 / filemode=false / bare=false
                                  [user] email=… name=…     ← 缺 user 会让 git_commit 报 "unable to auto-detect email address"
<repo>/.git/objects/info/.keep  → 空（需 create_dirs）
<repo>/.git/refs/heads/.keep    → 空（需 create_dirs）
<repo>/.git/refs/tags/.keep     → 空（需 create_dirs）
```

`git_status` 即会识别为合法仓库（`branch:"main"`）。

---

## Bug 3 — 路径/元字符类报错只回系统原文，不给替代工具指路（指路覆盖不一致）

**状态**：🆕 待上报（本轮实测确认，8 条对照）

**严重度**：低~中（不影响功能正确性，但损害可诊断性与模型自愈；因 `catalog` 已兜底全量工具名，尚未致任务失败）

**现象**：工具报错时的「指路」能力覆盖**不一致**——同一工具，部分错误分支会给出替代工具建议，`os error`（ENOENT）分支却只透传系统原文。

**对照实测**：

| 调用 | 返回 | 指路 |
|---|---|---|
| `fs_read` 路径不存在 | `系统找不到指定的文件。 (os error 2)` | ❌ 不指 `fs_list` |
| `fs_grep` 路径不存在 | 同上 | ❌ 不指 `fs_list` |
| `fs_list` 目录不存在 | 同上 | ❌ 不指上级/层级 |
| `shell_read` 命令含元字符 | `contains shell metacharacters (| & ; < > \` $ or newline); …` | ❌ 不指 `fs_grep` |
| `shell_read` `cwd` 非目录 | `shell cwd … is not a directory` | ❌ |
| **（以下为反例：已指路）** | | |
| `fs_read` 传目录 | `fs_read … is a directory (use fs_list)` | ✅ 指 `fs_list` |
| `shell_read` 非白名单命令 | `dir is not a read-only command (allowed: …); use the shell tool instead` | ✅ 指 `shell` |
| `edit_replace` old_string 不匹配 | `… fs_read the file and copy old_string character-for-character` | ✅ 指 `fs_read` |

**关键点**：指路能力**已具备且有先例**——`use fs_list` / `use the shell tool` / `fs_read … copy` 都是现成文案；问题只是这些文案**未覆盖 os-error 分支**，属「**不一致覆盖**」而非「完全缺失」，修复成本极低。

**期望**：在 os-error / 元字符拒绝分支追加一句替代建议，例如：
- `fs_read` / `fs_grep` 路径不存在 → “if the path may not exist, run `fs_list` on the parent directory first”
- `fs_list` 目录不存在 → “the parent directory may not exist; list it with `fs_list`”
- `shell_read` 元字符拒绝 → “for content search use `fs_grep`; for other reads use the `shell` tool”

**推测原因**：指路文案按错误分支逐条硬编码，ENOENT / 元字符拒绝分支直接透传底层错误或固定文案，未挂接替代建议。

---

## Bug 3 附 — 同族改进项：`catalog` 零描述 + 中文 query 不命中

- `search_tools` 的 `catalog` 返回 **26 个纯工具名字符串、零描述**：只有名字，光看名字仍需再搜一次拿 schema。
- 用**中文语义 query**（「在项目文件里搜索包含某段文本的文件」）实测：`source: "hot_fallback"`（关键词回退，非语义命中），**目标工具 `fs_grep` 未进入 results**（仅名字出现在 catalog 中）→ 中文场景语义检索基本失效，全靠 catalog 兜底。

**建议**：① `catalog` 每项附一行描述；② 对中文/跨语言 query 至少做关键词归一化，或让语义检索生效。
