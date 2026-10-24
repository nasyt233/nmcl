use crate::app::{App, ClickTarget, EditField, FocusArea, Tab, VersionManagerMode, VersionSide, WorkerMsg, PAGE_SIZE};
use crate::worker::meta::RemoteCategory;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::{collections::HashSet, io::Write, sync::{atomic::Ordering, mpsc::Sender}, time::Instant};

impl App {
    pub fn nav_up(&mut self) {
        match self.focus_area {
            FocusArea::Left => match self.tab {
                Tab::Home => self.prev_local_version(),
                Tab::Downloads => self.prev_version_side(),
                Tab::Settings => self.prev_settings_side(),
                Tab::Versions if self.version_manager_mode == VersionManagerMode::List => {
                    if self.version_left_selected > 0 { self.version_left_selected -= 1; }
                }
                Tab::Versions => self.prev_manager_mode(),
            },
            FocusArea::Right => match self.tab {
                Tab::Downloads => {
                    if self.version_side == VersionSide::Local { self.scroll_local(-1); }
                    else if self.version_side.is_community() { self.scroll_resource(-1); }
                    else { self.scroll_remote(-1); }
                }
                Tab::Versions => match self.version_manager_mode {
                    VersionManagerMode::List => self.scroll_local(-1),
                    VersionManagerMode::Settings => { if self.right_action_selected > 0 { self.right_action_selected -= 1; } }
                    VersionManagerMode::Mods => { if let Some(i) = self.mod_selected { if i > 0 { self.mod_selected = Some(i - 1); } } }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    pub fn nav_down(&mut self) {
        match self.focus_area {
            FocusArea::Left => match self.tab {
                Tab::Home => self.next_local_version(),
                Tab::Downloads => self.next_version_side(),
                Tab::Settings => self.next_settings_side(),
                Tab::Versions if self.version_manager_mode == VersionManagerMode::List => {
                    if self.version_left_selected < 3 { self.version_left_selected += 1; }
                }
                Tab::Versions => self.next_manager_mode(),
            },
            FocusArea::Right => match self.tab {
                Tab::Downloads => {
                    if self.version_side == VersionSide::Local { self.scroll_local(1); }
                    else if self.version_side.is_community() { self.scroll_resource(1); }
                    else { self.scroll_remote(1); }
                }
                Tab::Versions => match self.version_manager_mode {
                    VersionManagerMode::List => self.scroll_local(1),
                    VersionManagerMode::Settings => { if self.right_action_selected < 2 { self.right_action_selected += 1; } }
                    VersionManagerMode::Mods => {
                        if let Some(i) = self.mod_selected { if i + 1 < self.mods.len() { self.mod_selected = Some(i + 1); } }
                        else if !self.mods.is_empty() { self.mod_selected = Some(0); }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    pub fn prev_settings_side(&mut self) { let all = crate::app::SettingsSide::all(); let i = all.iter().position(|s| *s == self.settings_side).unwrap_or(0); self.settings_side = all[(i + all.len() - 1) % all.len()]; }
    pub fn next_settings_side(&mut self) { let all = crate::app::SettingsSide::all(); let i = all.iter().position(|s| *s == self.settings_side).unwrap_or(0); self.settings_side = all[(i + 1) % all.len()]; }
    pub fn prev_manager_mode(&mut self) { self.version_manager_mode = match self.version_manager_mode { VersionManagerMode::Basic => VersionManagerMode::Mods, VersionManagerMode::Settings => VersionManagerMode::Basic, VersionManagerMode::Mods => VersionManagerMode::Settings, _ => VersionManagerMode::Basic }; }
    pub fn next_manager_mode(&mut self) { self.version_manager_mode = match self.version_manager_mode { VersionManagerMode::Basic => VersionManagerMode::Settings, VersionManagerMode::Settings => VersionManagerMode::Mods, VersionManagerMode::Mods => VersionManagerMode::Basic, _ => VersionManagerMode::Basic }; }

    pub fn scroll_local(&mut self, delta: isize) {
        if self.versions.is_empty() { return; }
        let len = self.versions.len(); let cur = self.version_selected.unwrap_or(self.local_page * PAGE_SIZE);
        let next = if delta > 0 { (cur + delta as usize).min(len - 1) } else { cur.saturating_sub((-delta) as usize) };
        self.version_selected = Some(next); self.local_page = next / PAGE_SIZE;
        
        self.settings.selected_version = Some(self.versions[next].name.clone());
        self.save_settings();
    }

    pub fn scroll_remote(&mut self, delta: isize) {
        let len = self.remote_list().len(); if len == 0 { return; }
        let idx = self.version_side.index(); let cur = self.remote_sel[idx].unwrap_or(self.remote_page[idx] * PAGE_SIZE);
        let next = if delta > 0 { (cur + delta as usize).min(len - 1) } else { cur.saturating_sub((-delta) as usize) };
        self.remote_sel[idx] = Some(next); self.remote_page[idx] = next / PAGE_SIZE;
    }

    pub fn switch_version_side(&mut self, vs: VersionSide) {
        self.version_side = vs; let idx = vs.index();
        if vs.is_community() { self.resource_page = 0; self.resource_selected = None; self.resource_results.clear(); self.needs_hot = true; }
        else if vs == VersionSide::Local {
            let start = self.local_page * PAGE_SIZE;
            self.version_selected = if start < self.versions.len() { Some(start) } else if !self.versions.is_empty() { self.local_page = 0; Some(0) } else { None };
        } else {
            let page = self.remote_page[idx]; let len = self.remote_list().len(); let start = page * PAGE_SIZE;
            self.remote_sel[idx] = if start < len { Some(start) } else if len > 0 { self.remote_page[idx] = 0; Some(0) } else { None };
        }
    }
    pub fn next_version_side(&mut self) { let all = VersionSide::all(); let i = self.version_side.index(); self.switch_version_side(all[(i + 1) % all.len()]); }
    pub fn prev_version_side(&mut self) { let all = VersionSide::all(); let i = self.version_side.index(); self.switch_version_side(all[(i + all.len() - 1) % all.len()]); }
    
    pub fn next_local_version(&mut self) { 
        if self.versions.is_empty() { return; } 
        self.scroll_local(1); 
    }
    pub fn prev_local_version(&mut self) { 
        if self.versions.is_empty() { return; } 
        self.scroll_local(-1); 
    }
    
    pub fn next_page(&mut self, _tx: Sender<WorkerMsg>) {
        if self.version_side.is_community() {
            let total = if self.resource_total_hits == 0 { 1 } else { (self.resource_total_hits + PAGE_SIZE - 1) / PAGE_SIZE };
            if self.resource_page + 1 < total { self.resource_page += 1; self.needs_hot = true; }
        } else if self.version_side.is_mc() {
            let idx = self.version_side.index();
            if (self.remote_page[idx] + 1) * PAGE_SIZE < self.remote_list().len() { self.remote_page[idx] += 1; self.remote_sel[idx] = Some(self.remote_page[idx] * PAGE_SIZE); }
        }
    }
    pub fn prev_page(&mut self, _tx: Sender<WorkerMsg>) {
        if self.version_side.is_community() { if self.resource_page > 0 { self.resource_page -= 1; self.needs_hot = true; } }
        else if self.version_side.is_mc() {
            let idx = self.version_side.index();
            if self.remote_page[idx] > 0 { self.remote_page[idx] -= 1; self.remote_sel[idx] = Some(self.remote_page[idx] * PAGE_SIZE); }
        }
    }

    pub fn switch_profile(&mut self, i: usize) { if i < self.profiles.len() { self.current_profile = i; self.settings.player_name = self.profiles[i].name.clone(); self.settings.current_profile_id = Some(self.profiles[i].id.clone()); self.settings.profiles = self.profiles.clone(); self.save_settings(); } }
    pub fn add_profile(&mut self, name: String) { self.profiles.push(crate::app::Profile { id: uuid::Uuid::new_v4().to_string(), name }); self.settings.profiles = self.profiles.clone(); self.switch_profile(self.profiles.len() - 1); }
    pub fn delete_profile(&mut self, i: usize) { if self.profiles.len() > 1 { self.profiles.remove(i); self.settings.profiles = self.profiles.clone(); self.switch_profile(0); } }

    pub fn begin_edit(&mut self, field: EditField) {
        self.edit_buffer = match field {
            EditField::PlayerName => self.profiles[self.current_profile].name.clone(),
            EditField::JavaPath => self.settings.java_path.clone(),
            EditField::VersionName => self.managing_version.clone().unwrap_or_default(),
            EditField::Memory => self.settings.get_version(self.managing_version.as_deref().unwrap_or("")).max_memory_mb.unwrap_or(2048).to_string(),
            EditField::DefaultMemory => self.settings.default_max_memory_mb.to_string(),
            EditField::JavaArgs => self.settings.get_version(self.managing_version.as_deref().unwrap_or("")).jvm_args.clone(),
            EditField::GameArgs => self.settings.get_version(self.managing_version.as_deref().unwrap_or("")).game_args.clone(),
            _ => return,
        };
        self.editing = field;
    }

    pub fn commit_edit(&mut self) {
        let t = self.edit_buffer.trim().to_string(); let ver = self.managing_version.clone().unwrap_or_default();
        match self.editing {
            EditField::PlayerName => { if !t.is_empty() { self.profiles[self.current_profile].name = t; self.switch_profile(self.current_profile); } }
            EditField::JavaPath => { if !t.is_empty() { self.settings.java_path = t; self.save_settings(); } }
            EditField::VersionName => { if !t.is_empty() { self.rename_managed_version(&t); } }
            EditField::Memory => { if let Ok(mb) = t.parse::<u32>() { let mut vs = self.settings.get_version(&ver); vs.max_memory_mb = Some(mb); self.settings.versions.insert(ver, vs); self.save_settings(); } }
            EditField::DefaultMemory => { if let Ok(mb) = t.parse::<u32>() { self.settings.default_max_memory_mb = mb; self.save_settings(); } }
            EditField::JavaArgs => { let mut vs = self.settings.get_version(&ver); vs.jvm_args = t; self.settings.versions.insert(ver, vs); self.save_settings(); }
            EditField::GameArgs => { let mut vs = self.settings.get_version(&ver); vs.game_args = t; self.settings.versions.insert(ver, vs); self.save_settings(); }
            _ => {}
        }
        self.editing = EditField::None; self.edit_buffer.clear();
    }
    pub fn cancel_edit(&mut self) { self.editing = EditField::None; self.edit_buffer.clear(); }

    pub fn confirm(&mut self, tx: Sender<WorkerMsg>) {
        if self.busy { return; }
        if self.tab == Tab::Versions && self.focus_area == FocusArea::Left && self.version_manager_mode == VersionManagerMode::List {
            match self.version_left_selected {
                0 => self.tab = Tab::Home,
                1 | 2 => self.push_log("暂不支持修改目录"),
                3 => self.tab = Tab::Downloads,
                _ => {}
            }
            return;
        }

        match self.tab {
            Tab::Home => self.start_launch_selected(tx),
            Tab::Versions => {
                if self.version_manager_mode == VersionManagerMode::List {
                    if let Some(i) = self.version_selected {
                        if i < self.versions.len() { self.managing_version = Some(self.versions[i].name.clone()); self.version_manager_mode = VersionManagerMode::Basic; }
                    }
                } else if self.version_manager_mode == VersionManagerMode::Settings {
                    match self.right_action_selected { 0 => self.begin_edit(EditField::VersionName), 1 => self.copy_managed_version(), 2 => self.delete_selected_version(), _ => {} }
                } else if self.version_manager_mode == VersionManagerMode::Mods {
                    if let Some(i) = self.mod_selected { self.toggle_mod(i); }
                }
            }
            Tab::Downloads => {
                if self.version_side.is_community() { if self.resource_selected.is_some() { self.download_selected_resource(tx); } }
                else if self.selected_remote().is_some() { self.open_download_modal(tx); }
            }
            Tab::Settings => {}
        }
    }
    pub fn cancel_current(&mut self) { if self.busy { self.cancel.store(true, Ordering::Relaxed); self.status = "取消中".into(); } }

    pub fn start_launch_selected(&mut self, tx: Sender<WorkerMsg>) {
        let Some(v) = self.selected_local() else { return; };
        if v.inherits { self.push_log("不支持继承版本"); return; }
        if !v.installed { self.push_log("尚未安装"); return; }
        let version = v.name.clone(); let player = self.profiles[self.current_profile].name.clone();
        let vs = self.settings.get_version(&version);
        let java = if !vs.java_path.trim().is_empty() { vs.java_path.clone() } else { self.settings.java_path.clone() };

        self.cancel.store(false, Ordering::Relaxed); self.busy = true; self.status = "启动中".into();
        self.push_log(format!("启动 {version}（玩家: {player}）"));
        crate::worker::launch::start_launch(tx, self.cancel.clone(), self.mc_dir.clone(), version, player, java, vs);
    }

    pub fn handle_worker_msg(&mut self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::Log(s) => {
                if s.contains("http://") || s.contains("https://") {
                    let elapsed = self.start_instant.elapsed(); let ts = format!("+{}.{:03}", elapsed.as_secs(), elapsed.subsec_millis());
                    if let Some(f) = self.log_file.as_mut() { let _ = writeln!(f, "[{ts}] [DEBUG] {s}"); let _ = f.flush(); }
                } else { self.push_log(s); }
            }
            WorkerMsg::Progress { done, total, label } => {
                self.progress = Some((done, total, label.clone())); let now = Instant::now();
                if let Some((last, last_done)) = self.last_progress {
                    let dt = now.duration_since(last).as_secs_f64();
                    if dt > 0.3 && done > last_done { self.progress_speed = (done - last_done) as f64 / dt; self.last_progress = Some((now, done)); }
                } else { self.last_progress = Some((now, done)); }
                if total > 0 && (done == 1 || done == total || done % (total / 4).max(1) == 0) {
                    let cn = match label.as_str() { "libraries" => "依赖库", "assets" => "游戏资源", "modpack" => "整合包", "resource" => "资源文件", _ => label.as_str() };
                    self.push_log(format!("[进度] 正在下载{} {:.0}% ({}/{})", cn, (done as f64 / total as f64) * 100.0, done, total));
                }
            }
            WorkerMsg::RemoteVersionsLoaded(versions) => {
                let local: HashSet<String> = self.versions.iter().map(|v| v.name.clone()).collect();
                self.remote_versions = versions.into_iter().filter(|v| !local.contains(&v.id)).collect();
                let mut gvs: Vec<String> = self.remote_versions.iter().filter(|v| v.category == RemoteCategory::Release).map(|v| v.id.clone()).collect();
                gvs.sort_by(|a, b| b.cmp(a)); gvs.truncate(20); self.resource_game_versions = gvs;
                for side in [VersionSide::Release, VersionSide::Snapshot, VersionSide::AprilFools] {
                    let idx = side.index();
                    if self.remote_versions.iter().filter(|v| v.category == self.category_of(side)).count() > 0 && self.remote_sel[idx].is_none() { self.remote_sel[idx] = Some(0); }
                }
                self.push_log("[信息] 远程原版游戏版本清单已加载完成");
            }
            WorkerMsg::DownloadComplete(id) => { self.push_log(format!("[完成] 游戏版本 {} 安装成功！", id)); self.versions = crate::worker::meta::scan_versions(&self.mc_dir); self.version_selected = Some(0); }
            WorkerMsg::ResourceSearchResults { hits, total } => {
                self.resource_searching = false; self.resource_results = hits; self.resource_total_hits = total;
                if !self.resource_results.is_empty() { self.resource_selected = Some(0); }
                self.push_log(format!("[信息] 资源搜索完成，共找到 {} 个结果", self.resource_total_hits));
            }
            WorkerMsg::ResourceDownloadComplete(name) => { self.resource_downloading = false; self.push_log(format!("[完成] {} 下载完成！", name)); }
            WorkerMsg::ResourceVersionsLoaded { hit, versions } => {
                self.resource_modal_fetching = false;
                if self.resource_modal_hit.as_ref().map(|h| h.project_id == hit.project_id).unwrap_or(false) {
                    let mut mc_versions = Vec::new(); let mut last: Option<String> = None;
                    for v in &versions {
                        let mc = v.game_versions.first().cloned().unwrap_or_default();
                        if Some(&mc) != last.as_ref() { mc_versions.push(mc.clone()); last = Some(mc); }
                    }
                    self.resource_modal_versions = versions; self.resource_modal_mc_versions = mc_versions;
                    if !self.resource_modal_mc_versions.is_empty() { self.resource_modal_mc_idx = Some(0); self.resource_modal_sub_idx = Some(0); }
                    self.push_log(format!("[信息] {} 的版本列表获取成功", hit.title));
                }
            }
            WorkerMsg::LoaderVersionsLoaded { loader, versions } => {
                self.download_fetching_loader_versions = false;
                if self.download_loader == loader { self.download_loader_versions = versions; self.download_loader_ver_index = Some(0); }
                self.push_log(format!("[信息] {} 加载器版本获取成功", loader.title()));
            }
            WorkerMsg::VersionDownloadFailed(e) => self.push_log(format!("[错误] 版本下载失败: {}", e)),
            WorkerMsg::Finished(res) => {
                self.busy = false; self.resource_downloading = false; self.download_fetching_loader_versions = false; self.progress = None;
                match res { Ok(()) => { self.status = "就绪".into(); self.push_log("[完成] 所有任务已顺利结束"); } Err(e) => { self.status = "错误".into(); self.push_log(format!("[错误] 任务异常中断: {}", e)); } }
            }
        }
    }

    fn category_of(&self, side: VersionSide) -> RemoteCategory {
        match side { VersionSide::Latest | VersionSide::Release => RemoteCategory::Release, VersionSide::Snapshot => RemoteCategory::Snapshot, VersionSide::AprilFools => RemoteCategory::AprilFools, _ => RemoteCategory::Release }
    }

    pub fn handle_mouse(&mut self, m: MouseEvent, tx: Sender<WorkerMsg>) {
        let modal_open = self.profile_modal_open || self.download_modal_open || self.resource_modal_open || self.modpack_modal_open;
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => { if let Some(t) = self.layout.hit(m.column, m.row) { self.dispatch_click(t, tx); } }
            MouseEventKind::ScrollDown => { if modal_open { return; } if let Some(_) = self.layout.scroll_at(m.column, m.row) { self.nav_down(); } }
            MouseEventKind::ScrollUp => { if modal_open { return; } if let Some(_) = self.layout.scroll_at(m.column, m.row) { self.nav_up(); } }
            _ => {}
        }
    }

    fn dispatch_click(&mut self, target: ClickTarget, tx: Sender<WorkerMsg>) {
        if self.profile_modal_open {
            match target {
                ClickTarget::ProfileSelect(i) => self.profile_selected = Some(i),
                ClickTarget::ProfileCreate => { self.profile_adding = true; self.profile_name_buffer.clear(); }
                ClickTarget::ProfileDelete(i) => self.delete_profile(i),
                ClickTarget::CloseModal => { if let Some(i) = self.profile_selected { self.switch_profile(i); } self.profile_modal_open = false; }
                _ => {}
            }
            return;
        }
        if self.download_modal_open { self.handle_download_modal_click(target, tx); return; }
        if self.modpack_modal_open {
            match target {
                ClickTarget::SaveModal => { let k = crossterm::event::KeyEvent::new(crossterm::event::KeyCode::Enter, crossterm::event::KeyModifiers::NONE); self.handle_modpack_modal_key(k, tx); }
                ClickTarget::CloseModal => { let k = crossterm::event::KeyEvent::new(crossterm::event::KeyCode::Esc, crossterm::event::KeyModifiers::NONE); self.handle_modpack_modal_key(k, tx); }
                ClickTarget::ModpackGameVersion(i) => self.modpack_selected_gv = Some(i),
                _ => {}
            }
            return;
        }
        if self.resource_modal_open {
            match target {
                ClickTarget::ResourceModalMcVersion(i) => { self.resource_modal_mc_idx = Some(i); self.resource_modal_sub_idx = Some(0); self.resource_modal_focus = crate::app::ResourceModalFocus::McVersion; }
                ClickTarget::ResourceModalSubVersion(i) => { self.resource_modal_sub_idx = Some(i); self.resource_modal_focus = crate::app::ResourceModalFocus::SubVersion; }
                ClickTarget::ResourceModalInstance(i) => { self.resource_modal_instance_idx = Some(i); self.resource_modal_focus = crate::app::ResourceModalFocus::Instance; }
                ClickTarget::ResourceModalConfirm => self.confirm_resource_download(tx),
                ClickTarget::ResourceModalCancel => self.resource_modal_open = false,
                _ => {}
            }
            return;
        }
        if self.busy && !matches!(target, ClickTarget::Tab(_)) { return; }

        match target {
            ClickTarget::Tab(t) => { self.tab = t; self.version_manager_mode = VersionManagerMode::List; self.managing_version = None; self.focus_area = FocusArea::Right; }
            ClickTarget::SettingsSideItem(s) => self.settings_side = s,
            ClickTarget::VersionSideItem(v) => self.switch_version_side(v),
            ClickTarget::OpenProfileManager => { self.profile_modal_open = true; self.profile_selected = Some(self.current_profile); self.profile_adding = false; },
            ClickTarget::OpenHomeVersions => { self.home_version_modal_open = true; self.home_version_selected = self.version_selected.unwrap_or(0); },
            ClickTarget::Launch | ClickTarget::QuickLaunch => self.start_launch_selected(tx),
            ClickTarget::PrevVersion => self.prev_local_version(),
            ClickTarget::NextVersion => self.next_local_version(),

            ClickTarget::VersionManageBack => { self.version_manager_mode = VersionManagerMode::List; self.managing_version = None; }
            ClickTarget::VersionManageSelect(m) => { self.version_manager_mode = m; if m == VersionManagerMode::Mods { self.refresh_mods(); } }
            ClickTarget::VersionLeftItem(i) => { self.version_left_selected = i; }
            ClickTarget::LocalVersion(i) if self.version_manager_mode == VersionManagerMode::List => {
                self.version_selected = Some(i); self.managing_version = Some(self.versions[i].name.clone()); self.version_manager_mode = VersionManagerMode::Basic;
            }
            ClickTarget::EditField(f) => self.begin_edit(f),
            ClickTarget::SetJavaPath(i) => { if let Some(path) = self.java_installs.get(i).cloned() { self.settings.java_path = path.clone(); self.save_settings(); self.push_log(format!("已切换 Java 路径: {}", path)); } }
            ClickTarget::VersionManageLaunch => self.start_launch_selected(tx),
            ClickTarget::VersionManageDelete => self.delete_selected_version(),
            ClickTarget::VersionManageCopy => self.copy_managed_version(),
            ClickTarget::VersionManageRename => self.begin_edit(EditField::VersionName),
            ClickTarget::ModToggle(i) => self.toggle_mod(i),
            ClickTarget::ModDelete(i) => self.delete_mod(i),
            ClickTarget::ModDownload => { self.tab = Tab::Downloads; self.version_side = VersionSide::Mod; },
            ClickTarget::OpenDir => self.push_log(format!("{}", self.mc_dir.display())),

            ClickTarget::LoaderFilter(ld) => { self.resource_loader = if self.resource_loader == Some(ld) { None } else { Some(ld) }; self.needs_hot = true; }
            ClickTarget::SortToggle => { self.resource_sort = match self.resource_sort { crate::worker::SortOrder::Downloads => crate::worker::SortOrder::Follows, crate::worker::SortOrder::Follows => crate::worker::SortOrder::Newest, crate::worker::SortOrder::Newest => crate::worker::SortOrder::Relevance, _ => crate::worker::SortOrder::Downloads }; self.needs_hot = true; }
            ClickTarget::ResourceHit(i) => self.resource_selected = Some(i),
            ClickTarget::SearchSubmit => self.search_editing = true,
            ClickTarget::DownloadSelectedResource => self.download_selected_resource(tx),
            ClickTarget::RemoteVersion(i) => self.remote_sel[self.version_side.index()] = Some(i),
            ClickTarget::NextPage => self.next_page(tx),
            ClickTarget::PrevPage => self.prev_page(tx),
            _ => {}
        }
    }
}