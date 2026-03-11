# Phase 2: Display + Interaction - Research

**Researched:** 2026-03-11
**Domain:** Zellij plugin rendering, keyboard interaction, session management, focus control
**Confidence:** HIGH

## Summary

Phase 2 transforms the Phase 1 scaffold into a functional project sidebar. The plugin must: (1) parse pinned project folders from KDL configuration, (2) match them against live session data from SessionUpdate events, (3) render a navigable list with status indicators, (4) handle keyboard input for navigation and session management, and (5) manage its own selectability to avoid stealing focus.

The Zellij plugin API provides all needed primitives. Configuration is passed as `BTreeMap<String, String>` key-value pairs from KDL plugin blocks. Session switching uses `switch_session_with_cwd()` for creating sessions with a working directory, and `kill_sessions()` for termination. Keyboard input arrives via `Event::Key(KeyWithModifier)` events with `BareKey::Char('j')` style matching. Rendering uses `print_text_with_coordinates()` with `Text` objects supporting `.selected()` and `.color_range()` for styling. Focus control uses `set_selectable(false)` to prevent normal navigation to the plugin, with `reconfigure()` + `MessagePluginId` to set up a keybind that pipes a focus message to the plugin.

The main architectural challenge is session-to-project matching. `SessionInfo` has no cwd field, so matching must be by session name convention. The recommended approach is: the user configures projects with a display name and a folder path, the session name is derived from the folder's basename (e.g., `/Users/me/Git/help-self` -> session name `help-self`), and matching is done by comparing session names against these derived names.

**Primary recommendation:** Parse projects from KDL config as semicolon-delimited paths or numbered key-value pairs. Match sessions by folder basename. Use `print_text_with_coordinates` with `Text` for rendering. Set up `MessagePluginId` keybind via `reconfigure()` for focus activation. Use `set_selectable(false)` by default and toggle on pipe message receipt.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DISP-01 | Plugin renders a list of pinned project folders from KDL config | KDL plugin config as `BTreeMap<String, String>` in `load()`. Parse numbered entries like `project_0`, `project_1` or semicolon-delimited `projects` value. |
| DISP-02 | Each project shows live session status (running / exited / not started) | `SessionUpdate(Vec<SessionInfo>, Vec<(String, Duration)>)` provides active sessions (running) and resurrectable sessions (exited). Projects not in either list are "not started". |
| DISP-03 | Current active session is visually highlighted | `SessionInfo.is_current_session` flag identifies the active session. Use `Text::new().selected()` or distinct `color_range` for highlighting. |
| INTR-01 | User can navigate project list with j/k keys | Subscribe to `EventType::Key`. Match `Event::Key(key)` where `key.bare_key == BareKey::Char('j')` / `BareKey::Char('k')` with `key.has_no_modifiers()`. |
| INTR-02 | User can switch to a running session by pressing Enter | Match `BareKey::Enter`. Call `switch_session(Some(&session_name))` then `hide_self()` or `set_selectable(false)`. |
| INTR-03 | If no session exists, Enter creates one with cwd set to that folder | Call `switch_session_with_cwd(Some(&session_name), Some(PathBuf::from(&folder_path)))`. Session name derived from folder basename. |
| INTR-04 | User can kill a session by pressing x on a running project | Match `BareKey::Char('x')`. Call `kill_sessions(&[session_name.clone()])`. SessionUpdate event will fire with updated state. |
| INFR-04 | Sidebar is unselectable by default -- becomes selectable only during active interaction | `set_selectable(false)` after permissions granted. `reconfigure()` with `MessagePluginId` keybind to pipe focus message. On pipe receipt: `set_selectable(true)` + `show_self(false)`. On Esc or session switch: `set_selectable(false)`. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| zellij-tile | 0.43.1 | Plugin API | Already in Cargo.toml from Phase 1. Provides all rendering, event, and session management APIs. |
| serde | 1.0 | Serialization | Already in Cargo.toml from Phase 1. May be needed for state serialization in future. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::path::PathBuf | (stdlib) | Path handling | Creating cwd paths for `switch_session_with_cwd()`. |
| std::collections::BTreeMap | (stdlib) | Config storage | Already used for `load()` configuration parameter. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `print_text_with_coordinates` | `println!` with ANSI codes | ANSI works (session-manager uses it) but `print_text_with_coordinates` is the official API, integrates with Zellij's theme system, and is cleaner. |
| `Text` + `color_range` | `NestedListItem` + `print_nested_list_with_coordinates` | NestedListItem is designed for hierarchical lists like session-manager. Our flat project list is simpler -- `Text` per line is sufficient. |
| Numbered config keys (`project_0`, `project_1`) | Semicolon-delimited single key (`projects`) | Numbered keys are more KDL-idiomatic and allow per-project metadata (name, path as separate keys). Semicolon-delimited is simpler but harder to extend. |

