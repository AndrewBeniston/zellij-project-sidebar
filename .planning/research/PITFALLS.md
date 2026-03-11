# Domain Pitfalls

**Domain:** Zellij WASM plugin (docked sidebar for session management)
**Researched:** 2026-03-11

## Critical Pitfalls

Mistakes that cause rewrites or major issues.

### Pitfall 1: Selectable Sidebar Steals Focus From Terminal Panes

**What goes wrong:** A sidebar plugin that remains selectable (the default) will steal keyboard focus when users navigate with directional keys (Alt+h/j/k/l). The user presses Alt+Left to move between terminal panes and accidentally lands inside the sidebar plugin, losing their terminal context. Every subsequent keypress goes to the plugin instead of their shell.

**Why it happens:** By default, Zellij treats plugin panes as focusable like any other pane. Sidebar plugins that accept keyboard input (j/k navigation) MUST be selectable when they want input, but this creates a focus trap in normal workflow.

**Consequences:** Users constantly fight the sidebar for focus. The sidebar feels "bolted on" instead of ambient. Worst case: users disable the sidebar entirely because it disrupts their flow.

**Prevention:** Use `set_selectable(false)` by default so the sidebar is invisible to Zellij's pane navigation. When the user triggers the sidebar keybind (Cmd+P), use `set_selectable(true)` temporarily, handle the input, then `set_selectable(false)` when dismissed. This is the pattern used by built-in plugins like tab-bar and compact-bar. The vertical-tabs plugin (cfal/zellij-vertical-tabs) demonstrates this approach for sidebar-style plugins.

**Detection:** If directional key navigation feels broken or "sticky" around the sidebar, the selectable state is wrong.

**Phase relevance:** Phase 1 (MVP). Get this right from the first render or the plugin is unusable.

---

### Pitfall 2: Toggle Visibility Has No Built-In Action

**What goes wrong:** There is no native `TogglePlugin` action in Zellij. Developers assume they can bind a key to show/hide their plugin pane like a drawer, but discover there is no single action for this. GitHub issue #3243 confirms this is acknowledged but unimplemented due to complexity.

**Why it happens:** Zellij's keybinding actions include `LaunchOrFocusPlugin` (launch if not running, focus if running) and `MessagePlugin` (send a pipe message to a plugin), but neither implements toggle semantics. `hide_self()`/`show_self()` exist as plugin API commands, but they must be orchestrated by the plugin itself.

**Consequences:** Without a plan, the "toggle with Cmd+P" requirement becomes a multi-week detour into Zellij internals instead of a simple config line.

**Prevention:** Implement toggle via the `pipe` mechanism:
1. In `load()`, use `reconfigure()` to bind Cmd+P (Super p) to `MessagePluginId` with a `"toggle"` message name
2. In the plugin's `pipe()` method, check for the `"toggle"` message and call `hide_self()` or `show_self()` based on internal visibility state
3. Track visibility state internally (the `Visible(bool)` event fires when the plugin becomes visible/invisible)

This is the pattern used by the Zellij tutorial plugin and recommended in official docs. It works but requires the `Reconfigure` and `ChangeApplicationState` permissions.

**Detection:** If the keybind opens new plugin instances or does nothing, the pipe/message routing is misconfigured.

**Phase relevance:** Phase 1 (MVP). The toggle keybind is a core requirement.

---

### Pitfall 3: Event Ordering Is Not Guaranteed -- Permission Race Condition

**What goes wrong:** The plugin subscribes to events and calls `request_permission()` in `load()`. It then receives `ModeUpdate` or `SessionUpdate` events BEFORE receiving the `PermissionRequestResult` event. The plugin tries to call `switch_session()` or `reconfigure()` without permission and silently fails.

**Why it happens:** Zellij's event system is asynchronous. The official docs explicitly warn: "a plugin could receive certain events (like ModeUpdate) before the PermissionRequestResult event is received." There is no guaranteed ordering between event types.

**Consequences:** Plugin appears to load but does nothing. Session switching silently fails. The reconfigure keybind never gets set up. Extremely hard to debug because there are no error messages -- just silent no-ops.

**Prevention:** Implement a permission state machine in the plugin struct:
```rust
struct State {
    permissions_granted: bool,
    pending_actions: Vec<PendingAction>,
}
```
Queue any actions that require permissions. When `PermissionRequestResult::Granted` arrives, flush the queue. Never call `switch_session()`, `reconfigure()`, `kill_sessions()`, or `hide_self()`/`show_self()` before permissions are confirmed.

