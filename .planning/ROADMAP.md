# Roadmap: Zellij Project Sidebar

## Overview

Four phases take this from zero to a fully themed, docked project sidebar. Phase 1 establishes a compiling WASM plugin with correct permissions and event subscriptions -- the foundation everything else builds on. Phase 2 is the bulk of the work: parsing config, matching sessions, rendering the list, and wiring all keyboard interaction. Phase 3 turns the plugin into a proper sidebar (docked layout, fixed width, pipe-based toggle from any context). Phase 4 adds the information density features (tab count, active command, verbosity modes) and applies Catppuccin Frappe theming.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Scaffold + Lifecycle** - Compiling WASM plugin that loads in Zellij, requests permissions, and subscribes to session events
- [ ] **Phase 2: Display + Interaction** - Pinned project list with live session status, keyboard navigation, and session switch/create/kill
- [ ] **Phase 3: Sidebar Layout + Toggle** - Docked side panel with fixed width and Cmd+P pipe-based visibility toggle
- [ ] **Phase 4: Enrichment + Theme** - Tab count, active command, info verbosity modes, and Catppuccin Frappe colours

## Phase Details

### Phase 1: Scaffold + Lifecycle
**Goal**: A WASM plugin that compiles, loads in Zellij, requests the correct permissions, and receives live session data
**Depends on**: Nothing (first phase)
**Requirements**: INFR-01, INFR-02, INFR-03
**Success Criteria** (what must be TRUE):
  1. Running `cargo build --target wasm32-wasip1` produces a .wasm file that loads in Zellij 0.43.1 without errors
  2. On first load, plugin presents a permission prompt and proceeds after user grants permissions
  3. Plugin receives SessionUpdate events and logs session data to stderr (visible in Zellij logs)
**Plans**: TBD

Plans:
- [ ] 01-01: TBD

### Phase 2: Display + Interaction
**Goal**: Users see their pinned projects with live session status and can navigate, switch, create, and kill sessions entirely from the sidebar
**Depends on**: Phase 1
**Requirements**: DISP-01, DISP-02, DISP-03, INTR-01, INTR-02, INTR-03, INTR-04, INFR-04
**Success Criteria** (what must be TRUE):
  1. Plugin displays a list of project names parsed from KDL config, each showing whether its session is running, exited, or not started
  2. User can move selection up/down with j/k and the currently selected item is visually distinct
  3. Pressing Enter on a project with a running session switches to that session; pressing Enter on a project with no session creates one with cwd set to that folder
  4. Pressing x on a running session kills it and the list updates to reflect the change
  5. Sidebar pane does not steal focus during normal terminal work -- it becomes selectable only when the user actively interacts with it
**Plans**: TBD

Plans:
- [ ] 02-01: TBD
- [ ] 02-02: TBD

### Phase 3: Sidebar Layout + Toggle
**Goal**: The plugin operates as a docked side panel that users can show/hide from any context without needing to focus it first
**Depends on**: Phase 2
**Requirements**: LAYT-01, LAYT-02, LAYT-03, INTR-05, INFR-05
**Success Criteria** (what must be TRUE):
  1. Plugin renders as a tiled pane on the left side of the terminal with a fixed width (configurable, default ~20 chars)
  2. Pressing Cmd+P from any pane (including when sidebar is not focused) toggles sidebar visibility
  3. When sidebar is hidden, its space is reclaimed by adjacent panes; when shown, space is restored
**Plans**: TBD

Plans:
- [ ] 03-01: TBD

### Phase 4: Enrichment + Theme
**Goal**: The sidebar shows rich session metadata at user-chosen verbosity, styled to match the Catppuccin Frappe theme
**Depends on**: Phase 3
**Requirements**: DISP-04, DISP-05, DISP-06, THEM-01, THEM-02
**Success Criteria** (what must be TRUE):
  1. Running sessions display their tab count next to the project name (e.g. `help-self [3]`)
  2. The current session shows the active pane command (e.g. `claude` or `vim`)
  3. User can configure verbosity in KDL -- minimal mode shows only name + status dot, full mode shows tabs + command
  4. All colours (background, text, status indicators, selection highlight) match Catppuccin Frappe, with semantic status colours (green = running, dim = stopped, yellow = exited)
**Plans**: TBD

Plans:
- [ ] 04-01: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Scaffold + Lifecycle | 0/? | Not started | - |
| 2. Display + Interaction | 0/? | Not started | - |
| 3. Sidebar Layout + Toggle | 0/? | Not started | - |
| 4. Enrichment + Theme | 0/? | Not started | - |
