// 复制反馈状态机（UI shell doc §3.7 v4.18「点击复制变对号」）：剪贴板钮
// 成功复制后字形换对勾 ≈1.6s 再还原；失败静默不亮对勾（不谎报）。
// 两种形态：组件内钮（消息复制 / 复制调用）走 copiedId ref；v-html 委托
// 钮（代码块头行复制）走 flashDom 的 DOM class——状态落 DOM 不进 Vue 树
// （对勾字形由 markdown 模板双 SVG 预置，.is-copied 只切显示）。
import { onScopeDispose, ref } from "vue";

/** 对勾驻留 ms：§6 动效档里没有适配档——要够"被注意到"、又不至于被
 * 误读成新的常驻状态，1600ms 实测定档。 */
const FEEDBACK_MS = 1600;

export function useCopyFeedback() {
  /** 正在亮对勾的钮 id（组件内多钮用 id 区分，单钮给常量即可）。 */
  const copiedId = ref<string | null>(null);
  /** v-html 委托钮的计时表（Map 随组件作用域 dispose 一起清，弱引用无必要）。 */
  const domTimers = new Map<Element, ReturnType<typeof setTimeout>>();
  let timer: ReturnType<typeof setTimeout> | undefined;

  function markCopied(id: string) {
    copiedId.value = id;
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      copiedId.value = null;
      timer = undefined;
    }, FEEDBACK_MS);
  }

  /** 写剪贴板 + 成功才亮对勾。返回是否成功（调用方可据此补报错，缺省静默）。 */
  async function copyWithFeedback(
    text: string,
    id = "self",
  ): Promise<boolean> {
    try {
      await navigator.clipboard.writeText(text);
      markCopied(id);
      return true;
    } catch {
      return false;
    }
  }

  /** v-html 委托钮：对勾 = .copy-btn.is-copied（重复点击重置计时）。 */
  function flashDom(btn: Element) {
    btn.classList.add("is-copied");
    const prev = domTimers.get(btn);
    if (prev !== undefined) clearTimeout(prev);
    domTimers.set(
      btn,
      setTimeout(() => {
        btn.classList.remove("is-copied");
        domTimers.delete(btn);
      }, FEEDBACK_MS),
    );
  }

  /**
   * v-html 代码块头行的复制钮（事件在容器上委托，§3.7）。此前
   * `MessageRow.vue::onBodyClick` 与 `ToolDetailView.vue::onCodeClick` **函数体逐字
   * 相同**地各写一遍「closest('.copy-btn') → 找宿主 .code-block → 取 `pre code`
   * 文本 → 写剪贴板 → flashDom」（审计 C11：两者都已经在用本文件，只有这段没下沉）。
   * 形参收 `Event` 而不是 `MouseEvent`：模板 `@click` 传的是 MouseEvent，而这段只用
   * `target`——收窄到真正依赖的那一层。
   */
  async function copyDelegatedCode(e: Event) {
    const target = (e.target as HTMLElement).closest?.(".copy-btn");
    if (!target) return;
    const fig = target.closest(".code-block");
    const code = fig?.querySelector("pre code")?.textContent ?? "";
    try {
      await navigator.clipboard.writeText(code);
      flashDom(target);
    } catch {
      /* 剪贴板不可用不打断 */
    }
  }

  onScopeDispose(() => {
    if (timer !== undefined) clearTimeout(timer);
    for (const t of domTimers.values()) clearTimeout(t);
    domTimers.clear();
  });

  return { copiedId, copyWithFeedback, flashDom, copyDelegatedCode };
}
