use zellij_tile::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

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

// Semantic color constants (index_level for color_range)
const COLOR_GREEN: usize = 0;   // running sessions
#[allow(dead_code)]
const COLOR_RED: usize = 2;     // exited sessions (kept for reference)
const COLOR_YELLOW: usize = 3;  // exited sessions (semantic: warning/attention)
const COLOR_BLUE: usize = 4;    // info text (tab count, command)
const COLOR_GRAY: usize = 7;    // dim/stopped/not-started

// --- Verbosity ---

#[derive(Clone, PartialEq)]
enum Verbosity {
    Minimal,  // name + status dot only
    Full,     // name + status dot + tab count + active command
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

struct State {
    permissions_granted: bool,
    projects: Vec<Project>,
    selected_index: usize,
    initial_load_complete: bool,
    is_focused: bool,
    is_hidden: bool,
    verbosity: Verbosity,
}

impl Default for State {
    fn default() -> Self {
        Self {
            permissions_granted: false,
            projects: Vec::new(),
            selected_index: 0,
            initial_load_complete: false,
            is_focused: false,
            is_hidden: false,
            verbosity: Verbosity::default(),
        }
    }
}

register_plugin!(State);

// --- Session Actions ---

impl State {
    fn activate_selected_project(&mut self) {
        if let Some(project) = self.projects.get(self.selected_index) {
            match &project.status {
                SessionStatus::Running { .. } | SessionStatus::Exited => {
                    switch_session(Some(&project.name));
                }
                SessionStatus::NotStarted => {
                    switch_session_with_cwd(
                        Some(&project.name),
                        Some(PathBuf::from(&project.path)),
                    );
                }
            }
            // Deactivate sidebar after action
            set_selectable(false);
            self.is_focused = false;
        }
    }

    fn kill_selected_session(&mut self) {
        if let Some(project) = self.projects.get(self.selected_index) {
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
        bind "Super p" {{
            MessagePluginId {plugin_id} {{
                name "toggle_sidebar"
            }}
        }}
    }}
}}
"#,
        );
        reconfigure(config, false);
        eprintln!("Toggle keybind Super+p (Cmd+P) registered for plugin {}", plugin_id);
    }

    fn toggle_visibility(&mut self) {
        if self.is_hidden {
            // Show sidebar
            show_self(false); // false = tiled, not floating
            set_selectable(true);
            self.is_hidden = false;
            self.is_focused = true;
            eprintln!("Sidebar shown");
        } else {
            // Hide sidebar — reclaim space
            hide_self();
            set_selectable(false);
            self.is_hidden = true;
            self.is_focused = false;
            eprintln!("Sidebar hidden");
        }
    }
}

