use crate::app::{ModLoader, SortOrder, WorkerMsg};
use crate::worker::meta::{download_bytes, download_to_file};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    thread,
};
use zip::ZipArchive;

// ============================================================
// 资源分类
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceCategory {
    Modpack,
    Mod,
    Resourcepack,
    Shader,
}

impl ResourceCategory {
    #[allow(dead_code)]
    pub fn title(self) -> &'static str {
        match self {
            ResourceCategory::Modpack => "整合包",
            ResourceCategory::Mod => "模组",
            ResourceCategory::Resourcepack => "资源包",
            ResourceCategory::Shader => "光影包",
        }
    }
    pub fn target_dir(self) -> &'static str {
        match self {
            ResourceCategory::Modpack => "modpacks",
            ResourceCategory::Mod => "mods",
            ResourceCategory::Resourcepack => "resourcepacks",
            ResourceCategory::Shader => "shaderpacks",
        }
    }
    fn api_type(self) -> &'static str {
        match self {
            ResourceCategory::Modpack => "modpack",
            ResourceCategory::Mod => "mod",
            ResourceCategory::Resourcepack => "resourcepack",
            ResourceCategory::Shader => "shader",
        }
    }
}

// ============================================================
// 结构体
// ============================================================

#[derive(Debug, Clone)]
pub struct ResourceHit {
    pub project_id: String,
    pub title: String,
    pub author: String,
    pub downloads: u64,
    pub project_type: String,
}

#[derive(Debug, Clone)]
pub struct ModVersionInfo {
    #[allow(dead_code)]
    pub id: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    #[allow(dead_code)]
    pub loaders: Vec<String>,
    pub filename: String,
    pub download_url: String,
}

#[derive(Deserialize)]
struct SearchResp {
    hits: Vec<Hit>,
    total_hits: usize,
}

#[derive(Deserialize)]
struct Hit {
    project_id: String,
    name: String,
    author: String,
    downloads: u64,
    project_types: Vec<String>,
}

#[derive(Deserialize)]
struct VersionResp {
    id: String,
    version_number: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    files: Vec<VersionFile>,
}

#[derive(Deserialize, Clone)]
struct VersionFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
}

// ============================================================
// 搜索与获取版本
// ============================================================

pub fn start_search(
    tx: Sender<WorkerMsg>,
    cancel: Arc<AtomicBool>,
    category: ResourceCategory,
    query: String,
    loader: Option<ModLoader>,
    game_version: Option<String>,
    sort: SortOrder,
    offset: usize,
    limit: usize,
) {
    thread::spawn(move || {
        let log = |s: String| { let _ = tx.send(WorkerMsg::Log(s)); };

        let res = (|| -> Result<(Vec<ResourceHit>, usize), String> {
            let mut filters = vec![format!("project_types=\"{}\"", category.api_type())];

            if let Some(ld) = loader {
                let loader_str = loader_to_modrinth(ld);
                if !loader_str.is_empty() {
                    filters.push(format!("loaders=\"{}\"", loader_str));
                }
            }
            if let Some(gv) = &game_version {
                if !gv.is_empty() {
                    filters.push(format!("game_versions=\"{}\"", gv));
                }
            }

            let filter_expr = filters.join(" AND ");
            let filter_enc = urlencoding::encode(&filter_expr);

            let index = match sort {
                SortOrder::Downloads => "downloads",
                SortOrder::Follows => "follows",
                SortOrder::Newest => "newest",
                SortOrder::Relevance => "relevance",
            };

            let url = if query.trim().is_empty() {
                format!("https://api.modrinth.com/v3/search?new_filters={}&index={}&limit={}&offset={}", filter_enc, index, limit, offset)
            } else {
                format!("https://api.modrinth.com/v3/search?query={}&new_filters={}&index={}&limit={}&offset={}", urlencoding::encode(&query), filter_enc, index, limit, offset)
            };

            log(format!("[Modrinth] 请求: {url}"));

            let resp = match ureq::get(&url).call() {
                Ok(r) => r,
                Err(ureq::Error::Status(code, r)) => {
                    let body = r.into_string().unwrap_or_else(|_| "<无法读取响应体>".into());
                    return Err(format!("HTTP {code} 错误\n服务器返回: {body}"));
                }
                Err(ureq::Error::Transport(t)) => return Err(format!("网络/传输错误: {t}")),
            };

            let bytes = {
                let mut buf = Vec::new();
                resp.into_reader().read_to_end(&mut buf).map_err(|e| format!("读取失败: {e}"))?;
                buf
            };

            if cancel.load(Ordering::Relaxed) { return Err("已取消".into()); }

            let resp: SearchResp = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    let preview = String::from_utf8_lossy(&bytes);
                    let preview: String = preview.chars().take(400).collect();
                    return Err(format!("JSON 解析失败: {e}\n原始响应(前400字符): {preview}"));
                }
            };

            let hits = resp.hits.into_iter().map(|h| ResourceHit {
                project_id: h.project_id,
                title: h.name,
                author: h.author,
                downloads: h.downloads,
                project_type: h.project_types.first().cloned().unwrap_or_default(),
            }).collect();
            
            Ok((hits, resp.total_hits))
        })();

        match res {
            Ok((hits, total)) => { let _ = tx.send(WorkerMsg::ResourceSearchResults { hits, total }); }
            Err(e) => {
                log(format!("[Modrinth] 搜索失败: {e}"));
                let _ = tx.send(WorkerMsg::ResourceSearchResults { hits: Vec::new(), total: 0 });
            }
        }
    });
}

