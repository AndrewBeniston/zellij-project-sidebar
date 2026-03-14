# Domain Pitfalls: v1.1 Rich Cards

**Domain:** Adding multi-line card rendering, periodic command polling, and pipe-based metadata to an existing Zellij WASM sidebar plugin
**Researched:** 2026-03-14
**Milestone:** v1.1 Rich Cards
**Confidence:** HIGH (code analysis of existing plugin + verified API docs)

**Context:** The existing plugin is ~600 lines in `src/main.rs`. It uses 1-line-per-project rendering with `RenderLine::ProjectRow(usize)` mapped to screen y-coordinates. Mouse clicks use `click_y - y_offset + scroll_offset` to map screen rows to `render_lines` indices. Scroll moves `selected_index` by 1. This entire model breaks when projects occupy multiple lines.

---

## Critical Pitfalls

Mistakes that cause rewrites or major issues.

### Pitfall 1: Multi-Line Cards Break Mouse Click Y-Mapping

**What goes wrong:** The current mouse handler assumes 1 render line = 1 project. `Mouse::LeftClick(line, _col)` gives a screen y-coordinate, which is mapped to a project via `render_idx = scroll_offset + (click_y - y_offset)`, then `render_lines[render_idx]` gives the `ProjectRow(project_idx)`. When projects become 2-3 lines tall, clicking on the second or third line of a card either selects the wrong project or indexes out of bounds.

**Why it happens:** The current code (line 761):
```rust
let render_idx = self.scroll_offset + (click_y - y_offset);
if render_idx < render_lines.len() {
    if let RenderLine::ProjectRow(project_idx) = render_lines[render_idx] {
```
This treats every screen row as a distinct entry in `render_lines`. With multi-line cards, a click on y=3 might be the second line of project 1, not project 3.

**Consequences:** Wrong project activates on click. Users learn to distrust mouse interaction. Worst case: out-of-bounds panic crashes the plugin.

**Prevention:** Replace the flat `Vec<RenderLine>` with a structure that maps screen rows to project indices. Two approaches:

1. **Screen-row map (recommended):** Build a `Vec<Option<usize>>` of length `rows` where each screen y-coordinate maps to the project index that occupies that row. Clicking on any line of a 3-line card resolves to the same project index.
```rust
// Build during render
let mut y_to_project: Vec<Option<usize>> = vec![None; rows];
let mut screen_y = y_offset;
for (fi, &project_idx) in filtered.iter().enumerate() {
    let card_height = self.card_height_for(project_idx);
    for line_in_card in 0..card_height {
        if screen_y < rows {
            y_to_project[screen_y] = Some(fi); // fi = filtered index
        }
        screen_y += 1;
    }
}
// In mouse handler:
if let Some(fi) = y_to_project.get(click_y).copied().flatten() {
    self.selected_index = fi;
    self.activate_selected_project();
}
```

2. **Card boundary list:** Store `Vec<(usize, usize)>` of (start_y, end_y) per filtered project. Binary search for click_y. More complex but avoids allocating rows-length vec.

**Detection:** Click on the second line of any multi-line card. If it activates the wrong project or panics, the mapping is broken.

**Phase relevance:** Must be solved in the same plan that introduces multi-line cards. Cannot be deferred.

---

### Pitfall 2: Multi-Line Cards Break Scroll Offset Arithmetic

**What goes wrong:** The current scroll logic assumes each item is 1 row tall. `scroll_offset` is an index into `render_lines`, and `ensure_selection_visible` compares a single y-position against `scroll_offset + content_area`. With multi-line cards, scrolling by 1 moves past one card, which might be 3 rows -- so the "visible window" calculation is wrong. The selected card may be partially visible (cut off mid-card) or invisible despite being "within range."

**Why it happens:** The current code (line 449):
```rust
fn ensure_selection_visible(&mut self, render_lines: &[RenderLine], visible_rows: usize) {
    let selected_y = render_lines.iter().position(|line| { ... });
    if y >= self.scroll_offset + visible_rows {
        self.scroll_offset = y - visible_rows + 1;
    }
}
```
`scroll_offset` is an index into `render_lines` (where 1 entry = 1 row). With multi-line cards, `render_lines` and "screen rows" are no longer 1:1.

