# S01: Scaffold Lifecycle

**Goal:** Create the Rust WASM plugin scaffold that compiles, loads in Zellij, requests permissions, subscribes to session events, and logs received data.
**Demo:** Create the Rust WASM plugin scaffold that compiles, loads in Zellij, requests permissions, subscribes to session events, and logs received data.

## Must-Haves


## Tasks

- [x] **T01: 01-scaffold-lifecycle 01** `est:5min`
  - Create the Rust WASM plugin scaffold that compiles, loads in Zellij, requests permissions, subscribes to session events, and logs received data.

Purpose: Establish the foundation every subsequent phase builds on -- a working plugin binary with correct permissions and live session data flowing in.
Output: Compiling Rust project with ZellijPlugin implementation + development layout for hot-reload workflow.

## Files Likely Touched

- `Cargo.toml`
- `.cargo/config.toml`
- `src/main.rs`
- `zellij.kdl`
