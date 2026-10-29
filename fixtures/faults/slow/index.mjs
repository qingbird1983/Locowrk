// Fault fixture: the tool never answers, so the host must time out and record
// a diagnostic instead of hanging the turn.
export default function factory(pi) {
  pi.registerTool({
    name: "fault_slow",
    description: "Never answers (timeout fixture).",
    timeoutMs: 1000,
    parameters: { type: "object", properties: {} },
    execute: () => new Promise(() => {}),
  });

  pi.registerTool({
    name: "fault_ok",
    description: "Answers immediately, proving the process is alive.",
    parameters: { type: "object", properties: {} },
    execute: async () => ({ content: [{ type: "text", text: "ok" }] }),
  });
}