**Consequences:** Partial card rendering (card cut in half at top or bottom of viewport). Selection appears to jump erratically. Scrolling feels broken.

**Prevention:** Change `scroll_offset` from a render-line index to a pixel-row (screen-row) offset. Calculate cumulative heights:
```rust
fn y_position_of_card(&self, filtered_index: usize) -> usize {
    // Sum heights of all cards before this one
    let filtered = self.filtered_indices();
    filtered[..filtered_index].iter()
        .map(|&pi| self.card_height(pi))
        .sum()
}

fn ensure_selection_visible(&mut self, content_rows: usize) {
    let sel_y = self.y_position_of_card(self.selected_index);
    let sel_height = self.card_height_for_selected();
    // Scroll up if card starts above viewport
    if sel_y < self.scroll_offset {
        self.scroll_offset = sel_y;
    }
    // Scroll down if card ends below viewport
    if sel_y + sel_height > self.scroll_offset + content_rows {
        self.scroll_offset = sel_y + sel_height - content_rows;
    }
}
```

**Detection:** Navigate to the last project with Down key. If the card is only partially visible or the display shows blank rows, scroll math is wrong.

**Phase relevance:** Must be solved alongside multi-line card rendering. The scroll model change is foundational.

---

### Pitfall 3: run_command Results Arrive Out of Order With No Delivery Guarantee

**What goes wrong:** The plugin fires `run_command` for git branch detection on 15 projects simultaneously. Results come back as `RunCommandResult` events, but not in the order they were sent. If you track "which project this result belongs to" by position or sequence, you'll assign the wrong git branch to the wrong project. Worse: if a command hangs or fails, no result arrives at all, and the plugin waits forever.

**Why it happens:** `run_command` executes on the host in a background job. Multiple commands run concurrently. There is no ordering guarantee. The only correlation mechanism is the `context: BTreeMap<String, String>` parameter, which is returned verbatim with the result.

**Consequences:** Git branch `main` shows up on the wrong project. Port `3000` attributed to the wrong session. Silent data corruption that looks correct at first glance.

**Prevention:** Always use the context dictionary to tag every command:
```rust
let mut ctx = BTreeMap::new();
ctx.insert("cmd".to_string(), "git_branch".to_string());
ctx.insert("project".to_string(), project_name.clone());
run_command(
    &["git", "-C", &project_path, "branch", "--show-current"],
    ctx,
);
```
On `RunCommandResult`, match BOTH the command type AND the project name:
```rust
Event::RunCommandResult(exit_code, stdout, _stderr, context) => {
    let cmd = context.get("cmd").map(|s| s.as_str());
    let project = context.get("project").map(|s| s.as_str());
    match (cmd, project) {
        (Some("git_branch"), Some(name)) => {
            if exit_code == Some(0) {
                let branch = String::from_utf8_lossy(&stdout).trim().to_string();
                self.project_metadata.entry(name.to_string())
                    .or_default().git_branch = Some(branch);
            }
        }
        // ... other command types
    }
}
```
Never rely on arrival order. Never assume a result will arrive.

**Detection:** Add 15+ projects and poll git branches. If branches appear on wrong projects, context correlation is missing. If some projects never show branches, timeout/failure handling is missing.

**Phase relevance:** First plan that introduces `run_command` for git/port polling. Core to the data pipeline.

---

### Pitfall 4: Periodic Polling Without Backpressure Floods the Event Queue

**What goes wrong:** Using `set_timeout` to trigger `run_command` every N seconds for each project creates a storm of concurrent commands. With 20 projects and 2 command types (git branch + ports), that is 40 commands every poll cycle. If the poll interval is 5 seconds and commands take 2+ seconds to complete, results from cycle N overlap with commands from cycle N+1. The event queue grows unbounded.

