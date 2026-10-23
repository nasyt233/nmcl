use crate::app::VersionEntry;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteCategory {
    Release,
    Snapshot,
    AprilFools,
}

fn is_april_fools(id: &str) -> bool {
    matches!(
        id,
        "15w14a"
            | "1.RV-Pre1"
            | "3D Shareware v1.34"
            | "20w14infinite"
            | "22w13oneBlockAtATime"
            | "23w13a_or_b"
            | "24w14potato"
            | "25w14craftmine"
    )
}

pub fn classify(id: &str, mtype: &str) -> RemoteCategory {
    if is_april_fools(id) {
        RemoteCategory::AprilFools
    } else if mtype == "release" {
        RemoteCategory::Release
    } else {
        RemoteCategory::Snapshot
    }
}

#[derive(Debug, Clone)]
pub struct RemoteVersion {
    pub id: String,
    #[allow(dead_code)]
    pub version_type: String,
    pub url: String,
    pub category: RemoteCategory,
}

pub fn default_minecraft_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join(".minecraft"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|p| PathBuf::from(p).join("Library/Application Support/minecraft"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".minecraft"))
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        None
    }
}

pub fn scan_versions(mc_dir: &Path) -> Vec<VersionEntry> {
    let versions_dir = mc_dir.join("versions");
    let mut entries = Vec::new();
    let Ok(read) = std::fs::read_dir(&versions_dir) else {
        return entries;
    };

    for e in read.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        let json_path = path.join(format!("{name}.json"));
        let jar_path = path.join(format!("{name}.jar"));

        let (loader, inherits) = if json_path.exists() {
            match std::fs::read_to_string(&json_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            {
                Some(v) => {
                    let inherits = v.get("inheritsFrom").and_then(|t| t.as_str()).is_some();
                    if inherits {
                        let lower = name.to_lowercase();
                        let ldr = if lower.contains("neoforge") {
                            "NeoForge"
                        } else if lower.contains("forge") {
                            "Forge"
                        } else if lower.contains("fabric") {
                            "Fabric"
                        } else if lower.contains("quilt") {
                            "Quilt"
                        } else {
                            "继承"
                        };
                        (ldr.to_string(), true)
                    } else {
                        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("release");
                        let ldr = match ty {
                            "release" => "Vanilla",
                            "snapshot" => "Snapshot",
                            "old_beta" => "Beta",
                            "old_alpha" => "Alpha",
                            other => other,
                        };
                        (ldr.to_string(), false)
                    }
                }
                None => ("未知".into(), false),
            }
        } else {
            ("无 JSON".into(), false)
        };

        entries.push(VersionEntry {
            name,
            loader,
            installed: jar_path.exists(),
            inherits,
        });
    }
    entries.sort_by(|a, b| b.name.cmp(&a.name));
    entries
}

// ---------- 共享 JSON 结构 ----------

#[derive(Deserialize, Debug)]
pub struct VersionManifest {
    pub versions: Vec<VersionInfo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VersionInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct VersionJson {
    #[allow(dead_code)]
    pub id: String,
    #[serde(rename = "mainClass")]
    pub main_class: Option<String>,
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "assetIndex")]
    pub asset_index: Option<AssetIndex>,
    #[serde(rename = "type")]
    pub version_type: Option<String>,
    pub downloads: Option<Downloads>,
    pub assets: Option<String>,
    pub jar: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Downloads {
    pub client: Option<Artifact>,
}

#[derive(Deserialize, Debug)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibDownloads>,
    pub natives: Option<std::collections::HashMap<String, String>>,
    pub rules: Option<Vec<Rule>>,
}

#[derive(Deserialize, Debug)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<std::collections::HashMap<String, Artifact>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Artifact {
    pub path: Option<String>,
    pub url: Option<String>,
    pub sha1: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<std::collections::HashMap<String, bool>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Arguments {
    pub game: Option<Vec<serde_json::Value>>,
    pub jvm: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize, Debug)]
pub struct AssetIndex {
    pub id: Option<String>,
    pub url: Option<String>,
}

pub fn os_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        return "windows";
    }
    #[cfg(target_os = "macos")]
    {
        return "osx";
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        return "linux";
    }
}

pub fn rules_allow(rules: &[Rule]) -> bool {
    let mut allowed = false;
    for r in rules {
        if let Some(feats) = &r.features {
            if feats.values().any(|v| *v) {
                continue;
            }
        }
        if let Some(os) = &r.os {
            if let Some(name) = &os.name {
                if name != os_name() {
                    continue;
                }
            }
            if let Some(arch) = &os.arch {
                let my_arch = if cfg!(target_arch = "x86_64") {
                    "x86_64"
                } else if cfg!(target_arch = "x86") {
                    "x86"
                } else if cfg!(target_arch = "aarch64") {
                    "arm64"
                } else {
                    "x86_64"
                };
                if arch != my_arch {
                    continue;
                }
            }
        }
        match r.action.as_str() {
            "allow" => allowed = true,
            "disallow" => allowed = false,
            _ => {}
        }
    }
    allowed
}

