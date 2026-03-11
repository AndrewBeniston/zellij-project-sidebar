# T01: 01-scaffold-lifecycle 01

**Slice:** S01 — **Milestone:** M001

## Description

Create the Rust WASM plugin scaffold that compiles, loads in Zellij, requests permissions, subscribes to session events, and logs received data.

Purpose: Establish the foundation every subsequent phase builds on -- a working plugin binary with correct permissions and live session data flowing in.
Output: Compiling Rust project with ZellijPlugin implementation + development layout for hot-reload workflow.

## Must-Haves

- [ ] "cargo build produces a .wasm file at target/wasm32-wasip1/debug/zellij-project-sidebar.wasm"
- [ ] "Plugin loads in Zellij 0.43.1 without runtime errors"
- [ ] "First load presents a permission prompt listing ReadApplicationState, ChangeApplicationState, Reconfigure"
- [ ] "After granting permissions, plugin logs 'Permissions granted' to Zellij log file"
- [ ] "Plugin receives SessionUpdate events and logs session count + names to Zellij log file"

## Files

- `Cargo.toml`
- `.cargo/config.toml`
- `src/main.rs`
- `zellij.kdl`
