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

# Technology Stack

**Project:** Zellij Project Sidebar
**Researched:** 2026-03-11

## Recommended Stack

### Core Framework

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Rust (stable) | 1.88+ | Plugin language | Only supported language for Zellij WASM plugins. User has 1.88.0 installed. | HIGH |
| zellij-tile | 0.43.1 | Plugin API crate | Official Rust SDK for Zellij plugins. Must match installed Zellij version (user has 0.43.1). Provides `ZellijPlugin` trait, event system, rendering primitives, session/tab/pane management commands. | HIGH |
| wasm32-wasip1 | (target) | Compilation target | Required WASM target for Zellij plugins. Note: the old name `wasm32-wasi` was removed from stable Rust in 1.84 (Jan 2025). Must use `wasm32-wasip1` now. User needs to run `rustup target add wasm32-wasip1`. | HIGH |

### Supporting Libraries

| Library | Version | Purpose | When to Use | Confidence |
|---------|---------|---------|-------------|------------|
| serde | ^1.0, features=["derive"] | Config deserialization | Parsing plugin configuration from KDL block attributes into Rust structs. Already a transitive dep of zellij-tile but needed explicitly for derive macros. | HIGH |
| serde_json | ^1.0 | JSON serialization | Already a transitive dep of zellij-tile. Useful if storing/reading structured config. Only add explicitly if needed for custom serialization. | MEDIUM |

### Build Tooling

| Tool | Version | Purpose | Why | Confidence |
|------|---------|---------|-----|------------|
| cargo | 1.88+ | Build system | Standard Rust build. No special build tool needed -- just `cargo build --release` with the wasm32-wasip1 target configured in `.cargo/config.toml`. | HIGH |
| rustup | latest | Toolchain management | Need it to install `wasm32-wasip1` target: `rustup target add wasm32-wasip1` | HIGH |

### Development Environment

| Tool | Purpose | Why |
|------|---------|-----|
| zellij dev layout (zellij.kdl) | Hot-reload during development | Official pattern: a KDL layout file that opens editor panes + a build pane running `cargo build && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm`. Ctrl+Shift+R triggers rebuild via `develop-rust-plugin`. |
| watchexec (optional) | File watcher for auto-rebuild | Alternative to manual Ctrl+Shift+R. Watches `src/` and triggers `cargo build && zellij action start-or-reload-plugin`. Nice but not essential. |

## Critical Version Alignment

**zellij-tile version MUST match the installed Zellij version.**

The user has Zellij 0.43.1 installed. The zellij-tile crate is at 0.43.1 on crates.io. This is the version to use. Using an older version (e.g., 0.41.1 from templates) will miss API features and may cause protobuf deserialization issues.

zjstatus (the most actively maintained community plugin) uses zellij-tile 0.43.1 with Rust edition 2024, confirming this is the current standard.

## Project Configuration Files

### `.cargo/config.toml`

```toml
[build]
target = "wasm32-wasip1"
```

Sets the default compilation target so every `cargo build` produces WASM without needing `--target` flag.

### `Cargo.toml`

```toml
[package]
name = "zellij-project-sidebar"
version = "0.1.0"
edition = "2021"

[dependencies]
zellij-tile = "0.43.1"
serde = { version = "1.0", features = ["derive"] }

[profile.release]
opt-level = "s"
lto = true
strip = "debuginfo"
```

**Edition rationale:** Use 2021 (not 2024). While zjstatus uses 2024, it has a more complex dependency graph. Edition 2021 is the safest choice for a new wasm32-wasip1 project -- fully stable, no edge cases with WASM target. The 2024 edition works but offers no meaningful benefit for this project's scope.

**Release profile rationale:** WASM binary size directly affects plugin load time. `opt-level = "s"` optimizes for size, `lto = true` enables link-time optimization across crate boundaries for smaller output, `strip = "debuginfo"` removes debug symbols from release builds. This is standard practice for all Zellij community plugins.

### `src/main.rs` (skeleton)

```rust
use zellij_tile::prelude::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct State {
    // Plugin state goes here
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // Subscribe to events, request permissions
    }

    fn update(&mut self, event: Event) -> bool {
        // Handle events, return true to trigger re-render
        false
    }

    fn render(&mut self, rows: usize, cols: usize) {
        // Character-cell rendering with Text, Table, NestedList
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        // Handle piped messages from keybinds/CLI
        false
    }
}
```

### `zellij.kdl` (development layout)