fn loader_to_modrinth(l: ModLoader) -> &'static str {
    match l {
        ModLoader::Forge => "forge",
        ModLoader::NeoForge => "neoforge",
        ModLoader::Fabric => "fabric",
        ModLoader::Quilt => "quilt",
        ModLoader::Vanilla => "",
    }
}

pub fn start_fetch_resource_versions(
    tx: Sender<WorkerMsg>,
    cancel: Arc<AtomicBool>,
    hit: ResourceHit,
    loader: Option<ModLoader>,
    game_version: Option<String>,
) {
    thread::spawn(move || {
        let log = |s: String| { let _ = tx.send(WorkerMsg::Log(s)); };

        let res = (|| -> Result<Vec<ModVersionInfo>, String> {
            let mut qs: Vec<String> = Vec::new();
            if let Some(ld) = loader {
                let l = loader_to_modrinth(ld);
                if !l.is_empty() {
                    qs.push(format!("loaders={}", urlencoding::encode(&format!("[\"{}\"]", l))));
                }
            }
            if let Some(gv) = &game_version {
                if !gv.is_empty() {
                    qs.push(format!("game_versions={}", urlencoding::encode(&format!("[\"{}\"]", gv))));
                }
            }
            let query = if qs.is_empty() { String::new() } else { format!("?{}", qs.join("&")) };

            let url = format!("https://api.modrinth.com/v3/project/{}/version{}", hit.project_id, query);
            log(format!("[Modrinth] 获取版本: {url}"));

            let bytes = download_bytes(&url)?;
            if cancel.load(Ordering::Relaxed) { return Err("已取消".into()); }

            let versions: Vec<VersionResp> = serde_json::from_slice(&bytes).map_err(|e| format!("解析版本失败: {e}"))?;

            let mut out: Vec<ModVersionInfo> = versions.into_iter().filter_map(|v| {
                let file = v.files.iter().find(|f| f.primary).or_else(|| v.files.first()).cloned()?;
                Some(ModVersionInfo {
                    id: v.id,
                    version_number: v.version_number,
                    game_versions: v.game_versions,
                    loaders: v.loaders,
                    filename: file.filename,
                    download_url: file.url,
                })
            }).collect();

            out.sort_by(|a, b| {
                let ka = mc_version_key(a.game_versions.first().map(|s| s.as_str()).unwrap_or(""));
                let kb = mc_version_key(b.game_versions.first().map(|s| s.as_str()).unwrap_or(""));
                kb.cmp(&ka).then_with(|| b.version_number.cmp(&a.version_number))
            });

            Ok(out)
        })();

        match res {
            Ok(versions) => { let _ = tx.send(WorkerMsg::ResourceVersionsLoaded { hit, versions }); }
            Err(e) => {
                log(format!("[Modrinth] 获取版本失败: {e}"));
                let _ = tx.send(WorkerMsg::ResourceVersionsLoaded { hit, versions: Vec::new() });
            }
        }
    });
}

fn mc_version_key(v: &str) -> Vec<u32> {
    v.split('.').filter_map(|s| s.parse::<u32>().ok()).collect()
}

// ============================================================
// 下载普通资源（包含进度汇报）
// ============================================================

