# P-006：PowerShell 改写 UTF-8 源文件会丢中文

- 现象：用 `Set-Content` / `Add-Content` / `-replace` 处理含中文的 Go/TS 文件后，编译报 `invalid UTF-8 encoding`，中文注释和字符串变成 `�`；`git diff` 看起来像整文件乱码。
- 根因：Windows PowerShell 5 默认按本地代码页写回，未显式指定编码时把 UTF-8 字节重新编码；`Get-Content` 读入时同样可能按 ANSI 解码。读取显示乱码不等于文件损坏，但**写回**会造成真实损坏。
- 修复：用 `git checkout -- <file>` 还原，改动用编辑工具（write/edit）重做；本次 `internal/kernel/kernel.go` 因此丢过一次未提交改动。
- 预防：改仓库内文本一律用编辑工具；必须用 shell 时显式 `-Encoding utf8NoBOM`（PS 7）或 `Set-Content -Encoding utf8`（PS 5，仍可能带 BOM，需检查）。读取中文只在控制台显示乱码时，先用 Node/Go 或编辑器确认字节，不要据此改文件。
- 再次发生记录：（暂无）
