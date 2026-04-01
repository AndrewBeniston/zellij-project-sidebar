#!/bin/bash
# sidebar-status.sh -- Claude Code hook for Zellij sidebar AI state
# One file per pane: /tmp/sidebar-ai/<session>/<pane_id>
# Format: "state timestamp duration agent"
#
# Cross-session design:
# - State files in /tmp/sidebar-ai/<session>/<pane_id> are polled by the sidebar
#   plugin every 10s via its Timer event, so cross-session AI state (▶ ■ !)
#   is always reflected within ~10s — no cross-session pipe needed.
# - Same-session pipes give instant updates when Claude and the sidebar share a session.

INPUT=$(cat)
SESSION="$ZELLIJ_SESSION_NAME"
PANE="${ZELLIJ_PANE_ID:-0}"
[ -z "$SESSION" ] && exit 0

EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty' 2>/dev/null)
[ -z "$EVENT" ] && exit 0

# /tmp/sidebar-ai matches the hardcoded path the plugin reads
STATE_DIR="/tmp/sidebar-ai/$SESSION"
mkdir -p "$STATE_DIR" 2>/dev/null
NOW=$(date +%s)

# Broadcast to all sessions: zellij --session S pipe --name N
# Redirect </dev/null so zellij pipe doesn't consume the while-loop's stdin
pipe() {
  local name="$1"
  zellij list-sessions --no-formatting 2>/dev/null | awk '{print $1}' | while IFS= read -r s; do
    [ -n "$s" ] && zellij --session "$s" pipe --name "$name" </dev/null 2>/dev/null &
  done
}

case "$EVENT" in
  PostToolUse|SessionStart)
    CURRENT=$(cat "$STATE_DIR/$PANE" 2>/dev/null)
    if [ "${CURRENT%% *}" = "active" ]; then
      STARTED=$(echo "$CURRENT" | awk '{print $2}')
      echo "active ${STARTED:-$NOW} 0 claude" > "$STATE_DIR/$PANE"
    else
      echo "active $NOW 0 claude" > "$STATE_DIR/$PANE"
    fi
    pipe "sidebar::ai-active::${SESSION}::claude"
    pipe "sidebar::clear::${SESSION}"
    ;;
  Stop)
    CURRENT=$(cat "$STATE_DIR/$PANE" 2>/dev/null)
    STARTED=$(echo "$CURRENT" | awk '{print $2}')
    DURATION=0
    if [ "${CURRENT%% *}" = "active" ] && [ -n "$STARTED" ]; then
      DURATION=$((NOW - STARTED))
    fi
    echo "idle $NOW $DURATION claude" > "$STATE_DIR/$PANE"
    pipe "sidebar::ai-idle::${SESSION}::claude"
    pipe "sidebar::clear::${SESSION}"
    ;;
  Notification|PermissionRequest)
    CURRENT=$(cat "$STATE_DIR/$PANE" 2>/dev/null)
    STARTED=$(echo "$CURRENT" | awk '{print $2}')
    DURATION=0
    if [ "${CURRENT%% *}" = "active" ] && [ -n "$STARTED" ]; then
      DURATION=$((NOW - STARTED))
    fi
    echo "waiting $NOW $DURATION claude" > "$STATE_DIR/$PANE"
    pipe "sidebar::ai-waiting::${SESSION}::claude"
    pipe "sidebar::attention::${SESSION}"
    ;;
esac

exit 0
