<script setup lang="ts">
/**
 * 行内语言/文件类型图标（彩色，material-icon-theme 素材）：给一个文件路径，
 * 按 `lib/file-icons` 的认名规则取 `public/file-icons/<name>.svg`。
 *
 * 素材是 1146 个静态 SVG，前端认不出"这个名字有没有素材"，所以 404 时按
 * `@error` 退回 `document`（该素材恒在）；别把 src 直接写死在调用方，否则
 * 每个用点都要各写一遍兜底。
 */
import { ref, watch } from "vue";
import { fileIconSrc, resolveFileIcon } from "../../lib/file-icons";

const props = defineProps<{
  /** 文件路径（认的是末段文件名；空串不渲染）。 */
  path: string;
  size?: number;
}>();

const src = ref("");
watch(
  () => props.path,
  (p) => {
    src.value = p ? fileIconSrc(resolveFileIcon(p)) : "";
  },
  { immediate: true },
);

function onError() {
  src.value = fileIconSrc("document");
}
</script>

<template>
  <img
    v-if="src"
    class="ficon"
    :src="src"
    alt=""
    :width="size ?? 14"
    :height="size ?? 14"
    draggable="false"
    aria-hidden="true"
    @error="onError"
  />
</template>

<style scoped>
.ficon {
  display: inline-block;
  flex: none;
  vertical-align: -2px; /* 光学修正(孤值2,§二⑬)：14px 图标压到 12.5px 文字的光学中轴 */
}
</style>
