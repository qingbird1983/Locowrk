# Locowrk 工具层问题待办清单

> 来源：整轮工具复测
> 日期：2026-10-06
> 状态：**已处理**（v2.116，2026-10-06 当批全落——P0-1/P0-2/P1-1/P2-1/P2-2 + 事实 1/事实 2 逐条处置见
> §六「处理记录」；本文原挂的「状态：待处理」与 §六 自相矛盾，v2.149 批 E 订正）。
> **唯余一项 ⏸**：§五 第 6 项「shell 跨平台策略」= 短期缓解已做（工具描述如实），长期策略维持现状待评估。
> 一~五节保留为**复测当轮的原始工单**，不再逐条更新；现役环境事实看同目录
> [环境已知限制.md](环境已知限制.md)。

---

## 一、真问题（按优先级）

### P0-1：`fetch` 的 SideEffect 标错

**现象**：`fetch` 标注为 `pure`，但实际会对外发起网络请求。

**为什么是 P0**：
- `pure` 语义 = 自动放行、无需审批、回退免费
- 但网络请求会留下服务端日志、可能被 SSRF 利用、URL 里可能带敏感 token
- 现在 AI 调用 `fetch` 不触发审批，用户以为"纯查询"，实际在对外发数据

**修复**：
- 把 `fetch` 的 `side_effect` 从 `Pure` 改为 `Irreversible`
- 同步检查 `tool-fetch` 插件里其他方法的标注

---

### P0-2：`git_undo_commit` 无法撤销根提交

**现象**：非根提交正常撤销，根提交（无父）必失败。

**根因**：根提交没有父，撤销的语义不明确。

**当前行为**：隐性拒绝，但工具描述里没写。

**三个可选方向**：
1. **显式拒绝** —— 错误信息里写"根提交不可撤销，这是设计限制"
2. **实现它** —— 用 `git update-ref -d HEAD` 删除根引用
3. **降级** —— 退回到 `git init` 状态（清空所有历史）

**必须选一个，并写进工具描述。** 现在是"隐性失败 + 描述缺失"，AI 会反复尝试。

---

### P1-1：`git_push` 错误信息被截断

**现象**：无 remote 时，报错只剩 `"...and the repository exists."`，关键行丢失。

**影响**：AI 和用户都看不到真正的错误原因，会瞎试。

**修复方向**（任选）：
- 剥掉 git 输出的无关头部（`fatal: ` 等），只保留关键行
- 或保留**最后 N 行**（git 的关键信息通常在末尾）

---

### P2-1：`schedule_*` 多模式不互斥

**现象**：描述与行为不一致，多个调度模式可同时存在。

**修复**：要么实现互斥校验，要么改描述说明"可同时存在"。

---

### P2-2：`memory_undo` 描述与实际不符

**现象**：描述说需要审批，实际没有弹窗。

**修复**：改描述，或补上审批流程。

---

## 二、环境事实（重要）

### 🔴 事实 1：出站网络非全通

**现象**：国际站点被挡，国内站点可达。

**影响范围**：
- `tool-fetch` 抓国际站点会失败
- LLM 调用境外 API 会失败
- 未来插件市场如果从境外拉，会失败

**行动**：
- 写进 handoff，作为长期环境说明
- 所有"外网工具失败"的排查，**先怀疑网络，别怪工具**
- 长期维护一份"环境已知限制"文档

---

### 🔴 事实 2：本机 shell 是 Unix coreutils

**现象**：`rmdir /s /q` 不认，`rm -rf` 可用。

**影响**：
- `tool-shell` 里所有 Windows 命令都会失效
- AI 大概率会写 Windows 命令（因为它看到的是 Windows 系统）
- `fs_delete` 之前删不了目录，很可能就是这个原因

**需要排查的深层问题**：
- shell 工具到底跑在哪个环境里？（Git Bash / WSL / cmd / PowerShell）
- 工具描述里有没有明确说明？
- 跨平台兼容性怎么做？

**行动**：
- 短期：在工具描述里明确"本机使用 Unix 风格命令"
- 长期：排查 shell 工具的跨平台策略

---

## 三、本轮测试确认正常的项

**补偿类**：
- `fs_write` / `edit_lines` / `edit_replace` / `fs_restore` / `fs_delete`
- `memory_*` 系列

