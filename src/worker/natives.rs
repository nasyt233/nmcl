use crate::app::WorkerMsg;
use std::{fs, io::Read, path::PathBuf, sync::{atomic::{AtomicBool, Ordering}, mpsc::Sender, Arc}, thread};
use zip::ZipArchive;

/// 检测当前系统是否需要替换 ARM64 Natives
pub fn needs_arm64_natives() -> bool {
    // 1. 检查是否为 Linux 系统
    let is_linux = cfg!(target_os = "linux");
    // 2. 检查 CPU 架构是否为 aarch64
    let is_aarch64 = std::env::consts::ARCH == "aarch64";
    is_linux && is_aarch64
}

/// 根据 Minecraft 版本决定使用 LWJGL 2 还是 3 的 ARM64 资源
pub fn get_arm64_natives_jar_url(version_id: &str) -> Option<&'static str> {
    // 解析主版本号，Minecraft 1.13 及以上使用 LWJGL 3
    let is_modern = version_id
        .split('.')
        .nth(1)
        .and_then(|s| s.parse::<u32>().ok())
        .map_or(true, |minor| minor >= 13);

    if is_modern {
        // LWJGL 3 ARM64 的镜像源 (BMCLAPI 镜像，社区常用)
        // 注意：这里需要根据实际使用的 LWJGL 3.x 版本替换链接。
        // 下面以 3.3.1 为例，请根据你的 version.json 中 lwjgl 版本调整。
        Some("https://bmclapi2.bangbang93.com/maven/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux-arm64.jar")
    } else {
        // LWJGL 2 ARM64 的社区构建版 (JJTech0130 提供)
        Some("https://github.com/JJTech0130/Aarch64-Natives/raw/master/lwjgl-platform-2.9.4-nightly-20150209-natives-linux.jar")
    }
}

/// 异步下载并解压 ARM64 Natives
pub fn start_download_arm64_natives(
    tx: Sender<WorkerMsg>,
    cancel: Arc<AtomicBool>,
    natives_dir: PathBuf,
    version_id: String,
) {
    thread::spawn(move || {
        let log = |s: String| { let _ = tx.send(WorkerMsg::Log(s)); };

        if !needs_arm64_natives() {
            let _ = tx.send(WorkerMsg::Finished(Ok(())));
            return;
        }

        let Some(url) = get_arm64_natives_jar_url(&version_id) else {
            log("[警告] 未找到对应版本的 ARM64 Natives 下载源，将使用默认库".into());
            let _ = tx.send(WorkerMsg::Finished(Ok(())));
            return;
        };

        log(format!("[信息] 检测到 ARM64 架构，正在从社区源下载兼容的 Natives..."));

        // 1. 下载 Jar 包到内存
        let bytes = match ureq::get(url).call() {
            Ok(resp) => {
                let mut buf = Vec::new();
                if let Err(e) = resp.into_reader().read_to_end(&mut buf) {
                    let _ = tx.send(WorkerMsg::Finished(Err(format!("下载 Natives 失败: {e}"))));
                    return;
                }
                buf
            }
            Err(e) => {
                let _ = tx.send(WorkerMsg::Finished(Err(format!("请求 Natives 资源失败: {e}"))));
                return;
            }
        };

        if cancel.load(Ordering::Relaxed) {
            let _ = tx.send(WorkerMsg::Finished(Ok(())));
            return;
        }

        // 2. 清理旧的 natives 目录
        if natives_dir.exists() {
            let _ = fs::remove_dir_all(&natives_dir);
        }
        if let Err(e) = fs::create_dir_all(&natives_dir) {
            let _ = tx.send(WorkerMsg::Finished(Err(format!("创建 natives 目录失败: {e}"))));
            return;
        }

        // 3. 解压到 natives 目录
        let reader = std::io::Cursor::new(bytes);
        let mut archive = match ZipArchive::new(reader) {
            Ok(a) => a,
            Err(e) => {
                let _ = tx.send(WorkerMsg::Finished(Err(format!("解析 Natives Jar 失败: {e}"))));
                return;
            }
        };

        let mut extracted = 0;
        for i in 0..archive.len() {
            let mut file = match archive.by_index(i) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let name = file.name().to_string();
            // 忽略目录和元数据
            if name.ends_with('/') || name.starts_with("META-INF") || name.contains("..") {
                continue;
            }

            let out_path = natives_dir.join(&name);
            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            if let Ok(mut out_file) = fs::File::create(&out_path) {
                if std::io::copy(&mut file, &mut out_file).is_ok() {
                    extracted += 1;
                }
            }
        }

        log(format!("[信息] 成功解压 {} 个 ARM64 Natives 文件到 {:?}", extracted, natives_dir));
        let _ = tx.send(WorkerMsg::Finished(Ok(())));
    });
}