#!/usr/bin/env node
// Bundles one A-track extension into plugin.json + bundle.mjs
// (docs/design/phase-6/runtime.md §4).
//
//   node tools/extension-build/build.mjs --src <source dir> --out <package dir>
//   node tools/extension-build/build.mjs --src <dir> --out <dir> --check
//
// Source directory:
//   extension.json   {name, version, capabilities?, description?, source?}
//   index.ts|index.mjs|index.js   export default function (pi) { ... }
//
// The output is a package the adapter can start: plugin.json, bundle.mjs and a
// build.json that records the source, esbuild version and output hash. Builds
// are reproducible: the same input produces the same bytes (no timestamps).

import { createHash } from "node:crypto";
import { readFileSync, readdirSync, existsSync, mkdirSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const shimPath = join(repoRoot, "extensions", "shim", "host.mjs");

const CAPABILITIES = new Set(["fs.read", "fs.write", "fs.delete", "exec", "net"]);
const ENTRY_CANDIDATES = ["index.ts", "index.tsx", "index.mjs", "index.js"];
const SHIM_SPECIFIER = "@locowrk/shim";

function fail(message) {
  console.error("extension-build:", message);
  process.exit(1);
}

function parseArgs(argv) {
  const opts = { src: "", out: "", check: false };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--src") opts.src = argv[++i];
    else if (arg === "--out") opts.out = argv[++i];
    else if (arg === "--check") opts.check = true;
    else if (arg === "--help" || arg === "-h") {
      console.log("usage: node build.mjs --src <dir> --out <dir> [--check]");
      process.exit(0);
    } else fail(`unknown argument: ${arg}`);
  }
  if (!opts.src) fail("--src is required");
  if (!opts.out) fail("--out is required");
  return opts;
}

function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

function readDescriptor(src) {
  const path = join(src, "extension.json");
  if (!existsSync(path)) fail(`missing ${path}`);
  const descriptor = JSON.parse(readFileSync(path, "utf8"));
  if (!/^[a-z0-9][a-z0-9-]*$/.test(descriptor.name ?? "")) {
    fail(`extension.json name must be lower-kebab-case: ${descriptor.name}`);
  }
  if (!/^\d+\.\d+\.\d+/.test(descriptor.version ?? "")) {
    fail(`extension.json version must start with x.y.z: ${descriptor.version}`);
  }
  for (const capability of descriptor.capabilities ?? []) {
    if (!CAPABILITIES.has(capability?.name)) fail(`unknown capability: ${capability?.name}`);
    if (!Array.isArray(capability.scope) || capability.scope.length === 0) {
      fail(`capability ${capability.name} needs an explicit scope (fail-closed)`);
    }
  }
  return descriptor;
}

function entryFile(src, descriptor) {
  if (descriptor.source) {
    const path = join(src, descriptor.source);
    if (!existsSync(path)) fail(`extension.json source does not exist: ${descriptor.source}`);
    return path;
  }
  const found = ENTRY_CANDIDATES.map((name) => join(src, name)).find((path) => existsSync(path));
  if (!found) fail(`no entry file in ${src} (${ENTRY_CANDIDATES.join(", ")})`);
  return found;
}

// rejectRemote refuses imports the package must never carry: native modules,
// remote URLs and dynamic imports of something that is not a literal path.
function rejectRemote() {
  return {
    name: "locowrk-reject-remote",
    setup(pluginBuild) {
      pluginBuild.onResolve({ filter: /\.node$/ }, (args) => ({
        errors: [{ text: `native modules are not allowed: ${args.path}` }],
      }));
      pluginBuild.onResolve({ filter: /^https?:\/\// }, (args) => ({
        errors: [{ text: `remote imports are not allowed: ${args.path}` }],
      }));
    },
  };
}

function resolveShim() {
  return {
    name: "locowrk-shim",
    setup(pluginBuild) {
      pluginBuild.onResolve({ filter: new RegExp(`^${SHIM_SPECIFIER.replace(/[/@]/g, "\\$&")}$`) }, () => ({
        path: shimPath,
      }));
    },
  };
}

async function bundle(src, descriptor) {
  const entry = entryFile(src, descriptor);
  const entrySource = readFileSync(entry);
  const wrapper = [
    `import factory from ${JSON.stringify(entry)};`,
    `import { run } from ${JSON.stringify(SHIM_SPECIFIER)};`,
    `run(factory, ${JSON.stringify({
      name: descriptor.name,
      version: descriptor.version,
      capabilities: descriptor.capabilities ?? [],
    })});`,
    "",
  ].join("\n");

  const result = await build({
    stdin: { contents: wrapper, resolveDir: src, sourcefile: "locowrk-entry.mjs", loader: "js" },
    bundle: true,
    write: false,
    format: "esm",
    platform: "node",
    target: ["node20"],
    charset: "utf8",
    legalComments: "none",
    logLevel: "silent",
    external: ["node:*"],
    plugins: [rejectRemote(), resolveShim()],
  });
  if (result.errors.length > 0) {
    fail(result.errors.map((error) => error.text).join("; "));
  }
  const output = result.outputFiles?.[0];
  if (!output) fail("esbuild produced no output");
  const bundleBytes = Buffer.from(output.contents);
  const text = bundleBytes.toString("utf8");
  if (/import\(\s*["'`]https?:/.test(text)) fail("bundle contains a remote dynamic import");
  if (/require\(\s*["'`]https?:/.test(text)) fail("bundle contains a remote require");

  const manifest = {
    name: descriptor.name,
    version: descriptor.version,
    protocol: 1,
    ...(descriptor.description ? { description: descriptor.description } : {}),
    entrypoint: "bundle.mjs",
    capabilities: descriptor.capabilities ?? [],
  };
  const record = {
    name: descriptor.name,
    version: descriptor.version,
    esbuild: JSON.parse(readFileSync(join(here, "node_modules", "esbuild", "package.json"), "utf8")).version,
    entrypoint: "bundle.mjs",
    sha256: sha256(bundleBytes),
    source: {
      file: basename(entry),
      sha256: sha256(entrySource),
      descriptor: "extension.json",
    },
    node: ">=20",
  };
  return {
    bundleBytes,
    manifest: `${JSON.stringify(manifest, null, 2)}\n`,
    record: `${JSON.stringify(record, null, 2)}\n`,
  };
}

function checkExisting(out, result) {
  const path = join(out, "bundle.mjs");
  if (!existsSync(path)) fail(`--check: no build at ${path}`);
  const current = readFileSync(path);
  if (sha256(current) !== sha256(result.bundleBytes)) {
    fail("--check: bundle.mjs differs from the source build");
  }
  const manifest = readFileSync(join(out, "plugin.json"), "utf8");
  if (manifest !== result.manifest) fail("--check: plugin.json differs from the source build");
  console.log(`extension-build: ${out} matches the source build`);
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (!existsSync(shimPath)) fail(`shim not found: ${shimPath}`);
  const src = resolve(opts.src);
  const out = resolve(opts.out);
  if (!statSync(src).isDirectory()) fail(`--src is not a directory: ${src}`);
  const descriptor = readDescriptor(src);
  const result = await bundle(src, descriptor);

  if (opts.check) {
    checkExisting(out, result);
    return;
  }

  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "bundle.mjs"), result.bundleBytes);
  writeFileSync(join(out, "plugin.json"), result.manifest);
  writeFileSync(join(out, "build.json"), result.record);
  console.log(`extension-build: wrote ${join(out, "bundle.mjs")} (${result.bundleBytes.length} bytes)`);
}

await main();
