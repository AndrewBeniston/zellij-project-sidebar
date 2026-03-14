# Technology Stack: v1.1 Rich Cards

**Project:** Zellij Project Sidebar
**Milestone:** v1.1 Rich Cards (git branch, ports, pills, progress bars)
**Researched:** 2026-03-14

## Scope

This document covers ONLY the stack additions/changes needed for v1.1 features. The existing stack (Rust, zellij-tile 0.43.1, wasm32-wasip1, serde) is validated and unchanged.

## Executive Summary

**No new crate dependencies needed.** All v1.1 features are achievable with the existing `zellij-tile` API surface and Rust stdlib. The key unlocks are:

1. `set_timeout()` + `Event::Timer` for periodic polling (git branch, ports)
2. `run_command_with_env_variables_and_cwd()` for running git/lsof in project directories
3. Existing pipe message system for pills and progress bars
4. Existing `Text` + `color_range` + `print_text_with_coordinates` for multi-line card rendering

## New API Surface Required

### Timer System (periodic polling)

| API | Signature | Purpose | Confidence |
|-----|-----------|---------|------------|
| `set_timeout(secs: f64)` | `fn set_timeout(secs: f64)` | Schedule a future `Event::Timer` callback | HIGH |
| `Event::Timer(f64)` | enum variant | Fires when timer expires, carries the timeout value | HIGH |
| `EventType::Timer` | subscribe target | Must subscribe to receive Timer events | HIGH |

**Verified:** Read directly from `zellij-tile-0.43.1/src/shim.rs` line 305 and `zellij-utils-0.43.1/src/data.rs` line 891.

**Pattern** (set-timeout-rearm loop):

```rust
// In load():
subscribe(&[EventType::Timer, /* ...existing events... */]);
set_timeout(1.0); // Initial delay before first poll

// In update():
Event::Timer(_elapsed) => {
    self.poll_git_branches();
    self.poll_listening_ports();
    set_timeout(10.0); // Rearm for next poll cycle
    true // trigger re-render
}
```

The timer is one-shot. Calling `set_timeout` again inside the Timer handler creates a recurring poll loop. zjstatus uses this exact pattern with a 10-second default interval for git branch polling.

**No permission required** for `set_timeout`. It is a basic plugin command, not gated by any `PermissionType`.

### run_command with CWD (git branch per project)

| API | Signature | Purpose | Confidence |
|-----|-----------|---------|------------|
| `run_command_with_env_variables_and_cwd` | `fn run_command_with_env_variables_and_cwd(cmd: &[&str], env: BTreeMap<String, String>, cwd: PathBuf, ctx: BTreeMap<String, String>)` | Run command with a specific working directory | HIGH |

**Verified:** Read directly from `zellij-tile-0.43.1/src/shim.rs` lines 337-352.

This is critical because the plugin needs to run `git rev-parse --abbrev-ref HEAD` in each project's directory, not the plugin's own CWD. The basic `run_command()` hardcodes CWD to `PathBuf::from(".")` which is the plugin's directory.

**Already have** `PermissionType::RunCommands` requested in discovery mode. For non-discovery mode, this permission must be added.

## Feature-by-Feature Stack Analysis

### 1. Git Branch Detection

**Command:** `git rev-parse --abbrev-ref HEAD`
**Why this command:** Fast (no network), works in bare repos, returns clean branch name. zjstatus uses the same command. Takes <5ms.

**Edge cases verified locally:**
| State | Output | Handling |
|-------|--------|----------|
| Normal branch | `main`, `feature/foo` | Display as-is |
| Detached HEAD | `HEAD` (literal string) | Show short SHA instead via `git rev-parse --short HEAD` |
| Not a git repo | Exit code 128, stderr error | Show nothing (no branch indicator) |
| Bare repo | Works normally | N/A for project sidebar |

**Implementation approach:**

```rust
// For each project with a known path, fire off a run_command per poll cycle:
fn poll_git_branches(&self) {
    for (idx, project) in self.projects.iter().enumerate() {
        if project.path.is_empty() {
            continue;
        }
        let mut ctx = BTreeMap::new();
        ctx.insert("cmd".to_string(), "git_branch".to_string());
        ctx.insert("project_idx".to_string(), idx.to_string());
        run_command_with_env_variables_and_cwd(
            &["git", "rev-parse", "--abbrev-ref", "HEAD"],
            BTreeMap::new(),
            PathBuf::from(&project.path),
            ctx,
        );
    }
}
```

