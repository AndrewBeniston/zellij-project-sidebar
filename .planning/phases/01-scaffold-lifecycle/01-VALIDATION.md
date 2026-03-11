---
phase: 1
slug: scaffold-lifecycle
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-11
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual verification (Zellij WASM plugin — no unit test framework) |
| **Config file** | none — WASM plugins are tested by loading into Zellij |
| **Quick run command** | `cargo build && zellij action start-or-reload-plugin file:target/wasm32-wasip1/debug/zellij-project-sidebar.wasm` |
| **Full suite command** | `cargo build --release` (compilation success = structural correctness) |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build`
- **After every plan wave:** Load plugin in Zellij, verify permission flow and SessionUpdate logging
- **Before `/gsd:verify-work`:** All three success criteria verified manually in running Zellij
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | INFR-01 | smoke | `cargo build --target wasm32-wasip1 2>&1; echo "exit: $?"` | N/A (cargo) | ⬜ pending |
| 01-01-02 | 01 | 1 | INFR-02 | manual | Load plugin in Zellij, verify permission dialog, grant, check log | N/A | ⬜ pending |
| 01-01-03 | 01 | 1 | INFR-03 | manual | Load plugin, check Zellij log for "SessionUpdate" messages | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `Cargo.toml` — zellij-tile 0.43.1, serde, edition 2021
- [ ] `.cargo/config.toml` — wasm32-wasip1 target
- [ ] `src/main.rs` — ZellijPlugin trait scaffold
- [ ] `zellij.kdl` — development layout with hot-reload
- [ ] `rustup target add wasm32-wasip1` — WASM target installation

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Permission dialog appears on first load | INFR-02 | Requires Zellij runtime UI interaction | 1. Load plugin in Zellij 2. Verify permission dialog appears 3. Grant permissions 4. Check log for "Permissions granted" |
| SessionUpdate events received | INFR-03 | Requires live Zellij session with session data | 1. Load plugin in Zellij 2. Open/switch tabs 3. Check log for "SessionUpdate: N active" messages |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
