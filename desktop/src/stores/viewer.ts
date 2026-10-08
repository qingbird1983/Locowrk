// 图片查看器状态（批 E，多模态稿 §9.3 大图查看器）：全屏浮层，缩略图 /
// 工具行产物点击进入。列表 = 当前转录全部图片（用户 parts + 工具产物，
// chat store viewerImages 顺序），左右切换在整条转录的图片间游走；资产
// 不在列表（极端：裁剪态）时单开。
import { defineStore } from "pinia";
import { assetRead } from "../lib/api";
import type { AssetRefDto } from "../types";
import { useChatStore } from "./chat";

/** 缩放边界（倍）。下限留够小图放大起点，上限 5×（再大像素糊无增益）。 */
const ZOOM_MIN = 0.5;
const ZOOM_MAX = 5;

export const useViewerStore = defineStore("viewer", {
  state: () => ({
    open: false,
    list: [] as AssetRefDto[],
    index: 0,
    zoom: 1,
    x: 0,
    y: 0,
    /** 当前图 data URL（asset_read；查看器自持，不入 MessageRow 模块缓存） */
    src: "",
    loading: false,
    error: "",
  }),
  getters: {
    current(state): AssetRefDto | null {
      return state.list[state.index] ?? null;
    },
    canPrev(state): boolean {
      return state.index > 0;
    },
    canNext(state): boolean {
      return state.index < state.list.length - 1;
    },
  },
  actions: {
    /** 按资产打开：转录图片列表里定位（点击缩略图/工具产物的唯一入口）。 */
    openAsset(asset: AssetRefDto) {
      const chat = useChatStore();
      const images = chat.viewerImages;
      let i = images.findIndex((a) => a.id === asset.id && a.mime === asset.mime);
      if (i < 0) {
        this.list = [asset];
        i = 0;
      } else {
        this.list = images;
      }
      this.index = i;
      this.open = true;
      void this.loadCurrent();
    },
    close() {
      this.open = false;
      this.src = "";
      this.error = "";
      this.resetView();
    },
    nav(delta: number) {
      const next = this.index + delta;
      if (next < 0 || next >= this.list.length) return;
      this.index = next;
      this.resetView();
      void this.loadCurrent();
    },
    resetView() {
      this.zoom = 1;
      this.x = 0;
      this.y = 0;
    },
    zoomBy(factor: number) {
      this.zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, this.zoom * factor));
      if (this.zoom === 1) {
        this.x = 0;
        this.y = 0;
      }
    },
    zoomSet(v: number) {
      this.zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, v));
      if (this.zoom === 1) {
        this.x = 0;
        this.y = 0;
      }
    },
    panBy(dx: number, dy: number) {
      this.x += dx;
      this.y += dy;
    },
    async loadCurrent() {
      const cur = this.current;
      if (!cur) return;
      this.loading = true;
      this.error = "";
      try {
        this.src = await assetRead(cur.id, cur.mime);
      } catch (e) {
        this.src = "";
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
  },
});
