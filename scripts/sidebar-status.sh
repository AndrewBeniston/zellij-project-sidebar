#!/bin/bash
# sidebar-status.sh -- Claude Code hook for Zellij sidebar AI state
# Format: sidebar::ai-{state}::{session} (matches attention pattern)

INPUT=$(cat)
SESSION="$ZELLIJ_SESSION_NAME"

[ -z "$SESSION" ] && exit 0

EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty' 2>/dev/null)
[ -z "$EVENT" ] && exit 0

case "$EVENT" in
  PostToolUse)
    zellij pipe --name "sidebar::ai-active::${SESSION}" 2>/dev/null &
    ;;
  Stop)
    zellij pipe --name "sidebar::ai-idle::${SESSION}" 2>/dev/null &
    ;;
  Notification)
    zellij pipe --name "sidebar::ai-waiting::${SESSION}" 2>/dev/null &
    ;;
  SessionStart)
    zellij pipe --name "sidebar::ai-active::${SESSION}" 2>/dev/null &
    ;;
esac

exit 0
