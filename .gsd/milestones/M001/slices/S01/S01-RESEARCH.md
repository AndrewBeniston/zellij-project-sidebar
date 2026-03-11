# Phase 1: Scaffold + Lifecycle - Research

**Researched:** 2026-03-11
**Domain:** Rust WASM plugin scaffold for Zellij terminal multiplexer
**Confidence:** HIGH

## Summary

Phase 1 creates the foundational Rust project that compiles to a WASM plugin loadable in Zellij 0.43.1. The scope is narrow and well-understood: set up the cargo project, implement the `ZellijPlugin` trait with correct permissions and event subscriptions, verify the plugin loads and receives `SessionUpdate` events, and establish the development workflow with hot-reload.

The Zellij plugin API is stable and well-documented. The only meaningful risk is using outdated tooling -- the `wasm32-wasi` target was removed from Rust stable in 1.84 (January 2025) and must be replaced with `wasm32-wasip1`. The official `rust-plugin-example` repository has an outdated `Cargo.toml` (zellij-tile 0.41.1, edition 2018) but its `.cargo/config.toml` has been updated to `wasm32-wasip1`. Manual project setup with correct versions is more reliable than using the template.

The lifecycle flow is: `load()` requests permissions and subscribes to events, `update()` receives a `PermissionRequestResult` event confirming grants, then `SessionUpdate` events begin flowing with complete session data. The plugin logs to stderr via `eprintln!()` which Zellij routes to its log file (path discoverable via `zellij setup --check`).

**Primary recommendation:** Set up project manually (not from template) with zellij-tile 0.43.1, wasm32-wasip1 target, edition 2021. Request all permissions upfront in `load()`. Use `develop-rust-plugin` v0.3.0 for hot-reload during development.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INFR-01 | Plugin compiles to wasm32-wasip1 and loads in Zellij 0.43.1 | Cargo.toml with zellij-tile 0.43.1, .cargo/config.toml with wasm32-wasip1 target, register_plugin! macro, ZellijPlugin trait implementation |
| INFR-02 | Plugin requests and handles permissions correctly (first-launch UX) | request_permission() in load(), PermissionRequestResult event handling in update(), permission state tracking |
| INFR-03 | Plugin subscribes to SessionUpdate events for live data (no polling) | subscribe() in load() with EventType::SessionUpdate, Event::SessionUpdate handler in update() that logs SessionInfo data via eprintln!() |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust (stable) | 1.88+ | Plugin language | Only supported language for Zellij WASM plugins. User has 1.88.0. |
| zellij-tile | 0.43.1 | Plugin API crate | Official Rust SDK. Must match installed Zellij version (0.43.1). Provides ZellijPlugin trait, event system, commands. |
| wasm32-wasip1 | (target) | Compilation target | Required WASM target. Old name `wasm32-wasi` removed from Rust stable in 1.84. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde | ^1.0, features=["derive"] | Serialization | Transitive dep of zellij-tile but needed explicitly for derive macros in later phases. Add now to avoid Cargo.toml churn. |

### Development Tooling
| Tool | Version | Purpose |
|------|---------|---------|
| develop-rust-plugin | v0.3.0 | Hot-reload during development. Bind Ctrl+Shift+R to compile and reload plugin. Loaded as floating pane in dev layout. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Manual project setup | cargo-generate + rust-plugin-template | Template has outdated zellij-tile 0.41.1 and edition 2018. Manual is more reliable. |
| develop-rust-plugin | Manual `cargo build && zellij action start-or-reload-plugin` | Manual command works but develop-rust-plugin automates the cycle with a single keybind. |
| Edition 2021 | Edition 2024 | 2024 works (zjstatus uses it) but adds no value for this scope. 2021 is fully battle-tested for wasm32-wasip1. |

**Installation:**
```bash
# Add WASM target (one-time)
rustup target add wasm32-wasip1
```

## Architecture Patterns

### Project Structure
```
zellij-project-sidebar/
+-- .cargo/
|   +-- config.toml          # [build] target = "wasm32-wasip1"
+-- src/
|   +-- main.rs              # Plugin entry point with State struct
+-- zellij.kdl               # Development layout for hot-reload
+-- Cargo.toml               # zellij-tile 0.43.1, serde
```