```kdl
layout {
    pane split_direction="vertical" {
        pane edit="src/main.rs"
        pane split_direction="horizontal" {
            pane edit="Cargo.toml"
            pane command="bash" {
                args "-c" "cargo build && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm"
            }
        }
    }
    pane size=30 {
        plugin location="file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm"
    }
}
```

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Plugin crate | zellij-tile 0.43.1 | zellij-tile-extra | Extra is a community crate with minimal adoption. zellij-tile is the official SDK. |
| Build target | wasm32-wasip1 | wasm32-wasi | Removed from Rust stable in 1.84 (Jan 2025). Will not compile. |
| Build target | wasm32-wasip1 | wasm32-wasip2 | Zellij uses wasip1. wasip2 is not supported by the Zellij runtime. |
| Rust edition | 2021 | 2024 | 2024 works but adds no value for this scope. 2021 is battle-tested for wasm32-wasip1. |
| Rust edition | 2021 | 2018 | Old templates (zellij-org/rust-plugin-example) use 2018. No reason to use an outdated edition. |
| Serialization | serde (built-in dep) | Manual parsing | zellij-tile already depends on serde. Fighting it is pointless. |
| Hot reload | zellij dev layout | cargo-watch + separate terminal | Dev layout is self-contained inside Zellij. External watchers add unnecessary complexity. |
| Template | Manual setup | cargo-generate + rust-plugin-template | Template uses outdated zellij-tile 1.0.0 (a placeholder version). Manual setup with correct 0.43.1 version is more reliable. |
| Template | Manual setup | create-rust-plugin (zellij plugin) | Useful for learning but generates scaffolding we will customize immediately. Not worth the indirection. |
| Extra deps | None | kdl crate (6.5.0) | zjstatus uses this for parsing its complex config format. Our config is simple key-value from KDL block attributes -- the `BTreeMap<String, String>` from `load()` is sufficient. |
| Extra deps | None | chrono, regex, uuid | zjstatus dependencies for its statusbar features. We have no time/regex/ID needs. Keep deps minimal for WASM size. |

## What NOT to Use

| Technology | Why Not |
|------------|---------|
| `wasm32-wasi` target | Removed from Rust stable. Must use `wasm32-wasip1`. |
| `wasm32-wasip2` target | Zellij does not support wasip2 runtime. Plugin will fail to load. |
| `wasm-bindgen` / `serde-wasm-bindgen` | These are for browser WASM. Zellij uses WASI, not browser APIs. |
| `cargo-wasi` subcommand | Deprecated in favor of standard `cargo build` with target in `.cargo/config.toml`. |
| `proxy-wasm` | For Envoy/Istio proxies, not terminal multiplexer plugins. |
| Any async runtime (tokio, async-std) | WASM plugins are single-threaded event-driven. Use Zellij's `ZellijWorker` trait for background tasks and `set_timeout()` for timers. |
| `zellij-tile-utils` | Minimal crate, not widely adopted. Standard lib utilities suffice. |
| Mouse handling crates | Project is keyboard-first (v1). Zellij provides `Mouse` event directly. |

## Plugin API Surface (What We Need)

### Permissions Required

| Permission | Why | API Functions Unlocked |
|------------|-----|----------------------|
| `ReadApplicationState` | Read session list, tab info, pane info, active session | `SessionUpdate`, `TabUpdate`, `PaneUpdate` events |
| `ChangeApplicationState` | Switch sessions, kill sessions, create sessions | `switch_session()`, `switch_session_with_cwd()`, `kill_sessions()` |

### Events to Subscribe

