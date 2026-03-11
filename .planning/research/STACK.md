# Technology Stack

**Project:** Zellij Project Sidebar
**Researched:** 2026-03-11

## Recommended Stack

### Core Framework

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Rust (stable) | 1.88+ | Plugin language | Only supported language for Zellij WASM plugins. User has 1.88.0 installed. | HIGH |
| zellij-tile | 0.43.1 | Plugin API crate | Official Rust SDK for Zellij plugins. Must match installed Zellij version (user has 0.43.1). Provides `ZellijPlugin` trait, event system, rendering primitives, session/tab/pane management commands. | HIGH |
| wasm32-wasip1 | (target) | Compilation target | Required WASM target for Zellij plugins. Note: the old name `wasm32-wasi` was removed from stable Rust in 1.84 (Jan 2025). Must use `wasm32-wasip1` now. User needs to run `rustup target add wasm32-wasip1`. | HIGH |

### Supporting Libraries

| Library | Version | Purpose | When to Use | Confidence |
|---------|---------|---------|-------------|------------|
| serde | ^1.0, features=["derive"] | Config deserialization | Parsing plugin configuration from KDL block attributes into Rust structs. Already a transitive dep of zellij-tile but needed explicitly for derive macros. | HIGH |
| serde_json | ^1.0 | JSON serialization | Already a transitive dep of zellij-tile. Useful if storing/reading structured config. Only add explicitly if needed for custom serialization. | MEDIUM |

### Build Tooling

| Tool | Version | Purpose | Why | Confidence |
|------|---------|---------|-----|------------|
| cargo | 1.88+ | Build system | Standard Rust build. No special build tool needed -- just `cargo build --release` with the wasm32-wasip1 target configured in `.cargo/config.toml`. | HIGH |
| rustup | latest | Toolchain management | Need it to install `wasm32-wasip1` target: `rustup target add wasm32-wasip1` | HIGH |

### Development Environment

| Tool | Purpose | Why |
|------|---------|-----|
| zellij dev layout (zellij.kdl) | Hot-reload during development | Official pattern: a KDL layout file that opens editor panes + a build pane running `cargo build && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm`. Ctrl+Shift+R triggers rebuild via `develop-rust-plugin`. |
| watchexec (optional) | File watcher for auto-rebuild | Alternative to manual Ctrl+Shift+R. Watches `src/` and triggers `cargo build && zellij action start-or-reload-plugin`. Nice but not essential. |

## Critical Version Alignment

**zellij-tile version MUST match the installed Zellij version.**

The user has Zellij 0.43.1 installed. The zellij-tile crate is at 0.43.1 on crates.io. This is the version to use. Using an older version (e.g., 0.41.1 from templates) will miss API features and may cause protobuf deserialization issues.

zjstatus (the most actively maintained community plugin) uses zellij-tile 0.43.1 with Rust edition 2024, confirming this is the current standard.

## Project Configuration Files

### `.cargo/config.toml`

```toml
[build]
target = "wasm32-wasip1"
```

Sets the default compilation target so every `cargo build` produces WASM without needing `--target` flag.

### `Cargo.toml`

