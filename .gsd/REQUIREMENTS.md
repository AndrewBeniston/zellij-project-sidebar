# Requirements

## Active

### DISP-04 — Running sessions show tab count (e.g. `help-self [3]`)

- Status: active
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Running sessions show tab count (e.g. `help-self [3]`)

### DISP-05 — Active pane command displayed for current session (e.g. `claude` or `vim`)

- Status: active
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Active pane command displayed for current session (e.g. `claude` or `vim`)

### DISP-06 — Info verbosity configurable — minimal (name + status dot) through full (tabs + command)

- Status: active
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Info verbosity configurable — minimal (name + status dot) through full (tabs + command)

### INTR-05 — User can toggle sidebar visibility with a keybind (Cmd+P via pipe mechanism)

- Status: active
- Class: core-capability
- Source: inferred
- Primary Slice: S02

User can toggle sidebar visibility with a keybind (Cmd+P via pipe mechanism). Current code has pipe focus via Alt+s — needs full show/hide toggle cycle via Cmd+P (Super p).

### LAYT-03 — Toggle hides/shows sidebar and reclaims/restores space

- Status: active
- Class: core-capability
- Source: inferred
- Primary Slice: S02

Toggle hides/shows sidebar and reclaims/restores space. Current code only shows (show_self) — needs hide_self() and visibility state tracking.

### THEM-01 — Colours match Catppuccin Frappe via Zellij's color_range API

- Status: active
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Colours match Catppuccin Frappe via Zellij's color_range API

### THEM-02 — Status indicators use semantic colours (green = running, dim = stopped, yellow = exited)

- Status: active
- Class: core-capability
- Source: inferred
- Primary Slice: S03

Status indicators use semantic colours (green = running, dim = stopped, yellow = exited)

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

Each project shows live session status (running / exited / not started). Validated: SessionUpdate handler matches sessions to projects, render shows status chars.

### DISP-03 — Current active session is visually highlighted

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Current active session is visually highlighted. Validated: `>` indicator for is_current session, selected() for cursor position.

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

### INFR-04 — Sidebar is unselectable by default — becomes selectable only during active interaction

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Sidebar is unselectable by default — becomes selectable only during active interaction. Validated: set_selectable(false) after permissions, set_selectable(true) on pipe focus, set_selectable(false) on Esc/action.

### INFR-05 — Pipe-based toggle mechanism works from any context (unfocused)

- Status: validated
- Class: core-capability
- Source: inferred
- Primary Slice: S01

Pipe-based toggle mechanism works from any context (unfocused). Validated: pipe() handler for "focus_sidebar" message. Note: needs upgrade from Alt+s to Cmd+P and full hide/show cycle in S02.

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

## Deferred

## Out of Scope

---
