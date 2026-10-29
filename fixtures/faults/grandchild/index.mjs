// Fault fixture: spawns a long-lived child process that appends to a file, so a
// test can prove the host takes descendants down when the extension crashes.
import { spawn } from "node:child_process";

export default function factory(pi) {
  const file = process.env.LOCOWRK_FAULT_HEARTBEAT ?? "";
  if (file) {
    const script = `const fs = require("node:fs"); setInterval(() => fs.appendFileSync(${JSON.stringify(file)}, "tick\\n"), 50);`;
    spawn(process.execPath, ["-e", script], { stdio: "ignore" });
  }

  pi.registerCommand({
    name: "crash",
    description: "Exit the extension while the child keeps ticking.",
    handler: async () => {
      process.exit(9);
    },
  });
}
