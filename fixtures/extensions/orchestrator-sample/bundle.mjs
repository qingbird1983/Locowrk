// Prebuilt fixture entrypoint. Production packages bundle the shim with
// esbuild; this fixture imports the repository shim directly.
import { run } from "../../../extensions/shim/host.mjs";
import factory from "./index.mjs";

run(factory, { name: "orchestrator-sample", version: "0.1.0", capabilities: [] });
