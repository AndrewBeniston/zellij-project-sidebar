# Roadmap: Zellij Project Sidebar

## Milestones

- v1.0 Core Sidebar - Phases 1-4 (shipped 2026-03-14)
- v1.1 Rich Cards - Phases 5-8 (in progress)

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

<details>
<summary>v1.0 Core Sidebar (Phases 1-4) - SHIPPED 2026-03-14</summary>

- [x] **Phase 1: Scaffold + Lifecycle** - Compiling WASM plugin that loads in Zellij, requests permissions, and subscribes to session events
- [x] **Phase 2: Display + Interaction** - Pinned project list with live session status, keyboard navigation, and session switch/create/kill
- [x] **Phase 3: Sidebar Layout + Toggle** - Docked side panel with fixed width and Cmd+O pipe-based visibility toggle
- [x] **Phase 4: Enrichment + Theme** - Tab count, active command, info verbosity modes, and Catppuccin Frappe colours

</details>

### v1.1 Rich Cards (In Progress)

- [x] **Phase 5: Data Model + Polling Infrastructure** - ProjectMetadata struct, timer-driven polling loop, git branch detection via run_command
- [x] **Phase 6: Multi-Line Card Rendering** - Atomic refactor of rendering, mouse, scroll, and selection to support multi-line project cards
- [ ] **Phase 7: Pipe Protocol -- Pills + Progress** - External metadata injection via pipe messages with card rendering for pills and progress bars
- [ ] **Phase 8: Port Detection + Polish** - Listening port display via auto-detection (lsof) and pipe-based reporting

## Phase Details

<details>
<summary>v1.0 Core Sidebar (Phases 1-4) - SHIPPED 2026-03-14</summary>

### Phase 1: Scaffold + Lifecycle
**Goal**: A WASM plugin that compiles, loads in Zellij, requests the correct permissions, and receives live session data
**Depends on**: Nothing (first phase)
**Requirements**: INFR-01, INFR-02, INFR-03
**Success Criteria** (what must be TRUE):
  1. Running `cargo build --target wasm32-wasip1` produces a .wasm file that loads in Zellij 0.43.1 without errors
  2. On first load, plugin presents a permission prompt and proceeds after user grants permissions
  3. Plugin receives SessionUpdate events and logs session data to stderr (visible in Zellij logs)
**Plans**: 1 plan

Plans:
- [x] 01-01-PLAN.md -- Rust WASM plugin scaffold with permissions, event subscriptions, and dev layout

### Phase 2: Display + Interaction
**Goal**: Users see their pinned projects with live session status and can navigate, switch, create, and kill sessions entirely from the sidebar
**Depends on**: Phase 1
**Requirements**: DISP-01, DISP-02, DISP-03, INTR-01, INTR-02, INTR-03, INTR-04, INFR-04
**Plans**: 3 plans

Plans:
- [x] 02-01-PLAN.md -- Config parsing, session matching, and project list rendering with status indicators
- [x] 02-02-PLAN.md -- Keyboard navigation, session actions, and focus management
- [x] 02-03-PLAN.md -- Human verification of all requirements in live Zellij

### Phase 3: Sidebar Layout + Toggle
**Goal**: The plugin operates as a docked side panel that users can show/hide from any context
**Depends on**: Phase 2
**Plans**: Executed ad-hoc

### Phase 4: Enrichment + Theme
**Goal**: The sidebar shows rich session metadata styled to match Catppuccin Frappe
**Depends on**: Phase 3
**Plans**: Executed ad-hoc

</details>

### Phase 5: Data Model + Polling Infrastructure
**Goal**: The plugin maintains per-project metadata (starting with git branch) that refreshes automatically on a timer without user action
**Depends on**: Phase 4 (v1.0 complete codebase)
**Requirements**: DATA-01, DATA-02, DATA-03, DATA-04, GIT-01, GIT-02
**Success Criteria** (what must be TRUE):
  1. Each project with an active session displays its current git branch name in the sidebar
  2. Git branch updates automatically every ~10 seconds without any user interaction
  3. Only projects with running sessions trigger polling commands -- inactive/undiscovered projects produce no subprocess calls
  4. When the sidebar first loads or a new session starts, the project renders immediately with its existing info (no blank flash or layout shift before metadata arrives)
  5. Multiple concurrent run_command results (from different projects) route correctly to the right project's metadata -- no cross-contamination
