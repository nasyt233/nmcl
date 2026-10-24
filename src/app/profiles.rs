use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub fn handle_profile_modal_key(&mut self, key: KeyEvent) -> bool {
        if self.profile_adding {
            match key.code {
                KeyCode::Enter => {
                    let name = self.profile_name_buffer.trim().to_string();
                    if !name.is_empty() { self.add_profile(name); }
                    self.profile_adding = false;
                    self.profile_name_buffer.clear();
                }
                KeyCode::Esc => { self.profile_adding = false; self.profile_name_buffer.clear(); }
                KeyCode::Backspace => { self.profile_name_buffer.pop(); }
                KeyCode::Char(c) => { self.profile_name_buffer.push(c); }
                _ => {}
            }
            return false;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                if let Some(i) = self.profile_selected { self.switch_profile(i); }
                self.profile_modal_open = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(i) = self.profile_selected { if i > 0 { self.profile_selected = Some(i - 1); } }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(i) = self.profile_selected { if i + 1 < self.profiles.len() { self.profile_selected = Some(i + 1); } }
            }
            KeyCode::Char('n') => { self.profile_adding = true; self.profile_name_buffer.clear(); }
            KeyCode::Char('d') => {
                if let Some(i) = self.profile_selected { self.delete_profile(i); }
            }
            _ => {}
        }
        false
    }
}