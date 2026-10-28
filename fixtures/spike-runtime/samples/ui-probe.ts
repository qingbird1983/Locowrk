/**
 * Spike probe: the minimum conformance path from ADR-0008.
 *
 * start -> register a tool -> receive an event hook -> map a notification ->
 * call ctx.ui.setWidget (must be a no-op with one diagnostic, and the extension
 * must keep running afterwards).
 */

import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

export default function (pi: ExtensionAPI) {
	pi.on("session_start", async (_event, ctx) => {
		ctx.ui.setTitle("ui-probe");
		ctx.ui.setWidget("probe-status", ["ui-probe: waiting for a call"]);
		ctx.ui.notify("ui-probe started", "info");
	});

	pi.on("tool_call", async (event, ctx) => {
		if (event.toolName === "probe_echo") {
			ctx.ui.notify(`tool_call seen for ${event.toolName}`, "info");
		}
	});

	pi.registerTool({
		name: "probe_echo",
		label: "Probe Echo",
		description: "Echo the given text back; used by the runtime spike.",
		parameters: Type.Object({
			text: Type.String({ description: "Text to echo" }),
		}),

		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			ctx.ui.setStatus("probe", "echoing");
			ctx.ui.setWidget("probe-status", [`ui-probe: ${params.text}`]);
			return {
				content: [{ type: "text", text: `echo:${params.text}` }],
				details: { text: params.text },
			};
		},
	});

	pi.registerCommand("probe-ui", {
		description: "Exercise dialogs through the shim",
		handler: async (_args, ctx) => {
			const ok = await ctx.ui.confirm("Probe?", "Continue?");
			const picked = await ctx.ui.select("Pick one", ["alpha", "beta"]);
			ctx.ui.notify(`confirm=${String(ok)} select=${String(picked)}`, "info");
		},
	});
}
