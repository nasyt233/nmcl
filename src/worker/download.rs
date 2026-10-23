use crate::app::WorkerMsg;
use crate::worker::meta::{
    classify, download_bytes, download_to_file, rules_allow, RemoteVersion, VersionJson,
    VersionManifest,
};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::Sender,
        Arc, Mutex,
    },
    thread,
};

// ============================================================
// 获取远程版本清单
// ============================================================

pub fn start_fetch_manifest(tx: Sender<WorkerMsg>, cancel: Arc<AtomicBool>) {
    thread::spawn(move || {
        let log = |s: String| {
            let _ = tx.send(WorkerMsg::Log(s));
        };
        log("获取远程版本清单...".into());

        let res = (|| -> Result<Vec<RemoteVersion>, String> {
            let url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
            let bytes = download_bytes(url)?;
            if cancel.load(Ordering::Relaxed) {
                return Err("已取消".into());
            }
            let manifest: VersionManifest =
                serde_json::from_slice(&bytes).map_err(|e| format!("解析 manifest 失败: {e}"))?;
            Ok(manifest
                .versions
                .into_iter()
                .map(|v| {
                    let category = classify(&v.id, &v.version_type);
                    RemoteVersion {
                        id: v.id,
                        version_type: v.version_type,
                        url: v.url,
                        category,
                    }
                })
                .collect())
        })();

        match res {
            Ok(v) => {
                log(format!("远程版本加载完成: {} 个", v.len()));
                let _ = tx.send(WorkerMsg::RemoteVersionsLoaded(v));
            }
            Err(e) => {
                log(format!("获取远程版本失败: {e}"));
                let _ = tx.send(WorkerMsg::RemoteVersionsLoaded(Vec::new()));
            }
        }
    });
}

// ============================================================
// 下载原版版本（客户端 Jar + 依赖库 + 资源文件）
// ============================================================

pub fn start_download_version(
    tx: Sender<WorkerMsg>,
    cancel: Arc<AtomicBool>,
    mc_dir: PathBuf,
    version: RemoteVersion,
    install_name: String,
    loader: Option<crate::worker::ModLoader>,
    loader_version: Option<String>,
    memory_mb: Option<u32>,
) {
    thread::spawn(move || {
        let res = do_download_version(
            &tx,
            &cancel,
            &mc_dir,
            &version,
            &install_name,
            loader,
            loader_version.as_deref(),
            memory_mb,
        );
        if res.is_ok() {
            let _ = tx.send(WorkerMsg::DownloadComplete(install_name));
        }
        let _ = tx.send(WorkerMsg::Finished(res));
    });
}

