#!/bin/bash
# reload-all.sh — Rebuild and install the sidebar plugin
#
# NOTE: There is no CLI command to reload an existing layout-loaded plugin.
# After running this script, reload manually in each session:
#   Ctrl+O, P → select the sidebar → Enter
#
# The sidebar's snapshot restore ensures minimal flash on reload.

set -e

echo "Building release..."
cargo build --release --target wasm32-wasip1

echo "Installing..."
cp target/wasm32-wasip1/release/zellij-project-sidebar.wasm ~/.config/zellij/plugins/zellij-project-sidebar.wasm

echo ""
echo "Installed. Reload the plugin in each session:"
echo "  Ctrl+O, P → select sidebar → Enter"