**Why it happens:** `set_timeout` fires a `Timer` event after the specified delay. The plugin then fires 40 `run_command` calls. These run concurrently on the host. Meanwhile, the next `set_timeout` fires before all results have returned. There is no built-in backpressure -- the plugin has no way to know "all pending commands have completed."

**Consequences:** Memory usage grows as pending events accumulate. Results from stale poll cycles overwrite fresher data. In extreme cases, the plugin or Zellij becomes unresponsive.

**Prevention:**
1. **Gate on completion:** Track a `poll_in_flight: bool` flag. Set it when polling starts, clear it when ALL results for that cycle have arrived. Do not start a new cycle while `poll_in_flight` is true.
```rust
struct State {
    poll_in_flight: bool,
    pending_commands: usize, // count of outstanding run_command calls
    // ...
}
```
2. **Stagger commands:** Do not fire all 40 commands at once. Poll git branches on one timer cycle, ports on the next. Or poll only the visible projects.
3. **Use reasonable intervals:** 10-30 seconds for git branch (rarely changes). 15-30 seconds for ports (changes on server restart). Never poll more frequently than 5 seconds.
4. **Re-arm timer after results:** Instead of `set_timeout` at the end of the Timer handler, set it after processing the last result:
```rust
if self.pending_commands == 0 {
    self.poll_in_flight = false;
    set_timeout(self.poll_interval_secs);
}
```

**Detection:** Open Zellij logs (`eprintln!` output). If you see commands being dispatched while previous results are still arriving, backpressure is missing.

**Phase relevance:** Polling architecture must be designed correctly from the start. Retrofitting backpressure is painful.

---

### Pitfall 5: Pipe Message Protocol Becomes Unparseable Without Versioning

**What goes wrong:** The plugin defines a pipe protocol like `sidebar::pill::session_name` with the payload as the pill value. Later, you need to add pill color, priority, or expiry. The protocol has no versioning, no structured format, no way to extend without breaking existing senders.

**Why it happens:** Pipe messages have `name: String` and `payload: Option<String>`. It is tempting to encode everything in the name using `::` delimiters (the existing attention system does this: `sidebar::attention::session_name`). This works for simple boolean signals but fails for structured data.

**Consequences:** Every protocol extension breaks existing callers. You end up with `sidebar::pill::v2::session_name` hacks. External tools (Claude Code hooks, custom scripts) that send pipe messages break on upgrade.

**Prevention:** Use the `args` dictionary for structured data. Keep the `name` as a simple action identifier. Put all metadata in args:
```bash
# CLI sender:
zellij pipe --plugin "file:sidebar.wasm" \
  --name "sidebar::metadata" \
  --args "session=my-project,type=pill,key=status,value=building,color=yellow"
```
```rust
// Plugin receiver:
fn pipe(&mut self, msg: PipeMessage) -> bool {
    match msg.name.as_str() {
        "sidebar::metadata" => {
            let args = msg.args; // BTreeMap<String, String>
            let session = args.get("session");
            let msg_type = args.get("type");
            // Structured, extensible, backward-compatible
        }
    }
}
```
This is extensible: new keys can be added to args without changing the protocol name. Old senders that don't include new keys still work because you provide defaults.

**Detection:** If you find yourself parsing `name.split("::")` into 4+ segments, or adding version numbers to pipe names, the protocol design is wrong.

**Phase relevance:** Design the pipe protocol BEFORE implementing pills/progress. Changing it after external tools depend on it is a breaking change.

---

## Moderate Pitfalls

### Pitfall 6: color_range Indices Are Character-Based, Not Byte-Based

**What goes wrong:** The existing code uses `color_range(COLOR_GREEN, 1..2)` to color a single character (the status dot). This works because the dot is a single-byte ASCII character at position 1. With multi-line cards containing Unicode symbols (branch icon, port icon, progress bar characters like `[=====>   ]`), the indices become character positions in the string. If you calculate ranges using `.len()` (byte length) instead of `.chars().count()` (character count), the color bleeds into wrong characters or misses entirely.

