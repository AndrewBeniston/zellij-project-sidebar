use zellij_tile::prelude::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct State {
    permissions_granted: bool,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::Reconfigure,
        ]);
        subscribe(&[
            EventType::SessionUpdate,
            EventType::PermissionRequestResult,
        ]);
        eprintln!("Plugin loaded, requesting permissions");
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.permissions_granted = true;
                eprintln!("Permissions granted");
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                eprintln!("Permissions denied");
                false
            }
            Event::SessionUpdate(sessions, resurrectable) => {
                eprintln!(
                    "SessionUpdate: {} active, {} resurrectable",
                    sessions.len(),
                    resurrectable.len()
                );
                for session in &sessions {
                    eprintln!(
                        "  Session: {} (tabs: {}, current: {})",
                        session.name,
                        session.tabs.len(),
                        session.is_current_session
                    );
                }
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, _rows: usize, _cols: usize) {
        if self.permissions_granted {
            println!("Project Sidebar (loading...)");
        } else {
            println!("Waiting for permissions...");
        }
    }

    fn pipe(&mut self, _pipe_message: PipeMessage) -> bool {
        false
    }
}
