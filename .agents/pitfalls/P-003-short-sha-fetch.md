# P-003：git fetch 短 SHA 失败

## 现象

```
git fetch --depth 1 origin 38ff99177
fatal: couldn't find remote ref 38ff99177
```

上游 pin 只记了 7 位短 SHA，仓库按 ref 解析失败，取不到样本。

## 根因

`git fetch <remote> <sha>` 只接受能解析成 ref 或完整对象名的值；短 SHA 不是 ref，GitHub 也不为短 SHA 提供 `want`。短 SHA 还可能在历史增长后变得不唯一。

## 修复

1. 先用 GitHub API 解析全 SHA：`GET /repos/<owner>/<repo>/commits/<short>`，取 `sha` 字段。
2. 再 `git fetch --depth 1 origin <full-sha>`，`git checkout FETCH_HEAD`。
3. 文档里的 pin 一律写全 SHA（本次：`38ff99177fba21e9c57a670f4e95adcf407c99c7`）。

## 预防

- 记录上游版本时，短 SHA 只作人类可读别名，旁边必须带全 SHA。
- 取样本的脚本先解析再 fetch；不要把短 SHA 直接喂给 git。
