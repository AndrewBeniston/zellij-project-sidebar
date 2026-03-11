# Architecture Patterns

**Domain:** Zellij WASM Plugin (Sidebar Session Manager)
**Researched:** 2026-03-11
**Confidence:** HIGH (official docs, API reference, existing plugin source analysis)

## Recommended Architecture

A Zellij plugin is a single Rust binary compiled to `wasm32-wasi`. It runs inside Zellij's `wasmi` WASM interpreter in a sandboxed environment. There is no networking, no filesystem access beyond the startup folder (without permission), and no threads. The plugin communicates with Zellij exclusively through a command/event protocol serialised as Protocol Buffers.

The sidebar plugin has six architectural components: **Lifecycle**, **State**, **Event Router**, **Renderer**, **Configuration**, and **Toggle Mechanism**. These map directly to the `ZellijPlugin` trait methods and the Zellij host API.

```
+------------------+     Events (Key, SessionUpdate,     +------------------+
|                  |     PaneUpdate, TabUpdate, Pipe)     |                  |
|   Zellij Host    | ----------------------------------> |   Event Router   |
|                  |                                     |   (update/pipe)  |
+------------------+                                     +--------+---------+
        ^                                                         |
        |  Commands (switch_session,                              |  mutates
        |  kill_sessions, hide_self,                              v
        |  show_self, reconfigure)                       +------------------+
        |                                                |                  |
        +----------------------------------------------- |   Plugin State   |
        |                                                |                  |
        |                                                +--------+---------+
        |                                                         |
        |                                                         | reads
        |                                                         v
        |                                                +------------------+
        |    ANSI/DCS output (Text, Table,               |                  |
        +----------------------------------------------- |    Renderer      |
             print_text_with_coordinates)                 |    (render)      |
                                                         +------------------+
```

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| **Lifecycle** (`load`) | Permission requests, event subscriptions, keybind registration, config parsing | Zellij Host (one-time setup) |
| **State** (struct) | Holds session list, selection index, visibility flag, config, cached pane/tab data | Read by Renderer, mutated by Event Router |
| **Event Router** (`update` + `pipe`) | Dispatches incoming events to state mutation logic; returns `true` when re-render needed | State (write), Zellij Host (commands) |
| **Renderer** (`render`) | Produces ANSI/DCS output for the sidebar UI given current state and available dimensions | State (read-only), Zellij Host (stdout) |
| **Configuration** | Parsed from `BTreeMap<String, String>` in `load`; holds pinned folders, verbosity, theme colours | State (owned field) |
| **Toggle Mechanism** | Keybind-triggered pipe message that calls `hide_self()`/`show_self()` | Event Router (pipe handler), Zellij Host |

## Data Flow

### 1. Initialisation (load)

```
load(config: BTreeMap<String, String>)
  |
  +-- Parse config -> extract pinned_folders, verbosity, theme settings
  +-- request_permission([ReadApplicationState, ChangeApplicationState])
  +-- subscribe([SessionUpdate, TabUpdate, PaneUpdate, Key, Visible])
  +-- Register toggle keybind via reconfigure() + MessagePluginId
  +-- Store parsed config in State
```

**Config comes from KDL.** Zellij passes plugin configuration as `BTreeMap<String, String>`. The plugin parses this in `load()`. Example KDL:

```kdl
plugin location="file:~/.config/zellij/plugins/project-sidebar.wasm" {
    pinned_folders "~/Documents/01-Projects/Git/help-self,~/Documents/01-Projects/Git/svg-editor"
    verbosity "full"
}
```

### 2. Session Data Flow (update)

```
Zellij emits SessionUpdate(Vec<SessionInfo>, Vec<(String, Duration)>)
  |
  +-- update() receives Event::SessionUpdate(sessions, resurrectable_sessions)
  |     |
  |     +-- For each pinned folder in config:
  |     |     Match against sessions by name or cwd
  |     |     Determine status: Running / Exited / Not Started
  |     |     Extract tab count from SessionInfo.tabs.len()
  |     |     Extract active pane command from PaneInfo.terminal_command
  |     |
  |     +-- Store matched session data in State.projects
  |     +-- Return true (re-render needed)
  |
  +-- render() reads State.projects -> draws sidebar
```

