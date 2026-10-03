# Locowrk 插件元数据与打包设计（v2）

> 配套项目：Locowrk
> 版本：2.1（在 v1 ABI 基础上扩展元数据，不破坏 ABI；v2.1 增补 `origin` 来源分类）

---

## 一、设计目标

| 目标 | 含义 |
|---|---|
| **代码标识与展示名分离** | `id` 永不翻译，`display_name` 可翻译 |
| **多语言内嵌** | 插件自包含翻译，宿主无需预置 key |
| **图标可分发** | 插件带自己的视觉标识，不依赖宿主内置 |
| **零破坏兼容** | 旧 manifest 依然能被解析，不报错 |
| **任何字段可缺失** | 缺失时优雅 fallback，不崩列表 |

### 非目标

- 不支持插件侧自定义主题色（宿主统一控制）
- 不支持动态生成图标（作者自己出图）
- 不支持图标字体（SVG / PNG 足够）

---

## 二、Manifest Schema v2

### 2.1 完整示例

```json
{
  "id": "tool-shout",
  "version": "1.1.0",
  "abi_version": 1,
  "kind": "tool",

  "display_name": {
    "zh-CN": "大小写转换",
    "en-US": "Shout",
    "ja-JP": "シャウト"
  },
  "description": {
    "zh-CN": "把文本转换成大写输出",
    "en-US": "Shouts text back in uppercase"
  },
  "icon": "assets/icon.svg",

  "author": {
    "name": "Locowrk Team",
    "url": "https://github.com/locowrk"
  },
  "origin": "official",
  "homepage": "https://github.com/locowrk/plugins/tool-shout",
  "license": "MIT",

  "tools": [
    {
      "name": "shout",
      "display_name": {
        "zh-CN": "转换大写",
        "en-US": "Shout"
      },
      "description": {
        "zh-CN": "把输入的文本全部转成大写",
        "en-US": "Convert the input text to uppercase"
      },
      "schema": {
        "type": "object",
        "properties": {
          "text": { "type": "string", "description": "要转换的文本" }
        },
        "required": ["text"]
      },
      "side_effect": "Pure"
    }
  ],

  "reversible": {
    "kind": "Pure"
  }
}
```

### 2.2 字段规范

| 字段 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `id` | string | ✅ | ASCII、小写、连字符分隔。**永不翻译** |
| `version` | semver string | ✅ | 语义化版本 |
| `abi_version` | u32 | ✅ | ABI 版本，当前为 1 |
| `kind` | enum | ✅ | `tool` / `model` / `memory` / `middleware` |
| `display_name` | LocalizedString | ⭕ | 缺失时回退到 `id` |
| `description` | LocalizedString | ⭕ | 缺失时留空 |
| `icon` | string（相对路径） | ⭕ | 缺失时显示首字母兜底 |
| `author` | object | ⭕ | 作者信息 |
| `origin` | string | ⭕ | 来源分类：`official` / `community`，**缺省 `community`** |
| `homepage` | string | ⭕ | 项目主页 |
| `license` | string | ⭕ | SPDX 标识 |
| `tools` | ToolSpec[] | ✅ | 工具列表 |
| `reversible` | Reversible | ✅ | 插件级副作用声明 |

### 2.3 LocalizedString 规范

**定义**：`Record<string, string>`，key 为 BCP-47 语言标签，value 为该语言的展示文本。

**约束**：
- 至少包含 `en-US` 一个语言
- key 必须是有效的 BCP-47 标签（`zh-CN` / `en-US` / `ja-JP`）
- value 长度限制：`display_name` ≤ 32 字符，`description` ≤ 200 字符

**语言回退顺序**（`pickLocalized` 实现）：

```
1. 精确匹配      zh-CN → zh-CN
2. 主语言匹配    zh-CN → zh
3. 英语回退      → en-US
4. 任意一个      → Object.values[0]
5. 硬回退        → plugin.id
```