// --- Plugin Lifecycle ---

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // Parse verbosity config
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

        // Parse numbered project entries (project_0, project_1, ...)
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

        // Warn on duplicate basenames (session name collision)
        let names: Vec<&str> = self.projects.iter().map(|p| p.name.as_str()).collect();
        for (idx, name) in names.iter().enumerate() {
            if names[idx + 1..].contains(name) {
                eprintln!(
                    "WARNING: Duplicate project basename '{}'. Session matching will be ambiguous.",
                    name
                );
            }
        }

        eprintln!(
            "Loaded {} projects (verbosity: {})",
            self.projects.len(),
            match self.verbosity {
                Verbosity::Minimal => "minimal",
                Verbosity::Full => "full",
            }
        );

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::Reconfigure,
        ]);
        subscribe(&[
            EventType::SessionUpdate,
            EventType::PermissionRequestResult,
            EventType::Key,
        ]);
        eprintln!("Plugin loaded, requesting permissions");
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.permissions_granted = true;
                // Only set_selectable(false) AFTER permissions granted —
                // calling before would block the permission dialog
                set_selectable(false);
                self.setup_toggle_keybind();
                eprintln!("Permissions granted, sidebar set to unselectable");
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                eprintln!("Permissions denied — plugin cannot function without ReadApplicationState + ChangeApplicationState + Reconfigure");
                false
            }
            Event::SessionUpdate(sessions, resurrectable) => {
                for project in &mut self.projects {
                    if let Some(session) = sessions.iter().find(|s| s.name == project.name) {
                        // Extract tab count
                        let tab_count = session.tabs.len();

                        // Extract active pane command for current session
                        let active_command = if session.is_current_session {
                            // Find the active tab
                            session.tabs.iter()
                                .find(|t| t.active)
                                .and_then(|active_tab| {
                                    // Get panes for the active tab
                                    session.panes.panes.get(&active_tab.position)
                                        .and_then(|panes| {
                                            // Find the focused non-plugin pane
                                            panes.iter()
                                                .find(|p| p.is_focused && !p.is_plugin && !p.is_suppressed)
                                                .and_then(|pane| {
                                                    pane.terminal_command.as_ref()
                                                        .map(|cmd| {
                                                            // Extract just the binary name from the full path
                                                            PathBuf::from(cmd)
                                                                .file_name()
                                                                .and_then(|n| n.to_str())
                                                                .unwrap_or(cmd)
                                                                .to_string()
                                                        })
                                                })
                                        })
                                })
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
                true
            }
            Event::Key(key) => match key.bare_key {
                BareKey::Char('j') if key.has_no_modifiers() => {
                    if !self.projects.is_empty() {
                        self.selected_index = (self.selected_index + 1)
                            .min(self.projects.len().saturating_sub(1));
                    }
                    true
                }
                BareKey::Char('k') if key.has_no_modifiers() => {
                    self.selected_index = self.selected_index.saturating_sub(1);
                    true
                }
                BareKey::Enter if key.has_no_modifiers() => {
                    self.activate_selected_project();
                    true
                }
                BareKey::Char('x') if key.has_no_modifiers() => {
                    self.kill_selected_session();
                    true
                }
                BareKey::Esc if key.has_no_modifiers() => {
                    set_selectable(false);
                    self.is_focused = false;
                    eprintln!("Sidebar deactivated");
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
            println!("Loading...");
            return;
        }

        if self.projects.is_empty() {
            println!("No projects configured.");
            return;
        }

        // Header
        let header = " Projects";
        let header_text = Text::new(header).color_all(COLOR_BLUE);
        print_text_with_coordinates(header_text, 0, 0, Some(cols), None);

        // Separator line
        let separator = "─".repeat(cols.min(40));
        let sep_text = Text::new(&separator).color_all(COLOR_GRAY);
        print_text_with_coordinates(sep_text, 0, 1, Some(cols), None);

        for (i, project) in self.projects.iter().enumerate() {
            let y = i + 2; // offset for header + separator

            // Status indicator dot
            let status_dot = match &project.status {
                SessionStatus::Running { is_current: true, .. } => "●",
                SessionStatus::Running { is_current: false, .. } => "●",
                SessionStatus::Exited => "●",
                SessionStatus::NotStarted => "○",
            };

            // Build the line based on verbosity
            let line = match self.verbosity {
                Verbosity::Minimal => {
                    format!(" {} {}", status_dot, project.name)
                }
                Verbosity::Full => {
                    let mut parts = format!(" {} {}", status_dot, project.name);

                    // Add tab count for running sessions
                    if let SessionStatus::Running { tab_count, .. } = &project.status {
                        parts.push_str(&format!(" [{}]", tab_count));
                    }

                    // Add active command for current session
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

            // Truncate to fit column width
            let display_line: String = if line.chars().count() > cols {
                line.chars().take(cols.saturating_sub(1)).collect::<String>() + "…"
            } else {
                line
            };

            let mut text = Text::new(&display_line);

            // Apply selection highlight
            if i == self.selected_index {
                text = text.selected();
            }

            // Color the status dot (character at position 1)
            // The dot is at char index 1 (after leading space)
            match &project.status {
                SessionStatus::Running { is_current: true, .. } => {
                    text = text.color_range(COLOR_GREEN, 1..2);
                }
                SessionStatus::Running { is_current: false, .. } => {
                    text = text.color_range(COLOR_GREEN, 1..2);
                }
                SessionStatus::Exited => {
                    text = text.color_range(COLOR_YELLOW, 1..2);
                }
                SessionStatus::NotStarted => {
                    text = text.color_range(COLOR_GRAY, 1..2);
                }
            }

            // In full mode, color the enrichment info
            if self.verbosity == Verbosity::Full {
                if let SessionStatus::Running { tab_count, is_current, active_command, .. } = &project.status {
                    // Find where the tab count bracket starts
                    let name_end = 3 + project.name.chars().count(); // " ● name"
                    let bracket_str = format!("[{}]", tab_count);
                    let bracket_start = name_end + 1; // space before bracket
                    let bracket_end = bracket_start + bracket_str.chars().count() + 1; // +1 for the space

                    // Color the tab count info dim/blue
                    if display_line.chars().count() > bracket_start {
                        let actual_end = bracket_end.min(display_line.chars().count());
                        text = text.color_range(COLOR_GRAY, bracket_start..actual_end);
                    }

                    // Color the command info
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

            print_text_with_coordinates(text, 0, y, Some(cols), None);
        }

        // Footer with focus hint
        let footer_y = self.projects.len() + 3; // header + separator + projects + gap
        if footer_y < rows {
            let hint = if self.is_focused {
                " j/k:nav ↵:switch x:kill"
            } else {
                " ⌘P to toggle"
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
            "focus_sidebar" => {
                // Legacy support for old keybind
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
