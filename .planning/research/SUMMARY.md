# Project Research Summary

**Project:** Zellij Project Sidebar
**Domain:** v1.1 Rich Cards -- multi-line card layout with per-project metadata (git branch, ports, pills, progress bars)
**Researched:** 2026-03-14
**Confidence:** HIGH

## Executive Summary

The v1.1 "Rich Cards" milestone transforms the existing single-line project sidebar into a CMUX-inspired multi-line card layout displaying ambient metadata: git branch, listening ports, status pills, and progress bars. The research confirms this is achievable with **zero new crate dependencies**. All features are built on three existing primitives: `set_timeout` + `Timer` for periodic polling, `run_command_with_env_variables_and_cwd` for shelling out to git/lsof per project directory, and the existing pipe message system for external metadata injection (pills, progress, ports).

The recommended approach is to treat multi-line card rendering as the foundational refactor, then layer metadata sources on top. The architecture extends naturally: `RenderLine` gains sub-line variants (`ProjectDetail`, `ProjectPills`), `Project` gains a `ProjectMetadata` struct, and `State` gains `cached_metadata` alongside the existing `cached_statuses` pattern. The codebase grows from ~600 to ~900-1100 lines but stays single-file -- module splitting is unnecessary until ~1500 lines.

The primary risks are in the rendering refactor, not the data pipeline. Multi-line cards break three assumptions baked into the current code: mouse click y-mapping (1 row = 1 project), scroll offset arithmetic (flat index into render lines), and layout stability (card heights change as metadata arrives asynchronously). These must be solved together in the first rendering step. Port-to-session attribution is the only genuinely hard unsolved problem -- the recommendation is to defer auto-detection and use pipe-based port reporting, consistent with the existing attention system pattern.

## Key Findings

### Recommended Stack

No changes to `Cargo.toml`. The entire v1.1 feature set ships on the existing `zellij-tile = "0.43.1"` + `serde` stack. Crates like `regex`, `git2`, `chrono`, `serde_json`, and `tokio` were evaluated and explicitly rejected -- all parsing is achievable with stdlib `String` methods, git operations must shell out (libgit2 cannot compile to wasm32-wasip1), and the WASM sandbox is single-threaded.

**New API surface (already in zellij-tile 0.43.1):**
- `set_timeout(f64)` + `Event::Timer` -- one-shot timer for periodic polling loops (verified in shim.rs line 305)
- `run_command_with_env_variables_and_cwd` -- run git commands in each project's directory (verified in shim.rs line 337)
- `EventType::Timer` + `EventType::RunCommandResult` -- must be subscribed in all modes (not just discovery)

**Permission change:** `PermissionType::RunCommands` must be requested unconditionally (currently only in discovery mode). Users will see a one-time permission prompt on upgrade.

### Expected Features

**Must have (table stakes):**
- Multi-line card layout -- foundation for all other features; without it, metadata has nowhere to render
- Git branch display -- highest-value ambient metadata ("what branch am I on?" is universal)
- Session status + tab count on card -- already exist, must survive the card layout transition
- Active command on card -- already exists, must survive transition

**Should have (differentiators):**
- Status pills via pipe -- extensible key-value metadata (e.g., `env:prod`, `build:passing`) with zero plugin code changes per new pill type
- Progress bar via pipe -- inline 0-100% bar using Unicode block characters for builds/deploys
- Attention badge with message -- extend existing boolean attention flag to carry notification text

**Defer:**
- Auto port detection via lsof -- no reliable PID-to-session mapping from the WASM sandbox; pipe-based port reporting is more accurate
- PR status / GitHub integration -- requires OAuth, network calls, API rate limiting; out of scope for a sidebar plugin
- Per-pane working directory -- PaneInfo lacks CWD field; would need O(panes) shell-outs per refresh
- Custom Nerd Font icons -- assumes font installation; use universal Unicode symbols instead

### Architecture Approach

The architecture extends the existing event-driven, single-struct plugin pattern. `Project` gains a `ProjectMetadata` struct (git branch, ports, pills, progress). `State` gains `cached_metadata: BTreeMap<String, ProjectMetadata>` that survives `rebuild_projects()` calls, mirroring the existing `cached_statuses` pattern. `RenderLine` gains `ProjectDetail(usize)` and `ProjectPills(usize)` variants so each card emits 1-3 screen rows. A single `set_timeout` timer drives a tick-based polling loop where git branches poll every ~10 seconds and ports (if enabled) every ~30 seconds.

**Major components (changes only):**
1. `ProjectMetadata` struct -- holds per-project enrichment data (branch, ports, pills, progress) with `Default` for clean empty state
2. Timer + polling system -- single timer with tick counter, backpressure via `pending_commands` tracking, re-arm after all results arrive
3. Multi-line render engine -- `RenderLine` sub-variants, updated `build_render_lines()`, new `render_detail_line()` and `render_pills_line()` methods
4. Extended pipe protocol -- new message types for pills (`sidebar::pill::`), progress (`sidebar::progress::`), and ports (`sidebar::port::`)

