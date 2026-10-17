# Makefile —— 命令入口的**薄封装**：只做转发，不写实现。
# 实现唯一位置 = scripts/tasks.sh（★）。没装 make 时：sh scripts/tasks.sh <target>，完全等价。
# 单行 `target: ; cmd` 写法刻意避开 tab 依赖（Windows 编辑器容易把 tab 换成空格）。

.PHONY: help setup fmt fmt-check clippy type lint test gates doc check quick adr specs

help:       ; @sh scripts/tasks.sh help
setup:      ;  sh scripts/tasks.sh setup
fmt:        ;  sh scripts/tasks.sh fmt
fmt-check:  ;  sh scripts/tasks.sh fmt-check
clippy:     ;  sh scripts/tasks.sh clippy
type:       ;  sh scripts/tasks.sh type
lint:       ;  sh scripts/tasks.sh lint
test:       ;  sh scripts/tasks.sh test
gates:      ;  sh scripts/tasks.sh gates
doc:        ;  sh scripts/tasks.sh doc
check:      ;  sh scripts/tasks.sh check
quick:      ;  sh scripts/tasks.sh quick
adr:        ;  sh scripts/tasks.sh adr
specs:      ;  sh scripts/tasks.sh specs
