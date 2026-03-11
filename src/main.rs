use zellij_tile::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

// --- Data Model ---

#[derive(Clone, PartialEq)]
enum SessionStatus {
    Running { is_current: bool },
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
}

impl Default for State {
    fn default() -> Self {
        Self {
            permissions_granted: false,
            projects: Vec::new(),
            selected_index: 0,
            initial_load_complete: false,
            is_focused: false,
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
                SessionStatus::Running { is_current: true } => {
                    eprintln!("Cannot kill current session '{}'", project.name);
                }
                SessionStatus::Running { is_current: false } => {
                    kill_sessions(&[project.name.clone()]);
                }
                _ => {}
            }
        }
    }

    fn setup_focus_keybind(&self) {
        let plugin_id = get_plugin_ids().plugin_id;
        let config = format!(
            r#"
keybinds {{
    shared {{
        bind "Alt s" {{
            MessagePluginId {plugin_id} {{
                name "focus_sidebar"
            }}
        }}
    }}
}}
"#,
        );
        reconfigure(config, false);
        eprintln!("Focus keybind Alt+s registered for plugin {}", plugin_id);
    }
}

// --- Plugin Lifecycle ---

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
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

        eprintln!("Loaded {} projects from configuration", self.projects.len());

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
                self.setup_focus_keybind();
                eprintln!("Permissions granted, sidebar set to unselectable");
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                eprintln!("Permissions denied");
                false
            }
            Event::SessionUpdate(sessions, resurrectable) => {
                for project in &mut self.projects {
                    if let Some(session) = sessions.iter().find(|s| s.name == project.name) {
                        project.status = SessionStatus::Running {
                            is_current: session.is_current_session,
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

    fn render(&mut self, _rows: usize, cols: usize) {
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

        for (i, project) in self.projects.iter().enumerate() {
            let status_char = match &project.status {
                SessionStatus::Running { is_current: true } => ">",
                SessionStatus::Running { is_current: false } => "*",
                SessionStatus::Exited => "x",
                SessionStatus::NotStarted => " ",
            };
            let line = format!(" {} {}", status_char, project.name);
            let mut text = Text::new(&line);

            if i == self.selected_index {
                text = text.selected();
            }

            // Color the status character based on session status
            match &project.status {
                SessionStatus::Running { .. } => {
                    text = text.color_range(0, 1..=1);
                }
                SessionStatus::Exited => {
                    text = text.color_range(2, 1..=1);
                }
                _ => {}
            }

            print_text_with_coordinates(text, 0, i, Some(cols), None);
        }
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if pipe_message.name == "focus_sidebar" {
            set_selectable(true);
            show_self(false); // false = tiled, not floating
            self.is_focused = true;
            eprintln!("Sidebar activated via pipe");
            return true;
        }
        false
    }
}
