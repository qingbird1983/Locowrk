// stores/chat/attachments.ts —— 附件预览条（批 D §9.2，前端态不入会话日志）。
// 三条入口同进（按钮选择 / 粘贴 / 拖拽）⇒ 一套上传、一个状态机（uploading → ready | error），
// 发送前由 settleAttachments 收口（有失败就不发，留条如实展示错误文案）。
import { assetPut, assetPutPath } from "../../lib/api";
import { nextAttachmentKey } from "./ids";
import type { ChatState } from "./state";
import type { AttachmentItem } from "./types";
import type { ContentPartDto } from "../../types";

/** 内存文件上传（粘贴/拖拽）。`localUrl` = 本地 objectURL 预览（图片）。
 * 失败留条可移除重加，错误文案来自后端校验，如实展示。 */
export async function addAttachmentFile(st: ChatState, file: File) {
  const kind = file.type.startsWith("image/") ? "image" : "file";
  const item: AttachmentItem = {
    key: nextAttachmentKey(),
    name: file.name || "clipboard",
    mime: file.type || "application/octet-stream",
    kind,
    status: "uploading",
    ...(kind === "image" ? { localUrl: URL.createObjectURL(file) } : {}),
  };
  st.attachments.push(item);
  try {
    const buf = await file.arrayBuffer();
    const bytes = new Uint8Array(buf);
    let bin = "";
    const CH = 0x8000;
    for (let i = 0; i < bytes.length; i += CH) {
      bin += String.fromCharCode(...bytes.subarray(i, i + CH));
    }
    const asset = await assetPut(btoa(bin), item.mime, item.name);
    if (item.status === "uploading") {
      item.asset = asset;
      item.mime = asset.mime;
      item.status = "ready";
    }
  } catch (err) {
    item.status = "error";
    item.error = String(err);
  }
}

/** 已选文件路径上传（asset_put_path，零 IPC 大载荷）。 */
export async function addAttachmentPath(st: ChatState, path: string) {
  const item: AttachmentItem = {
    key: nextAttachmentKey(),
    name: path.split(/[\\/]/).pop() || path,
    mime: "",
    kind: "file",
    status: "uploading",
  };
  st.attachments.push(item);
  try {
    const asset = await assetPutPath(path);
    item.asset = asset;
    item.mime = asset.mime;
    item.kind = asset.mime.startsWith("image/") ? "image" : "file";
    if (item.kind === "image") {
      // 本地预览：读一次 data URL（小图，无 objectURL 可用）
      const { assetRead } = await import("../../lib/api");
      item.localUrl = await assetRead(asset.id, asset.mime);
    }
    item.status = "ready";
  } catch (err) {
    item.status = "error";
    item.error = String(err);
  }
}

export function removeAttachment(st: ChatState, key: number) {
  const idx = st.attachments.findIndex((a) => a.key === key);
  if (idx >= 0) {
    const [it] = st.attachments.splice(idx, 1);
    if (it?.localUrl?.startsWith("blob:")) URL.revokeObjectURL(it.localUrl);
  }
}

/** 「仅发送文字」门控动作（§9.5）：剥掉图片附件，PDF 与文本保留。 */
export function dropImageAttachments(st: ChatState) {
  for (const it of [...st.attachments]) {
    if (it.kind === "image") removeAttachment(st, it.key);
  }
}

/** 发送前收口：全部就绪才发（上传中等待完成；有失败 = 不发，留条
 * 如实展示）。返回 parts（空表 = 纯文本）。 */
export async function settleAttachments(
  st: ChatState,
): Promise<ContentPartDto[] | null> {
  const deadline = Date.now() + 30_000;
  while (st.attachments.some((a) => a.status === "uploading")) {
    if (Date.now() > deadline) break;
    await new Promise((r) => setTimeout(r, 80));
  }
  if (st.attachments.some((a) => a.status === "error")) return null;
  return st.attachments
    .filter((a) => a.asset)
    .map((a) =>
      a.kind === "image"
        ? ({ type: "image", asset: a.asset } as ContentPartDto)
        : ({ type: "file", asset: a.asset } as ContentPartDto),
    );
}
