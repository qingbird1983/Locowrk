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
        pi.emitEvent("subagent_settled", { id: "sample-a" });
        pi.emitEvent("subagent_settled", { id: "sample-b" });
        pi.workflowStep("实现", "done");
      }, 20);
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
