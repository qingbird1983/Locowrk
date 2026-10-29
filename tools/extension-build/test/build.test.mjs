// Tests for tools/extension-build/build.mjs. Run with: npm test
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const tool = resolve(here, "..", "build.mjs");
const fixture = resolve(here, "..", "..", "..", "fixtures", "extensions", "orchestrator-sample");
const scratch = mkdtempSync(join(tmpdir(), "locowrk-build-test-"));

function run(args) {
  return spawnSync(process.execPath, [tool, ...args], { encoding: "utf8" });
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

describe("extension-build", () => {
  it("builds a package with a matching build record", () => {
    const out = join(scratch, "built");
    const result = run(["--src", fixture, "--out", out]);
    assert.equal(result.status, 0, result.stderr);
    for (const file of ["bundle.mjs", "plugin.json", "build.json"]) {
      assert.ok(existsSync(join(out, file)), `${file} missing`);
    }
    const manifest = JSON.parse(readFileSync(join(out, "plugin.json"), "utf8"));
    assert.equal(manifest.name, "orchestrator-sample");
    assert.equal(manifest.protocol, 1);
    assert.equal(manifest.entrypoint, "bundle.mjs");
    assert.deepEqual(manifest.capabilities, []);

    const record = JSON.parse(readFileSync(join(out, "build.json"), "utf8"));
    assert.equal(record.sha256, sha256(join(out, "bundle.mjs")));
    assert.ok(record.esbuild.length > 0);

    const bundle = readFileSync(join(out, "bundle.mjs"), "utf8");
    assert.ok(bundle.includes("locowrk-shim/1"), "shim not bundled");
    assert.ok(bundle.includes("sample_echo"), "extension source not bundled");
    assert.ok(!bundle.includes("node_modules"), "bundle references node_modules");
  });

  it("is reproducible", () => {
    const a = join(scratch, "repro-a");
    const b = join(scratch, "repro-b");
    assert.equal(run(["--src", fixture, "--out", a]).status, 0);
    assert.equal(run(["--src", fixture, "--out", b]).status, 0);
    assert.equal(sha256(join(a, "bundle.mjs")), sha256(join(b, "bundle.mjs")));
    assert.equal(readFileSync(join(a, "plugin.json"), "utf8"), readFileSync(join(b, "plugin.json"), "utf8"));
    assert.equal(readFileSync(join(a, "build.json"), "utf8"), readFileSync(join(b, "build.json"), "utf8"));
  });

  it("--check accepts a matching build and rejects a changed one", () => {
    const out = join(scratch, "checked");
    assert.equal(run(["--src", fixture, "--out", out]).status, 0);
    assert.equal(run(["--src", fixture, "--out", out, "--check"]).status, 0);
    writeFileSync(join(out, "bundle.mjs"), "// tampered\n");
    const tampered = run(["--src", fixture, "--out", out, "--check"]);
    assert.notEqual(tampered.status, 0);
    assert.match(tampered.stderr, /differs/);
  });

  it("rejects native modules, remote imports and scopeless capabilities", () => {
    const make = (name, source, descriptor = {}) => {
      const dir = join(scratch, name);
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, "extension.json"), JSON.stringify({ name: "hostile", version: "0.1.0", ...descriptor }));
      writeFileSync(join(dir, "index.mjs"), source);
      return dir;
    };

    const native = make("native", 'import "./addon.node";\nexport default () => {};\n');
    const nativeRun = run(["--src", native, "--out", join(scratch, "native-out")]);
    assert.notEqual(nativeRun.status, 0);
    assert.match(nativeRun.stderr, /native modules are not allowed/);

    const remote = make("remote", 'export default () => {};\nimport "https://example.com/x.mjs";\n');
    const remoteRun = run(["--src", remote, "--out", join(scratch, "remote-out")]);
    assert.notEqual(remoteRun.status, 0);
    assert.match(remoteRun.stderr, /remote imports are not allowed/);

    const scopeless = make("scopeless", "export default () => {};\n", {
      capabilities: [{ name: "exec", scope: [] }],
    });
    const scopeRun = run(["--src", scopeless, "--out", join(scratch, "scope-out")]);
    assert.notEqual(scopeRun.status, 0);
    assert.match(scopeRun.stderr, /explicit scope/);
  });
});
