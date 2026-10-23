use crate::app::VersionSettings;
use crate::worker::meta::{
    extract_args, os_name, rules_allow,
};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
        Arc,
    },
    thread,
    time::Duration,
};
use zip::ZipArchive;

pub fn start_launch(
    tx: Sender<crate::app::WorkerMsg>,
    cancel: Arc<AtomicBool>,
    mc_dir: PathBuf,
    version_id: String,
    player_name: String,
    java_path: String,
    version_settings: VersionSettings,
) {
    thread::spawn(move || {
        let res = do_launch(
            &tx,
            &cancel,
            &mc_dir,
            &version_id,
            &player_name,
            &java_path,
            &version_settings,
        );
        let _ = tx.send(crate::app::WorkerMsg::Finished(res));
    });
}

fn do_launch(
    tx: &Sender<crate::app::WorkerMsg>,
    cancel: &Arc<AtomicBool>,
    mc_dir: &Path,
    version_id: &str,
    player_name: &str,
    java_path: &str,
    vs: &VersionSettings,
) -> Result<(), String> {
    let log = |s: String| {
        let _ = tx.send(crate::app::WorkerMsg::Log(s));
    };

    let version_dir = mc_dir.join("versions").join(version_id);

    let vj = crate::worker::meta::load_and_merge_json(mc_dir, version_id)?;

    let jar_path = if let Some(jar_id) = &vj.jar {
        mc_dir.join("versions").join(jar_id).join(format!("{}.jar", jar_id))
    } else {
        let own_jar = version_dir.join(format!("{}.jar", version_id));
        if own_jar.exists() {
            own_jar
        } else if let Some(parent_id) = &vj.inherits_from {
            mc_dir.join("versions").join(parent_id).join(format!("{}.jar", parent_id))
        } else {
            own_jar
        }
    };

    if !jar_path.exists() {
        return Err(format!("找不到客户端 jar: {}", jar_path.display()));
    }

    let asset_index = vj
        .asset_index
        .as_ref()
        .and_then(|a| a.id.clone())
        .unwrap_or_else(|| {
            match version_id {
                "1.8" | "1.8.1" | "1.8.2" | "1.8.3" | "1.8.4" | "1.8.5" | "1.8.6" | "1.8.7" | "1.8.8" | "1.8.9" => "1.8".to_string(),
                "1.7.10" => "1.7.10".to_string(),
                "1.7.2" => "1.7.2".to_string(),
                _ => "legacy".to_string(),
            }
        });

    let asset_index_path = mc_dir.join("assets").join("indexes").join(format!("{}.json", asset_index));

    if !asset_index_path.exists() {
        log(format!("[警告] 资源索引 {} 缺失，正在尝试下载...", asset_index));
        if let Some(ai) = &vj.asset_index {
            if let Some(url) = &ai.url {
                if let Ok(bytes) = crate::worker::meta::download_bytes(url) {
                    if let Some(parent) = asset_index_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(&asset_index_path, &bytes);
                    log(format!("[信息] 资源索引 {} 下载完成", asset_index));
                }
            }
        }
    }

    let libraries_dir = mc_dir.join("libraries");
    let mut classpath: Vec<PathBuf> = Vec::new();
    let mut natives_libs: Vec<(PathBuf, String)> = Vec::new();
    let os = os_name();

    for lib in &vj.libraries {
        if let Some(rules) = &lib.rules {
            if !rules_allow(rules) {
                continue;
            }
        }
        if let Some(natives) = &lib.natives {
            if let Some(key) = natives.get(os) {
                let key = key.replace(
                    "${arch}",
                    if cfg!(target_arch = "x86_64") { "64" } else { "32" },
                );
                if let Some(dl) = &lib.downloads {
                    if let Some(cls) = &dl.classifiers {
                        if let Some(art) = cls.get(&key) {
                            if let Some(p) = &art.path {
                                natives_libs.push((libraries_dir.join(p), lib.name.clone()));
                            }
                        }
                    }
                }
            }
            continue;
        }
        if let Some(dl) = &lib.downloads {
            if let Some(art) = &dl.artifact {
                if let Some(p) = &art.path {
                    classpath.push(libraries_dir.join(p));
                }
            }
        }
    }
    classpath.push(jar_path.clone());

    let cp_sep = if cfg!(target_os = "windows") { ";" } else { ":" };
    let classpath_str = classpath
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(cp_sep);

    let local_arm64_dir = version_dir.join("natives-linux-arm64");
    let default_natives_dir = version_dir.join("natives");

    let natives_dir = if local_arm64_dir.exists() && local_arm64_dir.is_dir() {
        log(format!("[信息] 使用本地 ARM64 Natives 目录: {:?}", local_arm64_dir));
        local_arm64_dir
    } else {
        if default_natives_dir.exists() {
            let _ = fs::remove_dir_all(&default_natives_dir);
        }
        fs::create_dir_all(&default_natives_dir).map_err(|e| format!("创建 natives 失败: {e}"))?;

        if crate::worker::natives::needs_arm64_natives() {
            log("[信息] 检测到 ARM64 架构，将自动下载兼容的 Natives 库".into());
            let (ntx, nrx) = mpsc::channel();

            crate::worker::natives::start_download_arm64_natives(
                ntx,
                cancel.clone(),
                default_natives_dir.clone(),
                version_id.to_string(),
            );

            let mut download_ok = false;
            loop {
                match nrx.recv_timeout(Duration::from_millis(100)) {
                    Ok(crate::app::WorkerMsg::Log(s)) => log(s),
                    Ok(crate::app::WorkerMsg::Finished(Ok(()))) => {
                        download_ok = true;
                        break;
                    }
                    Ok(crate::app::WorkerMsg::Finished(Err(e))) => {
                        log(format!("[警告] ARM64 Natives 下载失败: {e}"));
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if cancel.load(Ordering::Relaxed) {
                            return Err("已取消".into());
                        }
                    }
                    _ => {}
                }
            }

            if !download_ok {
                for (zip_path, _name) in &natives_libs {
                    if zip_path.exists() {
                        let _ = extract_natives(zip_path, &default_natives_dir);
                    }
                }
            }
        } else {
            for (zip_path, name) in &natives_libs {
                if zip_path.exists() {
                    if let Err(e) = extract_natives(zip_path, &default_natives_dir) {
                        log(format!("[警告] 解压 natives 失败 {}: {e}", name));
                    }
                }
            }
        }
        default_natives_dir
    };

    let natives_abs = natives_dir
        .canonicalize()
        .unwrap_or_else(|_| natives_dir.clone())
        .to_string_lossy()
        .to_string();

    let mut jvm_args: Vec<String> = Vec::new();
    let mut game_args: Vec<String> = Vec::new();

    if let Some(args) = &vj.arguments {
        if let Some(raw) = &args.jvm {
            jvm_args = extract_args(raw);
        }
        if let Some(raw) = &args.game {
            game_args = extract_args(raw);
        }
    } else if let Some(legacy) = &vj.minecraft_arguments {
        game_args = legacy.split_whitespace().map(String::from).collect();
    }

    jvm_args.retain(|a| {
        !a.starts_with("-Djava.library.path") && !a.starts_with("-Dorg.lwjgl.librarypath")
    });

    jvm_args.push(format!("-Djava.library.path={}", natives_abs));
    jvm_args.push(format!("-Dorg.lwjgl.librarypath={}", natives_abs));
    jvm_args.push("-Dfile.encoding=UTF-8".into());
    jvm_args.push("-Dstdout.encoding=UTF-8".into());
    jvm_args.push("-Dstderr.encoding=UTF-8".into());

    jvm_args.push("-Djava.rmi.server.useCodebaseOnly=true".into());
    jvm_args.push("-Dcom.sun.jndi.rmi.object.trustURLCodebase=false".into());
    jvm_args.push("-Dcom.sun.jndi.cosnaming.object.trustURLCodebase=false".into());
    jvm_args.push("-Dlog4j2.formatMsgNoLookups=true".into());

    let log4j_path = version_dir.join("log4j2.xml");
    if log4j_path.exists() {
        jvm_args.push(format!("-Dlog4j.configurationFile={}", log4j_path.display()));
    }

    jvm_args.push(format!("-Dminecraft.client.jar={}", jar_path.display()));
    jvm_args.push(format!("-Duser.home={}", std::env::var("HOME").unwrap_or_else(|_| "/home/kali".into())));
    jvm_args.push("-Djava.net.useSystemProxies=true".into());
    jvm_args.push("-Dminecraft.launcher.brand=HMCL".into());
    jvm_args.push("-Dminecraft.launcher.version=3.16.4".into());

    jvm_args.push("-XX:+UnlockExperimentalVMOptions".into());
    jvm_args.push("-XX:+UnlockDiagnosticVMOptions".into());
    jvm_args.push("-XX:+UseG1GC".into());
    jvm_args.push("-XX:G1MixedGCCountTarget=5".into());
    jvm_args.push("-XX:G1NewSizePercent=20".into());
    jvm_args.push("-XX:G1ReservePercent=20".into());
    jvm_args.push("-XX:MaxGCPauseMillis=50".into());
    jvm_args.push("-XX:G1HeapRegionSize=32m".into());
    jvm_args.push("-XX:-OmitStackTraceInFastThrow".into());
    jvm_args.push("-XX:-DontCompileHugeMethods".into());
    jvm_args.push("-XX:MaxNodeLimit=240000".into());
    jvm_args.push("-XX:NodeLimitFudgeFactor=8000".into());
    jvm_args.push("-XX:TieredCompileTaskTimeout=10000".into());
    jvm_args.push("-XX:ReservedCodeCacheSize=400M".into());
    jvm_args.push("-XX:NonNMethodCodeHeapSize=12M".into());
    jvm_args.push("-XX:ProfiledCodeHeapSize=194M".into());
    jvm_args.push("-XX:NmethodSweepActivity=1".into());
    jvm_args.push("-Dfml.ignoreInvalidMinecraftCertificates=true".into());
    jvm_args.push("-Dfml.ignorePatchDiscrepancies=true".into());

    let mb = vs.max_memory_mb.unwrap_or(2048);
    jvm_args.push(format!("-Xmx{}M", mb));
    jvm_args.push(format!("-Xms{}M", (mb / 2).max(512)));

    let has_cp = jvm_args.iter().any(|a| a == "${classpath}" || a == "-cp" || a == "-classpath");
    if !has_cp {
        jvm_args.push("-cp".into());
        jvm_args.push("${classpath}".into());
    }

    for a in vs.jvm_args.split_whitespace() {
        jvm_args.push(a.to_string());
    }
    for a in vs.game_args.split_whitespace() {
        game_args.push(a.to_string());
    }

    let mut final_game_args = Vec::new();
    let mut skip_next = false;
    for arg in &game_args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--assetIndex" || arg == "--assetsDir" || arg == "--gameDir" || arg == "--version" || arg == "--userProperties" {
            skip_next = true;
            continue;
        }
        final_game_args.push(arg.clone());
    }

    final_game_args.push("--version".into());
    final_game_args.push(version_id.to_string());
    final_game_args.push("--gameDir".into());
    final_game_args.push(version_dir.to_string_lossy().to_string());
    final_game_args.push("--assetsDir".into());
    final_game_args.push(mc_dir.join("assets").to_string_lossy().to_string());
    final_game_args.push("--assetIndex".into());
    final_game_args.push(asset_index.clone());
    final_game_args.push("--userProperties".into());
    final_game_args.push("{}".into());

    let mut ctx: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    ctx.insert("classpath", classpath_str);
    ctx.insert("classpath_separator", cp_sep.into());
    ctx.insert("natives_directory", natives_abs.clone());
    ctx.insert("launcher_name", "HMCL".into());
    ctx.insert("launcher_version", "3.16.4".into());
    ctx.insert("library_directory", libraries_dir.to_string_lossy().to_string());
    ctx.insert("version_name", version_id.into());
    ctx.insert("auth_player_name", player_name.into());
    ctx.insert("auth_uuid", "00000000000000000000000000000000".into());
    ctx.insert("auth_access_token", "0".into());
    ctx.insert("user_type", "legacy".into());
    ctx.insert("version_type", vj.version_type.clone().unwrap_or_else(|| "release".into()));
    ctx.insert("game_directory", version_dir.to_string_lossy().to_string());
    ctx.insert("assets_root", mc_dir.join("assets").to_string_lossy().to_string());
    ctx.insert("assets_index_name", asset_index.clone());
    ctx.insert("resolution_width", "854".into());
    ctx.insert("resolution_height", "480".into());
    
    // 【关键修复】补充 user_properties 占位符，避免游戏解析失败
    ctx.insert("user_properties", "{}".into());

    let replace = |s: &str| -> String {
        let mut out = s.to_string();
        for (k, v) in &ctx {
            out = out.replace(&format!("${{{k}}}"), v);
        }
        out
    };

    let jvm_args: Vec<String> = jvm_args.iter().map(|a| replace(a)).collect();
    let game_args: Vec<String> = final_game_args.iter().map(|a| replace(a)).collect();

    let main_class = vj.main_class.clone().ok_or("缺少 mainClass")?;
    let java = if java_path.trim().is_empty() {
        std::env::var("NMCL_JAVA").unwrap_or_else(|_| "java".into())
    } else {
        java_path.to_string()
    };

    log(format!("启动: {} {}", java, main_class));

    let mut cmd = Command::new(&java);
    cmd.args(&jvm_args).arg(&main_class).args(&game_args);
    cmd.current_dir(&version_dir);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("启动 Java 失败: {e}"))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let tx_o = tx.clone();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx_o.send(crate::app::WorkerMsg::Log(format!("[MC]  {line}")));
        }
    });
    let tx_e = tx.clone();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = tx_e.send(crate::app::WorkerMsg::Log(format!("[MC!] {line}")));
        }
    });

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("已取消".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                return Err(format!("游戏异常退出，退出码: {:?}", status.code()));
            }
            Ok(None) => thread::sleep(Duration::from_millis(300)),
            Err(e) => return Err(format!("等待进程失败: {e}")),
        }
    }
}

fn extract_natives(zip_path: &Path, dest: &Path) -> std::io::Result<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let name = entry.name().to_string();
        if name.ends_with('/') || name.contains("..") || name.starts_with("META-INF") {
            continue;
        }
        let out = dest.join(&name);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut f = fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut f)?;
    }
    Ok(())
}