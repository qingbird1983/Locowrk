/**
 * Spike probe: the orchestration shape the A track exists for.
 *
 * Mirrors what a delegation extension does: spawn a worker process, read its
 * stdout, keep a temp file, and return a structured tool result. It uses Node
 * built-ins the same way the upstream subagent example does; whether a runtime
 * can run this is exactly the question the spike has to answer.
 */

import { spawn } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

function runWorker(input: string): Promise<string> {
	return new Promise((resolve, reject) => {
		const proc = spawn(process.execPath, ["-e", "process.stdout.write('worker:' + process.argv[1])", input], {
			stdio: ["ignore", "pipe", "pipe"],
		});
		let out = "";
		proc.stdout.on("data", (chunk) => {
			out += chunk.toString();
		});
		proc.on("error", reject);
		proc.on("close", (code) => {
			if (code === 0) resolve(out);
			else reject(new Error(`worker exited with ${code}`));
		});
	});
}

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async (_event, ctx) => {
		ctx.ui.notify("orchestrator-probe started", "info");
	});

	pi.registerTool({
		name: "probe_delegate",
		label: "Probe Delegate",
		description: "Delegate one task to a worker process and return its output.",
		parameters: Type.Object({
			task: Type.String({ description: "Task text for the worker" }),
		}),

		async execute(_toolCallId, params, signal, _onUpdate, _ctx) {
			if (signal?.aborted) throw new Error("aborted");
			const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), "locowrk-probe-"));
			const file = path.join(dir, "task.txt");
			try {
				await fs.promises.writeFile(file, params.task, "utf8");
				const output = await runWorker(params.task);
				return {
					content: [{ type: "text", text: output }],
					details: { task: params.task, mode: "single", tempFile: true },
				};
			} finally {
				await fs.promises.rm(dir, { recursive: true, force: true });
			}
		},
	});
}
