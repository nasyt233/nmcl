use crate::app::{App, ClickTarget, WorkerMsg};
use crate::worker::ModLoader;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::mpsc::Sender;

impl App {
    pub fn open_download_modal(&mut self, tx: Sender<WorkerMsg>) {
        let Some(v) = self.selected_remote().cloned() else {
            self.push_log("没有选中可下载版本");
            return;
        };
        self.download_source = Some(v.clone());
        self.download_name_buffer = v.id.clone();
        self.download_loader = ModLoader::Vanilla;
        self.download_loader_index = 0;
        self.download_loader_versions.clear();
        self.download_loader_version = None;
        self.download_loader_ver_index = None;
        self.download_memory_buffer = self.settings.default_max_memory_mb.to_string();
        self.download_fetching_loader_versions = false;
        self.download_modal_open = true;
        
        let _ = tx; // 保持接口一致
    }

    pub fn close_download_modal(&mut self) {
        self.download_modal_open = false;
        self.download_source = None;
    }

    pub fn cycle_loader(&mut self, tx: Sender<WorkerMsg>) {
        let all = ModLoader::all();
        let idx = (self.download_loader_index + 1) % all.len();
        self.download_loader_index = idx;
        self.download_loader = all[idx];
        
        self.download_loader_versions.clear();
        self.download_loader_version = None;
        self.download_loader_ver_index = None;

        if self.download_loader == ModLoader::Vanilla {
            return;
        }
        let Some(v) = &self.download_source else { return; };
        let gv = v.id.clone();
        self.download_fetching_loader_versions = true;
        self.push_log(format!("获取 {} 可用版本（MC {}）...", self.download_loader.title(), gv));
        crate::worker::download::start_fetch_loader_versions(
            tx,
            self.cancel.clone(),
            self.download_loader,
            gv,
        );
    }

    pub fn confirm_download(&mut self, tx: Sender<WorkerMsg>) {
        let Some(v) = self.download_source.clone() else { return; };
        let name = self.download_name_buffer.trim().to_string();
        
        if name.is_empty() || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
            self.push_log("安装名称非法");
            return;
        }
        if self.mc_dir.join("versions").join(&name).exists() {
            self.push_log(format!("版本 {} 已存在", name));
            return;
        }
        let mem = self.download_memory_buffer.parse::<u32>().ok();

        // 检查加载器版本是否合法
        let loader_ver = if self.download_loader != ModLoader::Vanilla {
            let Some(idx) = self.download_loader_ver_index else {
                self.push_log("请选择加载器版本");
                return;
            };
            Some(self.download_loader_versions[idx].clone())
        } else {
            None
        };

        self.download_modal_open = false;
        self.cancel.store(false, std::sync::atomic::Ordering::Relaxed);
        self.busy = true;
        self.status = "下载中".into();
        self.progress_speed = 0.0;
        self.last_progress = None;
        
        let loader_name = match self.download_loader {
            ModLoader::Vanilla => "原版",
            ModLoader::Forge => "Forge",
            ModLoader::NeoForge => "NeoForge",
            ModLoader::Fabric => "Fabric",
            ModLoader::Quilt => "Quilt",
        };
        
        self.push_log(format!(
            "下载 {} → 安装为 {}（加载器: {}）",
            v.id, name, loader_name
        ));

        // 统一交给后台处理，后台会先确保原版下载，再安装加载器
        crate::worker::download::start_download_version(
            tx,
            self.cancel.clone(),
            self.mc_dir.clone(),
            v,
            name,
            if self.download_loader == ModLoader::Vanilla { None } else { Some(self.download_loader) },
            loader_ver,
            mem,
        );
    }

    pub fn handle_download_modal_key(&mut self, key: KeyEvent, tx: Sender<WorkerMsg>) -> bool {
        // 输入名称模式下
        if !self.download_name_buffer.is_empty() && key.modifiers.is_empty() {
            match key.code {
                KeyCode::Esc => { self.close_download_modal(); return false; }
                KeyCode::Enter => { self.confirm_download(tx); return false; }
                KeyCode::Tab => { self.cycle_loader(tx); return false; }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(i) = self.download_loader_ver_index {
                        if i > 0 {
                            self.download_loader_ver_index = Some(i - 1);
                            self.download_loader_version = self.download_loader_versions.get(i - 1).cloned();
                        }
                    }
                    return false;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.download_loader != ModLoader::Vanilla && !self.download_loader_versions.is_empty() {
                        let cur = self.download_loader_ver_index.unwrap_or(0);
                        let next = (cur + 1).min(self.download_loader_versions.len() - 1);
                        self.download_loader_ver_index = Some(next);
                        self.download_loader_version = self.download_loader_versions.get(next).cloned();
                    }
                    return false;
                }
                KeyCode::Backspace => { self.download_name_buffer.pop(); return false; }
                KeyCode::Char(c) => { self.download_name_buffer.push(c); return false; }
                _ => {}
            }
            return false;
        }

        match key.code {
            KeyCode::Esc => self.close_download_modal(),
            KeyCode::Enter => self.confirm_download(tx),
            KeyCode::Tab => self.cycle_loader(tx),
            _ => {}
        }
        false
    }

    pub fn handle_download_modal_click(&mut self, target: ClickTarget, tx: Sender<WorkerMsg>) {
        match target {
            ClickTarget::DownloadLoader => self.cycle_loader(tx),
            ClickTarget::ConfirmDownload => self.confirm_download(tx),
            ClickTarget::CancelDownload => self.close_download_modal(),
            _ => {}
        }
    }
}