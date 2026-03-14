# Architecture: Rich Card Integration

**Domain:** Zellij WASM Plugin -- Multi-line card sidebar with metadata polling
**Researched:** 2026-03-14
**Confidence:** HIGH (verified against zellij-tile 0.43.1 source, existing working codebase)

## Context: What Exists Today

The plugin is ~600 lines in a single `src/main.rs` with this architecture:

```
State struct
  +-- projects: Vec<Project>          (name, path, SessionStatus)
  +-- cached_statuses: BTreeMap       (session name -> SessionStatus)
  +-- attention_sessions: BTreeSet    (sessions needing attention)
  +-- UI state: selected_index, scroll_offset, browse_mode, search_query
  +-- Config: verbosity, scan_dir, session_layout, is_primary

RenderLine enum
  +-- Header(String)
  +-- ProjectRow(usize)               (index into self.projects)

Event flow:
  SessionUpdate -> update_cached_statuses() -> rebuild_projects() -> render()
  RunCommandResult -> handle scan_dir results -> rebuild_projects() -> render()
  Key/Mouse -> navigate/activate/kill -> render()
  Pipe -> toggle/attention/clear -> render()

Render flow:
  build_render_lines() -> ensure_selection_visible() -> iterate RenderLines
  Each RenderLine occupies exactly ONE screen row.
  render_project_line() produces a single Text with color_range calls.
```

## Integration Strategy

The v1.1 features (multi-line cards, git branch, ports, pills, progress) must integrate with this architecture, not replace it. The codebase is small enough that refactoring is cheap, but the core patterns (event-driven state, RenderLine-based rendering, `run_command` for external data) are sound and should be extended.

### Decision: Extend, Don't Rewrite

The current architecture handles the new features well with targeted additions:

1. **RenderLine** gains new variants for card sub-lines
2. **Project** gains a `metadata` field (new struct)
3. **State** gains polling timer state
4. **Pipe handler** gains pill/progress protocol
5. **Render** adapts to multi-line cards via existing `print_text_with_coordinates`

## Component Changes

### 1. Project Metadata Struct (NEW)

Create a `ProjectMetadata` struct to hold per-project enrichment data. This keeps the `Project` struct clean and makes metadata optional.

```rust
#[derive(Clone, Default)]
struct ProjectMetadata {
    git_branch: Option<String>,
    listening_ports: Vec<u16>,
    pills: BTreeMap<String, String>,      // key -> value (e.g. "env" -> "prod")
    progress: Option<u8>,                  // 0-100
}
```

**Why a separate struct instead of flat fields on Project:**
- Metadata is optional and independent of session status
- All metadata fields share the same lifecycle (populated by polling/pipe, cleared on session stop)
- Makes it trivial to add new metadata types later without touching Project
- `Default` trait gives clean "no metadata yet" state

**Integration point:** `Project` gains one field:

```rust
struct Project {
    name: String,
    path: String,
    status: SessionStatus,
    metadata: ProjectMetadata,  // NEW
}
```

### 2. RenderLine Extension (MODIFIED)

The current `RenderLine` enum maps 1:1 to screen rows. Multi-line cards break this assumption. Two approaches:

**Option A: Add sub-line variants**
```rust
enum RenderLine {
    Header(String),
    ProjectRow(usize),           // existing: name + status (line 1)
    ProjectDetail(usize),        // NEW: branch + ports (line 2)
    ProjectPills(usize),         // NEW: pills + progress (line 3)
}
```

**Option B: Make ProjectRow emit multiple screen rows**

Option A is correct because:
- `build_render_lines()` already iterates and pushes variants -- adding more is trivial
- `ensure_selection_visible()` needs to know how many screen rows a "card" occupies -- explicit variants make this countable
- Mouse click mapping (`LeftClick` handler) maps screen y-coordinate to `RenderLine` index -- sub-line variants let it resolve which project was clicked regardless of which line within the card
- Scroll offset arithmetic stays simple: each RenderLine = one screen row

**Implementation:**

