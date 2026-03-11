# Zellij Project Sidebar

## What This Is

A Zellij plugin that provides a docked sidebar panel for managing project sessions. Unlike existing floating popup plugins (sessionizer, choose-tree), this is a persistent side panel — always accessible via keybind — that shows your pinned project folders with live session status and lets you switch, create, or kill sessions with a single keypress. Think VS Code's sidebar, but for terminal sessions.

## Core Value

One-keypress project switching with always-visible session awareness — you should never have to think about session management, just glance and jump.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Pinned project list from configuration (explicit folder paths)
- [ ] Live session status per project (running / exited / not started)
- [ ] Switch to existing session on selection (Enter)
- [ ] Create new session from folder if none exists
- [ ] Kill session from sidebar (x key)
- [ ] Docked side panel (not floating popup) — left side, fixed width
- [ ] Toggle visibility with keybind (Cmd+P via Ghostty passthrough)
- [ ] Tab count display per session (e.g. `help-self [3]`)
- [ ] Active pane command display (what's running in focused pane)
- [ ] Configurable info verbosity — minimal (name + dot) through full (tabs + active command)
- [ ] Keyboard navigation (j/k, Enter to switch, x to kill)
- [ ] Visual indicator for current/active session
- [ ] Catppuccin Frappe theme integration (match user's Zellij theme)

### Out of Scope

- Fuzzy search / directory scanning — sessionizer already does this, this plugin is for pinned projects
- Rename session — low value, can do via CLI
- Apply custom layout per project — future feature, not v1
- Tab/pane management within sessions — choose-tree handles this
- Mouse interaction — keyboard-first for v1
- Multi-theme support — hardcode Catppuccin Frappe for now, themeable later
- Session resurrection / state persistence — Zellij handles this natively

## Context

### Ecosystem

- **Zellij plugin API**: Rust → WASM (wasm32-wasi). Official template: `zellij-org/create-rust-plugin`. Plugin API provides `switch_session()`, session listing, keyboard/mouse events, and character-level rendering.
- **Existing plugins**: `zellij-sessionizer` (fuzzy dir search, floating), `zellij-choose-tree` (session tree, floating), `zellij-attention` (already installed). None provide a persistent docked sidebar.
- **User's setup**: Ghostty + Zellij with Catppuccin Frappe theme. CMD keys unbound in Ghostty and passed as Super to Zellij. Current keybinds: Cmd+T (new tab), Cmd+N (new pane), Cmd+W (close), Cmd+F (float toggle), Cmd+E (yazi), Cmd+S (sessionizer), Cmd+D (choose-tree).
- **Available keybind**: Cmd+P for toggle (Super p in Zellij config).

### User's project structure

Projects live under `~/Documents/01-Projects/Git/` — currently includes `help-self`, `svg-editor`, `tungsten-flow`, and others. User creates named Zellij sessions per project.

### Design philosophy

SLC — Simple, Loveable, Complete. Small feature set but crafted interaction. The sidebar should feel native to Zellij, not bolted on.

## Constraints

- **Language**: Rust — Zellij plugins compile to wasm32-wasi, no other language option
- **Rendering**: Character-cell based — Zellij plugin API provides cell-by-cell rendering, no rich UI framework
- **Plugin API surface**: Limited to what Zellij exposes — session management, keyboard events, pane rendering. Cannot control other plugins or modify layouts programmatically.
- **No runtime dependencies**: WASM plugin must be self-contained, no external process calls at runtime
- **Config format**: KDL — Zellij uses KDL for all configuration, plugin config is passed via KDL block attributes

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Docked panel, not floating | Floating popups interrupt flow — sidebar is ambient awareness | — Pending |
| Pinned folders, not auto-scan | User knows their projects — explicit list is faster than scanning + filtering noise | — Pending |
| Keybind toggle (Cmd+P) | Some layouts are space-constrained — toggle lets user choose when sidebar is visible | — Pending |
| Keyboard-first, no mouse v1 | Matches Zellij's keyboard-centric philosophy, simpler to implement | — Pending |
| Catppuccin Frappe hardcoded | User's theme — ship fast, make themeable in v2 | — Pending |
| Info verbosity config | Different workflows need different density — minimal for focused work, full for overview | — Pending |

---
*Last updated: 2026-03-11 after initialization*
