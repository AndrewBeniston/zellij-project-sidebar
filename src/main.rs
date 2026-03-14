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

#[derive(Clone)]
struct Project {
    name: String,
    path: String,
    status: SessionStatus,
}

enum RenderLine {
    Header(String),
    ProjectRow(usize), // index into self.projects
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
                Project {
                    name: name.clone(),
                    path: path.clone(),
                    status,
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

        for &i in &filtered {
            lines.push(RenderLine::ProjectRow(i));
        }

        lines
    }

    fn ensure_selection_visible(&mut self, render_lines: &[RenderLine], visible_rows: usize) {
        if visible_rows == 0 {
            return;
        }
        let selected_proj = self.selected_project_index();
        let selected_y = render_lines.iter().position(|line| {
            match (line, selected_proj) {
                (RenderLine::ProjectRow(idx), Some(sel)) => *idx == sel,
                _ => false,
            }
        });
        if let Some(y) = selected_y {
            if y < self.scroll_offset {
                self.scroll_offset = y;
            }
            if y >= self.scroll_offset + visible_rows {
                self.scroll_offset = y - visible_rows + 1;
            }
        }
    }

    fn render_project_line(&self, project: &Project, is_selected: bool, cols: usize) -> Text {
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

        let mut permissions = vec![
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::Reconfigure,
        ];
        if self.use_discovery {
            permissions.push(PermissionType::RunCommands);
        }
        request_permission(&permissions);

        let mut events = vec![
            EventType::SessionUpdate,
            EventType::PermissionRequestResult,
            EventType::Key,
            EventType::Mouse,
        ];
        if self.use_discovery {
            events.push(EventType::RunCommandResult);
        }
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
                eprintln!("Permissions granted, sidebar set to unselectable");
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
                    _ => false,
                }
            }
            Event::SessionUpdate(sessions, resurrectable) => {
                if self.use_discovery {
                    self.update_cached_statuses(&sessions, &resurrectable);
                    self.has_session_data = true;
                    if self.scan_complete {
                        self.apply_cached_statuses();
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
                            if let RenderLine::ProjectRow(project_idx) = render_lines[render_idx] {
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
                    let text = self.render_project_line(project, is_selected, cols);
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
            _ => false,
        }
    }
}