### Critical Pitfalls

1. **Mouse click y-mapping breaks with multi-line cards** -- current code assumes 1 row = 1 project. Must build a screen-row-to-project-index map so clicking any line of a 3-line card selects the correct project. Solve in the same step as multi-line rendering.

2. **Scroll offset arithmetic breaks with variable-height cards** -- `scroll_offset` indexes into `render_lines` assuming 1:1 with screen rows. Must change to screen-row-based offset and recalculate `ensure_selection_visible` to account for card heights of 1-3 rows.

3. **run_command results arrive out of order** -- 15+ git commands fire concurrently with no ordering guarantee. Must use the context `BTreeMap` to tag every command with `cmd` type and `project` name. Never rely on arrival order.

4. **Polling without backpressure floods the event queue** -- 20 projects x 2 commands = 40 concurrent subprocesses per cycle. Must track `pending_commands`, gate new cycles on completion, and re-arm the timer only after all results arrive.

5. **Pipe protocol becomes unparseable without forethought** -- encoding structured data in `::` name segments breaks at 4+ segments. Use the `args` BTreeMap for structured data (session, type, key, value) and keep `name` as a simple action identifier. Design the protocol before implementation.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Data Model + Polling Infrastructure

**Rationale:** Every feature writes to and reads from `ProjectMetadata`. The polling pipeline (`set_timeout` -> `Timer` -> `run_command` -> `RunCommandResult` -> cached_metadata) must be proven before investing in rendering changes. Git branch is the simplest external data source and validates the entire pipeline end-to-end.

**Delivers:** `ProjectMetadata` struct, `cached_metadata` on State, Timer subscription, git branch polling for running sessions, context-based command routing in `RunCommandResult` handler. Data arrives and is stored correctly (verifiable via `eprintln!`).

**Addresses:** Git branch detection (table stakes), Timer + RunCommandResult infrastructure

**Avoids:** Pitfall 3 (out-of-order results -- use context dict), Pitfall 4 (backpressure -- track pending commands), Pitfall 10 (indistinguishable timers -- single timer with tick counter), Pitfall 14 (absolute paths), Pitfall 18 (must call `set_timeout` after permissions granted)

### Phase 2: Multi-Line Card Rendering

**Rationale:** The rendering refactor is the highest-risk change and touches the most code paths (mouse clicks, scroll, selection highlight). It depends on Phase 1's data model to have content to display. Solving it second means the data pipeline is already proven, so rendering bugs are isolated.

**Delivers:** `RenderLine::ProjectDetail` and `ProjectPills` variants, updated `build_render_lines()` with conditional sub-lines, `render_detail_line()` showing git branch, updated `ensure_selection_visible()` for variable-height cards, mouse click handler for sub-line variants.

**Addresses:** Multi-line card layout (table stakes), git branch display (visible in UI)

**Avoids:** Pitfall 1 (mouse y-mapping -- build screen-row-to-project map), Pitfall 2 (scroll arithmetic -- screen-row-based offset), Pitfall 6 (color_range char vs byte -- use `.chars().count()`), Pitfall 7 (variable card height -- consider fixed height per verbosity mode), Pitfall 17 (separators waste space -- use indentation)

### Phase 3: Pipe Protocol for Pills + Progress

**Rationale:** Pills and progress are pure pipe extensions with no external command dependencies. They build on Phase 2's multi-line rendering infrastructure. The pipe protocol design is the key decision here -- get it right before external tools depend on it.

**Delivers:** Extended pipe handler for `sidebar::pill::`, `sidebar::progress::`, `sidebar::port::` messages. `render_pills_line()` showing pills as compact badges and progress as Unicode bar. Metadata cleanup on session stop.

**Addresses:** Status pills (differentiator), progress bars (differentiator), attention badge with message (differentiator)

**Avoids:** Pitfall 5 (protocol extensibility -- use args dict for structured data), Pitfall 12 (progress bar Unicode fragility -- use ASCII bars or isolate on own line), Pitfall 15 (plugin targeting -- use broadcast + is_primary filter), Pitfall 16 (stale metadata -- clear on session status transition)

### Phase 4: Polish + Optional Port Detection

**Rationale:** Port detection has the weakest value/complexity ratio and can be descoped entirely if milestones run long. Pipe-based port reporting (already functional from Phase 3) covers the reliable use case. This phase is for polish: truncation at narrow widths, color refinement, card spacing, and optionally lsof-based port auto-detection.

**Delivers:** Graceful truncation for narrow sidebars (12-16 char width). Card visual separation. Progress bar coloring (green/yellow/red by percentage). Optional lsof port detection (single global call, unattributed or cwd-heuristic matched).

**Addresses:** Listening port detection (differentiator, stretch goal), width-adaptive rendering

**Avoids:** Pitfall 8 (lsof slow on macOS -- run once per cycle, 30s+ interval, cache aggressively), Pitfall 9 (port attribution impossible -- show globally or use pipe-based)

### Phase Ordering Rationale

