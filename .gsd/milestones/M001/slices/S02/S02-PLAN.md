# S02: Display Interaction

**Goal:** Parse pinned project folders from KDL configuration, match them against live session data, and render a styled project list with status indicators.
**Demo:** Parse pinned project folders from KDL configuration, match them against live session data, and render a styled project list with status indicators.

## Must-Haves


## Tasks

- [ ] **T01: 02-display-interaction 01**
  - Parse pinned project folders from KDL configuration, match them against live session data, and render a styled project list with status indicators.

Purpose: Establishes the data model (Project, SessionStatus, State) and rendering pipeline that all interaction logic in Plan 02 depends on. Without this, there is nothing to navigate or act upon.

Output: A plugin that displays a list of configured projects with live session status indicators and current-session highlighting.
- [ ] **T02: 02-display-interaction 02**
  - Add keyboard navigation (j/k), session actions (Enter to switch/create, x to kill), and focus management (unselectable by default, Alt+s to activate, Esc to deactivate).

Purpose: Completes the interaction layer that makes the sidebar usable. Without this, users can see the project list but cannot interact with it.

Output: A fully interactive project sidebar with keyboard navigation, session management, and proper focus control.
- [ ] **T03: 02-display-interaction 03**
  - Verify all Phase 2 functionality works end-to-end in a running Zellij instance with real sessions.

Purpose: WASM plugins have no unit test harness. The only way to verify correctness is loading the plugin into Zellij and manually testing each interaction. This checkpoint confirms that config parsing, session matching, rendering, keyboard navigation, session actions, and focus management all work together.

Output: Human verification that all 8 requirements are met.

## Files Likely Touched

- `src/main.rs`
- `zellij.kdl`
- `src/main.rs`
