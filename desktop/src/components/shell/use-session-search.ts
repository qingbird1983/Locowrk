// 会话正文搜索（v2.49 §3.6）：侧栏搜索框防抖 250ms → session_search，
// 序号守卫竞态（慢响应不覆盖新结果）；query 空 = 立即清空。
import { ref, watch, type Ref } from "vue";
import { sessionSearch } from "../../lib/api";
import type { SessionSearchHit } from "../../types";

export function useSessionSearch(query: Ref<string>) {
  const contentHits = ref<SessionSearchHit[]>([]);
  let seq = 0;
  let timer: number | undefined;

  watch(query, (q) => {
    window.clearTimeout(timer);
    const text = q.trim();
    if (!text) {
      contentHits.value = [];
      return;
    }
    timer = window.setTimeout(async () => {
      const at = ++seq;
      try {
        const hits = await sessionSearch(text, 20);
        if (at === seq) contentHits.value = hits;
      } catch {
        if (at === seq) contentHits.value = [];
      }
    }, 250);
  });

  return { contentHits };
}
