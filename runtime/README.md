# Node runtime (not tracked by git)

`runtime/node/` holds the pinned Node runtime that ships with a release
(ADR-0015). It is fetched by `scripts/fetch-node-runtime.mjs`; CI and local
builds verify the checksum below before use.

| Field | Value |
|---|---|
| Version | `v26.13.0` |
| Official source | https://nodejs.org/dist/v26.13.0/node-v26.13.0-win-x64.zip |
| Downloaded from | https://mirrors.ustc.edu.cn/node/v26.13.0/node-v26.13.0-win-x64.zip |
| SHA-256 | `dcc60d59a0746999f99064a01c020445d54d84228c691cdbf25b49ac3b59150a` |
| Binary | `runtime/node/node.exe` |
| License | Node.js (MIT); see `runtime/node/LICENSE` |

Reproduce:

```bash
node scripts/fetch-node-runtime.mjs --version v26.13.0
```

The launcher resolves this directory before PATH; a release build without it
keeps the A track disabled (fail-closed) instead of falling back silently.
