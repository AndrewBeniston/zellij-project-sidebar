# Feature Landscape

**Domain:** Zellij session management plugin (sidebar-style project switcher)
**Researched:** 2026-03-11

## Ecosystem Context

The Zellij session management plugin space is fragmented across 8+ community plugins and one built-in session manager. Every single existing solution uses **floating popups** -- there is no persistent sidebar session manager in the ecosystem. This is the core gap the project fills.

Existing plugins fall into three categories:
1. **Directory scanners** (zellij-sessionizer, zsm) -- find folders, create sessions from them
2. **Session navigators** (zellij-choose-tree, built-in session-manager, zellij-switch) -- list/switch active sessions
3. **Favourites managers** (zellij-favs) -- pin sessions, batch manage

None combine **pinned project awareness** + **live session status** + **persistent visibility**. The closest analogue outside Zellij is Speedmux (a Go/libghostty multiplexer with a persistent sidebar tracking session state), but that is an entirely separate terminal multiplexer, not a plugin.

---

## Table Stakes

Features users expect from any session switching tool. Missing these and users just stick with the built-in session manager or zellij-sessionizer.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Session list display | Every session tool shows active sessions. Bare minimum. | Low | Subscribe to `SessionUpdate` event |
| Switch to session on selection | The entire point -- Enter to jump. Built-in does this. | Low | `switch_session()` API command |
| Visual indicator for current session | Users need to know where they are. Every tool does this. | Low | Compare session name from `ModeUpdate` |
| Keyboard navigation (j/k, arrows) | Zellij is keyboard-first. Every plugin uses vim-style nav. | Low | Standard event handling |
| Session status indicators | Running vs exited vs not started. Minimum useful info. | Low | `SessionUpdate` provides `is_active` state |
| Create session from folder | If a pinned project has no session, Enter should create one. | Medium | `switch_session()` with cwd creates if missing |
| Kill/delete session | zellij-choose-tree has `x` to delete, built-in has it, users expect it. | Low | `kill_sessions()` API command |
| Configurable project list | Explicit pinned folders via KDL config. This IS the value prop. | Medium | Parse plugin config block in KDL |

## Differentiators

Features that set this plugin apart. Not expected because nothing in the ecosystem does them, but they define the product.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Persistent docked sidebar** | THE differentiator. No existing plugin is persistent -- all are floating popups that require invocation and dismiss after selection. Strider proves the docked sidebar pattern works in Zellij (it uses layout-based sizing at 15-20% width). This plugin brings that same pattern to session management. | Medium | Layout integration, not floating. Must handle resize events. |
| **Toggle visibility (keybind)** | Floating popups are all-or-nothing. A toggle lets users show the sidebar when they want ambient awareness and hide it when screen real estate matters. No session plugin offers this. | Medium | `hide_self()` / `show_self()` or pane manipulation. Need to verify API support. |
| **Live tab count per session** | "help-self [3]" -- at-a-glance complexity indicator. No existing plugin shows tab counts alongside session names. Built-in session-manager shows tabs only when you expand into a session. | Low | `SessionUpdate` event includes tab info |
| **Active pane command display** | Shows what is running in the focused pane of each session (e.g., "nvim", "npm run dev"). No session plugin surfaces this. The `PaneUpdate` event provides command info. | Medium | Requires cross-session pane data. May only be available for current session -- needs API verification. |
| **Configurable info verbosity** | Minimal (name + dot), standard (name + tab count), full (tabs + active command). Different workflows need different density. No plugin offers this. | Low | Config flag controlling render detail level |
| **Pinned project ordering** | Projects appear in config-defined order, not alphabetical or creation-time. User's mental model is preserved. Sessionizer sorts by search results; choose-tree sorts arbitrarily. | Low | Iterate config entries in order |
| **Catppuccin Frappe theming** | Visually native to the user's Zellij setup. Most community plugins use default terminal colours with no theme awareness. | Low | Hardcoded colour values matching Frappe palette |

## Anti-Features

Features to explicitly NOT build. These are tempting but wrong for this plugin's identity.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Fuzzy search / directory scanning | zellij-sessionizer already does this well. Duplicating it dilutes the plugin's identity. This plugin is for **known** projects, not discovery. | Pinned explicit folder paths in config. User adds projects manually. |
| Session tree (expand tabs/panes) | zellij-choose-tree already provides hierarchical session/tab/pane navigation. Reimplementing it adds complexity with no differentiation. | Show tab **count** per session, not tab tree. Link to choose-tree for deep navigation. |
| Session resurrection UI | Built-in session-manager handles resurrection. Adding it creates feature overlap and maintenance burden. | Show exited session status; let built-in handle resurrection flow. |
| Mouse interaction (v1) | Zellij is keyboard-first. Mouse adds input handling complexity for minimal gain. Most Zellij users are keyboard navigators. | Keyboard-only in v1. Mouse is a v2 consideration. |
| Per-project layout application | Applying custom layouts when creating sessions is useful but complex (layout file resolution, error handling, layout compatibility). zsm and zellij-workspace already tackle this. | Accept optional `layout` per project in config but defer implementation to v2. |
| Session rename from sidebar | Low frequency action. CLI `zellij action rename-session` exists. Adding inline rename adds text input handling complexity disproportionate to value. | Omit entirely. Users rename via CLI or built-in session-manager. |
| Multi-theme support | Only one user (you) for now. Catppuccin Frappe is the theme. Making it configurable adds config surface area for zero current users. | Hardcode Frappe. Extract colours to constants for future theming. |
| Fuzzy filter over pinned list | With 5-10 pinned projects, j/k navigation is faster than typing a filter query. Fuzzy search adds text input mode complexity. | Direct j/k navigation. If the list grows past ~15 items, reconsider. |

