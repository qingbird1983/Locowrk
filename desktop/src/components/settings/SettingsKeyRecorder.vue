<script setup lang="ts">
/**
 * 快捷键录制钮（UI shell 文档 §3.32·七 v3.6，用户裁定"快捷键可自定义"）：
 * 点击进入录制态 → 捕获下一组组合键 → 归一化为 global-hotkey 解析器接受的
 * 串（Ctrl/Alt/Shift/Meta 前缀 + 主键；键名取 e.code 映射，免受 Shift 产生的
 * 符号干扰）。纯修饰键按下继续等待；Esc 取消；**无修饰键且非 F 键的组合
 * 拒收继续等**（裸字母/数字做全局热键会吞掉系统输入）。提交由父级落
 * settings.json `shortcut`（即时重注册，占用冲突在后端如实报错）。
 */
import { onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{ commit: [accelerator: string] }>();

const { t } = useI18n();
const recording = ref(false);
const triggerRef = ref<HTMLButtonElement | null>(null);

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

/** e.code → 解析器键名（global-hotkey parse_key 接受单字母/数字/F 键/
 *  全词名与符号形）。 */
function keyName(code: string): string | null {
  const m = /^(?:Key([A-Z])|Digit([0-9])|F([1-9]|1[0-9]|2[0-4]))$/.exec(code);
  if (m) return m[1] ?? m[2] ?? m[3];
  const words = new Set([
    "Space",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Enter",
    "Tab",
    "Backspace",
    "Delete",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Insert",
  ]);
  if (words.has(code)) return code;
  const punct: Record<string, string> = {
    Comma: ",",
    Period: ".",
    Slash: "/",
    Backslash: "\\",
    BracketLeft: "[",
    BracketRight: "]",
    Semicolon: ";",
    Quote: "'",
    Minus: "-",
    Equal: "=",
    Backquote: "`",
  };
  return punct[code] ?? null;
}

function stop(): void {
  recording.value = false;
  window.removeEventListener("keydown", onKeydown, true);
}

function start(): void {
  if (recording.value) return;
  recording.value = true;
  window.addEventListener("keydown", onKeydown, true);
}

function onKeydown(e: KeyboardEvent): void {
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") {
    stop();
    return;
  }
  if (MODIFIER_CODES.has(e.code)) return; // 纯修饰键：等主键
  const key = keyName(e.code);
  if (!key) return; // 不认识的键：继续等
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Meta");
  // 全局热键安全底线：至少一个修饰键，或 F1–F24
  if (mods.length === 0 && !/^F([1-9]|1[0-9]|2[0-4])$/.test(key)) return;
  stop();
  emit("commit", [...mods, key].join("+"));
}

onBeforeUnmount(stop);

function onClick(): void {
  if (recording.value) return;
  start();
  triggerRef.value?.focus();
}
</script>

<template>
  <button
    ref="triggerRef"
    type="button"
    class="lw-btn px-3 py-1.5 font-mono text-xs"
    :class="recording ? 'text-ink-2' : 'text-ink-1'"
    :aria-label="t('settings.shortcut')"
    :aria-pressed="recording"
    @click="onClick"
  >
    {{ recording ? t("settings.shortcutRecording") : props.modelValue }}
  </button>
</template>
