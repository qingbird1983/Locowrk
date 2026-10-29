// Fault fixture: the command exits the extension process, and the tool proves
// the process was healthy before that.
export default function factory(pi) {
  pi.registerTool({
    name: "fault_echo",
    description: "Echo through the fault fixture.",
    parameters: { type: "object", properties: { text: { type: "string" } } },
    execute: async (_id, args) => ({
      content: [{ type: "text", text: "fault: " + String(args?.text ?? "") }],
    }),
  });

  pi.registerCommand({
    name: "crash",
    description: "Exit the extension process with code 7.",
    handler: async () => {
      process.exit(7);
    },
  });
}
