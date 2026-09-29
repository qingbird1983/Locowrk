// English UI copy (DESIGN.md §13.5). Session content is data, never listed here.
export default {
  app: {
    subtitle: "Stage 7 · plugin market + hot-swap management · model = scripted stub",
  },
  nav: {
    chat: "Chat",
    plugins: "Plugins",
    settings: "Settings",
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
  settings: {
    title: "Settings",
    llmTitle: "LLM configuration",
    llmHint:
      "Takes effect once the real model adapter lands (stage 8); chat is still driven by the scripted stub. The API key is stored only in the local data directory and is never echoed back to the UI.",
    provider: "Provider",
    model: "Model",
    baseUrl: "Base URL",
    baseUrlPlaceholder: "Leave empty to use the provider's default endpoint",
    apiKey: "API Key",
    apiKeyPlaceholder: "Not saved",
    apiKeyKeep: "Saved · leave empty to keep",
    clearKey: "Clear saved key",
    save: "Save",
    saved: "Saved",
    sessionsTitle: "Session storage location",
    dataDir: "Data root directory",
    dataDirHint:
      "Plugins, the lockfile and settings.json live here. Override with the LOCOWRK_DATA_DIR environment variable (restart required).",
    sessionsMemory:
      "Sessions currently live in memory and are cleared on restart; on-disk persistence lands in a later version.",
    langTitle: "Language / 语言",
    langSystem: "Follow system",
    langZh: "中文",
    langEn: "English",
    langHint:
      "Applies immediately and is remembered. \"Follow system\" shows Chinese on zh environments, English otherwise.",
  },
  error: {
    unknown: "Something went wrong",
  },
} as const;
