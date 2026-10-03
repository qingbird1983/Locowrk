/**
 * 欢迎页问候（UI shell 文档 §3.14；v2.61 收敛）：本模块只管时段四态。
 * 项目名不在幻觉里索引——与对话框下面工作区 chip 共用 chat.currentWorkspace
 * 同源真源（v2.61 用户裁定「把欢迎语项目名跟对话框下面的项目名关联」，
 * null = 未选项目 → greetingBare 不带名问候）。曾有的「最近活动会话 /
 * 第一个 fsRoot」回落随同源化退役。
 */
export type GreetingKey = "morning" | "afternoon" | "evening" | "lateNight";

/** 上午好 05:00–12:00 / 下午好 12:00–18:00 / 晚上好 18:00–23:00 / 夜深了 其余。 */
export function greetingKey(d: Date = new Date()): GreetingKey {
  const h = d.getHours();
  if (h >= 5 && h < 12) return "morning";
  if (h >= 12 && h < 18) return "afternoon";
  if (h >= 18 && h < 23) return "evening";
  return "lateNight";
}
