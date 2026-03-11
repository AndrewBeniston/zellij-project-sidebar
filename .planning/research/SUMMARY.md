# Research Summary: Zellij Project Sidebar

**Domain:** Terminal multiplexer plugin (WASM sidebar for session management)
**Researched:** 2026-03-11
**Overall confidence:** HIGH

## Executive Summary

The Zellij plugin ecosystem is mature and well-documented for WASM-based Rust plugins. The user's installed Zellij version (0.43.1) is the latest stable release, and the corresponding `zellij-tile` crate (0.43.1) provides a comprehensive API covering session management, event subscription, keyboard input, and character-cell UI rendering. The entire plugin surface needed for a session sidebar already exists in the official API -- no workarounds or undocumented features are required.

The stack is minimal and opinionated by necessity: Rust compiled to `wasm32-wasip1` is the only path. The only required dependency beyond `zellij-tile` is `serde` for config parsing (and it is already a transitive dependency). The development workflow uses a KDL layout file for hot-reload inside Zellij itself, with `cargo build` as the sole build step. There is no webpack, no bundler, no framework selection paralysis.

The plugin API provides everything needed for the core feature set. `SessionUpdate` events deliver complete session state including tab counts and pane information. `switch_session()` and `kill_sessions()` handle session management. `hide_self()`/`show_self()` handle sidebar toggle. The `pipe()` mechanism with `MessagePluginId` keybindings solves the "toggle from any context" requirement without needing plugin focus. The `Text` component with `color_range()` auto-maps to the user's Catppuccin Frappe theme.

One critical finding: the `wasm32-wasi` target name was removed from Rust stable in v1.84 (January 2025). The user must use `wasm32-wasip1` instead and needs to run `rustup target add wasm32-wasip1` before building. The official Zellij plugin example repo has been updated to reflect this, but older tutorials and blog posts still reference the deprecated name.

## Key Findings

**Stack:** Rust + `zellij-tile` 0.43.1 targeting `wasm32-wasip1`. Two dependencies total (zellij-tile + serde). No build tooling beyond cargo.

**Architecture:** Single-struct state machine with event-driven updates. `load()` for setup, `update()` for state mutation, `render()` for output, `pipe()` for keybind toggle. Transform `SessionInfo` into slim `ProjectEntry` structs in update, render from those.

**Critical pitfall:** `Event::Key` only fires when the plugin pane is focused. The sidebar toggle MUST use `pipe()` via `MessagePluginId` keybinding, not key event handling, or it will be unreachable when another pane has focus.

## Implications for Roadmap

Based on research, suggested phase structure:

1. **Scaffold + Lifecycle** - Get a compiling WASM plugin that loads in Zellij, requests permissions, subscribes to events, logs to stderr.
   - Addresses: Project setup, dev environment, wasm32-wasip1 target verification
   - Avoids: Build configuration pitfalls (wrong target, wrong zellij-tile version)

2. **Config + Session Data Model** - Parse pinned folders from KDL config, receive `SessionUpdate` events, build `ProjectEntry` list with status.
   - Addresses: Pinned project list, live session status matching
   - Avoids: Config parsing edge cases (empty strings, tilde expansion)

3. **Core Renderer + Navigation** - Display project list with status indicators, j/k selection, Enter to switch/create, x to kill.
   - Addresses: Session list display, active indicator, keyboard nav, switch, kill
   - Avoids: Rendering without data (needs Phase 2 complete)

4. **Toggle + Sidebar Layout** - Register keybind via `reconfigure()`, handle pipe messages for visibility toggle, produce final layout KDL.
   - Addresses: Docked sidebar, Cmd+P toggle, layout integration
   - Avoids: Toggle anti-pattern (Key events don't reach unfocused plugins)

5. **Polish + Tab Count + Verbosity** - Tab count display, active command (if feasible), info verbosity config, Catppuccin theme refinement.
   - Addresses: Tab count, verbosity modes, theme polish
   - Avoids: Premature polish before interaction model is solid

**Phase ordering rationale:**
- Scaffold must come first -- cannot iterate on anything without a compiling plugin
- Config before data model -- need folder list to know what to match against session list
- Data model before renderer -- need data to display
- Renderer before toggle -- need visible output to verify navigation works
- Toggle is feature-complete gate -- all core interaction working before polish
- Polish last -- aesthetic refinement after functional completeness

**Research flags for phases:**
- Phase 1: Standard patterns, unlikely to need research. Follow official example.
- Phase 2: `SessionUpdate` data matching needs verification (session name vs. cwd matching). Minor research on tilde expansion in WASM context.
- Phase 3: UI rendering well-documented. No research needed.
- Phase 4: `reconfigure()` for keybind registration is documented in tutorial but toggle pattern needs careful implementation. `hide_self()`/`show_self()` behaviour when sidebar is a tiled pane needs live testing.
- Phase 5: `PaneUpdate` data availability cross-session needs verification. Tab count is confirmed available. Active command display may only work for current session.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Only one option (Rust + zellij-tile). Version verified against installed Zellij. Target name change verified against Rust release notes. |
| Features | HIGH | API surface verified against official docs. SessionInfo, TabInfo, PaneInfo struct fields confirmed. Session management commands confirmed. |
| Architecture | HIGH | Follows official tutorial patterns. Event-driven model is mandatory. `pipe()` for keybinds confirmed in tutorial. Layout-based sidebar confirmed in docs. |
| Pitfalls | HIGH | wasm32-wasi deprecation verified. Key event vs pipe distinction verified. Theme colour indices documented officially. |

## Gaps to Address

- **Cross-session PaneInfo availability**: The `SessionUpdate` event provides `PaneManifest` per session, but whether `terminal_command` is populated for non-current sessions needs live testing. If not, "active pane command" display is limited to current session only.
- **`hide_self()` behaviour in tiled layout**: When a tiled plugin pane calls `hide_self()`, does Zellij reclaim the space or leave a gap? Needs live testing. The alternative is to use `close_self()` + re-open, but that is heavier.
- **Tilde expansion in WASM**: Plugin config may contain `~/path` strings. Whether `PathBuf` handles tilde in WASI context or if manual expansion is needed requires verification.
- **Session creation with layout**: `switch_session_with_cwd()` creates a session with default layout. If users want per-project layouts, `switch_session_with_layout()` exists but adds config complexity. Deferred to v2 but noted.
- **Permission prompt UX**: First-time permission grant requires user to focus the plugin pane and press `y`. This interrupts flow. Cannot be automated. Need clear instructions.
