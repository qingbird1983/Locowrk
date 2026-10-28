// Route B of the Phase 2 runtime spike: run the same bundled extension under
// Node with the same shim and the same JSONL host protocol as host-a.

import { createRequire } from "node:module";
import path from "node:path";
import readline from "node:readline";

const args = process.argv.slice(2);
const bundlePath = args[args.indexOf("--bundle") + 1];
if (!bundlePath) {
	console.error("host-b: --bundle is required");
	process.exit(2);
}

globalThis.__hostSend = (kind, payload) => {
	process.stdout.write(JSON.stringify({ jsonrpc: "2.0", method: kind, params: JSON.parse(payload) }) + "\n");
};

const require = createRequire(import.meta.url);
require(path.resolve(bundlePath));

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });

function send(message) {
	process.stdout.write(JSON.stringify(message) + "\n");
}

function ok(id, result) {
	send({ jsonrpc: "2.0", id, result: result === undefined ? null : result });
}

function fail(id, code, message) {
	send({ jsonrpc: "2.0", id, error: { code, message } });
}

async function waitBoot() {
	const deadline = Date.now() + 10000;
	while (!globalThis.__locowrkBootDone()) {
		await new Promise((r) => setTimeout(r, 2));
		if (Date.now() > deadline) throw new Error("boot timeout");
	}
}

async function handle(msg) {
	switch (msg.method) {
		case "initialize": {
			globalThis.__locowrkBoot(JSON.stringify({ cwd: msg.params?.cwd || ".", mode: "rpc" }));
			await waitBoot();
			ok(msg.id, JSON.parse(globalThis.__locowrkDescribe()));
			return false;
		}
		case "event":
		case "invoke_tool":
		case "invoke_command": {
			const kind = msg.method === "event" ? "event" : msg.method === "invoke_tool" ? "tool" : "command";
			const name = msg.method === "event" ? msg.params?.name : msg.params?.name;
			const payload = msg.method === "invoke_tool" ? msg.params?.arguments : msg.method === "invoke_command" ? msg.params?.args : msg.params?.event;
			const raw = await globalThis.__locowrk.dispatchPromise(kind, name, JSON.stringify(payload ?? null));
			const decoded = JSON.parse(raw);
			if (!decoded.ok) {
				fail(msg.id, 4404, decoded.error || "dispatch failed");
				return false;
			}
			ok(msg.id, decoded.value);
			return false;
		}
		case "metrics": {
			ok(msg.id, JSON.parse(globalThis.__locowrkDescribe()));
			return false;
		}
		case "shutdown":
			ok(msg.id, { ok: true });
			return true;
		default:
			fail(msg.id, 4400, `unsupported method: ${msg.method}`);
			return false;
	}
}

let stopping = false;
rl.on("line", (line) => {
	if (stopping || !line.trim()) return;
	let msg;
	try {
		msg = JSON.parse(line);
	} catch (err) {
		fail(null, 4405, "protocol_error: " + String(err));
		return;
	}
	handle(msg).then((stop) => {
		if (stop) {
			stopping = true;
			rl.close();
			process.exit(0);
		}
	});
});