### 2.4 `origin` 来源分类

**已安装列表按 `origin` 分两大类**：

| 分类 | 取值 | 现状 |
|---|---|---|
| 官方内置 | `official` | 仓库自带插件（tool-echo / tool-fs / tool-fetch / tool-shout）显式声明 |
| 第三方 | `community`（缺省） | **预留给未来**——市场开放第三方作者后启用 |

约束：

- **缺省一律 `community`**：官方标识必须显式声明，不能由缺失推断——防止第三方包伪造"官方内置"徽章（安全默认）
- `official` 只被仓库自带插件使用；未来若官方插件走市场分发，由签名/渠道校验兜底（本期不做）
- 宿主不因 `origin` 改变权限或审批行为——分类只影响展示分组，不做信任边界

---

## 三、目录结构

### 3.1 单个插件版本

```
plugins/tool-shout/
  └── 1.1.0/
      ├── manifest.json
      ├── tool_shout.dll
      └── assets/
          └── icon.svg
```

**约定**：
- `icon` 字段是**相对 manifest 的路径**，可放在任意子目录
- 图标目录建议统一为 `assets/`，但非强制
- dll 名字由作者决定，宿主不假设文件名

### 3.2 多版本共存

```
plugins/tool-shout/
  ├── 1.0.0/
  │   ├── manifest.json
  │   ├── tool_shout.dll
  │   └── assets/icon.svg
  ├── 1.1.0/
  │   ├── manifest.json
  │   ├── tool_shout.dll
  │   └── assets/icon.svg
  └── 1.1.1/
      └── ...
```

**每个版本目录自包含**。回退只需切换 `active` 指针，不需要移动文件。

---

## 四、图标规范

### 4.1 格式支持

| 格式 | 优先级 | 说明 |
|---|---|---|
| SVG | 推荐 | 无损缩放，可换色 |
| PNG | 兼容 | 建议 256×256 以上 |
| WebP | 兼容 | 体积小 |

**不支持**：GIF、JPEG、ICO。理由：动图会分散注意力，JPEG 有压缩伪影，ICO 是 Windows 专用格式。

### 4.2 尺寸与安全区

```
┌─────────────────────┐
│    安全边距 12.5%    │
│  ┌───────────────┐  │
│  │               │  │
│  │   图形主体     │  │  ← 有效绘制区
│  │               │  │
│  └───────────────┘  │
│                     │
└─────────────────────┘
```

- **画布**：正方形（1:1）
- **安全边距**：画布边长的 12.5%
- **视觉重心**：几何居中
- **背景**：透明（宿主提供卡片背景）

### 4.3 颜色约定

**推荐**：图标主体使用 `currentColor` 或宿主主题色，随卡片自适应。

**不推荐**：图标内置硬编码颜色，会和宿主主题冲突。

**允许**：图标自带品牌色（如 Slack 的紫色），此时宿主不再注入颜色。

### 4.4 SVG 模板

```svg
<svg viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg" fill="none">
  <!-- 主体图形，使用 currentColor -->
  <path d="..." stroke="currentColor" stroke-width="4" stroke-linecap="round"/>
  <!-- 如果需要强调色，用固定色 -->
  <circle cx="46" cy="22" r="3" fill="#5EEAD4"/>
</svg>
```

---

## 五、Rust 侧实现

### 5.1 类型定义

