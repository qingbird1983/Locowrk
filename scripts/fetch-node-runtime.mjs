#!/usr/bin/env node
// Downloads the pinned Node runtime into runtime/node/ (ADR-0015).
//
// Usage:
//   node scripts/fetch-node-runtime.mjs            # pinned version, this platform
//   node scripts/fetch-node-runtime.mjs --version v26.13.0
//   node scripts/fetch-node-runtime.mjs --check    # verify an installed runtime only
//
// The runtime directory is not tracked by git: the download is verified against
// the release's SHASUMS256.txt and runtime/README.md records version, source
// URL and checksum so a build can be reproduced offline from a cached artifact.

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PINNED = "v26.13.0";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtimeDir = join(root, "runtime");
const nodeDir = join(runtimeDir, "node");

function fail(message) {
  console.error("fetch-node-runtime:", message);
  process.exit(1);
}

function parseArgs(argv) {
  const opts = { version: PINNED, check: false, force: false, mirror: "" };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--version") opts.version = argv[++i];
    else if (arg === "--check") opts.check = true;
    else if (arg === "--force") opts.force = true;
    else if (arg === "--mirror") opts.mirror = argv[++i];
    else if (arg === "--help" || arg === "-h") {
      console.log("usage: node scripts/fetch-node-runtime.mjs [--version vX.Y.Z] [--mirror URL] [--check] [--force]");
      process.exit(0);
    } else fail(`unknown argument: ${arg}`);
  }
  if (!/^v\d+\.\d+\.\d+$/.test(opts.version ?? "")) fail(`invalid version: ${opts.version}`);
  return opts;
}

function platformParts() {
  const arch = { x64: "x64", arm64: "arm64" }[process.arch];
  if (!arch) fail(`unsupported architecture: ${process.arch}`);
  switch (process.platform) {
    case "win32":
      return { os: "win", arch, ext: "zip", binary: "node.exe" };
    case "darwin":
      return { os: "darwin", arch, ext: "tar.gz", binary: "node" };
    case "linux":
      return { os: "linux", arch, ext: "tar.gz", binary: "node" };
    default:
      fail(`unsupported platform: ${process.platform}`);
  }
}

async function download(url) {
  // curl handles locked-down networks (proxy settings, TLS) that Node's fetch
  // can stall on; fetch is the fallback when curl is absent.
  if (hasCurl()) {
    const file = join(mkdtempSync(join(tmpdir(), "locowrk-dl-")), "artifact");
    execFileSync("curl", ["--fail", "--silent", "--show-error", "--location", "--max-time", "900", "--output", file, url], { stdio: "inherit" });
    const data = readFileSync(file);
    rmSync(dirname(file), { recursive: true, force: true });
    return data;
  }
  const response = await fetch(url, { signal: AbortSignal.timeout(900000) });
  if (!response.ok) fail(`GET ${url} -> HTTP ${response.status}`);
  return Buffer.from(await response.arrayBuffer());
}

