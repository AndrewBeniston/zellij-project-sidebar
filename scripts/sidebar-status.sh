#!/bin/bash
# sidebar-status.sh -- Claude Code hook for Zellij sidebar AI state
# Install: copy to ~/.claude/hooks/sidebar-status.sh
# Register: add to ~/.claude/settings.json hooks (see README)
#
# This script is called by Claude Code on PostToolUse, Stop, Notification,
# and SessionStart events. It reads the hook event JSON from stdin and
# sends the appropriate pipe message to the Zellij sidebar plugin.
#
# MUST be registered with async: true to never block Claude Code.
#
# Example ~/.claude/settings.json hooks configuration:
#   "hooks": {
#     "PostToolUse": [{"matcher": "", "hooks": [{"type": "command", "command": "~/.claude/hooks/sidebar-status.sh", "async": true}]}],
#     "Stop":        [{"matcher": "", "hooks": [{"type": "command", "command": "~/.claude/hooks/sidebar-status.sh", "async": true}]}],
#     "Notification":[{"matcher": "", "hooks": [{"type": "command", "command": "~/.claude/hooks/sidebar-status.sh", "async": true}]}],
#     "SessionStart":[{"matcher": "", "hooks": [{"type": "command", "command": "~/.claude/hooks/sidebar-status.sh", "async": true}]}]
#   }

INPUT=$(cat)
SESSION="$ZELLIJ_SESSION_NAME"

# Exit silently if not in a Zellij session
[ -z "$SESSION" ] && exit 0

# Extract event name from hook JSON (jq must be available)
EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty' 2>/dev/null)
[ -z "$EVENT" ] && exit 0

case "$EVENT" in
  PostToolUse)
    TOOL=$(echo "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null)
    zellij pipe --name "sidebar::ai" \
      -a "session=$SESSION" \
      -a "state=active" \
      -a "tool=$TOOL" 2>/dev/null &
    ;;
  Stop)
    zellij pipe --name "sidebar::ai" \
      -a "session=$SESSION" \
      -a "state=idle" 2>/dev/null &
    ;;
  Notification)
    zellij pipe --name "sidebar::ai" \
      -a "session=$SESSION" \
      -a "state=waiting" 2>/dev/null &
    ;;
  SessionStart)
    zellij pipe --name "sidebar::ai" \
      -a "session=$SESSION" \
      -a "state=active" 2>/dev/null &
    ;;
esac

exit 0
