# T03: 02-display-interaction 03

**Slice:** S02 — **Milestone:** M001

## Description

Verify all Phase 2 functionality works end-to-end in a running Zellij instance with real sessions.

Purpose: WASM plugins have no unit test harness. The only way to verify correctness is loading the plugin into Zellij and manually testing each interaction. This checkpoint confirms that config parsing, session matching, rendering, keyboard navigation, session actions, and focus management all work together.

Output: Human verification that all 8 requirements are met.

## Must-Haves

- [ ] "All 8 Phase 2 requirements verified in a running Zellij instance"
- [ ] "Session status changes reflected in real-time"
- [ ] "Focus activation/deactivation cycle works end-to-end"
