use crate::app::VersionSettings;
use crate::worker::meta::{extract_args, os_name, rules_allow};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::{AtomicBool, Ordering}, mpsc::{self, Sender}, Arc},
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
    vs: VersionSettings,
) {
    thread::spawn(move || {
        if let Err(e) = do_launch(&tx, &cancel, &mc_dir, &version_id, &player_name, &java_path, &vs) {
            let _ = tx.send(crate::app::WorkerMsg::Log(format!("[错误] 启动失败: {e}")));
        }
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
    let log = |s: String| { let _ = tx.send(crate::app::WorkerMsg::Log(s)); };
    let version_dir = mc_dir.join("versions").join(version_id);
    let vj = crate::worker::meta::load_and_merge_json(mc_dir, version_id)?;
    
    let jar_path = if let Some(jar_id) = &vj.jar {
        mc_dir.join("versions").join(jar_id).join(format!("{}.jar", jar_id))
    } else {
        let own_jar = version_dir.join(format!("{}.jar", version_id));
        if own_jar.exists() { own_jar } else if let Some(pid) = &vj.inherits_from {
            mc_dir.join("versions").join(pid).join(format!("{}.jar", pid))
        } else { own_jar }
    };

    if !jar_path.exists() { return Err(format!("找不到客户端 jar: {}", jar_path.display())); }

    let asset_index = vj.asset_index.as_ref().and_then(|a| a.id.clone()).unwrap_or_else(|| {
        if version_id.starts_with("1.8.") { "1.8".into() } else { "legacy".into() }
    });

    let lib_dir = mc_dir.join("libraries");
    let mut classpath: Vec<PathBuf> = Vec::new();
    let mut natives_libs = Vec::new();

    for lib in &vj.libraries {
        if let Some(rules) = &lib.rules { if !rules_allow(rules) { continue; } }
        if let Some(natives) = &lib.natives {
            if let Some(key) = natives.get(os_name()) {
                let key = key.replace("${arch}", if cfg!(target_arch = "x86_64") { "64" } else { "32" });
                if let Some(dl) = &lib.downloads {
                    if let Some(cls) = &dl.classifiers {
                        if let Some(art) = cls.get(&key) {
                            if let Some(p) = &art.path { natives_libs.push((lib_dir.join(p), lib.name.clone())); }
                        }
                    }
                }
            }
            continue;
        }
        if let Some(dl) = &lib.downloads {
            if let Some(art) = &dl.artifact {
                if let Some(p) = &art.path { classpath.push(lib_dir.join(p)); }
            }
        }
    }
    classpath.push(jar_path.clone());
    let cp_str = classpath.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join(if cfg!(windows) { ";" } else { ":" });

    let local_arm64 = version_dir.join("natives-linux-arm64");
    let default_natives = version_dir.join("natives");
    let natives_dir = if local_arm64.exists() && local_arm64.is_dir() {
        local_arm64
    } else {
        let mut has_natives = false;
        if let Ok(read) = fs::read_dir(&default_natives) {
            for e in read.flatten() {
                if let Some(ext) = e.path().extension() {
                    if ext == "so" || ext == "dll" || ext == "dylib" { has_natives = true; break; }
                }
            }
        }
        if !has_natives {
            let _ = fs::remove_dir_all(&default_natives);
            fs::create_dir_all(&default_natives).map_err(|e| format!("创建 natives 失败: {e}"))?;
            if crate::worker::natives::needs_arm64_natives() {
                let (ntx, nrx) = mpsc::channel();
                crate::worker::natives::start_download_arm64_natives(ntx, cancel.clone(), default_natives.clone(), version_id.to_string());
                let mut ok = false;
                loop {
                    match nrx.recv_timeout(Duration::from_millis(100)) {
                        Ok(crate::app::WorkerMsg::Finished(Ok(()))) => { ok = true; break; }
                        Ok(crate::app::WorkerMsg::Finished(Err(_))) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => { if cancel.load(Ordering::Relaxed) { return Err("已取消".into()); } }
                        _ => {}
                    }
                }
                if !ok { for (z, _) in &natives_libs { let _ = extract_natives(z, &default_natives); } }
            } else {
                for (z, name) in &natives_libs {
                    if z.exists() { if let Err(e) = extract_natives(z, &default_natives) { log(format!("[警告] 解压失败 {name}: {e}")); } }
                }
            }
        }
        default_natives
    };

    let natives_abs = natives_dir.canonicalize().unwrap_or(natives_dir).to_string_lossy().to_string();
    let mut jvm_args = Vec::new();
    let mut game_args = Vec::new();

    if let Some(args) = &vj.arguments {
        if let Some(r) = &args.jvm { jvm_args = extract_args(r); }
        if let Some(r) = &args.game { game_args = extract_args(r); }
    } else if let Some(l) = &vj.minecraft_arguments {
        game_args = l.split_whitespace().map(String::from).collect();
    }

    jvm_args.retain(|a| !a.starts_with("-Djava.library.path") && !a.starts_with("-Dorg.lwjgl.librarypath"));
    jvm_args.extend([
        format!("-Djava.library.path={}", natives_abs),
        format!("-Dorg.lwjgl.librarypath={}", natives_abs),
        "-Dfile.encoding=UTF-8".into(), "-Dstdout.encoding=UTF-8".into(), "-Dstderr.encoding=UTF-8".into(),
        "-Djava.rmi.server.useCodebaseOnly=true".into(), "-Dlog4j2.formatMsgNoLookups=true".into(),
        "-Dminecraft.launcher.brand=HMCL".into(), "-Dminecraft.launcher.version=3.16.4".into(),
        format!("-Dminecraft.client.jar={}", jar_path.display()),
        "-XX:+UnlockExperimentalVMOptions".into(), "-XX:+UseG1GC".into(), "-XX:-OmitStackTraceInFastThrow".into(),
    ]);
    if version_dir.join("log4j2.xml").exists() {
        jvm_args.push(format!("-Dlog4j.configurationFile={}", version_dir.join("log4j2.xml").display()));
    }
    
    let mb = vs.max_memory_mb.unwrap_or(2048);
    jvm_args.push(format!("-Xmx{}M", mb));
    jvm_args.push(format!("-Xms{}M", (mb / 2).max(512)));

    if !jvm_args.iter().any(|a| a == "${classpath}") {
        jvm_args.push("-cp".into()); jvm_args.push("${classpath}".into());
    }

    for a in vs.jvm_args.split_whitespace() { jvm_args.push(a.to_string()); }
    for a in vs.game_args.split_whitespace() { game_args.push(a.to_string()); }

    let mut final_game_args = Vec::new();
    let mut skip = false;
    for arg in &game_args {
        if skip { skip = false; continue; }
        if arg == "--assetIndex" || arg == "--assetsDir" || arg == "--gameDir" || arg == "--version" || arg == "--userProperties" { skip = true; continue; }
        final_game_args.push(arg.clone());
    }
    final_game_args.extend([
        "--version".into(), version_id.into(), "--gameDir".into(), version_dir.to_string_lossy().to_string(),
        "--assetsDir".into(), mc_dir.join("assets").to_string_lossy().to_string(),
        "--assetIndex".into(), asset_index.clone(), "--userProperties".into(), "{}".into()
    ]);

    let mut ctx = std::collections::HashMap::new();
    ctx.insert("classpath", cp_str); ctx.insert("natives_directory", natives_abs); ctx.insert("launcher_name", "HMCL".into());
    ctx.insert("version_name", version_id.into()); ctx.insert("auth_player_name", player_name.into());
    ctx.insert("auth_uuid", "00000000000000000000000000000000".into()); ctx.insert("auth_access_token", "0".into());
    ctx.insert("game_directory", version_dir.to_string_lossy().to_string()); ctx.insert("assets_root", mc_dir.join("assets").to_string_lossy().to_string());
    ctx.insert("assets_index_name", asset_index); ctx.insert("user_type", "legacy".into());
    ctx.insert("version_type", vj.version_type.clone().unwrap_or_else(|| "release".into()));

    let replace = |s: &str| -> String {
        let mut out = s.to_string();
        for (k, v) in &ctx { out = out.replace(&format!("${{{k}}}"), v); }
        out
    };
    let jvm_args: Vec<String> = jvm_args.iter().map(|a| replace(a)).collect();
    let game_args: Vec<String> = final_game_args.iter().map(|a| replace(a)).collect();

    let main_class = vj.main_class.ok_or("缺少 mainClass")?;
    let java = if java_path.trim().is_empty() { std::env::var("NMCL_JAVA").unwrap_or_else(|_| "java".into()) } else { java_path.to_string() };

    log(format!("启动: {} {}", java, main_class));
    let mut cmd = Command::new(&java);
    cmd.args(&jvm_args).arg(&main_class).args(&game_args);
    cmd.current_dir(&version_dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("启动 Java 失败: {e}"))?;
    let pid = child.id();
    
    // 通知 UI 游戏已启动
    let _ = tx.send(crate::app::WorkerMsg::GameStarted { version: version_id.to_string(), player: player_name.to_string(), pid });

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let tx_o = tx.clone();
    thread::spawn(move || { for line in BufReader::new(stdout).lines().map_while(Result::ok) { let _ = tx_o.send(crate::app::WorkerMsg::Log(format!("[MC]  {line}"))); } });
    let tx_e = tx.clone();
    thread::spawn(move || { for line in BufReader::new(stderr).lines().map_while(Result::ok) { let _ = tx_e.send(crate::app::WorkerMsg::Log(format!("[MC!] {line}"))); } });

    loop {
        if cancel.load(Ordering::Relaxed) { let _ = child.kill(); break; }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => thread::sleep(Duration::from_millis(300)),
            Err(_) => break,
        }
    }
    
    // 通知 UI 游戏已退出
    let _ = tx.send(crate::app::WorkerMsg::GameExited(version_id.to_string()));
    Ok(())
}

fn extract_natives(zip_path: &Path, dest: &Path) -> std::io::Result<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let name = entry.name().to_string();
        if name.ends_with('/') || name.contains("..") || name.starts_with("META-INF") { continue; }
        let out = dest.join(&name);
        if let Some(p) = out.parent() { fs::create_dir_all(p)?; }
        let mut f = fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut f)?;
    }
    Ok(())
}