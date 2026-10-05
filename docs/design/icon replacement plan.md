# Locowrk 图标替换方案

> 版本：1.0
> 目标：将当前内置 SVG 图标替换为 Tabler Icons，并接入点击态切换。
> 用途：交给 AI 执行时，按本文档操作即可。

---

## 一、目标

1. **统一图标语言**：全站图标改用 **Tabler Icons**（MIT 协议，开源免费，6000+ 图标）
2. **统一规格**：24×24 网格、2px 描边、`currentColor` 着色
3. **点击态切换**：启用/禁用、运行/停用、展开/收起等状态用 **Outline ↔ Filled** 两种形态切换
4. **不引入重型依赖**：优先用 SVG 内联或本地文件，不引入整个图标库

---

## 二、为什么要换

| 现状 | 问题 |
|---|---|
| 当前内置 SVG 风格不统一 | 有的偏实心，有的偏线条，大小不一致 |
| 与 Locowrk 气质不搭 | 太"图标库默认"，缺少克制感 |
| 无状态切换 | 点击只有颜色变化，没有形态变化 |

**Tabler Icons 的优势**：
- 24×24 标准网格，描边粗细统一为 2px
- 每个图标都有 Outline 和 Filled 两个版本
- 用 `currentColor`，跟随文字颜色
- MIT 协议，商用无风险

---

## 三、图标清单

**先确定需要替换的所有图标位置。** 当前 Locowrk 涉及的图标：

### 3.1 侧边栏

| 位置 | 语义 | Tabler 图标名 |
|---|---|---|
| 新建任务 | `+` | `plus` |
| 搜索 | 放大镜 | `search` |
| 插件市场 | 拼图 | `puzzle` |
| 项目 | 文件夹 | `folder` |

### 3.2 左下角

| 位置 | 语义 | Tabler 图标名 |
|---|---|---|
| 设置 | 齿轮 | `settings` |
| 文档 | 书 | `book` |
| 历史 | 时钟 | `history` |
| 通知 | 铃铛 | `bell` |

### 3.3 会话区

| 位置 | 语义 | Tabler 图标名 |
|---|---|---|
| 发送 | 箭头向上 | `arrow-up` |
| 附件 | 加号 | `plus` |
| 模型选择 | 闪光 | `sparkles` |

### 3.4 插件管理

| 位置 | 语义 | Tabler 图标名 |
|---|---|---|
| 工具（通用） | 扳手 | `tool` |
| 记忆 | 数据库 | `database` |
| 文件 | 文件 | `file` |
| 网络 | 地球 | `world` |
| Git | Git 分支 | `git-branch` |
| 终端 | 命令行 | `terminal-2` |
| 沙箱 | 盾牌 | `shield` |
| 通知 | 铃铛 | `bell` |
| MCP | 插头 | `plug` |

### 3.5 状态与反馈

| 语义 | Tabler 图标名 |
|---|---|
| 成功 | `check` |
| 失败 | `x` |
| 警告 | `alert-triangle` |
| 信息 | `info-circle` |
| 加载中 | `loader-2`（带旋转动画） |
| 展开 | `chevron-down` |
| 收起 | `chevron-up` |
| 更多 | `dots` |

---

## 四、状态切换规则

**每个可切换的图标，准备两个版本：**

| 状态 | 图标变体 | 例子 |
|---|---|---|
| 未启用 / 未选中 | **Outline**（描边） | `bell` |
| 已启用 / 已选中 | **Filled**（实心） | `bell-filled` |

**具体场景**：

| 场景 | Outline | Filled |
|---|---|---|
| 插件运行中 vs 已停用 | 停用状态用 Outline | 运行中状态用 Filled |
| 侧边栏选中 vs 未选中 | 未选中用 Outline | 选中用 Filled |
| 通知有未读 vs 无未读 | 无未读用 Outline | 有未读用 Filled |
| 展开 vs 收起 | 收起用 `chevron-down` | 展开用 `chevron-up` |

**切换动画**：Outline 和 Filled 之间用 `opacity` 交叉淡入淡出，不要做路径变形动画（Tabler 不保证路径兼容）。

---

## 五、实施步骤

### 步骤 1：下载图标

从 Tabler Icons 官网下载所需图标：
- 官网：https://tabler.io/icons
- 搜索图标名（如 `settings`）
- 选择 **Outline** 或 **Filled** 版本
- 点击 **Copy SVG** 或 **Download SVG**

**或者**从 npm 安装（如果项目已有 npm 依赖）：

```bash
npm install @tabler/icons-vue
```

### 步骤 2：放入项目

**方案 A（推荐）：本地 SVG 文件**

在项目中建立图标目录：

```
desktop/src/assets/icons/
├── plus.svg
├── search.svg
├── puzzle.svg
├── folder.svg
├── settings.svg
├── ...
```

每个 SVG 保持原样，只改一个地方：

```svg
<!-- 原始 -->
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" fill="none">
  ...
</svg>
```

**不需要改任何内容。** Tabler 的 SVG 已经用了 `currentColor`，直接能跟随文字颜色。

**方案 B：使用 Vue 组件库**

```bash
npm install @tabler/icons-vue
```

```vue
<script setup>
import { IconSettings, IconSearch } from '@tabler/icons-vue';
</script>

<template>
  <IconSettings :size="20" :stroke-width="1.75" />
</template>
```

**方案 A 更适合 Locowrk**——你现在的图标是内联 SVG，切换到本地文件管理更可控。

