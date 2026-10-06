// 全局快捷键（UI shell doc §3.25 落地表）：Ctrl/⌘+B 折叠侧栏、Ctrl/⌘+J
// 折叠工作面板、Ctrl/⌘+N 新会话、Ctrl/⌘+, 打开设置、Ctrl/⌘+[ / ] 会话
// 前进/后退（沿侧栏顺序 = sessions 的 mtime 序）。⌘K 在 CommandPalette、
// Ctrl+Shift+L 在 Rust 全局注册，此处不重复。§3.25 冲突规则：与输入框
// 冲突时输入框优先——本表全是带修饰键的组合；Esc 与 ↑ 的输入框语义在
// Composer 内处理，不经这里。
import { onBeforeUnmount, onMounted } from "vue";
import { useChatStore } from "../../stores/chat";
import { useShellStore } from "../../stores/shell";
import { useWorkPanelStore } from "../../stores/work-panel";

export function useGlobalShortcuts() {
  const chat = useChatStore();
  const shell = useShellStore();
  const workPanel = useWorkPanelStore();

  /** 会话前进/后退：沿侧栏顺序（sessions 已是 mtime 降序）；运行中禁切。 */
  function stepSession(delta: number) {
    if (shell.destination !== "chat" || chat.running) return;
    const list = chat.sessions;
    if (list.length === 0) return;
    const idx = list.findIndex((s) => s.id === chat.sessionId);
    const next = idx === -1 ? list[0] : list[idx + delta];
    if (next && next.id !== chat.sessionId) void chat.switchSession(next.id);
  }

  function onKeydown(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
    switch (e.key.toLowerCase()) {
      case "b":
        e.preventDefault();
        shell.toggleSidebar();
        break;
      case "j":
        e.preventDefault();
        // §3.14 v3.9（issue #10）：首页空态不可展开工作面板（页面语义）。
        // §3.40④（v2.111）：任何宽度可开——overlay 档互斥由 setOpen 收口
        if (shell.destination === "chat" && chat.items.length > 0) {
          workPanel.toggle();
        }
        break;
      case "n":
        e.preventDefault();
        chat.newSession();
        shell.setDestination("chat");
        break;
      case ",":
        e.preventDefault();
        shell.openSettings();
        break;
      case "[":
        e.preventDefault();
        stepSession(-1);
        break;
      case "]":
        e.preventDefault();
        stepSession(1);
        break;
    }
  }

  onMounted(() => document.addEventListener("keydown", onKeydown));
  onBeforeUnmount(() => document.removeEventListener("keydown", onKeydown));
}