**Why it happens:** The existing code correctly uses `.chars().count()` in some places (line 503, 521) but the pattern is easy to break when adding new rendering for git branches and port numbers. Multi-byte Unicode characters (branch symbol, pill emoji, progress bar block characters) make byte-length and char-length diverge.

**Consequences:** Colors appear on wrong characters. A branch name like `feature/add-login` might have its color offset by 2-3 characters due to a preceding Unicode icon.

**Prevention:** Establish a rendering helper that always computes char-based ranges:
```rust
fn color_range_for(text: &str, start_char: usize, end_char: usize) -> std::ops::Range<usize> {
    start_char..end_char
}
```
Never use `.len()` for display width calculations. Always use `.chars().count()`. Consider creating a `StyledLine` builder that tracks character positions as you append segments.

**Detection:** Add a Unicode icon (like a git branch symbol) before colored text. If the color is offset by the icon's byte count minus 1, you have a byte-vs-char bug.

**Phase relevance:** Every rendering change in v1.1. Must be consistent from the first multi-line card render.

---

### Pitfall 7: Card Height Varies By State, Breaking Layout Assumptions

**What goes wrong:** A project with no git branch, no ports, and no pills renders as 1 line. A project with all metadata renders as 3 lines. The total content height changes dynamically as git/port data arrives asynchronously. This means:
- Scroll offset becomes invalid when card heights change
- Selection highlight jumps when a card above the selected one gains/loses a line
- The footer position shifts unpredictably

**Why it happens:** Card height depends on which metadata is available, which arrives asynchronously via `RunCommandResult`. The layout is recalculated on every render, but scroll state is sticky.

**Consequences:** Visual jank when data loads. Selection appears to jump. Footer "bounces" as cards expand. Partial card visibility at viewport edges.

**Prevention:**
1. **Fixed card height per verbosity mode:** In `Full` mode, always allocate 3 lines per card regardless of whether metadata has loaded. Show placeholder text ("---" or empty line) for missing data. This makes layout predictable.
2. **Recalculate scroll on data change:** When `RunCommandResult` updates metadata that changes a card's height, recalculate `scroll_offset` to keep the selected card in view. Call `ensure_selection_visible` after any metadata update.
3. **Pin footer to `rows - 1` absolutely:** Never compute footer position relative to content height. Always use `rows.saturating_sub(1)`.

**Detection:** Watch the sidebar as git branches load one by one after startup. If the display jitters or the selected card moves, heights are inconsistent.

**Phase relevance:** Card layout design. Decide fixed vs. variable height before implementing rendering.

---

### Pitfall 8: lsof Port Detection Is Slow and Permission-Dependent on macOS

**What goes wrong:** `lsof -nP -iTCP -sTCP:LISTEN` can take 1-3 seconds on macOS, especially without `-n` (DNS resolution). With 20 projects polling every 15 seconds, that is 20-60 seconds of lsof execution per cycle, blocking run_command slots.

**Why it happens:** `lsof` on macOS scans all file descriptors system-wide, not per-process. The `-n` flag avoids DNS lookups but lsof is still inherently slow because it queries the kernel for every open file descriptor. Additionally, ports below 1024 require `sudo`, which the WASM plugin cannot provide.

**Consequences:** Polling cycles overlap (see Pitfall 4). Port data is stale by the time it arrives. Plugin feels sluggish.

**Prevention:**
1. **Run lsof ONCE per poll cycle, not per project.** Parse the output to attribute ports to sessions by matching process trees or working directories.
```rust
// One command for all ports:
run_command(&["lsof", "-nP", "-iTCP", "-sTCP:LISTEN"], ctx);
// Parse output to find which session owns which port
```
2. **Use longer intervals:** 30 seconds minimum for port polling. Port changes are rare events (server restart).
3. **Cache aggressively:** Only update if the output differs from the previous poll.
4. **Consider alternatives:** On macOS, `netstat -an -p tcp` may be faster than lsof. On Linux, `ss -tlnp` is significantly faster. Detect the OS and use the fastest tool.

