# T01: 02-display-interaction 01

**Slice:** S02 — **Milestone:** M001

## Description

Parse pinned project folders from KDL configuration, match them against live session data, and render a styled project list with status indicators.

Purpose: Establishes the data model (Project, SessionStatus, State) and rendering pipeline that all interaction logic in Plan 02 depends on. Without this, there is nothing to navigate or act upon.

Output: A plugin that displays a list of configured projects with live session status indicators and current-session highlighting.

## Must-Haves

- [ ] "Plugin displays a list of project names parsed from KDL configuration"
- [ ] "Each project shows its live session status (running/exited/not started)"
- [ ] "The current active session is visually distinct from other projects"
- [ ] "Status updates in real-time when sessions are created, killed, or switched"

## Files

- `src/main.rs`
- `zellij.kdl`
