// Shared shim runtime for the Phase 2 runtime spike.
//
// This is deliberately not the production adapter: it exists to answer one
// question - can a pi extension's real code run on a given runtime, and what
// does the host have to supply? Every member is tagged in TABLE so the run
// reports "used but missing" instead of pretending to be compatible.
//
// Statuses: supported (real), stub (safe zero value + diagnostic), noop
// (terminal-UI member ignored + diagnostic), deferred (returns a rejected-free
// promise of a zero value + diagnostic), property (plain value).

const used = new Set();
const missing = new Set();
const diagnostics = new Map();
let tools = new Map();
let commands = new Map();
let handlers = new Map();
let opts = { cwd: ".", mode: "rpc" };
let bootDone = false;
let bootError = null;
let runDone = false;
let runResult;

function send(kind, payload) {
  globalThis.__hostSend(kind, JSON.stringify(payload));
}

function diag(member, code, message) {
  const key = member + "|" + code;
  const prev = diagnostics.get(key);
  if (prev) {
    prev.count += 1;
    send("diagnostic", prev);
    return;
  }
  const entry = { member, code, message, count: 1 };
  diagnostics.set(key, entry);
  send("diagnostic", entry);
}

// status: supported | noop | deferred | property
const TABLE = {
  "api.on": "supported",
  "api.registerTool": "supported",
  "api.registerCommand": "supported",
  "api.registerShortcut": "noop",
  "api.registerFlag": "deferred",
  "api.getFlag": "stub",
  "api.registerMessageRenderer": "noop",
  "api.registerMarkdownTransformer": "noop",
  "api.registerEntryRenderer": "noop",
  "api.registerToolRenderer": "noop",
  "api.sendMessage": "deferred",
  "api.sendUserMessage": "deferred",
  "api.appendEntry": "deferred",
  "api.setSessionName": "deferred",
  "api.getSessionName": "stub",
  "api.setLabel": "deferred",
  "api.exec": "deferred",
  "api.getActiveTools": "stub",
  "api.getAllTools": "stub",
  "api.getSettings": "stub",
  "api.setActiveTools": "deferred",
  "api.getCommands": "stub",
  "api.registerProvider": "deferred",
  "api.registerMcpServer": "deferred",
  "api.unregisterMcpServer": "deferred",
  "api.registerVirtualModel": "deferred",
  "api.setModel": "deferred",
  "api.getModel": "stub",
  "api.setThinkingLevel": "deferred",
  "api.getThinkingLevel": "stub",
  "api.events": "property",
  "api.reload": "deferred",

  "ctx.ui": "property",
  "ctx.mode": "property",
  "ctx.hasUI": "property",
  "ctx.cwd": "property",
  "ctx.sessionManager": "property",
  "ctx.modelRegistry": "property",
  "ctx.model": "property",
  "ctx.thinkingLevel": "property",
  "ctx.isIdle": "supported",
  "ctx.isProjectTrusted": "supported",
  "ctx.signal": "property",
  "ctx.abort": "supported",
  "ctx.hasPendingMessages": "supported",
  "ctx.shutdown": "supported",
  "ctx.getContextUsage": "stub",
  "ctx.compact": "deferred",
  "ctx.getSystemPrompt": "deferred",
  "ctx.getSystemPromptOptions": "deferred",
  "ctx.waitForIdle": "deferred",
  "ctx.newSession": "deferred",
  "ctx.fork": "deferred",
  "ctx.navigateTree": "deferred",
  "ctx.switchSession": "deferred",
  "ctx.reload": "deferred",
  "ctx.executeTool": "deferred",

  "ui.select": "supported",
  "ui.confirm": "supported",
  "ui.input": "supported",
  "ui.notify": "supported",
  "ui.onTerminalInput": "noop",
  "ui.setStatus": "noop",
  "ui.setWorkingMessage": "noop",
  "ui.setWorkingVisible": "noop",
  "ui.setWorkingIndicator": "noop",
  "ui.setHiddenThinkingLabel": "noop",
  "ui.setWidget": "noop",
  "ui.setFooter": "noop",
  "ui.setHeader": "noop",
  "ui.setTitle": "noop",
  "ui.custom": "noop",
  "ui.pasteToEditor": "noop",
  "ui.setEditorText": "noop",
  "ui.getEditorText": "stub",
  "ui.editor": "supported",
  "ui.addAutocompleteProvider": "noop",
  "ui.setEditorComponent": "noop",
  "ui.getEditorComponent": "noop",
  "ui.theme": "property",
  "ui.getAllThemes": "stub",
  "ui.getTheme": "stub",
  "ui.setTheme": "deferred",
  "ui.getToolsExpanded": "stub",
  "ui.setToolsExpanded": "deferred",

  "session.getBranch": "stub",
  "session.getLeafEntry": "stub",
  "session.getEntries": "stub",
  "session.getSessionFile": "stub",
};

