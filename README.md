# Locowrk

**A native desktop workbench for [pi](https://github.com/earendil-works/pi), built on a pure-Go kernel.**

Locowrk reuses the pure-Go port [`sky-valley/pi`](https://github.com/sky-valley/pi) as its agent kernel and adds a desktop host built with [MyGo](https://github.com/egoist/mygo). It keeps pi's behaviour as the contract, keeps TUI-only extension calls out of the desktop, and gives plugin interfaces two first-class rendering paths: native `ui.View` and the system WebView.

- **Kernel, not a re-port**: agent loop, LLM providers, tools, sessions, skills and compaction come from `sky-valley/pi`.
- **Orchestration compatibility**: pi extensions that only register tools, commands and event hooks are adapted; terminal UI calls degrade to no-ops with diagnostics.
- **Desktop plugins**: native Go views for structured visualisation, WebView panels for rich interaction, and subprocess isolation for third-party plugins.
- **Documentation first**: architecture, decisions (ADRs), standards and phase acceptance criteria are written before implementation.

This repository currently contains the documentation baseline. Implementation starts at Phase 1; see the [roadmap](docs/roadmap/00-roadmap.md).

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

[MIT](LICENSE). pi and `sky-valley/pi` are MIT; MyGo is MIT.