```rust
// plugin-api/src/manifest.rs
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 本地化字符串：语言标签 → 展示文本
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocalizedString(pub HashMap<String, String>);

impl LocalizedString {
    /// 按回退顺序挑选最适合的语言
    pub fn pick(&self, locale: &str) -> Option<&str> {
        // 1. 精确匹配
        if let Some(s) = self.0.get(locale) {
            return Some(s);
        }
        // 2. 主语言匹配：zh-CN → zh
        let primary = locale.split('-').next().unwrap_or(locale);
        if let Some((_, v)) = self.0.iter()
            .find(|(k, _)| k.split('-').next() == Some(primary))
        {
            return Some(v);
        }
        // 3. 英语回退
        if let Some(s) = self.0.get("en-US") {
            return Some(s);
        }
        // 4. 任意一个
        self.0.values().next().map(|s| s.as_str())
    }
}

/// 兼容旧版：description 曾为 String
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LocalizedOrString {
    Plain(String),
    Localized(LocalizedString),
}

impl LocalizedOrString {
    pub fn into_localized(self) -> LocalizedString {
        match self {
            Self::Plain(s) => {
                let mut m = HashMap::new();
                m.insert("en-US".to_string(), s);
                LocalizedString(m)
            }
            Self::Localized(l) => l,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub version: String,
    pub abi_version: u32,
    pub kind: PluginKind,

    #[serde(default)]
    pub display_name: Option<LocalizedString>,
    #[serde(default)]
    pub description: Option<LocalizedOrString>,
    #[serde(default)]
    pub icon: Option<String>,

    #[serde(default)]
    pub author: Option<Author>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub license: Option<String>,

    pub tools: Vec<ToolSpec>,
    pub reversible: Reversible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}
```

### 5.2 图标加载（base64 内嵌方案）

```rust
// plugin-host/src/icon.rs
use std::path::{Path, PathBuf};
use base64::Engine;

pub struct IconLoader {
    /// 已加载的图标缓存，key = "plugin_id@version"
    cache: dashmap::DashMap<String, Option<String>>,
}

impl IconLoader {
    /// 读取图标并编码为 data URL
    pub fn load(&self, plugin_dir: &Path, icon_rel: &str) -> Option<String> {
        let cache_key = format!("{}::{}", plugin_dir.display(), icon_rel);
        if let Some(v) = self.cache.get(&cache_key) {
            return v.clone();
        }

        let result = self.load_inner(plugin_dir, icon_rel);
        self.cache.insert(cache_key, result.clone());
        result
    }

    fn load_inner(&self, plugin_dir: &Path, icon_rel: &str) -> Option<String> {
        let path = plugin_dir.join(icon_rel);
        // 防路径穿越
        let canon_dir = plugin_dir.canonicalize().ok()?;
        let canon_path = path.canonicalize().ok()?;
        if !canon_path.starts_with(&canon_dir) {
            tracing::warn!(?path, "icon path escapes plugin dir, rejected");
            return None;
        }

        let bytes = std::fs::read(&canon_path).ok()?;
        // 大小限制：1MB
        if bytes.len() > 1024 * 1024 {
            tracing::warn!(?path, "icon too large, rejected");
            return None;
        }

        let mime = match canon_path.extension().and_then(|e| e.to_str()) {
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            Some("webp") => "image/webp",
            _ => {
                tracing::warn!(?path, "unsupported icon format");
                return None;
            }
        };

        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Some(format!("data:{};base64,{}", mime, b64))
    }
}
```

### 5.3 PluginUiState 扩展

```rust
// plugin-host/src/ui_state.rs
#[derive(Debug, Clone, Serialize)]
pub struct PluginUiState {
    pub id: String,
    pub version: String,
    pub state: PluginState,

    /// 原始 LocalizedString，前端根据当前语言挑选
    pub display_name: Option<HashMap<String, String>>,
    pub description:  Option<HashMap<String, String>>,
    /// data URL，前端可直接塞进 <img src>
    pub icon: Option<String>,

    pub author: Option<Author>,
    pub homepage: Option<String>,
    pub license: Option<String>,

    pub side_effect_summary: Vec<SideEffectInfo>,
}
```

**注意**：图标字段命名为 `icon`，值是 data URL，前端不需要感知加载细节。

---

## 六、前端渲染

### 6.1 类型定义

