// fixtures/extensions/orchestrator-sample/index.mjs
function factory(pi) {
  pi.registerTool({
    name: "sample_echo",
    description: "Echo text through the A-track shim.",
    parameters: {
      type: "object",
      properties: { text: { type: "string" } },
      required: ["text"]
    },
    execute: async (_id, args) => ({
      content: [{ type: "text", text: "sample: " + String(args?.text ?? "") }],
      details: { source: "orchestrator-sample" }
    })
  });
  pi.registerCommand({
    name: "sample_events",
    description: "Emit two parallel subagents plus a workflow step.",
    handler: async () => {
      pi.emitEvent("subagent_spawned", { id: "sample-a", name: "探索", parentId: "" });
      pi.emitEvent("subagent_spawned", { id: "sample-b", name: "实现", parentId: "" });
      pi.workflowStep("实现", "running");
      pi.ui.notify("sample: two subagents started", "info");
      setTimeout(() => {
        pi.emitEvent("subagent_settled", { id: "sample-a" });
        pi.emitEvent("subagent_settled", { id: "sample-b" });
        pi.workflowStep("实现", "done");
      }, 20);
    }
  });
  pi.on("tool_execution_start", async (event) => {
    pi.workflowStep("观察工具", "running", String(event?.toolName ?? ""));
  });
  pi.on("tool_execution_end", async () => {
    pi.workflowStep("观察工具", "done");
  });
  pi.on("tool_call", async (event) => {
    if (event?.toolName === "danger" || String(event?.input?.command ?? "").includes("rm -rf")) {
      return { block: true, reason: "fixture blocks destructive commands" };
    }
    return void 0;
  });
}