function hasCurl() {
  try {
    execFileSync("curl", ["--version"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

function installedVersion(binaryPath) {
  try {
    const out = execFileSync(binaryPath, ["--version"], { encoding: "utf8" }).trim();
    return out;
  } catch (err) {
    fail(`cannot run ${binaryPath}: ${err.message}`);
  }
}

function writeReadme({ version, url, official, sha, binary }) {
  const lines = [
    "# Node runtime (not tracked by git)",
    "",
    "`runtime/node/` holds the pinned Node runtime that ships with a release",
    "(ADR-0015). It is fetched by `scripts/fetch-node-runtime.mjs`; CI and local",
    "builds verify the checksum below before use.",
    "",
    "| Field | Value |",
    "|---|---|",
    `| Version | \`${version}\` |`,
    `| Official source | ${official} |`,
    `| Downloaded from | ${url} |`,
    `| SHA-256 | \`${sha}\` |`,
    `| Binary | \`runtime/node/${binary}\` |`,
    "| License | Node.js (MIT); see `runtime/node/LICENSE` |",
    "",
    "Reproduce:",
    "",
    "```bash",
    `node scripts/fetch-node-runtime.mjs --version ${version}`,
    "```",
    "",
    "The launcher resolves this directory before PATH; a release build without it",
    "keeps the A track disabled (fail-closed) instead of falling back silently.",
    "",
  ];
  writeFileSync(join(runtimeDir, "README.md"), lines.join("\n"));
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const { os, arch, ext, binary } = platformParts();
  const target = join(nodeDir, binary);

  if (opts.check) {
    if (!existsSync(target)) fail(`no runtime at ${target}; run without --check to fetch it`);
    const got = installedVersion(target);
    if (got !== opts.version) fail(`runtime reports ${got}, expected ${opts.version}`);
    console.log(`runtime OK: ${got} at ${target}`);
    return;
  }

  if (existsSync(target) && !opts.force) {
    const got = installedVersion(target);
    if (got === opts.version) {
      console.log(`runtime already present: ${got} at ${target}`);
      return;
    }
    console.log(`replacing runtime ${got} with ${opts.version}`);
  }

  const file = `node-${opts.version}-${os}-${arch}.${ext}`;
  const official = `https://nodejs.org/dist/${opts.version}/${file}`;
  let fetched = null;
  for (const base of mirrorBases(opts.mirror)) {
    const url = `${base}/${opts.version}`;
    try {
      const sums = (await download(`${url}/SHASUMS256.txt`)).toString("utf8");
      const expected = sums
        .split("\n")
        .map((line) => line.trim().split(/\s+/))
        .find((parts) => parts[1] === file)?.[0];
      if (!expected) {
        console.error(`fetch-node-runtime: ${base} has no checksum for ${file}`);
        continue;
      }
      console.log(`downloading ${url}/${file}`);
      const archive = await download(`${url}/${file}`);
      const actual = sha256(archive);
      if (actual !== expected) {
        console.error(`fetch-node-runtime: checksum mismatch from ${base}: ${actual} != ${expected}`);
        continue;
      }
      fetched = { archive, sha: actual, url: `${url}/${file}` };
      break;
    } catch (err) {
      console.error(`fetch-node-runtime: ${base} failed: ${err.message}`);
    }
  }
  if (!fetched) fail(`no mirror served ${file}`);
  const { archive, sha, url } = fetched;

  const work = mkdtempSync(join(tmpdir(), "locowrk-node-"));
  try {
    const archivePath = join(work, file);
    writeFileSync(archivePath, archive);
    execFileSync("tar", ["-xf", archivePath, "-C", work], { stdio: "inherit" });
    const top = readdirSync(work).find((name) => statSync(join(work, name)).isDirectory() && name.startsWith("node-"));
    if (!top) fail("archive did not contain a node-* directory");
    mkdirSync(nodeDir, { recursive: true });
    copyFileSync(join(work, top, binary), target);
    const license = join(work, top, "LICENSE");
    if (existsSync(license)) copyFileSync(license, join(nodeDir, "LICENSE"));
    const got = installedVersion(target);
    if (got !== opts.version) fail(`downloaded runtime reports ${got}, expected ${opts.version}`);
    writeReadme({ version: opts.version, url, official, sha, binary });
    console.log(`installed ${got} at ${target} (sha256 ${sha})`);
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

// mirrorBases lists download bases in priority order: an explicit --mirror or
// LOCOWRK_NODE_MIRROR, a nearby mirror (fast on locked-down networks, and the
// checksum still comes from that host), then the official distribution site.
function mirrorBases(explicit) {
  const bases = [];
  if (explicit) bases.push(explicit.replace(/\/$/, ""));
  const env = process.env.LOCOWRK_NODE_MIRROR;
  if (env) bases.push(env.replace(/\/$/, ""));
  bases.push("https://mirrors.ustc.edu.cn/node", "https://nodejs.org/dist");
  return [...new Set(bases)];
}

await main();
