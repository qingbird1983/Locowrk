/** **定时任务目的地页**（v2.57 §3.35）：全部待触发调度与删除一条。两条都读那个调度库，
 * 调度库缺席时一条给空表、一条给否（幂等）。 */

import { invoke } from "@tauri-apps/api/core";
import type { SessionSchedule } from "../../types";
// ---- 定时任务目的地页（v2.57 §3.35）：⑭ 调度库的页面命令 ----

/** 全部待触发调度（新→旧）；调度库缺席 = 空表。 */
export function scheduleList(): Promise<SessionSchedule[]> {
  return invoke("schedule_list");
}

/** 删除一条调度（返回是否确有此行；已触发被引擎删掉 = false，幂等）。 */
export function scheduleDelete(id: string): Promise<boolean> {
  return invoke("schedule_delete", { id });
}
