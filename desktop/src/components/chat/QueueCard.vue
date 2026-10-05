<script setup lang="ts">
/**
 * 追加消息排队卡（UI shell doc §3.39②，v4.11）——运行中发送的消息排队
 * 等待，坞内贴合柱面（卡面由 Composer.dock-cards 统一持有）。版式按用户
 * 参考图（ZCode 实卡）：头行 = 「N 条排队中 ⌄」（点按折叠）+ 右侧
 * [继续]（仅暂停态）+ 垃圾桶（清空全部）+ 深色圆形 ↑（立即发送队首，
 * 抢占语义）；下方每条排队消息一行预览（单行省略），行悬停显 ✕ 删除
 * 单条（静态观感 = 参考图，动作不可见）。
 *
 * 语义（后端零新链路）：队列为前端 UI 态（chat store，不进会话日志——
 * 未发送的消息不是会话事实）；自动派发 = 每轮 turn_done 后取队首续跑
 * （一次一条）；立即发送 = 摘出 → 停止当前轮 → 关卡后重发（抢占）；
 * 停止/出错后暂停自动派发（queueDraining=false），手动「继续」恢复。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";

const { t } = useI18n();
const chat = useChatStore();

const open = ref(true);
const rows = computed(() =>
  chat.queue.map((q) => ({
    id: q.id,
    text: q.text,
    firstLine:
      q.text
        .split("\n")
        .map((l) => l.trim())
        .find((l) => l) ?? "",
  })),
);

function remove(id: number) {
  chat.removeQueued(id);
}
function clearAll() {
  chat.clearQueue();
}
function sendHead() {
  const head = chat.queue[0];
  if (head) void chat.sendQueuedNow(head.id);
}
function resume() {
  chat.resumeDrain();
}
</script>

<template>
  <section class="queue-card" role="region" :aria-label="t('queue.cardTitle')">
    <div class="queue-card__head">
      <button
        class="queue-card__toggle"
        type="button"
        :aria-expanded="open"
        @click="open = !open"
      >
        <span class="queue-card__title">
          {{ t("queue.cardTitle", { n: chat.queue.length }) }}
        </span>
        <Icon
          :name="open ? 'chevronDown' : 'chevronRight'"
          :size="12"
          class="queue-card__chev"
        />
      </button>
      <div class="queue-card__acts">
        <button
          v-if="!chat.queueDraining"
          class="queue-card__resume"
          type="button"
          @click="resume"
        >
          {{ t("queue.resume") }}
        </button>
        <HoverTip :text="t('queue.clearAll')">
          <button
            class="queue-card__iconbtn"
            type="button"
            :aria-label="t('queue.clearAll')"
            @click="clearAll"
          >
            <Icon name="trash" :size="15" />
          </button>
        </HoverTip>
        <HoverTip :text="t('queue.sendNowHint')">
          <button
            class="queue-card__go"
            type="button"
            :aria-label="t('queue.sendNow')"
            @click="sendHead"
          >
            <Icon name="sendUp" :size="14" />
          </button>
        </HoverTip>
      </div>
    </div>
    <ul v-show="open" class="queue-card__rows">
      <li v-for="r in rows" :key="r.id" class="queue-card__row" :title="r.text">
        <span class="queue-card__line">{{
          r.firstLine || t("queue.emptyPreview")
        }}</span>
        <button
          class="queue-card__rowdel"
          type="button"
          :aria-label="t('queue.remove')"
          @click="remove(r.id)"
        >
          <Icon name="close" :size="12" />
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
/* 卡面（底色/圆角/暗影）由 Composer.dock-cards 容器统一持有（贴合柱面
 * 语义），本组件只管内容版式。 */
.queue-card {
  padding: var(--lw-gap-8) var(--lw-gap-10) var(--lw-gap-6) var(--lw-gap-14);
}

.queue-card__head {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
}
.queue-card__toggle {
  display: inline-flex;
  align-items: center;
  gap: 5px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值5,§二⑬) */
  height: 26px;
  padding: 0 var(--lw-gap-6);
  border-radius: var(--lw-r-sm);
  transition: background var(--dur-fast) var(--ease-standard);
}
.queue-card__toggle:hover {
  background: var(--lw-wash-2);
}
.queue-card__title {
  font-size: var(--lw-fs-12half);
  font-weight: 600;
  color: var(--lw-ink-1);
}
.queue-card__chev {
  color: var(--lw-ink-3);
}

.queue-card__acts {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: var(--lw-gap-4);
}
.queue-card__resume {
  height: 24px;
  padding: 0 var(--lw-gap-10);
  border-radius: var(--lw-r-sm);
  background: var(--lw-wash-3);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-1);
}
.queue-card__resume:hover {
  background: color-mix(in oklab, var(--lw-ink-1) 10%, transparent);
}
.queue-card__iconbtn {
  width: 26px;
  height: 26px;
  border-radius: var(--lw-r-sm);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--lw-ink-3);
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.queue-card__iconbtn:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
/* 深色圆形 ↑（参考图）：立即发送队首（抢占） */
.queue-card__go {
  width: 28px;
  height: 28px;
  border-radius: var(--lw-r-full);
  background: var(--lw-accent);
  color: var(--lw-accent-ink);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background var(--dur-fast) var(--ease-standard),
    transform var(--dur-instant) var(--ease-standard);
}
.queue-card__go:hover {
  background: var(--lw-accent-hover);
}
.queue-card__go:active {
  transform: scale(0.96);
}

.queue-card__rows {
  list-style: none;
  margin: 0;
  padding: var(--lw-gap-2) 0 var(--lw-gap-4);
  display: flex;
  flex-direction: column;
}
.queue-card__row {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  padding: 3px var(--lw-gap-4) 3px var(--lw-gap-6); /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值3,§二⑬) */
  border-radius: var(--lw-r-sm);
}
.queue-card__row:hover {
  background: var(--lw-wash-1);
}
.queue-card__line {
  flex: 1;
  min-width: 0;
  font-size: var(--lw-fs-12half);
  line-height: 1.5;
  color: var(--lw-ink-2);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.queue-card__rowdel {
  flex: 0 0 auto;
  width: 20px;
  height: 20px;
  border-radius: var(--lw-r-sm);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--lw-ink-3);
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-standard),
    background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.queue-card__rowdel:hover {
  background: color-mix(in oklab, var(--lw-danger) 12%, transparent);
  color: var(--lw-danger);
}
.queue-card__row:hover .queue-card__rowdel,
.queue-card__row:focus-within .queue-card__rowdel {
  opacity: 1;
}
</style>