---

## Feature Dependencies

```
Configurable project list ──> Session list display (need projects to show sessions for)
                          ──> Create session from folder (need folder path from config)
                          ──> Pinned project ordering (order comes from config)

Session list display ──> Visual indicator for current session
                    ──> Session status indicators
                    ──> Live tab count per session (enhancement of list)
                    ──> Active pane command display (enhancement of list)

Keyboard navigation ──> Switch to session on selection (Enter action)
                   ──> Kill/delete session (x action)

Configurable info verbosity ──> Live tab count per session (shown at standard+)
                           ──> Active pane command display (shown at full only)

Persistent docked sidebar ──> Toggle visibility (toggle requires sidebar to exist)
```

## MVP Recommendation

Prioritize in this order:

1. **Configurable project list** -- without this, nothing works. Parse KDL config for pinned folders.
2. **Session list display with status** -- the core render. Show each pinned project with running/exited/none status.
3. **Keyboard navigation + switch** -- j/k to navigate, Enter to switch/create. The core interaction.
4. **Visual indicator for current session** -- highlight which session you are in.
5. **Kill session** -- x to kill. Low effort, high utility.
6. **Persistent docked sidebar** -- the differentiating UX. Render as a tiled pane, not floating.
7. **Toggle visibility** -- keybind to show/hide the sidebar.
8. **Live tab count** -- "[3]" next to session name. Low effort polish.

**Defer to v2:**
- Active pane command display: May require cross-session pane data that is not available via plugin API. Needs feasibility verification. If only current-session pane data is available, this feature is significantly less useful.
- Per-project layout application: Useful but adds config complexity and error paths.
- Configurable info verbosity: Natural evolution once tab count and command display exist. Ship with a sensible default first.

## Competitive Landscape Summary

| Plugin | UX Model | Session Create | Session Switch | Session Kill | Persistence | Pinned Projects | Tab/Pane Info |
|--------|----------|----------------|----------------|--------------|-------------|-----------------|---------------|
| Built-in session-manager | Floating popup | Yes | Yes | Yes | No (on-demand) | No | Expandable tree |
| zellij-sessionizer | Floating fuzzy finder | Yes (from dir) | Yes | No | No | No (scans dirs) | No |
| zellij-choose-tree | Floating tree view | No | Yes | Yes | No | No | Expandable tree |
| zellij-switch | CLI pipe (no UI) | Yes | Yes | No | N/A | No | No |
| zellij-favs | Floating list | No | Yes | Yes (batch) | Disk cache | Yes (favourites) | Optional |
| zsm | Floating fuzzy finder | Yes (from zoxide) | Yes | No | No | No (zoxide ranked) | No |
| **This plugin** | **Docked sidebar** | **Yes** | **Yes** | **Yes** | **Yes (always visible)** | **Yes (config)** | **Tab count** |

The unique combination is: **persistent + pinned + live status**. No existing plugin occupies this niche.

## Sources

- [zellij-choose-tree](https://github.com/laperlej/zellij-choose-tree) - tmux choose-tree inspired session tree
- [zellij-sessionizer (cunialino)](https://github.com/cunialino/zellij-sessionizer) - Fuzzy directory session creator
- [zellij-sessionizer (silicakes)](https://github.com/silicakes/zellij-sessionizer) - FZF-based session launcher
- [zellij-switch](https://github.com/mostafaqanbaryan/zellij-switch) - CLI pipe session switcher
- [zellij-favs](https://github.com/JoseMM2002/zellij-favs) - Session favourites manager
- [zsm](https://github.com/liam-mackie/zsm) - Zoxide session manager
- [zbuffers](https://github.com/Strech/zbuffers) - Tab switcher with search
- [harpoon](https://github.com/Nacho114/harpoon) - Pane bookmarking (nvim harpoon clone)
- [room](https://github.com/rvcas/room) - Tab fuzzy finder
- [Zellij built-in session-manager](https://zellij.dev/documentation/session-manager-alias.html)
- [Zellij plugin API events](https://zellij.dev/documentation/plugin-api-events.html)
- [Zellij plugin API commands](https://zellij.dev/documentation/plugin-api-commands.html)
- [Zellij 0.38.0 release](https://zellij.dev/news/session-manager-protobuffs/)
- [awesome-zellij](https://github.com/zellij-org/awesome-zellij)
- [tmux-sessionx](https://github.com/omerxx/tmux-sessionx) - Feature-rich tmux session manager
- [sesh](https://github.com/joshmedeski/sesh) - Multiplexer-agnostic session CLI
- [Speedmux](https://github.com/webforspeed/speedmux) - Go multiplexer with persistent sidebar (prior art for sidebar concept)
- [Zellij session management tutorial](https://zellij.dev/tutorials/session-management/)
- [Zellij built-in plugins (DeepWiki)](https://deepwiki.com/zellij-org/zellij/4.3-built-in-plugins)