### Pattern 1: Minimal ZellijPlugin Implementation
**What:** The `ZellijPlugin` trait requires four methods. Phase 1 implements `load()` with permissions/subscriptions, `update()` with event logging, `render()` as a minimal placeholder, and `pipe()` as a no-op stub.
**When to use:** Phase 1 -- establishing the skeleton.
**Example:**
```rust
// Source: https://zellij.dev/tutorials/developing-a-rust-plugin/
use zellij_tile::prelude::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct State {
    permissions_granted: bool,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::Reconfigure,
        ]);
        subscribe(&[
            EventType::SessionUpdate,
            EventType::PermissionRequestResult,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.permissions_granted = true;
                eprintln!("Permissions granted");
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                eprintln!("Permissions denied");
                false
            }
            Event::SessionUpdate(sessions, resurrectable) => {
                eprintln!("SessionUpdate: {} active, {} resurrectable",
                    sessions.len(), resurrectable.len());
                for session in &sessions {
                    eprintln!("  Session: {} (tabs: {}, current: {})",
                        session.name, session.tabs.len(), session.is_current_session);
                }
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, _rows: usize, _cols: usize) {
        if self.permissions_granted {
            println!("Project Sidebar (loading...)");
        } else {
            println!("Waiting for permissions...");
        }
    }

    fn pipe(&mut self, _pipe_message: PipeMessage) -> bool {
        false
    }
}
```

### Pattern 2: Permission State Machine
**What:** Track whether permissions have been granted before executing privileged operations. Events like `SessionUpdate` can arrive before `PermissionRequestResult` due to asynchronous delivery.
**When to use:** Always. Permission-dependent code must check `self.permissions_granted`.
**Example:**
```rust
// Source: https://zellij.dev/tutorials/developing-a-rust-plugin/
// "a plugin could receive certain events (like ModeUpdate) before
// the PermissionRequestResult event is received"
// "Permissions are cached by the plugin url... the user will not be
// prompted for permission if they have already accepted it. We will
// however always receive the PermissionRequestResult after plugin load,
// making it safe to leave logic there."

Event::PermissionRequestResult(PermissionStatus::Granted) => {
    self.permissions_granted = true;
    // Safe to now call privileged operations
    // e.g., reconfigure() for keybinds, switch_session(), etc.
}
```

### Pattern 3: Logging via eprintln!
**What:** All debug output goes to stderr, which Zellij routes to its log file. `println!` goes to the plugin's rendered output (stdout) and corrupts the display.
**When to use:** Always for debugging. Never use `println!` for debug output.
**Example:**
```rust
// Source: https://zellij.dev/tutorials/developing-a-rust-plugin/
// "By printing to STDERR with eprintln, we're telling Zellij to
// place these messages in its log file."
// Find log path: zellij setup --check

eprintln!("Plugin loaded, requesting permissions");
eprintln!("Received {} sessions", sessions.len());
```

### Anti-Patterns to Avoid
- **Using `println!` for logging:** Corrupts the rendered UI. Use `eprintln!` exclusively.
- **Not handling PermissionRequestResult:** Plugin appears to load but does nothing because privileged calls silently fail.
- **Using wasm32-wasi target:** Removed from Rust stable in 1.84. Build fails immediately.
- **Returning `true` from update() on every event:** Causes unnecessary re-renders. Return `true` only when state actually changed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WASM plugin scaffold | Custom build scripts, wasm-bindgen setup | `register_plugin!` macro + ZellijPlugin trait | The macro handles all WASM entry point setup. Manual FFI is wrong for Zellij. |
| Hot-reload workflow | File watchers, custom bash scripts | `develop-rust-plugin` v0.3.0 in dev layout | Official tool, single keybind (Ctrl+Shift+R), handles compile + reload. |
| Event subscription | Polling with set_timeout | `subscribe()` + event-driven `update()` | SessionUpdate events fire on every state change. Polling is wasteful and adds latency. |

**Key insight:** Zellij plugins are entirely event-driven. There is no main loop, no polling, no threads. The `register_plugin!` macro sets up the WASM entry points and the runtime calls your trait methods. Fighting this model breaks everything.

