use crate::app::{App, ResourceModalFocus, VersionSide, WorkerMsg, PAGE_SIZE};
use crate::worker::modrinth::{ResourceCategory, ResourceHit};
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::mpsc::Sender;

impl App {
    pub fn submit_resource_search(&mut self, tx: Sender<WorkerMsg>) {
        if self.resource_searching || !self.version_side.is_community() { return; }
        let cat = match self.version_side { VersionSide::Mod => ResourceCategory::Mod, VersionSide::Modpack => ResourceCategory::Modpack, VersionSide::Resourcepack => ResourceCategory::Resourcepack, VersionSide::Shader => ResourceCategory::Shader, _ => ResourceCategory::Mod };
        let q = self.search_buffer.trim().to_string();
        self.resource_query = q; self.resource_searching = true; self.resource_results.clear();
        self.resource_selected = None; self.resource_page = 0; self.needs_hot = false;
        crate::worker::modrinth::start_search(tx, self.cancel.clone(), cat, self.resource_query.clone(), self.resource_loader, self.resource_game_version.clone(), self.resource_sort, 0, PAGE_SIZE);
    }

    pub fn submit_hot_search(&mut self, tx: Sender<WorkerMsg>) {
        if self.resource_searching || !self.version_side.is_community() { return; }
        let cat = match self.version_side { VersionSide::Mod => ResourceCategory::Mod, VersionSide::Modpack => ResourceCategory::Modpack, VersionSide::Resourcepack => ResourceCategory::Resourcepack, VersionSide::Shader => ResourceCategory::Shader, _ => ResourceCategory::Mod };
        self.resource_searching = true; self.resource_results.clear(); self.resource_selected = None;
        crate::worker::modrinth::start_search(tx, self.cancel.clone(), cat, String::new(), self.resource_loader, self.resource_game_version.clone(), self.resource_sort, self.resource_page * PAGE_SIZE, PAGE_SIZE);
    }

    pub fn download_selected_resource(&mut self, tx: Sender<WorkerMsg>) {
        let Some(hit) = self.resource_selected.and_then(|i| self.resource_results.get(i)).cloned() else { return; };
        if self.resource_downloading { return; }
        
        if self.version_side == VersionSide::Modpack {
            self.open_modpack_modal(hit);
            return;
        }
        
        // 打开资源下载弹窗
        self.resource_modal_open = true;
        self.resource_modal_hit = Some(hit.clone());
        self.resource_modal_fetching = true;
        self.resource_modal_versions.clear();
        self.resource_modal_mc_versions.clear();
        self.resource_modal_mc_idx = None;
        self.resource_modal_sub_idx = None;
        
        let mut instances = self.versions.iter().map(|v| v.name.clone()).collect::<Vec<_>>();
        instances.sort();
        instances.insert(0, String::new()); // 全局
        self.resource_modal_instances = instances;
        self.resource_modal_instance_idx = Some(1);
        self.resource_modal_focus = ResourceModalFocus::McVersion;
        
        crate::worker::modrinth::start_fetch_resource_versions(tx, self.cancel.clone(), hit, self.resource_loader, self.resource_game_version.clone());
    }

    pub fn open_modpack_modal(&mut self, hit: ResourceHit) {
        let mut versions: Vec<String> = self.remote_versions.iter().filter(|v| v.category == crate::worker::meta::RemoteCategory::Release).map(|v| v.id.clone()).collect();
        versions.sort_by(|a, b| b.cmp(a)); versions.truncate(50);
        if versions.is_empty() { return; }
        self.modpack_instance = Some(hit.title.clone()); self.modpack_game_versions = versions;
        self.modpack_selected_gv = Some(0); self.modpack_pending_hit = Some(hit); self.modpack_modal_open = true;
    }

    pub fn handle_modpack_modal_key(&mut self, key: KeyEvent, tx: Sender<WorkerMsg>) -> bool {
        match key.code {
            KeyCode::Esc => { self.modpack_modal_open = false; self.modpack_pending_hit = None; }
            KeyCode::Up | KeyCode::Char('k') => { if let Some(i) = self.modpack_selected_gv { if i > 0 { self.modpack_selected_gv = Some(i - 1); } } }
            KeyCode::Down | KeyCode::Char('j') => { if let Some(i) = self.modpack_selected_gv { if i + 1 < self.modpack_game_versions.len() { self.modpack_selected_gv = Some(i + 1); } } }
            KeyCode::Enter => {
                let Some(hit) = self.modpack_pending_hit.clone() else { return false; };
                let Some(idx) = self.modpack_selected_gv else { return false; };
                let Some(gv) = self.modpack_game_versions.get(idx).cloned() else { return false; };
                self.modpack_modal_open = false; self.modpack_pending_hit = None;
                self.resource_downloading = true; self.busy = true; self.status = "安装整合包中".into();
                crate::worker::modrinth::start_download_modpack(tx, self.cancel.clone(), self.mc_dir.clone(), hit, gv);
            }
            _ => {}
        }
        false
    }