```typescript
// desktop/src/types/plugin.ts
export interface PluginUiState {
  id: string;
  version: string;
  state: 'installed' | 'active' | 'loading' | 'error';
  displayName: Record<string, string> | null;
  description: Record<string, string> | null;
  icon: string | null;        // data URL
  author?: { name: string; url?: string };
  homepage?: string;
  license?: string;
  sideEffects: SideEffectInfo[];
}
```

### 6.2 本地化工具

```typescript
// desktop/src/lib/i18n-helpers.ts
export function pickLocalized(
  obj: Record<string, string> | null | undefined,
  locale: string,
  fallback: string,
): string {
  if (!obj) return fallback;
  if (obj[locale]) return obj[locale];

  const primary = locale.split('-')[0];
  const hit = Object.entries(obj).find(
    ([k]) => k.split('-')[0] === primary,
  );
  if (hit) return hit[1];

  return obj['en-US'] ?? Object.values(obj)[0] ?? fallback;
}
```

### 6.3 卡片组件

```vue
<!-- desktop/src/components/PluginCard.vue -->
<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { pickLocalized } from '../lib/i18n-helpers';
import type { PluginUiState } from '../types/plugin';

const props = defineProps<{ plugin: PluginUiState }>();
const { locale } = useI18n();

const name = computed(() =>
  pickLocalized(props.plugin.displayName, locale.value, props.plugin.id)
);

const desc = computed(() =>
  pickLocalized(props.plugin.description, locale.value, '')
);

const iconFallback = computed(() =>
  props.plugin.id[0]?.toUpperCase() ?? '?'
);

function onIconError(e: Event) {
  (e.target as HTMLImageElement).style.display = 'none';
}
</script>

<template>
  <div class="plugin-card">
    <div class="plugin-icon-wrap">
      <img
        v-if="plugin.icon"
        :src="plugin.icon"
        :alt="name"
        class="plugin-icon"
        @error="onIconError"
      />
      <div v-else class="plugin-icon-fallback">
        {{ iconFallback }}
      </div>
    </div>

    <div class="plugin-info">
      <div class="plugin-header">
        <h3>{{ name }}</h3>
        <span class="version">v{{ plugin.version }}</span>
      </div>
      <p v-if="desc" class="desc">{{ desc }}</p>
      <div class="side-effects">
        <span
          v-for="se in plugin.sideEffects"
          :key="se.kind"
          class="se-badge"
          :class="`se-${se.kind.toLowerCase()}`"
        >{{ se.kind }}</span>
      </div>
    </div>

    <div class="plugin-actions">
      <slot name="actions" />
    </div>
  </div>
</template>

<style scoped>
.plugin-card {
  display: grid;
  grid-template-columns: 48px 1fr auto;
  gap: 12px;
  align-items: center;
  padding: 12px 16px;
  background: var(--card-bg);
  border-radius: 10px;
}
.plugin-icon-wrap {
  width: 48px;
  height: 48px;
  display: grid;
  place-items: center;
}
.plugin-icon {
  width: 100%;
  height: 100%;
  object-fit: contain;
}
.plugin-icon-fallback {
  width: 48px;
  height: 48px;
  border-radius: 10px;
  background: var(--bg-elevated);
  color: var(--text-secondary);
  display: grid;
  place-items: center;
  font-weight: 600;
  font-size: 20px;
}
</style>
```

### 6.4 副作用徽章配色

```css
.se-pure         { background: rgba(94, 234, 212, 0.15); color: #5EEAD4; }
.se-compensable  { background: rgba(250, 204, 21, 0.15); color: #FACC15; }
.se-irreversible { background: rgba(248, 113, 113, 0.15); color: #F87171; }
```

---

## 七、Fallback 矩阵

**任何字段缺失都必须能渲染。** 以下矩阵定义所有降级路径：

