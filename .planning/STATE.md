---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: Rich Cards
status: completed
stopped_at: "Checkpoint: 07.1-01 Task 2 human-verify (cross-session AI visibility live verification)"
last_updated: "2026-03-14T22:38:38.193Z"
last_activity: 2026-03-14 — Completed Phase 7 Plan 01 (AgentState/AgentStatus, pills/progress fields, 5 pipe handlers)
progress:
  total_phases: 9
  completed_phases: 5
  total_plans: 9
  completed_plans: 6
  percent: 50
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-14)

**Core value:** One-keypress project switching with always-visible session awareness
**Current focus:** Phase 7 — Pipe Protocol / Pills + Progress

## Current Position

Phase: 7 of 8 (Pipe Protocol — Pills + Progress) — In Progress
Plan: 1 of 2 (Complete)
Status: Phase 7 Plan 01 complete — data model and pipe ingestion layer done
Last activity: 2026-03-14 — Completed Phase 7 Plan 01 (AgentState/AgentStatus, pills/progress fields, 5 pipe handlers)

Progress: [█████░░░░░] 50%

## Performance Metrics

**Velocity:**
- Total plans completed: 4 (v1.1)
- Average duration: 3.3min
- Total execution time: 10min

| Phase | Plan | Duration | Tasks | Files |
|-------|------|----------|-------|-------|
| 05    | 01   | 3min     | 2     | 1     |
| 06    | 01   | 2min     | 3     | 1     |
| 07    | 01   | 5min     | 2     | 1     |
| Phase 07 P02 | 3min | 2 tasks | 2 files |
| Phase 07.1-01 P01 | 8min | 1 tasks | 3 files |

## Accumulated Context

### Decisions

- [v1.0]: Session-based over pin-based default view
- [v1.0]: Browse mode replaces manual curation
- [v1.0]: Pipe-based attention system
- [v1.0]: Discovery mode (scan_dir) as primary
- [v1.1]: Multi-line card layout (CMUX-inspired)
- [v1.1]: run_command for git/port detection (zero new crate deps)
- [v1.1]: Status pills + progress bar via pipe messages
- [v1.1]: Data pipeline before rendering refactor (prove polling before investing in UI)
- [v1.1]: Multi-line card refactor is atomic (mouse, scroll, selection all break together)
- [P5]: RunCommands permission and Timer/RunCommandResult events now unconditional
- [P5]: Backpressure via pending_commands counter -- timer skips if previous cycle pending
- [P5]: is_git_repo field prevents re-polling non-git directories every cycle
- [P6]: Git branch moved from name line to dedicated detail line (render_detail_line)
- [P6]: NotStarted projects remain single-line cards in browse mode
- [P6]: project_index() pattern for variant-agnostic click/scroll handling
- [Phase 07]: AgentState::Unknown as default — avoids false active/idle signals before first pipe message
- [Phase 07]: Pipe protocol uses args dict (session, state, tool, key, value, pct) for all sidebar:: handlers
- [Phase 07]: Left border │ and dot share border_dot_color for consistent AI state visual signal
- [Phase 07]: Separator renders as top rule ┌─── (between-card); first card has no top rule to avoid click mapping changes
- [Phase 07.1]: serde_json + /cache as shared AI state bus across Zellij sessions
- [Phase 07.1]: Timer handler is read-only from /cache; pipe handlers are write path only
- [Phase 07.1]: extract_active_command called unconditionally for all sessions — passive cross-session detection

### Pending Todos

None yet.

### Roadmap Evolution

- Phase 07.1 inserted after Phase 7: Cross-Session AI Visibility (URGENT)

### Blockers/Concerns

- ~~Pipe protocol format: `::` name encoding vs `args` dict~~ (RESOLVED in Phase 7 Plan 01: args dict used for sidebar::ai/pill/progress)
- ~~Cross-session AI state visibility~~ (SOLUTION FOUND: SessionUpdate has terminal_command for all sessions, /cache for persistence, -s flag for cross-session pipes)
- Port attribution: PID-to-session mapping impossible from WASM sandbox -- pipe-based is reliable, lsof is stretch goal
- ~~Fixed vs variable card height: decide during Phase 6 planning~~ (RESOLVED: 2-line cards for Running/Exited, 1-line for NotStarted)
- ~~RunCommands permission: must be requested unconditionally~~ (RESOLVED in Phase 5 Plan 01)

## Session Continuity

Last session: 2026-03-14T22:38:38.191Z
Stopped at: Checkpoint: 07.1-01 Task 2 human-verify (cross-session AI visibility live verification)
Resume file: None