## Common Pitfalls

### Pitfall 1: wasm32-wasi Target Removed
**What goes wrong:** Build fails with "target not found" error.
**Why it happens:** Rust 1.84 (January 2025) removed `wasm32-wasi`, renaming it to `wasm32-wasip1`. Older docs and tutorials still reference the old name.
**How to avoid:** Use `wasm32-wasip1` in `.cargo/config.toml` and `rustup target add wasm32-wasip1`.
**Warning signs:** Build error mentioning "wasm32-wasi is not a valid target."

### Pitfall 2: Outdated zellij-tile Version
**What goes wrong:** API mismatches, missing features, protobuf deserialization errors at runtime.
**Why it happens:** Official rust-plugin-example uses zellij-tile 0.41.1 (outdated). Must use 0.43.1 to match installed Zellij.
**How to avoid:** Explicitly set `zellij-tile = "0.43.1"` in Cargo.toml. Do not use `"*"` or `"^0.41"`.
**Warning signs:** Runtime errors about protobuf decoding, missing Event variants.

### Pitfall 3: Permission Race Condition
**What goes wrong:** Plugin receives `SessionUpdate` events before `PermissionRequestResult`. Code that depends on permissions silently fails.
**Why it happens:** Zellij's event system is asynchronous. Official docs warn about this explicitly.
**How to avoid:** Track permission state in a boolean. Gate privileged operations on `self.permissions_granted`.
**Warning signs:** Plugin loads but keybinds don't work, session switching silently fails.

### Pitfall 4: Missing Permissions Not Requested Upfront
**What goes wrong:** Plugin requests `ReadApplicationState` but forgets `ChangeApplicationState` or `Reconfigure`. These are needed in later phases but permissions are cached by plugin URL -- the dialog only fires on first load.
**Why it happens:** Incremental development adds features requiring new permissions, but the user has already granted (cached) the old set.
**How to avoid:** Request ALL permissions needed across all phases in `load()`: `ReadApplicationState`, `ChangeApplicationState`, `Reconfigure`.
**Warning signs:** Feature works in dev (frequent reloads clear cache) but fails for installed plugins.

### Pitfall 5: println! Corrupts Plugin Display
**What goes wrong:** Debug output appears inside the plugin's rendered pane instead of going to logs.
**Why it happens:** `println!` writes to stdout, which is the plugin's render surface. `eprintln!` writes to stderr, which Zellij routes to its log file.
**How to avoid:** Use `eprintln!()` exclusively for logging. Find log path with `zellij setup --check`.
**Warning signs:** Random text appearing in the plugin pane.

## Code Examples

### Cargo.toml (verified configuration)
```toml
# Source: Verified against zellij-tile 0.43.1 on crates.io and rust-plugin-example
[package]
name = "zellij-project-sidebar"
version = "0.1.0"
edition = "2021"

[dependencies]
zellij-tile = "0.43.1"
serde = { version = "1.0", features = ["derive"] }

[profile.release]
opt-level = "s"
lto = true
strip = "debuginfo"
```

### .cargo/config.toml
```toml
# Source: https://github.com/zellij-org/rust-plugin-example/blob/main/.cargo/config.toml
[build]
target = "wasm32-wasip1"
```

### Development Layout (zellij.kdl)
```kdl
# Source: Adapted from https://github.com/zellij-org/rust-plugin-example/blob/main/zellij.kdl
layout {
    cwd "."
    pane size=1 borderless=true {
        plugin location="tab-bar"
    }
    pane split_direction="vertical" {
        pane edit="src/main.rs" size="60%"
        pane split_direction="horizontal" {
            pane edit="Cargo.toml"
            pane {
                plugin location="file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm"
            }
        }
    }
    pane size=1 borderless=true {
        plugin location="status-bar"
    }
    floating_panes {
        pane {
            plugin location="https://github.com/zellij-org/develop-rust-plugin/releases/download/v0.3.0/develop-rust-plugin.wasm"
        }
    }
}
```