| 场景 | 行为 |
|---|---|
| `display_name` 缺失 | 显示 `id` |
| `display_name` 无当前语言 | 按回退顺序挑选 |
| `description` 缺失 | 不渲染描述区 |
| `icon` 字段缺失 | 显示 `id` 首字母方块 |
| 图标文件不存在 | `<img>` onerror 隐藏，显示首字母 |
| 图标格式不支持 | 加载器返回 `None`，显示首字母 |
| 图标超过 1MB | 拒绝加载，显示首字母 |
| 图标路径穿越 | 拒绝加载，显示首字母 |
| `author` / `homepage` / `license` 缺失 | 对应信息不渲染 |

**核心原则**：任何一个字段的问题都不能导致列表崩溃。

---

## 八、插件脚手架

`locowrk new plugin <name>` 生成的项目结构：

```
my-plugin/
├── Cargo.toml
├── src/
│   └── lib.rs
├── manifest.json
└── assets/
    └── icon.svg          ← 默认占位图标
```

### 8.1 manifest.json 模板

```json
{
  "id": "tool-my-plugin",
  "version": "0.1.0",
  "abi_version": 1,
  "kind": "tool",

  "display_name": {
    "zh-CN": "我的插件",
    "en-US": "My Plugin"
  },
  "description": {
    "zh-CN": "描述这个插件做什么",
    "en-US": "Describe what this plugin does"
  },
  "icon": "assets/icon.svg",

  "author": {
    "name": "Your Name",
    "url": "https://github.com/your-name"
  },
  "license": "MIT",

  "tools": [
    {
      "name": "my_tool",
      "display_name": {
        "zh-CN": "我的工具",
        "en-US": "My Tool"
      },
      "description": {
        "zh-CN": "工具描述",
        "en-US": "Tool description"
      },
      "schema": {
        "type": "object",
        "properties": {
          "input": { "type": "string", "description": "输入" }
        },
        "required": ["input"]
      },
      "side_effect": "Pure"
    }
  ],

  "reversible": { "kind": "Pure" }
}
```

### 8.2 默认占位图标

```svg
<svg viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg" fill="none">
  <rect x="8" y="8" width="48" height="48" rx="12"
        stroke="currentColor" stroke-width="3"/>
  <circle cx="32" cy="32" r="6" fill="currentColor"/>
  <path d="M32 14 V26 M32 38 V50 M14 32 H26 M38 32 H50"
        stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
</svg>
```

---

## 九、版本与兼容性

### 9.1 ABI 不变

本次扩展**不修改 ABI**：
- 新增字段只在 `manifest.json` 中，不影响符号约定
- `abi_version` 保持为 1
- 旧插件无需重新编译

### 9.2 Manifest 版本演进

在 manifest 中可选地加一个 `manifest_version`：

```json
{
  "manifest_version": 2,
  "id": "tool-shout",
  ...
}
```

- 缺失时默认视为 1（旧格式，description 为 String）
- 宿主同时支持 v1 和 v2 解析
- 未来 v3 引入时，v2 依然能读

### 9.3 兼容矩阵

| manifest_version | description 类型 | 图标 | 多语言 |
|---|---|---|---|
| 1（或无） | String | ❌ | ❌ |
| 2 | LocalizedString | ✅ | ✅ |

**解析器必须同时支持两者**，用 `LocalizedOrString` 做分支。

### 9.4 内置插件版本策略（S2 定稿，2026-09-30）

《Setting and Constrained Optimization.md》§1 顺带项的裁决：**方案 B——独立升**
（插件随仓库自己的节奏 bump 版本号，主程序升级不强制携带版本跳变）。

- **不加 `bundled` 字段**。UI 与服务端的"内置"判定一律看 lockfile 的
  `source` 字段（`InstallSource::Builtin`，S1 预留枚举本批启用）；
  manifest 自声明的 `origin` 只管展示分组（official/community），
  **不作为任何操作门控依据**——防市场插件伪装 official 骗分组。
- **持久化**：开发态 `plugins/` 扫描每成功装载一个内置插件，服务层
  `register_builtin` 在 lockfile 写/更 `Builtin` 条目（版本同步，保留
  `enabled`——重启不复活被停用的插件）；停用的/已被市场接管的 id 由
  `dev_scan_skip_ids` 给出，启动扫描跳过。
