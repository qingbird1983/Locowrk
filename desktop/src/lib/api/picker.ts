/** **原生对话框**（⑱，UI shell 文档 §7 规则 8）：选目录、多选文件、选保存路径。
 * 三条都以 pick 开头、返回的都是文件系统路径（取消 = null），Rust 侧由同一份 dialog 插件封装。 */

import { invoke } from "@tauri-apps/api/core";
/** 原生目录选择对话框（⑱，UI shell 文档 §7 规则 8）：Rust 侧
 * tauri-plugin-dialog 封装；取消/关窗返回 null。 */
export function pickDirectory(): Promise<string | null> {
  return invoke("pick_directory");
}

/** 原生多选文件（§3.11 v3.9 附件按钮 v1）：路径引用，不上传内容。 */
export function pickFiles(): Promise<string[]> {
  return invoke("pick_files");
}

/** 原生存储对话框（v2.49 会话导出用）：defaultName 建议文件名；
 * 取消/关窗返回 null。 */
export function pickSavePath(defaultName: string): Promise<string | null> {
  return invoke("pick_save_path", { defaultName });
}