**Detection:** Time the lsof command in isolation: `time lsof -nP -iTCP -sTCP:LISTEN`. If it takes >1 second, it will cause backpressure issues at scale.

**Phase relevance:** Port detection implementation. Must benchmark before committing to lsof as the detection mechanism.

---

### Pitfall 9: run_command Cannot Determine Which Session Owns a Port

**What goes wrong:** `lsof` shows that port 3000 is open, but there is no reliable way to map that port to a specific Zellij session. Multiple sessions may have independent Node.js servers on different ports. The plugin needs to show "port 3000" on the correct project card, but `lsof` output shows the PID, not the Zellij session name.

**Why it happens:** Zellij sessions do not have a "session PID" or process group that neatly contains all child processes. A session's shell spawns processes that may outlive it. `lsof` output shows PIDs and command names, not Zellij session names or working directories. There is no API in `zellij-tile` to query a session's process tree.

**Consequences:** Ports are either shown globally (not per-project) or attributed to the wrong project. Users see misleading port information.

**Prevention:** Two viable approaches:
1. **Match by working directory:** Run `lsof -nP -iTCP -sTCP:LISTEN -F pcn` to get PID, command, and file name. Then for each PID, check `/proc/{pid}/cwd` (Linux) or `lsof -p {pid} -Fn` (macOS) to find the working directory. Match cwd against project paths.
2. **Show ports globally:** Instead of per-project ports, show a combined "listening ports" section. Simpler and honest about the limitation.
3. **Pipe-based ports:** Let projects self-report their ports via pipe messages (like the attention system). This is the most reliable approach -- the process knows its own port.

**Detection:** Start two projects both running servers. If ports appear on the wrong project or both ports appear on both projects, attribution is broken.

**Phase relevance:** Port detection design phase. Decide the attribution strategy before implementation.

---

### Pitfall 10: Timer Events Cannot Identify Which Timer Fired

**What goes wrong:** You set two timers: `set_timeout(5.0)` for git polling and `set_timeout(15.0)` for port polling. The `Timer` event has no identifier -- it is just `Event::Timer(f64)` where the f64 is the elapsed time. Both timers fire the same event type. You cannot distinguish which timer expired.

**Why it happens:** `set_timeout` takes only a duration, not an ID. The `Timer` event returns only the elapsed seconds. Multiple `set_timeout` calls create multiple `Timer` events, but they are indistinguishable.

**Consequences:** If you use separate intervals for git and port polling, both trigger the same handler. You end up polling both on every timer, defeating the purpose of separate intervals.

**Prevention:** Use a SINGLE timer with a unified poll cycle. Track what to poll using internal state:
```rust
struct State {
    poll_tick: usize,
    git_interval: usize,  // e.g., 2 = every 2 ticks
    port_interval: usize, // e.g., 6 = every 6 ticks
}

// In Timer handler:
Event::Timer(_) => {
    self.poll_tick += 1;
    if self.poll_tick % self.git_interval == 0 {
        self.poll_git_branches();
    }
    if self.poll_tick % self.port_interval == 0 {
        self.poll_ports();
    }
    set_timeout(self.base_interval_secs); // re-arm
    true
}
```
This gives you fine-grained control with a single timer source.

**Detection:** If git and port data refresh at the same rate despite different configured intervals, the timer demultiplexing is wrong.

**Phase relevance:** Polling architecture design. Must be decided before implementing any periodic commands.

---

### Pitfall 11: Render Thrashing Multiplied by Polling Results

**What goes wrong:** With 20 projects and 2 command types, each poll cycle generates 40 `RunCommandResult` events. Each event updates metadata and returns `true` from `update()`, triggering 40 re-renders in rapid succession. Combined with `SessionUpdate` events, the sidebar re-renders 50+ times per poll cycle.

