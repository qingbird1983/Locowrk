// Minimal node:os stand-in for the neutral-platform bundle.

export function tmpdir() {
  return "/tmp";
}

export function homedir() {
  return "/home/spike";
}

export default { tmpdir, homedir };
