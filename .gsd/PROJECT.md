# Zellij Project Sidebar

## What This Is

A Zellij plugin that provides a docked sidebar panel for managing project sessions. Unlike existing floating popup plugins (sessionizer, choose-tree), this is a persistent side panel — always accessible via keybind — that shows your pinned project folders with live session status and lets you switch, create, or kill sessions with a single keypress. Think VS Code's sidebar, but for terminal sessions.

## Core Value

One-keypress project switching with always-visible session awareness — you should never have to think about session management, just glance and jump.

## Current State

All M001 (v1.0) slices complete. The plugin is feature-complete:

- **Scaffold**: Rust WASM plugin compiles to wasm32-wasip1, loads in Zellij 0.43.1, requests permissions, subscribes to SessionUpdate events
- **Display**: Pinned project list from KDL config with live session status (running/exited/not-started), tab count, active pane command
- **Interaction**: j/k navigation, Enter to switch/create sessions, x to kill, Esc to deactivate
- **Toggle**: Cmd+P (Super p) toggles sidebar visibility with full hide/show cycle and space reclaim
- **Theme**: Catppuccin Frappe colors via Zellij palette indices, semantic status dots (green=running, yellow=exited, gray=stopped)
- **Config**: Verbosity configurable via KDL (minimal/full)

## Requirements

### Validated

- [x] Plugin compiles to wasm32-wasip1 and loads in Zellij 0.43.1
- [x] Plugin requests and handles permissions correctly
- [x] Plugin subscribes to SessionUpdate events for live data
- [x] Pinned project list from configuration
- [x] Live session status per project (running / exited / not started)
- [x] Switch to existing session on selection (Enter)
- [x] Create new session from folder if none exists
- [x] Kill session from sidebar (x key)
- [x] Docked side panel (not floating popup) — left side, fixed width
- [x] Toggle visibility with keybind (Cmd+P via pipe mechanism)
- [x] Tab count display per session (e.g. `help-self [3]`)
- [x] Active pane command display (what's running in focused pane)
- [x] Configurable info verbosity — minimal (name + dot) through full (tabs + active command)
- [x] Keyboard navigation (j/k, Enter to switch, x to kill)
- [x] Visual indicator for current/active session
- [x] Catppuccin Frappe theme integration (semantic colors via Zellij palette)
- [x] Sidebar unselectable by default — becomes selectable only during interaction
- [x] Pipe-based toggle from any context (unfocused)

### Out of Scope

- Fuzzy search / directory scanning — sessionizer already does this
- Rename session — low value, can do via CLI
- Apply custom layout per project — future feature
- Tab/pane management within sessions — choose-tree handles this
- Mouse interaction — keyboard-first for v1
- Multi-theme support — hardcode Catppuccin Frappe for now
- Session resurrection / state persistence — Zellij handles this natively

## Context

### Ecosystem

- **Zellij plugin API**: Rust → WASM (wasm32-wasi). Plugin API provides `switch_session()`, session listing, keyboard/mouse events, and character-level rendering.
- **User's setup**: Ghostty + Zellij with Catppuccin Frappe theme. CMD keys unbound in Ghostty and passed as Super to Zellij.
- **Keybind**: Cmd+P (Super p) for toggle.

### User's project structure

Projects live under `~/Documents/01-Projects/Git/` — configured as `project_N` entries in the plugin KDL block.

## Constraints

- **Language**: Rust — Zellij plugins compile to wasm32-wasi
- **Rendering**: Character-cell based — Zellij plugin API provides cell-by-cell rendering
- **Plugin API surface**: Limited to what Zellij exposes
- **Config format**: KDL — Zellij uses KDL for all configuration

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| Docked panel, not floating | Floating popups interrupt flow — sidebar is ambient awareness |
| Pinned folders, not auto-scan | User knows their projects — explicit list is faster |
| Keybind toggle (Cmd+P) | Space-constrained layouts need toggle |
| Yellow for exited (not red) | Exited is recoverable, not an error |
| Verbosity defaults to Full | Minimal is opt-in for focused work |
| Color via palette indices | Adapts to user's theme automatically |
| Basename-only for commands | Full path would overflow narrow sidebar |

---
*Last updated: 2026-03-11 after S03 completion*
