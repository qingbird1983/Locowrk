<script setup lang="ts">
/**
 * 图片页签（specs/08 批 2；标准 §二⑱）：点缩略图 / 消息里的图在右栏就地看大图——只读。
 * 数据面两条：带 `assetId` 的走 `asset_read`（内容寻址资产的 data URL，与全屏查看器同源）；
 * 只有路径的走 `preview-url.ts` 的 lwpreview 协议（本地文件服务）。
 * 「全屏查看」把资产交给既有 `ViewerOverlay`（viewer store），本组件不新写浮层。
 * 头行几何在 style.css 的 `.lw-tabhead`（三类产物页签共用）。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import HoverTip from "../settings/HoverTip.vue";
import { assetRead } from "../../lib/api";
import { previewUrlFor } from "../../lib/preview-url";
import { useViewerStore } from "../../stores/viewer";

const props = defineProps<{
  path: string;
  assetId?: string;
  assetMime?: string;
}>();
const { t } = useI18n();
const viewer = useViewerStore();

const src = ref("");
const loading = ref(true);
const error = ref("");

const fileName = computed(() => props.path.split(/[\\/]/).pop() ?? props.path);

/** 全屏查看：交给既有查看器——资产在转录图片列表里就跟着左右切换，不在就单开一张。 */
function openFullscreen() {
  if (!props.assetId) return;
  viewer.openAsset({
    id: props.assetId,
    mime: props.assetMime ?? "image/png",
    name: fileName.value,
  });
}

async function load() {
  loading.value = true;
  error.value = "";
  src.value = "";
  try {
    src.value = props.assetId
      ? await assetRead(props.assetId, props.assetMime ?? "image/png")
      : previewUrlFor(props.path);
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

watch(() => [props.path, props.assetId], () => void load(), { immediate: true });
</script>

<template>
  <div class="scroll-y min-h-0 flex-1 px-3 py-2">
    <div class="lw-tabhead">
      <HoverTip class="min-w-0" :text="path" :label="fileName">
        <span class="lw-tabhead__path">{{ fileName }}</span>
      </HoverTip>
      <span class="lw-tabhead__meta">
        <button v-if="assetId" type="button" class="itab__full" @click="openFullscreen">
          {{ t("panel.imageOpenFull") }}
        </button>
      </span>
    </div>

    <p v-if="loading" class="itab__note">{{ t("panel.imageLoading") }}</p>
    <p v-else-if="error" class="itab__note">{{ error }}</p>
    <img v-else class="itab__img" :src="src" :alt="fileName" />
  </div>
</template>

<style scoped>
.itab__note {
  padding: var(--lw-gap-12) var(--lw-gap-4);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-4);
}
/* 画布 = 井口径（标准 §二⑱）：图自适应宽度、不裁切，底与代码块同色 */
.itab__img {
  display: block;
  max-width: 100%;
  margin-top: var(--lw-gap-8);
  border-radius: var(--lw-r-md);
  background: var(--lw-bg-inset);
}
.itab__full {
  font-family: inherit;
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-3);
}
.itab__full:hover {
  color: var(--lw-ink-1);
}
</style>