**Key data structures from the API:**

- `SessionInfo` -- name, tabs (`Vec<TabInfo>`), panes (`PaneManifest`), `is_current_session`, `connected_clients`
- `TabInfo` -- position, name, active, pane counts
- `PaneManifest` -- `HashMap<usize, Vec<PaneInfo>>` (panes indexed by tab position)
- `PaneInfo` -- id, title, `terminal_command` (Option<String>), `is_focused`, `is_plugin`, `exited`, `exit_status`

The `SessionUpdate` event fires whenever session state changes (new session created, session killed, tab opened, etc). It provides ALL sessions, not deltas -- the plugin rebuilds its view each time.

### 3. Keyboard Input Flow (update)

```
User presses key while plugin is focused
  |
  +-- Zellij sends Event::Key(KeyWithModifier)
  |
  +-- update() pattern matches:
  |     j / Down  -> State.selected_idx += 1 (with wrap)
  |     k / Up    -> State.selected_idx -= 1 (with wrap)
  |     Enter     -> switch_session(selected_project.session_name)
  |                  OR switch_session_with_focus(name, tab, pane)
  |     x         -> kill_sessions([selected_project.session_name])
  |     Esc / q   -> hide_self()
  |
  +-- Return true for navigation, false for no-ops
```

### 4. Toggle Visibility Flow (pipe)

```
User presses Cmd+P (Super p in Zellij)
  |
  +-- Zellij keybind fires MessagePluginId with message name "toggle"
  |
  +-- pipe(pipe_message: PipeMessage) called
  |     |
  |     +-- Check: pipe_message.name == "toggle"
  |     +-- Check: pipe_message.source == PipeSource::Keybind
  |     +-- Toggle State.visible
  |     +-- If visible: show_self()
  |     +-- If hidden: hide_self()
  |     +-- Return true
```

The toggle keybind is registered in `load()` using `reconfigure()`:

```rust
fn load(&mut self, config: BTreeMap<String, String>) {
    let plugin_ids = get_plugin_ids();
    // Bind Super+p to pipe "toggle" to this plugin instance
    reconfigure(
        format!(r#"
            keybinds {{
                shared {{
                    bind "Super p" {{
                        MessagePluginId {} {{
                            name "toggle"
                        }}
                    }}
                }}
            }}
        "#, plugin_ids.plugin_id),
        false // don't write to disk (temporary keybind)
    );
}
```

### 5. Render Flow

```
render(rows: usize, cols: usize) called by Zellij
  |
  +-- Previous frame is auto-cleared by Zellij
  |
  +-- For each project in State.projects:
  |     +-- Compute display string: icon + name + status dot + [tab_count]
  |     +-- If verbosity == "full": append active command
  |     +-- Apply selection highlight if idx == selected_idx
  |     +-- Apply current-session indicator if is_current
  |     +-- Use color_range() for theme colours (indices 0-3, theme-aware)
  |     +-- print_text_with_coordinates(text, x, y, Some(cols), None)
  |
  +-- Render header (optional title)
  +-- Render footer (keybind hints if space permits)
```

**Rendering is character-cell based.** The plugin receives `rows` and `cols` and must fit content within. Every `render()` call starts from a blank slate -- Zellij clears the previous frame. Output is either raw ANSI via `println!()` or structured `Text` components via `print_text_with_coordinates()`.

