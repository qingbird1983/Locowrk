# Makefile —— 命令入口的**薄封装**：只做转发，不写实现，也不抄清单。
#
# target 清单的唯一出处 = scripts/tasks.sh（★）。这里以前逐个写 14 行，加一个 target 就要
# 在两处各记一遍——CI 那份手抄清单实测已经漏了两道闸（docs/adr/0002 有记录），同一个道理。
# 剩下的未知 target 由 tasks.sh 明确报错并列出可用项，不静默通过。
#
# 没装 make 时完全等价：sh scripts/tasks.sh <target>
# 单行 `target: ; cmd` 写法刻意避开 tab 依赖（Windows 编辑器容易把 tab 换成空格）。

.PHONY: help

help: ; @sh scripts/tasks.sh help

# 兜底转发：make check / make gates / make ledger LEDGER=settings-llm 等都走这一条。
%: ;  @sh scripts/tasks.sh $@