```rust
fn build_render_lines(&self) -> Vec<RenderLine> {
    let mut lines = Vec::new();
    let filtered = self.filtered_indices();

    if self.use_discovery && self.browse_mode && !filtered.is_empty() {
        lines.push(RenderLine::Header("All projects".to_string()));
    }

    for &i in &filtered {
        lines.push(RenderLine::ProjectRow(i));
        // Only emit detail/pill lines for running sessions with metadata
        if self.verbosity == Verbosity::Full {
            let project = &self.projects[i];
            if let SessionStatus::Running { .. } = &project.status {
                let meta = &project.metadata;
                if meta.git_branch.is_some() || !meta.listening_ports.is_empty() {
                    lines.push(RenderLine::ProjectDetail(i));
                }
                if !meta.pills.is_empty() || meta.progress.is_some() {
                    lines.push(RenderLine::ProjectPills(i));
                }
            }
        }
    }

    lines
}
```

**Verbosity interaction:**
- `Minimal`: Only `ProjectRow` (name + dot) -- no detail lines, same as today
- `Full`: `ProjectRow` + optional `ProjectDetail` + optional `ProjectPills`

This means cards are 1-3 lines tall depending on available metadata and verbosity.

### 3. Selection Model (MODIFIED)

Currently `selected_index` points into the filtered list. With multi-line cards, selection is per-project, not per-line. The existing `selected_project_index()` already resolves to a project index, so selection semantics don't change -- but navigation (Up/Down) must skip sub-lines.

**Key change:** Up/Down should move between projects, not between render lines.

```rust
// Navigation moves between projects in filtered list, not render lines
BareKey::Down => {
    let filtered_len = self.filtered_indices().len();
    if filtered_len > 0 {
        self.selected_index = (self.selected_index + 1)
            .min(filtered_len.saturating_sub(1));
    }
    true
}
```

This is actually unchanged from today because `selected_index` indexes into `filtered_indices()` (projects), not into `render_lines`. The render system handles mapping project selection to multi-line highlight.

**Mouse click change:** Click on any line of a card should select that project. The `LeftClick` handler maps screen y -> render_line index -> extract project index regardless of whether it hit `ProjectRow`, `ProjectDetail`, or `ProjectPills`:

```rust
Mouse::LeftClick(line, _col) => {
    let render_idx = self.scroll_offset + (click_y - y_offset);
    if render_idx < render_lines.len() {
        let project_idx = match render_lines[render_idx] {
            RenderLine::ProjectRow(idx) => Some(idx),
            RenderLine::ProjectDetail(idx) => Some(idx),
            RenderLine::ProjectPills(idx) => Some(idx),
            _ => None,
        };
        if let Some(idx) = project_idx {
            // find position in filtered list
            let filtered = self.filtered_indices();
            if let Some(fi) = filtered.iter().position(|&i| i == idx) {
                self.selected_index = fi;
                self.activate_selected_project();
            }
        }
    }
}
```

### 4. Scroll Visibility (MODIFIED)

`ensure_selection_visible()` must account for multi-line cards. The selected project may span 1-3 render lines. All lines of the selected card must be visible:

```rust
fn ensure_selection_visible(&mut self, render_lines: &[RenderLine], visible_rows: usize) {
    if visible_rows == 0 { return; }
    let selected_proj = self.selected_project_index();

    // Find first and last render line for selected project
    let first_y = render_lines.iter().position(|line| {
        matches!((line, selected_proj),
            (RenderLine::ProjectRow(idx), Some(sel))
            | (RenderLine::ProjectDetail(idx), Some(sel))
            | (RenderLine::ProjectPills(idx), Some(sel))
            if *idx == sel
        )
    });
    let last_y = render_lines.iter().rposition(|line| {
        matches!((line, selected_proj),
            (RenderLine::ProjectRow(idx), Some(sel))
            | (RenderLine::ProjectDetail(idx), Some(sel))
            | (RenderLine::ProjectPills(idx), Some(sel))
            if *idx == sel
        )
    });

    if let (Some(first), Some(last)) = (first_y, last_y) {
        if first < self.scroll_offset {
            self.scroll_offset = first;
        }
        if last >= self.scroll_offset + visible_rows {
            self.scroll_offset = last - visible_rows + 1;
        }
    }
}
```

### 5. Periodic Polling with set_timeout + Timer (NEW)

Git branch and listening ports cannot be obtained from the Zellij plugin API -- they require shelling out via `run_command`. The plugin needs periodic polling.

**Mechanism:** `set_timeout(secs)` schedules a `Timer(f64)` event. On each Timer, the plugin fires `run_command` for git branches and ports, then schedules the next timeout.

