/**
 * 记忆树侧的文案映射（队列 Q14 第三刀立）。
 *
 * 为什么单开一个文件而不是留在组件里：章节 `source` 到人类文案这条映射，画布的
 * **小节头**（`board/StreamPanel.vue`）与「目录」档的章节列表（`TreeSection.vue`）
 * 都要用。写在组件里 = 两份字典，字典这种东西一旦分两处就必然出现「一处补了
 * placeholder、另一处显裸值」。`tr` 注入而不是 import i18n：与
 * `lib/format.ts` 的 `relativeTimeLabel` 同一条纪律——纯逻辑可以直接被用例驱动。
 */

/** 章节来源三档（后端 `ChapterSource` 的 serde 形：recorder / rule / placeholder）。 */
export function treeSourceLabel(
  source: string,
  tr: (key: string) => string,
): string {
  return tr(
    source === "recorder"
      ? "memory.sourceMemoryManager"
      : source === "rule"
        ? "memory.sourceRule"
        : "memory.sourcePlaceholder",
  );
}

/** 该来源是不是降级产物（规则切分 / 占位）⇒ 徽章走 warn 档（§二⑧ 状态色）。 */
export function isDegradedSource(source: string): boolean {
  return source !== "recorder";
}

/** 会话 id 的人类替身（投影在、会话已删时用它当标题）。8 位这个数是呈现约定，
 * 不散进各组件——同一把尺写两处就必然出现第三处。 */
export function shortSessionId(id: string): string {
  return id.slice(0, 8);
}

/** 会话标题（旁车 / 会话元数据优先，取不到 = 回落 id 前缀）。 */
export function sessionTitleOf(title: string | undefined | null, sessionId: string): string {
  const t = title?.trim();
  return t ? t : shortSessionId(sessionId);
}
