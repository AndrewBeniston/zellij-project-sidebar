---
phase: 07-pipe-protocol----pills-+-progress
plan: 02
subsystem: ui
tags: [rust, wasm, zellij, rendering, card-styling, pills, progress-bar, ai-state, hook-script]

# Dependency graph
requires:
  - phase: 07-pipe-protocol----pills-+-progress
    plan: 01
    provides: AgentState, AgentStatus, pills/progress_pct fields on ProjectMetadata, 5 pipe handlers

provides:
  - render_progress_bar() helper (━ filled, ░ empty, percentage display)
  - Left border │ on name and detail lines (card styling)
  - AI state dot coloring on name line (green=active, yellow=waiting, gray=idle, red=attention)
  - Border color matching AI state (consistent visual signal)
  - Pills rendered as key:value pairs on detail line (max 3)
  - Progress bar rendered on detail line with percentage
  - Separator lines rendered as top rules (┌──────) between cards
  - scripts/sidebar-status.sh hook script template for Claude Code integration
affects:
  - 08 (card layout foundation established; any further styling builds on this)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Left border │ at char index 1, dot at char index 3 in name line format ' │ ● name'"
    - "AI state color shared between border and dot for visual consistency"
    - "Detail line prefix ' │ ' (3 chars) with branch_start tracking for color_range accuracy"
    - "render_progress_bar() as a free function, not a method"

key-files:
  created:
    - scripts/sidebar-status.sh
  modified:
    - src/main.rs

key-decisions:
  - "Left border │ and dot use the same color (border_dot_color) for consistent AI state signaling"
  - "Separator renders as top rule ┌─── for the NEXT card (between-card position); first card has no top rule — avoids click mapping changes"
  - "Progress bar only renders if remaining space allows (char count check) to avoid overflow"
  - "Hook script uses async: true pattern with & backgrounding to never block Claude Code"

patterns-established:
  - "Card line format: ' │ ● name' — use char index 1 for border, char index 3 for dot"
  - "Detail line format: ' │ branch pills progress' — branch_start/end tracked before appending pills"

requirements-completed: [PILL-02, PROG-02]

# Metrics
duration: 6min
completed: 2026-03-14
---

# Phase 7 Plan 02: Rendering Pipeline — Pills, Progress, AI Dot, and Card Styling Summary

**Pill key:value pairs, character-cell progress bars, AI-state-colored left border/dot, card top-rule separators, and Claude Code hook script template — full rendering layer for Phase 7 pipe protocol**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-14T17:00:58Z
- **Completed:** 2026-03-14T17:06:58Z
- **Tasks:** 2 (auto) + 1 (checkpoint:human-verify, pending)
- **Files modified:** 2

## Accomplishments
- Added `render_progress_bar()` helper using ━ (filled) and ░ (empty) Unicode characters
- Updated `render_project_name_line()` with left border `│`, AI-state dot coloring (active/waiting/idle/unknown), and correct char index offsets for the new format
- Updated `render_detail_line()` with left border `│`, git branch (blue), pills (space-separated key:value, max 3), and progress bar (conditional on remaining space)
- Updated `Separator` rendering to emit `┌────` top rules between cards (gray, fills to cols width)
- Created `scripts/sidebar-status.sh` hook script template handling PostToolUse, Stop, Notification, and SessionStart events via `zellij pipe --name "sidebar::ai"`

## Task Commits

Each task was committed atomically:

1. **Task 1: Update rendering pipeline for AI dot, pills, progress, and card styling** - `0772994` (feat)
2. **Task 2: Create sidebar-status.sh hook script template** - `2352c1e` (feat)

**Plan metadata:** (docs commit follows after checkpoint verification)

## Files Created/Modified
- `src/main.rs` - Updated render_project_name_line(), render_detail_line(), Separator arm; added render_progress_bar() helper
- `scripts/sidebar-status.sh` - Claude Code hook script for AI state pipe messages

## Decisions Made
- Left border `│` and status dot share the same `border_dot_color` so agent state shows as a unified visual signal
- Separator position (between-card) used for top rule; first card deliberately has no top rule to avoid click mapping changes
- Progress bar guarded by remaining space check to prevent overflow on narrow panes
- Hook script uses `&` backgrounding in addition to `async: true` to ensure zero blocking

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - both tasks compiled cleanly on the first attempt. The char index arithmetic for the new ` │ ● name` format (border at index 1, dot at index 3) was verified by counting chars explicitly.

## User Setup Required

The hook script at `scripts/sidebar-status.sh` must be manually installed by the user:
1. Copy `scripts/sidebar-status.sh` to `~/.claude/hooks/sidebar-status.sh`
2. Add hook entries to `~/.claude/settings.json` for PostToolUse, Stop, Notification, and SessionStart events

See the script header comments for the exact `settings.json` configuration format.

## Next Phase Readiness
- Full Phase 7 pipe protocol complete pending live verification (Task 3 checkpoint)
- Phase 8 (port detection) can build on this card layout foundation
- Hook script template is ready for user installation and testing

---
*Phase: 07-pipe-protocol----pills-+-progress*
*Completed: 2026-03-14*

## Self-Check: PASSED

- scripts/sidebar-status.sh: FOUND
- 07-02-SUMMARY.md: FOUND
- Commit 0772994 (Task 1): FOUND
- Commit 2352c1e (Task 2): FOUND
- render_progress_bar(): FOUND in src/main.rs
- Left border │ character: FOUND in src/main.rs (at lines 628, 631, 727)
- sidebar::ai in hook script: FOUND
