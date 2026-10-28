import { copyFile, mkdir } from "node:fs/promises";

await mkdir("dist", { recursive: true });
await copyFile("index.html", "dist/index.html");
await copyFile("diff.html", "dist/diff.html");
await copyFile("src/style.css", "dist/style.css");
await copyFile("src/diff/style.css", "dist/diff.css");
