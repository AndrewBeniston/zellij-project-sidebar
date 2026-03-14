# Feature Landscape: v1.1 Rich Cards

**Domain:** Rich metadata sidebar for Zellij session management (CMUX-inspired card layout)
**Researched:** 2026-03-14
**Milestone context:** Adding multi-line card layout with per-project metadata to an existing working sidebar plugin

## Ecosystem Context

CMUX (cmux.dev) is the direct inspiration. It is a native macOS terminal built on libghostty that displays a vertical sidebar with rich workspace metadata: git branch, listening ports, PR status, progress bars, status pills, and notification text per workspace. CMUX achieves this natively with Swift/AppKit and full system access. This plugin must approximate similar information density within Zellij's WASM sandbox and character-cell rendering constraints.

The key constraint difference: CMUX has direct process access and GUI rendering. This plugin has `run_command` (async shell-outs with callback), `pipe` (message-based metadata injection), and `print_text_with_coordinates` (character-cell text positioning with `color_range` styling). Everything must be adapted to these primitives.

### What CMUX Shows Per Workspace

Based on CMUX documentation and community discussion:

1. **Workspace name** (with custom rename)
2. **Git branch** (auto-detected from working directory, instant HEAD change detection)
3. **Listening ports** (auto-detected from processes in workspace)
4. **PR status/number** (linked, clickable -- opens in browser)
5. **Progress bars** (for long-running tasks)
6. **Status pills** (custom key-value labels)
7. **Latest notification text** (from OSC 9/99/777 sequences or CLI)
8. **Working directory** (per-pane)
9. **Color indicator rail** (17 presets + custom per workspace)
10. **Log entries** (scrollable per workspace)

### What to Adopt vs Skip from CMUX

| CMUX Feature | Adopt? | Rationale |
|-------------|--------|-----------|
| Git branch | YES | High-value ambient awareness. Achievable via `run_command` with `git branch --show-current`. |
| Listening ports | YES | Valuable for dev servers. Achievable via `lsof -P -n -iTCP -sTCP:LISTEN` with PID filtering. |
| Status pills | YES | Extensible metadata. Existing pipe system (`sidebar::attention::`) already proves the pattern. Extend it. |
| Progress bars | YES | Useful for builds/deploys. Pipe-based, no external detection needed. |
| PR status | NO | Requires GitHub API auth, network calls from WASM. Over-complex. Anti-feature for a sidebar. |
| Notification text | PARTIAL | Already have attention system. Showing the last notification message adds value but is lower priority. |
| Working directory | NO | Not available in SessionInfo or PaneInfo. Would need `run_command` per pane -- too expensive. |
| Color indicator rail | NO | Zellij theme colours are limited. Custom colour assignment adds config complexity for minimal gain. |
| Log entries | NO | Scrollable logs per workspace is a CMUX-native feature. Character-cell sidebar is too narrow. |

---

## Table Stakes

Features that feel mandatory once you commit to multi-line cards. Without these, the card layout is just wasted vertical space.

| Feature | Why Expected | Complexity | Dependencies | Notes |
|---------|--------------|------------|--------------|-------|
| Multi-line card layout | Single-line list wastes the "rich card" premise. Users expect visual hierarchy once cards are introduced. | Medium | Existing render loop refactor | Currently each project is 1 line. Must become 2-3 lines with structured content areas. Requires changing `build_render_lines()` from 1:1 project:line to 1:N. |
| Git branch display | CMUX shows it. Any dev dashboard shows it. "What branch am I on?" is the first question after "what project is this?" | Medium | `run_command` infrastructure, project path knowledge, `set_timeout` for periodic refresh | Shell out to `git branch --show-current` per project directory. Fallback to `git rev-parse --short HEAD` for detached HEAD. Must handle non-git directories gracefully. |
| Session status on card | Already exists (status dot). Must survive the card layout transition. | Low | Existing `SessionStatus` enum | Move status dot into card line 1. Keep green/yellow/gray semantic colours. |
| Tab count on card | Already exists (bracket notation). Must survive card layout transition. | Low | Existing `SessionStatus::Running.tab_count` | Move `[3]` into card layout, likely on line 1 after name. |
| Active command on card | Already exists for current session. Must survive card layout transition. | Low | Existing `extract_active_command()` | Show command name on line 2 or as inline badge. |

## Differentiators

Features that elevate beyond a simple card layout. These create the "command center" feeling CMUX achieves.