**Installation:**
```bash
# No new dependencies needed -- Phase 1 Cargo.toml is sufficient
```

## Architecture Patterns

### Recommended Project Structure
```
src/
+-- main.rs              # ZellijPlugin implementation (State, load, update, render, pipe)
```

Phase 2 keeps everything in `main.rs`. The file will grow from ~67 lines to ~250-350 lines. Module extraction (separating rendering, config, session logic) is Phase 4 scope if needed. For now, keeping it in one file avoids premature abstraction.

### Pattern 1: Configuration Parsing from KDL
**What:** Plugin configuration is passed as `BTreeMap<String, String>` to `load()`. KDL plugin blocks define key-value pairs that become entries in this map.
**When to use:** Always -- this is how Zellij plugins receive configuration.
**KDL config example:**
```kdl
plugin location="file:path/to/plugin.wasm" {
    project_0 "/Users/me/Documents/01-Projects/Git/help-self"
    project_1 "/Users/me/Documents/01-Projects/Git/tungsten-flow"
    project_2 "/Users/me/Documents/01-Projects/Git/svg-editor"
}
```
**Rust parsing example:**
```rust
// Source: Verified pattern from zjstatus, zellij-sessionizer, official tutorial
fn load(&mut self, configuration: BTreeMap<String, String>) {
    // Parse numbered project entries
    let mut projects = Vec::new();
    let mut i = 0;
    while let Some(path) = configuration.get(&format!("project_{}", i)) {
        let path = PathBuf::from(path);
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        projects.push(Project { name, path: path.to_string_lossy().to_string() });
        i += 1;
    }
    self.projects = projects;
    // ... permissions, subscriptions
}
```

### Pattern 2: Session Status Matching
**What:** Match configured project names against SessionUpdate data to determine status (running / exited / not started).
**When to use:** Every time a `SessionUpdate` event arrives.
**Example:**
```rust
// Source: Derived from SessionInfo docs + sessionizer pattern
#[derive(Clone, PartialEq)]
enum SessionStatus {
    Running { is_current: bool },
    Exited,
    NotStarted,
}

struct Project {
    name: String,          // Derived from folder basename
    path: String,          // Full absolute path
    status: SessionStatus, // Updated on each SessionUpdate
}

// In update() handler:
Event::SessionUpdate(sessions, resurrectable) => {
    for project in &mut self.projects {
        if let Some(session) = sessions.iter().find(|s| s.name == project.name) {
            project.status = SessionStatus::Running {
                is_current: session.is_current_session,
            };
        } else if resurrectable.iter().any(|(name, _)| name == &project.name) {
            project.status = SessionStatus::Exited;
        } else {
            project.status = SessionStatus::NotStarted;
        }
    }
    true // trigger re-render
}
```

### Pattern 3: Keyboard Event Handling
**What:** Subscribe to `EventType::Key`, match `BareKey` variants with modifier checks.
**When to use:** All keyboard interaction.
**Example:**
```rust
// Source: Verified from zellij session-manager main.rs pattern
Event::Key(key) => {
    match key.bare_key {
        BareKey::Char('j') if key.has_no_modifiers() => {
            self.selected_index = (self.selected_index + 1).min(self.projects.len().saturating_sub(1));
            true
        }
        BareKey::Char('k') if key.has_no_modifiers() => {
            self.selected_index = self.selected_index.saturating_sub(1);
            true
        }
        BareKey::Enter if key.has_no_modifiers() => {
            self.activate_selected_project();
            true
        }
        BareKey::Char('x') if key.has_no_modifiers() => {
            self.kill_selected_session();
            true
        }
        BareKey::Esc if key.has_no_modifiers() => {
            // Unfocus: make unselectable again
            set_selectable(false);
            true
        }
        _ => false,
    }
}
```

