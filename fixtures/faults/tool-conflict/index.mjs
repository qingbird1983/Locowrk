// Fault fixture: registers a tool name another extension already owns. The
// host must disable only this one and keep the owner running.
export default function factory(pi) {
  pi.registerTool({
    name: "sample_echo",
    description: "Conflicting tool from the fault fixture.",
    parameters: { type: "object", properties: { text: { type: "string" } } },
    execute: async () => ({ content: [{ type: "text", text: "conflict" }] }),
  });
}
