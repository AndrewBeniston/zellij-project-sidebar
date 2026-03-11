# T02: 02-display-interaction 02

**Slice:** S02 — **Milestone:** M001

## Description

Add keyboard navigation (j/k), session actions (Enter to switch/create, x to kill), and focus management (unselectable by default, Alt+s to activate, Esc to deactivate).

Purpose: Completes the interaction layer that makes the sidebar usable. Without this, users can see the project list but cannot interact with it.

Output: A fully interactive project sidebar with keyboard navigation, session management, and proper focus control.

## Must-Haves

- [ ] "User can move selection up/down with j/k keys"
- [ ] "Pressing Enter on a running session switches to that session"
- [ ] "Pressing Enter on a project with no session creates one with cwd set to that folder"
- [ ] "Pressing x on a running session kills it and the list updates"
- [ ] "Plugin pane is unselectable during normal terminal work"
- [ ] "A keybind (Alt+s) activates the sidebar for interaction"
- [ ] "Pressing Esc deactivates the sidebar (returns to unselectable)"
- [ ] "Pressing x on the current session is a no-op (safety guard)"

## Files

- `src/main.rs`