**Detection:** Plugin loads but keybind doesn't work, or session switching silently fails. Check `eprintln!` logs via `zellij setup --check` to find the log path.

**Phase relevance:** Phase 1 (MVP). Must be correct from first implementation or nothing works.

---

### Pitfall 4: Fixed-Size Panes Are Unstable For Selectable Plugins

**What goes wrong:** The sidebar uses a fixed character-width pane (e.g., `size=25` for 25 columns). Zellij's own docs warn: "specifying fixed values that are not unselectable plugins is currently unstable and might lead to unexpected behaviour when resizing or closing panes." When the terminal window is resized, the sidebar pane may collapse, overlap, or cause layout corruption.

**Why it happens:** Zellij's layout engine treats fixed-size panes differently based on selectability. Unselectable plugins (status bars) with fixed sizes are stable. Selectable plugins with fixed sizes trigger edge cases in the resize algorithm.

**Consequences:** Layout breaks on terminal resize. Sidebar disappears or takes over the screen. Users have to restart Zellij to recover.

**Prevention:** Two approaches:
1. **Preferred:** Use `set_selectable(false)` as default state (see Pitfall 1). This makes the fixed-size pane stable. Toggle selectable only during active interaction.
2. **Alternative:** Use percentage-based sizing (`size="20%"`) instead of fixed character counts. Less precise but more resilient to resize.

The vertical-tabs plugin uses approach 1 (fixed size + unselectable) successfully.

**Detection:** Resize the terminal window aggressively while the sidebar is visible. If the layout breaks, sizing strategy is wrong.

**Phase relevance:** Phase 1 (layout definition). The KDL layout must be correct from the start.

---

### Pitfall 5: wasm32-wasi Target Is Removed From Stable Rust

**What goes wrong:** Developer follows older Zellij docs or blog posts that reference `wasm32-wasi` as the build target. Starting Rust 1.84 (January 2025), `wasm32-wasi` was removed from stable Rust. The build fails with a cryptic target-not-found error.

**Why it happens:** Rust renamed `wasm32-wasi` to `wasm32-wasip1` to reserve the `wasm32-wasi` name for WASI 1.0. The Zellij official docs and the rust-plugin-example repo still reference the old target name in some places. Zellij itself now uses `wasm32-wasip1` internally.

**Consequences:** Build fails immediately. Developer wastes time debugging toolchain issues instead of writing plugin code.

**Prevention:**
- Use `wasm32-wasip1` in `.cargo/config.toml`: `[build] target = "wasm32-wasip1"`
- Add the target: `rustup target add wasm32-wasip1`
- If using the official template, check `.cargo/config.toml` and update if it says `wasm32-wasi`
- Zellij 0.41+ expects `wasm32-wasip1` compiled plugins

**Detection:** Build fails with "target not found" or "wasm32-wasi is not a valid target."

**Phase relevance:** Phase 0 (project setup). Must be correct before any code is written.

## Moderate Pitfalls

### Pitfall 6: Returning true From update() On Every Event Causes Render Thrashing

**What goes wrong:** The `update()` method returns `true` (requesting a re-render) for every event, even when the UI hasn't changed. For a sidebar subscribing to `SessionUpdate`, `TabUpdate`, `PaneUpdate`, and `ModeUpdate`, this means continuous re-rendering on every keystroke in any pane.

**Why it happens:** The tutorial example returns `true` from update() as a simplification: "If we wanted, we could be even more exact about this - only setting should_render to true if our UI actually changed." Developers copy this pattern without understanding the performance cost.

**Prevention:** Compare incoming event data against cached state. Only return `true` when the UI actually needs to change:
```rust
fn update(&mut self, event: Event) -> bool {
    match event {
        Event::SessionUpdate(sessions, _) => {
            if sessions != self.cached_sessions {
                self.cached_sessions = sessions;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}
```

**Detection:** If the plugin pane flickers or the terminal feels sluggish with the sidebar open, check render frequency.

**Phase relevance:** Phase 2 (session status display). Performance matters once live data is flowing.

---

### Pitfall 7: render() Clears All State Every Call -- No Incremental Updates