### 步骤 3：替换位置

**遍历项目里的所有 SVG 或图标组件，逐个替换。**

关键搜索词：
- `<svg` （内联 SVG）
- `icon-` （图标类名）
- `Icon` （Vue 组件名）

**替换时的注意事项**：

| 原来的 | 换成 |
|---|---|
| 硬编码颜色（如 `#333`） | `currentColor` |
| 不同尺寸（16/20/24/32） | 统一用 `:size="20"` 或 `24`，由 CSS 控制 |
| 自定义 stroke-width | 统一 `1.75` 或 `2` |

### 步骤 4：接入状态切换

以插件列表的状态图标为例：

```vue
<template>
  <!-- 运行中：Filled -->
  <IconBellFilled v-if="plugin.enabled" :size="18" class="icon-active" />
  
  <!-- 已停用：Outline -->
  <IconBell v-else :size="18" class="icon-inactive" />
</template>

<style scoped>
.icon-active {
  color: var(--color-primary);
  transition: color 0.15s, opacity 0.15s;
}
.icon-inactive {
  color: var(--color-text-muted);
  transition: color 0.15s, opacity 0.15s;
}
</style>
```

**不要做路径变形**（Outline 到 Filled 的 path 不一样），只做颜色和透明度的过渡。

### 步骤 5：给加载图标加旋转

```vue
<template>
  <IconLoader2 :size="16" class="icon-spin" />
</template>

<style scoped>
.icon-spin {
  animation: spin 1s linear infinite;
}
@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
```

**只给 `loader-2` 加旋转**，其他图标不加动画。

---

## 六、尺寸规范

**统一尺寸，不要一图一尺寸。**

| 场景 | 尺寸 | stroke-width |
|---|---|---|
| 侧边栏 | 20 | 1.75 |
| 按钮内 | 16 | 2 |
| 卡片内 | 18 | 1.75 |
| 大图标（空状态） | 48 | 1.5 |
| 输入框内 | 16 | 2 |

**实现方式**：定义 CSS 变量或 Vue 的 props。

```vue
<IconSettings :size="20" :stroke-width="1.75" />
```

---

## 七、颜色规范

**全部用 `currentColor`，不硬编码颜色。**

| 语义 | CSS 变量 |
|---|---|
| 默认 | `var(--color-text)` |
| 次要 | `var(--color-text-secondary)` |
| 弱化 | `var(--color-text-muted)` |
| 主色 | `var(--color-primary)` |
| 危险 | `var(--color-danger)` |
| 成功 | `var(--color-success)` |
| 警告 | `var(--color-warning)` |

**由父元素的 `color` 决定图标颜色。** 不要在 SVG 里写死颜色。

---

## 八、验证清单

替换完成后，逐项检查：

- [ ] 所有图标风格统一（描边宽度、圆角）
- [ ] 所有图标使用 `currentColor`，无硬编码颜色
- [ ] 所有图标尺寸用统一规范（20 / 16 / 18 / 48）
- [ ] 可切换状态的图标有 Outline 和 Filled 两个版本
- [ ] 切换时是颜色/透明度过渡，不是路径变形
- [ ] `loader-2` 有旋转动画
- [ ] 深色模式下颜色跟随（用 `currentColor` 自动生效）
- [ ] 无遗留旧图标（搜索 `<svg` 检查）
- [ ] 图标对齐基线正确（`vertical-align: middle`）

---

## 九、常见坑

| 坑 | 原因 | 解法 |
|---|---|---|
| 图标大小不一 | 每个图标单独设尺寸 | 统一规范，用 props 或 CSS 控制 |
| 颜色跟随失效 | SVG 里写了 `fill="#333"` | 改成 `currentColor` |
| 描边粗细不一致 | 有的 2px 有的 1.5px | 统一设为 1.75 或 2 |
| Filled 图标没有 | Tabler 部分图标没有 Filled 版 | 选有 Filled 的图标，或改用颜色区分状态 |
| 图标偏移 | 没有对齐基线 | 父元素加 `display: flex; align-items: center` |
| 深色模式下看不清 | 硬编码了黑色 | 全用 `currentColor` |
| 加载图标不旋转 | 忘了加动画 | 只给 `loader-2` 加 `spin` |

---

## 十、操作顺序

**AI 执行时按这个顺序来**：

1. 列出当前项目里所有用到图标的位置（搜索 `<svg`、`icon-`、`Icon`）
2. 建立图标目录 `desktop/src/assets/icons/`
3. 按本文档 §3 清单逐个下载 Tabler 图标
4. 逐个替换，替换时遵守 §6 尺寸、§7 颜色规范
5. 对可切换状态的图标，准备 Outline 和 Filled 两版
6. 按 §8 验证清单逐项检查
7. 提交前搜索一遍，确保没有遗留旧 SVG

---

## 十一、备注

**为什么不用整个图标库**

引入 `@tabler/icons-vue` 会打包 6000+ 图标，虽然支持 tree-shaking，但对于 Locowrk 只需要几十个图标的场景，**本地 SVG 文件更轻**。

**什么时候该用组件库**

如果未来图标数量超过 100 个，或者需要频繁切换图标，再考虑引入组件库。

**关于自定义图标**

Locowrk 的核心品牌图标（云朵、主 Logo）**不替换**，继续用自定义 SVG。Tabler 只用于功能图标。

---

*文档版本：v1.0*
*配套项目：Locowrk*
*图标来源：https://tabler.io/icons*
*协议：MIT*