| Feature | Value Proposition | Complexity | Dependencies | Notes |
|---------|-------------------|------------|--------------|-------|
| **Status pills via pipe** | Extensible metadata without plugin code changes. Any script/tool can tag a session with key-value pills (e.g., "build: passing", "deploy: staging"). Mirrors CMUX's status pill pattern. | Medium | Extend existing pipe protocol (`sidebar::pill::session::key=value`). Store as `BTreeMap<String, BTreeMap<String, String>>` per session. | The existing attention pipe pattern (`sidebar::attention::name`) proves this works. Pills are a generalisation: instead of a boolean flag, they carry a label and optional colour hint. |
| **Progress bar via pipe** | Long-running operations (builds, deploys, test suites) can show 0-100% completion inline in the card. Visual progress is immediately useful -- CMUX shows this prominently. | Medium | New pipe message (`sidebar::progress::session=value`). Store as `Option<u8>` per session. Render using Unicode block elements (U+2588-U+258F). | Unicode partial blocks give 8 sub-character steps per cell. A 10-char progress bar gives 80 visual steps -- more than enough for percentage display. |
| **Listening port detection** | "What ports is this project serving?" is a constant question during development. CMUX shows this automatically. Knowing port 3000 is active saves switching to check. | High | `run_command` with `lsof -P -n -iTCP -sTCP:LISTEN`, PID-to-session mapping, periodic refresh via `set_timeout`, result parsing. | Hardest feature. Must map ports to sessions (not just list all ports). Requires knowing session PIDs or filtering by CWD. Since PaneInfo lacks PID, may need heuristic: run lsof per project directory and correlate. Consider making this opt-in via config. |
| **Attention badge with message** | Extend existing boolean attention flag to carry the notification text. Instead of just a red diamond, show "Waiting for input" or "Build failed". | Low | Extend pipe payload: `sidebar::attention::session` with `--payload "message text"`. Pipe API already supports payloads. | Existing `attention_sessions: BTreeSet<String>` becomes `BTreeMap<String, Option<String>>` to store the message. Truncate to card width. |

## Anti-Features

Features to explicitly NOT build for this milestone. Tempting extensions that would bloat scope or degrade the sidebar experience.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| PR status / GitHub integration | Requires OAuth, network calls from WASM sandbox, API rate limiting. CMUX does this because it is a native app with full network access. | If users want PR info, they can send it via status pills from a git hook: `zellij pipe --name "sidebar::pill::myproject::pr=#123"` |
| Per-pane working directory display | PaneInfo has no CWD field. Would need `run_command` per pane per refresh cycle -- O(panes) shell-outs. Too expensive for a sidebar. | Show project-level path (already known from config/discovery). Branch implies the repo context. |
| Inline git diff stats | `git diff --stat` per project is expensive and produces too much output for a narrow sidebar. | Status pills can carry "dirty" / "clean" state if users wire it up via hooks. |
| Scrollable per-project logs | CMUX shows log entries per workspace. In a 15-20% width character-cell sidebar, scrollable logs per card would be illegible and complex to implement. | Single-line notification message via attention system. |
| Custom icons per project type | Nerd Font glyphs for project types (Rust/Node/Python). Assumes Nerd Font is installed. Not everyone has Nerd Fonts. Adds config surface. | Use Unicode symbols that work in any font: dots, diamonds, arrows. Keep icons semantic (status) not decorative (language). |
| Real-time port monitoring | Continuously polling `lsof` every second is wasteful. CMUX can do this because it monitors process spawns natively. | Poll ports on a timer (every 10-30 seconds) or on session switch events. Stale-by-seconds is fine for a sidebar. |
| Card expand/collapse | Tapping a card to show more detail adds interaction state complexity. The sidebar should show all info at once at the configured verbosity. | Use verbosity config (existing). Minimal mode = 1 line. Full mode = 3 lines. No per-card toggle. |

---

## Feature Dependencies

```
Existing pipe infrastructure
  |
  +---> Status pills via pipe (extends pipe protocol)
  +---> Progress bar via pipe (extends pipe protocol)
  +---> Attention badge with message (extends pipe payload)

Existing run_command infrastructure (scan_dir)
  |
  +---> Git branch detection (new run_command usage)
  +---> Listening port detection (new run_command usage)
  |
  +---> set_timeout for periodic refresh (new: Timer event subscription)

Multi-line card layout (render refactor)
  |
  +---> Git branch display (needs line 2 in card)
  +---> Port display (needs line 2 in card)
  +---> Status pills (needs line 2 or 3 in card)
  +---> Progress bar (needs line 2 or 3 in card)

Project path knowledge (from config/discovery)
  |
  +---> Git branch detection (needs path to run git commands in)
  +---> Port detection (needs path for process correlation)
```

## Card Layout Design

