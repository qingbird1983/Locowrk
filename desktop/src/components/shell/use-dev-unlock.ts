/**
 * 版本号连点解锁开发者选项（UI shell 文档 §3.2 / §7-16②，安卓式）：
 * 7 连点（2s 内重计）写 settings.json `developer.unlocked`；第 3 次起
 * toast 进度（同 key 原位替换）。
 */
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import { useToastStore } from "../../stores/toast";

const TAPS = 7;
const RESET_MS = 2000;

export function useDevUnlock() {
  const { t } = useI18n();
  const settings = useSettingsStore();
  const toast = useToastStore();
  let taps = 0;
  let lastTapAt = 0;

  function onTap() {
    if (settings.view?.developer.unlocked) return;
    const now = Date.now();
    if (now - lastTapAt > RESET_MS) taps = 0;
    lastTapAt = now;
    taps += 1;
    if (taps >= TAPS) {
      taps = 0;
      void settings.saveDeveloper({ unlocked: true });
      toast.push("ok", t("sidebar.devUnlocked"));
    } else if (taps >= 3) {
      toast.push(
        "ok",
        t("sidebar.devUnlockProgress", { n: TAPS - taps }),
        undefined,
        "dev-unlock",
      );
    }
  }

  return { onTap };
}
