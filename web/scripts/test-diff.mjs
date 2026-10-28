import { build } from "esbuild";
import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const dir = await mkdtemp(join(tmpdir(), "locowrk-diff-test-"));
const outfile = join(dir, "diff.test.mjs");
try {
  await build({
    entryPoints: ["tests/diff.test.ts"],
    bundle: true,
    platform: "node",
    format: "esm",
    target: "es2020",
    outfile,
    logLevel: "warning",
  });
  const code = await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["--test", outfile], { stdio: "inherit" });
    child.on("error", reject);
    child.on("exit", (value) => resolve(value ?? 1));
  });
  process.exitCode = Number(code);
} finally {
  await rm(dir, { recursive: true, force: true });
}