### Proposed Visual Layout (Full Verbosity, ~25 chars wide)

```
 ● my-project          [3]
   main   :3000  claude
   ■ build:ok ━━━━━━━ 78%

 ○ other-project
   feat/new-ui
```

**Line 1: Identity + Status**
- Status dot (existing: green/yellow/gray/red)
- Project name (existing)
- Tab count in brackets (existing, for running sessions)

**Line 2: Context**
- Git branch name (new, truncated to fit)
- Listening port(s) (new, `:3000` or `:3000,:8080`)
- Active command (existing, for current session)

**Line 3: Metadata (optional, only if pills or progress exist)**
- Status pills (new, compact: `build:ok`)
- Progress bar (new, Unicode blocks: `━━━━━━━ 78%`)

**Spacing: blank line between cards for visual separation**

### Proposed Visual Layout (Minimal Verbosity)

```
 ● my-project
 ○ other-project
```

Same as current -- single line per project, no metadata. The verbosity setting controls how many lines per card.

### Width Considerations

The sidebar is typically 15-20% of terminal width. At 200 columns, that is 30-40 chars. At 80 columns, that is 12-16 chars.

| Width | Line 1 | Line 2 | Line 3 |
|-------|--------|--------|--------|
| 12-16 | `● proj...` | `main` | Omit |
| 20-25 | `● my-project [3]` | `main :3000` | `━━━ 78%` |
| 30-40 | `● my-project       [3]` | `main  :3000  claude` | `build:ok ━━━━━━ 78%` |

Must truncate/elide gracefully. Branch names and project names are the primary truncation targets.

---

## Implementation Approach by Feature

### Git Branch Detection

**Method:** `run_command(&["git", "branch", "--show-current"], context)` with CWD set per project path.

**Problem:** Zellij's `run_command` does not support setting CWD for the spawned process. The command runs in whatever the host's CWD is.

**Workaround:** Use `git -C /path/to/project branch --show-current` which tells git to operate on a different directory without changing CWD.

**Refresh strategy:** Use `set_timeout(5.0)` to re-poll git branch every 5 seconds. Subscribe to `Timer` event. On timer fire, re-run git commands for all projects and reset timer.

**Context routing:** Use `BTreeMap` context with a `"cmd"` key set to `"git_branch"` and a `"project"` key set to the project name, so `RunCommandResult` can be routed back to the correct project.

**Edge cases:**
- Non-git directories: command exits non-zero, store `None` for branch
- Detached HEAD: `branch --show-current` returns empty, fallback to `git -C path rev-parse --short HEAD`
- Many projects: running 20+ git commands every 5s is fine -- git status is fast (<10ms each)

**Confidence:** HIGH -- `run_command` with context routing already works for scan_dir. Git commands are lightweight.

### Listening Port Detection

**Method:** `run_command(&["lsof", "-P", "-n", "-iTCP", "-sTCP:LISTEN"], context)` then parse output.

**Problem:** lsof returns ALL listening ports system-wide. Need to correlate ports to projects/sessions.

**Correlation approaches:**
1. **By PID tree:** Get session pane PIDs, find child processes, match against lsof PIDs. But PaneInfo has no PID field -- dead end.
2. **By CWD heuristic:** Run `lsof` with CWD filter. But lsof doesn't filter by CWD natively.
3. **By known ports:** User configures expected ports per project in KDL config. Plugin just checks if those ports are listening.
4. **By process name:** Match lsof process names against known dev server names (node, python, ruby, etc.) and correlate by... nothing useful.

**Recommended approach:** Option 3 -- user declares expected ports in config. Plugin verifies they are listening. This is deterministic, cheap (single lsof call), and avoids false correlations.

```kdl
plugin location="file:..." {
    scan_dir "/Users/me/Projects"
    // Port hints per project (optional)
    ports_myproject "3000,3001"
    ports_other "8080"
}
```

Or via pipe message: `zellij pipe --name "sidebar::port::myproject" --payload "3000"`

**Confidence:** MEDIUM -- port detection is feasible but correlation is the hard part. Config-declared ports are reliable; auto-detection is fragile.

### Status Pills via Pipe

**Method:** Extend existing pipe protocol.

**Protocol:**
```bash
# Set a pill
zellij pipe --name "sidebar::pill::session-name::key" --payload "value"

# Clear a pill
zellij pipe --name "sidebar::pill::session-name::key"  # empty payload = clear

# Clear all pills for a session
zellij pipe --name "sidebar::pills-clear::session-name"
```

**Storage:** Add `pills: BTreeMap<String, BTreeMap<String, String>>` to `State`. Key = session name, inner key = pill name, inner value = pill display text.