**Plans**: 1 plan

Plans:
- [x] 05-01-PLAN.md -- Data model, timer-driven polling, git branch detection, and inline branch display

### Phase 6: Multi-Line Card Rendering
**Goal**: Each project renders as a multi-line card showing name, status, and metadata, with mouse clicks, scroll, and keyboard all working correctly on variable-height cards
**Depends on**: Phase 5
**Requirements**: CARD-01, CARD-02, CARD-03, CARD-04, CARD-05
**Success Criteria** (what must be TRUE):
  1. Each project card occupies multiple lines -- project name and status on line 1, git branch and metadata on line 2, with visual separation between cards
  2. Clicking any line of a multi-line card (name line, detail line, or separator) selects the correct project
  3. Scrolling and keyboard navigation (Up/Down/j/k) move between projects correctly regardless of how many screen rows each card occupies
  4. When the sidebar is unfocused, the selection automatically tracks whichever session the user is currently working in
  5. All existing v1.0 functionality (switch, create, kill, browse mode, attention badges) continues to work after the card refactor
**Plans**: 1 plan

Plans:
- [x] 06-01-PLAN.md -- Atomic card refactor: RenderLine variants, multi-line rendering, mouse/scroll/selection updates

### Phase 7: Pipe Protocol -- Pills + Progress
**Goal**: External tools can push arbitrary metadata (key-value pills and progress percentages) via pipe messages, and these render on the corresponding project cards
**Depends on**: Phase 6
**Requirements**: PILL-01, PILL-02, PILL-03, PROG-01, PROG-02, PROG-03
**Success Criteria** (what must be TRUE):
  1. An external tool can send a pipe message and see a pill badge appear on the target project's card within one render cycle
  2. Multiple pills from different sources display together on the card (e.g., `env:prod` and `build:passing` both visible)
  3. Sending a clear message removes the specified pill or progress bar from the card
  4. An external tool can push a progress percentage (0-100) and see a character-cell progress bar rendered on the project card
  5. When a session exits, any pills and progress associated with that project are automatically cleared
**Plans**: 2 plans

Plans:
- [ ] 07-01-PLAN.md -- Data model (AgentState/AgentStatus, pills, progress) and pipe message handlers
- [ ] 07-02-PLAN.md -- Card rendering (AI dot, pills, progress bar, card styling) and hook script template

### Phase 8: Port Detection + Polish
**Goal**: Listening ports are visible on project cards, detected automatically via lsof or reported via pipe messages
**Depends on**: Phase 5, Phase 6, Phase 7
**Requirements**: PORT-01, PORT-02, PORT-03
**Success Criteria** (what must be TRUE):
  1. Projects with active listening ports show port numbers on their card (e.g., `:3000 :8080`)
  2. External tools can report ports via pipe messages as an alternative to auto-detection
  3. Port information refreshes automatically on the polling timer and clears when a session stops
**Plans**: TBD

Plans:
- [ ] 08-01: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 5 -> 6 -> 7 -> 8

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Scaffold + Lifecycle | v1.0 | 1/1 | Complete | 2026-03-11 |
| 2. Display + Interaction | v1.0 | 3/3 | Complete | 2026-03-14 |
| 3. Sidebar Layout + Toggle | v1.0 | -/- | Complete | 2026-03-14 |
| 4. Enrichment + Theme | v1.0 | -/- | Complete | 2026-03-14 |
| 5. Data Model + Polling Infrastructure | v1.1 | 1/1 | Complete | 2026-03-14 |
| 6. Multi-Line Card Rendering | v1.1 | 1/1 | Complete | 2026-03-14 |
| 7. Pipe Protocol -- Pills + Progress | 1/2 | In Progress|  | - |
| 8. Port Detection + Polish | v1.1 | 0/? | Not started | - |