### SessionUpdate Event Data (verified SessionInfo fields)
```rust
// Source: https://docs.rs/zellij-tile/latest/zellij_tile/prelude/struct.SessionInfo.html
// Event::SessionUpdate(Vec<SessionInfo>, Vec<(String, Duration)>)
//
// SessionInfo fields:
//   name: String                          -- Session name (e.g., "help-self")
//   tabs: Vec<TabInfo>                    -- Active tabs in session
//   panes: PaneManifest                   -- All panes (HashMap<usize, Vec<PaneInfo>>)
//   connected_clients: usize             -- Number of attached clients
//   is_current_session: bool             -- Whether this is the active session
//   available_layouts: Vec<LayoutInfo>    -- Layout configurations
//   plugins: BTreeMap<u32, PluginInfo>    -- Running plugins by ID
//   web_clients_allowed: bool            -- WebSocket permission flag
//   web_client_count: usize              -- Web client count
//   tab_history: BTreeMap<u16, Vec<usize>>  -- Navigation history
//
// The second parameter Vec<(String, Duration)> contains resurrectable sessions
// (name + time since exit).
```

### Permission Types Needed (all phases)
```rust
// Source: https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.PermissionType.html
// Request all upfront -- dialog only fires on first load, permissions cached by plugin URL.
request_permission(&[
    PermissionType::ReadApplicationState,    // SessionUpdate, TabUpdate, PaneUpdate events
    PermissionType::ChangeApplicationState,  // switch_session, kill_sessions, hide/show_self
    PermissionType::Reconfigure,             // Dynamic keybind setup via reconfigure()
]);
```

### Event Types for Phase 1 Subscription
```rust
// Source: https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.EventType.html
// Phase 1 subscribes to minimal set; later phases add more.
subscribe(&[
    EventType::SessionUpdate,            // Live session data
    EventType::PermissionRequestResult,  // Permission grant/deny
]);
```

