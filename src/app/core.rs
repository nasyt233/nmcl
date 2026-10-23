use crate::app::{
    App, ClickTarget, EditField, Tab, VersionManagerMode, VersionSide, WorkerMsg, PAGE_SIZE, ResourceModalFocus,
};
use crate::worker::meta::RemoteCategory;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::{collections::HashSet, fs, io::Write, sync::{atomic::Ordering, mpsc::Sender}, time::Instant};

impl App {
    pub fn next_page(&mut self, _tx: Sender<WorkerMsg>) {
        if self.version_side.is_community() {
            let total = if self.resource_total_hits == 0 { 1 } else { (self.resource_total_hits + PAGE_SIZE - 1) / PAGE_SIZE };
            if self.resource_page + 1 < total {
                self.resource_page += 1;
                self.needs_hot = true;
            }
        } else if self.version_side.is_mc() {
            let idx = self.version_side.index();
            let len = self.remote_list().len();
            if (self.remote_page[idx] + 1) * PAGE_SIZE < len {
                self.remote_page[idx] += 1;
                self.remote_sel[idx] = Some(self.remote_page[idx] * PAGE_SIZE);
            }
        }
    }    
    pub fn prev_page(&mut self, _tx: Sender<WorkerMsg>) {
        if self.version_side.is_community() {
            if self.resource_page > 0 {
                self.resource_page -= 1;
                self.needs_hot = true;
            }
        } else if self.version_side.is_mc() {
            let idx = self.version_side.index();
            if self.remote_page[idx] > 0 {
                self.remote_page[idx] -= 1;
                self.remote_sel[idx] = Some(self.remote_page[idx] * PAGE_SIZE);
            }
        }
    }
    pub fn handle_scroll_up(&mut self) {
        if self.tab == Tab::Versions {
            if self.version_manager_mode == VersionManagerMode::Mods {
                if let Some(i) = self.mod_selected { if i > 0 { self.mod_selected = Some(i - 1); } }
            } else if self.version_manager_mode == VersionManagerMode::List {
                self.scroll_local(-1);
            }
        } else if self.tab == Tab::Downloads {
            if self.version_side == VersionSide::Local { self.scroll_local(-1); }
            else if self.version_side.is_community() { self.scroll_resource(-1); }
            else { self.scroll_remote(-1); }
        }
    }

    pub fn handle_scroll_down(&mut self) {
        if self.tab == Tab::Versions {
            if self.version_manager_mode == VersionManagerMode::Mods {
                if let Some(i) = self.mod_selected { if i + 1 < self.mods.len() { self.mod_selected = Some(i + 1); } }
                else if !self.mods.is_empty() { self.mod_selected = Some(0); }
            } else if self.version_manager_mode == VersionManagerMode::List {
                self.scroll_local(1);
            }
        } else if self.tab == Tab::Downloads {
            if self.version_side == VersionSide::Local { self.scroll_local(1); }
            else if self.version_side.is_community() { self.scroll_resource(1); }
            else { self.scroll_remote(1); }
        }
    }

    pub fn scroll_local(&mut self, delta: isize) {
        if self.versions.is_empty() { return; }
        let len = self.versions.len();
        let cur = self.version_selected.unwrap_or(self.local_page * PAGE_SIZE);
        let next = if delta > 0 { (cur + delta as usize).min(len - 1) } else { cur.saturating_sub((-delta) as usize) };
        self.version_selected = Some(next);
        self.local_page = next / PAGE_SIZE;
    }

    pub fn scroll_remote(&mut self, delta: isize) {
        let len = self.remote_list().len(); if len == 0 { return; }
        let idx = self.version_side.index();
        let cur = self.remote_sel[idx].unwrap_or(self.remote_page[idx] * PAGE_SIZE);
        let next = if delta > 0 { (cur + delta as usize).min(len - 1) } else { cur.saturating_sub((-delta) as usize) };
        self.remote_sel[idx] = Some(next); self.remote_page[idx] = next / PAGE_SIZE;
    }