pub fn extract_args(raw: &[serde_json::Value]) -> Vec<String> {
    let mut out = Vec::new();
    for item in raw {
        match item {
            serde_json::Value::String(s) => out.push(s.clone()),
            serde_json::Value::Object(map) => {
                let allowed = if let Some(rules) = map.get("rules") {
                    let rules: Vec<Rule> =
                        serde_json::from_value(rules.clone()).unwrap_or_default();
                    rules_allow(&rules)
                } else {
                    true
                };
                if !allowed {
                    continue;
                }
                if let Some(v) = map.get("value") {
                    match v {
                        serde_json::Value::String(s) => out.push(s.clone()),
                        serde_json::Value::Array(arr) => {
                            for a in arr {
                                if let Some(s) = a.as_str() {
                                    out.push(s.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    out
}

pub fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    // 直接请求原 URL，不再强行替换为 BMCLAPI，避免你网络环境下 DNS 解析失败的问题
    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("请求失败 {url}: {e}"))?;
    
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("读取失败 {url}: {e}"))?;
        
    // 简单校验是否下到了 HTML 网页
    let preview = String::from_utf8_lossy(&buf[..buf.len().min(100)]).trim().to_lowercase();
    if preview.starts_with("<!doctype") || preview.starts_with("<html") {
        return Err(format!("下载失败，服务器返回了网页内容。URL: {}", url));
    }
    
    Ok(buf)
}

pub fn download_to_file(
    tx: &std::sync::mpsc::Sender<crate::app::WorkerMsg>,
    cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    url: &str,
    dest: &Path,
    expect_sha1: Option<&str>,
) -> Result<(), String> {
    use sha1::{Digest, Sha1};
    use std::io::Write;
    use std::sync::atomic::Ordering;

    if cancel.load(Ordering::Relaxed) {
        return Err("已取消".into());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let bytes = download_bytes(url)?;

    if let Some(expected) = expect_sha1 {
        let mut hasher = Sha1::new();
        hasher.update(&bytes);
        let actual = format!("{:x}", hasher.finalize());
        if actual != expected {
            let _ = tx.send(crate::app::WorkerMsg::Log(format!(
                "[警告] SHA1 不匹配 {}: 预期 {} 实际 {}",
                dest.display(),
                expected,
                actual
            )));
            return Err(format!("SHA1 校验失败: {}", dest.display()));
        }
    }

    let mut f =
        std::fs::File::create(dest).map_err(|e| format!("创建文件失败 {}: {e}", dest.display()))?;
    f.write_all(&bytes).map_err(|e| format!("写入失败: {e}"))?;
    Ok(())
}

pub fn load_and_merge_json(mc_dir: &Path, version_id: &str) -> Result<VersionJson, String> {
    load_and_merge_json_inner(mc_dir, version_id, &mut Vec::new())
}

fn load_and_merge_json_inner(
    mc_dir: &Path,
    version_id: &str,
    visited: &mut Vec<String>,
) -> Result<VersionJson, String> {
    // 1. 检测循环继承
    if visited.contains(&version_id.to_string()) {
        return Err(format!(
            "检测到版本继承死循环: {} -> {}",
            visited.join(" -> "),
            version_id
        ));
    }
    if visited.len() > 10 {
        return Err("版本继承层级过深（超过 10 层），请检查 JSON".into());
    }
    
    visited.push(version_id.to_string());

    let version_dir = mc_dir.join("versions").join(version_id);
    let json_path = version_dir.join(format!("{}.json", version_id));
    
    let json_str = std::fs::read_to_string(&json_path)
        .map_err(|e| format!("读取 version.json 失败 {}: {e}", json_path.display()))?;
    let mut vj: VersionJson = serde_json::from_str(&json_str)
        .map_err(|e| format!("解析 version.json 失败 {}: {e}", json_path.display()))?;

    // 2. 如果存在父版本，递归合并
    if let Some(parent_id) = vj.inherits_from.clone() {
        let mut parent_vj = load_and_merge_json_inner(mc_dir, &parent_id, visited)?;
        
        if vj.main_class.is_none() {
            vj.main_class = parent_vj.main_class;
        }
        if vj.asset_index.is_none() {
            vj.asset_index = parent_vj.asset_index;
        }
        if vj.assets.is_none() {
            vj.assets = parent_vj.assets;
        }
        
        // 合并 libraries
        parent_vj.libraries.extend(vj.libraries);
        vj.libraries = parent_vj.libraries;
        
        // 合并 arguments
        match (&mut vj.arguments, parent_vj.arguments) {
            (Some(args), Some(p_args)) => {
                if let Some(p_game) = p_args.game {
                    let mut game = args.game.take().unwrap_or_default();
                    game.extend(p_game);
                    args.game = Some(game);
                }
                if let Some(p_jvm) = p_args.jvm {
                    let mut jvm = args.jvm.take().unwrap_or_default();
                    jvm.extend(p_jvm);
                    args.jvm = Some(jvm);
                }
            }
            (None, Some(p_args)) => vj.arguments = Some(p_args),
            _ => {}
        }

        // 合并 minecraftArguments
        match (&mut vj.minecraft_arguments, parent_vj.minecraft_arguments) {
            (Some(s), Some(p_s)) => {
                *s = format!("{} {}", p_s, s);
            }
            (None, Some(p_s)) => vj.minecraft_arguments = Some(p_s),
            _ => {}
        }
    }

    // 回溯，移除当前节点
    visited.pop();

    Ok(vj)
}