    pub fn handle_resource_modal_key(&mut self, key: KeyEvent, tx: Sender<WorkerMsg>) -> bool {
        match key.code {
            KeyCode::Esc => self.resource_modal_open = false,
            KeyCode::Tab => {
                self.resource_modal_focus = match self.resource_modal_focus {
                    ResourceModalFocus::McVersion => ResourceModalFocus::SubVersion,
                    ResourceModalFocus::SubVersion => ResourceModalFocus::Instance,
                    ResourceModalFocus::Instance => ResourceModalFocus::Confirm,
                    ResourceModalFocus::Confirm => ResourceModalFocus::McVersion,
                };
            }
            KeyCode::BackTab => {
                self.resource_modal_focus = match self.resource_modal_focus {
                    ResourceModalFocus::McVersion => ResourceModalFocus::Confirm,
                    ResourceModalFocus::SubVersion => ResourceModalFocus::McVersion,
                    ResourceModalFocus::Instance => ResourceModalFocus::SubVersion,
                    ResourceModalFocus::Confirm => ResourceModalFocus::Instance,
                };
            }
            KeyCode::Up | KeyCode::Char('k') => self.scroll_resource_modal(-1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_resource_modal(1),
            KeyCode::Enter => self.confirm_resource_download(tx),
            _ => {}
        }
        false
    }
    
    pub fn scroll_resource_modal(&mut self, delta: isize) {
        match self.resource_modal_focus {
            ResourceModalFocus::McVersion => {
                let len = self.resource_modal_mc_versions.len();
                if len == 0 { return; }
                let cur = self.resource_modal_mc_idx.unwrap_or(0);
                let next = if delta > 0 { (cur + delta as usize).min(len - 1) } else { cur.saturating_sub((-delta) as usize) };
                if next != cur { self.resource_modal_mc_idx = Some(next); self.resource_modal_sub_idx = Some(0); }
            }
            ResourceModalFocus::SubVersion => {
                let len = self.resource_modal_filtered_indices().len();
                if len == 0 { return; }
                let cur = self.resource_modal_sub_idx.unwrap_or(0);
                self.resource_modal_sub_idx = Some(if delta > 0 { (cur + delta as usize).min(len - 1) } else { cur.saturating_sub((-delta) as usize) });
            }
            ResourceModalFocus::Instance => {
                let len = self.resource_modal_instances.len();
                if len == 0 { return; }
                let cur = self.resource_modal_instance_idx.unwrap_or(0);
                self.resource_modal_instance_idx = Some(if delta > 0 { (cur + delta as usize).min(len - 1) } else { cur.saturating_sub((-delta) as usize) });
            }
            ResourceModalFocus::Confirm => {}
        }
    }

    pub fn confirm_resource_download(&mut self, tx: Sender<WorkerMsg>) {
        let Some(hit) = self.resource_modal_hit.clone() else { return; };
        let Some(sub_idx) = self.resource_modal_sub_idx else { return; };
        let indices = self.resource_modal_filtered_indices();
        let Some(&abs_idx) = indices.get(sub_idx) else { return; };
        let Some(version) = self.resource_modal_versions.get(abs_idx).cloned() else { return; };
        
        let instance = self.resource_modal_instance_idx.and_then(|i| self.resource_modal_instances.get(i)).cloned().filter(|s| !s.is_empty());
        self.resource_modal_open = false;
        self.resource_downloading = true; self.busy = true; self.status = "下载资源中".into();
        crate::worker::modrinth::start_download_resource(tx, self.cancel.clone(), self.mc_dir.clone(), hit, version, instance);
    }

    pub fn scroll_resource(&mut self, delta: isize) {
        if self.resource_results.is_empty() { return; }
        let len = self.resource_results.len(); let cur = self.resource_selected.unwrap_or(0);
        self.resource_selected = Some(if delta > 0 { (cur + delta as usize).min(len - 1) } else { cur.saturating_sub((-delta) as usize) });
    }
}