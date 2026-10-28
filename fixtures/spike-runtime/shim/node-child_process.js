// node:child_process stand-in for the neutral-platform bundle. Spawning is a
// declared capability in the real adapter; here it fails fast so the run
// records "this path needs a host capability", not a hang.

export function spawn(_command, _args, _options) {
  const listeners = new Map();
  const stream = { on: () => stream, once: () => stream };
  const proc = {
    stdout: stream,
    stderr: stream,
    stdin: { write: () => {}, end: () => {} },
    on(event, cb) {
      listeners.set(event, cb);
      if (event === "error") {
        queueMicrotask(() => cb(new Error("not_implemented: node:child_process.spawn")));
      }
      return proc;
    },
    once(event, cb) {
      return proc.on(event, cb);
    },
    kill: () => {},
  };
  return proc;
}

export function execFile() {
  return Promise.reject(new Error("not_implemented: node:child_process.execFile"));
}

export default { spawn, execFile };
