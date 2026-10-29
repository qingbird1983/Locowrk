// A production-shaped orchestration fixture for Phase 6: registers a tool and
// a command, emits parallel subagent and workflow events, and honours abort.
export default function factory(pi) {
  pi.registerTool({
    name: "sample_echo",
    description: "Echo text through the A-track shim.",
    parameters: {
      type: "object",
      properties: { text: { type: "string" } },
      required: ["text"],
    },
    execute: async (_id, args) => ({
      content: [{ type: "text", text: "sample: " + String(args?.text ?? "") }],
      details: { source: "orchestrator-sample" },
    }),
  });

  pi.registerCommand({
    name: "sample_events",
    description: "Emit two parallel subagents plus a workflow step.",
    handler: async () => {
      pi.emitEvent("subagent_spawned", { id: "sample-a", name: "探索", parentId: "" });
      pi.emitEvent("subagent_spawned", { id: "sample-b", name: "实现", parentId: "" });
      pi.workflowStep("实现", "running");
      pi.ui.notify("sample: two subagents started", "info");
      setTimeout(() => {
        // The slower subagent settles last, so the order proves the host keeps
        // completion order rather than spawn order.
        pi.emitEvent("subagent_settled", { id: "sample-b" });
      }, 30);
      setTimeout(() => {
        pi.emitEvent("subagent_settled", { id: "sample-a" });
        pi.workflowStep("实现", "done");
      }, 80);
    },
  });

  let timers = [];

  pi.registerCommand({
    name: "sample_chain",
    description: "Advance three workflow steps in order, then notify.",
    handler: async () => {
      const steps = ["收集", "实现", "验证"];
      let delay = 0;
      for (const step of steps) {
        setTimeout(() => pi.workflowStep(step, "running"), delay);
        delay += 30;
        setTimeout(() => pi.workflowStep(step, "done"), delay);
        delay += 30;
      }
      timers.push(
        setTimeout(() => pi.ui.notify("sample: chain finished", "info"), delay + 20),
      );
    },
  });

  pi.registerCommand({
    name: "sample_chain_long",
    description: "Start a chain that keeps running until aborted.",
    handler: async () => {
      pi.workflowStep("长任务", "running");
      for (let i = 0; i < 40; i++) {
        timers.push(setTimeout(() => pi.workflowStep("长任务", "running", "tick " + i), i * 50));
      }
    },
  });

  pi.on("abort", async () => {
    for (const timer of timers) clearTimeout(timer);
    timers = [];
    // Cancellation is not a failure: the step is skipped, not failed.
    pi.workflowStep("长任务", "skipped", "cancelled");
    pi.ui.notify("sample: cancelled", "warning");
  });

  pi.registerCommand({
    name: "sample_deferred",
    description: "Call every deferred member and report diagnostics.",
    handler: async (_args, ctx) => {
      await ctx.executeTool("read", { path: "note.txt" });
      pi.sendMessage({ text: "hello" });
      pi.sendUserMessage("hello");
      pi.appendEntry({ kind: "note" });
      await ctx.compact();
      ctx.ui.setWidget("sample", ["x"]);
      ctx.ui.setEditorText("text");
      ctx.ui.pasteToEditor("text");
      ctx.ui.custom(() => ({}));
      pi.registerShortcut("ctrl+x", { handler: async () => {} });
      pi.ui.notify("sample: deferred members exercised", "info");
    },
  });

  pi.on("tool_execution_start", async (event) => {
    pi.workflowStep("观察工具", "running", String(event?.toolName ?? ""));
  });
  pi.on("tool_execution_end", async () => {
    pi.workflowStep("观察工具", "done");
  });

  // Permission gate: the host sends tool_call as a request because the shim
  // declares wantsEventAck, and blocks the call when a handler says so.
  pi.on("tool_call", async (event) => {
    if (event?.toolName === "danger" || String(event?.input?.command ?? "").includes("rm -rf")) {
      return { block: true, reason: "fixture blocks destructive commands" };
    }
    return undefined;
  });
}