**Result handling** in `Event::RunCommandResult`: match on `context["cmd"] == "git_branch"`, parse `project_idx`, store branch name in project state. If stdout is `"HEAD\n"`, optionally run a follow-up `git rev-parse --short HEAD` for the short SHA, or just display a detached indicator.

**Polling interval:** 10 seconds (matches zjstatus default). Git branch changes are infrequent enough that 10s is perfectly responsive without wasting cycles.

**No new crates needed.** `String::from_utf8_lossy(&stdout).trim().to_string()` parses the output.

### 2. Listening Port Detection

**Command:** `lsof -nP -iTCP -sTCP:LISTEN`
**Platform:** macOS only (the user's setup is Ghostty + Zellij on macOS).

**Why lsof, not ss:** `ss` is Linux-only. macOS has `lsof` and `netstat`. `lsof -nP -iTCP -sTCP:LISTEN` is the standard macOS approach. Verified locally: runs in ~69ms with `-n` (skip DNS) and `-P` (numeric ports).

**Flag breakdown:**
| Flag | Purpose |
|------|---------|
| `-n` | No DNS resolution (faster) |
| `-P` | Show port numbers, not service names |
| `-iTCP` | Only TCP connections |
| `-sTCP:LISTEN` | Only LISTEN state (servers, not clients) |

**Output format:**
```
COMMAND   PID  USER  FD  TYPE  DEVICE  SIZE/OFF  NODE  NAME
node      1234 user  23u IPv4  0x...   0t0       TCP   *:3000 (LISTEN)
```

**Parsing approach:** No regex crate needed. Simple line-by-line parsing:

```rust
// Parse lsof output: extract port numbers from NAME column
fn parse_lsof_output(stdout: &[u8], project_name: &str) -> Vec<u16> {
    let output = String::from_utf8_lossy(stdout);
    output.lines()
        .skip(1) // skip header
        .filter_map(|line| {
            // NAME column is last, format: "*:3000 (LISTEN)" or "127.0.0.1:8080 (LISTEN)"
            let name = line.rsplit_whitespace().nth(1)?; // second-to-last field
            let port_str = name.rsplit(':').next()?;
            port_str.parse::<u16>().ok()
        })
        .collect()
}
```

**Challenge: Mapping ports to projects.** lsof shows system-wide ports. We need to filter to processes owned by each Zellij session. Two approaches:

| Approach | How | Complexity | Accuracy |
|----------|-----|------------|----------|
| **A: Session CWD matching** | Run `lsof` once globally, then match process CWDs against project paths using `/proc` (Linux) or additional lsof calls | HIGH | MEDIUM |
| **B: Single global list** | Run `lsof` once, show ALL listening ports grouped as a global info section (not per-project) | LOW | LOW (but useful) |
| **C: Per-project lsof with CWD** | Run `lsof +D /project/path` per project | HIGH (slow) | LOW (doesn't catch processes that cd away) |
| **D: Skip per-project, show session-level** | Only detect ports for the currently active session by inspecting process tree | MEDIUM | MEDIUM |

**Recommendation: Approach B (global port list) for v1.1, with a view toward Approach A in a future version.** Per-project port mapping is genuinely hard because processes don't necessarily stay in their initial CWD. A global "listening ports" indicator keeps scope manageable.

However, an even better approach: **skip automatic port detection for v1.1 and use pipe messages instead.** Just as attention is signaled via pipes, a dev tool or shell hook can signal ports:

```bash
# Shell hook (in .zshrc or project-specific):
zellij pipe "sidebar::ports::my-project" -- "3000,8080"
```

This is more reliable, more extensible, and zero CPU cost vs polling lsof every 10 seconds. It also avoids platform-specific commands.

**Pragmatic recommendation:** Implement BOTH. Auto-detect via lsof as a convenience (show ports in a global section or skip), and support pipe-based port reporting for accuracy. Start with pipe-based in v1.1, add lsof auto-detect as an enhancement.

**Polling interval:** If using lsof, 15-30 seconds is fine. Port changes are rare events.

### 3. Status Pills via Pipe Messages

**No new API needed.** The existing pipe system handles this perfectly. The plugin already processes pipe messages with prefix-based routing (`sidebar::attention::`, `sidebar::clear::`).

**Protocol extension:**

```
# Set a pill (key-value metadata displayed on card)
zellij pipe "sidebar::pill::SESSION_NAME::KEY" -- "VALUE"

# Clear a pill
zellij pipe "sidebar::pill::SESSION_NAME::KEY" -- ""

# Examples:
zellij pipe "sidebar::pill::my-project::env" -- "staging"
zellij pipe "sidebar::pill::my-project::status" -- "deploying"
zellij pipe "sidebar::pill::my-project::ports" -- "3000,8080"
```

**Data model addition:**

```rust
struct Project {
    name: String,
    path: String,
    status: SessionStatus,
    // v1.1 additions:
    git_branch: Option<String>,
    pills: BTreeMap<String, String>,  // key -> value
    progress: Option<u8>,             // 0-100
}
```

**Rendering:** Pills are small colored badges. Use `color_range` with different color indices per pill type. Convention: use the pipe message key to determine color (e.g., "env" = blue, "status" = yellow).

**No new crates.** String splitting on `::` separators is trivial with stdlib.

### 4. Progress Bar via Pipe Messages

**Protocol:**

```
# Set progress (0-100)
zellij pipe "sidebar::progress::SESSION_NAME" -- "75"

# Clear progress
zellij pipe "sidebar::progress::SESSION_NAME" -- ""
```

**Rendering:** Character-cell progress bar using block characters:

```rust
fn render_progress_bar(progress: u8, width: usize) -> String {
    let filled = (progress as usize * width) / 100;
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}
```

Width is constrained by sidebar column count. A typical 20-char sidebar gives ~16 chars for the bar (after padding). Use `color_range` to color the filled portion green and empty portion gray.

**No new crates.** Unicode block characters are stdlib strings.

### 5. Multi-Line Card Layout

**No new API.** Uses existing `print_text_with_coordinates(text, x, y, Some(cols), None)` with different y-offsets per line of each card.

**Card layout (3 lines per project at full verbosity):**

```
Line 1: ● project-name [3]           ← name + status dot + tab count
Line 2:   main  :3000 :8080          ← branch + ports
Line 3:   [env:staging] [deploying]   ← pills
```

**At minimal verbosity:** Single line (current behavior).
**At full verbosity:** 2-3 lines depending on available metadata.

The `RenderLine` enum needs extension:

```rust
enum RenderLine {
    Header(String),
    ProjectRow(usize),
    // v1.1 additions:
    ProjectDetail(usize),    // branch + ports line
    ProjectPills(usize),     // pills + progress line
}
```

## What NOT to Add

| Crate | Why Not |
|-------|---------|
| `regex` | All parsing is simple enough for `str::split`, `str::rsplit`, `str::parse`. Adding regex bloats WASM binary (~100KB+). |
| `chrono` | No time formatting needed. `set_timeout` uses `f64` seconds. |
| `git2` / `libgit2-sys` | Cannot compile to wasm32-wasip1. Uses C FFI. Shell out to `git` CLI instead. |
| `nix` / `libc` (for port detection) | WASM sandbox has no direct syscall access. Shell out to `lsof` instead. |
| `tokio` / `async-std` | WASM plugins are single-threaded. Use `set_timeout` + `run_command` (async by design). |
| `serde_json` | Pills/progress use simple string values, not JSON. Pipe message payload is already a string. |

## Permissions Update

v1.1 requires `RunCommands` permission for ALL modes (not just discovery mode). Currently, it's only requested in discovery mode.

**Change in `load()`:**

```rust
// Before (v1.0):
let mut permissions = vec![
    PermissionType::ReadApplicationState,
    PermissionType::ChangeApplicationState,
    PermissionType::Reconfigure,
];
if self.use_discovery {
    permissions.push(PermissionType::RunCommands);
}

// After (v1.1):
let permissions = vec![
    PermissionType::ReadApplicationState,
    PermissionType::ChangeApplicationState,
    PermissionType::Reconfigure,
    PermissionType::RunCommands, // Always needed now for git/port detection
];
```

**Impact:** Users in legacy (non-discovery) mode will see the permissions dialog again on first load after upgrade. This is a one-time prompt.

## Event Subscription Update

v1.1 requires subscribing to `EventType::Timer` and `EventType::RunCommandResult` in ALL modes.

**Change in `load()`:**

```rust
// Before (v1.0):
let mut events = vec![
    EventType::SessionUpdate,
    EventType::PermissionRequestResult,
    EventType::Key,
    EventType::Mouse,
];
if self.use_discovery {
    events.push(EventType::RunCommandResult);
}

// After (v1.1):
let events = vec![
    EventType::SessionUpdate,
    EventType::PermissionRequestResult,
    EventType::Key,
    EventType::Mouse,
    EventType::RunCommandResult, // Always needed for git/port commands
    EventType::Timer,            // For periodic polling
];
```

## Configuration Additions

New KDL config keys for v1.1:

| Key | Type | Default | Purpose |
|-----|------|---------|---------|
| `poll_interval` | String (seconds) | `"10"` | How often to poll git branches and ports |
| `show_git_branch` | String ("true"/"false") | `"true"` | Enable/disable git branch display |
| `show_ports` | String ("true"/"false") | `"false"` | Enable/disable auto port detection (lsof) |

**Note:** All KDL plugin config values are strings. Parse in `load()` with `.parse::<f64>()` for the interval.

## Timer Architecture

### Poll Scheduling

```
load() -> permissions granted -> set_timeout(1.0) [initial delay]
         |
         v
Timer fires -> poll_git_branches() [fire N run_commands]
            -> poll_ports() [fire 1 run_command if enabled]
            -> set_timeout(poll_interval) [rearm]
         |
         v
RunCommandResult -> update project state -> return true (re-render)
```

**Key insight:** `run_command` is asynchronous. Firing 20 git commands at once is fine -- they run on the host concurrently and results arrive as individual `RunCommandResult` events. The context map (`BTreeMap<String, String>`) tracks which command belongs to which project.

### Command Multiplexing

The existing `CMD_KEY`/`CMD_SCAN_DIR` pattern extends naturally:

```rust
const CMD_SCAN_DIR: &str = "scan_dir";
const CMD_GIT_BRANCH: &str = "git_branch";
const CMD_PORTS: &str = "ports";

// In RunCommandResult handler:
match context.get("cmd").map(|s| s.as_str()) {
    Some("scan_dir") => { /* existing */ }
    Some("git_branch") => { /* parse branch, store in project */ }
    Some("ports") => { /* parse lsof output, store ports */ }
    _ => false,
}
```

## Summary: Dependency Delta

| v1.0 Cargo.toml | v1.1 Cargo.toml | Change |
|------------------|------------------|--------|
| `zellij-tile = "0.43.1"` | `zellij-tile = "0.43.1"` | No change |
| `serde = { version = "1.0", features = ["derive"] }` | `serde = { version = "1.0", features = ["derive"] }` | No change |
| | | **Zero new dependencies** |

The entire v1.1 feature set ships with zero new crates. All capabilities come from:
- zellij-tile API: `set_timeout`, `run_command_with_env_variables_and_cwd`, `Event::Timer`
- Rust stdlib: `String` parsing, `BTreeMap`, `PathBuf`
- Existing patterns: pipe messages, `color_range` rendering, context-based command routing

## Sources

- [zellij-tile 0.43.1 shim.rs](https://docs.rs/zellij-tile/0.43.1/zellij_tile/shim/) - `set_timeout` (line 305), `run_command_with_env_variables_and_cwd` (line 337). Verified by reading source directly from `~/.cargo/registry/src`.
- [zellij-utils 0.43.1 data.rs](https://docs.rs/zellij-tile/0.43.1/zellij_tile/prelude/enum.Event.html) - `Event::Timer(f64)` (line 891), `EventType` discriminant. Verified by reading source directly.
- [Plugin API Commands](https://zellij.dev/documentation/plugin-api-commands) - `set_timeout` and `run_command` official documentation.
- [Plugin API Events](https://zellij.dev/documentation/plugin-api-events.html) - Timer and RunCommandResult event documentation.
- [zjstatus](https://github.com/dj95/zjstatus) - Community statusbar plugin using `git rev-parse --abbrev-ref HEAD` with 10-second interval. Validates the timer + run_command polling pattern.
- [lsof macOS usage](https://til.simonwillison.net/macos/lsof-macos) - `lsof -nP -iTCP -sTCP:LISTEN` for port detection on macOS. Verified locally: ~69ms execution time.
