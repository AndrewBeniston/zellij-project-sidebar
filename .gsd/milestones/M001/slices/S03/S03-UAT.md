# S03: Toggle + Enrichment + Theme — UAT

**Milestone:** M001
**Written:** 2026-03-11

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: Plugin is a WASM binary that requires a running Zellij instance with specific session state. All code paths were verified by tracing requirements to specific functions and confirming compilation. Live runtime testing requires the user's Zellij environment with multiple named sessions.

## Preconditions

- Zellij 0.43.1 installed
- Plugin compiled: `cargo build --target wasm32-wasip1`
- Dev layout loaded: `zellij -l zellij.kdl`
- Permissions granted on first launch
- At least 2 of the configured project sessions running (e.g. `help-self`, `tungsten-flow`)
- Ghostty configured to pass Cmd keys as Super to Zellij

## Smoke Test

Press Cmd+P from any pane. The sidebar should appear as a tiled left pane showing project names with colored status dots. Press Cmd+P again — sidebar should disappear and space should be reclaimed by other panes.

## Test Cases

### 1. Toggle visibility with Cmd+P

1. Focus any non-sidebar pane
2. Press Cmd+P (Super p)
3. **Expected:** Sidebar appears as tiled left pane, gains focus
4. Press Cmd+P again
5. **Expected:** Sidebar disappears, remaining panes expand to fill the space

### 2. Tab count display

1. With sidebar visible, observe a running session entry
2. **Expected:** Running sessions show tab count in brackets, e.g. `● help-self [3]`
3. Open a new tab in one of the configured sessions
4. **Expected:** Tab count updates on next SessionUpdate event

### 3. Active command display

1. In the current session, run a command (e.g. `vim`, `claude`)
2. Toggle sidebar visible with Cmd+P
3. **Expected:** Current session's entry shows the command name after the tab count, e.g. `● help-self [3] vim`
4. Switch to a different pane running a different command
5. **Expected:** Command name updates to reflect the newly focused pane

### 4. Semantic status colors

1. Observe sidebar with mixed session states
2. **Expected:** Running sessions have green dots (●), exited sessions have yellow dots (●), not-started projects have gray/dim hollow dots (○)

### 5. Minimal verbosity mode

1. Add `verbosity "minimal"` to the plugin config block in zellij.kdl
2. Reload the plugin
3. **Expected:** Each project shows only status dot and name — no tab count, no command

### 6. Header and footer

1. Observe sidebar layout
2. **Expected:** "Projects" header at top in blue, separator line below, project list, then footer hint text in gray
3. When sidebar is focused, footer shows `j/k:nav ↵:switch x:kill`
4. When sidebar is not focused (after Esc), footer shows `⌘P to toggle`

## Edge Cases

### No running sessions

1. Start fresh with no project sessions running
2. **Expected:** All projects show gray hollow dots (○), no tab counts or commands displayed

### Narrow pane width

1. Resize sidebar pane to very narrow (< 15 chars)
2. **Expected:** Lines truncate with ellipsis (…) instead of wrapping or overflowing

### Toggle from hidden state

1. Hide sidebar with Cmd+P
2. Navigate to a different tab
3. Press Cmd+P
4. **Expected:** Sidebar reappears in its tiled position with current session data

## Failure Signals

- Cmd+P does nothing — keybind not registered (check Zellij log for "Toggle keybind Super+p registered")
- Sidebar shows but Cmd+P doesn't hide — toggle_visibility() not dispatching hide_self()
- No tab count shown — SessionInfo.tabs may be empty (check session has tabs)
- No command shown — focused pane may be a plugin pane or terminal_command is None for shell panes
- Wrong colors — theme palette may not be configured or Zellij palette index mapping differs

## Requirements Proved By This UAT

- INTR-05 — Test 1 proves toggle keybind works from any context
- LAYT-03 — Test 1 proves hide/show cycle with space reclaim
- DISP-04 — Test 2 proves tab count display
- DISP-05 — Test 3 proves active command display
- DISP-06 — Test 5 proves verbosity configuration
- THEM-01 — Test 4+6 proves Catppuccin Frappe color integration
- THEM-02 — Test 4 proves semantic status colors

## Not Proven By This UAT

- Actual Catppuccin Frappe color rendering — requires visual inspection in user's themed terminal
- Active command updates in real-time — depends on SessionUpdate event frequency
- Behavior with > 10 projects — vertical overflow not handled (no scrolling)
- Behavior when Ghostty doesn't pass Super key — keybind won't work without passthrough config

## Notes for Tester

- The `terminal_command` field may be `None` for bare shell panes (no explicit command) — this is expected, the command field will simply be absent
- Color appearance depends entirely on the user's Zellij theme — if not using Catppuccin Frappe, colors will differ but semantic meaning (green=running, etc.) is preserved
- The plugin logs to stderr which Zellij captures — check Zellij's log output for diagnostic messages
