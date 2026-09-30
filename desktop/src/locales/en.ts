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
  // Idle badge (IdleBadge, ⑩ wired per exposure doc §8.4/§11.3): backend
  // sends structured IdleInfo only; the frontend renders human phrasing.
  idle: {
    // Level names (legend / fallback when unit is missing)
    fresh: "newly enabled",
    normal: "normal",
    idle: "idle",
    stale: "long idle",
    abandoned: "likely abandoned",
    never_used: "never used",
    // §8.6 phrasing table
    today: "Used today",
    this_week: "Used this week",
    last_week: "Used last week",
    last_month: "Used last month",
    month: "Unused for over {n} month(s)",
    half_year: "Unused for over half a year",
    year: "Unused for over {n} year(s)",
    // §8.7 never-used (strictly distinct from long-unused) + trial window
    just_installed: "Just installed",
    never2w: "Installed two weeks ago, never used",
    neverMonths: "Installed over {n} month(s) ago, never used",
    neverHalfYear: "Installed over half a year ago, never used",
  },
  memory: {
    search: "Search memories…",
    empty: "No memories yet. Say \"remember …\" in a chat and it will show up here.",
    noMatch: "No matching memory entries.",
    hint: "Memories persist across sessions; injection toggle lives in Settings → Memory. Every entry can be edited or deleted.",
    source_explicit: "Explicit",
    source_extracted: "Auto-extracted",
    edit: "Edit",
    delete: "Delete",
    deleteConfirm: "Confirm delete",
    save: "Save",
    cancel: "Cancel",
    clearAll: "Clear all memories",
    clearConfirm: "Click again to confirm",
    time: {
      just_now: "just now",
      minutes: "{n} min ago",
      hours: "{n} h ago",
      days: "{n} d ago",
      weeks: "{n} wk ago",
      months: "{n} mo ago",
    },
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
    // ⑩ idle UI (exposure doc §8.8/§9.2/§9.3/§9.5): filter chips use "how
    // long" phrasing; hints appear only when the user opens the tab.
    usageDisabled: "Disabled",
    idleGt1m: "Unused for over 1 month",
    idleGt3m: "Unused for over 3 months",
    idleGt6m: "Unused for over half a year",
    hintIdle: "{n} plugin(s) unused for over 3 months",
    hintGo: "Take a look",
    selectAll: "Select all",
    selectedCount: "{n} selected",
    batchDisable: "Disable selected",
    batchEnable: "Enable selected",
    batchUninstall: "Uninstall selected",
    batchUninstallConfirm: "Uninstall these {n} plugins? This cannot be undone:",
    batchPartialFail: "{n} failed:",
    cancel: "Cancel",
    builtinNotSelectable: "Built-in dev plugins are not batch-managed",  },
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
    // ⑪ Memory section (real section, replaces shell placeholder)
    memoryTitle: "Memory",
    memoryHint:
      "Relevant memories are injected into the conversation (via system prompt, outside compaction); per-turn count is adjustable.",
    memoryEnabled: "Inject memories automatically",
    memoryTopK: "Entries per turn",
    memoryTopKHint: "1–20, default 5; injected content is viewable and deletable on the Memory page.",
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
