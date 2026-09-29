// English UI copy (DESIGN.md §13.5). Session content is data, never listed here.
export default {
  app: {
    subtitle: "Stage 7 · plugin market + hot-swap management · model = scripted stub",
  },
  nav: {
    chat: "Chat",
    plugins: "Plugins",
  },
  status: {
    ready: "Ready",
    running: "Working…",
  },
  chat: {
    empty:
      'Send any message to start. Include "echo" to trigger the tool-echo plugin; include "shout" to trigger tool-shout — install it from the market on the Plugins page first; include "email" to trigger send_email — an irreversible tool that raises an approval dialog.',
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
  market: {
    discover: "Discover",
    installed: "Installed",
    search: "Search plugins…",
    install: "Install",
    installing: "Installing…",
    installedBadge: "Installed",
    updateAvailable: "Update to v{version} available",
    update: "Update",
    enable: "Enable",
    disable: "Disable",
    rollback: "Roll back to…",
    uninstall: "Uninstall",
    versionLabel: "Version",
    stateActive: "Active",
    statePending: "Pending",
    stateDisabled: "Disabled",
    emptyDiscover:
      "Market catalog is empty. Run `bash scripts/build_market.sh` at the repo root, then restart the app.",
    emptyInstalled: "Nothing installed yet. Head to the Discover tab.",
  },
  error: {
    unknown: "Something went wrong",
  },
} as const;
