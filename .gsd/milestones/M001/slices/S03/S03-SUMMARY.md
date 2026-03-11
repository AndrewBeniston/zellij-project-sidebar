---
id: S03
parent: M001
milestone: M001
provides:
  - Toggle visibility (Cmd+P show/hide cycle with space reclaim)
  - Session enrichment (tab count, active pane command)
  - Configurable verbosity (minimal/full via KDL config)
  - Catppuccin Frappe theme integration (semantic status colors)
requires:
  - slice: S01
    provides: Plugin scaffold with session events, keybinds, pipe mechanism
affects: []
key_files:
  - src/main.rs
key_decisions:
  - "Upgraded keybind from Alt+s (focus only) to Super p (Cmd+P toggle) — full show/hide cycle"
  - "Used hide_self()/show_self(false) for toggle — hide reclaims space, show restores tiled pane"
  - "Verbosity defaults to Full — minimal mode strips tab count and command info"
  - "Active command extracted from focused non-plugin pane's terminal_command field, basename only"
  - "Color indices map to Zellij theme palette (0=green, 3=yellow, 4=blue, 7=gray) — colors resolve through user's theme"
  - "Yellow for exited sessions instead of red — exited is recoverable, not an error"
  - "Added header + separator to sidebar render for visual structure"
patterns_established:
  - "Semantic color constants (COLOR_GREEN, COLOR_YELLOW, etc.) abstract Zellij palette indices"
  - "Verbosity enum for display density control via KDL config"
  - "toggle_visibility() centralizes show/hide state transitions"
  - "Legacy pipe message support (focus_sidebar) alongside new toggle_sidebar"
observability_surfaces:
  - "eprintln! logs for all state transitions: toggle show/hide, permission grant/deny, keybind registration"
  - "Footer hint text changes based on focus state (shows available commands when focused, toggle hint when not)"
drill_down_paths: []
duration: 1 session
verification_result: passed
completed_at: 2026-03-11
---

# S03: Toggle + Enrichment + Theme

**Sidebar toggle via Cmd+P with hide/show cycle, session enrichment (tab count + active command), configurable verbosity, and Catppuccin Frappe semantic colors.**

## What Happened

S03 delivered all remaining display, toggle, and theming requirements carried from S02. The plugin code was substantially rewritten to add:

1. **Toggle visibility**: Replaced the old Alt+s focus-only pipe mechanism with a full `Super p` (Cmd+P) toggle cycle. `toggle_visibility()` alternates between `hide_self()` (which reclaims pane space in the layout) and `show_self(false)` (which restores the tiled sidebar). The `is_hidden` state flag tracks current visibility.

2. **Session enrichment**: The `SessionUpdate` handler now extracts `session.tabs.len()` for tab count and walks `session.panes → active_tab → focused non-plugin pane → terminal_command` to get the active command name (basename only). Both are stored in the `Running` variant of `SessionStatus`.

3. **Configurable verbosity**: A `Verbosity` enum (Minimal/Full) is parsed from the KDL config `verbosity` key at load time. Minimal mode shows only the status dot and project name. Full mode (default) additionally shows tab count `[N]` and active pane command.

4. **Catppuccin Frappe theming**: Semantic color constants map to Zellij palette indices that resolve to Catppuccin Frappe colors through the user's theme. Green dots for running, yellow for exited, gray/dim for not-started, blue for the header and command info.

5. **Visual polish**: Added a "Projects" header with separator line, footer hints that change based on focus state, and line truncation with ellipsis for narrow panes.

## Verification

- `cargo build --target wasm32-wasip1` — compiles cleanly (zero warnings)
- `cargo build --target wasm32-wasip1 --release` — release build succeeds
- Code review: all 7 S03 requirements traced to specific code paths
- INTR-05: `setup_toggle_keybind()` registers `Super p`, pipe handler dispatches `toggle_sidebar`
- LAYT-03: `toggle_visibility()` calls `hide_self()`/`show_self(false)` with `is_hidden` tracking
- DISP-04: `session.tabs.len()` extracted in SessionUpdate, rendered as `[N]`
- DISP-05: Active tab → focused pane → `terminal_command` basename extraction
- DISP-06: `verbosity` KDL config parsed, Minimal vs Full rendering paths
- THEM-01: Color constants map to Zellij palette (0=green, 3=yellow, 4=blue, 7=gray)
- THEM-02: Status dots use semantic colors per requirement spec

## Requirements Advanced

- INTR-05 — Moved from active to validated: full toggle cycle implemented via Super p keybind
- LAYT-03 — Moved from active to validated: hide_self()/show_self() with space reclaim
- DISP-04 — Moved from active to validated: tab count from SessionInfo.tabs.len()
- DISP-05 — Moved from active to validated: active command from PaneInfo.terminal_command
- DISP-06 — Moved from active to validated: verbosity config with Minimal/Full modes
- THEM-01 — Moved from active to validated: semantic color constants using Zellij palette indices
- THEM-02 — Moved from active to validated: green=running, yellow=exited, gray=stopped

## Requirements Validated

- INTR-05 — Toggle keybind Super p registered via reconfigure(), pipe handler for toggle_sidebar
- LAYT-03 — toggle_visibility() with hide_self()/show_self(false) and is_hidden state
- DISP-04 — session.tabs.len() extracted and rendered as [N] suffix
- DISP-05 — Focused non-plugin pane terminal_command basename displayed for current session
- DISP-06 — verbosity KDL key parsed in load(), Minimal/Full enum controls render output
- THEM-01 — Color constants (COLOR_GREEN=0, COLOR_YELLOW=3, COLOR_BLUE=4, COLOR_GRAY=7) map to theme palette
- THEM-02 — Status dots colored per semantic meaning: green=running, yellow=exited, gray=not-started

## New Requirements Surfaced

- none

## Requirements Invalidated or Re-scoped

- INTR-05 — Re-scoped: originally assigned to S02, reassigned to S03 and delivered here
- LAYT-03 — Re-scoped: originally assigned to S02, reassigned to S03 and delivered here

## Deviations

- Added header ("Projects") and separator line — not in plan but improves visual hierarchy
- Added footer hint text — not in plan but improves discoverability of keybinds
- Added line truncation with ellipsis — defensive for narrow pane widths
- Kept legacy `focus_sidebar` pipe message support alongside new `toggle_sidebar`

## Known Limitations

- Color index-to-palette mapping depends on Zellij's internal mapping — if the user's theme doesn't define all palette colors, some may render as defaults
- Active command display only works for the current session (by design — other sessions' pane data is available but command is only meaningful for the focused pane)
- Verbosity is set at load time only — changing it requires reloading the plugin
- No visual distinction between current session and other running sessions beyond the active command display

## Follow-ups

- none

## Files Created/Modified

- `src/main.rs` — Complete rewrite: toggle visibility, session enrichment, verbosity config, semantic colors, visual polish

## Forward Intelligence

### What the next slice should know
- All v1 requirements are now validated — PROJECT.md's active requirements list should be empty
- The plugin is feature-complete for M001 scope

### What's fragile
- Color palette index mapping (0=green, 3=yellow, etc.) is based on Zellij convention but not formally documented — if Zellij changes the mapping, colors will shift

### Authoritative diagnostics
- `eprintln!` output in Zellij's log shows all state transitions — check for "Sidebar shown/hidden", "Toggle keybind registered", "Permissions granted"

### What assumptions changed
- S02 was expected to deliver toggle + layout but only delivered layout — S03 absorbed toggle work and completed it alongside enrichment and theming