### Pattern 4: Rendering with Text + print_text_with_coordinates
**What:** Render each project as a styled `Text` element at specific coordinates. Use `.selected()` for the currently highlighted item and `.color_range()` for status indicators.
**When to use:** In `render(rows, cols)`.
**Example:**
```rust
// Source: Verified from official plugin-ui-rendering docs + session-manager
fn render(&mut self, rows: usize, cols: usize) {
    if !self.permissions_granted {
        println!("Waiting for permissions...");
        return;
    }

    for (i, project) in self.projects.iter().enumerate() {
        let status_char = match &project.status {
            SessionStatus::Running { is_current: true } => ">",
            SessionStatus::Running { is_current: false } => "*",
            SessionStatus::Exited => "x",
            SessionStatus::NotStarted => " ",
        };
        let line = format!(" {} {}", status_char, project.name);
        let mut text = Text::new(&line);

        if i == self.selected_index {
            text = text.selected();
        }

        // Color the status character (index 1) based on status
        match &project.status {
            SessionStatus::Running { .. } => {
                text = text.color_range(0, 1..=1); // color index 0 for running
            }
            SessionStatus::Exited => {
                text = text.color_range(2, 1..=1); // color index 2 for exited
            }
            _ => {}
        }

        print_text_with_coordinates(text, 0, i, Some(cols), None);
    }
}
```

### Pattern 5: Focus Management with set_selectable + reconfigure
**What:** Plugin is unselectable by default. A keybind (set up via `reconfigure()`) pipes a message to the plugin to activate it. On activation, `set_selectable(true)` + `show_self(false)`. On deactivation (Esc, session switch), `set_selectable(false)`.
**When to use:** INFR-04 implementation.
**Example:**
```rust
// Source: Verified from official Zellij tutorial reconfigure() example
fn setup_focus_keybind(&self) {
    let plugin_id = get_plugin_ids().plugin_id;
    let config = format!(
        r#"
        keybinds {{
            shared {{
                bind "Alt s" {{
                    MessagePluginId {plugin_id} {{
                        name "focus_sidebar"
                    }}
                }}
            }}
        }}
        "#,
    );
    reconfigure(config, false); // false = don't write to disk
}

// In pipe() handler:
fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
    if pipe_message.name == "focus_sidebar" {
        set_selectable(true);
        show_self(false); // false = don't float
        self.is_focused = true;
        return true;
    }
    false
}

// In update() Key handler, on Esc:
BareKey::Esc if key.has_no_modifiers() => {
    set_selectable(false);
    self.is_focused = false;
    true
}
```

### Pattern 6: Session Actions
**What:** Switch to existing session, create new session with cwd, or kill a session.
**When to use:** When user presses Enter or x on a project.
**Example:**
```rust
// Source: Verified from zellij-tile docs + sessionizer pattern
fn activate_selected_project(&mut self) {
    if let Some(project) = self.projects.get(self.selected_index) {
        match &project.status {
            SessionStatus::Running { .. } | SessionStatus::Exited => {
                // Switch to existing or resurrect exited session
                switch_session(Some(&project.name));
            }
            SessionStatus::NotStarted => {
                // Create new session with cwd
                switch_session_with_cwd(
                    Some(&project.name),
                    Some(PathBuf::from(&project.path)),
                );
            }
        }
        // Unfocus sidebar after switching
        set_selectable(false);
        self.is_focused = false;
    }
}

fn kill_selected_session(&mut self) {
    if let Some(project) = self.projects.get(self.selected_index) {
        if matches!(&project.status, SessionStatus::Running { .. }) {
            kill_sessions(&[project.name.clone()]);
            // Status will update via next SessionUpdate event
        }
    }
}
```