The `Text` component supports:
- `.selected()` -- background highlight for focused items
- `.color_range(index, range)` -- theme-aware colours (0-3 mapped to user's theme)
- Nested components: `Table`, `Ribbon`, `NestedList`

**Use `print_text_with_coordinates` over raw `println!`.** It positions content precisely and integrates with Zellij's theme system. Raw ANSI works but bypasses theming.

## Patterns to Follow

### Pattern 1: Event-Driven State Machine

**What:** All state changes flow through `update()` or `pipe()`. No polling, no timers (unless explicitly set with `set_timeout`). The plugin is reactive.

**When:** Always. This is the only pattern Zellij supports.

**Example:**
```rust
#[derive(Default)]
struct State {
    projects: Vec<ProjectEntry>,
    selected_idx: usize,
    visible: bool,
    config: PluginConfig,
}

struct ProjectEntry {
    name: String,
    folder: PathBuf,
    status: SessionStatus,
    tab_count: usize,
    active_command: Option<String>,
    is_current: bool,
}

enum SessionStatus {
    Running,
    Exited,      // resurrectable
    NotStarted,  // no session exists
}
```

### Pattern 2: Selective Re-rendering

**What:** `update()` and `pipe()` return `bool`. Return `true` only when state actually changed. Zellij skips `render()` when `false`.

**When:** Every event handler. Prevents unnecessary redraws.

**Example:**
```rust
fn update(&mut self, event: Event) -> bool {
    match event {
        Event::SessionUpdate(sessions, resurrectable) => {
            let new_projects = self.build_project_list(&sessions, &resurrectable);
            if new_projects != self.projects {
                self.projects = new_projects;
                true  // state changed, re-render
            } else {
                false // no change, skip render
            }
        }
        Event::Key(key) => self.handle_key(key),
        _ => false,
    }
}
```

### Pattern 3: Config-as-BTreeMap Parsing

**What:** Plugin config arrives as `BTreeMap<String, String>`. Parse in `load()`, store in a typed struct.

**When:** Always. No other config mechanism exists for plugins.

**Example:**
```rust
struct PluginConfig {
    pinned_folders: Vec<PathBuf>,
    verbosity: Verbosity,
}

enum Verbosity {
    Minimal,  // name + status dot
    Normal,   // name + status + tab count
    Full,     // name + status + tab count + active command
}

impl PluginConfig {
    fn from_btreemap(config: &BTreeMap<String, String>) -> Self {
        let folders = config.get("pinned_folders")
            .map(|s| s.split(',').map(|p| PathBuf::from(p.trim())).collect())
            .unwrap_or_default();

        let verbosity = match config.get("verbosity").map(|s| s.as_str()) {
            Some("minimal") => Verbosity::Minimal,
            Some("full") => Verbosity::Full,
            _ => Verbosity::Normal,
        };

        Self { pinned_folders: folders, verbosity }
    }
}
```

### Pattern 4: Session Matching by Name Convention

**What:** Match pinned folders to sessions by deriving session name from folder name. The user names sessions after projects (e.g., folder `help-self` maps to session `help-self`).

**When:** On every `SessionUpdate`. This is the core data-join logic.

**Example:**
```rust
fn build_project_list(
    &self,
    sessions: &[SessionInfo],
    resurrectable: &[(String, Duration)],
) -> Vec<ProjectEntry> {
    self.config.pinned_folders.iter().map(|folder| {
        let name = folder.file_name().unwrap().to_string_lossy().to_string();

        // Check running sessions first
        if let Some(session) = sessions.iter().find(|s| s.name == name) {
            ProjectEntry {
                name: name.clone(),
                folder: folder.clone(),
                status: SessionStatus::Running,
                tab_count: session.tabs.len(),
                active_command: self.extract_active_command(session),
                is_current: session.is_current_session,
            }
        }
        // Check resurrectable (exited) sessions
        else if resurrectable.iter().any(|(n, _)| n == &name) {
            ProjectEntry {
                name, folder: folder.clone(),
                status: SessionStatus::Exited,
                tab_count: 0, active_command: None, is_current: false,
            }
        }
        // No session exists
        else {
            ProjectEntry {
                name, folder: folder.clone(),
                status: SessionStatus::NotStarted,
                tab_count: 0, active_command: None, is_current: false,
            }
        }
    }).collect()
}
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: Polling for State

**What:** Using `set_timeout` to periodically check session state.
**Why bad:** `SessionUpdate` events already fire on every state change. Polling wastes CPU and adds latency.
**Instead:** Subscribe to `SessionUpdate`, `TabUpdate`, `PaneUpdate` and react to events.

### Anti-Pattern 2: Storing Raw SessionInfo

**What:** Keeping `Vec<SessionInfo>` as plugin state and re-deriving display data in `render()`.
**Why bad:** `SessionInfo` contains far more data than needed (layouts, plugin lists, web client state). Bloats WASM memory. Mixing data transformation with rendering makes both harder to test.
**Instead:** Transform `SessionInfo` into a slim `ProjectEntry` in `update()`, store only what `render()` needs.

### Anti-Pattern 3: Raw ANSI Instead of Text Components

**What:** Using `print!("\x1b[32m...")` for colouring instead of `Text::new().color_range()`.
**Why bad:** Bypasses Zellij's theme system. Will look wrong in any theme other than the one you tested with.
**Instead:** Use `Text` with `color_range(0..3)` indices. These map to the user's active theme automatically.

### Anti-Pattern 4: Handling Toggle in update() via Key Event

**What:** Listening for the toggle key in `update()` via `Event::Key`.
**Why bad:** `Event::Key` only fires when the plugin pane is focused. If the plugin is hidden or another pane is focused, the key event never reaches the plugin.
**Instead:** Use `pipe()` with `MessagePluginId` keybinding. Pipe messages are delivered regardless of focus.

### Anti-Pattern 5: Mutable Render

**What:** Modifying state inside `render()`.
**Why bad:** `render()` can be called multiple times, or not at all. State changes in render create unpredictable behaviour.
**Instead:** All state mutation in `update()` or `pipe()`. Render reads state immutably.

## Docked Sidebar: Layout Approach

The sidebar is a **tiled pane with fixed width** in a Zellij layout, not a floating pane. This is the standard approach for persistent side panels.

**Layout KDL:**
```kdl
layout {
    pane split_direction="vertical" {
        pane size=30 {
            plugin location="file:~/.config/zellij/plugins/project-sidebar.wasm" {
                pinned_folders "~/Documents/01-Projects/Git/help-self,~/Documents/01-Projects/Git/svg-editor"
                verbosity "normal"
            }
        }
        pane  // main workspace expands to fill
    }
}
```

The plugin uses `hide_self()` / `show_self()` to toggle visibility. When hidden, Zellij reclaims the space for adjacent panes. When shown, it reappears at its configured width.

**Alternative: `load_plugins` for background startup.** The plugin can be loaded in the background on session start via `load_plugins` in `config.kdl`, then `show_self()` when the user triggers the toggle keybind. This avoids needing the layout to include the plugin pane -- the plugin creates its own pane on first show.

## Suggested Build Order (Dependencies)

Build order follows a dependency chain where each phase produces a testable artifact:

```
Phase 1: Scaffold + Lifecycle
    |  (produces: compilable plugin that loads, requests permissions, logs)
    v
Phase 2: Configuration Parsing
    |  (produces: plugin that reads pinned_folders from KDL config)
    v
Phase 3: Session Data Model
    |  (produces: plugin that receives SessionUpdate, builds ProjectEntry list, logs to stderr)
    |  DEPENDS ON: Phase 2 (needs pinned_folders to match against sessions)
    v
Phase 4: Renderer (Core)
    |  (produces: plugin that displays project list with status indicators)
    |  DEPENDS ON: Phase 3 (needs ProjectEntry data to render)
    v
Phase 5: Keyboard Navigation
    |  (produces: j/k selection, Enter to switch, x to kill)
    |  DEPENDS ON: Phase 4 (needs visual feedback for selection)
    v
Phase 6: Toggle Mechanism
    |  (produces: Cmd+P toggles sidebar visibility via pipe)
    |  DEPENDS ON: Phase 5 (full interaction model)
    v
Phase 7: Polish + Verbosity Modes
    (produces: Catppuccin theme, info density config, edge cases)
    DEPENDS ON: Phase 6 (all features in place)
```

**Why this order:**
1. **Scaffold first** -- can't do anything without a compiling WASM plugin
2. **Config before data** -- need folder list to know what to match
3. **Data before render** -- need data model to know what to display
4. **Render before navigation** -- need visual output to verify navigation
5. **Navigation before toggle** -- core interaction before convenience features
6. **Toggle before polish** -- feature-complete before aesthetic refinement

## Key Architectural Decisions

| Decision | Rationale |
|----------|-----------|
| Single `State` struct, no ECS/component system | Plugin is simple enough for a flat struct. Over-engineering here wastes time. |
| Transform SessionInfo in `update()`, not `render()` | Keeps render pure. Enables future diff-based skip of unchanged frames. |
| `MessagePluginId` pipe for toggle, not `Event::Key` | Key events require focus. Pipe messages work from any context. |
| Theme colours via `color_range(0-3)`, not hardcoded ANSI | Automatic Catppuccin Frappe support without hardcoding. Future theme changes work for free. |
| Fixed-width tiled pane in layout | Docked sidebar feel. `hide_self()`/`show_self()` handles toggle. Floating would feel wrong for an "always there" panel. |
| No background workers | All operations are synchronous event handling. No long-running tasks. Workers add complexity with no benefit here. |

## Sources

- [Zellij Plugin Development Tutorial](https://zellij.dev/tutorials/developing-a-rust-plugin/) -- HIGH confidence, official
- [Plugin API Commands](https://zellij.dev/documentation/plugin-api-commands.html) -- HIGH confidence, official
- [Plugin API Events](https://zellij.dev/documentation/plugin-api-events.html) -- HIGH confidence, official
- [Plugin API Permissions](https://zellij.dev/documentation/plugin-api-permissions.html) -- HIGH confidence, official
- [Plugin UI Rendering](https://zellij.dev/documentation/plugin-ui-rendering.html) -- HIGH confidence, official
- [Plugin Pipes](https://zellij.dev/documentation/plugin-pipes.html) -- HIGH confidence, official
- [Workers for Async Tasks](https://zellij.dev/documentation/plugin-api-workers.html) -- HIGH confidence, official
- [Creating Layouts](https://zellij.dev/documentation/creating-a-layout.html) -- HIGH confidence, official
- [Loading Plugins](https://zellij.dev/documentation/plugin-loading.html) -- HIGH confidence, official
- [zellij-tile API docs](https://docs.rs/zellij-tile/latest/zellij_tile/) -- HIGH confidence, official
- [SessionInfo struct](https://docs.rs/zellij-utils/latest/zellij_utils/data/struct.SessionInfo.html) -- HIGH confidence, official
- [PaneInfo struct](https://docs.rs/zellij-utils/latest/zellij_utils/data/struct.PaneInfo.html) -- HIGH confidence, official
- [TabInfo struct](https://docs.rs/zellij-utils/latest/zellij_utils/data/struct.TabInfo.html) -- HIGH confidence, official
- [rust-plugin-example](https://github.com/zellij-org/rust-plugin-example) -- HIGH confidence, official template
- [zellij-sessionizer](https://github.com/cunialino/zellij-sessionizer) -- MEDIUM confidence, community plugin
- [zellij-choose-tree](https://github.com/laperlej/zellij-choose-tree) -- MEDIUM confidence, community plugin
- [DeepWiki: Plugin Communication](https://deepwiki.com/zellij-org/zellij/3.4-plugin-communication) -- MEDIUM confidence
- [DeepWiki: Built-in Plugins](https://deepwiki.com/zellij-org/zellij/4.3-built-in-plugins) -- MEDIUM confidence