```
load() -> subscribe to [Timer, RunCommandResult] -> set_timeout(5.0)
                                                        |
Timer(5.0) -> poll_metadata() -> set_timeout(10.0) ----+
                |                                       |
                +-- run_command(["git", ...], ctx)       |
                +-- run_command(["lsof", ...], ctx)      |
                                                        |
RunCommandResult(exit, stdout, stderr, ctx) ------> parse results
                |                                   store in ProjectMetadata
                +-- ctx["cmd"] == "git_branch"      rebuild_projects()
                +-- ctx["cmd"] == "lsof_ports"      render()
```

**Polling intervals:**
- First poll: 2 seconds after permissions granted (fast initial data)
- Subsequent polls: 10 seconds (git branches and ports don't change that fast)
- Only poll for running sessions (don't waste commands on NotStarted/Exited)

**State additions:**
```rust
struct State {
    // ... existing fields ...
    polling_active: bool,
    metadata: BTreeMap<String, ProjectMetadata>,  // session_name -> metadata
}
```

**Why `metadata` on State as a BTreeMap, separate from Project:**
The `metadata` map persists across `rebuild_projects()` calls. When `rebuild_projects()` reconstructs `self.projects` from `discovered_dirs` + `cached_statuses`, it can merge metadata from this map. This follows the same pattern as `cached_statuses` -- a stable cache that survives project list rebuilds.

Actually, on reflection, the simpler approach is to store metadata directly on `Project.metadata` and preserve it during `rebuild_projects()` the same way statuses are preserved. This avoids a second parallel map. The `rebuild_projects()` function already does a lookup by name for status -- adding a metadata lookup is trivial:

```rust
fn rebuild_projects(&mut self) {
    // ... existing logic ...
    self.projects = self.discovered_dirs.iter()
        .map(|(name, path)| {
            let status = self.cached_statuses.get(name).cloned()
                .unwrap_or(SessionStatus::NotStarted);
            let metadata = self.cached_metadata.get(name).cloned()
                .unwrap_or_default();
            Project { name: name.clone(), path: path.clone(), status, metadata }
        })
        .collect();
}
```

So State gets `cached_metadata: BTreeMap<String, ProjectMetadata>` alongside `cached_statuses`.

### 6. Git Branch Detection (NEW)

**Command:** `git -C <project_path> rev-parse --abbrev-ref HEAD`

This is fast (~5ms), works in detached HEAD (returns `HEAD`), and requires no setup.

**Implementation:**

```rust
const CMD_GIT_BRANCH: &str = "git_branch";

fn poll_git_branches(&self) {
    for project in &self.projects {
        if matches!(project.status, SessionStatus::Running { .. }) && !project.path.is_empty() {
            let mut ctx = BTreeMap::new();
            ctx.insert(CMD_KEY.to_string(), CMD_GIT_BRANCH.to_string());
            ctx.insert("project".to_string(), project.name.clone());
            run_command_with_env_variables_and_cwd(
                &["git", "rev-parse", "--abbrev-ref", "HEAD"],
                BTreeMap::new(),
                PathBuf::from(&project.path),
                ctx,
            );
        }
    }
}
```

**Result handling in `RunCommandResult`:**
```rust
Some(CMD_GIT_BRANCH) => {
    if exit_code == Some(0) {
        let branch = String::from_utf8_lossy(&stdout).trim().to_string();
        if let Some(project_name) = context.get("project") {
            let meta = self.cached_metadata
                .entry(project_name.clone())
                .or_default();
            meta.git_branch = Some(branch);
            self.apply_cached_metadata();
        }
    }
    true
}
```

### 7. Port Detection (NEW)

**Command (macOS):** `lsof -i -P -n -sTCP:LISTEN`

This lists all listening TCP ports. The plugin filters to ports owned by processes in the session's pane tree.

**Challenge:** The plugin cannot know which PIDs belong to which session. Zellij's `PaneInfo` does not expose PID. Two approaches:

**Approach A (Recommended): Session-scoped port detection via cwd heuristic**

Rather than matching PIDs, use the session's working directory as a heuristic. Many dev servers (Next.js, Vite, Rails, etc.) bind to well-known ports. The plugin can detect ports by scanning for common dev server ports. But this is fragile.

**Approach B (Simpler, recommended): Pipe-based port reporting**

External tools (dev server wrappers, shell hooks) send port info to the plugin via pipe messages. This is consistent with the existing attention system pattern -- the plugin is a display surface, not a scanner.

```bash
# In a dev server wrapper or shell hook:
zellij pipe "sidebar::port::my-project" --payload "3000,5173"
```

The plugin receives this in `pipe()` and stores it in metadata.

**Approach C (Pragmatic hybrid): Best-effort lsof + pipe override**

Run `lsof` globally, parse all listening ports, display them unattributed (just "ports: 3000, 5173" at the bottom of the sidebar or on the current session's card). Allow pipe messages to attribute ports to specific sessions.

**Recommendation: Start with Approach B (pipe-based), add Approach C later.**

Pipe-based is simpler, more accurate, and follows the plugin's existing pattern. The `lsof` approach adds complexity (parsing, PID attribution) for uncertain value. The pill system can display port info sent via pipe.

However, for a quick win without external tool setup, a lightweight lsof scan for JUST the current session is viable:

```rust
const CMD_LSOF_PORTS: &str = "lsof_ports";

fn poll_ports(&self) {
    let mut ctx = BTreeMap::new();
    ctx.insert(CMD_KEY.to_string(), CMD_LSOF_PORTS.to_string());
    // Scan all listening TCP ports -- attribute later
    run_command(
        &["lsof", "-i", "-P", "-n", "-sTCP:LISTEN"],
        ctx,
    );
}
```

Parse output to extract port numbers, display on current session's card. For non-current sessions, rely on pipe messages.

### 8. Pipe Protocol for Pills and Progress (NEW)

Extend the existing pipe protocol (which already handles `sidebar::attention::` and `sidebar::clear::`) with new message types.

**Protocol design:**

| Message Name | Payload | Effect |
|---|---|---|
| `sidebar::pill::<session>::<key>` | `<value>` | Set pill `key=value` on session's card |
| `sidebar::pill::clear::<session>::<key>` | (none) | Remove pill `key` from session |
| `sidebar::progress::<session>` | `<0-100>` | Set progress bar percentage |
| `sidebar::progress::clear::<session>` | (none) | Remove progress bar |
| `sidebar::port::<session>` | `<port1,port2,...>` | Set listening ports |
| `sidebar::port::clear::<session>` | (none) | Clear ports |

**CLI usage examples:**

```bash
# Set a pill showing environment
zellij pipe "sidebar::pill::my-project::env" --payload "prod"

# Set progress for a build
zellij pipe "sidebar::progress::my-project" --payload "42"

# Report listening ports
zellij pipe "sidebar::port::my-project" --payload "3000,5173"

# Clear progress when done
zellij pipe "sidebar::progress::clear::my-project"
```

**Implementation in pipe():**

```rust
fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
    let name = pipe_message.name.as_str();
    match name {
        // ... existing handlers ...

        _ if name.starts_with("sidebar::pill::clear::") => {
            // Parse: sidebar::pill::clear::<session>::<key>
            let rest = name.strip_prefix("sidebar::pill::clear::").unwrap_or("");
            if let Some((session, key)) = rest.split_once("::") {
                let meta = self.cached_metadata.entry(session.to_string()).or_default();
                meta.pills.remove(key);
                self.apply_cached_metadata();
            }
            true
        }
        _ if name.starts_with("sidebar::pill::") => {
            let rest = name.strip_prefix("sidebar::pill::").unwrap_or("");
            if let Some((session_and_key, _)) = rest.split_once("::").or(Some((rest, ""))) {
                // Parse: sidebar::pill::<session>::<key>
                if let Some((session, key)) = rest.split_once("::") {
                    let value = pipe_message.payload.unwrap_or_default();
                    let meta = self.cached_metadata.entry(session.to_string()).or_default();
                    meta.pills.insert(key.to_string(), value);
                    self.apply_cached_metadata();
                }
            }
            true
        }
        _ if name.starts_with("sidebar::progress::clear::") => {
            let session = name.strip_prefix("sidebar::progress::clear::").unwrap_or("");
            if !session.is_empty() {
                let meta = self.cached_metadata.entry(session.to_string()).or_default();
                meta.progress = None;
                self.apply_cached_metadata();
            }
            true
        }
        _ if name.starts_with("sidebar::progress::") => {
            let session = name.strip_prefix("sidebar::progress::").unwrap_or("");
            if !session.is_empty() {
                if let Some(payload) = pipe_message.payload {
                    if let Ok(pct) = payload.trim().parse::<u8>() {
                        let meta = self.cached_metadata.entry(session.to_string()).or_default();
                        meta.progress = Some(pct.min(100));
                        self.apply_cached_metadata();
                    }
                }
            }
            true
        }

        _ => false,
    }
}
```

**Note on ordering:** The `pill::clear::` prefix must be matched BEFORE `pill::` to avoid ambiguity. Same for `progress::clear::` before `progress::`.

### 9. Rendering Multi-Line Cards (NEW render methods)

Each card renders as 1-3 rows depending on metadata availability:

```
Row 1 (ProjectRow):    ● project-name [3] claude
Row 2 (ProjectDetail):   main  :3000 :5173
Row 3 (ProjectPills):    prod  ██████░░░░ 60%
```

**Row 1 -- ProjectRow (existing, unchanged):**
Status dot + name + tab count + active command. Uses existing `render_project_line()`.

**Row 2 -- ProjectDetail (NEW):**
```rust
fn render_detail_line(&self, project: &Project, is_selected: bool, cols: usize) -> Text {
    let meta = &project.metadata;
    let mut parts = String::from("  "); // indent to align with name

    if let Some(ref branch) = meta.git_branch {
        parts.push_str(&format!(" {}", branch)); // git branch icon
    }

    for port in &meta.listening_ports {
        parts.push_str(&format!(" :{}", port));
    }

    let display: String = if parts.chars().count() > cols {
        parts.chars().take(cols.saturating_sub(1)).collect::<String>() + "..."
    } else {
        parts
    };

    let mut text = Text::new(&display);
    if is_selected {
        text = text.selected();
    }
    // Branch in blue, ports in yellow
    // Color ranges calculated from string positions
    text = text.color_all(COLOR_GRAY); // dim by default
    text
}
```

**Row 3 -- ProjectPills (NEW):**
```rust
fn render_pills_line(&self, project: &Project, is_selected: bool, cols: usize) -> Text {
    let meta = &project.metadata;
    let mut parts = String::from("  "); // indent

    // Pills: key-value pairs
    for (key, value) in &meta.pills {
        parts.push_str(&format!(" {}", value)); // just show value, compact
    }

    // Progress bar
    if let Some(pct) = meta.progress {
        let bar_width = 8;
        let filled = (bar_width * pct as usize) / 100;
        let empty = bar_width - filled;
        let bar: String = format!(
            " {}{}  {}%",
            "█".repeat(filled),
            "░".repeat(empty),
            pct
        );
        parts.push_str(&bar);
    }

    let display: String = if parts.chars().count() > cols {
        parts.chars().take(cols.saturating_sub(1)).collect::<String>() + "..."
    } else {
        parts
    };

    let mut text = Text::new(&display);
    if is_selected {
        text = text.selected();
    }
    text
}
```

**Selected card highlight:** When a project is selected, ALL of its render lines (Row 1-3) get `.selected()`. The existing render loop handles this naturally -- each line checks if its project index matches the selected project.

### 10. Timer + Polling Lifecycle (NEW)

**State additions:**

```rust
struct State {
    // ... existing ...
    polling_active: bool,
    cached_metadata: BTreeMap<String, ProjectMetadata>,
}
```

**Event subscription update in load():**

```rust
let mut events = vec![
    EventType::SessionUpdate,
    EventType::PermissionRequestResult,
    EventType::Key,
    EventType::Mouse,
    EventType::Timer,           // NEW
];
if self.use_discovery {
    events.push(EventType::RunCommandResult);
}
// RunCommandResult also needed for git/port polling even in non-discovery mode
// Actually: always subscribe to RunCommandResult for metadata polling
events.push(EventType::RunCommandResult);
```

**Permission update:**
```rust
let mut permissions = vec![
    PermissionType::ReadApplicationState,
    PermissionType::ChangeApplicationState,
    PermissionType::Reconfigure,
    PermissionType::RunCommands,  // Always needed now for git/port polling
];
```

**Timer handler:**

```rust
Event::Timer(_elapsed) => {
    if self.polling_active {
        self.poll_git_branches();
        // self.poll_ports();  // if using lsof approach
        set_timeout(10.0); // schedule next poll
    }
    false // don't re-render on timer itself, wait for RunCommandResult
}
```

**Start polling after permissions granted:**

```rust
Event::PermissionRequestResult(PermissionStatus::Granted) => {
    // ... existing setup ...
    self.polling_active = true;
    set_timeout(2.0); // first poll after 2s (give sessions time to appear)
    true
}
```

## Data Flow Summary

```
                    +-----------+
                    |  Timer    |----> poll_git_branches()
                    |  (10s)    |----> poll_ports() [optional]
                    +-----------+
                         |
                         v
              +--------------------+
              | RunCommandResult   |----> parse git branch / ports
              | (context routing)  |----> update cached_metadata
              +--------------------+----> apply_cached_metadata()
                                          rebuild_projects()
                                               |
                    +-----------+              |
                    |  Pipe     |----> update cached_metadata
                    |  messages |      (pills, progress, ports)
                    +-----------+              |
                         |                     v
                    +----------+        +-----------+
                    | Session  |------> | Project   |
                    | Update   |        | (merged)  |
                    +----------+        +-----------+
                                              |
                                              v
                                       +-----------+
                                       |  Render   |
                                       | (1-3 rows |
                                       |  per card)|
                                       +-----------+
```

## File Organization

The current single-file architecture (`src/main.rs`) will grow from ~600 to ~900-1100 lines with these additions. This is still manageable in a single file, but the natural module boundaries are clear if refactoring is desired later:

| Concern | Current Location | After v1.1 |
|---------|-----------------|------------|
| Data types (Project, SessionStatus, etc.) | main.rs top | Could be `src/types.rs` |
| State struct + methods | main.rs middle | Could be `src/state.rs` |
| Render methods | main.rs (render_project_line) | Could be `src/render.rs` |
| Pipe protocol | main.rs (pipe()) | Could be `src/protocol.rs` |
| Polling logic | N/A | Could be `src/polling.rs` |

**Recommendation: Stay single-file for now.** The plugin is a self-contained unit. Module splitting adds indirection without meaningful benefit until the file exceeds ~1500 lines. The Zellij plugin pattern (single State struct implementing ZellijPlugin) naturally centralizes logic.

## Build Order (Dependencies)

Features must be built in this order due to data and rendering dependencies:

```
Step 1: ProjectMetadata struct + cached_metadata on State
   |    (data foundation -- everything else depends on this)
   |
   +-- No rendering changes yet, just the data model
   |
Step 2: Timer + git branch polling
   |    (first external data source -- proves the polling pattern)
   |
   +-- subscribe to Timer + RunCommandResult
   +-- set_timeout in permission handler
   +-- run_command for git rev-parse
   +-- parse RunCommandResult, store in cached_metadata
   +-- Verify: eprintln! shows branch data arriving
   |
Step 3: RenderLine variants + multi-line card rendering
   |    (make the data visible -- requires Step 1 data to exist)
   |
   +-- Add ProjectDetail, ProjectPills to RenderLine
   +-- Update build_render_lines() for conditional sub-lines
   +-- render_detail_line() showing branch
   +-- Update ensure_selection_visible() for multi-line
   +-- Update mouse click handler for sub-line variants
   +-- Verify: cards show branch on second line
   |
Step 4: Pipe protocol for pills + progress
   |    (extend pipe handler -- requires Step 3 rendering to display)
   |
   +-- Add pill/progress/port pipe handlers
   +-- render_pills_line() showing pills and progress bar
   +-- Verify: zellij pipe commands update the sidebar
   |
Step 5: Port detection (pipe-based or lsof)
   |    (last because it's the most optional / complex)
   |
   +-- If pipe-based: done at Step 4 (port pipe already handled)
   +-- If lsof-based: add poll_ports(), parse lsof output
   +-- Display ports on ProjectDetail line
   |
Step 6: Polish
   +-- Card separator lines or spacing for visual clarity
   +-- Truncation handling for narrow sidebars
   +-- Progress bar color (green for high, yellow for mid, red for low)
   +-- Clear metadata when session stops
```

**Why this order:**
1. Data model first -- all features write to and read from ProjectMetadata
2. Git branch before rendering -- proves the polling pipeline works end-to-end before investing in UI changes
3. Multi-line rendering before pills -- the rendering infrastructure must exist before adding more data types to display
4. Pills before ports -- pills are simpler (pure pipe, no external commands) and validate the pipe protocol
5. Ports last -- highest complexity, most uncertain value, can be deferred

## Anti-Patterns to Avoid

### Anti-Pattern: Polling Every Project on Every Timer

**What:** Running `git rev-parse` for all 20+ discovered projects every 10 seconds.
**Why bad:** Each `run_command` is a subprocess spawn. 20 projects x 2 commands = 40 subprocesses per poll cycle.
**Instead:** Only poll running sessions. If 4 of 20 projects have active sessions, that is 4-8 commands per cycle -- reasonable.

### Anti-Pattern: Blocking Render on Missing Metadata

**What:** Waiting for git branch data before rendering a project card.
**Why bad:** `RunCommandResult` is async. Cards should render immediately with available data, then fill in metadata as it arrives.
**Instead:** Render cards with whatever metadata exists. Missing git branch = no branch line. Missing ports = no port display. Cards grow as data arrives.

### Anti-Pattern: Parsing PID from lsof to Attribute Ports

**What:** Running `lsof` output through PID -> session mapping to attribute ports to specific sessions.
**Why bad:** Zellij's PaneInfo does not expose PID. There's no reliable way to map a PID to a Zellij session from within the plugin sandbox.
**Instead:** Use pipe-based port reporting where the tool that opens the port tells the sidebar. Or display ports unattributed on the current session only.

### Anti-Pattern: Over-Complex Pipe Message Parsing

**What:** Using payload JSON for pill/progress messages.
**Why bad:** Adds a JSON parser dependency (serde_json). The plugin currently has only `zellij-tile` and `serde` as dependencies.
**Instead:** Use the pipe message `name` field for routing (with `::` separators) and `payload` for simple string values. This matches the existing attention protocol pattern.

## Key Decisions Summary

| Decision | Rationale |
|----------|-----------|
| `ProjectMetadata` as separate struct on `Project` | Clean separation, `Default` gives empty state, extensible |
| `cached_metadata` BTreeMap on State | Survives `rebuild_projects()`, same pattern as `cached_statuses` |
| RenderLine sub-variants (not multi-row ProjectRow) | Explicit screen row mapping, mouse click resolution, scroll math |
| `set_timeout` + `Timer` for polling (not spawning threads) | Only mechanism available in WASM sandbox, matches API design |
| 10-second poll interval | Git branches / ports rarely change faster; avoids subprocess spam |
| Pipe-based ports over lsof scanning | No PID attribution possible, pipe follows existing attention pattern |
| `::` separators in pipe names (not JSON payload) | Zero new dependencies, matches existing `sidebar::attention::` pattern |
| Stay single-file | ~1000 lines is manageable; split at ~1500 if needed |
| `run_command_with_env_variables_and_cwd` for git | Sets cwd to project path, avoids `-C` flag, cleaner API usage |

## Sources

- zellij-tile 0.43.1 source (`~/.cargo/registry/src/`) -- HIGH confidence, verified locally
- `Text` struct API: `color_range`, `color_all`, `selected`, `opaque`, `color_indices`, `color_substring` -- HIGH confidence, read from source
- `set_timeout(f64)` -> `Timer(f64)` event -- HIGH confidence, verified in zellij-tile shim.rs
- `run_command(&[&str], BTreeMap)` -> `RunCommandResult(Option<i32>, Vec<u8>, Vec<u8>, BTreeMap)` -- HIGH confidence, verified in source
- `run_command_with_env_variables_and_cwd(&[&str], BTreeMap, PathBuf, BTreeMap)` -- HIGH confidence, verified in source
- [Plugin API Commands](https://zellij.dev/documentation/plugin-api-commands.html) -- HIGH confidence, official
- [Plugin API Events](https://zellij.dev/documentation/plugin-api-events.html) -- HIGH confidence, official
- [Plugin Pipes](https://zellij.dev/documentation/plugin-pipes.html) -- HIGH confidence, official
- [Plugin UI Rendering](https://zellij.dev/documentation/plugin-ui-rendering.html) -- HIGH confidence, official
- Existing `src/main.rs` codebase (~600 lines, working plugin) -- HIGH confidence, primary source
