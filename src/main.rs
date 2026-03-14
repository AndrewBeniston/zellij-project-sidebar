use zellij_tile::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

// --- Catppuccin Frappe Palette ---
// Mapped to Zellij color_range index levels:
//   0 = green  (Catppuccin Frappe: #a6d189)
//   1 = cyan   (Catppuccin Frappe: #99d1db)  — unused currently
//   2 = red    (Catppuccin Frappe: #e78284)
//   3 = yellow (Catppuccin Frappe: #e5c890)
//   4 = blue   (Catppuccin Frappe: #8caaee)
//   5 = magenta(Catppuccin Frappe: #f4b8e4)  — unused currently
//   6 = orange (Catppuccin Frappe: #ef9f76)  — unused currently
//   7 = gray   (Catppuccin Frappe: #737994)  — for dim/stopped

const COLOR_GREEN: usize = 0;
#[allow(dead_code)]
const COLOR_RED: usize = 2;
const COLOR_YELLOW: usize = 3;
const COLOR_BLUE: usize = 4;
const COLOR_GRAY: usize = 7;

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

#[derive(Clone, PartialEq)]
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
    Separator,            // blank line between cards
}

impl RenderLine {
    fn project_index(&self) -> Option<usize> {
        match self {
            RenderLine::ProjectRow(idx) | RenderLine::ProjectDetail(idx) => Some(*idx),
            RenderLine::Header(_) | RenderLine::Separator => None,
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
            let active_command = if session.is_current_session {
                extract_active_command(session)
            } else {
                None
            };
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

        if self.use_discovery && self.browse_mode && !filtered.is_empty() {
            lines.push(RenderLine::Header("All projects".to_string()));
        }

        for (fi, &i) in filtered.iter().enumerate() {
            let project = &self.projects[i];
            lines.push(RenderLine::ProjectRow(i));

            // Detail line for projects with sessions (Running or Exited) — not NotStarted
            if !matches!(project.status, SessionStatus::NotStarted) {
                lines.push(RenderLine::ProjectDetail(i));
            }

            // Separator between cards (not after last)
            if fi < filtered.len() - 1 {
                lines.push(RenderLine::Separator);
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
        let status_dot = if needs_attention {
            "◆" // diamond for attention
        } else {
            match &project.status {
                SessionStatus::Running { .. } => "●",
                SessionStatus::Exited => "●",
                SessionStatus::NotStarted => "○",
            }
        };

        let line = match self.verbosity {
            Verbosity::Minimal => {
                format!(" {} {}", status_dot, project.name)
            }
            Verbosity::Full => {
                let mut parts = format!(" {} {}", status_dot, project.name);
                if let SessionStatus::Running { tab_count, .. } = &project.status {
                    parts.push_str(&format!(" [{}]", tab_count));
                }
                if let SessionStatus::Running {
                    is_current: true,
                    active_command: Some(cmd),
                    ..
                } = &project.status
                {
                    parts.push_str(&format!(" {}", cmd));
                }
                // Git branch is now rendered on the detail line — do NOT add it here
                parts
            }
        };

        let display_line: String = if line.chars().count() > cols {
            line.chars().take(cols.saturating_sub(1)).collect::<String>() + "…"
        } else {
            line
        };

        let mut text = Text::new(&display_line);
        let is_current_session = matches!(&project.status, SessionStatus::Running { is_current: true, .. });

        if is_selected {
            text = text.selected();
        }

        if needs_attention {
            // Red dot for attention needed
            text = text.color_range(COLOR_RED, 1..2);
        } else if is_current_session {
            // Highlight entire line green for the current session
            text = text.color_range(COLOR_GREEN, 0..display_line.chars().count());
        } else {
            // Just color the status dot
            match &project.status {
                SessionStatus::Running { .. } => {
                    text = text.color_range(COLOR_GREEN, 1..2);
                }
                SessionStatus::Exited => {
                    text = text.color_range(COLOR_YELLOW, 1..2);
                }
                SessionStatus::NotStarted => {
                    text = text.color_range(COLOR_GRAY, 1..2);
                }
            }
        }

        if self.verbosity == Verbosity::Full && !is_current_session {
            if let SessionStatus::Running { tab_count, is_current, active_command, .. } = &project.status {
                let name_end = 3 + project.name.chars().count();
                let bracket_str = format!("[{}]", tab_count);
                let bracket_start = name_end + 1;
                let bracket_end = bracket_start + bracket_str.chars().count() + 1;

                if display_line.chars().count() > bracket_start {
                    let actual_end = bracket_end.min(display_line.chars().count());
                    text = text.color_range(COLOR_GRAY, bracket_start..actual_end);
                }

                if *is_current {
                    if let Some(cmd) = active_command {
                        let cmd_start = bracket_end;
                        let cmd_end = cmd_start + cmd.chars().count() + 1;
                        if display_line.chars().count() > cmd_start {
                            let actual_end = cmd_end.min(display_line.chars().count());
                            text = text.color_range(COLOR_BLUE, cmd_start..actual_end);
                        }
                    }
                }
            }
        }

        text
    }

    fn render_detail_line(&self, project: &Project, is_selected: bool, cols: usize) -> Text {
        let mut parts = String::from("   "); // 3-space indent to align under project name

        // Git branch
        if let Some(ref branch) = project.metadata.git_branch {
            let display_branch = if branch == "HEAD" { "detached" } else { branch.as_str() };
            parts.push_str(&format!(" {}", display_branch));
        }

        // Future phases add pills, ports, progress here

        let display_line: String = if parts.chars().count() > cols {
            parts.chars().take(cols.saturating_sub(1)).collect::<String>() + "..."
        } else {
            parts
        };

        let mut text = Text::new(&display_line);

        if is_selected {
            text = text.selected();
        }

        // Color branch text in blue for non-current sessions, green for current
        let is_current_session = matches!(&project.status, SessionStatus::Running { is_current: true, .. });
        if is_current_session {
            // Current session: green across entire detail line (matches name line behavior)
            text = text.color_range(COLOR_GREEN, 0..display_line.chars().count());
        } else if project.metadata.git_branch.is_some() {
            // Non-current: blue for the branch portion (starting after the 3-space indent)
            let content_start = 3; // "   " indent
            text = text.color_range(COLOR_BLUE, content_start..display_line.chars().count());
        }

        text
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

                    // Clear cached metadata for sessions that are no longer running
                    let running_names: BTreeSet<String> = self.cached_statuses.iter()
                        .filter(|(_, s)| matches!(s, SessionStatus::Running { .. }))
                        .map(|(name, _)| name.clone())
                        .collect();
                    self.cached_metadata.retain(|name, _| running_names.contains(name));

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
                        self.clamp_selection();
                        self.initial_load_complete = true;
                    }
                } else {
                    for project in &mut self.projects {
                        if let Some(session) = sessions.iter().find(|s| s.name == project.name) {
                            let tab_count = session.tabs.len();
                            let active_command = if session.is_current_session {
                                extract_active_command(session)
                            } else {
                                None
                            };
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
                if self.pending_commands == 0 {
                    self.poll_tick += 1;
                    self.poll_git_branches();
                    eprintln!("Poll tick {} -- dispatched git commands (pending: {})", self.poll_tick, self.pending_commands);
                } else {
                    // Commands still pending from last cycle, skip this tick
                    eprintln!("Poll tick skipped -- {} commands still pending", self.pending_commands);
                }
                false // don't re-render on timer, wait for results
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
            if self.use_discovery {
                println!("Scanning...");
            } else {
                println!("Loading...");
            }
            return;
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
            let text = Text::new(&display).color_range(COLOR_BLUE, 0..display.chars().count());
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
            let text = Text::new(msg).color_all(COLOR_GRAY);
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
                    let hint_text = Text::new(hint).color_all(COLOR_GRAY);
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
                    let text = Text::new(&header_line).color_all(COLOR_GRAY);
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
                RenderLine::Separator => {
                    // Blank line — Zellij clears pane before render(), so no output needed
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
            let hint_text = Text::new(&hint_line).color_all(COLOR_GRAY);
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
            "sidebar::ai" => {
                if let Some(session) = pipe_message.args.get("session").cloned() {
                    let meta = self.cached_metadata.entry(session.clone()).or_default();
                    meta.agent.state = match pipe_message.args.get("state").map(|s| s.as_str()) {
                        Some("active") => AgentState::Active,
                        Some("idle") => AgentState::Idle,
                        Some("waiting") => AgentState::Waiting,
                        _ => AgentState::Unknown,
                    };
                    if let Some(tool) = pipe_message.args.get("tool") {
                        meta.agent.last_tool = Some(tool.clone());
                    }
                    eprintln!("AI state updated: {:?} for {}", pipe_message.args.get("state"), session);
                    self.apply_cached_metadata();
                    true
                } else {
                    false
                }
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
