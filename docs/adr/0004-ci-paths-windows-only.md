# ADR-0004：路径语义只在 Windows 上验证

- 状态：被取代（2026-10-17，同一天复核后推翻；见 [`ADR-0005`](0005-linux-verified-on-both-platforms.md)）
- 日期：2026-10-17
- 决策人：青鸟（"Linux 暂时没打算出，先选二"）/ Qoder（逐条判定哪些断言真依赖平台）
- 出处：首次推送后的真 CI run 39032117025（`kernel / Tests (workspace root)` 红 4 条）；台账 T18
- 取代：无
- 被取代：Superseded by [`ADR-0005`](0005-linux-verified-on-both-platforms.md)

## 背景

仓库推上远端后 CI 第一次真跑。结构闸那个作业绿，内核测试作业在 **ubuntu** 上红 4 条，而本机（Windows）
同一批全绿、合计 515 条不变：

- `fsutil::missing_path_outside_roots_reports_outside_not_os_error` —— 实测 `got: cannot resolve any existing ancestor`：
  POSIX 上这条夹具路径先撞上"中间段解析不到"，撞不到"越界"那条裁决。
- `memlayout::project_dir_name_is_readable_and_collision_safe` 与
  `memlayout::project_dir_keeps_any_workspace_string_under_the_root` —— 输入是 `F:\Warehouse\…`、`a\b\..\c` 这类
  反斜杠串；在 POSIX 上反斜杠是**合法文件名字符**，整串就是一个目录名，"取末段""含穿越材料"这两个判读法不适用。
- `wskey::dotdot_inside_the_path_is_resolved_not_left_as_text` —— 依赖 `canonicalize` 折叠 `..`：Windows 的路径
  API 对不存在的中间段也做词法折叠，POSIX 不做，于是 Linux 上 `/a/x/../b` 与 `/a/b` 落到两个键位。

关键事实：这四条**不是实现回归**（本轮没动过一行 Rust），而是断言本身按 Windows 语义出题。
项目定位（AGENTS.md §1）本就写着单人自用内测，不含服务端与跨平台发布。

## 决策

把这四条标成 `#[cfg(windows)]` 并在每处上方写清"为什么在 Linux 上不成立"，Linux 上其余 49 条内核用例
照跑；CI 保持在 ubuntu 上跑。同时把"路径语义只在 Windows 上验证"写成本仓的公开口径 —— 不是"我们还没修"，
而是"这个平台的语义我们不承诺"。

将来真要出 Linux 版时，要做的不是解开这四个标记，而是**另立一批 POSIX 用例**：符号链接、`..` 词法折叠、
大小写敏感性三件都得重新出题（`wskey.rs` 的头注已经写明 POSIX 上 `/Tmp/X` 与 `/tmp/x` 是两个真不同的目录）。

## 被否决的方案

| 方案 | 否决原因 |
| --- | --- |
| CI 换成 Windows 机器跑 | 要动禁区 `.github/workflows/`；Windows runner 排队更慢、私有仓按双倍计免费分钟数；而换过来只是让云端重复本机已经天天在跑的那 515 条，不新增任何证据 |
| 把实现补成两边都成立（在 `posix_form` 里自己做词法折叠） | 等于事实上开始支持 Linux：要正确处理 `..` 就必须同时处理符号链接与大小写，改动落在**冻结 ABI 边上**的 `plugin-api`，还要重立记忆目录名与键位 —— 为一个不出货的平台付这个代价不值 |
| 直接删掉这四条测试 | 它们在 Windows 上挡的都是真错（穿越材料进目录名、同目录得两个键位、越界文案被误读成读失败），删了就是把承重面拆了 |
| CI 里整个跳过内核测试作业 | Linux 面上其余 49 条有证明力（含跨 crate 的围栏与 ABI 用例），全跳等于把闸拆掉换绿 |
| 让每条测试"按平台各断言各的" | 在没有主人的平台上写断言 = 猜 POSIX 该是什么样。真要支持 Linux，先出题再写断言，而不是反过来 |

## 后果

**正面：**
CI 恢复有意义且为绿：结构闸作业 + 49 条 Linux 内核用例继续跑，Windows 上那 4 条继续在你本机挡错。
"哪些性质是平台特定的"从口头变成写在测试上方与提交记录里的东西，四条红也都留了实测文案而不是含糊过去。

**负面 / 需要接受的代价：**
Linux 上的路径行为从此**没有任何证据**：如果哪天有人在 Linux 上跑这个内核，目录名、键位、围栏判定都可能
与 Windows 不一致，而 CI 不会报。这条要写进入口口径，别让 CI 的绿标被误读成"跨平台可用"。

**后续需要跟进的事项：**
- 台账 T18 按本决策收口（勾掉，写"选二已落地"），不再挂着。
- `docs/design/authority/DESIGN.md` 里若要新增跨平台相关规范，先回来更新本篇。
- 真要做 Linux：先写一份 `specs/<新阶段>` 出题（符号链接 / `..` / 大小写三面），再动 `plugin-api`。

## 参考

[`ADR-0002`](0002-gates-rewritten-to-template.md)（闸与 CI 的清单唯一出处）·
[`specs/03-structural-debt`](../../specs/03-structural-debt/spec.md) 的 T18 ·
`crates/plugin-api/src/wskey.rs` 头注（POSIX 大小写敏感这条既有说明）
