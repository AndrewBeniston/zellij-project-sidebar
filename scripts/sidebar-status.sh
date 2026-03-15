#!/bin/bash
# sidebar-status.sh -- Claude Code hook for Zellij sidebar AI state
# One file per pane: /tmp/sidebar-ai/<session>/<pane_id>
# Format: "state timestamp [duration]"

INPUT=$(cat)
SESSION="$ZELLIJ_SESSION_NAME"
PANE="${ZELLIJ_PANE_ID:-0}"
[ -z "$SESSION" ] && exit 0

EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty' 2>/dev/null)
[ -z "$EVENT" ] && exit 0

STATE_DIR="${TMPDIR:-/tmp/}zellij-$(id -u)/sidebar-ai/$SESSION"
mkdir -p "$STATE_DIR" 2>/dev/null
NOW=$(date +%s)

case "$EVENT" in
  PostToolUse|SessionStart)
    CURRENT=$(cat "$STATE_DIR/$PANE" 2>/dev/null)
    if [ "${CURRENT%% *}" != "active" ]; then
      echo "active $NOW" > "$STATE_DIR/$PANE"
    fi
    zellij pipe --name "sidebar::ai-active::${SESSION}" 2>/dev/null &
    ;;
  Stop|Notification)
    CURRENT=$(cat "$STATE_DIR/$PANE" 2>/dev/null)
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
    echo "$STATE $NOW $DURATION" > "$STATE_DIR/$PANE"
    zellij pipe --name "${PIPE}::${SESSION}" 2>/dev/null &
    ;;
esac

exit 0