**Rendering:** On line 3 of card, render as compact badges: `build:ok test:3/5`. Truncate if too many. Use colour hints: green for "ok"/"pass", red for "fail"/"error", gray for neutral.

**Confidence:** HIGH -- follows established pipe pattern. No external dependencies.

### Progress Bar via Pipe

**Method:** Pipe message with percentage value.

**Protocol:**
```bash
# Set progress (0-100)
zellij pipe --name "sidebar::progress::session-name" --payload "78"

# Clear progress
zellij pipe --name "sidebar::progress::session-name" --payload ""
```

**Storage:** Add `progress: BTreeMap<String, u8>` to `State`.

**Rendering:** Unicode block elements for smooth fill:
- Full block: `\u{2588}` (100%)
- 7/8 block: `\u{2589}` through 1/8 block: `\u{258F}`
- Empty: space

A 8-character bar at 78%: `\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{258A} ` + `78%`

**Confidence:** HIGH -- pure rendering, no external dependencies. Pipe protocol is proven.

### Attention Badge with Message

**Method:** Extend existing attention pipe to accept payload.

**Protocol:** Already supported by Zellij pipes:
```bash
# Attention with message
zellij pipe --name "sidebar::attention::session-name" --payload "Waiting for input"

# Attention without message (existing behavior preserved)
zellij pipe --name "sidebar::attention::session-name"
```

**Storage:** Change `attention_sessions: BTreeSet<String>` to `BTreeMap<String, Option<String>>`.

**Rendering:** Show message text on line 2 or 3, truncated to card width. Red colour for urgency.

**Confidence:** HIGH -- minimal change to existing system. Backward compatible.

---

## MVP Recommendation for v1.1

Prioritize in this order based on value/complexity ratio and dependencies:

1. **Multi-line card layout refactor** -- Foundation. Nothing else works without this. Refactor render loop to support N lines per project. Controlled by verbosity setting.
2. **Git branch display** -- Highest value single metadata item. "What branch am I on?" is universal. Uses proven `run_command` infrastructure.
3. **Status pills via pipe** -- Extensible metadata that grows with the user's workflow. Low implementation cost given existing pipe infrastructure.
4. **Progress bar via pipe** -- Complements pills. Pure rendering feature with no external dependencies.
5. **Attention badge with message** -- Minimal extension to existing attention system. Backward compatible.
6. **Listening port detection** -- Highest complexity, most fragile. Start with config-declared ports. Auto-detection is a stretch goal.

**Defer from v1.1:**
- Port auto-detection (no reliable PID correlation from WASM sandbox)
- PR status (requires GitHub API, auth, network -- out of scope)
- Per-pane working directory (PaneInfo lacks CWD field)

## Phase Ordering Rationale

The card layout refactor must come first because every subsequent feature needs the multi-line rendering infrastructure. Git branch follows because it exercises the `run_command` + `set_timeout` + context routing pattern that port detection will also need. Pills and progress are pure pipe extensions and can be built independently. Port detection comes last because it has the weakest value/complexity ratio and can be descoped if the milestone runs long.

## Sources

- [CMUX - Official site](https://www.cmux.dev/)
- [CMUX - GitHub repository](https://github.com/manaflow-ai/cmux)
- [CMUX - Changelog](https://www.cmux.dev/docs/changelog)
- [CMUX - HN discussion](https://news.ycombinator.com/item?id=47079718)
- [Calyx vs cmux comparison](https://dev.to/yuu1ch13/calyx-vs-cmux-choosing-the-right-ghostty-based-terminal-for-macos-26-28e7)
- [Zellij Plugin API Commands](https://zellij.dev/documentation/plugin-api-commands.html)
- [Zellij Plugin API Events](https://zellij.dev/documentation/plugin-api-events.html)
- [Zellij Plugin Pipes](https://zellij.dev/documentation/plugin-pipes)
- [Zellij Plugin UI Rendering](https://zellij.dev/documentation/plugin-ui-rendering.html)
- [zellij-tile 0.43.1 SessionInfo](https://docs.rs/zellij-tile/0.43.1/zellij_tile/prelude/struct.SessionInfo.html)
- [zellij-tile 0.43.1 PaneInfo](https://docs.rs/zellij-tile/0.43.1/zellij_tile/prelude/struct.PaneInfo.html)
- [Unicode Block Elements for progress bars](https://changaco.oy.lc/unicode-progress-bars/)
- [Git branch detection best practices](https://adamj.eu/tech/2023/08/20/git-output-just-current-branch-name/)
- [macOS lsof usage](https://til.simonwillison.net/macos/lsof-macos)