// extensions/shim/host.mjs
import readline from "node:readline";
import { stdin, stdout } from "node:process";
var SHIM = "locowrk-shim/1";
function run(factory2, meta = {}) {
  const tools = /* @__PURE__ */ new Map();
  const commands = /* @__PURE__ */ new Map();
  const handlers = /* @__PURE__ */ new Map();
  const pending = /* @__PURE__ */ new Map();
  let nextID = 1;
  let closed = false;
  const send = (value) => {
    stdout.write(JSON.stringify(value) + "\n");
  };
  const notify = (method, params) => send({ jsonrpc: "2.0", method, params });
  const request = (method, params) => new Promise((resolve) => {
    const id = "shim-" + nextID++;
    pending.set(id, resolve);
    send({ jsonrpc: "2.0", id, method, params });
  });
  const diagnose = (member, message) => {
    const payload = {
      member,
      category: "unsupported",
      code: "unsupported",
      message
    };
    notify("diagnostic", payload);
  };
  const deferred = (member, message) => {
    notify("diagnostic", {
      member,
      category: "not_implemented",
      code: "not_implemented",
      message
    });
  };
  const silent = (member, fallback) => {
    diagnose(member, "terminal member has no desktop equivalent");
    return fallback;
  };
  const ctx = {
    mode: "rpc",
    hasUI: true,
    signal: void 0,
    isIdle: false,
    isProjectTrusted: false,
    model: void 0,
    thinkingLevel: void 0,
    sessionManager: {
      // The shim has no session entries on the wire yet; read-only callers get
      // an empty branch instead of throwing, and the loss is visible in the
      // report rather than as a handler error.
      getBranch: () => [],
      getEntries: () => [],
      getLeafId: () => void 0
    },
    ui: {
      notify: (message, type = "info") => notify("ui_notify", { message, type }),
      select: (title, options) => request("ui_select", { title, options }),
      confirm: (title, message) => request("ui_confirm", { title, message }),
      input: (title, placeholder = "") => request("ui_input", { title, placeholder }),
      editor: (title, prefill = "") => request("ui_editor", { title, prefill }),
      setWidget: () => silent("ui.setWidget", void 0),
      setFooter: () => silent("ui.setFooter", void 0),
      setHeader: () => silent("ui.setHeader", void 0),
      setTitle: () => silent("ui.setTitle", void 0),
      setStatus: () => silent("ui.setStatus", void 0),
      custom: () => silent("ui.custom", void 0),
      setEditorText: () => silent("ui.setEditorText", void 0),
      pasteToEditor: () => silent("ui.pasteToEditor", void 0)
    },
    executeTool: async () => {
      deferred("ctx.executeTool", "host tool execution is not implemented");
      return void 0;
    },
    compact: async () => {
      deferred("ctx.compact", "compaction is not implemented");
      return void 0;
    },
    shutdown: () => silent("ctx.shutdown", void 0)
  };
  const pi = {
    registerTool(nameOrTool, maybeTool) {
      const tool = typeof nameOrTool === "string" ? { name: nameOrTool, ...maybeTool } : nameOrTool;
      tools.set(tool.name, tool);
      notify("register_tool", {
        name: tool.name,
        description: tool.description ?? tool.name,
        parameters: tool.parameters ?? { type: "object", properties: {} },
        timeoutMs: tool.timeoutMs
      });
    },
    registerCommand(nameOrCommand, maybeCommand) {
      const command = typeof nameOrCommand === "string" ? { name: nameOrCommand, ...maybeCommand } : nameOrCommand;
      commands.set(command.name, command);
      notify("register_command", {
        name: command.name,
        description: command.description ?? command.name,
        args: command.args
      });
    },
    registerShortcut() {
      silent("registerShortcut", void 0);
    },
    on(event, handler) {
      const list = handlers.get(event) ?? [];
      list.push(handler);
      handlers.set(event, list);
      return () => {
        handlers.set(event, (handlers.get(event) ?? []).filter((h) => h !== handler));
      };
    },
    onSession() {
      return () => {
      };
    },
    ui: ctx.ui,
    sendMessage: () => deferred("pi.sendMessage", "host messaging is not implemented"),
    sendUserMessage: () => deferred("pi.sendUserMessage", "host messaging is not implemented"),
    appendEntry: () => deferred("pi.appendEntry", "session writes are not implemented"),
    emitEvent: (kind, payload) => notify("emit_event", { kind, payload }),
    workflowStep: (name, status, detail = "") => notify("workflow_step", { name, status, detail }),
    events: {
      emit: (name, payload) => notify("emit_event", { kind: name, payload })
    }
  };
  const dispatch = async (event, payload) => {
    const results = [];
    for (const handler of handlers.get(event) ?? []) {
      try {
        results.push(await handler({ ...payload, event }, ctx));
      } catch (err) {
        notify("diagnostic", {
          member: "on(" + event + ")",
          category: "unsupported",
          code: "handler_error",
          message: String(err)
        });
      }
    }
    return results;
  };
  const mergeEventResults = (results) => {
    let block = false;
    let cancel = false;
    let reason = "";
    let last;
    for (const result of results) {
      if (result === void 0 || result === null) continue;
      last = result;
      if (typeof result === "object") {
        if (result.block) {
          block = true;
          if (!reason && result.reason) reason = String(result.reason);
        }
        if (result.cancel) cancel = true;
      }
    }
    if (!block && !cancel) return last ?? null;
    const out = {};
    if (block) {
      out.block = true;
      out.reason = reason || "blocked by extension";
    }
    if (cancel) out.cancel = true;
    return out;
  };
  const invokeTool = async (params, respond) => {
    const tool = tools.get(params?.name);
    if (!tool) {
      respond({ jsonrpc: "2.0", id: params?.id, error: { code: 4401, message: "unknown tool" } });
      return;
    }
    try {
      const result = await tool.execute?.(
        params.toolCallId ?? "tool",
        params.arguments ?? {},
        void 0,
        void 0,
        ctx
      );
      respond({
        jsonrpc: "2.0",
        id: params.id,
        result: {
          content: result?.content ?? [{ type: "text", text: String(result ?? "") }],
          details: result?.details ?? void 0,
          isError: result?.isError ?? false
        }
      });
    } catch (err) {
      respond({ jsonrpc: "2.0", id: params.id, result: { content: [{ type: "text", text: String(err) }], isError: true } });
    }
  };
  const invokeCommand = async (params, respond) => {
    const command = commands.get(params?.commandId);
    if (!command) {
      respond({ jsonrpc: "2.0", id: params.id, error: { code: 4401, message: "unknown command" } });
      return;
    }
    try {
      await command.handler?.(params.args ?? "", ctx);
      respond({ jsonrpc: "2.0", id: params.id, result: null });
    } catch (err) {
      respond({ jsonrpc: "2.0", id: params.id, error: { code: 4401, message: String(err) } });
    }
  };
  const rl = readline.createInterface({ input: stdin });
  rl.on("line", async (line) => {
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      return;
    }
    if (msg.id && pending.has(msg.id)) {
      const resolve = pending.get(msg.id);
      pending.delete(msg.id);
      resolve(msg.result);
      return;
    }
    if (!msg.method) return;
    const respond = (value) => send(value);
    switch (msg.method) {
      case "initialize": {
        respond({
          jsonrpc: "2.0",
          id: msg.id,
          result: {
            protocol: 1,
            plugin: { name: meta.name ?? "extension", version: meta.version ?? "0.0.0" },
            capabilities: meta.capabilities ?? [],
            shim: SHIM,
            // The host may send gateable events (tool_call) as requests and
            // wait for the answer.
            wantsEventAck: true
          }
        });
        try {
          await factory2(pi);
        } catch (err) {
          notify("diagnostic", { member: "initialize", category: "unsupported", code: "factory_error", message: String(err) });
        }
        break;
      }
      case "invoke_tool":
        await invokeTool({ ...msg.params, id: msg.id }, respond);
        break;
      case "invoke_command":
        await invokeCommand({ ...msg.params, id: msg.id }, respond);
        break;
      case "ping":
        respond({ jsonrpc: "2.0", id: msg.id, result: "pong" });
        break;
      case "event": {
        const results = await dispatch(msg.params?.kind, msg.params?.payload ?? {});
        if (msg.id !== void 0) {
          respond({ jsonrpc: "2.0", id: msg.id, result: mergeEventResults(results) });
        }
        break;
      }
      case "abort":
        respond({ jsonrpc: "2.0", id: msg.id, result: null });
        break;
      case "shutdown_plugin":
        respond({ jsonrpc: "2.0", id: msg.id, result: null });
        closed = true;
        rl.close();
        setTimeout(() => process.exit(0), 10);
        break;
      default:
        if (msg.id) {
          respond({ jsonrpc: "2.0", id: msg.id, error: { code: 4400, message: "unsupported method " + msg.method } });
        }
    }
  });
  rl.on("close", () => {
    if (!closed) process.exit(0);
  });
}

// fixtures/extensions/orchestrator-sample/locowrk-entry.mjs
run(factory, { "name": "orchestrator-sample", "version": "0.1.0", "capabilities": [] });
