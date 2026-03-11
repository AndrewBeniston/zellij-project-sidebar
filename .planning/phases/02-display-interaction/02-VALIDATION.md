---
phase: 2
slug: display-interaction
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-11
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual verification (Zellij WASM plugin — no unit test harness) |
| **Config file** | none — WASM plugins tested by loading into Zellij |
| **Quick run command** | `cargo build --target wasm32-wasip1` |
| **Full suite command** | `cargo build --target wasm32-wasip1 && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm` |
| **Estimated runtime** | ~5 seconds (build) + manual verification |

---

## Sampling Rate

- **After every task commit:** Run `cargo build --target wasm32-wasip1`
- **After every plan wave:** Run full suite — build + reload in Zellij + manual check
- **Before `/gsd:verify-work`:** Full suite must be green (compiles + all 8 requirements manually verified)
- **Max feedback latency:** 5 seconds (compilation)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | DISP-01 | smoke | `cargo build --target wasm32-wasip1` | N/A | ⬜ pending |
| 02-01-02 | 01 | 1 | DISP-02 | manual | Build + load plugin, create/kill sessions, verify status | N/A | ⬜ pending |
| 02-01-03 | 01 | 1 | DISP-03 | manual | Switch sessions, verify current session highlight | N/A | ⬜ pending |
| 02-01-04 | 01 | 1 | INTR-01 | manual | Focus plugin, press j/k, verify cursor moves | N/A | ⬜ pending |
| 02-01-05 | 01 | 1 | INTR-02 | manual | Select running session, Enter, verify switch | N/A | ⬜ pending |
| 02-01-06 | 01 | 1 | INTR-03 | manual | Select no-session project, Enter, verify new session with cwd | N/A | ⬜ pending |
| 02-01-07 | 01 | 1 | INTR-04 | manual | Select running session, press x, verify kill + status update | N/A | ⬜ pending |
| 02-01-08 | 01 | 1 | INFR-04 | manual | Verify Tab skips plugin. Focus keybind activates. Esc deactivates. | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements.* Phase 1 scaffold provides Cargo.toml, src/main.rs, and build target. No new test framework or stubs needed — WASM plugins are verified by compilation + manual Zellij interaction.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Project list renders from KDL config | DISP-01 | WASM plugin UI has no headless rendering API | Load plugin with project_0..N config, verify names display |
| Session status shows running/exited/not started | DISP-02 | Requires live Zellij sessions | Create sessions for some projects, kill one, verify 3 states display |
| Current session highlighted | DISP-03 | Requires visual inspection | Switch to different sessions, verify highlight follows |
| j/k navigation | INTR-01 | Key events only fire in focused plugin pane | Focus plugin via Alt+s, press j/k, verify selection moves |
| Enter switches to running session | INTR-02 | Requires live session switching | Select running session, Enter, verify Zellij switches |
| Enter creates session with cwd | INTR-03 | Requires session creation + cwd verification | Select unstarted project, Enter, verify new session + `pwd` shows correct dir |
| x kills session | INTR-04 | Requires live session termination | Select running (non-current) session, x, verify terminated |
| Unselectable by default | INFR-04 | Focus behavior is a UI state | Tab through panes, verify plugin skipped. Alt+s focuses. Esc unfocuses. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