pub fn start_download_resource(
    tx: Sender<WorkerMsg>,
    cancel: Arc<AtomicBool>,
    mc_dir: PathBuf,
    hit: ResourceHit,
    version: ModVersionInfo,
    instance: Option<String>,
) {
    thread::spawn(move || {
        let res = do_download_resource(&tx, &cancel, &mc_dir, &hit, &version, instance);
        match res {
            Ok(name) => {
                let _ = tx.send(WorkerMsg::ResourceDownloadComplete(name));
                let _ = tx.send(WorkerMsg::Finished(Ok(())));
            }
            Err(e) => { let _ = tx.send(WorkerMsg::Finished(Err(e))); }
        }
    });
}

fn do_download_resource(
    tx: &Sender<WorkerMsg>,
    cancel: &Arc<AtomicBool>,
    mc_dir: &std::path::Path,
    hit: &ResourceHit,
    version: &ModVersionInfo,
    instance: Option<String>,
) -> Result<String, String> {
    let log = |s: String| { let _ = tx.send(WorkerMsg::Log(s)); };

    if version.download_url.is_empty() { return Err("该版本没有可下载的文件".into()); }

    let category = match hit.project_type.as_str() {
        "modpack" => ResourceCategory::Modpack,
        "mod" => ResourceCategory::Mod,
        "resourcepack" => ResourceCategory::Resourcepack,
        "shader" => ResourceCategory::Shader,
        _ => ResourceCategory::Mod,
    };

    let dest_dir = if let Some(inst) = &instance {
        mc_dir.join("versions").join(inst).join(category.target_dir())
    } else {
        mc_dir.join(category.target_dir())
    };
    fs::create_dir_all(&dest_dir).map_err(|e| format!("创建目录失败: {e}"))?;
    let dest = dest_dir.join(&version.filename);

    log(format!("[下载] 开始下载 {} ({})", hit.title, version.version_number));    
    let _ = tx.send(WorkerMsg::Progress { done: 1, total: 2, label: "resource".into() });
    download_to_file(tx, cancel, &version.download_url, &dest, None)?;    
    let _ = tx.send(WorkerMsg::Progress { done: 2, total: 2, label: "resource".into() });

    Ok(hit.title.clone())
}

// ============================================================
// 下载整合包（包含解压和文件下载）
// ============================================================

pub fn start_download_modpack(
    tx: Sender<WorkerMsg>,
    cancel: Arc<AtomicBool>,
    mc_dir: PathBuf,
    hit: ResourceHit,
    game_version: String,
) {
    thread::spawn(move || {
        let res = do_download_modpack(&tx, &cancel, &mc_dir, &hit, &game_version);
        match res {
            Ok(name) => {
                let _ = tx.send(WorkerMsg::ResourceDownloadComplete(name));
                let _ = tx.send(WorkerMsg::Finished(Ok(())));
            }
            Err(e) => { let _ = tx.send(WorkerMsg::Finished(Err(e))); }
        }
    });
}

