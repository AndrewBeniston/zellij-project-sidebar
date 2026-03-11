# Requirements: Zellij Project Sidebar

**Defined:** 2026-03-11
**Core Value:** One-keypress project switching with always-visible session awareness

## v1 Requirements

### Display

- [ ] **DISP-01**: Plugin renders a list of pinned project folders from KDL config
- [ ] **DISP-02**: Each project shows live session status (running / exited / not started)
- [ ] **DISP-03**: Current active session is visually highlighted
- [ ] **DISP-04**: Running sessions show tab count (e.g. `help-self [3]`)
- [ ] **DISP-05**: Active pane command displayed for current session (e.g. `claude` or `vim`)
- [ ] **DISP-06**: Info verbosity configurable — minimal (name + status dot) through full (tabs + command)

### Interaction

- [ ] **INTR-01**: User can navigate project list with j/k keys
- [ ] **INTR-02**: User can switch to a running session by pressing Enter
- [ ] **INTR-03**: If no session exists for a folder, Enter creates one with cwd set to that folder
- [ ] **INTR-04**: User can kill a session by pressing x on a running project
- [ ] **INTR-05**: User can toggle sidebar visibility with a keybind (Cmd+P via pipe mechanism)

### Layout

- [ ] **LAYT-01**: Plugin renders as a docked side panel (tiled pane, not floating)
- [ ] **LAYT-02**: Sidebar has fixed width (configurable, default ~20 chars)
- [ ] **LAYT-03**: Toggle hides/shows sidebar and reclaims/restores space

### Infrastructure

- [x] **INFR-01**: Plugin compiles to wasm32-wasip1 and loads in Zellij 0.43.1
- [x] **INFR-02**: Plugin requests and handles permissions correctly (first-launch UX)
- [x] **INFR-03**: Plugin subscribes to SessionUpdate events for live data (no polling)
- [ ] **INFR-04**: Sidebar is unselectable by default — becomes selectable only during active interaction
- [ ] **INFR-05**: Pipe-based toggle mechanism works from any context (unfocused)

### Theme

- [ ] **THEM-01**: Colours match Catppuccin Frappe via Zellij's color_range API
- [ ] **THEM-02**: Status indicators use semantic colours (green = running, dim = stopped, yellow = exited)

## v2 Requirements

### Enhanced Display

- **DISP-07**: Cross-session active pane command display (pending API verification)
- **DISP-08**: Per-project custom layout on session creation

### Interaction

- **INTR-06**: Mouse click to switch sessions
- **INTR-07**: Rename session in-place
- **INTR-08**: Drag to reorder pinned projects

### Theme

- **THEM-03**: Themeable via Zellij theme system (not hardcoded to Frappe)
- **THEM-04**: Custom icons/glyphs for project types

### Configuration

- **CONF-01**: Auto-scan directory mode (scan + pin favourites)
- **CONF-02**: Per-project layout assignment in config

## Out of Scope

| Feature | Reason |
|---------|--------|
| Fuzzy search / directory scanning | Sessionizer already does this — this plugin is for pinned projects |
| Tab/pane management within sessions | Choose-tree handles this |
| Session resurrection controls | Zellij handles this natively |
| Multi-plugin communication | No established pattern, adds complexity |
| Floating popup mode | Contradicts core value — sidebar is the differentiator |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| INFR-01 | Phase 1 | Complete |
| INFR-02 | Phase 1 | Complete |
| INFR-03 | Phase 1 | Complete |
| DISP-01 | Phase 2 | Pending |
| DISP-02 | Phase 2 | Pending |
| DISP-03 | Phase 2 | Pending |
| INTR-01 | Phase 2 | Pending |
| INTR-02 | Phase 2 | Pending |
| INTR-03 | Phase 2 | Pending |
| INTR-04 | Phase 2 | Pending |
| INFR-04 | Phase 2 | Pending |
| LAYT-01 | Phase 3 | Pending |
| LAYT-02 | Phase 3 | Pending |
| LAYT-03 | Phase 3 | Pending |
| INTR-05 | Phase 3 | Pending |
| INFR-05 | Phase 3 | Pending |
| DISP-04 | Phase 4 | Pending |
| DISP-05 | Phase 4 | Pending |
| DISP-06 | Phase 4 | Pending |
| THEM-01 | Phase 4 | Pending |
| THEM-02 | Phase 4 | Pending |

**Coverage:**
- v1 requirements: 21 total
- Mapped to phases: 21
- Unmapped: 0

---
*Requirements defined: 2026-03-11*
*Last updated: 2026-03-11 after roadmap creation (4-phase structure)*