```toml
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

**Edition rationale:** Use 2021 (not 2024). While zjstatus uses 2024, it has a more complex dependency graph. Edition 2021 is the safest choice for a new wasm32-wasip1 project -- fully stable, no edge cases with WASM target. The 2024 edition works but offers no meaningful benefit for this project's scope.

**Release profile rationale:** WASM binary size directly affects plugin load time. `opt-level = "s"` optimizes for size, `lto = true` enables link-time optimization across crate boundaries for smaller output, `strip = "debuginfo"` removes debug symbols from release builds. This is standard practice for all Zellij community plugins.

### `src/main.rs` (skeleton)

```rust
use zellij_tile::prelude::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct State {
    // Plugin state goes here
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // Subscribe to events, request permissions
    }

    fn update(&mut self, event: Event) -> bool {
        // Handle events, return true to trigger re-render
        false
    }

    fn render(&mut self, rows: usize, cols: usize) {
        // Character-cell rendering with Text, Table, NestedList
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        // Handle piped messages from keybinds/CLI
        false
    }
}
```

### `zellij.kdl` (development layout)

```kdl
layout {
    pane split_direction="vertical" {
        pane edit="src/main.rs"
        pane split_direction="horizontal" {
            pane edit="Cargo.toml"
            pane command="bash" {
                args "-c" "cargo build && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm"
            }
        }
    }
    pane size=30 {
        plugin location="file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm"
    }
}
```

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Plugin crate | zellij-tile 0.43.1 | zellij-tile-extra | Extra is a community crate with minimal adoption. zellij-tile is the official SDK. |
| Build target | wasm32-wasip1 | wasm32-wasi | Removed from Rust stable in 1.84 (Jan 2025). Will not compile. |
| Build target | wasm32-wasip1 | wasm32-wasip2 | Zellij uses wasip1. wasip2 is not supported by the Zellij runtime. |
| Rust edition | 2021 | 2024 | 2024 works but adds no value for this scope. 2021 is battle-tested for wasm32-wasip1. |
| Rust edition | 2021 | 2018 | Old templates (zellij-org/rust-plugin-example) use 2018. No reason to use an outdated edition. |
| Serialization | serde (built-in dep) | Manual parsing | zellij-tile already depends on serde. Fighting it is pointless. |
| Hot reload | zellij dev layout | cargo-watch + separate terminal | Dev layout is self-contained inside Zellij. External watchers add unnecessary complexity. |
| Template | Manual setup | cargo-generate + rust-plugin-template | Template uses outdated zellij-tile 1.0.0 (a placeholder version). Manual setup with correct 0.43.1 version is more reliable. |
| Template | Manual setup | create-rust-plugin (zellij plugin) | Useful for learning but generates scaffolding we will customize immediately. Not worth the indirection. |
| Extra deps | None | kdl crate (6.5.0) | zjstatus uses this for parsing its complex config format. Our config is simple key-value from KDL block attributes -- the `BTreeMap<String, String>` from `load()` is sufficient. |
| Extra deps | None | chrono, regex, uuid | zjstatus dependencies for its statusbar features. We have no time/regex/ID needs. Keep deps minimal for WASM size. |

## What NOT to Use

| Technology | Why Not |
|------------|---------|
| `wasm32-wasi` target | Removed from Rust stable. Must use `wasm32-wasip1`. |
| `wasm32-wasip2` target | Zellij does not support wasip2 runtime. Plugin will fail to load. |
| `wasm-bindgen` / `serde-wasm-bindgen` | These are for browser WASM. Zellij uses WASI, not browser APIs. |
| `cargo-wasi` subcommand | Deprecated in favor of standard `cargo build` with target in `.cargo/config.toml`. |
| `proxy-wasm` | For Envoy/Istio proxies, not terminal multiplexer plugins. |
| Any async runtime (tokio, async-std) | WASM plugins are single-threaded event-driven. Use Zellij's `ZellijWorker` trait for background tasks and `set_timeout()` for timers. |
| `zellij-tile-utils` | Minimal crate, not widely adopted. Standard lib utilities suffice. |
| Mouse handling crates | Project is keyboard-first (v1). Zellij provides `Mouse` event directly. |

## Plugin API Surface (What We Need)

### Permissions Required

| Permission | Why | API Functions Unlocked |
|------------|-----|----------------------|
| `ReadApplicationState` | Read session list, tab info, pane info, active session | `SessionUpdate`, `TabUpdate`, `PaneUpdate` events |
| `ChangeApplicationState` | Switch sessions, kill sessions, create sessions | `switch_session()`, `switch_session_with_cwd()`, `kill_sessions()` |

### Events to Subscribe

| Event | Data Type | Use Case |
|-------|-----------|----------|
| `SessionUpdate` | `Vec<SessionInfo>, Vec<(String, Duration)>` | Live session list with tab/pane counts, active session indicator |
| `Key` | `KeyWithModifier` | Keyboard navigation (j/k, Enter, x, Esc) |
| `PaneUpdate` | `PaneManifest` | Active pane command display (what's running in focused pane) |

### Key API Functions

| Function | Purpose |
|----------|---------|
| `switch_session(name)` | Switch to existing session |
| `switch_session_with_cwd(name, cwd)` | Create + switch to session with working directory |
| `kill_sessions(names)` | Kill session from sidebar |
| `hide_self()` | Toggle sidebar hidden |
| `show_self()` | Toggle sidebar visible |
| `request_permission(&[...])` | Request ReadApplicationState + ChangeApplicationState on load |
| `subscribe(&[...])` | Subscribe to SessionUpdate, Key, PaneUpdate events |

### SessionInfo Fields (from `SessionUpdate` event)

| Field | Type | Use |
|-------|------|-----|
| `name` | `String` | Session name display |
| `tabs` | `Vec<TabInfo>` | Tab count per session |
| `panes` | `PaneManifest` | Pane info including active command |
| `is_current_session` | `bool` | Highlight active session |
| `connected_clients` | `usize` | Show if other clients are connected |

### Rendering Primitives

| Component | Use Case |
|-----------|----------|
| `Text::new()` with `.color_range()` and `.selected()` | Session name with status indicator, highlight selected row |
| `print_text_with_coordinates(text, x, y, w, h)` | Position elements in the sidebar |
| `NestedList` with `.indent()` and `.selected()` | Session list with optional tab sub-items |
| `Table` with `.add_row()` | Structured display if needed (likely overkill for sidebar) |

Color indices 0-3 map to theme colors automatically (Catppuccin Frappe will apply correctly).

## Setup Commands

```bash
# 1. Add WASM target (one-time)
rustup target add wasm32-wasip1