| Event | Data Type | Use Case |
|-------|-----------|----------|
| `SessionUpdate` | `Vec<SessionInfo>, Vec<(String, Duration)>` | Live session list with tab/pane counts, active session indicator |
| `Key` | `KeyWithModifier` | Keyboard navigation (j/k, Enter, x, Esc) |
| `PaneUpdate` | `PaneManifest` | Active pane command display (what's running in focused pane) |

### Key API Functions

| Function | Purpose |
|----------|---------|
| `switch_session(name)` | Switch to existing session |
| `switch_session_with_cwd(name, cwd)` | Create + switch to session with working directory |
| `kill_sessions(names)` | Kill session from sidebar |
| `hide_self()` | Toggle sidebar hidden |
| `show_self()` | Toggle sidebar visible |
| `request_permission(&[...])` | Request ReadApplicationState + ChangeApplicationState on load |
| `subscribe(&[...])` | Subscribe to SessionUpdate, Key, PaneUpdate events |

### SessionInfo Fields (from `SessionUpdate` event)

| Field | Type | Use |
|-------|------|-----|
| `name` | `String` | Session name display |
| `tabs` | `Vec<TabInfo>` | Tab count per session |
| `panes` | `PaneManifest` | Pane info including active command |
| `is_current_session` | `bool` | Highlight active session |
| `connected_clients` | `usize` | Show if other clients are connected |

### Rendering Primitives

| Component | Use Case |
|-----------|----------|
| `Text::new()` with `.color_range()` and `.selected()` | Session name with status indicator, highlight selected row |
| `print_text_with_coordinates(text, x, y, w, h)` | Position elements in the sidebar |
| `NestedList` with `.indent()` and `.selected()` | Session list with optional tab sub-items |
| `Table` with `.add_row()` | Structured display if needed (likely overkill for sidebar) |

Color indices 0-3 map to theme colors automatically (Catppuccin Frappe will apply correctly).

## Setup Commands

```bash
# 1. Add WASM target (one-time)
rustup target add wasm32-wasip1

# 2. Build (debug, for development)
cargo build

# 3. Build (release, for distribution)
cargo build --release

# 4. Load into running Zellij session (debug)
zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm

# 5. Load into running Zellij session (release)
zellij action start-or-reload-plugin file:target/wasm32-wasip1/release/zellij-project-sidebar.wasm

# 6. Development with hot reload (use dev layout)
zellij -l zellij.kdl
```

## Sources

- [Zellij Rust Plugin Tutorial](https://zellij.dev/tutorials/developing-a-rust-plugin/) - Official tutorial, verified 2026-03-11
- [zellij-tile 0.43.1 docs.rs](https://docs.rs/zellij-tile/latest/zellij_tile/) - API reference
- [zellij-tile shim (commands)](https://docs.rs/zellij-tile/latest/zellij_tile/shim/index.html) - All plugin commands
- [Plugin API Events](https://zellij.dev/documentation/plugin-api-events.html) - Event types
- [Plugin API Commands](https://zellij.dev/documentation/plugin-api-commands.html) - Command reference
- [Plugin API Permissions](https://zellij.dev/documentation/plugin-api-permissions) - Permission types
- [Plugin UI Rendering](https://zellij.dev/documentation/plugin-ui-rendering.html) - Text, Table, NestedList, Ribbon
- [Plugin Loading](https://zellij.dev/documentation/plugin-loading) - URL schemas, layout loading
- [Creating Layouts](https://zellij.dev/documentation/creating-a-layout.html) - KDL layout syntax, pane sizing
- [Plugin Dev Environment](https://zellij.dev/documentation/plugin-dev-env.html) - Hot reload setup
- [rust-plugin-example](https://github.com/zellij-org/rust-plugin-example) - Official example, Cargo.toml reference
- [zjstatus Cargo.toml](https://github.com/dj95/zjstatus) - Community plugin using zellij-tile 0.43.1, edition 2024
- [zellij-sessionizer](https://github.com/cunialino/zellij-sessionizer) - Session-switching plugin, minimal deps
- [Rust WASI target rename](https://blog.rust-lang.org/2024/04/09/updates-to-rusts-wasi-targets/) - wasm32-wasi to wasm32-wasip1
- [SessionInfo struct](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/struct.SessionInfo.html) - Session data fields

# Feature Landscape

**Domain:** Zellij session management plugin (sidebar-style project switcher)
**Researched:** 2026-03-11

## Ecosystem Context

The Zellij session management plugin space is fragmented across 8+ community plugins and one built-in session manager. Every single existing solution uses **floating popups** -- there is no persistent sidebar session manager in the ecosystem. This is the core gap the project fills.

Existing plugins fall into three categories:
1. **Directory scanners** (zellij-sessionizer, zsm) -- find folders, create sessions from them
2. **Session navigators** (zellij-choose-tree, built-in session-manager, zellij-switch) -- list/switch active sessions
3. **Favourites managers** (zellij-favs) -- pin sessions, batch manage

None combine **pinned project awareness** + **live session status** + **persistent visibility**. The closest analogue outside Zellij is Speedmux (a Go/libghostty multiplexer with a persistent sidebar tracking session state), but that is an entirely separate terminal multiplexer, not a plugin.

---

## Table Stakes

Features users expect from any session switching tool. Missing these and users just stick with the built-in session manager or zellij-sessionizer.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Session list display | Every session tool shows active sessions. Bare minimum. | Low | Subscribe to `SessionUpdate` event |
| Switch to session on selection | The entire point -- Enter to jump. Built-in does this. | Low | `switch_session()` API command |
| Visual indicator for current session | Users need to know where they are. Every tool does this. | Low | Compare session name from `ModeUpdate` |
| Keyboard navigation (j/k, arrows) | Zellij is keyboard-first. Every plugin uses vim-style nav. | Low | Standard event handling |
| Session status indicators | Running vs exited vs not started. Minimum useful info. | Low | `SessionUpdate` provides `is_active` state |
| Create session from folder | If a pinned project has no session, Enter should create one. | Medium | `switch_session()` with cwd creates if missing |
| Kill/delete session | zellij-choose-tree has `x` to delete, built-in has it, users expect it. | Low | `kill_sessions()` API command |
| Configurable project list | Explicit pinned folders via KDL config. This IS the value prop. | Medium | Parse plugin config block in KDL |

## Differentiators

Features that set this plugin apart. Not expected because nothing in the ecosystem does them, but they define the product.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Persistent docked sidebar** | THE differentiator. No existing plugin is persistent -- all are floating popups that require invocation and dismiss after selection. Strider proves the docked sidebar pattern works in Zellij (it uses layout-based sizing at 15-20% width). This plugin brings that same pattern to session management. | Medium | Layout integration, not floating. Must handle resize events. |
| **Toggle visibility (keybind)** | Floating popups are all-or-nothing. A toggle lets users show the sidebar when they want ambient awareness and hide it when screen real estate matters. No session plugin offers this. | Medium | `hide_self()` / `show_self()` or pane manipulation. Need to verify API support. |
| **Live tab count per session** | "help-self [3]" -- at-a-glance complexity indicator. No existing plugin shows tab counts alongside session names. Built-in session-manager shows tabs only when you expand into a session. | Low | `SessionUpdate` event includes tab info |
| **Active pane command display** | Shows what is running in the focused pane of each session (e.g., "nvim", "npm run dev"). No session plugin surfaces this. The `PaneUpdate` event provides command info. | Medium | Requires cross-session pane data. May only be available for current session -- needs API verification. |
| **Configurable info verbosity** | Minimal (name + dot), standard (name + tab count), full (tabs + active command). Different workflows need different density. No plugin offers this. | Low | Config flag controlling render detail level |
| **Pinned project ordering** | Projects appear in config-defined order, not alphabetical or creation-time. User's mental model is preserved. Sessionizer sorts by search results; choose-tree sorts arbitrarily. | Low | Iterate config entries in order |
| **Catppuccin Frappe theming** | Visually native to the user's Zellij setup. Most community plugins use default terminal colours with no theme awareness. | Low | Hardcoded colour values matching Frappe palette |

## Anti-Features

Features to explicitly NOT build. These are tempting but wrong for this plugin's identity.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Fuzzy search / directory scanning | zellij-sessionizer already does this well. Duplicating it dilutes the plugin's identity. This plugin is for **known** projects, not discovery. | Pinned explicit folder paths in config. User adds projects manually. |
| Session tree (expand tabs/panes) | zellij-choose-tree already provides hierarchical session/tab/pane navigation. Reimplementing it adds complexity with no differentiation. | Show tab **count** per session, not tab tree. Link to choose-tree for deep navigation. |
| Session resurrection UI | Built-in session-manager handles resurrection. Adding it creates feature overlap and maintenance burden. | Show exited session status; let built-in handle resurrection flow. |
| Mouse interaction (v1) | Zellij is keyboard-first. Mouse adds input handling complexity for minimal gain. Most Zellij users are keyboard navigators. | Keyboard-only in v1. Mouse is a v2 consideration. |
| Per-project layout application | Applying custom layouts when creating sessions is useful but complex (layout file resolution, error handling, layout compatibility). zsm and zellij-workspace already tackle this. | Accept optional `layout` per project in config but defer implementation to v2. |
| Session rename from sidebar | Low frequency action. CLI `zellij action rename-session` exists. Adding inline rename adds text input handling complexity disproportionate to value. | Omit entirely. Users rename via CLI or built-in session-manager. |
| Multi-theme support | Only one user (you) for now. Catppuccin Frappe is the theme. Making it configurable adds config surface area for zero current users. | Hardcode Frappe. Extract colours to constants for future theming. |
| Fuzzy filter over pinned list | With 5-10 pinned projects, j/k navigation is faster than typing a filter query. Fuzzy search adds text input mode complexity. | Direct j/k navigation. If the list grows past ~15 items, reconsider. |

---

## Feature Dependencies

```
Configurable project list ──> Session list display (need projects to show sessions for)
                          ──> Create session from folder (need folder path from config)
                          ──> Pinned project ordering (order comes from config)

Session list display ──> Visual indicator for current session
                    ──> Session status indicators
                    ──> Live tab count per session (enhancement of list)
                    ──> Active pane command display (enhancement of list)

Keyboard navigation ──> Switch to session on selection (Enter action)
                   ──> Kill/delete session (x action)

Configurable info verbosity ──> Live tab count per session (shown at standard+)
                           ──> Active pane command display (shown at full only)

Persistent docked sidebar ──> Toggle visibility (toggle requires sidebar to exist)
```

## MVP Recommendation

Prioritize in this order:

1. **Configurable project list** -- without this, nothing works. Parse KDL config for pinned folders.
2. **Session list display with status** -- the core render. Show each pinned project with running/exited/none status.
3. **Keyboard navigation + switch** -- j/k to navigate, Enter to switch/create. The core interaction.
4. **Visual indicator for current session** -- highlight which session you are in.
5. **Kill session** -- x to kill. Low effort, high utility.
6. **Persistent docked sidebar** -- the differentiating UX. Render as a tiled pane, not floating.
7. **Toggle visibility** -- keybind to show/hide the sidebar.
8. **Live tab count** -- "[3]" next to session name. Low effort polish.

**Defer to v2:**
- Active pane command display: May require cross-session pane data that is not available via plugin API. Needs feasibility verification. If only current-session pane data is available, this feature is significantly less useful.
- Per-project layout application: Useful but adds config complexity and error paths.
- Configurable info verbosity: Natural evolution once tab count and command display exist. Ship with a sensible default first.

## Competitive Landscape Summary

| Plugin | UX Model | Session Create | Session Switch | Session Kill | Persistence | Pinned Projects | Tab/Pane Info |
|--------|----------|----------------|----------------|--------------|-------------|-----------------|---------------|
| Built-in session-manager | Floating popup | Yes | Yes | Yes | No (on-demand) | No | Expandable tree |
| zellij-sessionizer | Floating fuzzy finder | Yes (from dir) | Yes | No | No | No (scans dirs) | No |
| zellij-choose-tree | Floating tree view | No | Yes | Yes | No | No | Expandable tree |
| zellij-switch | CLI pipe (no UI) | Yes | Yes | No | N/A | No | No |
| zellij-favs | Floating list | No | Yes | Yes (batch) | Disk cache | Yes (favourites) | Optional |
| zsm | Floating fuzzy finder | Yes (from zoxide) | Yes | No | No | No (zoxide ranked) | No |
| **This plugin** | **Docked sidebar** | **Yes** | **Yes** | **Yes** | **Yes (always visible)** | **Yes (config)** | **Tab count** |

The unique combination is: **persistent + pinned + live status**. No existing plugin occupies this niche.

## Sources

- [zellij-choose-tree](https://github.com/laperlej/zellij-choose-tree) - tmux choose-tree inspired session tree
- [zellij-sessionizer (cunialino)](https://github.com/cunialino/zellij-sessionizer) - Fuzzy directory session creator
- [zellij-sessionizer (silicakes)](https://github.com/silicakes/zellij-sessionizer) - FZF-based session launcher
- [zellij-switch](https://github.com/mostafaqanbaryan/zellij-switch) - CLI pipe session switcher
- [zellij-favs](https://github.com/JoseMM2002/zellij-favs) - Session favourites manager
- [zsm](https://github.com/liam-mackie/zsm) - Zoxide session manager
- [zbuffers](https://github.com/Strech/zbuffers) - Tab switcher with search
- [harpoon](https://github.com/Nacho114/harpoon) - Pane bookmarking (nvim harpoon clone)
- [room](https://github.com/rvcas/room) - Tab fuzzy finder
- [Zellij built-in session-manager](https://zellij.dev/documentation/session-manager-alias.html)
- [Zellij plugin API events](https://zellij.dev/documentation/plugin-api-events.html)
- [Zellij plugin API commands](https://zellij.dev/documentation/plugin-api-commands.html)
- [Zellij 0.38.0 release](https://zellij.dev/news/session-manager-protobuffs/)
- [awesome-zellij](https://github.com/zellij-org/awesome-zellij)
- [tmux-sessionx](https://github.com/omerxx/tmux-sessionx) - Feature-rich tmux session manager
- [sesh](https://github.com/joshmedeski/sesh) - Multiplexer-agnostic session CLI
- [Speedmux](https://github.com/webforspeed/speedmux) - Go multiplexer with persistent sidebar (prior art for sidebar concept)
- [Zellij session management tutorial](https://zellij.dev/tutorials/session-management/)
- [Zellij built-in plugins (DeepWiki)](https://deepwiki.com/zellij-org/zellij/4.3-built-in-plugins)

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