fn do_download_version(
    tx: &Sender<WorkerMsg>,
    cancel: &Arc<AtomicBool>,
    mc_dir: &Path,
    version: &RemoteVersion,
    install_name: &str,
    loader: Option<crate::worker::ModLoader>,
    loader_version: Option<&str>,
    memory_mb: Option<u32>,
) -> Result<(), String> {
    let log = |s: String| {
        let _ = tx.send(WorkerMsg::Log(s));
    };

    // 1. 获取版本详情 JSON
    log(format!("开始下载 {}（安装名: {}）", version.id, install_name));
    let details_bytes = download_bytes(&version.url)?;
    let details: VersionJson =
        serde_json::from_slice(&details_bytes).map_err(|e| format!("解析版本详情失败: {e}"))?;

    let base_dir = mc_dir.join("versions").join(&version.id);
    fs::create_dir_all(&base_dir).map_err(|e| format!("创建目录失败: {e}"))?;
    fs::write(base_dir.join(format!("{}.json", version.id)), &details_bytes)
        .map_err(|e| format!("写入 json 失败: {e}"))?;

    // 2. 下载客户端 JAR
    if let Some(dl) = &details.downloads {
        if let Some(client) = &dl.client {
            if let Some(url) = &client.url {
                let jar = base_dir.join(format!("{}.jar", version.id));
                if !jar.exists() {
                    log("下载客户端 JAR...".into());
                    download_to_file(tx, cancel, url, &jar, client.sha1.as_deref())?;
                }
            }
        }
    }

    // 3. 下载依赖库 (libraries)
    let libraries_dir = mc_dir.join("libraries");
    let total_libs = details.libraries.len() as u64;
    let mut done = 0u64;
    let _ = tx.send(WorkerMsg::Progress {
        done: 0,
        total: total_libs,
        label: "libraries".into(),
    });

    for lib in &details.libraries {
        if cancel.load(Ordering::Relaxed) {
            return Err("已取消".into());
        }
        if let Some(rules) = &lib.rules {
            if !rules_allow(rules) {
                continue;
            }
        }
        if let Some(dl) = &lib.downloads {
            // 普通库
            if let Some(art) = &dl.artifact {
                if let (Some(path), Some(url)) = (&art.path, &art.url) {
                    let dest = libraries_dir.join(path);
                    if !dest.exists() {
                        if let Err(e) =
                            download_to_file(tx, cancel, url, &dest, art.sha1.as_deref())
                        {
                            log(format!("[警告] 库下载失败 {}: {e}", lib.name));
                        }
                    }
                }
            }
            // 原生库 (Natives)
            if let Some(natives) = &lib.natives {
                let os = crate::worker::meta::os_name();
                if let Some(key) = natives.get(os) {
                    let key = key.replace(
                        "${arch}",
                        if cfg!(target_arch = "x86_64") { "64" } else { "32" },
                    );
                    if let Some(classifiers) = &dl.classifiers {
                        if let Some(art) = classifiers.get(&key) {
                            if let (Some(path), Some(url)) = (&art.path, &art.url) {
                                let dest = libraries_dir.join(path);
                                if !dest.exists() {
                                    if let Err(e) =
                                        download_to_file(tx, cancel, url, &dest, art.sha1.as_deref())
                                    {
                                        log(format!("[警告] natives 下载失败 {}: {e}", lib.name));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        done += 1;
        if done % 5 == 0 || done == total_libs {
            let _ = tx.send(WorkerMsg::Progress {
                done,
                total: total_libs,
                label: "libraries".into(),
            });
        }
    }

    // 4. 下载资源文件 (assets)
    if let Some(ai) = &details.asset_index {
        if let Some(url) = &ai.url {
            let id = ai.id.as_deref().unwrap_or("legacy");
            download_assets(tx, cancel, mc_dir, id, url)?;
        }
    }

    // 5. 处理加载器 (Forge/Fabric 等)
    if let Some(ld) = loader {
        if ld != crate::worker::ModLoader::Vanilla {
            log(format!("准备安装加载器 {:?}", ld));
            install_loader_stub(mc_dir, install_name, &version.id, ld, loader_version)?;
        }
    } else {
        // 纯原版：如果使用了自定义安装名，复制一份 JSON 和 JAR
        if install_name != version.id {
            let dst_dir = mc_dir.join("versions").join(install_name);
            fs::create_dir_all(&dst_dir).map_err(|e| format!("创建目录失败: {e}"))?;
            fs::copy(
                base_dir.join(format!("{}.json", version.id)),
                dst_dir.join(format!("{}.json", install_name)),
            )
            .map_err(|e| format!("复制 json 失败: {e}"))?;
            if fs::copy(
                base_dir.join(format!("{}.jar", version.id)),
                dst_dir.join(format!("{}.jar", install_name)),
            )
            .is_ok()
            {
                log("已复制客户端 JAR".into());
            }
        }
    }

    // 6. 保存内存设置
    if let Some(mb) = memory_mb {
        let mut s = crate::app::LauncherSettings::load(mc_dir);
        let mut vs = s.get_version(install_name);
        vs.max_memory_mb = Some(mb);
        s.versions.insert(install_name.to_string(), vs);
        let _ = s.save(mc_dir);
    }

    log("版本下载流程完成".into());
    Ok(())
}

// ============================================================
// 辅助：安装加载器占位
// ============================================================

fn install_loader_stub(
    mc_dir: &Path,
    install_name: &str,
    base_version: &str,
    loader: crate::worker::ModLoader,
    loader_version: Option<&str>,
) -> Result<(), String> {
    let dst = mc_dir.join("versions").join(install_name);
    fs::create_dir_all(&dst).map_err(|e| format!("创建目录失败: {e}"))?;

    let loader_str = match loader {
        crate::worker::ModLoader::Forge => "forge",
        crate::worker::ModLoader::NeoForge => "neoforge",
        crate::worker::ModLoader::Fabric => "fabric",
        crate::worker::ModLoader::Quilt => "quilt",
        crate::worker::ModLoader::Vanilla => "vanilla",
    };

    let json = serde_json::json!({
        "id": install_name,
        "inheritsFrom": base_version,
        "type": "release",
        "libraries": [],
        "arguments": { "game": [], "jvm": [] },
        "mainClass": "",
        "_nmcl_loader": loader_str,
        "_nmcl_loader_version": loader_version.unwrap_or("latest"),
    });

    fs::write(
        dst.join(format!("{}.json", install_name)),
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    )
    .map_err(|e| format!("写入加载器 JSON 失败: {e}"))?;

    Ok(())
}

// ============================================================
// 下载资源文件（带损坏校验和镜像加速）
// ============================================================

fn download_assets(
    tx: &Sender<WorkerMsg>,
    cancel: &Arc<AtomicBool>,
    mc_dir: &Path,
    index_id: &str,
    index_url: &str,
) -> Result<(), String> {
    let log = |s: String| {
        let _ = tx.send(WorkerMsg::Log(s));
    };
    let objects_dir = mc_dir.join("assets").join("objects");
    let indexes_dir = mc_dir.join("assets").join("indexes");
    fs::create_dir_all(&objects_dir).map_err(|e| format!("创建 assets/objects 失败: {e}"))?;
    fs::create_dir_all(&indexes_dir).map_err(|e| format!("创建 assets/indexes 失败: {e}"))?;

    let index_path = indexes_dir.join(format!("{index_id}.json"));

    // 校验本地文件是否损坏
    let index_bytes = if index_path.exists() {
        let bytes = fs::read(&index_path).map_err(|e| format!("读取索引失败: {e}"))?;
        // 严格校验本地的整个文件是否是合法 JSON
        if serde_json::from_slice::<serde_json::Value>(&bytes).is_ok() {
            bytes
        } else {
            log(format!("[警告] 本地资源索引 {} 已损坏，正在重新下载...", index_id));
            let _ = fs::remove_file(&index_path);
            let b = download_bytes(index_url)?;
            // 严格校验下载的整个文件是否是合法 JSON
            if serde_json::from_slice::<serde_json::Value>(&b).is_err() {
                return Err(format!("资源索引 {} 下载内容不是合法的 JSON，请检查你的网络环境是否劫持了请求", index_id));
            }
            fs::write(&index_path, &b).map_err(|e| format!("写入索引失败: {e}"))?;
            b
        }
    } else {
        log(format!("下载资源索引 {index_id}"));
        let b = download_bytes(index_url)?;
        // 严格校验下载的整个文件是否是合法 JSON
        if serde_json::from_slice::<serde_json::Value>(&b).is_err() {
            return Err(format!("资源索引 {} 下载内容不是合法的 JSON，请检查你的网络环境是否劫持了请求", index_id));
        }
        fs::write(&index_path, &b).map_err(|e| format!("写入索引失败: {e}"))?;
        b
    };

    // 本地定义结构体，避免外部依赖遗漏
    #[derive(serde::Deserialize)]
    struct AssetIndex {
        objects: HashMap<String, AssetObj>,
    }
    #[derive(serde::Deserialize)]
    struct AssetObj {
        hash: String,
        #[allow(dead_code)]
        size: u64,
    }

    let index: AssetIndex =
        serde_json::from_slice(&index_bytes).map_err(|e| format!("解析资源索引失败: {e}"))?;

    let mut to_download: Vec<(String, String)> = Vec::new();
    for obj in index.objects.values() {
        if obj.hash.len() < 2 {
            continue;
        }
        let prefix = &obj.hash[0..2];
        let dest = objects_dir.join(prefix).join(&obj.hash);
        if !dest.exists() {
            to_download.push((prefix.to_string(), obj.hash.clone()));
        }
    }

    let total = to_download.len() as u64;
    if total == 0 {
        log("资源文件已就绪".into());
        return Ok(());
    }
    log(format!("资源文件: 需要下载 {} 个（8 并发）", total));

    let queue = Arc::new(Mutex::new(to_download));
    let done = Arc::new(AtomicU64::new(0));

    thread::scope(|scope| {
        for _ in 0..8 {
            let queue = queue.clone();
            let done = done.clone();
            let cancel = cancel.clone();
            let objects_dir = objects_dir.clone();
            let tx_c = tx.clone();

            scope.spawn(move || loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let item = { queue.lock().unwrap().pop() };
                let Some((prefix, hash)) = item else {
                    break;
                };
                let dest = objects_dir.join(&prefix).join(&hash);
                if !dest.exists() {
                    let url = format!("https://resources.download.minecraft.net/{prefix}/{hash}");
                    if let Err(e) = download_to_file(&tx_c, &cancel, &url, &dest, Some(&hash)) {
                        let _ = tx_c.send(WorkerMsg::Log(format!("[警告] {hash}: {e}")));
                    }
                }
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                if d % 20 == 0 || d == total {
                    let _ = tx_c.send(WorkerMsg::Progress {
                        done: d,
                        total,
                        label: "assets".into(),
                    });
                }
            });
        }
    });

    if cancel.load(Ordering::Relaxed) {
        return Err("已取消".into());
    }
    Ok(())
}

// ============================================================
// 获取加载器版本列表
// ============================================================

pub fn start_fetch_loader_versions(
    tx: Sender<WorkerMsg>,
    _cancel: Arc<AtomicBool>,
    loader: crate::worker::ModLoader,
    game_version: String,
) {
    thread::spawn(move || {
        let log = |s: String| {
            let _ = tx.send(WorkerMsg::Log(s));
        };
        let res = (|| -> Result<Vec<String>, String> {
            match loader {
                crate::worker::ModLoader::Fabric => {
                    let url = format!(
                        "https://meta.fabricmc.net/v2/versions/loader/{}",
                        game_version
                    );
                    let bytes = download_bytes(&url)?;
                    let arr: Vec<serde_json::Value> = serde_json::from_slice(&bytes)
                        .map_err(|e| format!("解析 Fabric 版本失败: {e}"))?;
                    Ok(arr
                        .into_iter()
                        .filter_map(|v| {
                            v.get("loader")
                                .and_then(|l| l.get("version"))
                                .and_then(|x| x.as_str())
                                .map(String::from)
                        })
                        .collect())
                }
                crate::worker::ModLoader::Quilt => {
                    let url = format!(
                        "https://meta.quiltmc.org/v3/versions/loader/{}",
                        game_version
                    );
                    let bytes = download_bytes(&url)?;
                    let arr: Vec<serde_json::Value> = serde_json::from_slice(&bytes)
                        .map_err(|e| format!("解析 Quilt 版本失败: {e}"))?;
                    Ok(arr
                        .into_iter()
                        .filter_map(|v| {
                            v.get("loader")
                                .and_then(|l| l.get("version"))
                                .and_then(|x| x.as_str())
                                .map(String::from)
                        })
                        .collect())
                }
                crate::worker::ModLoader::Forge => {
                    let url = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
                    let bytes = download_bytes(url)?;
                    let json: serde_json::Value = serde_json::from_slice(&bytes)
                        .map_err(|e| format!("解析 Forge promotions 失败: {e}"))?;
                    let promos = json.get("promos").and_then(|p| p.as_object());
                    let mut out = Vec::new();
                    if let Some(p) = promos {
                        for key in [
                            format!("{}-latest", game_version),
                            format!("{}-recommended", game_version),
                        ] {
                            if let Some(v) = p.get(&key).and_then(|x| x.as_str()) {
                                let full = format!("{}-{}", game_version, v);
                                if !out.contains(&full) {
                                    out.push(full);
                                }
                            }
                        }
                    }
                    if out.is_empty() {
                        return Err(format!("Forge 没有 MC {} 的可用版本", game_version));
                    }
                    Ok(out)
                }
                crate::worker::ModLoader::NeoForge => {
                    let url = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
                    let xml = String::from_utf8(download_bytes(url)?)
                        .map_err(|e| format!("读取 NeoForge metadata 失败: {e}"))?;
                    let mut out = Vec::new();
                    let mut start = 0;
                    while let Some(i) = xml[start..].find("<version>") {
                        let abs = start + i + "<version>".len();
                        if let Some(j) = xml[abs..].find("</version>") {
                            let ver = xml[abs..abs + j].to_string();
                            if ver.starts_with(&game_version.replace("1.", "")) {
                                out.push(ver);
                            }
                            start = abs + j;
                        } else {
                            break;
                        }
                    }
                    out.reverse();
                    if out.is_empty() {
                        return Err(format!("NeoForge 没有 MC {} 的可用版本", game_version));
                    }
                    Ok(out)
                }
                crate::worker::ModLoader::Vanilla => Ok(Vec::new()),
            }
        })();

        match res {
            Ok(versions) => {
                let _ = tx.send(WorkerMsg::LoaderVersionsLoaded { loader, versions });
            }
            Err(e) => {
                log(format!("获取加载器版本失败: {e}"));
                let _ = tx.send(WorkerMsg::LoaderVersionsLoaded {
                    loader,
                    versions: Vec::new(),
                });
            }
        }
    });
}