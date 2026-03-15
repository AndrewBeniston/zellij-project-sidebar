# zellij-project-sidebar

A persistent sidebar plugin for [Zellij](https://zellij.dev) that shows your active project sessions at a glance. Switch between projects with a keypress, start new sessions, and see which ones need your attention.

![screenshot](screenshot.png)

## Quick start

Give this prompt to Claude Code (or your AI coding tool of choice) and it will handle everything:

> Install the zellij-project-sidebar plugin from https://github.com/AndrewBeniston/zellij-project-sidebar. Clone the repo, build with `cargo build --target wasm32-wasip1 --release`, copy the .wasm to `~/.config/zellij/plugins/`. Then update my Zellij layout to include the sidebar with `scan_dir` pointing to my projects directory. Set up Claude Code hooks using the sidebar-status.sh script from the repo so the sidebar shows real-time AI activity indicators across all sessions (see the "AI activity indicators" section in the README for full setup). Also configure the attention system hooks for `sidebar::attention::` and `sidebar::clear::` pipe messages.

## Why?

Zellij has great session management, but no ambient awareness. You can't see at a glance which projects are running, which session you're in, or which one has Claude Code waiting for input. This plugin gives you a docked sidebar that stays visible across tabs. Think VS Code's sidebar, but for terminal sessions.

## Features

- **Active sessions at a glance**: only shows projects with running or exited sessions, no clutter
- **Current session highlighted**: green text shows you exactly where you are
- **Browse mode**: press `/` to search all discovered projects and start new sessions
- **Attention indicators**: a red diamond appears when a session needs your input (e.g. Claude Code waiting)
- **Session lifecycle**: create, switch to, or kill sessions from the sidebar
- **Auto-discovery**: scans a directory for projects instead of manual configuration
- **New tab with sidebar**: `Cmd+T` creates tabs that include the sidebar
- **Mouse support**: click a project to switch, scroll wheel to navigate
- **Toggle visibility**: `Cmd+O` to focus/unfocus the sidebar
- **Fuzzy search**: subsequence matching in browse mode

## Install

### Build from source

```bash
git clone https://github.com/AndrewBeniston/zellij-project-sidebar.git
cd zellij-project-sidebar
cargo build --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/zellij-project-sidebar.wasm ~/.config/zellij/plugins/
```

> Requires Rust with the `wasm32-wasip1` target: `rustup target add wasm32-wasip1`

## Configuration

Add the plugin to your Zellij layout (e.g. `~/.config/zellij/layouts/default.kdl`):

### Discovery mode (recommended)

Automatically discovers projects from a directory:

```kdl
layout {
    pane size=1 borderless=true {
        plugin location="tab-bar"
    }
    pane split_direction="vertical" {
        pane size="15%" name="Projects" {
            plugin location="file:~/.config/zellij/plugins/zellij-project-sidebar.wasm" {
                scan_dir "/Users/you/Projects"
                session_layout "/Users/you/.config/zellij/layouts/default.kdl"
            }
        }
        pane
    }
}
```

| Option | Description |
|--------|-------------|
| `scan_dir` | Directory to scan for project folders |
| `session_layout` | Layout file applied when creating new sessions |
| `verbosity` | `"full"` (default) or `"minimal"` to control tab count and command display |

### Legacy mode

Manually list projects:

```kdl
plugin location="file:~/.config/zellij/plugins/zellij-project-sidebar.wasm" {
    project_0 "/Users/you/Projects/my-app"
    project_1 "/Users/you/Projects/api-server"
    project_2 "/Users/you/Projects/docs"
}
```

## Keybindings

### When sidebar is focused

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate projects |
| `Enter` | Switch to session (or create if not started) |
| `Delete` | Kill selected session |
| `/` | Enter browse mode (search all projects) |
| `Esc` | Deactivate sidebar |
| `Alt+R` | Rescan project directory |
| Click | Switch to clicked project |
| Scroll | Navigate projects |

### Browse mode

| Key | Action |
|-----|--------|
| Type | Fuzzy search projects |
| `Enter` | Open selected project |
| `Backspace` | Delete search character |
| `Esc` | Exit browse mode |

### Global (registered by plugin)

| Key | Action |
|-----|--------|
| `Cmd+O` / `Super+O` | Toggle sidebar focus |
| `Cmd+T` / `Super+T` | New tab with sidebar |

> `Cmd` keys require a terminal that passes them through (e.g. Ghostty with `keybind = cmd+o=unbind`).

## Attention system

The sidebar shows a magenta `!` indicator when a session needs your attention. This is powered by Zellij's pipe messaging:

```bash
# Flag a session as needing attention
zellij pipe --name "sidebar::attention::session-name"

# Clear attention for a session
zellij pipe --name "sidebar::clear::session-name"
```

Attention is automatically cleared when you switch to a session via the sidebar.

## AI activity indicators

The sidebar shows real-time AI agent activity across all your Zellij sessions. When Claude Code (or any AI tool) is working in a session, you'll see it at a glance without switching sessions.

| Symbol | Colour | Meaning |
|--------|--------|---------|
| `▶` | Green | AI agent is actively working |
| `■` | Cyan | AI agent is idle (done/waiting) |
| `!` | Magenta | Needs attention |
| `·` | Orange | No AI activity |

Sessions with AI activity also show "claude" on a detail line beneath the session name.

### How it works

AI state is shared across all sessions via per-session files in `/tmp/sidebar-ai/`. Each sidebar instance reads these files on a 10-second timer, so cross-session state appears within seconds.

There are two complementary mechanisms:
1. **Pipe messages** (instant, current session): `zellij pipe --name "sidebar::ai-active::session-name"`
2. **Shared files** (cross-session, ~10s delay): write `active`/`idle`/`waiting` to `$TMPDIR/zellij-$(id -u)/sidebar-ai/<session-name>`

### Setting up Claude Code hooks

Add the hook script to `~/.claude/hooks/sidebar-status.sh`:

```bash
#!/bin/bash
INPUT=$(cat)
SESSION="$ZELLIJ_SESSION_NAME"
[ -z "$SESSION" ] && exit 0
EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty' 2>/dev/null)
[ -z "$EVENT" ] && exit 0

STATE_DIR="${TMPDIR:-/tmp/}zellij-$(id -u)/sidebar-ai"
mkdir -p "$STATE_DIR" 2>/dev/null

case "$EVENT" in
  PostToolUse|SessionStart)
    echo "active" > "$STATE_DIR/$SESSION"
    zellij pipe --name "sidebar::ai-active::${SESSION}" 2>/dev/null &
    ;;
  Stop)
    echo "idle" > "$STATE_DIR/$SESSION"
    zellij pipe --name "sidebar::ai-idle::${SESSION}" 2>/dev/null &
    ;;
  Notification)
    echo "waiting" > "$STATE_DIR/$SESSION"
    zellij pipe --name "sidebar::ai-waiting::${SESSION}" 2>/dev/null &
    ;;
esac
exit 0
```

Then register it in `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PostToolUse": [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/sidebar-status.sh", "async": true }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/sidebar-status.sh", "async": true }] }],
    "Notification": [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/sidebar-status.sh", "async": true }] }],
    "SessionStart": [{ "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/sidebar-status.sh", "async": true }] }]
  }
}
```

### Other AI tools

Any tool can integrate — just write to the shared state directory:

```bash
# Signal that an AI agent is working in the current session
echo "active" > "${TMPDIR:-/tmp/}zellij-$(id -u)/sidebar-ai/$ZELLIJ_SESSION_NAME"

# Or use pipes for instant updates
zellij pipe --name "sidebar::ai-active::$ZELLIJ_SESSION_NAME"
```

### Pipe API reference

| Pipe name | Effect |
|-----------|--------|
| `sidebar::ai-active::<session>` | Show AI as working (▶ green) |
| `sidebar::ai-idle::<session>` | Show AI as idle (■ cyan) |
| `sidebar::ai-waiting::<session>` | Show AI as waiting (■ cyan) |
| `sidebar::attention::<session>` | Flag session for attention (! magenta) |
| `sidebar::clear::<session>` | Clear attention flag |

## Reloading the plugin

After rebuilding, you can reload the plugin in a single session via the Zellij plugin manager: **Ctrl+O, P**, select the sidebar, then press Enter to reload.

### Reload across all sessions

A convenience script is included to reload the plugin in every active session at once:

```bash
./scripts/reload-all.sh
```

> **Known issue:** Zellij's `start-or-reload-plugin` CLI command may spawn a duplicate plugin pane in each session instead of reloading the existing one. If this happens, open the plugin manager (**Ctrl+O, P**), select the duplicate (it will show "No projects configured"), and press **Delete** (Fn+Backspace on Mac) to remove it. The original sidebar will continue working.

## Pairs well with

This plugin handles session-level awareness. For the full picture, it works nicely alongside:

- [**zellij-sessionizer**](https://github.com/lapce/zellij-sessionizer): fuzzy directory search for starting sessions from anywhere on disk, not just your `scan_dir`. Good for one-off projects.
- [**zellij-choose-tree**](https://github.com/lapce/zellij-choose-tree): tree view for jumping between tabs and panes *within* a session. The sidebar handles between-session navigation, choose-tree handles within-session.

## Requirements

- Zellij 0.43.x+
- Rust with `wasm32-wasip1` target

## Licence

MIT
