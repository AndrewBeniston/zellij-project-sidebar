# Requirements

## Active

## Validated

### INFR-01 — Plugin compiles to wasm32-wasip1 and loads in Zellij 0.43.1

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Plugin compiles to wasm32-wasip1 and loads in Zellij 0.43.1

### INFR-02 — Plugin requests and handles permissions correctly (first-launch UX)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Plugin requests and handles permissions correctly (first-launch UX)

### INFR-03 — Plugin subscribes to SessionUpdate events for live data (no polling)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Plugin subscribes to SessionUpdate events for live data (no polling)

### DISP-01 — Plugin renders a list of pinned project folders from KDL config

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Plugin renders a list of pinned project folders from KDL config. Validated: load() parses project_N entries from KDL configuration.

### DISP-02 — Each project shows live session status (running / exited / not started)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Each project shows live session status (running / exited / not started). Validated: SessionUpdate handler matches sessions to projects, render shows status dots with semantic colors.

### DISP-03 — Current active session is visually highlighted

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Current active session is visually highlighted. Validated: selected() for cursor position, green dot for running sessions.

### DISP-04 — Running sessions show tab count (e.g. `help-self [3]`)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Running sessions show tab count (e.g. `help-self [3]`). Validated: SessionUpdate extracts session.tabs.len(), render appends `[N]` in full verbosity mode.

### DISP-05 — Active pane command displayed for current session (e.g. `claude` or `vim`)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Active pane command displayed for current session (e.g. `claude` or `vim`). Validated: SessionUpdate finds focused non-plugin pane in active tab, extracts terminal_command basename, render appends command in full verbosity mode.

### DISP-06 — Info verbosity configurable — minimal (name + status dot) through full (tabs + command)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Info verbosity configurable — minimal (name + status dot) through full (tabs + command). Validated: `verbosity` KDL config key parsed in load(), controls render output between Minimal and Full modes.

### INTR-01 — User can navigate project list with j/k keys

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

User can navigate project list with j/k keys. Validated: Key event handler for j/k with bounds checking.

### INTR-02 — User can switch to a running session by pressing Enter

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

User can switch to a running session by pressing Enter. Validated: activate_selected_project() calls switch_session().

### INTR-03 — If no session exists for a folder, Enter creates one with cwd set to that folder

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

If no session exists for a folder, Enter creates one with cwd set to that folder. Validated: activate_selected_project() calls switch_session_with_cwd() for NotStarted.

### INTR-04 — User can kill a session by pressing x on a running project

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

User can kill a session by pressing x on a running project. Validated: kill_selected_session() with current-session guard.

### INTR-05 — User can toggle sidebar visibility with a keybind (Cmd+P via pipe mechanism)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S03

User can toggle sidebar visibility with a keybind (Cmd+P via pipe mechanism). Validated: setup_toggle_keybind() registers `Super p` via reconfigure, pipe handler dispatches "toggle_sidebar" message to toggle_visibility().

### INFR-04 — Sidebar is unselectable by default — becomes selectable only during active interaction

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Sidebar is unselectable by default — becomes selectable only during active interaction. Validated: set_selectable(false) after permissions, set_selectable(true) on toggle show, set_selectable(false) on Esc/action/hide.

### INFR-05 — Pipe-based toggle mechanism works from any context (unfocused)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Pipe-based toggle mechanism works from any context (unfocused). Validated: pipe() handler for "toggle_sidebar" and legacy "focus_sidebar" messages. Upgraded from Alt+s to Super p (Cmd+P) in S03.

### LAYT-01 — Plugin renders as a docked side panel (tiled pane, not floating)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Plugin renders as a docked side panel (tiled pane, not floating). Validated: show_self(false) uses tiled mode, dev layout positions as left pane.

### LAYT-02 — Sidebar has fixed width (configurable, default ~20 chars)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Sidebar has fixed width (configurable, default ~20 chars). Validated: Dev layout sets size=25 on plugin pane.

### LAYT-03 — Toggle hides/shows sidebar and reclaims/restores space

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Toggle hides/shows sidebar and reclaims/restores space. Validated: toggle_visibility() calls hide_self() to hide (reclaims space) and show_self(false) to show (restores tiled pane). is_hidden state tracks visibility.

### THEM-01 — Colours match Catppuccin Frappe via Zellij's color_range API

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Colours match Catppuccin Frappe via Zellij's color_range API. Validated: Semantic color constants map to Zellij palette indices (green=0, yellow=3, blue=4, gray=7) which resolve to Catppuccin Frappe colors when the user's theme is configured.

### THEM-02 — Status indicators use semantic colours (green = running, dim = stopped, yellow = exited)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Status indicators use semantic colours (green = running, dim = stopped, yellow = exited). Validated: Status dots use COLOR_GREEN for running, COLOR_YELLOW for exited, COLOR_GRAY for not-started. Header uses COLOR_BLUE, metadata uses COLOR_GRAY.

## Deferred

## Out of Scope

---
