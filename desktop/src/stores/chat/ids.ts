// stores/chat/ids.ts —— 条目 id 与附件 key 的**唯一**发号处（T22 刀二随投影面下沉）。
// 原来这两颗计数器是 `stores/chat.ts` 的模块级 `let`，拆片后多片都要取号 ⇒ 必须有一个
// 公共出处（同类只许一种形态；把 `nextId++` 抄进三个文件 = 三个真相，早晚会漂）。
// 语义与原实现一致：模块级单例、从 1 起、只增不减（测试里断言的是"同序分配"，不是具体值）。
let seq = 1;
let attachmentSeq = 1;

export const nextId = (): number => seq++;
export const nextAttachmentKey = (): number => attachmentSeq++;
