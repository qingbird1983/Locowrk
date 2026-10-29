// locowrk-shim/1: the Node side of the A-track extension protocol.
//
// It is bundled into an extension's entrypoint. The host sends JSONL JSON-RPC
// on stdin/stdout; this file exposes the supported pi ExtensionAPI subset and
// never throws for a missing TUI member.
import readline from "node:readline";
import { stdin, stdout } from "node:process";

const SHIM = "locowrk-shim/1";

export function run(factory, meta = {}) {
  const tools = new Map();
  const commands = new Map();
  const handlers = new Map();
  const pending = new Map();
  let nextID = 1;
  let closed = false;

  const send = (value) => {
    stdout.write(JSON.stringify(value) + "\n");
  };
  const notify = (method, params) => send({ jsonrpc: "2.0", method, params });
  const request = (method, params) =>
    new Promise((resolve) => {
      const id = "shim-" + nextID++;
      pending.set(id, resolve);
      send({ jsonrpc: "2.0", id, method, params });
    });

  const diagnose = (member, message) => {
    const payload = {
      member,
      category: "unsupported",
      code: "unsupported",
      message,
    };
    notify("diagnostic", payload);
  };
  const deferred = (member, message) => {
    notify("diagnostic", {
      member,
      category: "not_implemented",
      code: "not_implemented",
      message,
    });
  };
  const silent = (member, fallback) => {
    diagnose(member, "terminal member has no desktop equivalent");
    return fallback;
  };

  const ctx = {
    mode: "rpc",
    hasUI: true,
    signal: undefined,
    isIdle: false,
    isProjectTrusted: false,
    model: undefined,
    thinkingLevel: undefined,
    sessionManager: {
      // The shim has no session entries on the wire yet; read-only callers get
      // an empty branch instead of throwing, and the loss is visible in the
      // report rather than as a handler error.
      getBranch: () => [],
      getEntries: () => [],
      getLeafId: () => undefined,
    },
    ui: {
      notify: (message, type = "info") => notify("ui_notify", { message, type }),
      select: (title, options) => request("ui_select", { title, options }),
      confirm: (title, message) => request("ui_confirm", { title, message }),
      input: (title, placeholder = "") => request("ui_input", { title, placeholder }),
      editor: (title, prefill = "") => request("ui_editor", { title, prefill }),
      setWidget: () => silent("ui.setWidget", undefined),
      setFooter: () => silent("ui.setFooter", undefined),
      setHeader: () => silent("ui.setHeader", undefined),
      setTitle: () => silent("ui.setTitle", undefined),
      setStatus: () => silent("ui.setStatus", undefined),
      custom: () => silent("ui.custom", undefined),
      setEditorText: () => silent("ui.setEditorText", undefined),
      pasteToEditor: () => silent("ui.pasteToEditor", undefined),
    },
    executeTool: async () => {
      deferred("ctx.executeTool", "host tool execution is not implemented");
      return undefined;
    },
    compact: async () => {
      deferred("ctx.compact", "compaction is not implemented");
      return undefined;
    },
    shutdown: () => silent("ctx.shutdown", undefined),
  };

  const pi = {
    registerTool(nameOrTool, maybeTool) {
      const tool = typeof nameOrTool === "string" ? { name: nameOrTool, ...maybeTool } : nameOrTool;
      tools.set(tool.name, tool);
      notify("register_tool", {
        name: tool.name,
        description: tool.description ?? tool.name,
        parameters: tool.parameters ?? { type: "object", properties: {} },
        timeoutMs: tool.timeoutMs,
      });
    },
    registerCommand(nameOrCommand, maybeCommand) {
      const command =
        typeof nameOrCommand === "string" ? { name: nameOrCommand, ...maybeCommand } : nameOrCommand;
      commands.set(command.name, command);
      notify("register_command", {
        name: command.name,
        description: command.description ?? command.name,
        args: command.args,
      });
    },
    registerShortcut() {
      silent("registerShortcut", undefined);
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
      return () => {};
    },
    ui: ctx.ui,
    sendMessage: () => deferred("pi.sendMessage", "host messaging is not implemented"),
    sendUserMessage: () => deferred("pi.sendUserMessage", "host messaging is not implemented"),
    appendEntry: () => deferred("pi.appendEntry", "session writes are not implemented"),
    emitEvent: (kind, payload) => notify("emit_event", { kind, payload }),
    workflowStep: (name, status, detail = "") => notify("workflow_step", { name, status, detail }),
    events: {
      emit: (name, payload) => notify("emit_event", { kind: name, payload }),
    },
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
          message: String(err),
        });
      }
    }
    return results;
  };

  // mergeEventResults folds handler answers into one event_result: a block
  // wins (first reason kept), then a cancel, then any plain value.
  const mergeEventResults = (results) => {
    let block = false;
    let cancel = false;
    let reason = "";
    let last;
    for (const result of results) {
      if (result === undefined || result === null) continue;
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
        undefined,
        undefined,
        ctx,
      );
      respond({
        jsonrpc: "2.0",
        id: params.id,
        result: {
          content: result?.content ?? [{ type: "text", text: String(result ?? "") }],
          details: result?.details ?? undefined,
          isError: result?.isError ?? false,
        },
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
            wantsEventAck: true,
          },
        });
        try {
          await factory(pi);
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
        if (msg.id !== undefined) {
          respond({ jsonrpc: "2.0", id: msg.id, result: mergeEventResults(results) });
        }
        break;
      }
      case "abort":
        // Extensions with long-running work stop timers here; the reply is sent
        // after handlers run so the host does not race the cleanup.
        await dispatch("abort", msg.params?.payload ?? {});
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
