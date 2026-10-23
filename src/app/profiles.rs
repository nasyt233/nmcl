use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub fn handle_profile_modal_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                if let Some(i) = self.profile_selected {
                    self.current_profile = i;
                    self.settings.player_name = self.profiles[i].name.clone();
                    self.save_settings();
                }
                self.profile_modal_open = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(i) = self.profile_selected { if i > 0 { self.profile_selected = Some(i - 1); } }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(i) = self.profile_selected { if i + 1 < self.profiles.len() { self.profile_selected = Some(i + 1); } }
            }
            KeyCode::Char('n') => {
                self.profiles.push(crate::app::Profile { id: uuid::Uuid::new_v4().to_string(), name: "New Player".into() });
                self.profile_selected = Some(self.profiles.len() - 1);
            }
            KeyCode::Char('d') => {
                if self.profiles.len() > 1 {
                    if let Some(i) = self.profile_selected {
                        self.profiles.remove(i);
                        self.profile_selected = Some(0);
                        if self.current_profile >= self.profiles.len() { self.current_profile = 0; }
                    }
                }
            }
            _ => {}
        }
        false
    }
}