#!/bin/bash
# reload-all.sh — Reload the sidebar plugin across ALL active Zellij sessions
PLUGIN_URL="file:$HOME/.config/zellij/plugins/zellij-project-sidebar.wasm"
CONFIG='scan_dir "/Users/andrewbeniston/Documents/01-Projects/Git",pin_file "/Users/andrewbeniston/.config/zellij/pinned-projects.json",session_layout "/Users/andrewbeniston/.config/zellij/layouts/clean.kdl"'

for session in $(zellij list-sessions 2>&1 | grep -v EXITED | sed 's/\x1b\[[0-9;]*m//g' | awk '{print $1}'); do
  echo "Reloading in: $session"
  zellij --session "$session" action start-or-reload-plugin "$PLUGIN_URL" -c "$CONFIG" 2>&1 &
done
wait
echo "Done — all sessions reloaded"
