#!/bin/bash
# sidebar-status.sh -- Claude Code hook for Zellij sidebar AI state
# Writes one file per session to a shared dir — no JSON, no races

INPUT=$(cat)
SESSION="$ZELLIJ_SESSION_NAME"
[ -z "$SESSION" ] && exit 0

EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty' 2>/dev/null)
[ -z "$EVENT" ] && exit 0

# Shared state dir — all sidebar instances read this via WASI /tmp
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