const ZERO = {
  "ui.select": undefined,
  "ui.confirm": false,
  "ui.input": undefined,
  "ui.editor": undefined,
  "ui.getEditorText": "",
  "ui.getAllThemes": [],
  "ui.getEditorComponent": undefined,
  "ui.getToolsExpanded": false,
  "ctx.getContextUsage": undefined,
  "session.getBranch": [],
  "session.getLeafEntry": undefined,
  "session.getEntries": [],
  "session.getSessionFile": undefined,
  "api.getActiveTools": [],
  "api.getAllTools": [],
  "api.getCommands": [],
  "api.getSettings": {},
  "api.getFlag": undefined,
  "api.getSessionName": undefined,
  "api.getModel": undefined,
  "api.getThinkingLevel": undefined,
  "ui.theme": undefined,
};

function statusOf(member) {
  const s = TABLE[member];
  if (!s) {
    missing.add(member);
    return "missing";
  }
  return s;
}

function record(member) {
  used.add(member);
  return statusOf(member);
}

function fail(member) {
  const st = record(member);
  const code = st === "deferred" || st === "stub" ? "not_implemented" : "unsupported";
  diag(member, code, member);
  const zero = ZERO[member];
  if (st === "deferred") return Promise.resolve(zero);
  return zero;
}

function fn(member, impl) {
  return function (...args) {
    record(member);
    const st = TABLE[member];
    if (st === "supported" && impl) return impl(...args);
    return fail(member);
  };
}

function prop(member, value) {
  record(member);
  return value;
}

function makeTheme() {
  const id = (s) => s;
  return { fg: (_c, s) => s, bg: (_c, s) => s, bold: id, dim: id, italic: id, underline: id };
}

function makeUI() {
  const dialogs = {
    defaultSelect: "Yes",
  };
  return {
    select: fn("ui.select", async (_title, options) => (options && options[0]) || dialogs.defaultSelect),
    confirm: fn("ui.confirm", async () => true),
    input: fn("ui.input", async () => "probe-input"),
    notify: fn("ui.notify", (message, type) => {
      send("notify", { message: String(message), type: type || "info" });
    }),
    setWidget: fn("ui.setWidget"),
    setStatus: fn("ui.setStatus"),
    setTitle: fn("ui.setTitle"),
    setFooter: fn("ui.setFooter"),
    setHeader: fn("ui.setHeader"),
    setWorkingMessage: fn("ui.setWorkingMessage"),
    setWorkingVisible: fn("ui.setWorkingVisible"),
    setWorkingIndicator: fn("ui.setWorkingIndicator"),
    setHiddenThinkingLabel: fn("ui.setHiddenThinkingLabel"),
    custom: fn("ui.custom"),
    onTerminalInput: fn("ui.onTerminalInput", () => () => {}),
    pasteToEditor: fn("ui.pasteToEditor"),
    setEditorText: fn("ui.setEditorText"),
    getEditorText: fn("ui.getEditorText"),
    editor: fn("ui.editor"),
    addAutocompleteProvider: fn("ui.addAutocompleteProvider"),
    setEditorComponent: fn("ui.setEditorComponent"),
    getEditorComponent: fn("ui.getEditorComponent"),
    getAllThemes: fn("ui.getAllThemes"),
    getTheme: fn("ui.getTheme"),
    setTheme: fn("ui.setTheme"),
    getToolsExpanded: fn("ui.getToolsExpanded"),
    setToolsExpanded: fn("ui.setToolsExpanded"),
    get theme() {
      return prop("ui.theme", makeTheme());
    },
  };
}

