// locales/en/developer.ts —— 3 named domains: developer / approval / perm
// (S4 blade ② retired the memory face; relative time moved to shell.ts `time`)
// 由批 F-22b 从 locales/en.ts 逐字节搬来；命名域必须**只在本文件**定义一次
// （index.ts 用对象展开合并，重复定义 = 后一份静默覆盖前一份，scripts/check-i18n.mjs 判红）。
export default {

  developer: {
    hint: "Developer and danger switches. Hidden by default: tap the version number at the sidebar bottom 7 times to unlock.",
    allowDeleteRunning: "Allow deleting a running session",
    allowDeleteRunningHint: "Blocked by default; deleting may leave the transcript incomplete.",
    allowSwitchRunning: "Allow switching away from a running session",
    allowSwitchRunningHint: "Blocked by default; an unfinished turn may spill into the new session.",
    hide: "Hide developer options",
    hideHint:
      "Lock this section away again (danger switches keep their values); tap the version 7 times to unlock.",
    hidden: "Developer options hidden",
    panelTitle: "Last exposure breakdown",
    refresh: "Refresh",
    empty: "No data yet — start a chat and come back.",
    disabled: "Truncation disabled — the full catalog is currently exposed.",
    exposed: "Exposed {exposed} / {total} tools this request",
    always: "always passthrough ({n})",
    sessionHot: "Used this session ({n})",
    globalHot: "Globally hot ({n})",
    retrieved: "Semantically retrieved ({n})",
    saved: "Context saved: ~{n} tokens",
    cacheTitle: "Prefix cache readout",
    cacheHint: "Session totals and per-turn usage; hit rate excludes cache writes.",
    cacheEmpty: "Provider has not reported cache fields yet — start a chat and come back.",
    cacheHitRate: "Session cumulative hit rate: {pct}% ({steps} steps)",
    cacheHitMiss: "Hit / miss / total input: {hit} / {miss} / {input} tokens",
    cacheTurn: "This turn: in {input} · hit {hit} · out {out}",
    cacheAlertTitle: "Cache hit-rate alert",
    // M6.1 break diagnostics (KV v2 §7.4)
    cacheDiagnostics: "Prefix break diagnostics",
    cacheDiagnosticsHint:
      "Fingerprint each request in blocks and compare with the previous turn to locate which block broke cache; off by default.",
    cacheDiagTitle: "Latest step diagnosis",
    cacheDiagEmpty: "No records yet (run a conversation with this on).",
    cacheDiagHealthy: "Step {step}: prefix healthy, pure append ({n} messages).",
    cacheDiagBroken: "Step {step} broke at: {blocks}.",
    cacheAlertHint: "Alerts once per session when the hit rate falls below the threshold.",
    cacheAlertPercent: "Alert threshold (%)",
    prefixTitle: "Prefix discipline",
    prefixHint: "A stable request prefix keeps the cache; changes rebuild it once.",
    pinSession: "Pin tool set per session",
    pinSessionHint: "Tool list fixed per session; search_tools finds new ones; off lowers hit rate.",
  },
  approval: {
    // §3.17 inline permission card (ApprovalModal retired; approval is
    // not a modal)
    cardTitle: "Permission needed",
    queue: "{n} more pending",
    envProject: "Project {path}",
    envTemp: "Temporary session",
    timeoutHint: "Auto-denied in {n} s if unanswered",
    deny: "Deny",
    allowSession: "Allow for this chat",
    allowProject: "Always allow in this project",
    allowProjectHint: "Persists a sticky rule for this project (shell commands stored exactly; other tools by tool name), effective across sessions. Manage in Settings → Tools & exposure.",
    allowOnce: "Allow once",
    // ㉘·B outside-roots gate / ㉘·C privacy gate (two buttons only)
    externalHint: "Target is outside the declared workspace roots; one-time approval only",
    sensitiveLead: "Touches a privacy-sensitive or high-risk target: {kind}",
    sensitive: {
      ssh: "SSH credentials directory",
      aws: "AWS credentials directory",
      gpg: "GPG keyring",
      kube: "Kubernetes cluster credentials",
      creds_file: "Credential files (.git-credentials / .netrc)",
      docker: "Docker registry credentials",
      browser: "Browser credential store (logins / cookies)",
      os_store: "OS credential store access",
      cred_dump: "Credential dumping tool",
      catastrophic: "Destructive command (format / whole-disk delete)",
    },
    // ⑮ Tag for calls spawned by a sub-agent (session_id differs)
    subagentTag: "Sub-agent call",
    decisionFailed: "Failed to submit decision: {reason}",
    pendingBadge: "Approval awaiting decision",
  },
  perm: {
    title: "Permission mode",
    short: {
      plan: "Plan",
      ask: "Ask",
      acceptEdits: "Auto-edits",
      auto: "Full",
    },
    desc: {
      plan: "Read-only: write tools hidden",
      ask: "Approve every non-read action",
      acceptEdits: "Undoable writes run free",
      auto: "Allow nearly all; only outside-roots & privacy ask",
    },
    setFailed: "Switch failed: {reason}",
  },} as const;
