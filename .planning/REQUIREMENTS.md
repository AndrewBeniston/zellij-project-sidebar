# Requirements: Zellij Project Sidebar

**Defined:** 2026-03-14
**Core Value:** One-keypress project switching with always-visible session awareness

## v1.1 Requirements

Requirements for Rich Cards milestone. Each maps to roadmap phases.

### Card Layout

- [ ] **CARD-01**: Each project renders as a multi-line card (name+status on line 1, metadata on subsequent lines)
- [ ] **CARD-02**: Cards have visual separators between them
- [ ] **CARD-03**: Mouse click maps correctly to multi-line cards (clicking any line of a card selects that project)
- [ ] **CARD-04**: Scroll and keyboard navigation work correctly with variable-height cards
- [ ] **CARD-05**: Selection auto-tracks current session when sidebar is unfocused

### Data Pipeline

- [x] **DATA-01**: Plugin polls for metadata (git branch, ports) on a timer interval
- [x] **DATA-02**: run_command results are correctly routed via context tagging (type + project name)
- [x] **DATA-03**: Polling only runs for projects with active sessions (not all discovered dirs)
- [x] **DATA-04**: No loading flash — render gracefully before data arrives

### Git Integration

- [x] **GIT-01**: Each project with an active session shows current git branch name
- [x] **GIT-02**: Branch updates automatically on timer interval without user action

### Status Pills

- [ ] **PILL-01**: External tools can push key-value metadata via pipe messages
- [ ] **PILL-02**: Pills display on the card below the project name
- [ ] **PILL-03**: Pills are cleared when the source tool sends a clear message

### Progress Bar

- [ ] **PROG-01**: External tools can push a progress percentage via pipe messages
- [ ] **PROG-02**: Progress renders as a character-cell bar on the card
- [ ] **PROG-03**: Progress is cleared when complete or explicitly cleared

### Port Detection

- [ ] **PORT-01**: Plugin auto-detects listening ports per project via lsof on timer
- [ ] **PORT-02**: External tools can also report ports via pipe messages
- [ ] **PORT-03**: Detected ports display on the project card

## v1.0 Requirements (Complete)

### Infrastructure

- [x] **INFR-01**: Plugin compiles to wasm32-wasip1 and loads in Zellij 0.43.1
- [x] **INFR-02**: Plugin requests and handles permissions correctly
- [x] **INFR-03**: Plugin subscribes to SessionUpdate events for live data

### Display & Interaction

- [x] **DISP-01**: Session-based default view (running/exited only)
- [x] **DISP-02**: Browse mode with fuzzy search
- [x] **DISP-03**: Current session highlighted green
- [x] **DISP-04**: Tab count and active command display
- [x] **INTR-01**: Keyboard navigation (Up/Down, Enter, Delete, Esc)
- [x] **INTR-02**: Mouse click and scroll support
- [x] **INTR-03**: Toggle focus (Cmd+O), new tab with sidebar (Cmd+T)
- [x] **INTR-04**: Attention system via pipe messages

## Out of Scope

| Feature | Reason |
|---------|--------|
| Rename session | Low value, can do via CLI |
| Tab/pane management within sessions | Choose-tree handles this |
| Multi-theme support | Hardcode Catppuccin Frappe, themeable later |
| Floating popup mode | Contradicts core value |
| Git dirty/clean indicator | Keep simple for v1.1, branch name only |
| Expandable/collapsible cards | Over-engineering for v1.1, fixed multi-line is sufficient |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CARD-01 | Phase 6 | Pending |
| CARD-02 | Phase 6 | Pending |
| CARD-03 | Phase 6 | Pending |
| CARD-04 | Phase 6 | Pending |
| CARD-05 | Phase 6 | Pending |
| DATA-01 | Phase 5 | Complete |
| DATA-02 | Phase 5 | Complete |
| DATA-03 | Phase 5 | Complete |
| DATA-04 | Phase 5 | Complete |
| GIT-01 | Phase 5 | Complete |
| GIT-02 | Phase 5 | Complete |
| PILL-01 | Phase 7 | Pending |
| PILL-02 | Phase 7 | Pending |
| PILL-03 | Phase 7 | Pending |
| PROG-01 | Phase 7 | Pending |
| PROG-02 | Phase 7 | Pending |
| PROG-03 | Phase 7 | Pending |
| PORT-01 | Phase 8 | Pending |
| PORT-02 | Phase 8 | Pending |
| PORT-03 | Phase 8 | Pending |

**Coverage:**
- v1.1 requirements: 20 total
- Mapped to phases: 20
- Unmapped: 0

---
*Requirements defined: 2026-03-14*
*Last updated: 2026-03-14 after roadmap creation*
