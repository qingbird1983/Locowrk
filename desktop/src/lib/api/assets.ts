/** **多模态附件面**（批 D，多模态稿 §6/§9）：上传（内存字节 / 已选文件路径）、读取缩略图、
 * 在文件夹中显示、另存为、手动孤儿回收 —— 六条都以 asset 开头，都在同一命令族上。
 *
 * 其中手动回收那条是**预留·前端零消费者**（原注释的裁定凭证跟着搬）：它留着是因为只删包装
 * 会让命令三向闭合闸当场判红，要退役必须与后端命令同批。 */

import { invoke } from "@tauri-apps/api/core";
import type { AssetRefDto } from "../../types";
// ---- 多模态附件（批 D，多模态稿 §6/§9）----

/** 上传内存文件（粘贴/拖拽）：base64 + mime + 原名 → AssetRef。后端做
 * 魔数白名单/体积/单边校验，失败 reject 带面向用户的文案。 */
export function assetPut(dataBase64: string, mime: string, name: string): Promise<AssetRefDto> {
  return invoke("asset_put", { dataBase64, mime, name });
}

/** 上传已选文件（原生对话框路径，零 IPC 大载荷）→ AssetRef。 */
export function assetPutPath(path: string): Promise<AssetRefDto> {
  return invoke("asset_put_path", { path });
}

/** 缩略图/查看读取：返回 data URL（AssetStore 后端带 LRU）。 */
export function assetRead(id: string, mime: string): Promise<string> {
  return invoke("asset_read", { id, mime });
}

/** 在文件夹中显示资产原文件（大图查看器落地前的系统打开器顶位）。 */
export function assetReveal(id: string, mime: string): Promise<void> {
  return invoke("asset_reveal", { id, mime });
}

/** 查看器「另存为」（批 E）：原生保存对话框 + 资产文件复制，取消 = null。 */
export function assetSaveAs(
  id: string,
  mime: string,
  defaultName: string,
): Promise<string | null> {
  return invoke("asset_save_as", { id, mime, defaultName });
}

/** 手动孤儿资产 GC（走查/开发者）：按 gcThresholdDays 判定，返回计数。
 *
 * **预留·前端零消费者**（审计 C3 / 用户 2026-10-10 裁定保留）：调用入口在
 * 后端命令 `asset_gc`（启动 + 24h 自动循环之外的手动档），包装留着给走查与
 * 将来的开发者钮用。要退役它必须连 Rust 命令一起拔——只删包装会让
 * `check-commands.mjs` 的「定义 = 注册 = 前端引用」三向闭合当场判红。 */
export function assetGc(): Promise<{ scanned: number; removed: number }> {
  return invoke("asset_gc");
}
