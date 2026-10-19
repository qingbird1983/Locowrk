/**
 * 插件面与市场：插件展示元数据、安装态、闲置与用量、批量结果，以及事件里用到的资产引用 DTO。
 *
 * 由批 F-22（v2.175）从 `desktop/src/types.ts` 目录化拆出：**每个类型声明块逐字节原样搬移**，
 * 只改两处——①按引用面决定 `export` 与否（外部 import 过的与兄弟簇要用的留 `export`，
 * 其余降为模块内部）；②跨簇引用走 `import type`。等价判据见 scripts/../tmp/v180-equiv.mjs。
 */

/** 插件管理区投影（⑬）：自动停用天数（0 = 关闭，缺省）。 */
export interface PluginsView {
  autoDisableIdleDays: number;
}

/** 插件管理区部分更新：字段缺省 = 保持不变。 */
export interface PluginsUpdate {
  autoDisableIdleDays?: number;
}

/** 插件展示元数据（packaging.md §5.3 投影；manifest.json 缺失时为
 * 缺省投影：origin = "community"，无图标——UI 首字母兜底）。 */
interface PluginDisplay {
  display_name?: LocalizedMap;
  description?: LocalizedMap;
  /** data URL（svg/png/webp）；缺省 = 首字母方块。 */
  icon?: string;
  author?: { name: string; url?: string | null };
  homepage?: string;
  license?: string;
  /** "official" | "community"；只影响展示分组，不是信任边界。 */
  origin: string;
  /** 副作用去重清单（pure → compensable → irreversible 固定序）。 */
  side_effects: string[];
}

/** 已安装插件（lockfile 投影 + 注册表运行态合并 + 展示投影）。 */
export interface InstalledPlugin {
  id: string;
  version: string;
  enabled: boolean;
  installed_at: number;
  source: string;
  active_version: string | null;
  loaded_versions: string[];
  display: PluginDisplay;
}

/** plugin://state-changed 载荷（plugin-service PluginStateChange 镜像，
 * type 标签）。 */
export type PluginStateChange =
  | { type: "installed"; id: string; version: string }
  | { type: "enabled_changed"; id: string; enabled: boolean }
  | { type: "uninstalled"; id: string }
  | { type: "active_version_changed"; id: string; version: string };

/** 市场目录条目（plugin_registry_list 返回项；versions 升序，末位 latest；
 * display 取自 latest 版本目录的 manifest.json，packaging.md §5.3）。 */
export interface MarketplaceItem {
  id: string;
  versions: string[];
  display: PluginDisplay;
}

/** ⑩ 闲置信息（后端 IdleInfo 契约，exposure 文档 §8.5）：后端只给结构化
 * 数据，前端按档位渲染"多久没用"人类文案（§11）——永不显示裸天数。 */
export interface IdleInfo {
  level: "fresh" | "normal" | "idle" | "stale" | "abandoned" | "never_used";
  value?: number | null;
  unit?:
    | "today"
    | "this_week"
    | "last_week"
    | "last_month"
    | "month"
    | "half_year"
    | "year"
    | "never_used"
    | "just_installed"
    | null;
}

/** plugin_list_with_usage 行（⑩⑤）：按 id join 到已安装列表的增量视图。 */
export interface PluginUsageView {
  id: string;
  enabled: boolean;
  active: boolean;
  installed_at: number;
  first_used_at: number | null;
  last_used_at: number | null;
  /** builtin 开发态插件不评估闲置（installed_at 无 lockfile 真源）。 */
  idle: IdleInfo | null;
  /** 筛选档位（后端裁决，嵌套：gt3m ⊂ gt1m ⊂ recent）。 */
  filter_key: "recent" | "gt1m" | "gt3m" | "gt6m" | null;
}

/** 多语言文本：BCP-47 标签 → 文案。渲染经 pickLocalized 回退链。 */
type LocalizedMap = Record<string, string>;

/** 资产引用（批 D 多模态，session-store AssetRef）：内容寻址 id + mime +
 * 原文件名（展示用）。字节在 `<数据根>/assets/`，消息只存引用。 */
export interface AssetRefDto {
  id: string;
  mime: string;
  name: string;
}

/** 消息附件部件（批 D，session-store ContentPart）：text = 正文段（用户
 * 消息正文在 content 字段，此变体当前不产生）；image/file 携带 AssetRef。 */
export type ContentPartDto =
  | { type: "text"; text: string }
  | { type: "image"; asset: AssetRefDto }
  | { type: "file"; asset: AssetRefDto };

/** 按路径读回的文本（后端 `file_read_text`，specs/08 批 1）：`truncated` 为真时
 * `text` 只是前缀、`bytes` 是文件真实大小；`binary` 为真时 `text` 为空串。 */
export interface FileTextDto {
  path: string;
  bytes: number;
  text: string;
  truncated: boolean;
  binary: boolean;
}
