use crate::app::{App, ModEntry};
use std::fs;
use std::path::Path;

impl App {
    pub fn rename_managed_version(&mut self, new_name: &str) {
        let Some(old_name) = self.managing_version.clone() else { return; };
        if old_name == new_name { return; }
        let old_dir = self.mc_dir.join("versions").join(&old_name);
        let new_dir = self.mc_dir.join("versions").join(new_name);
        if new_dir.exists() {
            self.push_log(format!("版本 {} 已存在", new_name));
            return;
        }
        if fs::rename(&old_dir, &new_dir).is_ok() {
            let _ = fs::rename(new_dir.join(format!("{}.json", old_name)), new_dir.join(format!("{}.json", new_name)));
            let _ = fs::rename(new_dir.join(format!("{}.jar", old_name)), new_dir.join(format!("{}.jar", new_name)));
            
            if let Some(vs) = self.settings.versions.remove(&old_name) {
                self.settings.versions.insert(new_name.to_string(), vs);
                self.save_settings();
            }
            self.managing_version = Some(new_name.to_string());
            self.versions = crate::worker::meta::scan_versions(&self.mc_dir);
            self.push_log(format!("已重命名为 {}", new_name));
        }
    }

    pub fn copy_managed_version(&mut self) {
        let Some(old_name) = self.managing_version.clone() else { return; };
        let new_name = format!("{}_copy", old_name);
        let old_dir = self.mc_dir.join("versions").join(&old_name);
        let new_dir = self.mc_dir.join("versions").join(&new_name);
        if new_dir.exists() { return; }
        
        if copy_dir_all(&old_dir, &new_dir).is_ok() {
            let _ = fs::rename(new_dir.join(format!("{}.json", old_name)), new_dir.join(format!("{}.json", new_name)));
            let _ = fs::rename(new_dir.join(format!("{}.jar", old_name)), new_dir.join(format!("{}.jar", new_name)));
            
            let json_path = new_dir.join(format!("{}.json", new_name));
            if let Ok(s) = fs::read_to_string(&json_path) {
                if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&s) {
                    v["id"] = serde_json::Value::String(new_name.clone());
                    let _ = fs::write(&json_path, serde_json::to_string_pretty(&v).unwrap_or_default());
                }
            }
            self.versions = crate::worker::meta::scan_versions(&self.mc_dir);
            self.push_log(format!("已复制为 {}", new_name));
        }
    }

    pub fn refresh_mods(&mut self) {
        self.mods.clear();
        self.mod_selected = None;
        let Some(ver) = &self.managing_version else { return; };
        let mods_dir = self.mc_dir.join("versions").join(ver).join("mods");
        if let Ok(read) = fs::read_dir(mods_dir) {
            for e in read.flatten() {
                let path = e.path();
                if !path.is_file() { continue; }
                let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                if filename.ends_with(".jar") {
                    self.mods.push(ModEntry { filename, enabled: true });
                } else if filename.ends_with(".jar.disabled") {
                    self.mods.push(ModEntry { filename, enabled: false });
                }
            }
        }
        if !self.mods.is_empty() {
            self.mod_selected = Some(0);
        }
    }

    pub fn toggle_mod(&mut self, i: usize) {
        let Some(ver) = &self.managing_version else { return; };
        if i >= self.mods.len() { return; }
        let m = self.mods[i].clone();
        let mods_dir = self.mc_dir.join("versions").join(ver).join("mods");
        let old_path = mods_dir.join(&m.filename);
        let new_filename = if m.enabled {
            format!("{}.disabled", m.filename)
        } else {
            m.filename.replace(".disabled", "")
        };
        let new_path = mods_dir.join(new_filename);
        if fs::rename(old_path, new_path).is_ok() {
            self.refresh_mods();
        }
    }

    pub fn delete_mod(&mut self, i: usize) {
        let Some(ver) = &self.managing_version else { return; };
        if i >= self.mods.len() { return; }
        let m = self.mods[i].clone();
        let mods_dir = self.mc_dir.join("versions").join(ver).join("mods");
        if fs::remove_file(mods_dir.join(&m.filename)).is_ok() {
            self.refresh_mods();
        }
    }
    pub fn delete_selected_version(&mut self) {
        if let Some(v) = &self.managing_version {
            let path = self.mc_dir.join("versions").join(v);
            if std::fs::remove_dir_all(path).is_ok() {
                self.push_log(format!("已删除 {}", v));
                self.versions = crate::worker::meta::scan_versions(&self.mc_dir);
                self.version_manager_mode = crate::app::VersionManagerMode::List;
                self.managing_version = None;
            }
        }
    }
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}