use zellij_tile::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

// --- Zellij Emphasis Colors ---
// color_range only supports indices 0-3 (four emphasis levels)
// Actual colors depend on the user's Zellij theme
//   0 = emphasis_0 (typically green)
//   1 = emphasis_1 (typically cyan/blue)
//   2 = emphasis_2 (typically red)
//   3 = emphasis_3 (typically yellow/orange)

// Zellij Text palette: 0=orange, 1=cyan, 2=green, 3=magenta
const COLOR_ORANGE: usize = 0;
const COLOR_CYAN: usize = 1;
const COLOR_GREEN: usize = 2;
const COLOR_MAGENTA: usize = 3;

const CMD_KEY: &str = "cmd";
const CMD_SCAN_DIR: &str = "scan_dir";
const CMD_GIT_BRANCH: &str = "git_branch";
const PROJECT_KEY: &str = "project";

// --- Verbosity ---

#[derive(Clone, PartialEq)]
enum Verbosity {
    Minimal,
    Full,
}

impl Default for Verbosity {
    fn default() -> Self {
        Verbosity::Full
    }
}

// --- Data Model ---

#[derive(Clone, PartialEq)]
enum SessionStatus {
    Running {
        is_current: bool,
        tab_count: usize,
        active_command: Option<String>,
    },
    Exited,
    NotStarted,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum AgentState {
    Active,
    Idle,
    Waiting,
    Unknown,
}

impl Default for AgentState {
    fn default() -> Self {
        AgentState::Unknown
    }
}

#[derive(Clone, Default)]
struct AgentStatus {
    state: AgentState,
    last_tool: Option<String>,
}

#[derive(Clone, Default)]
struct ProjectMetadata {
    git_branch: Option<String>,
    is_git_repo: Option<bool>, // None = unknown, Some(false) = not git, Some(true) = is git
    agent: AgentStatus,
    pills: BTreeMap<String, String>,
    progress_pct: Option<u8>,
}

#[derive(Clone)]
struct Project {
    name: String,
    path: String,
    status: SessionStatus,
    metadata: ProjectMetadata,
}

enum RenderLine {
    Header(String),
    ProjectRow(usize),    // index into self.projects (name line)
    ProjectDetail(usize), // index into self.projects (detail line: git branch, future metadata)
    CardTop,              // ╭───────────────╮
    CardBottom,           // ╰───────────────╯
    CardDivider,          // ├───────────────┤ (shared border between cards)
}

impl RenderLine {
    fn project_index(&self) -> Option<usize> {
        match self {
            RenderLine::ProjectRow(idx) | RenderLine::ProjectDetail(idx) => Some(*idx),
            RenderLine::Header(_) | RenderLine::CardTop | RenderLine::CardBottom | RenderLine::CardDivider => None,
        }
    }
}

struct State {
    permissions_granted: bool,
    projects: Vec<Project>,
    selected_index: usize, // index into filtered list
    scroll_offset: usize,
    initial_load_complete: bool,
    is_focused: bool,
    is_hidden: bool,
    verbosity: Verbosity,

    // Search + Browse mode
    search_query: String,
    browse_mode: bool, // true = browsing all projects to find/start one

    // Discovery mode
    scan_dir: Option<String>,
    use_discovery: bool,
    discovered_dirs: Vec<(String, String)>,
    scan_complete: bool,
    has_session_data: bool,

    // Layout for new sessions
    session_layout: Option<String>,

    // Whether this instance owns keybinds (false for secondary instances in new tabs)
    is_primary: bool,

    // Attention tracking — sessions that need user attention
    attention_sessions: BTreeSet<String>,

    // Cached session statuses
    cached_statuses: BTreeMap<String, SessionStatus>,

    // Metadata polling
    cached_metadata: BTreeMap<String, ProjectMetadata>,
    pending_commands: usize,
    poll_tick: usize,

