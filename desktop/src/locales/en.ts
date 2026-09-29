// English UI copy (DESIGN.md §13.5). Session content is data, never listed here.
export default {
  app: {
    subtitle: "Stage 6 · Approval flow wired · model = scripted stub",
  },
  status: {
    ready: "Ready",
    running: "Working…",
  },
  chat: {
    empty:
      'Send any message to start. Include "echo" to trigger the tool-echo plugin; include "email" to trigger send_email — an irreversible tool that raises an approval dialog.',
    placeholder: "Type a message… (Enter to send, Shift+Enter for newline)",
    send: "Send",
  },
  tool: {
    params: "Arguments",
    result: "Result",
    running: "Running…",
    success: "Success",
    failed: "Failed",
  },
  effect: {
    pure: "pure",
    compensable: "compensable",
    irreversible: "irreversible",
  },
  approval: {
    title: "Confirmation required",
    subtitle:
      "This tool has an irreversible side effect. Review the arguments before deciding.",
    toolLabel: "Tool",
    argsLabel: "Arguments",
    deny: "Deny",
    allow: "Allow",
  },
  error: {
    unknown: "Something went wrong",
  },
} as const;
