// Toast 状态（UI shell 文档 §3.22）：右下栈，"刚发生的事"（导出完成、
// 删除完成、解锁进度）——需要用户处理的不许只发 toast。
import { defineStore } from "pinia";

export type ToastTone = "ok" | "warn" | "danger";

export interface ToastItem {
  id: number;
  /** 同 key 再推 = 原位替换（进度型 toast 不堆叠，如解锁倒数）。 */
  key?: string;
  tone: ToastTone;
  title: string;
  body?: string;
}

let nextId = 1;

export const useToastStore = defineStore("toast", {
  state: () => ({ items: [] as ToastItem[] }),
  getters: {
    /** §3.22 同屏最多 3 条，其余排队（计时器随进入视口启动）。 */
    visible: (s) => s.items.slice(0, 3),
  },
  actions: {
    push(tone: ToastTone, title: string, body?: string, key?: string): number {
      if (key) {
        const at = this.items.findIndex((t) => t.key === key);
        if (at !== -1) {
          const item = this.items[at];
          item.tone = tone;
          item.title = title;
          item.body = body;
          return item.id;
        }
      }
      const id = nextId++;
      this.items.push({ id, key, tone, title, body });
      return id;
    },
    dismiss(id: number) {
      this.items = this.items.filter((t) => t.id !== id);
    },
  },
});