### Build and Test Commands
```bash
# Build (debug, for development -- fast compile)
cargo build

# Build (release, for distribution -- optimized for size)
cargo build --release

# Load into running Zellij session (debug)
zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm

# Load into running Zellij session (release)
zellij action start-or-reload-plugin file:target/wasm32-wasip1/release/zellij-project-sidebar.wasm

# Development with hot reload (use dev layout)
zellij -l zellij.kdl

# Find Zellij log path (for verifying eprintln! output)
zellij setup --check

# Tail Zellij logs in another terminal/pane
tail -f /tmp/zellij-$(id -u)/zellij-log/zellij.log
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `wasm32-wasi` target | `wasm32-wasip1` target | Rust 1.84, Jan 2025 | Build fails with old target name |
| Manual `cargo build && zellij action start-or-reload-plugin` | `develop-rust-plugin` v0.3.0 floating pane | Jan 2025 | Single keybind (Ctrl+Shift+R) for compile + reload |
| zellij-tile 0.41.x | zellij-tile 0.43.1 | Aug 2025 | Matches latest Zellij release, new APIs for web server and multi-select |
| Edition 2018 in templates | Edition 2021 (or 2024) | Templates not updated | Official example still uses 2018; zjstatus uses 2024 |

**Deprecated/outdated:**
- `wasm32-wasi` target: Removed from Rust stable 1.84+. Use `wasm32-wasip1`.
- `cargo-wasi` subcommand: Deprecated. Use standard `cargo build` with target in `.cargo/config.toml`.
- `zellij-tile-utils`: Minimal community crate, not adopted. Standard lib suffices.
- Official `rust-plugin-example` Cargo.toml: Uses zellij-tile 0.41.1 and edition 2018. Do not copy verbatim.

## Open Questions

1. **Zellij log path format**
   - What we know: `zellij setup --check` reveals the log path. It is typically `/tmp/zellij-<uid>/zellij-log/zellij.log`.
   - What's unclear: The exact path format may vary by OS or Zellij configuration.
   - Recommendation: Run `zellij setup --check` during implementation to discover the actual path. Document it in verification steps.

2. **develop-rust-plugin version compatibility**
   - What we know: v0.3.0 released January 2025. Official example references it.
   - What's unclear: Whether it's been updated since, and whether it works correctly with wasm32-wasip1 target.
   - Recommendation: Use it from the official URL. If it fails, fall back to manual `cargo build && zellij action start-or-reload-plugin` command.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Manual verification (Zellij plugin -- no unit test framework for WASM plugins) |
| Config file | none -- WASM plugins are tested by loading into Zellij |
| Quick run command | `cargo build && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm` |
| Full suite command | `cargo build --release` (compilation success = structural correctness) |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| INFR-01 | WASM compiles and loads in Zellij | smoke | `cargo build --target wasm32-wasip1 2>&1; echo "exit: $?"` | N/A (cargo) |
| INFR-02 | Permission prompt appears on first load, plugin proceeds after grant | manual | Load plugin in Zellij, verify permission dialog, grant permissions, check eprintln log for "Permissions granted" | N/A |
| INFR-03 | SessionUpdate events received and logged | manual | Load plugin, check Zellij log file for "SessionUpdate: N active" messages | N/A |

### Sampling Rate
- **Per task commit:** `cargo build` (compilation check)
- **Per wave merge:** Load plugin in Zellij, verify permission flow and SessionUpdate logging
- **Phase gate:** All three success criteria verified manually in running Zellij

### Wave 0 Gaps
- [ ] Project structure: Cargo.toml, .cargo/config.toml, src/main.rs -- all must be created from scratch
- [ ] Development layout: zellij.kdl for hot-reload workflow
- [ ] Target installation: `rustup target add wasm32-wasip1`

*(No test files needed -- WASM plugins are verified by loading into the host runtime. Compilation success is the primary automated check.)*

## Sources

### Primary (HIGH confidence)
- [Zellij Rust Plugin Tutorial](https://zellij.dev/tutorials/developing-a-rust-plugin/) -- lifecycle, permissions, logging
- [zellij-tile 0.43.1 API docs](https://docs.rs/zellij-tile/latest/zellij_tile/) -- crate reference
- [Event enum](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.Event.html) -- SessionUpdate payload
- [EventType enum](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.EventType.html) -- subscribe() variants
- [PermissionType enum](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.PermissionType.html) -- 13 permission variants
- [PermissionStatus enum](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.PermissionStatus.html) -- Granted/Denied
- [SessionInfo struct](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/struct.SessionInfo.html) -- 10 fields verified
- [Plugin API Permissions](https://zellij.dev/documentation/plugin-api-permissions) -- permission descriptions
- [Plugin API Events](https://zellij.dev/documentation/plugin-api-events.html) -- event types and required permissions
- [Plugin Loading](https://zellij.dev/documentation/plugin-loading) -- file: schema, layout loading
- [Plugin Dev Environment](https://zellij.dev/documentation/plugin-dev-env.html) -- hot-reload setup
- [rust-plugin-example](https://github.com/zellij-org/rust-plugin-example) -- official template (.cargo/config.toml confirmed wasm32-wasip1)
- [develop-rust-plugin v0.3.0](https://github.com/zellij-org/develop-rust-plugin) -- hot-reload tool
- [Zellij releases](https://github.com/zellij-org/zellij/releases) -- v0.43.1 confirmed latest (Aug 8 2025)
- [Rust WASI target rename](https://blog.rust-lang.org/2024/04/09/updates-to-rusts-wasi-targets/) -- wasm32-wasi to wasm32-wasip1

### Secondary (MEDIUM confidence)
- [zellij-tile on crates.io](https://crates.io/crates/zellij-tile) -- version 0.43.1 confirmed available
- Prior domain research in `.planning/research/` -- STACK.md, ARCHITECTURE.md, PITFALLS.md

### Tertiary (LOW confidence)
- None -- all Phase 1 findings verified against official sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- only one option (Rust + zellij-tile), versions verified against crates.io and installed Zellij
- Architecture: HIGH -- follows official tutorial patterns, ZellijPlugin trait is mandatory, event system is the only model
- Pitfalls: HIGH -- wasm32-wasi removal verified against Rust release notes, permission race documented in official tutorial, eprintln logging documented officially

**Research date:** 2026-03-11
**Valid until:** 2026-04-11 (stable domain, Zellij 0.43.1 is latest, no breaking changes expected)