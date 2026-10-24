pub mod core;
pub mod download_modal;
pub mod profiles;
pub mod resources;
pub mod versions;

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

pub use crate::worker::meta::{RemoteCategory, RemoteVersion};
pub use crate::worker::modrinth::{ModVersionInfo, ResourceHit};
pub use crate::worker::{ModLoader, SortOrder};

pub const PAGE_SIZE: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab { Home, Versions, Downloads, Settings }
impl Tab {
    pub fn title(self) -> &'static str {
        match self { Tab::Home => "主页", Tab::Versions => "版本", Tab::Downloads => "下载", Tab::Settings => "设置" }
    }
    pub fn next(self) -> Self {
        match self { Tab::Home => Tab::Versions, Tab::Versions => Tab::Downloads, Tab::Downloads => Tab::Settings, Tab::Settings => Tab::Home }
    }
    pub fn prev(self) -> Self {
        match self { Tab::Home => Tab::Settings, Tab::Versions => Tab::Home, Tab::Downloads => Tab::Versions, Tab::Settings => Tab::Downloads }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea { Left, Right }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile { pub id: String, pub name: String }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionManagerMode { List, Basic, Settings, Mods }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionSide {
    Local, Latest, Release, Snapshot, Old, AprilFools, Mod, Modpack, Resourcepack, Shader,
}
impl VersionSide {
    pub fn title(self) -> &'static str {
        match self {
            VersionSide::Local => "本地版本", VersionSide::Latest => "最新版", VersionSide::Release => "正式版", VersionSide::Snapshot => "快照版",
            VersionSide::Old => "远古版", VersionSide::AprilFools => "愚人节", VersionSide::Mod => "模组", VersionSide::Modpack => "整合包",
            VersionSide::Resourcepack => "资源包", VersionSide::Shader => "光影包",
        }
    }
    pub fn index(self) -> usize {
        match self {
            VersionSide::Local => 0, VersionSide::Latest => 1, VersionSide::Release => 2, VersionSide::Snapshot => 3,
            VersionSide::Old => 4, VersionSide::AprilFools => 5, VersionSide::Mod => 6, VersionSide::Modpack => 7,
            VersionSide::Resourcepack => 8, VersionSide::Shader => 9,
        }
    }
    pub fn is_mc(self) -> bool {
        matches!(self, VersionSide::Local | VersionSide::Latest | VersionSide::Release | VersionSide::Snapshot | VersionSide::Old | VersionSide::AprilFools)
    }
    pub fn is_community(self) -> bool { !self.is_mc() }
    pub fn all() -> [VersionSide; 10] {
        [VersionSide::Local, VersionSide::Latest, VersionSide::Release, VersionSide::Snapshot, VersionSide::Old, VersionSide::AprilFools, VersionSide::Mod, VersionSide::Modpack, VersionSide::Resourcepack, VersionSide::Shader]
    }
    pub fn mc_group() -> [VersionSide; 6] { [VersionSide::Latest, VersionSide::Release, VersionSide::Snapshot, VersionSide::Old, VersionSide::AprilFools, VersionSide::Local] }
    pub fn community_group() -> [VersionSide; 4] { [VersionSide::Mod, VersionSide::Modpack, VersionSide::Resourcepack, VersionSide::Shader] }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSide { General, Java, Launcher, About }
impl SettingsSide {
    pub fn title(self) -> &'static str {
        match self { SettingsSide::General => "通用", SettingsSide::Java => "Java", SettingsSide::Launcher => "启动器", SettingsSide::About => "关于" }
    }
    pub fn all() -> [SettingsSide; 4] { [SettingsSide::General, SettingsSide::Java, SettingsSide::Launcher, SettingsSide::About] }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditField { None, PlayerName, JavaPath, VersionName, Memory, DefaultMemory, JavaArgs, GameArgs }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceModalFocus { McVersion, SubVersion, Instance, Confirm }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionSettings {
    pub jvm_args: String,
    pub game_args: String,
    pub java_path: String,
    pub max_memory_mb: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherSettings {
    pub player_name: String,
    pub java_path: String,
    pub default_max_memory_mb: u32,
    pub versions: HashMap<String, VersionSettings>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub current_profile_id: Option<String>,
    #[serde(default)]
    pub selected_version: Option<String>,
}

impl LauncherSettings {
    pub fn load(mc_dir: &Path) -> Self {
        let path = mc_dir.join("nmcl_settings.json");
        if let Ok(s) = fs::read_to_string(&path) { 
            if let Ok(mut cfg) = serde_json::from_str::<Self>(&s) { 
                if cfg.profiles.is_empty() {
                    cfg.profiles.push(Profile { id: "0".into(), name: "Steve".into() });
                }
                return cfg; 
            } 
        }
        Self { 
            player_name: "Steve".into(), java_path: "java".into(), default_max_memory_mb: 2048, 
            versions: HashMap::new(), profiles: vec![Profile { id: "0".into(), name: "Steve".into() }],
            current_profile_id: None, selected_version: None
        }
    }
    pub fn save(&self, mc_dir: &Path) -> Result<(), String> {
        let s = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(mc_dir.join("nmcl_settings.json"), s).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn get_version(&self, id: &str) -> VersionSettings { self.versions.get(id).cloned().unwrap_or_default() }
}

#[derive(Debug, Clone)]
pub struct VersionEntry {
    pub name: String,
    pub loader: String,
    pub installed: bool,
    pub inherits: bool,
}

#[derive(Debug, Clone)]
pub struct ModEntry {
    pub filename: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum ClickTarget {
    Tab(Tab), VersionSideItem(VersionSide), SettingsSideItem(SettingsSide),
    LocalVersion(usize), RemoteVersion(usize), ResourceHit(usize), SearchSubmit, Launch, QuickLaunch, OpenVersionSettings,
    OpenProfileManager, ProfileSelect(usize), ProfileCreate, ProfileDelete(usize),
    VersionManageBack, VersionManageSelect(VersionManagerMode), VersionManageLaunch, VersionManageDetect, VersionManageDelete, VersionManageCopy, VersionManageRename,
    VersionLeftItem(usize),
    ModToggle(usize), ModDelete(usize), ModDownload,
    EditField(EditField), CloseModal, SaveModal,
    DownloadResource, LoaderFilter(ModLoader), SortToggle, GameVersionFilter, DownloadSelectedResource,
    ModpackGameVersion(usize), PrevPage, NextPage, DownloadName, DownloadLoader, DownloadLoaderVersion, DownloadMemory, ConfirmDownload, CancelDownload,
    ResourceModalMcVersion(usize), ResourceModalSubVersion(usize), ResourceModalInstance(usize), ResourceModalConfirm, ResourceModalCancel,
    ManageMemory, ManageRename, ManageDelete, ManageReinstallLoader, OpenDir,
    SetJavaPath(usize), PrevVersion, NextVersion, OpenHomeVersions,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum ScrollTarget { LocalList, RemoteList, ResourceList, ProfileList, ModList }

#[derive(Debug, Default)]
pub struct UiLayout {
    pub click_targets: Vec<(ratatui::layout::Rect, ClickTarget)>,
    pub scroll_areas: Vec<(ratatui::layout::Rect, ScrollTarget)>,
}
impl UiLayout {
    pub fn hit(&self, x: u16, y: u16) -> Option<ClickTarget> {
        self.click_targets.iter().find(|(r, _)| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height).map(|(_, t)| *t)
    }
    pub fn scroll_at(&self, x: u16, y: u16) -> Option<ScrollTarget> {
        self.scroll_areas.iter().find(|(r, _)| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height).map(|(_, t)| *t)
    }
}

#[allow(dead_code)]
pub enum WorkerMsg {
    Log(String), Progress { done: u64, total: u64, label: String },
    RemoteVersionsLoaded(Vec<RemoteVersion>), DownloadComplete(String),
    ResourceSearchResults { hits: Vec<ResourceHit>, total: usize }, ResourceDownloadComplete(String),
    ResourceVersionsLoaded { hit: ResourceHit, versions: Vec<ModVersionInfo> },
    LoaderVersionsLoaded { loader: ModLoader, versions: Vec<String> },
    VersionDownloadFailed(String), Finished(Result<(), String>),
}

#[allow(dead_code)]
pub struct App {
    pub should_quit: bool,
    pub tab: Tab,
    pub version_side: VersionSide,
    pub settings_side: SettingsSide,
    pub focus_area: FocusArea,

    pub home_version_modal_open: bool,
    pub home_version_selected: usize,
    pub version_left_selected: usize,
    pub right_action_selected: usize,

    pub profiles: Vec<Profile>,
    pub current_profile: usize,
    pub profile_selected: Option<usize>,
    pub profile_modal_open: bool,
    pub profile_adding: bool,
    pub profile_name_buffer: String,

    pub versions: Vec<VersionEntry>,
    pub version_selected: Option<usize>,
    pub version_manager_mode: VersionManagerMode,
    pub managing_version: Option<String>,
    pub mods: Vec<ModEntry>,
    pub mod_selected: Option<usize>,

    pub remote_versions: Vec<RemoteVersion>,
    pub remote_sel: [Option<usize>; 10],
    pub resource_results: Vec<ResourceHit>,
    pub resource_selected: Option<usize>,
    pub resource_query: String,
    pub resource_searching: bool,
    pub search_editing: bool,
    pub search_buffer: String,
    pub resource_downloading: bool,
    pub needs_hot: bool,
    pub resource_loader: Option<ModLoader>,
    pub resource_game_version: Option<String>,
    pub resource_sort: SortOrder,
    pub resource_game_versions: Vec<String>,

    pub local_page: usize,
    pub remote_page: [usize; 10],
    pub resource_page: usize,
    pub resource_total_hits: usize,

    pub settings: LauncherSettings,
    pub editing: EditField,
    pub edit_buffer: String,
    pub java_installs: Vec<String>,

    pub modpack_modal_open: bool,
    pub modpack_instance: Option<String>,
    pub modpack_game_versions: Vec<String>,
    pub modpack_selected_gv: Option<usize>,
    pub modpack_pending_hit: Option<ResourceHit>,

    pub download_modal_open: bool,
    pub download_source: Option<RemoteVersion>,
    pub download_name_buffer: String,
    pub download_loader: ModLoader,
    pub download_loader_index: usize,
    pub download_loader_versions: Vec<String>,
    pub download_loader_version: Option<String>,
    pub download_loader_ver_index: Option<usize>,
    pub download_memory_buffer: String,
    pub download_fetching_loader_versions: bool,

    pub resource_modal_open: bool,
    pub resource_modal_hit: Option<ResourceHit>,
    pub resource_modal_versions: Vec<ModVersionInfo>,
    pub resource_modal_mc_versions: Vec<String>,
    pub resource_modal_mc_idx: Option<usize>,
    pub resource_modal_sub_idx: Option<usize>,
    pub resource_modal_instances: Vec<String>,
    pub resource_modal_instance_idx: Option<usize>,
    pub resource_modal_focus: ResourceModalFocus,
    pub resource_modal_fetching: bool,

    pub status: String,
    pub busy: bool,
    pub progress: Option<(u64, u64, String)>,
    pub progress_speed: f64,
    pub last_progress: Option<(Instant, u64)>,
    pub logs: Vec<String>,
    pub mc_dir: PathBuf,
    pub cancel: Arc<AtomicBool>,
    pub layout: UiLayout,
    pub log_file: Option<BufWriter<fs::File>>,
    pub start_instant: Instant,
}

impl App {
    pub fn new(mc_dir: PathBuf) -> Self {
        let settings = LauncherSettings::load(&mc_dir);
        let log_file = open_log_file(&mc_dir);
        let java_installs = Self::scan_java_installs();
        let current_profile = settings.current_profile_id.as_ref()
            .and_then(|id| settings.profiles.iter().position(|p| p.id == *id))
            .unwrap_or(0);

        let mut app = Self {
            should_quit: false, tab: Tab::Home, version_side: VersionSide::Local, settings_side: SettingsSide::General,
            focus_area: FocusArea::Left,
            home_version_modal_open: false, home_version_selected: 0, version_left_selected: 0, right_action_selected: 0,
            profiles: settings.profiles.clone(),
            current_profile, profile_selected: Some(current_profile), profile_modal_open: false,
            profile_adding: false, profile_name_buffer: String::new(),
            versions: Vec::new(), version_selected: None, version_manager_mode: VersionManagerMode::List, managing_version: None,
            mods: Vec::new(), mod_selected: None,
            remote_versions: Vec::new(), remote_sel: [None; 10],
            resource_results: Vec::new(), resource_selected: None, resource_query: String::new(),
            resource_searching: false, search_editing: false, search_buffer: String::new(),
            resource_downloading: false, needs_hot: true, resource_loader: None,
            resource_game_version: None, resource_sort: SortOrder::Downloads, resource_game_versions: Vec::new(),
            local_page: 0, remote_page: [0; 10], resource_page: 0, resource_total_hits: 0,
            settings, editing: EditField::None, edit_buffer: String::new(),
            java_installs,
            modpack_modal_open: false, modpack_instance: None, modpack_game_versions: Vec::new(), modpack_selected_gv: None, modpack_pending_hit: None,
            download_modal_open: false, download_source: None, download_name_buffer: String::new(), download_loader: ModLoader::Vanilla, download_loader_index: 0, download_loader_versions: Vec::new(), download_loader_version: None, download_loader_ver_index: None, download_memory_buffer: String::new(), download_fetching_loader_versions: false,
            resource_modal_open: false, resource_modal_hit: None, resource_modal_versions: Vec::new(), resource_modal_mc_versions: Vec::new(), resource_modal_mc_idx: None, resource_modal_sub_idx: None, resource_modal_instances: Vec::new(), resource_modal_instance_idx: None, resource_modal_focus: ResourceModalFocus::McVersion, resource_modal_fetching: false,
            status: "就绪".into(), busy: false, progress: None, progress_speed: 0.0, last_progress: None,
            logs: Vec::new(), mc_dir, cancel: Arc::new(AtomicBool::new(false)), layout: UiLayout::default(), log_file, start_instant: Instant::now(),
        };
        app.push_log("启动器初始化");
        app
    }

    fn scan_java_installs() -> Vec<String> {
        let mut javas = Vec::new();
        let paths = ["/usr/lib/jvm", "/usr/java", "/opt/java", "/opt/jdk", "C:\\Program Files\\Java"];
        for p in paths {
            if let Ok(read) = std::fs::read_dir(p) {
                for e in read.flatten() {
                    let path = e.path().join("bin").join("java");
                    if path.exists() { javas.push(path.to_string_lossy().to_string()); }
                }
            }
        }
        if let Ok(home) = std::env::var("JAVA_HOME") {
            let path = std::path::PathBuf::from(home).join("bin").join("java");
            if path.exists() {
                let s = path.to_string_lossy().to_string();
                if !javas.contains(&s) { javas.push(s); }
            }
        }
        javas.sort(); javas.dedup(); javas
    }

    pub fn push_log(&mut self, line: impl Into<String>) {
        let s = line.into();
        let elapsed = self.start_instant.elapsed();
        let ts = format!("+{}.{:03}", elapsed.as_secs(), elapsed.subsec_millis());
        if let Some(f) = self.log_file.as_mut() { let _ = writeln!(f, "[{ts}] {s}"); let _ = f.flush(); }
        self.logs.push(s);
        if self.logs.len() > 500 { let n = self.logs.len() - 500; self.logs.drain(..n); }
    }

    pub fn flush_log(&mut self) { if let Some(f) = self.log_file.as_mut() { let _ = f.flush(); } }
    pub fn save_settings(&mut self) { let _ = self.settings.save(&self.mc_dir); }
    pub fn selected_local(&self) -> Option<&VersionEntry> { self.version_selected.and_then(|i| self.versions.get(i)) }

    pub fn remote_list(&self) -> Vec<&RemoteVersion> {
        match self.version_side {
            VersionSide::Latest => {
                let mut list = Vec::new();
                if let Some(r) = self.remote_versions.iter().find(|v| v.category == RemoteCategory::Release) { list.push(r); }
                if let Some(s) = self.remote_versions.iter().find(|v| v.category == RemoteCategory::Snapshot) { list.push(s); }
                list
            }
            VersionSide::Release => self.remote_versions.iter().filter(|v| v.category == RemoteCategory::Release).collect(),
            VersionSide::Snapshot => self.remote_versions.iter().filter(|v| v.category == RemoteCategory::Snapshot).collect(),
            VersionSide::Old => self.remote_versions.iter().filter(|v| v.version_type == "old_beta" || v.version_type == "old_alpha").collect(),
            VersionSide::AprilFools => self.remote_versions.iter().filter(|v| v.category == RemoteCategory::AprilFools).collect(),
            _ => Vec::new(),
        }
    }
    
    pub fn selected_remote(&self) -> Option<&RemoteVersion> {
        if !self.version_side.is_mc() || self.version_side == VersionSide::Local { return None; }
        let list = self.remote_list(); let idx = self.remote_sel[self.version_side.index()]?; list.get(idx).copied()
    }
    
    pub fn resource_modal_filtered_indices(&self) -> Vec<usize> {
        let Some(mc_idx) = self.resource_modal_mc_idx else { return Vec::new(); };
        let Some(mc) = self.resource_modal_mc_versions.get(mc_idx) else { return Vec::new(); };
        self.resource_modal_versions.iter().enumerate().filter(|(_, v)| v.game_versions.first().map(|s| s.as_str()) == Some(mc.as_str())).map(|(i, _)| i).collect()
    }
}

fn open_log_file(mc_dir: &Path) -> Option<BufWriter<fs::File>> {
    let path = mc_dir.join("nmcl.log");
    if let Some(p) = path.parent() { let _ = fs::create_dir_all(p); }
    let file = OpenOptions::new().create(true).append(true).open(&path).ok()?;
    let mut w = BufWriter::new(file);
    let _ = writeln!(w, "\n===== nmcl 启动 =====");
    Some(w)
}