function makeSessionManager() {
  return {
    getBranch: fn("session.getBranch"),
    getLeafEntry: fn("session.getLeafEntry"),
    getEntries: fn("session.getEntries"),
    getSessionFile: fn("session.getSessionFile"),
  };
}

function makeCtx(extra) {
  const session = makeSessionManager();
  return new Proxy(
    {
      ui: prop("ctx.ui", makeUI()),
      mode: prop("ctx.mode", opts.mode),
      hasUI: prop("ctx.hasUI", true),
      cwd: prop("ctx.cwd", opts.cwd),
      sessionManager: prop("ctx.sessionManager", session),
      modelRegistry: prop("ctx.modelRegistry", { streamSimple: fn("ctx.modelRegistry.streamSimple") }),
      model: prop("ctx.model", undefined),
      thinkingLevel: prop("ctx.thinkingLevel", undefined),
      isIdle: fn("ctx.isIdle", () => true),
      isProjectTrusted: fn("ctx.isProjectTrusted", () => true),
      signal: prop("ctx.signal", undefined),
      abort: fn("ctx.abort", () => {}),
      hasPendingMessages: fn("ctx.hasPendingMessages", () => false),
      shutdown: fn("ctx.shutdown", () => {}),
      getContextUsage: fn("ctx.getContextUsage"),
      compact: fn("ctx.compact"),
      getSystemPrompt: fn("ctx.getSystemPrompt"),
      ...extra,
    },
    {
      get(target, key) {
        if (key in target) return target[key];
        if (typeof key === "symbol") return undefined;
        record("ctx." + String(key));
        return undefined;
      },
    },
  );
}

function makeCommandCtx() {
  return makeCtx({
    getSystemPromptOptions: fn("ctx.getSystemPromptOptions"),
    waitForIdle: fn("ctx.waitForIdle"),
    newSession: fn("ctx.newSession"),
    fork: fn("ctx.fork"),
    navigateTree: fn("ctx.navigateTree"),
    switchSession: fn("ctx.switchSession"),
    reload: fn("ctx.reload"),
    executeTool: fn("ctx.executeTool"),
  });
}

function makeAPI() {
  const api = {
    on: fn("api.on", (event, handler) => {
      if (!handlers.has(event)) handlers.set(event, []);
      handlers.get(event).push(handler);
      return () => {
        handlers.set(event, (handlers.get(event) || []).filter((h) => h !== handler));
      };
    }),
    registerTool: fn("api.registerTool", (def) => {
      if (!def || !def.name) throw new Error("registerTool requires a name");
      tools.set(def.name, def);
    }),
    registerCommand: fn("api.registerCommand", (name, def) => {
      commands.set(name, def);
    }),
    registerShortcut: fn("api.registerShortcut"),
    registerFlag: fn("api.registerFlag"),
    getFlag: fn("api.getFlag"),
    registerMessageRenderer: fn("api.registerMessageRenderer"),
    registerMarkdownTransformer: fn("api.registerMarkdownTransformer"),
    registerEntryRenderer: fn("api.registerEntryRenderer"),
    registerToolRenderer: fn("api.registerToolRenderer"),
    sendMessage: fn("api.sendMessage"),
    sendUserMessage: fn("api.sendUserMessage"),
    appendEntry: fn("api.appendEntry"),
    setSessionName: fn("api.setSessionName"),
    getSessionName: fn("api.getSessionName"),
    setLabel: fn("api.setLabel"),
    exec: fn("api.exec"),
    getActiveTools: fn("api.getActiveTools"),
    getAllTools: fn("api.getAllTools"),
    getSettings: fn("api.getSettings"),
    setActiveTools: fn("api.setActiveTools"),
    getCommands: fn("api.getCommands"),
    registerProvider: fn("api.registerProvider"),
    registerMcpServer: fn("api.registerMcpServer"),
    unregisterMcpServer: fn("api.unregisterMcpServer"),
    registerVirtualModel: fn("api.registerVirtualModel"),
    setModel: fn("api.setModel"),
    getModel: fn("api.getModel"),
    setThinkingLevel: fn("api.setThinkingLevel"),
    getThinkingLevel: fn("api.getThinkingLevel"),
    reload: fn("api.reload"),
    get events() {
      return prop("api.events", { on: () => () => {}, emit: () => {} });
    },
  };
  return new Proxy(api, {
    get(target, key) {
      if (key in target) return target[key];
      if (typeof key === "symbol") return undefined;
      record("api." + String(key));
      return undefined;
    },
  });
}

