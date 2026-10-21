// locales/en/chat.ts —— 11 个命名域：chat / turn / ask / queue / composer / uncertain / notice / slash / idle / toast / notify
// 由批 F-22b 从 locales/en.ts 逐字节搬来；命名域必须**只在本文件**定义一次
// （index.ts 用对象展开合并，重复定义 = 后一份静默覆盖前一份，scripts/check-i18n.mjs 判红）。
export default {

  chat: {
    // Welcome greeting (§3.14 v3.9: four time-of-day states + indexed project
    // name; v2.60: greetingBare = no-project variant, project name hidden)
    greeting: {
      morning: "Good morning, {name}. Which task shall we start today?",
      afternoon: "Good afternoon, {name}. A good time to push one concrete thing forward.",
      evening: "Good evening, {name}. What do you want to wrap up tonight?",
      lateNight: "It's late, {name}. The rest can wait until tomorrow.",
    },
    greetingBare: {
      morning: "Good morning. Which task shall we start today?",
      afternoon: "Good afternoon. A good time to push one concrete thing forward.",
      evening: "Good evening. What do you want to wrap up tonight?",
      lateNight: "It's late. The rest can wait until tomorrow.",
    },
    placeholder: "Type / use commands · {'@'} reference files",
    send: "Send",
    copyMessage: "Copy this message",
    editMessage: "Edit and resend",
    copyCode: "Copy code",
    // Long-message fold component: circular expand/collapse button aria
    expandMessage: "Expand long message",
    collapseMessage: "Collapse long message",
    replaying: "Restoring session…",
    stopGeneration: "Stop generating",
    widthAria: "Conversation width {n} pixels",
    // v4.11 §3.39: send-button state machine + queue + back-to-latest
    placeholderQueue: "AI is working — your message will be queued",
    queueAdd: "Queue message",
    backToLatest: "Back to latest",
  },
  // Turn timer row (the container's only head row; AF-14 final: the group
  // header layer was removed)
  turn: {
    working: "Working {t}",
    worked: "Worked {t}",
    stopped: "Stopped",
    problemCount: "{n} problems",
  },
  // ask_user choice card (v4.11 §3.39①): blocking question, answered in dock
  ask: {
    cardTitle: "Your choice",
    queue: "{n} more pending",
    skip: "Skip",
    customPlaceholder: "Or type a custom reply…",
    send: "Send",
    submitFailed: "Submit failed: {reason}",
    timeoutHint: "Auto-skips in {n}s if unanswered",
  },
  // Queued-message card (v4.11 §3.39②): auto-drains one per finished turn
  queue: {
    cardTitle: "{n} queued",
    resume: "Resume",
    sendNow: "Send now",
    sendNowHint: "Stop the current run and send this now",
    remove: "Remove queued message",
    clearAll: "Clear queue",
    emptyPreview: "(empty message)",
  },
  composer: {
    attach: "Add files or images",
    attachHint: "images / PDF sent with the message · others as @path refs",
  },
  // G2 crash-recovery uncertain decision card (reliability doc §5)
  uncertain: {
    title: "Unknown tool outcome",
    detailMissing: "Call details missing (never written to the log)",
    desc: "The app exited during the last run. This call may have executed fully, partially, or not at all.",
    completed: "Mark as completed",
    notExecuted: "Mark as not executed",
    inspect: "Inspect & record…",
    notePlaceholder: "Record what you actually observed…",
    confirm: "Confirm",
    resolved: "Resolved",
    rCompleted: "completed",
    rNotExecuted: "not executed",
    rInspected: "inspection note",
  },
  // In-chat system messages (SystemNotice pattern): copy is UI, not
  // session data — the store keeps an i18n key, translated at render time
  // so locale switches apply to past notices too.
  notice: {
    action: {
      details: "View details",
      undo: "Undo",
      view: "View",
    },
    editResendFailed: "Edit & resend failed: {reason}",
    // ⑨ Compaction notice ([View details][Undo], Context doc §3.8.3)
    contextCompacted: "Compacted earlier messages, saved {saved} tokens",
    // S4 blade ② retired the organize done/paused keys with the memory face
    // (the frontend no longer subscribes to lw://organize-done / -paused).
    // S2 knife ⑦ `/compact`: the backend only returns the *name* of the criterion
    // that stopped it; the human sentence lives here. Names stay in sync with the
    // Rust side via check SS24, and an unrecognized name falls through to
    // compactUnknown with the name shown verbatim — never silently "compacted".
    compactRunning: "Compacting conversation context…",
    // specs/13: the failure taxonomy now mirrors upstream (criterion name ↔
    // sentence, same-source gate SS24 on both sides = 9 entries). "Not compacted"
    // has to say *why*: template not filled / no summary / output limit / cannot
    // shrink further / provider refused / content filter — six causes, six
    // sentences, never merged.
    compactNothingToCompact:
      "Not compacted: there was nothing to summarize (the conversation is too short, or the recent-text allowance already covers the whole history).",
    compactTemplateMismatch:
      "Not compacted: the summary did not follow the required format (it was reminded once and still did not match), so nothing was recorded.",
    compactNoSummary: "Not compacted: the model produced no summary.",
    compactOutputLimit: "Not compacted: the summary hit the output token limit.",
    compactCannotShrink:
      "Not compacted: shrinking further would drop the latest exchange, so it gave up.",
    compactProviderError:
      "Not compacted: the provider refused the summary request (network or quota).",
    compactContentFilter:
      "Not compacted: the provider's content filter blocked the summary.",
    compactInterrupted: "Compaction stopped.",
    // specs/12 knife ①: the summary call ran past its total time budget (the
    // hard ceiling on the auxiliary path). Kept apart from provider-error:
    // that one is "it did not answer", this one is "it did not get there".
    compactBudget:
      "Not compacted: the summary call exceeded its time budget (adjust \"Summary call budget\" in Settings → Context).",
    // specs/12 knife ②: the breaker tripped (compaction kept failing, so
    // automatic compaction pauses while the turn carries on). This one rides
    // the *phase* (`paused` on agent://compact-status), not the criterion-name
    // table — the name `fused` is retired as of specs/13 (the two sides had
    // different scopes for it, so neither keeps it).
    compactPaused:
      "Automatic compaction is paused: it failed several times in a row, so this turn will not compact on its own. The manual `/压缩` command still works.",
    compactUnknown: "Not compacted: {reason}",
    compactUndone:
      "Compaction undone: full pre-compaction context restored (later turns were rewound too).",
    compactUndoFailed: "Failed to undo compaction: {reason}",
    // ⑭ Scheduled task fired (announced before the injected turn runs)
    scheduleFired: "Scheduled task fired: {title}",
    // LLM.md v3.0: transport retry / step cap
    llmRetried: "Provider temporarily unresponsive, retried automatically (attempt {attempt})",
    turnCapped: "Turn step limit reached ({steps} steps), ending the turn here — send \"continue\" to resume, or raise the limit in Settings → Tools & Dispatch",
  },
  // Slash command panel (v2.143, §3.36① extension): list shares the
  // registry with the submit intercept (lib/slash-commands.ts)
  slash: {
    title: "Commands",
    hint: "↑↓ to navigate, Enter or click to run, Esc to close",
    none: "No matching commands",
    desc: {
      compact: "Compact the context right now (ignores the trigger line and the breaker; undoable)",
    },
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
  // ⑫ Exposure debug panel (read-only part of developer options; separate
  // from settings.developer* — this is "user comes looking" info)
  toast: {
    exported: "Exported",
    exportFailed: "Export failed",
    deleted: "Session deleted",
    deleteFailed: "Delete failed",
    renameFailed: "Rename failed",
    projectAdded: "Project added",
    projectAddFailed: "Failed to add project",
    projectRemoved: "Project removed",
    projectRemoveFailed: "Failed to remove project",
    revealFailed: "Failed to open file manager",
  },
  // Notification popover (§3.13, v2.51): records store kind + params only,
  // title templates render here
  notify: {
    title: "Notifications",
    markAll: "Mark all read",
    clear: "Clear all",
    all: "All",
    unread: "Unread",
    empty: "No notifications",
    emptyUnread: "Nothing unread",
    noContext: "No linked chat",
    scheduleFired: "Scheduled task fired",
    subagentDone: "Sub-agent finished",
    subagentFailed: "Sub-agent ended with failure",
    contextCompacted: "Context compacted",
    savedTokens: "{n} tokens saved",
    approvalTimeout: "Approval timed out, denied automatically",
  },
} as const;