**What goes wrong:** Developer tries to keep track of what changed and render only the diff. Zellij clears the plugin's terminal state before every `render()` call. Any attempt at incremental rendering produces visual artifacts or blank areas.

**Why it happens:** The official docs state: "Every time the render function is called, the previous state of the terminal is cleared." This is by design -- it eliminates the need to track screen state -- but developers coming from TUI frameworks (ratatui, cursive) expect retained-mode rendering.

**Prevention:** Accept the full-repaint model. Keep all renderable state in your plugin struct and repaint everything in `render()`. This is actually simpler than incremental updates -- embrace it. Pre-compute layout in `update()`, then `render()` just prints.

**Detection:** Blank areas or ghost text in the sidebar after updates.

**Phase relevance:** Phase 1 (first render implementation). Must understand the rendering model before writing any UI code.

---

### Pitfall 8: LaunchOrFocusPlugin Creates Duplicates With Plugin Aliases

**What goes wrong:** Using `LaunchOrFocusPlugin` with a plugin alias (e.g., a custom name) creates duplicate plugin instances instead of focusing the existing one. Each press of the keybind spawns a new sidebar pane.

**Why it happens:** GitHub issue #3409 documents this bug. When using alias-style URLs, Zellij fails to match the running instance. The bug was reported in v0.40.1 and may be fixed in v0.43.1+ (maintainer could not reproduce), but is not guaranteed to be resolved.

**Consequences:** Multiple sidebar panes appear, confusing the user and wasting resources.

**Prevention:**
1. Use `file:` schema URLs directly instead of aliases: `file:/path/to/plugin.wasm`
2. Better: Use the `MessagePlugin` / pipe approach for toggle (see Pitfall 2), which avoids `LaunchOrFocusPlugin` entirely
3. If you must use `LaunchOrFocusPlugin`, test with your exact Zellij version

**Detection:** Multiple sidebar panes appear after pressing the toggle key repeatedly.

**Phase relevance:** Phase 1 (keybind configuration). Affects the launch mechanism.

---

### Pitfall 9: Coordinate Calculations Panic On Small Terminal Windows

**What goes wrong:** The `render()` function receives `rows` and `cols` parameters. If the terminal window is very small (e.g., 3 rows, 10 cols) and the plugin does arithmetic like `cols - 5`, it panics with underflow because the result is negative for a `usize`.

**Why it happens:** Zellij can resize plugin panes to very small dimensions. The render function is called regardless of how small the pane is. Standard subtraction on unsigned integers (`usize`) wraps or panics.

**Consequences:** Plugin crashes. Zellij may recover but the plugin pane goes blank.

**Prevention:** Use `saturating_sub()` for ALL coordinate arithmetic: `cols.saturating_sub(5)` returns 0 instead of panicking. Add early returns in `render()` if dimensions are below a minimum threshold:
```rust
fn render(&mut self, rows: usize, cols: usize) {
    if cols < 10 || rows < 3 {
        return; // Too small to render anything useful
    }
    // ...
}
```

**Detection:** Resize the terminal very small while the sidebar is visible. If Zellij crashes or the plugin goes blank, coordinate math is unsafe.

**Phase relevance:** Phase 1 (rendering). Must be defensive from the first render implementation.

---

### Pitfall 10: PaneUpdate Race -- New Tabs Show Stale Titles

**What goes wrong:** When a new tab is created, Zellij sends the `PaneUpdate` event before the shell has set the terminal title. The sidebar displays a placeholder or empty string for the new pane's command/title, which only corrects on the next update.

**Why it happens:** The vertical-tabs plugin documents this: "When a new tab is created, zellij sends the PaneUpdate event before the shell has set the terminal title." This is a Zellij timing issue, not a plugin bug.

**Consequences:** Sidebar briefly shows incorrect information for new sessions/tabs. Users see a flash of wrong data.

**Prevention:** Implement a short debounce or accept the momentary stale data. For session names (the primary display), this is less of an issue since session names are set at creation time, not by the shell. For "active command" display, expect initial staleness and re-render when the next PaneUpdate arrives with correct data.

**Detection:** Create a new session or tab and watch the sidebar. If it shows blank/wrong info briefly, this is the race.

**Phase relevance:** Phase 2+ (active command display). Less critical for session-name-only display.

---

### Pitfall 11: Session Switching Closes the Current Session's Plugin Instance