**Why it happens:** The existing pattern returns `true` from `update()` on every `RunCommandResult`. Each re-render is a full repaint (Zellij clears the plugin terminal on every render). 40 full repaints in 1-2 seconds causes visible flicker.

**Consequences:** Sidebar flickers during poll cycles. CPU usage spikes. Terminal feels sluggish.

**Prevention:**
1. **Batch updates:** Do not return `true` immediately on each `RunCommandResult`. Instead, update internal state and check if the displayed data actually changed:
```rust
Event::RunCommandResult(exit, stdout, _, ctx) => {
    let old_branch = self.metadata.get(project).and_then(|m| m.git_branch.as_ref());
    let new_branch = /* parse stdout */;
    if old_branch != Some(&new_branch) {
        self.metadata.entry(project).or_default().git_branch = Some(new_branch);
        true // Only re-render if data changed
    } else {
        false
    }
}
```
2. **Coalesce timer renders:** If you need all results to render at once, track `pending_commands` and only trigger render when it reaches 0.

**Detection:** Add `eprintln!("render called")` to `render()` and count calls during a poll cycle. If >5 renders occur within 2 seconds, batching is needed.

**Phase relevance:** Performance optimization. Should be addressed in the polling implementation plan, not deferred.

---

### Pitfall 12: Progress Bar Rendering With Block Characters Is Fragile

**What goes wrong:** Rendering a progress bar like `[======>   ]` using Unicode block characters (e.g., `\u{2588}` full block, `\u{2591}` light shade) breaks `color_range` calculations because these are multi-byte UTF-8 characters. Additionally, `print_text_with_coordinates` treats each character cell as 1 column, but some block characters may render as 2 columns wide depending on the terminal's Unicode support.

**Why it happens:** Block characters like `\u{2588}` are 3 bytes in UTF-8 but 1 character and 1 column wide in most terminals. If you calculate color ranges using string slicing or byte offsets, they will be wrong. If the terminal treats certain block characters as wide (ambiguous width characters), the progress bar will overflow the available columns.

**Consequences:** Progress bar color bleeds into adjacent text. Bar overflows the column width. Visual corruption on some terminals.