### Anti-Patterns to Avoid
- **Matching sessions by folder path:** `SessionInfo` has no cwd field. Match by session name (derived from folder basename).
- **Polling for session data:** Never use `set_timeout` loops. `SessionUpdate` events fire on every state change automatically.
- **Forgetting to return `true` from update():** State-changing events must return `true` to trigger `render()`. Forgetting this results in stale UI.
- **Using `println!` for status text:** Debug output via `println!` corrupts the render surface. Use `eprintln!` for logging, `print_text_with_coordinates` for rendering.
- **Hardcoding paths with `~/`:** WASM plugins cannot expand tildes. Use absolute paths in configuration.
- **Calling `set_selectable(false)` in `load()`:** Must wait until after `PermissionRequestResult` is received. Calling before permissions are granted prevents the user from granting permissions (the plugin pane can't be selected to interact with the permission dialog).
- **Killing the current session:** If the user presses x on the session they're currently in, the behavior is undefined/dangerous. Guard against killing `is_current_session`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Session switching | Custom IPC to Zellij | `switch_session()` / `switch_session_with_cwd()` | Official API handles session creation, resurrection, and tab switching atomically. |
| Session termination | Shell commands or manual cleanup | `kill_sessions(&[name])` | API handles graceful session shutdown including all panes and processes. |
| Text rendering with selection | Manual ANSI escape codes | `Text::new().selected().color_range()` + `print_text_with_coordinates()` | Official rendering API integrates with Zellij's theme system for consistent colors. |
| Focus keybind setup | Asking user to add keybinds manually | `reconfigure()` with `MessagePluginId` | Dynamic keybind injection avoids manual config, works across input modes, and doesn't persist to disk. |
| Tilde path expansion | Custom `~` -> home resolution | Require absolute paths in config | WASM sandbox doesn't reliably support `$HOME` or tilde expansion. Absolute paths are deterministic. |

**Key insight:** Every interaction in Phase 2 maps to a single Zellij API call. The plugin's job is to maintain state (project list + selection index + session status) and translate user actions into API calls. There is no complex business logic -- it's a thin UI layer over Zellij's session management API.

## Common Pitfalls

### Pitfall 1: Tilde Paths in WASM Sandbox
**What goes wrong:** Config entries like `~/Documents/Git/help-self` fail because the WASM sandbox cannot resolve `~` to the user's home directory.
**Why it happens:** WASM plugins run in a sandboxed environment. The `/host` path maps to the plugin's cwd, and `std::env::var("HOME")` may not be available.
**How to avoid:** Document that all project paths must be absolute (e.g., `/Users/andrewbeniston/Documents/01-Projects/Git/help-self`). Validate in `load()` and log a warning via `eprintln!` if a path starts with `~`.
**Warning signs:** Sessions created with wrong cwd, or `switch_session_with_cwd` silently creates sessions in the wrong directory.

### Pitfall 2: set_selectable(false) Before Permissions
**What goes wrong:** Plugin becomes unselectable before the permission dialog appears. User cannot select the plugin pane to grant permissions, resulting in a permanently broken plugin.
**Why it happens:** Calling `set_selectable(false)` in `load()` fires before the permission dialog.
**How to avoid:** Call `set_selectable(false)` only inside the `PermissionRequestResult::Granted` handler, after permissions are confirmed.
**Warning signs:** Plugin loads but permission dialog never appears or can't be interacted with.

### Pitfall 3: Killing the Current Session
**What goes wrong:** User presses x on the session they're currently using. `kill_sessions` terminates the active session, potentially disconnecting the user.
**Why it happens:** No guard against self-destruction.
**How to avoid:** Check `project.status == SessionStatus::Running { is_current: true }` before allowing kill. Show a visual indicator or silently ignore the x keypress on the current session.
**Warning signs:** User gets disconnected from their terminal session.

### Pitfall 4: Session Name Collisions
**What goes wrong:** Two projects have the same folder basename (e.g., `~/work/api` and `~/personal/api`). Both map to session name `api`, causing incorrect status display and session switching.
**Why it happens:** Session name is derived from folder basename without disambiguation.
**How to avoid:** Detect duplicate basenames during config parsing and warn via `eprintln!`. For v1, document that project folder basenames must be unique. Future: allow explicit session name override in config.
**Warning signs:** Wrong session activates when pressing Enter, or multiple projects show the same status.

### Pitfall 5: Key Events Only When Focused
**What goes wrong:** Developer expects j/k/Enter to work even when the plugin pane is not focused.
**Why it happens:** `Event::Key` only fires when the user is focused on the plugin pane. If `set_selectable(false)`, the user can never focus the pane via normal navigation.
**How to avoid:** The focus activation flow is: keybind -> pipe message -> `set_selectable(true)` + `show_self(false)` -> now Key events work -> Esc to deactivate.
**Warning signs:** Key events never arrive, plugin appears unresponsive.

### Pitfall 6: Render Called Before State is Ready
**What goes wrong:** `render()` is called before the first `SessionUpdate` event arrives. The project list shows all projects as "not started" even though sessions are running.
**Why it happens:** Zellij may call `render()` immediately after `load()`, before any events are processed.
**How to avoid:** Track whether the first `SessionUpdate` has been received. Show a "Loading..." message until then.
**Warning signs:** Brief flash of incorrect status on plugin startup.

## Code Examples

Verified patterns from official sources:

### Complete State Struct
```rust
// Source: Derived from session-manager State pattern + Phase 1 scaffold
use zellij_tile::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, PartialEq)]
enum SessionStatus {
    Running { is_current: bool },
    Exited,
    NotStarted,
}

#[derive(Clone)]
struct Project {
    name: String,
    path: String,
    status: SessionStatus,
}

#[derive(Default)]
struct State {
    permissions_granted: bool,
    projects: Vec<Project>,
    selected_index: usize,
    is_focused: bool,
    initial_load_complete: bool,
}

register_plugin!(State);
```

### KDL Configuration Block (user's config.kdl or layout file)
```kdl
// Source: Verified KDL plugin config pattern from zjstatus, zellij-sessionizer
layout {
    pane size=1 borderless=true {
        plugin location="tab-bar"
    }
    pane split_direction="vertical" {
        pane size=25 {
            plugin location="file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm" {
                project_0 "/Users/andrewbeniston/Documents/01-Projects/Git/help-self"
                project_1 "/Users/andrewbeniston/Documents/01-Projects/Git/tungsten-flow"
                project_2 "/Users/andrewbeniston/Documents/01-Projects/Git/svg-editor"
                project_3 "/Users/andrewbeniston/Documents/01-Projects/Git/zellij-project-sidebar"
            }
        }
        pane
    }
    pane size=1 borderless=true {
        plugin location="status-bar"
    }
}
```

### MessagePluginId Keybind Setup (Rust)
```rust
// Source: Official Zellij tutorial on reconfigure() + MessagePluginId
fn setup_focus_keybind(plugin_id: u32) {
    let config = format!(
        r#"
        keybinds {{
            shared {{
                bind "Alt s" {{
                    MessagePluginId {plugin_id} {{
                        name "focus_sidebar"
                    }}
                }}
            }}
        }}
        "#,
    );
    reconfigure(config, false);
}
```

### switch_session_with_cwd Signature
```rust
// Source: https://docs.rs/zellij-tile/latest/zellij_tile/shim/fn.switch_session_with_cwd.html
pub fn switch_session_with_cwd(name: Option<&str>, cwd: Option<PathBuf>)
// Creates a new session with the given name and working directory if it doesn't exist.
// Switches to it if it already exists.
```

### switch_session_with_layout Signature (alternative)
```rust
// Source: https://docs.rs/zellij-tile/latest/zellij_tile/shim/fn.switch_session_with_layout.html
pub fn switch_session_with_layout(
    name: Option<&str>,
    layout: LayoutInfo,
    cwd: Option<PathBuf>,
)
// Use LayoutInfo::BuiltIn("default".to_string()) for default layout
// or LayoutInfo::File("/path/to/layout.kdl".to_string()) for custom
```

### kill_sessions Signature
```rust
// Source: https://docs.rs/zellij-tile/latest/zellij_tile/shim/fn.kill_sessions.html
pub fn kill_sessions(sessions: &[String])
// Terminates all listed sessions by name
// Requires ChangeApplicationState permission
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `println!` with ANSI codes for rendering | `print_text_with_coordinates(Text::new().color_range())` | Zellij 0.39+ | Theme-integrated rendering, no manual ANSI management |
| Manual keybind instructions for users | `reconfigure()` + `MessagePluginId` dynamic keybinds | Zellij 0.41.0 (Nov 2024) | Plugin self-configures keybinds at runtime without persisting to disk |
| `switch_session(name)` only | `switch_session_with_cwd(name, cwd)` available | Zellij 0.40+ | Session creation with working directory in a single API call |
| Key event used `Key` enum (old) | `KeyWithModifier` with `BareKey` + modifier set | Zellij 0.40+ | Cleaner key matching with `.has_no_modifiers()`, `.bare_key` pattern matching |

**Deprecated/outdated:**
- Old `Key` enum (pre-0.40): Replaced by `KeyWithModifier` struct with `bare_key: BareKey` field.
- Manual ANSI rendering: Still works (session-manager uses it) but `Text` API is preferred for theme integration.
- `Event::Key(Key)` pattern: Now `Event::Key(KeyWithModifier)` -- code from old tutorials won't compile.

## Open Questions

1. **Exact keybind for sidebar focus activation**
   - What we know: `reconfigure()` + `MessagePluginId` works for any keybind. Phase 3 will use Cmd+P (Super+p) for full toggle.
   - What's unclear: Best temporary keybind for Phase 2. Alt+s avoids conflicts with common Zellij keybinds.
   - Recommendation: Use `Alt s` for Phase 2 focus activation. Phase 3 will replace this with the full Cmd+P pipe toggle.

2. **switch_session_with_cwd absolute path handling**
   - What we know: The function signature accepts `Option<PathBuf>`. The sessionizer plugin passes full paths successfully.
   - What's unclear: Whether the path is interpreted relative to the WASM sandbox or the host filesystem. Evidence strongly suggests host filesystem (since session-manager and sessionizer use absolute host paths).
   - Recommendation: Use absolute host paths. Test during implementation to confirm.

3. **Resurrectable session behavior with switch_session**
   - What we know: `switch_session(Some("name"))` switches to a running session. Exited sessions appear in the resurrectable list.
   - What's unclear: Does `switch_session` automatically resurrect an exited session, or do we need `delete_dead_session` first?
   - Recommendation: Test with a resurrectable session during implementation. The session-manager uses `switch_session` for resurrection, suggesting it auto-resurrects.

4. **show_self behavior when set_selectable was false**
   - What we know: `show_self(false)` "unsuppresses, focuses, and switches to tab". `set_selectable(true)` must be called first.
   - What's unclear: Whether `show_self` also makes the pane selectable, or if `set_selectable(true)` must be called explicitly before it.
   - Recommendation: Always call `set_selectable(true)` before `show_self(false)` to be safe. Test during implementation.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Manual verification (Zellij WASM plugin -- no unit test harness for WASM plugins) |
| Config file | none -- WASM plugins tested by loading into Zellij |
| Quick run command | `cargo build && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm` |
| Full suite command | `cargo build --release` (compilation = structural correctness) + manual Zellij verification |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DISP-01 | Project list rendered from KDL config | smoke | `cargo build` (compile check) + manual: load plugin with project config, verify list displays | N/A |
| DISP-02 | Live session status (running/exited/not started) | manual | Load plugin, create/kill sessions, verify status changes in real-time | N/A |
| DISP-03 | Current session visually highlighted | manual | Switch between sessions, verify highlight moves to current session | N/A |
| INTR-01 | j/k navigation with visual selection | manual | Focus plugin, press j/k, verify selection cursor moves | N/A |
| INTR-02 | Enter switches to running session | manual | Select a running session, press Enter, verify Zellij switches to it | N/A |
| INTR-03 | Enter creates session with cwd if none exists | manual | Select a project with no session, press Enter, verify new session created with correct cwd | N/A |
| INTR-04 | x kills a running session | manual | Select a running session, press x, verify session terminated and status updates | N/A |
| INFR-04 | Plugin unselectable by default, selectable on activation | manual | Verify Tab navigation skips plugin pane. Press focus keybind, verify plugin becomes focusable. Press Esc, verify plugin becomes unselectable again. | N/A |

### Sampling Rate
- **Per task commit:** `cargo build` (compilation check)
- **Per wave merge:** Load plugin in Zellij, verify rendering and basic interaction
- **Phase gate:** All 8 requirements verified manually in running Zellij with multiple sessions

### Wave 0 Gaps
None -- existing project structure from Phase 1 is sufficient. The only file modified is `src/main.rs`. New KDL layout with plugin configuration will be added as part of implementation tasks.

## Sources

### Primary (HIGH confidence)
- [zellij-tile shim functions](https://docs.rs/zellij-tile/latest/zellij_tile/shim/index.html) -- complete API surface: switch_session, switch_session_with_cwd, switch_session_with_layout, kill_sessions, set_selectable, show_self, hide_self, print_text_with_coordinates, reconfigure, get_plugin_ids
- [Event enum](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.Event.html) -- Key(KeyWithModifier), SessionUpdate, PermissionRequestResult, Visible
- [EventType enum](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.EventType.html) -- 35 variants including Key, SessionUpdate, Visible
- [BareKey enum](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.BareKey.html) -- Char(char), Enter, Esc, Up, Down, etc.
- [KeyWithModifier struct](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/struct.KeyWithModifier.html) -- bare_key, key_modifiers, has_no_modifiers(), has_modifiers()
- [SessionInfo struct](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/struct.SessionInfo.html) -- name, tabs, is_current_session, connected_clients (no cwd field)
- [LayoutInfo enum](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.LayoutInfo.html) -- BuiltIn, File, Url, Stringified variants
- [Plugin UI Rendering](https://zellij.dev/documentation/plugin-ui-rendering.html) -- Text, NestedListItem, print_text_with_coordinates, color_range, selected
- [Plugin API Commands](https://zellij.dev/documentation/plugin-api-commands.html) -- set_selectable, show_self, hide_self, switch_session docs
- [Plugin API Events](https://zellij.dev/documentation/plugin-api-events.html) -- Key event fires when user focused on plugin pane
- [Zellij Rust Plugin Tutorial](https://zellij.dev/tutorials/developing-a-rust-plugin/) -- reconfigure() + MessagePluginId keybind example, get_plugin_ids, configuration BTreeMap
- [Plugin Filesystem](https://zellij.dev/documentation/plugin-api-file-system.html) -- /host, /data, /tmp mapping (sandbox constraints)
- [Keybinding Actions](https://zellij.dev/documentation/keybindings-possible-actions.html) -- MessagePlugin, MessagePluginId action syntax

### Secondary (MEDIUM confidence)
- [Zellij session-manager source](https://github.com/zellij-org/zellij/blob/main/default-plugins/session-manager/src/main.rs) -- BareKey matching pattern, kill_sessions usage, switch_session_with_focus, rendering with ANSI + print_text_with_coordinates
- [zellij-sessionizer source](https://github.com/cunialino/zellij-sessionizer) -- switch_session_with_cwd usage with PathBuf, configuration parsing from BTreeMap, key event handling
- [zjstatus KDL config](https://github.com/dj95/zjstatus) -- Real-world example of KDL plugin configuration with key-value pairs
- [Common Zellij Plugin Snippets](https://blog.nerd.rocks/posts/common-snippets-for-zellij-development/) -- set_selectable(false) after PermissionRequestResult pattern, pending_events queue

### Tertiary (LOW confidence)
- [WASI Home Directory Issue](https://github.com/WebAssembly/wasi-filesystem/issues/59) -- Confirms HOME env var may not be available in WASI sandbox
- [Zellij env var issue #4240](https://github.com/zellij-org/zellij/issues/4240) -- Confirms env vars not exposed to plugin config (but may be available via WASI runtime)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- No new dependencies, all APIs verified in docs.rs and official tutorials
- Architecture: HIGH -- Patterns directly derived from session-manager and sessionizer source code (production plugins)
- Pitfalls: HIGH -- Tilde expansion, set_selectable timing, and current-session kill guard all verified from official docs and real-world plugin patterns
- Rendering: HIGH -- print_text_with_coordinates, Text, color_range all verified from official rendering docs
- Focus management: MEDIUM -- set_selectable + reconfigure + MessagePluginId verified individually, but the full toggle flow (unselectable -> pipe -> selectable -> Esc -> unselectable) hasn't been verified as an end-to-end pattern in a production plugin

**Research date:** 2026-03-11
**Valid until:** 2026-04-11 (stable domain, Zellij 0.43.1 is latest, APIs are stable)