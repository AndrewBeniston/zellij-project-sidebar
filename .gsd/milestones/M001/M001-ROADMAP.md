# M001: v1.0

**Vision:** One-keypress project switching with always-visible session awareness — a persistent docked sidebar for Zellij that shows pinned project folders with live session status.

## Success Criteria


## Slices

- [x] **S01: Scaffold Lifecycle** `risk:medium` `depends:[]`
  > After this: Create the Rust WASM plugin scaffold that compiles, loads in Zellij, requests permissions, subscribes to session events, and logs received data.
- [x] **S02: Toggle + Layout** `risk:medium` `depends:[S01]`
  > After this: Cmd+P (Super p) toggles sidebar visibility from any context via pipe. hide_self()/show_self() cycle works, space reclaimed on hide and restored on show. Keybind registered via reconfigure. Sidebar renders as tiled left pane with fixed width.
- [x] **S03: Toggle + Enrichment + Theme** `risk:medium` `depends:[S02]`
  > After this: Cmd+P (Super p) toggles sidebar visibility — hide_self()/show_self() cycle with space reclaim. Running sessions show tab count, current session shows active pane command, info verbosity is configurable (minimal/full via KDL config), and all colours match Catppuccin Frappe with semantic status indicators (green=running, dim=stopped, yellow=exited).