**副作用类**：
- `schedule_*` / `notify` / `sandbox_run` / `subagent_run`
- `git_*`（除根提交问题外）
- `mcp_reload` / `fetch`

**发现性**：
- `search_tools.catalog` 正常
- 初始列表扩容正常

**未测**：
- `send_email`（按用户意思跳过）

---

## 四、现场清理

- 临时仓库 `_gittest` 已用 `rm -rf` 清除
- `whack-a-mole.html` 全程未动（24432 字节）
- 记忆已更新 7 条
- `handoff.md` 已同步

---

## 五、建议处理顺序

| 顺序 | 事项 | 预估 |
|---|---|---|
| 1 | `fetch` 改 SideEffect | 5 分钟 |
| 2 | `git_undo_commit` 根提交行为明确 | 30 分钟 |
| 3 | `git_push` 错误信息处理 | 15 分钟 |
| 4 | 环境限制写进长期文档 | 10 分钟 |
| 5 | `schedule_*` / `memory_undo` 描述修正 | 20 分钟 |
| 6 | shell 跨平台策略排查 | 待评估 |

**1-5 一小时内可完成。第 6 项单独排期。**

---

## 六、处理记录（2026-10-06，v2.116 落地）

| 事项 | 处置 | 落点 |
|---|---|---|
| P0-1 fetch SideEffect | ✅ Pure → Irreversible；描述补「不是只读查询」；plugins.md §二表同步 | tool-fetch 0.1.1 |
| P0-2 根提交 | ✅ **选方向①显式拒绝**：`rev-list --parents -n 1` 判父，报错明说 ROOT commit 系设计限制 + 给 `git update-ref -d HEAD` via shell 的出路（需用户同意）；描述写明例外。方向②（实现 update-ref）记档不取——unborn HEAD 状态对用户困惑；方向③（降级 init）破坏性最大 | tool-git 0.1.3 |
| P1-1 push 报错截断 | ✅ `require_ok` 保留 stderr 末 **6 条**非空行（关键行在倒数第 4 行被旧「只取末行」截掉）；共享 helper，全部 git 工具受益 | tool-git 0.1.3 |
| P2-1 schedule 多模式 | ✅ **实现互斥校验**（描述承诺 exactly one of 是对的，错在代码静默优先级 interval > at > delay）：多模式同给显式拒绝并列出收到的字段；`start_at` 双锚点歧义路徠除 | desktop scheduler |
| P2-2 memory_undo 描述 | ✅ 改描述（行为正确，描述撒谎）：删「usually invoked by the compensation layer」劝退措辞（v2.115 在 fs_restore 同款裁定）；「requires user approval」改「跟随会话权限档」——决策表实况：**完全档 Irreversible 直接放行不出卡**（复测所见「没弹窗」即此档）；git_undo_commit 同款措辞一并修 | memory-sqlite 0.1.4 |
| 事实 1 出站网络 | ✅ 长期文档 [环境已知限制.md](环境已知限制.md)（LLM/fetch/未来市场三方影响 + 排查口诀 + 国内外二分对照法）；代理配置不做（记档） | docs/design/BUG/ |
| 事实 2 shell 环境 | ✅ 根因探明：tool-shell **无 shell** 直 spawn（`Command::new(首词)`），命令经 PATH 解析——本机 PATH 有 Git 的 usr/bin（Unix coreutils），故 `rm -rf` 可用、`rmdir /s /q` 把 `/s` `/q` 当路径、cmd 内建（dir/del/copy）无 exe 不存在。**不是「跑在 Git Bash 里」**。shell/shell_read 描述已写明「用 Unix 风格命令，cmd 内建不存在」（模型工具清单可见，不再试错）；跨平台策略维持现状、单独排期记档 | tool-shell 0.1.3 |
| 6 shell 跨平台策略 | ⏸ 短期缓解已做（描述如实）；长期策略（宿主声明环境 / 兼容层 / 不做）维持现状待评估 | 单独排期 |

**搭车**：desktop `organize::auto_collapse` flaky 在本批复测中从偶发变必现，定位为**测试自身竞速**（after_turn 派生的整理任务经 tauri::async_runtime 独立 runtime 抹掉 collapsed_at 回写）——夹具抬整理触发线根修，已记档 4 例 flaky 之一闭账。

*处理记录版本：v1.1*

---

*文档版本：v1.0（原始清单）→ v1.1（附处理记录）*
*配套项目：Locowrk*