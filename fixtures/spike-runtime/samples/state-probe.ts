/**
 * Spike probe: branch-aware state and a terminal-only view, mirroring the
 * upstream todo example. Session reads and ctx.ui.custom are the two areas the
 * compatibility matrix calls "deferred" and "no-op"; the extension must still
 * register its tool and keep running.
 */

import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

interface Item {
	id: number;
	text: string;
	done: boolean;
}

export default function (pi: ExtensionAPI) {
	let items: Item[] = [];
	let nextId = 1;

	const rebuild = (ctx: ExtensionContext) => {
		items = [];
		nextId = 1;
		for (const entry of ctx.sessionManager.getBranch()) {
			if (entry.type !== "message") continue;
			const details = (entry.message as { details?: { items?: Item[] } }).details;
			if (details?.items) items = details.items;
		}
	};

	pi.on("session_start", async (_event, ctx) => rebuild(ctx));
	pi.on("session_tree", async (_event, ctx) => rebuild(ctx));

	pi.registerTool({
		name: "probe_state",
		label: "Probe State",
		description: "Add or list probe items.",
		parameters: Type.Object({
			action: Type.Union([Type.String(), Type.String()]),
			text: Type.Optional(Type.String()),
		}),

		async execute(_toolCallId, params, _signal, _onUpdate, _ctx) {
			if (params.action === "add" && params.text) {
				items.push({ id: nextId, text: params.text, done: false });
				nextId += 1;
			}
			const text = items.length ? items.map((i) => `#${i.id} ${i.text}`).join("\n") : "empty";
			return {
				content: [{ type: "text", text }],
				details: { items: [...items], nextId },
			};
		},
	});

	pi.registerCommand("probe-list", {
		description: "Show items in a terminal panel",
		handler: async (_args, ctx) => {
			ctx.ui.notify(`items=${items.length}`, "info");
			await ctx.ui.custom(() => ({ render: () => [], invalidate: () => {} }));
		},
	});
}