- **Data before rendering:** The polling pipeline must be proven before investing in multi-line UI. Git branch data arriving correctly (verifiable via `eprintln!`) validates `set_timeout`, `run_command`, and context routing -- all infrastructure that rendering depends on.
- **Rendering before pipes:** The multi-line card layout must exist before adding pills/progress. These features need a visual slot to occupy (line 3 of the card).
- **Pipes before ports:** Pills are simpler (pure pipe, no external commands) and validate the pipe protocol extension pattern. Port detection is the most complex and uncertain feature.
- **Polish last:** Truncation handling, color refinement, and optional lsof are low-risk improvements that don't affect architecture.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2 (Multi-Line Card Rendering):** The scroll + mouse refactor is the highest-risk change. Consider writing a focused plan with exact code diffs for `ensure_selection_visible`, the mouse handler, and `build_render_lines`. The ARCHITECTURE.md includes implementation sketches but they need validation against the actual current code structure.
- **Phase 3 (Pipe Protocol):** The protocol design (args dict vs name encoding) needs a final decision. PITFALLS.md recommends args dict for extensibility; STACK.md and FEATURES.md use `::` name encoding following the existing attention pattern. Reconcile before implementation.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Data Model + Polling):** Well-documented pattern. zjstatus uses identical `set_timeout` + `run_command` + context routing. API verified against zellij-tile source.
- **Phase 4 (Polish):** Standard rendering improvements. No new API surface.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Zero new dependencies. All APIs verified against zellij-tile 0.43.1 source code (shim.rs, data.rs). |
| Features | HIGH | CMUX comparison provides clear adopt/skip decisions. Feature dependencies mapped. MVP ordering justified. |
| Architecture | HIGH | Extension-based approach validated against existing ~600-line codebase. All patterns (cached_metadata, RenderLine variants, context routing) mirror proven existing patterns. |
| Pitfalls | HIGH | 18 pitfalls identified from code analysis + API docs. Critical pitfalls (mouse mapping, scroll, command ordering) have concrete prevention strategies with code examples. |

**Overall confidence:** HIGH

### Gaps to Address

- **Pipe protocol format:** PITFALLS.md recommends `args` dict for extensibility. STACK.md and FEATURES.md use `::` name encoding. The pragmatic choice is `::` encoding for v1.1 (matches existing attention pattern, simpler CLI usage) with awareness that a v2 protocol migration may be needed. Decide during Phase 3 planning.
- **Port attribution strategy:** All three research files agree that PID-based attribution is impossible from the WASM sandbox. The gap is whether to ship lsof at all in v1.1 or rely entirely on pipe-based ports. Recommendation: pipe-only for v1.1, lsof as Phase 4 stretch goal.
- **Fixed vs variable card height:** PITFALLS.md recommends fixed height (always 3 lines) to avoid layout jank. ARCHITECTURE.md and FEATURES.md show variable height (1-3 lines based on available metadata). Fixed height wastes space for projects without metadata; variable height causes jank as data loads. Decide during Phase 2 planning -- a hybrid (fixed for running sessions, single-line for not-started) may be optimal.
- **color_range character indexing:** Confirmed as character-based by code analysis, but no official documentation explicitly states this. Needs runtime verification early in Phase 2.

## Sources

### Primary (HIGH confidence)
- zellij-tile 0.43.1 source (`~/.cargo/registry/src/`) -- `set_timeout` (shim.rs:305), `run_command_with_env_variables_and_cwd` (shim.rs:337), `Event::Timer` (data.rs:891)
- [Plugin API Commands](https://zellij.dev/documentation/plugin-api-commands.html) -- official run_command, set_timeout docs
- [Plugin API Events](https://zellij.dev/documentation/plugin-api-events.html) -- Timer, RunCommandResult event docs
- [Plugin Pipes](https://zellij.dev/documentation/plugin-pipes.html) -- PipeMessage struct (name, payload, args)
- [Plugin UI Rendering](https://zellij.dev/documentation/plugin-ui-rendering.html) -- print_text_with_coordinates, color_range
- Existing `src/main.rs` codebase (~600 lines) -- verified current architecture

### Secondary (MEDIUM confidence)
- [zjstatus](https://github.com/dj95/zjstatus) -- validates timer + run_command polling pattern, git branch interval
- [CMUX](https://www.cmux.dev/) -- feature landscape reference (git branch, ports, pills, progress)
- [Zellij Plugin Dev Guide (dasroot.net)](https://dasroot.net/posts/2026/03/developing-plugins-for-zellij-comprehensive-guide/) -- context-based command correlation
- [lsof macOS (Simon Willison)](https://til.simonwillison.net/macos/lsof-macos) -- lsof flags, ~69ms timing

### Tertiary (LOW confidence)
- color_range character vs byte indexing -- inferred from existing code patterns, needs runtime verification
- Timer event f64 value semantics -- observed as elapsed seconds, not officially documented
- lsof output format stability across macOS versions -- tested on current macOS only

---
*Research completed: 2026-03-14*
*Ready for roadmap: yes*
