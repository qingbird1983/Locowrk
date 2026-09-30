// English UI copy (DESIGN.md §13.5). Session content is data, never listed here.
export default {
  app: {
    subtitle: "Stage 8 · real model wired · current model = {model}",
    modelFallback: "scripted stub (Settings → LLM)",
  },
  nav: {
    chat: "Chat",
    plugins: "Plugins",
    memory: "Memory",
    settings: "Settings",
  },
  status: {
    ready: "Ready",
    running: "Working…",
  },
  chat: {
    empty:
      "Send any message to start. The model calls enabled plugin tools as needed (each call shows as a tool card); irreversible tools such as send_email raise an approval dialog first.",
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
  // In-chat system messages (SystemNotice pattern): copy is UI, not
  // session data — the store keeps an i18n key, translated at render time
  // so locale switches apply to past notices too.
  notice: {
    action: {
      details: "View details",
      undo: "Undo",
    },
    // ⑨ Compaction notice ([View details][Undo], Context doc §3.8.3)
    contextCompacted: "Compacted earlier messages, saved {saved} tokens",
    compactUndone:
      "Compaction undone: full pre-compaction context restored (later turns were rewound too).",
    compactUndoFailed: "Failed to undo compaction: {reason}",
  },
  // Idle badge (IdleBadge): six levels → gray/yellow/red; with value+unit
  // the badge renders a human duration string instead of the level name.
  idle: {
    fresh: "newly enabled",
    normal: "normal",
    idle: "idle",
    stale: "long idle",
    abandoned: "likely abandoned",
    never_used: "never used",
    unusedFor: "unused for {n} {unit}",
    unit: {
      minute: "min",
      hour: "h",
      day: "day(s)",
      week: "week(s)",
    },
  },
  memory: {
    search: "Search memories…",
    empty: "No manageable memory entries yet.",
    hint: "Memory injection and management arrive in a later version (for now the agent can memorize/recall via chat).",
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
    builtinBadge: "Built-in (ships with the repo, managed via build scripts)",
    origin_official: "Official",
    origin_community: "Third-party",
    filterAll: "All",
    noMatch: "No matching entries.",
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
      "Applies immediately after saving (no restart). The API key is stored only in the local data directory and is never echoed back to the UI.",
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
    shortcutTitle: "Global shortcut",
    shortcut: "Toggle main window",
    shortcutPlaceholder: "e.g. Ctrl+Shift+L",
    shortcutHint:
      "Applies immediately after saving (no restart). Press it in any app to show/hide the main window. If the key is taken by another program, saving fails — pick another combo.",
    // Placeholder sections (replaced with real sections as ⑪/⑫ land)
    memoryTitle: "Memory",
    memoryHint: "Memory injection toggle and management entry arrive in a later version.",
    developerTitle: "Developer options",
    developerHint:
      "Exposure debug panel (Top-N, distribution stats, token-savings estimate) arrives in a later version.",
    // ⑨ Context management section (real section, replaces shell placeholder)
    contextTitle: "Context management",
    contextHint:
      "Long conversations are auto-compacted as they approach the window limit. Threshold and strategy are adjustable anytime; compaction is undoable.",
    window: "Model context window",
    windowUnset: "Not set (conservative 32k protection)",
    windowCustom: "Custom…",
    windowCustomLabel: "Window size (tokens)",
    threshold: "Compaction threshold",
    thresholdValue: "Current: {pct}% · {tokens} tokens",
    thresholdSuggest: "ⓘ Suggested {pct}%",
    thresholdSuggestHint: "(larger windows suggest lower thresholds)",
    thresholdLowWarning: "Lower thresholds may cost more than they save",
    strategy: "Compaction strategy",
    strategy_sliding_window: "Sliding window (keep recent messages only, no model call)",
    strategy_layered: "Layered (summary + recent messages, recommended)",
    strategy_summary_only: "Summary only (compress everything)",
  },
  error: {
    unknown: "Something went wrong",
  },
} as const;
