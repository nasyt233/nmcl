use crate::app::{App, ClickTarget, WorkerMsg};
use crate::worker::ModLoader;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::mpsc::Sender;

impl App {
    /// 在版本页选中远程版本按 Enter 时调用
    pub fn open_download_modal(&mut self, tx: Sender<WorkerMsg>) {
        let Some(v) = self.selected_remote().cloned() else {
            self.push_log("没有选中可下载版本");
            return;
        };
        let default_name = v.id.clone();
        let mem = self.settings.default_max_memory_mb.to_string();
        self.download_source = Some(v);
        self.download_name_buffer = default_name;
        self.download_loader = ModLoader::Vanilla;
        self.download_loader_index = 0;
        self.download_loader_versions.clear();
        self.download_loader_version = None;
        self.download_loader_ver_index = None;
        self.download_memory_buffer = mem;
        self.download_fetching_loader_versions = false;
        self.download_modal_open = true;
        let _ = tx;
    }

    pub fn close_download_modal(&mut self) {
        self.download_modal_open = false;
        self.download_source = None;
    }

    /// 切换加载器时按需拉取可用版本
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
        let Some(v) = &self.download_source else {
            return;
        };
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
        let Some(v) = self.download_source.clone() else {
            return;
        };
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

        // 加载器版本校验
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
        self.push_log(format!(
            "下载 {} → 安装为 {}（加载器: {}）",
            v.id,
            name,
            self.download_loader.title()
        ));

        crate::worker::download::start_download_version(
            tx,
            self.cancel.clone(),
            self.mc_dir.clone(),
            v,
            name,
            if self.download_loader == ModLoader::Vanilla {
                None
            } else {
                Some(self.download_loader)
            },
            loader_ver,
            mem,
        );
    }

    pub fn handle_download_modal_key(&mut self, key: KeyEvent, tx: Sender<WorkerMsg>) -> bool {
        // 统一在文本输入字段上处理
        let editing_name = self.download_modal_open
            && !self.download_name_buffer.is_empty()
            && key.modifiers.is_empty();
        // 简化：用 Tab 切换焦点，但当前版本只用一个“当前编辑目标”字段
        // 这里用一个更直接的方案：所有 Char/Backspace 只作用于 name（最常见的输入）
        // 内存框用 Ctrl+方向键或单独热键切换——为保持简单，先只允许 name 输入
        match key.code {
            KeyCode::Esc => {
                self.close_download_modal();
                return false;
            }
            KeyCode::Enter => {
                self.confirm_download(tx);
                return false;
            }
            KeyCode::Tab => {
                self.cycle_loader(tx);
                return false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(i) = self.download_loader_ver_index {
                    if i > 0 {
                        self.download_loader_ver_index = Some(i - 1);
                        self.download_loader_version =
                            self.download_loader_versions.get(i - 1).cloned();
                    }
                }
                return false;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.download_loader != ModLoader::Vanilla
                    && !self.download_loader_versions.is_empty()
                {
                    let cur = self.download_loader_ver_index.unwrap_or(0);
                    let next = (cur + 1).min(self.download_loader_versions.len() - 1);
                    self.download_loader_ver_index = Some(next);
                    self.download_loader_version =
                        self.download_loader_versions.get(next).cloned();
                }
                return false;
            }
            KeyCode::Backspace => {
                if editing_name {
                    self.download_name_buffer.pop();
                }
                return false;
            }
            KeyCode::Char(c) => {
                if editing_name {
                    self.download_name_buffer.push(c);
                }
                return false;
            }
            _ => {}
        }
        false
    }

    /// 点击弹窗内的某个可点区域
    #[allow(dead_code)]
    pub fn handle_download_modal_click(&mut self, target: ClickTarget, tx: Sender<WorkerMsg>) {
        match target {
            ClickTarget::DownloadLoader => self.cycle_loader(tx),
            ClickTarget::ConfirmDownload => self.confirm_download(tx),
            ClickTarget::CancelDownload => self.close_download_modal(),
            _ => {}
        }
    }
}