fn do_download_modpack(
    tx: &Sender<WorkerMsg>,
    cancel: &Arc<AtomicBool>,
    mc_dir: &std::path::Path,
    hit: &ResourceHit,
    game_version: &str,
) -> Result<String, String> {
    let log = |s: String| { let _ = tx.send(WorkerMsg::Log(s)); };

    let url = format!("https://api.modrinth.com/v3/project/{}/version?game_versions={}", hit.project_id, urlencoding::encode(&format!("[\"{}\"]", game_version)));
    log(format!("获取整合包版本: {url}"));
    let bytes = download_bytes(&url)?;
    let versions: Vec<VersionResp> = serde_json::from_slice(&bytes).map_err(|e| format!("解析版本失败: {e}"))?;
    let Some(ver) = versions.first() else { return Err(format!("该整合包没有 MC {} 的发布", game_version)); };
    let Some(file) = ver.files.iter().find(|f| f.primary) else { return Err("找不到 mrpack 文件".into()); };

    let tmp_dir = mc_dir.join("modpacks").join("_tmp");
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建临时目录失败: {e}"))?;
    let mrpack_path = tmp_dir.join(&file.filename);
    log(format!("[下载] 正在获取整合包主文件 {}", file.url));
    download_to_file(tx, cancel, &file.url, &mrpack_path, None)?;

    let file_handle = fs::File::open(&mrpack_path).map_err(|e| format!("打开 mrpack 失败: {e}"))?;
    let mut archive = ZipArchive::new(file_handle).map_err(|e| format!("解析 zip 失败: {e}"))?;

    let mut index_str = String::new();
    {
        let mut entry = archive.by_name("modrinth.index.json").map_err(|e| format!("找不到 modrinth.index.json: {e}"))?;
        entry.read_to_string(&mut index_str).map_err(|e| format!("读取 index 失败: {e}"))?;
    }

    let index: MrpackIndex = serde_json::from_str(&index_str).map_err(|e| format!("解析 index 失败: {e}"))?;
    let instance_name = format!("{}-{}", sanitize(&hit.title), game_version);
    let instance_dir = mc_dir.join("versions").join(&instance_name);
    fs::create_dir_all(&instance_dir).map_err(|e| format!("创建实例目录失败: {e}"))?;

    extract_override(&mut archive, "overrides", &instance_dir)?;
    extract_override(&mut archive, "client-overrides", &instance_dir)?;

    let total = index.files.len() as u64;
    log(format!("[信息] 整合包包含 {} 个额外的模组/文件", total));

    for (i, f) in index.files.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) { return Err("已取消".into()); }
        if f.path.contains("..") || f.path.starts_with('/') { log(format!("[警告] 跳过可疑路径: {}", f.path)); continue; }
        let Some(dl_url) = f.downloads.first() else { continue; };
        let dest = instance_dir.join(&f.path);
        if !dest.exists() {
            if let Err(e) = download_to_file(tx, cancel, dl_url, &dest, f.hashes.sha1.as_deref()) {
                log(format!("[警告] 下载失败 {}: {e}", f.path));
            }
        }
        let _ = tx.send(WorkerMsg::Progress { done: i as u64 + 1, total, label: "modpack".into() });
    }

    write_stub_version_json(&instance_dir, &instance_name, &index.dependencies)?;
    log(format!("[完成] 整合包 {} 已成功安装到 {}", hit.title, instance_name));
    let _ = fs::remove_file(&mrpack_path);

    Ok(hit.title.clone())
}

#[derive(Deserialize)]
struct MrpackIndex {
    files: Vec<MrpackFile>,
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
struct MrpackFile {
    path: String,
    #[serde(default)]
    downloads: Vec<String>,
    hashes: MrpackHashes,
}

#[derive(Deserialize)]
struct MrpackHashes { sha1: Option<String> }

fn extract_override(archive: &mut ZipArchive<fs::File>, dir: &str, dest: &std::path::Path) -> Result<(), String> {
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("zip 索引失败: {e}"))?;
        let name = entry.name().to_string();
        if !name.starts_with(&format!("{dir}/")) { continue; }
        let rel = &name[dir.len() + 1..];
        if rel.is_empty() || rel.contains("..") { continue; }
        let out = dest.join(rel);
        if let Some(parent) = out.parent() { fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?; }
        let mut f = fs::File::create(&out).map_err(|e| format!("创建文件失败: {e}"))?;
        std::io::copy(&mut entry, &mut f).map_err(|e| format!("写入失败: {e}"))?;
    }
    Ok(())
}

fn detect_loader(deps: &HashMap<String, String>) -> Option<ModLoader> {
    if deps.contains_key("neoforge") { Some(ModLoader::NeoForge) }
    else if deps.contains_key("forge") { Some(ModLoader::Forge) }
    else if deps.contains_key("fabric-loader") { Some(ModLoader::Fabric) }
    else if deps.contains_key("quilt-loader") { Some(ModLoader::Quilt) }
    else { None }
}

fn write_stub_version_json(instance_dir: &std::path::Path, name: &str, deps: &HashMap<String, String>) -> Result<(), String> {
    let mc_version = deps.get("minecraft").cloned().unwrap_or_default();
    let loader = detect_loader(deps);

    let json = serde_json::json!({
        "id": name,
        "inheritsFrom": mc_version,
        "type": "release",
        "libraries": [],
        "arguments": { "game": [], "jvm": [] },
        "mainClass": "",
        "_nmcl_loader": format!("{:?}", loader),
    });

    fs::write(instance_dir.join(format!("{}.json", name)), serde_json::to_string_pretty(&json).unwrap_or_default())
        .map_err(|e| format!("写入版本 JSON 失败: {e}"))?;
    Ok(())
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect::<String>().trim_matches('_').to_string()
}