- **启用取最新**：内置插件启用时从内置根解析**最新**版本（独立升级
  后条目版本可能滞后），漂移同步回条目。
- **接管规则**：同 id 从市场显式安装 → lockfile 条目转 `Market`
  （安装态赢过开发态），开发态扫描让位；卸载市场版后开发态副本在
  下次启动自然回归。
- **不给内置插件卸载/更新**（随程序分发，删了下个版本又回来）；
  版本演进随仓库提交，运行期"回退"依赖 registry 内多版本并存。

### 9.4·补 版本号基准重置（㉘·B 搭车，2026-10-04 用户裁定）

全部插件版本号**重置为 0.1.0 起步**，与主程序递进对齐（避免插件 1.x 对主程序
早期版本的量级悬殊）：

- **小迭代**：patch 位递增（`0.1.0 → 0.1.1`），随批正常 bump，不需确认；
- **大改版**：minor 位递增（`0.1.x → 0.2.0`），**须经用户确认**后执行；
- 语义化版本与 semver 比较规则不变；重置批对"lockfile 条目版本高于内置根可用
  版本"的旧数据（1.x 遗留）要求服务层**回退到可用最新版**而非报错——pinned 版本
  在内置根无对应目录即视为不可用，warn 后取剩余最新（本就比"启动失败"诚实）。

---

## 十、落地路径

| 阶段 | 内容 | 验收标准 |
|---|---|---|
| 1 | 改 `manifest.json`，加 `display_name` / `description` / `icon` | 卡片显示中文名 |
| 2 | Rust 侧 `PluginManifest` 加字段，`LocalizedString` 实现 | 解析不报错 |
| 3 | `pickLocalized` 前端工具 + 卡片渲染 | 切换语言时卡片文案跟着变 |
| 4 | `IconLoader` base64 加载 + 前端 `<img>` | 卡片显示图标 |
| 5 | Fallback 矩阵全覆盖 | 删字段、删图标、错格式都不崩 |
| 6 | 更新 `locowrk new plugin` 模板 | 新插件自带 i18n 和图标占位 |
| 7 | 版本升级测试：`1.1.0` → `1.1.1` | 版本下拉并存，回退可用 |

---

## 十一、常见坑

| 坑 | 原因 | 解法 |
|---|---|---|
| 图标不显示 | 路径相对 manifest，不是相对 dll | 用 `manifest_dir.join(icon)` |
| 图标加载慢 | 每次 `plugin_list` 都读盘 | `IconLoader` 加缓存 |
| 路径穿越 | `icon: "../../etc/passwd"` | `canonicalize` + `starts_with` 校验 |
| 大图标卡 UI | 图片几 MB 直接 base64 | 限制 1MB，超了拒绝 |
| 语言不匹配 | 只匹配了 `zh-CN`，用户用 `zh-TW` | 主语言回退 |
| 列表崩 | 某插件 manifest 格式错 | 解析失败时跳过该插件，不整体报错 |
| 旧插件加载失败 | 严格 schema 校验 | `#[serde(default)]` + `LocalizedOrString` |

---

## 十二、未来扩展

以下方向留出扩展空间，本期不做：

- **图标主题变体**：`icon_light` / `icon_dark`
- **截图与预览**：`screenshots: string[]`，用于市场详情页
- **分类标签**：`categories: string[]`，用于市场筛选
- **依赖声明**：`dependencies: { "plugin_id": ">=1.0.0" }`
- **权限声明**：`permissions: ["fs.read", "net.http"]`，用于安装前提示
- **资助链接**：`funding: string[]`，开源作者收益入口

这些字段的命名和位置在本设计中预留，未来加时保持向后兼容。

---

*文档版本：v2.0*
*配套项目：Locowrk*
*前置文档：《Locowrk Agent 内核设计》《Locowrk 插件清单》*