    pub fn begin_edit(&mut self, field: EditField) {
        self.edit_buffer = match field {
            EditField::PlayerName => self.profiles[self.current_profile].name.clone(),
            EditField::JavaPath => self.settings.java_path.clone(),
            EditField::VersionName => self.managing_version.clone().unwrap_or_default(),
            EditField::Memory => {
                let mem = self.settings.get_version(self.managing_version.as_deref().unwrap_or("")).max_memory_mb.unwrap_or(2048);
                mem.to_string()
            }
            EditField::JavaArgs => self.settings.get_version(self.managing_version.as_deref().unwrap_or("")).jvm_args.clone(),
            EditField::GameArgs => self.settings.get_version(self.managing_version.as_deref().unwrap_or("")).game_args.clone(),
            _ => return,
        };
        self.editing = field;
    }

    pub fn commit_edit(&mut self) {
        let t = self.edit_buffer.trim().to_string();
        let ver = self.managing_version.clone().unwrap_or_default();
        match self.editing {
            EditField::PlayerName => { if !t.is_empty() { self.profiles[self.current_profile].name = t; } }
            EditField::JavaPath => { if !t.is_empty() { self.settings.java_path = t; self.save_settings(); } }
            EditField::VersionName => { if !t.is_empty() { self.rename_managed_version(&t); } }
            EditField::Memory => {
                if let Ok(mb) = t.parse::<u32>() {
                    let mut vs = self.settings.get_version(&ver); vs.max_memory_mb = Some(mb);
                    self.settings.versions.insert(ver, vs); self.save_settings();
                }
            }
            EditField::JavaArgs => {
                let mut vs = self.settings.get_version(&ver); vs.jvm_args = t;
                self.settings.versions.insert(ver, vs); self.save_settings();
            }
            EditField::GameArgs => {
                let mut vs = self.settings.get_version(&ver); vs.game_args = t;
                self.settings.versions.insert(ver, vs); self.save_settings();
            }
            _ => {}
        }
        self.editing = EditField::None; self.edit_buffer.clear();
    }
    
    pub fn cancel_edit(&mut self) { self.editing = EditField::None; self.edit_buffer.clear(); }

    pub fn confirm(&mut self, tx: Sender<WorkerMsg>) {
        if self.busy { return; }
        match self.tab {
            Tab::Home => self.start_launch_selected(tx),
            Tab::Downloads => {
                if self.version_side.is_community() { if self.resource_selected.is_some() { self.download_selected_resource(tx); } }
                else if self.selected_remote().is_some() { self.open_download_modal(tx); }
            }
            _ => {}
        }
    }
    pub fn cancel_current(&mut self) { if self.busy { self.cancel.store(true, Ordering::Relaxed); self.status = "取消中".into(); } }

    pub fn start_launch_selected(&mut self, tx: Sender<WorkerMsg>) {
        let Some(v) = self.selected_local() else { return; };
        //if v.inherits { self.push_log("不支持继承版本"); return; }
        if !v.installed { self.push_log("尚未安装"); return; }
        let version = v.name.clone();
        let player = self.profiles[self.current_profile].name.clone();
        let vs = self.settings.get_version(&version);
        let java = if !vs.java_path.trim().is_empty() { vs.java_path.clone() } else { self.settings.java_path.clone() };

        self.cancel.store(false, Ordering::Relaxed); self.busy = true; self.status = "启动中".into();
        self.push_log(format!("启动 {version}（玩家: {player}）"));
        crate::worker::launch::start_launch(tx, self.cancel.clone(), self.mc_dir.clone(), version, player, java, vs);
    }