**Prevention:**
1. **Use ASCII-only progress bars:** `[====>     ]` or `[####.......]`. No Unicode width ambiguity. `color_range` works predictably with char indices.
2. **Keep progress bar on its own line:** Do not mix progress bar characters with normal text on the same line. This isolates color_range calculations.
3. **Calculate with `.chars().count()`:** Never use `.len()` for progress bar width.
4. **Test with the actual terminal:** Ghostty (the user's terminal) has good Unicode support, but verify block characters render as expected.

**Detection:** Set a progress bar to 50% and check if the colored portion exactly fills half the bar. If it overflows or underflows, width calculation is wrong.

**Phase relevance:** Progress bar rendering. Design the bar format before implementing.

---

## Minor Pitfalls

### Pitfall 13: git branch --show-current Fails on Detached HEAD

**What goes wrong:** `git branch --show-current` returns an empty string when HEAD is detached (common during rebase, bisect, or CI checkouts). The plugin shows an empty branch name or crashes on unwrap.

**Prevention:** Handle empty output explicitly:
```rust
let branch = String::from_utf8_lossy(&stdout).trim().to_string();
let display_branch = if branch.is_empty() {
    // Detached HEAD -- try rev-parse for short SHA
    // Or just show "detached"
    "detached".to_string()
} else {
    branch
};
```
Consider also running `git rev-parse --short HEAD` as a fallback when `--show-current` returns empty.

**Phase relevance:** Git branch display implementation.

---

### Pitfall 14: run_command Paths Must Be Absolute From WASM Sandbox

**What goes wrong:** `run_command(&["git", "-C", "~/projects/foo", "branch", "--show-current"], ctx)` fails because `~` is not expanded in the WASM sandbox. Similarly, relative paths are relative to the WASM sandbox's working directory, not the user's home.

**Prevention:** All paths passed to `run_command` must be absolute. The project paths in `self.projects[i].path` should already be absolute (validated in `load()`). Use them directly:
```rust
run_command(&["git", "-C", &project.path, "branch", "--show-current"], ctx);
```
The existing code already validates this in `load()` with the tilde warning, but it is worth re-emphasizing for every `run_command` call.

**Phase relevance:** Every run_command implementation. Use `&project.path` directly.

---

### Pitfall 15: Pipe Message Senders Must Know the Plugin's Plugin ID

**What goes wrong:** The existing attention system works because senders use `sidebar::attention::session_name` as the pipe name, which broadcasts to all plugins. But if pill/progress senders need to target THIS specific plugin instance (to avoid duplicate processing in secondary sidebar instances), they need the plugin ID, which changes on every load.

**Prevention:** Two approaches:
1. **Continue using broadcast names:** Accept that all sidebar instances (primary + secondary in new tabs) receive the same pipe messages. Filter in the `pipe()` handler based on `is_primary`.
2. **Use plugin URL targeting:** Senders can target by plugin URL instead of plugin ID. The URL is stable: `file:~/.config/zellij/plugins/zellij-project-sidebar.wasm`.

The existing attention system uses broadcast (approach 1) successfully. Pill/progress messages should follow the same pattern for consistency.

**Phase relevance:** Pipe protocol design. Decide routing strategy before implementing.

---

### Pitfall 16: Metadata State Not Cleared When Session Ends

**What goes wrong:** Project "my-app" shows git branch `main` and port `3000`. User kills the session. The project moves to "Exited" status, but the git branch and port metadata persist from the last poll. The card shows stale branch/port info for a dead session.

**Prevention:** Clear project metadata when session status changes to `Exited` or `NotStarted`:
```rust
Event::SessionUpdate(sessions, resurrectable) => {
    // ... update statuses ...
    // Clear metadata for sessions that are no longer running
    for project in &self.projects {
        if matches!(project.status, SessionStatus::Exited | SessionStatus::NotStarted) {
            self.project_metadata.remove(&project.name);
        }
    }
}
```

**Detection:** Kill a session that had git branch and port data displayed. If the metadata persists after kill, cleanup is missing.

**Phase relevance:** Metadata lifecycle management. Add cleanup whenever session status transitions.

---

### Pitfall 17: Card Separator Lines Consume Scarce Vertical Space

**What goes wrong:** Adding visual separators between cards (e.g., `---` or blank lines) seems clean but consumes precious vertical space. With a 40-row terminal and 3-line cards with 1-line separators, only 10 projects fit on screen. Without separators, ~13 projects fit. In a narrow sidebar, every row counts.

**Prevention:** Use visual differentiation through color/indentation rather than separator lines:
- Alternating subtle background shading (if theme supports it)
- Indenting metadata lines (line 2-3) under the project name
- Using `selected()` highlight as the primary visual separator
- Adding a blank line only between cards when there are few projects (<8)

**Detection:** Add 15+ projects and check how many are visible without scrolling. If fewer than 10, separators are too expensive.

**Phase relevance:** Card layout design. Make this decision before implementing multi-line rendering.

---

### Pitfall 18: Subscribing to Timer Events Without set_timeout Causes Silent Failure

**What goes wrong:** The plugin subscribes to `EventType::Timer` in `load()` but forgets to call `set_timeout()` to actually start the timer. No timer events ever fire. No error is raised. The plugin simply never polls.

**Prevention:** After subscribing to Timer events and receiving permission, call `set_timeout()` to start the polling cycle:
```rust
Event::PermissionRequestResult(PermissionStatus::Granted) => {
    // ... existing setup ...
    subscribe(&[EventType::Timer, EventType::RunCommandResult]);
    set_timeout(1.0); // Start first poll after 1 second
}
```
Note: The subscription to `Timer` and `RunCommandResult` events should be added to the existing `subscribe` call in `load()`, and the initial `set_timeout` should happen after permissions are granted.

**Detection:** If git branches and ports never appear despite correct `run_command` code, check that `set_timeout` is being called.

**Phase relevance:** Polling setup. Easy to miss, easy to fix, but confusing to debug.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Multi-line card rendering | Mouse y-mapping breaks (Pitfall 1) | Build screen-row-to-project map |
| Multi-line card rendering | Scroll offset arithmetic breaks (Pitfall 2) | Change scroll_offset to screen-row based |
| Multi-line card rendering | Variable card height jank (Pitfall 7) | Use fixed height per verbosity mode |
| Multi-line card rendering | Vertical space wasted on separators (Pitfall 17) | Use indentation not separators |
| Git branch polling | run_command results out of order (Pitfall 3) | Always use context dict for correlation |
| Git branch polling | Detached HEAD returns empty (Pitfall 13) | Handle empty string, show "detached" |
| Git branch polling | Paths must be absolute (Pitfall 14) | Use project.path directly |
| Port detection | lsof is slow on macOS (Pitfall 8) | Run once per cycle, not per project |
| Port detection | Cannot attribute ports to sessions (Pitfall 9) | Match by cwd or use pipe self-reporting |
| Periodic polling | Backpressure/flooding (Pitfall 4) | Gate on completion, stagger commands |
| Periodic polling | Timer events indistinguishable (Pitfall 10) | Single timer with tick counter |
| Periodic polling | Render thrashing from batch results (Pitfall 11) | Only re-render on actual data change |
| Periodic polling | Forgot to call set_timeout (Pitfall 18) | Call after permissions granted |
| Pipe protocol | Protocol not extensible (Pitfall 5) | Use args dict, not name encoding |
| Pipe protocol | Plugin ID routing for secondary instances (Pitfall 15) | Use broadcast + is_primary filter |
| Pipe protocol | Stale metadata after session kill (Pitfall 16) | Clear metadata on status transition |
| Rendering | color_range byte vs char mismatch (Pitfall 6) | Always use .chars().count() |
| Rendering | Progress bar Unicode fragility (Pitfall 12) | Use ASCII bars, isolate on own line |

## Sources

**HIGH confidence (official docs + verified API):**
- [Plugin API Commands](https://zellij.dev/documentation/plugin-api-commands.html) -- run_command, set_timeout signatures and behavior
- [Plugin API Events](https://zellij.dev/documentation/plugin-api-events) -- RunCommandResult format (exit_code, stdout, stderr, context), Timer event
- [Plugin Pipes](https://zellij.dev/documentation/plugin-pipes) -- PipeMessage struct (name, payload, args, source, is_private)
- [Plugin UI Rendering](https://zellij.dev/documentation/plugin-ui-rendering) -- print_text_with_coordinates, color_range, full-repaint model
- [Plugin Communication (DeepWiki)](https://deepwiki.com/zellij-org/zellij/3.4-plugin-communication) -- run_command background execution, event routing
- [Plugin System (DeepWiki)](https://deepwiki.com/zellij-org/zellij/4-cli-and-commands) -- plugin thread assignment (plugin_id % num_threads), atomic event IDs

**MEDIUM confidence (verified community sources):**
- [zjstatus command widget](https://github.com/dj95/zjstatus/wiki/4-%E2%80%90-Widgets) -- run_command interval polling pattern, command_git_branch_interval
- [Zellij Plugin Dev Guide (dasroot.net)](https://dasroot.net/posts/2026/03/developing-plugins-for-zellij-comprehensive-guide/) -- context-based command correlation
- [lsof macOS performance (Simon Willison)](https://til.simonwillison.net/macos/lsof-macos) -- lsof -nP flags for speed
- [Mouse LeftClick(isize, usize)](https://github.com/zellij-org/zellij/blob/main/zellij-tile/src/data.rs) -- Mouse enum definition

**LOW confidence (code analysis, needs runtime verification):**
- color_range character vs byte indexing behavior -- verified by pattern in existing code, but no official doc explicitly states "character-based"
- Timer event f64 value meaning -- observed to be elapsed seconds but not documented precisely
- lsof output format stability across macOS versions -- tested on current macOS but format could change