    // AI state — stored separately so SessionUpdate never wipes it
    ai_states: BTreeMap<String, AgentState>,
    ai_state_since: BTreeMap<String, u64>, // unix timestamp when state started
}

impl Default for State {
    fn default() -> Self {
        Self {
            permissions_granted: false,
            projects: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            initial_load_complete: false,
            is_focused: false,
            is_hidden: false,
            verbosity: Verbosity::default(),
            search_query: String::new(),
            browse_mode: false,
            scan_dir: None,
            use_discovery: false,
            discovered_dirs: Vec::new(),
            scan_complete: false,
            has_session_data: false,
            session_layout: None,
            is_primary: true,
            attention_sessions: BTreeSet::new(),
            cached_statuses: BTreeMap::new(),
            cached_metadata: BTreeMap::new(),
            pending_commands: 0,
            poll_tick: 0,
            ai_states: BTreeMap::new(),
            ai_state_since: BTreeMap::new(),
        }
    }
}

register_plugin!(State);

// --- Helpers ---

fn extract_active_command(session: &SessionInfo) -> Option<String> {
    session.tabs.iter()
        .find(|t| t.active)
        .and_then(|active_tab| {
            session.panes.panes.get(&active_tab.position)
                .and_then(|panes| {
                    panes.iter()
                        .find(|p| p.is_focused && !p.is_plugin && !p.is_suppressed)
                        .and_then(|pane| {
                            pane.terminal_command.as_ref()
                                .map(|cmd| {
                                    PathBuf::from(cmd)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or(cmd)
                                        .to_string()
                                })
                        })
                })
        })
}

/// Fuzzy subsequence match — all query chars must appear in order in the name
fn fuzzy_matches(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let name_lower = name.to_lowercase();
    let mut name_chars = name_lower.chars();
    for qc in query.to_lowercase().chars() {
        loop {
            match name_chars.next() {
                Some(nc) if nc == qc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

fn render_progress_bar(pct: u8, width: usize) -> String {
    let filled = ((pct as usize) * width) / 100;
    let empty = width.saturating_sub(filled);
    "━".repeat(filled) + &"░".repeat(empty)
}

// --- State Methods ---

impl State {
    /// Get indices into self.projects visible in current mode
    fn filtered_indices(&self) -> Vec<usize> {
        if self.use_discovery {
            if self.browse_mode {
                // Browse mode: all projects, filtered by search
                self.projects.iter().enumerate()
                    .filter(|(_, p)| fuzzy_matches(&p.name, &self.search_query))
                    .map(|(i, _)| i)
                    .collect()
            } else {
                // Normal mode: only projects with active sessions (Running or Exited)
                self.projects.iter().enumerate()
                    .filter(|(_, p)| !matches!(p.status, SessionStatus::NotStarted))
                    .map(|(i, _)| i)
                    .collect()
            }
        } else {
            // Legacy mode: show all
            (0..self.projects.len()).collect()
        }
    }

    /// Resolve selected_index (into filtered list) to actual project index
    fn selected_project_index(&self) -> Option<usize> {
        let filtered = self.filtered_indices();
        filtered.get(self.selected_index).copied()
    }

    fn activate_selected_project(&mut self) {
        if let Some(idx) = self.selected_project_index() {
            let project = &self.projects[idx];
            // Clear attention when switching to a session
            self.attention_sessions.remove(&project.name);
            match &project.status {
                SessionStatus::Running { .. } | SessionStatus::Exited => {
                    switch_session(Some(&project.name));
                }
                SessionStatus::NotStarted => {
                    if let Some(ref layout_path) = self.session_layout {
                        switch_session_with_layout(
                            Some(&project.name),
                            LayoutInfo::File(layout_path.clone()),
                            Some(PathBuf::from(&project.path)),
                        );
                    } else {
                        switch_session_with_cwd(
                            Some(&project.name),
                            Some(PathBuf::from(&project.path)),
                        );
                    }
                }
            }
            self.browse_mode = false;
            self.search_query.clear();
            self.selected_index = 0;
            self.scroll_offset = 0;
            set_selectable(false);
            self.is_focused = false;
        }
    }

    fn kill_selected_session(&mut self) {
        if let Some(idx) = self.selected_project_index() {
            let project = &self.projects[idx];
            match &project.status {
                SessionStatus::Running { is_current: true, .. } => {
                    eprintln!("Cannot kill current session '{}'", project.name);
                }
                SessionStatus::Running { is_current: false, .. } => {
                    kill_sessions(&[project.name.clone()]);
                }
                _ => {}
            }
        }
    }

    fn setup_toggle_keybind(&self) {
        let plugin_id = get_plugin_ids().plugin_id;
        let config = format!(
            r#"
keybinds {{
    shared {{
        bind "Super o" {{
            MessagePluginId {plugin_id} {{
                name "toggle_sidebar"
            }}
        }}
        bind "Super t" {{
            MessagePluginId {plugin_id} {{
                name "new_tab_with_sidebar"
            }}
        }}
    }}
}}
"#,
        );
        reconfigure(config, false);
        eprintln!("Keybinds registered for plugin {}: Super+o (toggle), Super+t (new tab)", plugin_id);
    }

    fn create_tab_with_sidebar(&self) {
        let scan_dir = self.scan_dir.as_deref().unwrap_or("");
        let session_layout = self.session_layout.as_deref().unwrap_or("");

        let layout = if self.use_discovery {
            format!(
                r#"
layout {{
    pane size=1 borderless=true {{
        plugin location="zellij:tab-bar"
    }}
    pane split_direction="vertical" {{
        pane size="15%" name="Projects" {{
            plugin location="file:~/.config/zellij/plugins/zellij-project-sidebar.wasm" {{
                scan_dir "{scan_dir}"
                session_layout "{session_layout}"
                is_primary "false"
            }}
        }}
        pane
    }}
    pane size=1 borderless=true {{
        plugin location="file:~/.config/zellij/plugins/zellij-attention.wasm" {{
            enabled "true"
            waiting_icon "⏳"
            completed_icon "✅"
        }}
    }}
}}
"#
            )
        } else {
            // Legacy mode — plain tab
            String::from(
                r#"
layout {
    pane
}
"#
            )
        };

        new_tabs_with_layout(&layout);
        eprintln!("Created new tab with sidebar layout");
    }

    fn toggle_visibility(&mut self) {
        if self.is_focused {
            self.search_query.clear();
            self.browse_mode = false;
            set_selectable(false);
            self.is_focused = false;
            eprintln!("Sidebar deactivated");
        } else {
            set_selectable(true);
            focus_plugin_pane(get_plugin_ids().plugin_id, false);
            self.is_focused = true;
            eprintln!("Sidebar activated");
        }
    }

    // --- Discovery ---

    fn trigger_scan(&self) {
        if let Some(ref dir) = self.scan_dir {
            let mut ctx = BTreeMap::new();
            ctx.insert(CMD_KEY.to_string(), CMD_SCAN_DIR.to_string());
            run_command(
                &["find", dir, "-maxdepth", "1", "-mindepth", "1", "-type", "d", "-not", "-name", ".*"],
                ctx,
            );
            eprintln!("Scanning directory: {}", dir);
        }
    }

    fn rebuild_projects(&mut self) {
        if !self.use_discovery {
            return;
        }

        let selected_name = self.selected_project_index()
            .map(|idx| self.projects[idx].name.clone());

        self.projects = self.discovered_dirs.iter()
            .map(|(name, path)| {
                let status = self.cached_statuses
                    .get(name)
                    .cloned()
                    .unwrap_or(SessionStatus::NotStarted);
                let metadata = self.cached_metadata.get(name).cloned()
                    .unwrap_or_default();
                Project {
                    name: name.clone(),
                    path: path.clone(),
                    status,
                    metadata,
                }
            })
            .collect();

        self.projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        // Restore selection to same project name within filtered view
        if let Some(name) = selected_name {
            let filtered = self.filtered_indices();
            if let Some(fi) = filtered.iter().position(|&i| self.projects[i].name == name) {
                self.selected_index = fi;
            }
        }
        self.clamp_selection();

        if self.scan_complete && self.has_session_data {
            self.initial_load_complete = true;
        }
    }

    fn update_cached_statuses(
        &mut self,
        sessions: &[SessionInfo],
        resurrectable: &[(String, Duration)],
    ) {
        self.cached_statuses.clear();
        for session in sessions {
            let tab_count = session.tabs.len();
            let active_command = extract_active_command(session);
            self.cached_statuses.insert(
                session.name.clone(),
                SessionStatus::Running {
                    is_current: session.is_current_session,
                    tab_count,
                    active_command,
                },
            );
        }
        for (name, _) in resurrectable {
            if !self.cached_statuses.contains_key(name) {
                self.cached_statuses.insert(name.clone(), SessionStatus::Exited);
            }
        }
    }

    fn apply_cached_statuses(&mut self) {
        for project in &mut self.projects {
            project.status = self.cached_statuses
                .get(&project.name)
                .cloned()
                .unwrap_or(SessionStatus::NotStarted);
        }
    }

    fn poll_git_branches(&mut self) {
        for project in &self.projects {
            if !matches!(project.status, SessionStatus::Running { .. }) {
                continue;
            }
            if project.path.is_empty() {
                continue;
            }
            // Skip projects we know are not git repos (until session restarts)
            if project.metadata.is_git_repo == Some(false) {
                continue;
            }
            let mut ctx = BTreeMap::new();
            ctx.insert(CMD_KEY.to_string(), CMD_GIT_BRANCH.to_string());
            ctx.insert(PROJECT_KEY.to_string(), project.name.clone());
            run_command_with_env_variables_and_cwd(
                &["git", "rev-parse", "--abbrev-ref", "HEAD"],
                BTreeMap::new(),
                PathBuf::from(&project.path),
                ctx,
            );
            self.pending_commands += 1;
        }
        if self.pending_commands == 0 {
            // No running projects to poll, re-arm timer immediately
            set_timeout(10.0);
        }
    }

    fn apply_cached_metadata(&mut self) {
        let keys_with_agent: Vec<_> = self.cached_metadata.iter()
            .filter(|(_, m)| !matches!(m.agent.state, AgentState::Unknown))
            .map(|(k, m)| format!("{}={:?}", k, m.agent.state))
            .collect();
        if !keys_with_agent.is_empty() {
            eprintln!("apply_cached_metadata: AI states in cache: {:?}", keys_with_agent);
            let project_names: Vec<_> = self.projects.iter().map(|p| p.name.clone()).collect();
            eprintln!("apply_cached_metadata: project names: {:?}", project_names);
        }
        for project in &mut self.projects {
            if let Some(meta) = self.cached_metadata.get(&project.name) {
                project.metadata = meta.clone();
            }
        }
    }

    fn handle_git_branch_result(
        &mut self,
        exit_code: Option<i32>,
        stdout: &[u8],
        context: &BTreeMap<String, String>,
    ) -> bool {
        if let Some(project_name) = context.get(PROJECT_KEY) {
            let meta = self.cached_metadata.entry(project_name.clone()).or_default();
            if exit_code == Some(0) {
                let branch = String::from_utf8_lossy(stdout).trim().to_string();
                meta.is_git_repo = Some(true);
                let changed = meta.git_branch.as_ref() != Some(&branch);
                meta.git_branch = Some(branch);
                if changed {
                    self.apply_cached_metadata();
                }
                return changed;
            } else {
                // Non-zero exit = not a git repo (or git not installed)
                meta.is_git_repo = Some(false);
                meta.git_branch = None;
                return false;
            }
        }
        false
    }

    fn clamp_selection(&mut self) {
        let filtered_len = self.filtered_indices().len();
        if filtered_len == 0 {
            self.selected_index = 0;
        } else {
            self.selected_index = self.selected_index.min(filtered_len - 1);
        }
    }

    fn build_render_lines(&self) -> Vec<RenderLine> {
        let mut lines = Vec::new();
        let filtered = self.filtered_indices();

        if self.browse_mode && !filtered.is_empty() {
            lines.push(RenderLine::Header("All projects".to_string()));
        }

        let total = filtered.len();
        for (fi, &i) in filtered.iter().enumerate() {
            let project = &self.projects[i];

            if fi == 0 {
                lines.push(RenderLine::CardTop);
            } else {
                lines.push(RenderLine::CardDivider);
            }

            lines.push(RenderLine::ProjectRow(i));

            // Detail line when there's meaningful content
            let multi_tab = matches!(project.status, SessionStatus::Running { tab_count, .. } if tab_count > 1);
            let has_command = matches!(project.status, SessionStatus::Running { active_command: Some(_), .. });
            let ai_state = self.ai_states.get(&project.name);
            let has_claude = ai_state.is_some() && !matches!(ai_state, Some(AgentState::Unknown));
            let has_pills = !project.metadata.pills.is_empty();
            let has_progress = project.metadata.progress_pct.is_some();
            if multi_tab || has_command || has_claude || has_pills || has_progress {
                lines.push(RenderLine::ProjectDetail(i));
            }

            if fi == total - 1 {
                lines.push(RenderLine::CardBottom);
            }
        }

        lines
    }

    fn ensure_selection_visible(&mut self, render_lines: &[RenderLine], visible_rows: usize) {
        if visible_rows == 0 {
            return;
        }
        let selected_proj = self.selected_project_index();

        // Find the first and last render line belonging to the selected project
        let mut first_y: Option<usize> = None;
        let mut last_y: Option<usize> = None;
        for (y, line) in render_lines.iter().enumerate() {
            if line.project_index() == selected_proj && selected_proj.is_some() {
                if first_y.is_none() {
                    first_y = Some(y);
                }
                last_y = Some(y);
            }
        }

        if let (Some(first), Some(last)) = (first_y, last_y) {
            // Scroll up if card starts above viewport
            if first < self.scroll_offset {
                self.scroll_offset = first;
            }
            // Scroll down if card ends below viewport
            if last >= self.scroll_offset + visible_rows {
                self.scroll_offset = last.saturating_sub(visible_rows - 1);
            }
        }
    }

    fn render_project_name_line(&self, project: &Project, is_selected: bool, cols: usize) -> Text {
        let needs_attention = self.attention_sessions.contains(&project.name);
        let is_current_session = matches!(&project.status, SessionStatus::Running { is_current: true, .. });

        // Determine icon + color based on state priority
        let ai_state = self.ai_states.get(&project.name);
        let (status_icon, dot_color) = if needs_attention {
            ("!", COLOR_MAGENTA)      // ! = needs attention (magenta)
        } else {
            match ai_state {
                Some(AgentState::Active) => ("▶", COLOR_GREEN),    // ▶ = Claude working (green)
                Some(AgentState::Waiting) | Some(AgentState::Idle) =>
                    ("■", COLOR_CYAN),                             // ■ = Claude stopped (cyan)
                _ => ("·", COLOR_ORANGE),                          // · = no AI state
            }
        };

        // Name line: "│ ✦ name                    │"
        let content = format!(" {} {}", status_icon, project.name);
        let inner_width = cols.saturating_sub(2);
        let padded: String = if content.chars().count() > inner_width {
            content.chars().take(inner_width.saturating_sub(1)).collect::<String>() + "…"
        } else {
            format!("{:<width$}", content, width = inner_width)
        };
        let display_line = format!("│{}│", padded);

        let mut text = Text::new(&display_line);

        if is_selected {
            text = text.selected();
        }

        let line_len = display_line.chars().count();

        // First color wins per character — apply most specific first
        // Icon color (starts at char 2, length varies: "▶"=1, "??"=2, "!!"=2)
        let icon_end = 2 + status_icon.chars().count();
        text = text.color_range(dot_color, 2..icon_end);
        // Borders (cyan to match Zellij frame)
        text = text.color_range(COLOR_CYAN, 0..1);
        text = text.color_range(COLOR_CYAN, line_len.saturating_sub(1)..line_len);

        if is_current_session {
            // Green the name text (after icon, before right border)
            text = text.color_range(COLOR_GREEN, 4..line_len.saturating_sub(1));
        } else if matches!(project.status, SessionStatus::NotStarted | SessionStatus::Exited) {
            // Dim inactive projects — they're background context
            text = text.color_range(COLOR_CYAN, 4..line_len.saturating_sub(1));
        }
        // Running non-current: name text stays default (full contrast, these matter)

        text
    }

    fn render_detail_line(&self, project: &Project, is_selected: bool, cols: usize) -> Text {
        let mut content = String::from("   "); // indent past dot, aligned under name
        let mut segments: Vec<(usize, usize, usize)> = Vec::new(); // (start, end, color) — indices relative to final display_line
        let mut has_content = false;

        // Order: claude indicator → active command → tab count

        // Claude indicator — show for ANY session with AI state (not just current)
        let ai_state = self.ai_states.get(&project.name);
        if ai_state.is_some() && !matches!(ai_state, Some(AgentState::Unknown)) {
            let elapsed = self.format_elapsed(&project.name);
            let label = if elapsed.is_empty() {
                "claude".to_string()
            } else {
                format!("claude · {}", elapsed)
            };
            let detail_color = match ai_state.unwrap() {
                AgentState::Active => COLOR_GREEN,
                _ => COLOR_CYAN,
            };
            let start = content.chars().count() + 1; // +1 for left │
            content.push_str(&label);
            let end = content.chars().count() + 1;
            segments.push((start, end, detail_color));
            has_content = true;
        } else if let SessionStatus::Running { active_command: Some(cmd), .. } = &project.status {
            // Fallback: show active_command from Zellij API (any session)
            if has_content { content.push_str(" · "); }
            let start = content.chars().count() + 1;
            content.push_str(cmd);
            let end = content.chars().count() + 1;
            segments.push((start, end, COLOR_ORANGE));
            has_content = true;
        }

        // Tab count — only show when >1
        if let SessionStatus::Running { tab_count, .. } = &project.status {
            if *tab_count > 1 {
                if has_content { content.push_str(" · "); }
                content.push_str(&format!("{} tabs", tab_count));
                has_content = true;
            }
        }

        // Git branch — disabled for now, may return on a dedicated third line

        // Pills: render as key:value pairs (limit to first 3)
        for (key, value) in project.metadata.pills.iter().take(3) {
            if has_content { content.push_str(" · "); }
            content.push_str(&format!("{}:{}", key, value));
            has_content = true;
        }

        // Progress bar: only render if there is room
        if let Some(pct) = project.metadata.progress_pct {
            let bar = render_progress_bar(pct, 7);
            let progress_str = format!(" {} {}%", bar, pct);
            if content.chars().count() + progress_str.chars().count() + 2 < cols {
                if has_content { content.push_str("  "); }
                content.push_str(&progress_str);
            }
        }

        // Pad and wrap with borders: "│content          │"
        let inner_width = cols.saturating_sub(2);
        let padded: String = if content.chars().count() > inner_width {
            content.chars().take(inner_width.saturating_sub(1)).collect::<String>() + "…"
        } else {
            format!("{:<width$}", content, width = inner_width)
        };
        let display_line = format!("│{}│", padded);

        let mut text = Text::new(&display_line);

        if is_selected {
            text = text.selected();
        }

        let is_current_session = matches!(&project.status, SessionStatus::Running { is_current: true, .. });
        let line_len = display_line.chars().count();

        // First color wins — apply specific segments first, then borders
        if is_current_session {
            text = text.color_range(COLOR_GREEN, 1..line_len.saturating_sub(1));
        } else {
            // Colored segments first (e.g., active command in cyan)
            for (start, end, color) in &segments {
                if *end < line_len {
                    text = text.color_range(*color, *start..*end);
                }
            }
            // Detail text stays default (uncolored = theme foreground, naturally dimmer than names)
        }
        // Borders last (cyan to match Zellij frame)
        text = text.color_range(COLOR_CYAN, 0..1);
        text = text.color_range(COLOR_CYAN, line_len.saturating_sub(1)..line_len);

        text
    }

    fn save_ai_states(&self) {
        // Write per-session files for cross-session visibility
        let _ = std::fs::create_dir_all("/tmp/sidebar-ai");
        for (session, state) in &self.ai_states {
            let state_str = match state {
                AgentState::Active => "active",
                AgentState::Idle => "idle",
                AgentState::Waiting => "waiting",
                AgentState::Unknown => "unknown",
            };
            let ts = self.ai_state_since.get(session).copied().unwrap_or(0);
            let _ = std::fs::write(format!("/tmp/sidebar-ai/{}", session), format!("{} {}", state_str, ts));
        }
    }

    fn load_ai_states(&mut self) {
        // Read per-session files: format is "state timestamp" (e.g. "active 1710460800")
        if let Ok(entries) = std::fs::read_dir("/tmp/sidebar-ai") {
            for entry in entries.flatten() {
                if let Some(session) = entry.file_name().to_str().map(|s| s.to_string()) {
                    if let Ok(data) = std::fs::read_to_string(entry.path()) {
                        let parts: Vec<&str> = data.trim().splitn(2, ' ').collect();
                        let state = match parts.first().copied() {
                            Some("active") => AgentState::Active,
                            Some("idle") => AgentState::Idle,
                            Some("waiting") => AgentState::Waiting,
                            _ => continue,
                        };
                        self.ai_states.insert(session.clone(), state);
                        if let Some(ts_str) = parts.get(1) {
                            if let Ok(ts) = ts_str.parse::<u64>() {
                                self.ai_state_since.insert(session, ts);
                            }
                        }
                    }
                }
            }
        }
    }

    fn now_secs(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn format_elapsed(&self, session: &str) -> String {
        if let Some(&since) = self.ai_state_since.get(session) {
            let elapsed = self.now_secs().saturating_sub(since);
            if elapsed < 60 {
                format!("{}s", elapsed)
            } else if elapsed < 3600 {
                format!("{}m", elapsed / 60)
            } else {
                format!("{}h", elapsed / 3600)
            }
        } else {
            String::new()
        }
    }
}

// --- Plugin Lifecycle ---

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        if let Some(verbosity_str) = configuration.get("verbosity") {
            self.verbosity = match verbosity_str.as_str() {
                "minimal" => Verbosity::Minimal,
                "full" => Verbosity::Full,
                other => {
                    eprintln!("WARNING: Unknown verbosity '{}', defaulting to 'full'", other);
                    Verbosity::Full
                }
            };
        }

        self.scan_dir = configuration.get("scan_dir").cloned();
        self.session_layout = configuration.get("session_layout").cloned();
        self.is_primary = configuration.get("is_primary").map(|v| v != "false").unwrap_or(true);
        self.use_discovery = self.scan_dir.is_some();

        if self.use_discovery {
            eprintln!("Discovery mode: scan_dir={:?}", self.scan_dir);
        } else {
            let mut i = 0;
            while let Some(path_str) = configuration.get(&format!("project_{}", i)) {
                let path = PathBuf::from(path_str);
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                if path_str.starts_with('~') {
                    eprintln!(
                        "WARNING: project_{} uses tilde path '{}'. Use absolute paths.",
                        i, path_str
                    );
                }

                self.projects.push(Project {
                    name,
                    path: path_str.clone(),
                    status: SessionStatus::NotStarted,
                    metadata: ProjectMetadata::default(),
                });
                i += 1;
            }

            let names: Vec<&str> = self.projects.iter().map(|p| p.name.as_str()).collect();
            for (idx, name) in names.iter().enumerate() {
                if names[idx + 1..].contains(name) {
                    eprintln!(
                        "WARNING: Duplicate project basename '{}'. Session matching will be ambiguous.",
                        name
                    );
                }
            }

            eprintln!("Legacy mode: loaded {} projects", self.projects.len());
        }

        let permissions = vec![
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::Reconfigure,
            PermissionType::RunCommands, // Always needed for git polling
        ];
        request_permission(&permissions);

        let events = vec![
            EventType::SessionUpdate,
            EventType::PermissionRequestResult,
            EventType::Key,
            EventType::Mouse,
            EventType::Timer,            // Needed for metadata polling
            EventType::RunCommandResult, // Needed for git polling + discovery scan
        ];
        subscribe(&events);

        // Ensure pane is focusable so user can accept the permissions dialog
        set_selectable(true);

        // Restore persisted AI state so cross-session dots are visible immediately
        self.load_ai_states();

        eprintln!("Plugin loaded, requesting permissions");
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.permissions_granted = true;
                set_selectable(false);
                if self.is_primary {
                    self.setup_toggle_keybind();
                }
                if self.use_discovery {
                    self.trigger_scan();
                }
                // Start polling timer (first poll after 2 seconds)
                set_timeout(2.0);
                eprintln!("Permissions granted, sidebar set to unselectable, polling timer started");
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                eprintln!("Permissions denied — plugin cannot function");
                false
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                match context.get(CMD_KEY).map(|s| s.as_str()) {
                    Some(CMD_SCAN_DIR) => {
                        if exit_code == Some(0) {
                            let output = String::from_utf8_lossy(&stdout);
                            self.discovered_dirs = output
                                .lines()
                                .filter(|line| !line.is_empty())
                                .map(|full_path| {
                                    let name = PathBuf::from(full_path)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    (name, full_path.to_string())
                                })
                                .collect();
                            eprintln!("Discovered {} directories", self.discovered_dirs.len());
                        } else {
                            eprintln!(
                                "scan_dir failed (exit {:?}): {}",
                                exit_code,
                                String::from_utf8_lossy(&stderr)
                            );
                        }
                        self.scan_complete = true;
                        self.rebuild_projects();
                        true
                    }
                    Some(CMD_GIT_BRANCH) => {
                        let changed = self.handle_git_branch_result(exit_code, &stdout, &context);
                        if self.pending_commands > 0 {
                            self.pending_commands -= 1;
                        }
                        // Re-arm timer when all results are in
                        if self.pending_commands == 0 {
                            eprintln!("All git commands complete, re-arming timer");
                            set_timeout(10.0);
                        }
                        changed
                    }
                    _ => false,
                }
            }
            Event::SessionUpdate(sessions, resurrectable) => {
                if self.use_discovery {
                    self.update_cached_statuses(&sessions, &resurrectable);
                    self.has_session_data = true;

                    // Keep metadata for all known sessions (not just running)
                    // AI state from pipe messages must survive SessionUpdate cycles
                    let known_names: BTreeSet<String> = self.cached_statuses.keys().cloned().collect();
                    self.cached_metadata.retain(|name, _| known_names.contains(name));
                    // Prune stale AI states for sessions that no longer exist
                    self.ai_states.retain(|name, _| known_names.contains(name));

                    if self.scan_complete {
                        self.apply_cached_statuses();
                        self.apply_cached_metadata();
                        self.initial_load_complete = true;
                    } else {
                        // Show live sessions immediately while scan runs in background.
                        // Default view only shows Running/Exited, which we have from SessionUpdate.
                        self.projects = self.cached_statuses.iter()
                            .filter(|(_, status)| !matches!(status, SessionStatus::NotStarted))
                            .map(|(name, status)| Project {
                                name: name.clone(),
                                path: String::new(),
                                status: status.clone(),
                                metadata: ProjectMetadata::default(),
                            })
                            .collect();
                        self.projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                        self.apply_cached_metadata(); // Preserve AI state from pipe messages
                        self.clamp_selection();
                        self.initial_load_complete = true;
                    }
                } else {
                    for project in &mut self.projects {
                        if let Some(session) = sessions.iter().find(|s| s.name == project.name) {
                            let tab_count = session.tabs.len();
                            let active_command = extract_active_command(session);
                            project.status = SessionStatus::Running {
                                is_current: session.is_current_session,
                                tab_count,
                                active_command,
                            };
                        } else if resurrectable.iter().any(|(name, _)| name == &project.name) {
                            project.status = SessionStatus::Exited;
                        } else {
                            project.status = SessionStatus::NotStarted;
                        }
                    }
                    // Prune stale AI states for sessions that no longer exist
                    let active_session_names: BTreeSet<String> = sessions.iter().map(|s| s.name.clone()).collect();
                    let resurrectable_names: BTreeSet<String> = resurrectable.iter().map(|(n, _)| n.clone()).collect();
                    self.ai_states.retain(|name, _| active_session_names.contains(name) || resurrectable_names.contains(name));
                    self.initial_load_complete = true;
                }
                // Auto-track current session when sidebar is not actively navigated
                if !self.is_focused {
                    let filtered = self.filtered_indices();
                    if let Some(fi) = filtered.iter().position(|&i| {
                        matches!(self.projects[i].status, SessionStatus::Running { is_current: true, .. })
                    }) {
                        self.selected_index = fi;
                    }
                }
                true
            }
            Event::Mouse(mouse) => {
                match mouse {
                    Mouse::LeftClick(line, _col) => {
                        let click_y = line as usize;
                        let y_offset: usize = if self.browse_mode { 1 } else { 0 };

                        if click_y < y_offset {
                            // Clicked on search bar — ignore
                            return true;
                        }

                        let render_lines = self.build_render_lines();
                        let render_idx = self.scroll_offset + (click_y - y_offset);

                        if render_idx < render_lines.len() {
                            if let Some(project_idx) = render_lines[render_idx].project_index() {
                                let filtered = self.filtered_indices();
                                if let Some(fi) = filtered.iter().position(|&i| i == project_idx) {
                                    self.selected_index = fi;
                                    self.activate_selected_project();
                                }
                            }
                        }
                        true
                    }
                    Mouse::ScrollUp(_) => {
                        self.selected_index = self.selected_index.saturating_sub(1);
                        true
                    }
                    Mouse::ScrollDown(_) => {
                        let filtered_len = self.filtered_indices().len();
                        if filtered_len > 0 {
                            self.selected_index = (self.selected_index + 1)
                                .min(filtered_len.saturating_sub(1));
                        }
                        true
                    }
                    _ => false,
                }
            }
            Event::Key(key) => match key.bare_key {
                // --- Navigation (always works) ---
                BareKey::Down if key.has_no_modifiers() => {
                    let filtered_len = self.filtered_indices().len();
                    if filtered_len > 0 {
                        self.selected_index = (self.selected_index + 1)
                            .min(filtered_len.saturating_sub(1));
                    }
                    true
                }
                BareKey::Up if key.has_no_modifiers() => {
                    self.selected_index = self.selected_index.saturating_sub(1);
                    true
                }
                BareKey::Enter if key.has_no_modifiers() => {
                    self.activate_selected_project();
                    true
                }
                BareKey::Esc if key.has_no_modifiers() => {
                    if self.browse_mode {
                        // Exit browse mode
                        self.browse_mode = false;
                        self.search_query.clear();
                        self.selected_index = 0;
                        self.scroll_offset = 0;
                        eprintln!("Exited browse mode");
                    } else {
                        // Deactivate sidebar
                        set_selectable(false);
                        self.is_focused = false;
                        eprintln!("Sidebar deactivated");
                    }
                    true
                }
                BareKey::Backspace if key.has_no_modifiers() => {
                    if self.browse_mode && !self.search_query.is_empty() {
                        self.search_query.pop();
                        self.selected_index = 0;
                        self.scroll_offset = 0;
                    }
                    true
                }

                // --- Commands ---
                BareKey::Delete if key.has_no_modifiers() => {
                    if !self.browse_mode {
                        self.kill_selected_session();
                    }
                    true
                }
                BareKey::Char('r') if key.has_modifiers(&[KeyModifier::Alt]) => {
                    if self.use_discovery {
                        self.scan_complete = false;
                        self.trigger_scan();
                    }
                    true
                }

                // --- `/` enters browse mode (discovery only) ---
                BareKey::Char('/') if key.has_no_modifiers() && !self.browse_mode => {
                    if self.use_discovery {
                        self.browse_mode = true;
                        self.search_query.clear();
                        self.selected_index = 0;
                        self.scroll_offset = 0;
                        eprintln!("Entered browse mode");
                    }
                    true
                }

                // --- Search typing (browse mode only) ---
                BareKey::Char(c) if key.has_no_modifiers() && self.browse_mode => {
                    self.search_query.push(c);
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                    true
                }

                _ => false,
            },
            Event::Timer(_elapsed) => {
                // Refresh cross-session AI state from /cache on every tick
                self.load_ai_states();
                if self.pending_commands == 0 {
                    self.poll_tick += 1;
                    self.poll_git_branches();
                    eprintln!("Poll tick {} -- dispatched git commands (pending: {})", self.poll_tick, self.pending_commands);
                } else {
                    // Commands still pending from last cycle, skip this tick
                    eprintln!("Poll tick skipped -- {} commands still pending", self.pending_commands);
                }
                true // re-render to show updated cross-session AI state
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if !self.permissions_granted {
            println!("Waiting for permissions...");
            return;
        }


        if !self.initial_load_complete {
            return; // Render nothing until first SessionUpdate arrives
        }

        if self.projects.is_empty() {
            if self.use_discovery {
                println!("No projects found.");
            } else {
                println!("No projects configured.");
            }
            return;
        }

        let mut y_offset: usize = 0;

        // Search bar (browse mode)
        if self.browse_mode {
            let search_line = if self.search_query.is_empty() {
                " / search...".to_string()
            } else {
                format!(" / {}", self.search_query)
            };
            let display: String = if search_line.chars().count() > cols {
                search_line.chars().take(cols).collect()
            } else {
                search_line
            };
            let text = Text::new(&display).color_range(COLOR_CYAN, 0..display.chars().count());
            print_text_with_coordinates(text, 0, 0, Some(cols), None);
            y_offset = 1;
        }

        let render_lines = self.build_render_lines();

        // Empty states
        if render_lines.is_empty() {
            let msg = if self.browse_mode {
                " No matches"
            } else {
                " No active sessions"
            };
            let text = Text::new(msg).color_all(COLOR_CYAN);
            print_text_with_coordinates(text, 0, y_offset, Some(cols), None);

            // Still show footer with hint
            let footer_y = rows.saturating_sub(1);
            if footer_y > y_offset {
                let hint = if self.is_focused && self.use_discovery {
                    " /:browse"
                } else if !self.is_focused {
                    " ⌘O to toggle"
                } else {
                    ""
                };
                if !hint.is_empty() {
                    let hint_text = Text::new(hint).color_all(COLOR_CYAN);
                    print_text_with_coordinates(hint_text, 0, footer_y, Some(cols), None);
                }
            }
            return;
        }

        let content_area = rows.saturating_sub(1).saturating_sub(y_offset); // reserve footer + search bar

        self.ensure_selection_visible(&render_lines, content_area);

        let visible_end = (self.scroll_offset + content_area).min(render_lines.len());

        for (i, line_idx) in (self.scroll_offset..visible_end).enumerate() {
            let screen_y = i + y_offset;
            match &render_lines[line_idx] {
                RenderLine::Header(title) => {
                    let header = format!(" ─ {}", title);
                    let header_line: String = if header.chars().count() > cols {
                        header.chars().take(cols).collect()
                    } else {
                        header
                    };
                    let text = Text::new(&header_line).color_all(COLOR_CYAN);
                    print_text_with_coordinates(text, 0, screen_y, Some(cols), None);
                }
                RenderLine::ProjectRow(project_idx) => {
                    let project = &self.projects[*project_idx];
                    let is_selected = self.selected_project_index() == Some(*project_idx);
                    let text = self.render_project_name_line(project, is_selected, cols);
                    print_text_with_coordinates(text, 0, screen_y, Some(cols), None);
                }
                RenderLine::ProjectDetail(project_idx) => {
                    let project = &self.projects[*project_idx];
                    let is_selected = self.selected_project_index() == Some(*project_idx);
                    let text = self.render_detail_line(project, is_selected, cols);
                    print_text_with_coordinates(text, 0, screen_y, Some(cols), None);
                }
                RenderLine::CardTop => {
                    let inner_width = cols.saturating_sub(2);
                    let rule = format!("╭{}╮", "─".repeat(inner_width));
                    let display: String = rule.chars().take(cols).collect();
                    let text = Text::new(&display).color_all(COLOR_CYAN);
                    print_text_with_coordinates(text, 0, screen_y, Some(cols), None);
                }
                RenderLine::CardBottom => {
                    let inner_width = cols.saturating_sub(2);
                    let rule = format!("╰{}╯", "─".repeat(inner_width));
                    let display: String = rule.chars().take(cols).collect();
                    let text = Text::new(&display).color_all(COLOR_CYAN);
                    print_text_with_coordinates(text, 0, screen_y, Some(cols), None);
                }
                RenderLine::CardDivider => {
                    let inner_width = cols.saturating_sub(2);
                    let rule = format!("├{}┤", "─".repeat(inner_width));
                    let display: String = rule.chars().take(cols).collect();
                    let text = Text::new(&display).color_all(COLOR_CYAN);
                    print_text_with_coordinates(text, 0, screen_y, Some(cols), None);
                }
            }
        }

        // Footer — pinned to bottom
        let footer_y = rows.saturating_sub(1);
        if footer_y > 0 {
            let hint = if !self.is_focused {
                " ⌘O to toggle"
            } else if self.browse_mode {
                " ↵:open esc:back"
            } else if self.use_discovery {
                " ↵:go /:browse del:kill"
            } else {
                " ↵:switch del:kill"
            };
            let hint_line: String = if hint.chars().count() > cols {
                hint.chars().take(cols).collect()
            } else {
                hint.to_string()
            };
            let hint_text = Text::new(&hint_line).color_all(COLOR_CYAN);
            print_text_with_coordinates(hint_text, 0, footer_y, Some(cols), None);
        }
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        match pipe_message.name.as_str() {
            "toggle_sidebar" => {
                self.toggle_visibility();
                true
            }
            "new_tab_with_sidebar" => {
                self.create_tab_with_sidebar();
                true
            }
            name if name.starts_with("sidebar::attention::") => {
                let session_name = name.strip_prefix("sidebar::attention::").unwrap_or("").to_string();
                if !session_name.is_empty() {
                    eprintln!("Attention flagged: {}", session_name);
                    self.attention_sessions.insert(session_name);
                }
                true
            }
            name if name.starts_with("sidebar::clear::") => {
                let session_name = name.strip_prefix("sidebar::clear::").unwrap_or("");
                self.attention_sessions.remove(session_name);
                eprintln!("Attention cleared: {}", session_name);
                true
            }
            "focus_sidebar" => {
                set_selectable(true);
                show_self(false);
                self.is_focused = true;
                self.is_hidden = false;
                eprintln!("Sidebar activated via pipe (legacy focus_sidebar)");
                true
            }
            // sidebar::ai-active::{session} / sidebar::ai-idle::{session} / sidebar::ai-waiting::{session}
            name if name.starts_with("sidebar::ai-active::") => {
                let session = name.strip_prefix("sidebar::ai-active::").unwrap_or("").to_string();
                if !session.is_empty() {
                    // Only reset timestamp if transitioning TO active
                    if !matches!(self.ai_states.get(&session), Some(AgentState::Active)) {
                        self.ai_state_since.insert(session.clone(), self.now_secs());
                    }
                    self.ai_states.insert(session, AgentState::Active);
                    self.save_ai_states();
                }
                true
            }
            name if name.starts_with("sidebar::ai-idle::") => {
                let session = name.strip_prefix("sidebar::ai-idle::").unwrap_or("").to_string();
                if !session.is_empty() {
                    self.ai_state_since.insert(session.clone(), self.now_secs());
                    self.ai_states.insert(session, AgentState::Idle);
                    self.save_ai_states();
                }
                true
            }
            name if name.starts_with("sidebar::ai-waiting::") => {
                let session = name.strip_prefix("sidebar::ai-waiting::").unwrap_or("").to_string();
                if !session.is_empty() {
                    self.ai_state_since.insert(session.clone(), self.now_secs());
                    self.ai_states.insert(session, AgentState::Waiting);
                    self.save_ai_states();
                }
                true
            }
            "sidebar::pill" => {
                let session = pipe_message.args.get("session").cloned();
                let key = pipe_message.args.get("key").cloned();
                let value = pipe_message.args.get("value").cloned();
                if let (Some(session), Some(key), Some(value)) = (session, key, value) {
                    let meta = self.cached_metadata.entry(session.clone()).or_default();
                    meta.pills.insert(key.clone(), value.clone());
                    eprintln!("Pill set: {}={} for {}", key, value, session);
                    self.apply_cached_metadata();
                    true
                } else {
                    false
                }
            }
            "sidebar::pill-clear" => {
                if let Some(session) = pipe_message.args.get("session").cloned() {
                    let meta = self.cached_metadata.entry(session.clone()).or_default();
                    if let Some(key) = pipe_message.args.get("key") {
                        meta.pills.remove(key);
                        eprintln!("Pill cleared: {} for {}", key, session);
                    } else {
                        meta.pills.clear();
                        eprintln!("All pills cleared for {}", session);
                    }
                    self.apply_cached_metadata();
                    true
                } else {
                    false
                }
            }
            "sidebar::progress" => {
                let session = pipe_message.args.get("session").cloned();
                let pct_str = pipe_message.args.get("pct").cloned();
                if let (Some(session), Some(pct_str)) = (session, pct_str) {
                    if let Ok(pct) = pct_str.parse::<u8>() {
                        let meta = self.cached_metadata.entry(session.clone()).or_default();
                        meta.progress_pct = if pct == 0 { None } else { Some(pct.min(100)) };
                        eprintln!("Progress set: {}% for {}", pct, session);
                        self.apply_cached_metadata();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "sidebar::progress-clear" => {
                if let Some(session) = pipe_message.args.get("session").cloned() {
                    let meta = self.cached_metadata.entry(session.clone()).or_default();
                    meta.progress_pct = None;
                    eprintln!("Progress cleared for {}", session);
                    self.apply_cached_metadata();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}