**What goes wrong:** When calling `switch_session("other-project")`, the plugin may lose its state or need to reinitialize because the plugin instance is per-session. Each Zellij session loads its own plugin instances from its layout.

**Why it happens:** Zellij sessions are isolated. When you switch to another session, that session has its own layout and its own plugin instances. The sidebar plugin in the original session is not the same instance as the sidebar in the target session.

**Consequences:** If the sidebar relies on accumulated runtime state (cached session list, scroll position, selected index), that state exists only in the current session's plugin instance. Switching sessions and switching back resets the sidebar to its initial state.

**Prevention:**
1. Keep the sidebar stateless between sessions -- rebuild state from `SessionUpdate` events on every focus
2. The sidebar's `load()` should subscribe to `SessionUpdate` immediately, and the first event will populate the current session list
3. Do NOT try to share state between plugin instances across sessions -- there is no mechanism for this

**Detection:** Switch sessions and switch back. If the sidebar's selection/scroll position resets, state is not being rebuilt from events.

**Phase relevance:** Phase 2 (session switching). Must understand the session isolation model before implementing switch.

## Minor Pitfalls

### Pitfall 12: eprintln! Is the Only Debug Tool -- Logs Are Hidden

**What goes wrong:** Developer adds `println!` for debugging. Output goes to the plugin's rendered UI (STDOUT), corrupting the display. Or developer expects a debugger/REPL and finds none available.

**Prevention:** Use `eprintln!()` exclusively for debug output. STDERR is routed to Zellij's log file. Find the log path with `zellij setup --check`. Use `wrangler tail`-style monitoring: `tail -f /path/to/zellij-log` in another pane.

**Phase relevance:** All phases. Set up logging infrastructure in Phase 0.

---

### Pitfall 13: WASM Binary Size Bloat From Unnecessary Dependencies

**What goes wrong:** Adding Rust crates that pull in heavy dependencies (serde_json, tokio, etc.) bloats the WASM binary from ~100KB to multiple megabytes. Plugin load time becomes noticeable.

**Prevention:** Keep dependencies minimal. `zellij-tile` and `zellij-tile-utils` are sufficient for most plugins. Use `wasm-opt` (from binaryen) on release builds to strip and optimize. In `Cargo.toml`:
```toml
[profile.release]
opt-level = "s"    # Optimize for size
lto = true         # Link-time optimization
strip = true       # Strip debug symbols
```

**Phase relevance:** Phase 0 (project setup). Set up Cargo.toml profiles correctly from the start.

---

### Pitfall 14: Multibyte/Unicode Characters Break Width Calculations

**What goes wrong:** Rendering project names containing emoji, CJK characters, or other wide Unicode characters causes alignment issues. A character that displays as 2 columns wide is counted as 1 during width calculation, pushing subsequent text off-screen.

**Prevention:** Use Unicode width calculation (the `unicode-width` crate) instead of `str::len()` for display width. Truncate based on display width, not byte length. Alternatively, keep project display names ASCII-only in the config and avoid the problem entirely.

**Phase relevance:** Phase 2 (rendering polish). Not critical for MVP if project names are ASCII.

---

### Pitfall 15: Forgetting to Request All Needed Permissions Upfront

**What goes wrong:** Plugin requests `ReadApplicationState` in `load()` but forgets `ChangeApplicationState` (needed for `switch_session`, `kill_sessions`, `hide_self`, `show_self`) or `Reconfigure` (needed for dynamic keybind setup). The permission dialog only fires on first load. After that, permissions are cached by plugin URL. Adding a new permission later requires the user to clear their cache.

**Prevention:** Request ALL permissions you will ever need in `load()`:
- `ReadApplicationState` -- for SessionUpdate, TabUpdate, PaneUpdate, ModeUpdate events
- `ChangeApplicationState` -- for switch_session, kill_sessions, hide_self, show_self
- `Reconfigure` -- for dynamic keybind setup via reconfigure()

Over-requesting is better than under-requesting. Users see one permission dialog on first load.

**Detection:** A feature works in development (where you frequently reload) but fails for users who installed the plugin once.

**Phase relevance:** Phase 1 (load implementation). Get the permission list right from the start.

---

### Pitfall 16: KDL Config Parsing Has No Schema Validation