# 2. Build (debug, for development)
cargo build

# 3. Build (release, for distribution)
cargo build --release

# 4. Load into running Zellij session (debug)
zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm

# 5. Load into running Zellij session (release)
zellij action start-or-reload-plugin file:target/wasm32-wasip1/release/zellij-project-sidebar.wasm

# 6. Development with hot reload (use dev layout)
zellij -l zellij.kdl
```

## Sources

- [Zellij Rust Plugin Tutorial](https://zellij.dev/tutorials/developing-a-rust-plugin/) - Official tutorial, verified 2026-03-11
- [zellij-tile 0.43.1 docs.rs](https://docs.rs/zellij-tile/latest/zellij_tile/) - API reference
- [zellij-tile shim (commands)](https://docs.rs/zellij-tile/latest/zellij_tile/shim/index.html) - All plugin commands
- [Plugin API Events](https://zellij.dev/documentation/plugin-api-events.html) - Event types
- [Plugin API Commands](https://zellij.dev/documentation/plugin-api-commands.html) - Command reference
- [Plugin API Permissions](https://zellij.dev/documentation/plugin-api-permissions) - Permission types
- [Plugin UI Rendering](https://zellij.dev/documentation/plugin-ui-rendering.html) - Text, Table, NestedList, Ribbon
- [Plugin Loading](https://zellij.dev/documentation/plugin-loading) - URL schemas, layout loading
- [Creating Layouts](https://zellij.dev/documentation/creating-a-layout.html) - KDL layout syntax, pane sizing
- [Plugin Dev Environment](https://zellij.dev/documentation/plugin-dev-env.html) - Hot reload setup
- [rust-plugin-example](https://github.com/zellij-org/rust-plugin-example) - Official example, Cargo.toml reference
- [zjstatus Cargo.toml](https://github.com/dj95/zjstatus) - Community plugin using zellij-tile 0.43.1, edition 2024
- [zellij-sessionizer](https://github.com/cunialino/zellij-sessionizer) - Session-switching plugin, minimal deps
- [Rust WASI target rename](https://blog.rust-lang.org/2024/04/09/updates-to-rusts-wasi-targets/) - wasm32-wasi to wasm32-wasip1
- [SessionInfo struct](https://docs.rs/zellij-tile/latest/zellij_tile/prelude/struct.SessionInfo.html) - Session data fields