function formatError(e) {
  if (!e) return "unknown error";
  const message = e.message ? String(e.message) : "";
  const stack = e.stack ? String(e.stack) : String(e);
  return message && !stack.includes(message) ? message + "\n" + stack : stack;
}

async function dispatch(kind, name, payload) {  if (kind === "event") {
    const list = handlers.get(name) || [];
    const ctx = makeCtx();
    for (const h of list) await h(payload, ctx);
    return null;
  }
  if (kind === "tool") {
    const def = tools.get(name);
    if (!def) throw new Error("unknown tool: " + name);
    const ctx = makeCtx({ executeTool: fn("ctx.executeTool") });
    return await def.execute("probe-call-1", payload, undefined, undefined, ctx);
  }
  if (kind === "command") {
    const def = commands.get(name);
    if (!def) throw new Error("unknown command: " + name);
    return await def.handler("", makeCommandCtx());
  }
  throw new Error("unknown dispatch kind: " + kind);
}

function coverage() {
  const implemented = [];
  const handled = [];
  const absent = [];
  for (const member of used) {
    const st = TABLE[member];
    if (st === "supported") implemented.push(member);
    if (st) handled.push(member);
    else absent.push(member);
  }
  return {
    used: [...used].sort(),
    implemented: implemented.sort(),
    handled: handled.sort(),
    missing: absent.sort(),
    diagnostics: [...diagnostics.values()],
  };
}

globalThis.__locowrk = {
  boot(optsJSON) {
    opts = JSON.parse(optsJSON || "{}");
    bootDone = false;
    bootError = null;
    try {
      const factory = globalThis.__locowrkExtension;
      if (typeof factory !== "function") throw new Error("bundle exported no extension factory");
      const api = makeAPI();
      Promise.resolve(factory(api))
        .then(() => {
          bootDone = true;
        })
        .catch((err) => {
          bootError = String(err && err.stack ? err.stack : err);
          send("diagnostic", { member: "boot", code: "crashed", message: bootError, count: 1 });
          bootDone = true;
        });
    } catch (err) {
      bootError = String(err && err.stack ? err.stack : err);
      send("diagnostic", { member: "boot", code: "crashed", message: bootError, count: 1 });
      bootDone = true;
    }
  },
  describe() {
    return JSON.stringify({
      tools: [...tools.keys()],
      commands: [...commands.keys()],
      events: [...handlers.keys()],
      coverage: coverage(),
      bootError,
    });
  },
  dispatchPromise(kind, name, payloadJSON) {
    return dispatch(kind, name, JSON.parse(payloadJSON || "null")).then((v) => JSON.stringify({ ok: true, value: v }), (e) => JSON.stringify({ ok: false, error: formatError(e) }));
  },
  start(kind, name, payloadJSON) {
    runDone = false;
    runResult = undefined;
    this.dispatchPromise(kind, name, payloadJSON).then((r) => {
      runResult = r;
      runDone = true;
    });
  },
  runDone() {
    return runDone;
  },
  runResult() {
    return runResult || JSON.stringify({ ok: false, error: "no result" });
  },
  bootDone() {
    return bootDone;
  },
  coverage() {
    return JSON.stringify(coverage());
  },
};

// Flat globals so a Go host can call them by name without object plumbing.
globalThis.__locowrkBoot = (optsJSON) => globalThis.__locowrk.boot(optsJSON);
globalThis.__locowrkDescribe = () => globalThis.__locowrk.describe();
globalThis.__locowrkStart = (kind, name, payloadJSON) => globalThis.__locowrk.start(kind, name, payloadJSON);
globalThis.__locowrkBootDone = () => globalThis.__locowrk.bootDone();
globalThis.__locowrkRunDone = () => globalThis.__locowrk.runDone();
globalThis.__locowrkRunResult = () => globalThis.__locowrk.runResult();