    pub fn handle_worker_msg(&mut self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::Log(s) => {
                // 过滤掉包含 http 的长链接日志，只在 nmcl.log 记录
                if s.contains("http://") || s.contains("https://") {
                    let elapsed = self.start_instant.elapsed();
                    let ts = format!("+{}.{:03}", elapsed.as_secs(), elapsed.subsec_millis());
                    if let Some(f) = self.log_file.as_mut() {
                        let _ = writeln!(f, "[{ts}] [DEBUG] {s}");
                        let _ = f.flush();
                    }
                } else {
                    self.push_log(s);
                }
            }
            WorkerMsg::Progress { done, total, label } => {
                self.progress = Some((done, total, label.clone()));
                let now = Instant::now();
                
                // 计算下载速度
                if let Some((last, last_done)) = self.last_progress {
                    let dt = now.duration_since(last).as_secs_f64();
                    if dt > 0.5 && done > last_done { 
                        self.progress_speed = (done - last_done) as f64 / dt; 
                        self.last_progress = Some((now, done)); 
                    }
                } else { 
                    self.last_progress = Some((now, done)); 
                }
    
                // 翻译进度标签，并每隔一段进度写入一次日志，防止刷屏
                if total > 0 {
                    let should_log = done == 1 || done == total || done % (total / 4).max(1) == 0;
                    if should_log {
                        let cn_label = match label.as_str() {
                            "libraries" => "依赖库",
                            "assets" => "游戏资源",
                            "modpack" => "整合包",
                            "resource" => "资源文件",
                            _ => label.as_str(),
                        };
                        let pct = (done as f64 / total as f64) * 100.0;
                        self.push_log(format!("[进度] 正在下载{} {:.0}% ({}/{})", cn_label, pct, done, total));
                    }
                }
            }
            WorkerMsg::RemoteVersionsLoaded(versions) => {
                let local: HashSet<String> = self.versions.iter().map(|v| v.name.clone()).collect();
                self.remote_versions = versions.into_iter().filter(|v| !local.contains(&v.id)).collect();
                
                // 填充资源页的游戏版本筛选列表
                let mut gvs: Vec<String> = self.remote_versions.iter().filter(|v| v.category == RemoteCategory::Release).map(|v| v.id.clone()).collect();
                gvs.sort_by(|a, b| b.cmp(a)); 
                gvs.truncate(20); 
                self.resource_game_versions = gvs;
                
                for side in [VersionSide::Release, VersionSide::Snapshot, VersionSide::AprilFools] {
                    let idx = side.index();
                    let count = self.remote_versions.iter().filter(|v| v.category == self.category_of(side)).count();
                    if count > 0 && self.remote_sel[idx].is_none() { self.remote_sel[idx] = Some(0); }
                }
                self.push_log("[信息] 远程原版游戏版本清单已加载完成");
            }
            WorkerMsg::DownloadComplete(id) => {
                self.push_log(format!("[完成] 游戏版本 {} 安装成功！", id));
                self.versions = crate::worker::meta::scan_versions(&self.mc_dir);
                self.version_selected = Some(0);
            }
            WorkerMsg::ResourceSearchResults { hits, total } => {
                self.resource_searching = false; 
                self.resource_results = hits; 
                self.resource_total_hits = total;
                if !self.resource_results.is_empty() { self.resource_selected = Some(0); }
                self.push_log(format!("[信息] 资源搜索完成，共找到 {} 个结果", self.resource_total_hits));
            }
            WorkerMsg::ResourceDownloadComplete(name) => { 
                self.resource_downloading = false; 
                self.push_log(format!("[完成] {} 下载完成！", name)); 
            }
            WorkerMsg::ResourceVersionsLoaded { hit, versions } => {
                self.resource_modal_fetching = false;
                if self.resource_modal_hit.as_ref().map(|h| h.project_id == hit.project_id).unwrap_or(false) {
                    let mut mc_versions = Vec::new(); 
                    let mut last: Option<String> = None;
                    for v in &versions {
                        let mc = v.game_versions.first().cloned().unwrap_or_default();
                        if Some(&mc) != last.as_ref() { 
                            mc_versions.push(mc.clone()); 
                            last = Some(mc); 
                        }
                    }
                    self.resource_modal_versions = versions; 
                    self.resource_modal_mc_versions = mc_versions;
                    if !self.resource_modal_mc_versions.is_empty() { 
                        self.resource_modal_mc_idx = Some(0); 
                        self.resource_modal_sub_idx = Some(0); 
                    }
                    self.push_log(format!("[信息] {} 的版本列表获取成功", hit.title));
                }
            }
            WorkerMsg::LoaderVersionsLoaded { loader, versions } => {
                self.download_fetching_loader_versions = false;
                if self.download_loader == loader { 
                    self.download_loader_versions = versions; 
                    self.download_loader_ver_index = Some(0); 
                }
                self.push_log(format!("[信息] {} 加载器版本获取成功", loader.title()));
            }
            WorkerMsg::VersionDownloadFailed(e) => {
                self.push_log(format!("[错误] 版本下载失败: {}", e));
            }
            WorkerMsg::Finished(res) => {
                self.busy = false; 
                self.resource_downloading = false; 
                self.download_fetching_loader_versions = false; 
                self.progress = None;
                match res { 
                    Ok(()) => {
                        self.status = "就绪".into();
                        self.push_log("[完成] 所有任务已顺利结束");
                    }, 
                    Err(e) => { 
                        self.status = "错误".into(); 
                        self.push_log(format!("[错误] 任务异常中断: {}", e)); 
                    } 
                }
            }
        }
    }

    fn category_of(&self, side: VersionSide) -> RemoteCategory {
        match side { VersionSide::Latest | VersionSide::Release => RemoteCategory::Release, VersionSide::Snapshot => RemoteCategory::Snapshot, VersionSide::AprilFools => RemoteCategory::AprilFools, _ => RemoteCategory::Release }
    }

    pub fn handle_mouse(&mut self, m: MouseEvent, tx: Sender<WorkerMsg>) {
        let modal_open = self.resource_modal_open || self.profile_modal_open || self.download_modal_open || self.modpack_modal_open;
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => { if let Some(t) = self.layout.hit(m.column, m.row) { self.dispatch_click(t, tx); } }
            MouseEventKind::ScrollDown => { if modal_open { return; } if let Some(_) = self.layout.scroll_at(m.column, m.row) { self.handle_scroll_down(); } }
            MouseEventKind::ScrollUp => { if modal_open { return; } if let Some(_) = self.layout.scroll_at(m.column, m.row) { self.handle_scroll_up(); } }
            _ => {}
        }
    }

    fn dispatch_click(&mut self, target: ClickTarget, tx: Sender<WorkerMsg>) {
        if self.profile_modal_open {
            match target {
                ClickTarget::ProfileSelect(i) => self.profile_selected = Some(i),
                ClickTarget::ProfileCreate => { self.profiles.push(crate::app::Profile { id: uuid::Uuid::new_v4().to_string(), name: "New Player".into() }); self.profile_selected = Some(self.profiles.len()-1); }
                ClickTarget::ProfileDelete(i) => { if self.profiles.len() > 1 { self.profiles.remove(i); self.profile_selected = Some(0); if self.current_profile >= self.profiles.len() { self.current_profile = 0; } } }
                ClickTarget::CloseModal => { if let Some(i) = self.profile_selected { self.current_profile = i; self.settings.player_name = self.profiles[i].name.clone(); self.save_settings(); } self.profile_modal_open = false; }
                _ => {}
            }
            return;
        }
        
        if self.resource_modal_open {
            match target {
                ClickTarget::ResourceModalMcVersion(i) => { self.resource_modal_mc_idx = Some(i); self.resource_modal_sub_idx = Some(0); self.resource_modal_focus = ResourceModalFocus::McVersion; }
                ClickTarget::ResourceModalSubVersion(i) => { self.resource_modal_sub_idx = Some(i); self.resource_modal_focus = ResourceModalFocus::SubVersion; }
                ClickTarget::ResourceModalInstance(i) => { self.resource_modal_instance_idx = Some(i); self.resource_modal_focus = ResourceModalFocus::Instance; }
                ClickTarget::ResourceModalConfirm => self.confirm_resource_download(tx),
                ClickTarget::ResourceModalCancel => self.resource_modal_open = false,
                _ => {}
            }
            return;
        }

        if self.busy && !matches!(target, ClickTarget::Tab(_)) { return; }

        match target {
            ClickTarget::Tab(t) => { self.tab = t; self.version_manager_mode = VersionManagerMode::List; self.managing_version = None; }
            ClickTarget::OpenProfileManager => { self.profile_modal_open = true; self.profile_selected = Some(self.current_profile); }
            ClickTarget::Launch | ClickTarget::QuickLaunch => self.start_launch_selected(tx),
            
            ClickTarget::VersionManageBack => { self.version_manager_mode = VersionManagerMode::List; self.managing_version = None; }
            ClickTarget::VersionManageSelect(mode) => { 
                self.version_manager_mode = mode; 
                if mode == VersionManagerMode::Mods { self.refresh_mods(); }
            }
            ClickTarget::LocalVersion(i) => { 
                if self.version_manager_mode == VersionManagerMode::List {
                    self.version_selected = Some(i); 
                    self.managing_version = Some(self.versions[i].name.clone());
                    self.version_manager_mode = VersionManagerMode::Basic;
                }
            }
            ClickTarget::EditField(f) => self.begin_edit(f),
            ClickTarget::VersionManageLaunch => self.start_launch_selected(tx),
            ClickTarget::VersionManageDetect => {
                self.push_log(format!("检测 {} 完整性...", self.managing_version.as_deref().unwrap_or("")));
            }
            ClickTarget::VersionManageDelete => {
                if let Some(v) = &self.managing_version {
                    let path = self.mc_dir.join("versions").join(v);
                    if fs::remove_dir_all(path).is_ok() {
                        self.push_log(format!("已删除 {v}"));
                        self.versions = crate::worker::meta::scan_versions(&self.mc_dir);
                        self.version_manager_mode = VersionManagerMode::List;
                    }
                }
            }
            ClickTarget::VersionManageCopy => { self.copy_managed_version(); }
            ClickTarget::VersionManageRename => self.begin_edit(EditField::VersionName),
            ClickTarget::ModToggle(i) => self.toggle_mod(i),
            ClickTarget::ModDelete(i) => self.delete_mod(i),
            ClickTarget::ModDownload => { self.tab = crate::app::Tab::Downloads; self.version_side = crate::app::VersionSide::Mod; },
            ClickTarget::OpenDir => { self.push_log(format!("{}", self.mc_dir.display())); },
            
            ClickTarget::VersionSideItem(v) => { self.version_side = v; if v.is_community() { self.needs_hot = true; } }
            ClickTarget::LoaderFilter(ld) => { self.resource_loader = if self.resource_loader == Some(ld) { None } else { Some(ld) }; self.needs_hot = true; }
            ClickTarget::SortToggle => { self.resource_sort = match self.resource_sort { crate::worker::SortOrder::Downloads => crate::worker::SortOrder::Follows, crate::worker::SortOrder::Follows => crate::worker::SortOrder::Newest, crate::worker::SortOrder::Newest => crate::worker::SortOrder::Relevance, _ => crate::worker::SortOrder::Downloads }; self.needs_hot = true; }
            ClickTarget::ResourceHit(i) => self.resource_selected = Some(i),
            ClickTarget::SearchSubmit => self.search_editing = true,
            ClickTarget::DownloadSelectedResource => self.download_selected_resource(tx),
            ClickTarget::RemoteVersion(i) => { self.remote_sel[self.version_side.index()] = Some(i); }
            ClickTarget::NextPage => self.next_page(tx),
            ClickTarget::PrevPage => self.prev_page(tx),
            
            _ => {}
        }
    }
}