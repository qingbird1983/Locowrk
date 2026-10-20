// S4（specs/11 刀④）AC-4 的设置面判据：
// `settings_surface_has_no_user_facing_number_inputs` —— 普通分区模板里
// 不许再有 `type="number"`；开发者分区仍需承载数值输入（正对照，证明
// 扫描面与判定不是空转）。
//
// 为什么扫模板而不是挂组件：数字输入是**模板形态**，挂载十个分区还要
// 造十份投影与假 IPC，扫源码直接对着"用户会看到的控件类型"判；新分区
// 文件落进 `components/settings/` 就自动进扫描面，不需要维护清单。
// 扫描面下限 + 关键文件点名 = 防"路径写错扫到零个文件"的假绿。
import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve, sep } from "node:path";

// 为什么用 `process.cwd()` 而不是 `import.meta.url`：happy-dom 环境下
// `import.meta.url` 不是 `file:` scheme，`fileURLToPath` 会抛
// "The URL must be of scheme file"（`ui-font.spec.ts` 头注记过同一坑）。
// vitest 以 desktop 包根为 cwd 运行；路径写错时 readdirSync 直接抛错，
// 不会退化成"扫到零个文件"的假绿（另有扫描面下限兜底）。
const SETTINGS_DIR = resolve(process.cwd(), "src/components/settings");
const NUMBER_INPUT = /type\s*=\s*["']number["']/;

const vueFiles = (): string[] =>
  readdirSync(SETTINGS_DIR, { recursive: true })
    .map((p) => String(p))
    .filter((p) => p.endsWith(".vue"));

/** 开发者分区 = 主文件 + `developer/` 子块（S4 刀④ 拆分后的新家）。 */
const isDeveloper = (rel: string): boolean =>
  rel === "DeveloperSection.vue" || rel.startsWith(`developer${sep}`);

describe("设置面数值输入的归类", () => {
  it("settings_surface_has_no_user_facing_number_inputs", () => {
    const files = vueFiles();
    // 扫描面下限：当前设置组件树 ≥ 10 个 .vue；少于这个数说明路径/过滤错了。
    expect(files.length).toBeGreaterThanOrEqual(10);
    // 关键文件点名：这些必须真的在扫描面里（开发者分区单独做正对照）。
    for (const rel of [
      "AppearanceSection.vue",
      "ChatSection.vue",
      "ContextSection.vue",
      "ToolsSection.vue",
      "SettingsPage.vue",
      join("model", "ModelPanes.vue"),
      join("model", "ProviderSetupDialog.vue"),
    ]) {
      expect(files, `${rel} 必须在扫描面里`).toContain(rel);
    }
    const offenders = files
      .filter((rel) => !isDeveloper(rel))
      .filter((rel) => NUMBER_INPUT.test(readFileSync(join(SETTINGS_DIR, rel), "utf8")));
    expect(offenders, "普通分区出现数字输入（应改成开关/预置或移入开发者）").toEqual([]);
  });

  it("开发者分区仍承载数值输入（正对照：判定不是空转）", () => {
    const devFiles = vueFiles().filter(isDeveloper);
    // 主文件 + 工具参数 / 插件自动停用 / 模型覆盖三个子块；少于 4 个说明
    // 路径分隔符或 isDeveloper 分类写错，正对照已在空转。
    expect(devFiles.length).toBeGreaterThanOrEqual(4);
    const sources = devFiles
      .map((rel) => readFileSync(join(SETTINGS_DIR, rel), "utf8"))
      .join("\n");
    expect(sources).toMatch(NUMBER_INPUT);
  });
});
