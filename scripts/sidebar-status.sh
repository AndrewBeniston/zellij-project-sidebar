#!/bin/bash
# sidebar-status.sh -- Claude Code hook for Zellij sidebar AI state
# File format: "state started_at" (e.g. "active 1710460800")
# For idle/waiting: "state started_at duration" (e.g. "idle 1710460830 30")
#   where duration = how long the active turn lasted

INPUT=$(cat)
SESSION="$ZELLIJ_SESSION_NAME"
[ -z "$SESSION" ] && exit 0

EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty' 2>/dev/null)
[ -z "$EVENT" ] && exit 0

STATE_DIR="${TMPDIR:-/tmp/}zellij-$(id -u)/sidebar-ai"
mkdir -p "$STATE_DIR" 2>/dev/null
NOW=$(date +%s)

case "$EVENT" in
  PostToolUse|SessionStart)
    CURRENT=$(cat "$STATE_DIR/$SESSION" 2>/dev/null)
    if [ "${CURRENT%% *}" != "active" ]; then
      echo "active $NOW" > "$STATE_DIR/$SESSION"
    fi
    zellij pipe --name "sidebar::ai-active::${SESSION}" 2>/dev/null &
    ;;
  Stop|Notification)
    # Calculate how long the active turn lasted
    CURRENT=$(cat "$STATE_DIR/$SESSION" 2>/dev/null)
    STARTED=$(echo "$CURRENT" | awk '{print $2}')
    DURATION=0
    if [ "${CURRENT%% *}" = "active" ] && [ -n "$STARTED" ]; then
      DURATION=$((NOW - STARTED))
    fi
    STATE="idle"
    PIPE="sidebar::ai-idle"
    if [ "$EVENT" = "Notification" ]; then
      STATE="waiting"
      PIPE="sidebar::ai-waiting"
    fi
    echo "$STATE $NOW $DURATION" > "$STATE_DIR/$SESSION"
    zellij pipe --name "${PIPE}::${SESSION}" 2>/dev/null &
    ;;
esac

exit 0