**What goes wrong:** Plugin configuration is passed as `BTreeMap<String, String>` -- flat key-value pairs, all strings. There is no type system, no schema validation, no error reporting for invalid config keys. A typo in a config key (e.g., `proejcts` instead of `projects`) silently produces an empty value.

**Prevention:** Validate all config keys in `load()`. Log warnings via `eprintln!` for unrecognized keys. Provide sensible defaults for all values. Document the exact config schema in the README with examples.

**Phase relevance:** Phase 1 (configuration parsing). Users will misconfigure the plugin -- fail gracefully.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Project setup | wasm32-wasi target removed (Pitfall 5) | Use wasm32-wasip1 from day one |
| Project setup | Binary size bloat (Pitfall 13) | Configure Cargo.toml release profile |
| Layout & KDL config | Fixed-size pane instability (Pitfall 4) | Use unselectable + fixed size pattern |
| Toggle keybind | No built-in toggle action (Pitfall 2) | Implement via MessagePluginId pipe pattern |
| Plugin load | Permission race condition (Pitfall 3) | State machine for permission tracking |
| Plugin load | Missing permissions (Pitfall 15) | Request all permissions upfront |
| Config parsing | Silent config errors (Pitfall 16) | Validate and log in load() |
| Rendering | Full repaint model (Pitfall 7) | Accept it, pre-compute in update() |
| Rendering | Coordinate underflow (Pitfall 9) | saturating_sub everywhere |
| Rendering | Unicode width (Pitfall 14) | Use unicode-width crate |
| Focus management | Selectable sidebar steals focus (Pitfall 1) | Toggle selectable state dynamically |
| Session display | Render thrashing (Pitfall 6) | Compare state before returning true |
| Session display | Stale pane titles (Pitfall 10) | Accept momentary staleness, debounce |
| Session switching | State reset across sessions (Pitfall 11) | Rebuild from events, stay stateless |
| Session switching | LaunchOrFocusPlugin duplicates (Pitfall 8) | Use pipe/message pattern instead |
| Debugging | Hidden logs (Pitfall 12) | eprintln! + tail log file |

## Sources

**HIGH confidence (official docs):**
- [Plugin Lifecycle](https://zellij.dev/documentation/plugin-lifecycle.html) -- event ordering warning, render model
- [Plugin API Commands](https://zellij.dev/documentation/plugin-api-commands.html) -- permission requirements, command reference
- [Plugin API Events](https://zellij.dev/documentation/plugin-api-events.html) -- event types and payloads
- [Plugin UI Rendering](https://zellij.dev/documentation/plugin-ui-rendering.html) -- full-repaint model, coordinate system
- [Developing a Rust Plugin Tutorial](https://zellij.dev/tutorials/developing-a-rust-plugin/) -- saturating_sub, render best practices
- [Plugin Upgrade Guide 0.38.0](https://zellij.dev/documentation/plugin-upgrade-0.38.0) -- breaking changes, permission system
- [Creating a Layout](https://zellij.dev/documentation/creating-a-layout) -- fixed size instability warning
- [Keybinding Possible Actions](https://zellij.dev/documentation/keybindings-possible-actions.html) -- LaunchOrFocusPlugin, MessagePlugin
- [Plugin Pipes](https://zellij.dev/documentation/plugin-pipes) -- pipe communication patterns
- [Rust Blog: WASI Target Changes](https://blog.rust-lang.org/2024/04/09/updates-to-rusts-wasi-targets/) -- wasm32-wasip1 migration

**MEDIUM confidence (verified community sources):**
- [GitHub Issue #3243](https://github.com/zellij-org/zellij/issues/3243) -- toggle visibility not implemented
- [GitHub Issue #3409](https://github.com/zellij-org/zellij/issues/3409) -- LaunchOrFocusPlugin duplicate instances
- [GitHub Issue #4656](https://github.com/zellij-org/zellij/issues/4656) -- focus/layout state bugs with stacked+floating
- [zellij-vertical-tabs](https://github.com/cfal/zellij-vertical-tabs) -- sidebar plugin patterns, set_selectable usage
- [yazelix](https://github.com/luccahuguet/yazelix) -- sidebar layout architecture, pane orchestration
- [DeepWiki: Built-in Plugins](https://deepwiki.com/zellij-org/zellij/4.3-built-in-plugins) -- session-manager patterns
- [zellij-tile docs.rs](https://docs.rs/zellij-tile/latest/zellij_tile/) -- API surface reference
