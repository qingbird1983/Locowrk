// node:fs stand-in for the neutral-platform bundle. File access is a declared
// capability in the real adapter, so the spike reports it instead of faking it.

function unavailable(name) {
  return (..._args) => Promise.reject(new Error("not_implemented: node:fs." + name));
}

export const promises = {
  mkdtemp: unavailable("promises.mkdtemp"),
  readFile: unavailable("promises.readFile"),
  writeFile: unavailable("promises.writeFile"),
  rm: unavailable("promises.rm"),
  mkdir: unavailable("promises.mkdir"),
  readdir: unavailable("promises.readdir"),
  unlink: unavailable("promises.unlink"),
};

export function readFileSync() {
  throw new Error("not_implemented: node:fs.readFileSync");
}

export function existsSync() {
  return false;
}

export default { promises, readFileSync, existsSync };
