# Locowrk

English | [简体中文](README.zh-CN.md)

**A native desktop workbench for [pi](https://github.com/earendil-works/pi), built on a pure-Go kernel.**

Locowrk keeps the pure-Go agent kernel [`sky-valley/pi`](https://github.com/sky-valley/pi) **in this repository** as the `pi/` fork (MIT), so the kernel can evolve with the product instead of waiting on an external module. The desktop host is built with [MyGo](https://github.com/egoist/mygo). pi's behaviour remains the blueprint for unmodified kernel code; TUI-only extension calls are kept out of the desktop; plugin interfaces have two first-class rendering paths: native `ui.View` and the system WebView.

- **Kernel in-repo**: agent loop, LLM providers, tools, sessions, skills and compaction live in `pi/`, forked from `sky-valley/pi` (MIT) at a recorded upstream commit and maintained here; `internal/kernel` is the only package that talks to it.
- **Orchestration compatibility**: pi extensions that only register tools, commands and event hooks are adapted; terminal UI calls degrade to no-ops with diagnostics.
- **Desktop plugins**: native Go views for structured visualisation, WebView panels for rich interaction, and subprocess isolation for third-party plugins.
- **Documentation first**: architecture, decisions (ADRs), standards and phase acceptance criteria are written before implementation.

Progress and current state live in the [roadmap](docs/roadmap/00-roadmap.md) and the [changelog](CHANGELOG.md); this page only describes the project.

## Start here

| What you need | Where |
|---|---|
| Documentation map | [docs/README.md](docs/README.md) |
| Roadmap and progress board | [docs/roadmap/00-roadmap.md](docs/roadmap/00-roadmap.md) |
| Decisions and rationale | [docs/adr/](docs/adr/) |
| Rules for working here | [docs/standards/](docs/standards/) |
| AI session handoff | [.agents/CONTEXT.md](.agents/CONTEXT.md) |
| Change log | [CHANGELOG.md](CHANGELOG.md) |
| Glossary | [docs/glossary.md](docs/glossary.md) |

## Repository layout

| Path | Contents |
|---|---|
| `docs/` | The single source of truth: research, ADRs, architecture, roadmap, standards, phase reports |
| `.agents/` | Working memory for AI sessions: handoff card, pitfalls, reusable methods |
| `cmd/` | Desktop executable (created in Phase 3) |
| `internal/` | Kernel wrapper, event bus, plugin adapter, skills, host bridge (created in Phase 1) |
| `desktop/` | Plugin host and desktop plugins (created in Phase 3) |
| `proto/` | Subprocess plugin protocol (created in Phase 2) |

## License

[MIT](LICENSE). The `pi/` kernel is forked from `sky-valley/pi` (MIT); MyGo is MIT.
