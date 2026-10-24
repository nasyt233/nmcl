use crate::worker::meta::download_bytes;
use std::fs;
use std::path::Path;

/// 获取 Fabric 指定 MC 版本 + Loader 版本的 profile JSON
pub fn fetch_fabric_profile(
    mc_version: &str,
    loader_version: &str,
) -> Result<serde_json::Value, String> {
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
        mc_version, loader_version
    );
    let bytes = download_bytes(&url)?;
    let profile: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("解析 Fabric profile 失败: {e}"))?;
    Ok(profile)
}

/// 下载并保存 Fabric 的 JSON 文件（供 download.rs 同步调用）
pub fn install_fabric_json(
    mc_dir: &Path,
    install_name: &str,
    mc_version: &str,
    loader_version: &str,
) -> Result<(), String> {
    let mut profile = fetch_fabric_profile(mc_version, loader_version)?;
    
    profile["id"] = serde_json::Value::String(install_name.to_string());
    profile["inheritsFrom"] = serde_json::Value::String(mc_version.to_string());

    let version_dir = mc_dir.join("versions").join(install_name);
    fs::create_dir_all(&version_dir).map_err(|e| format!("创建目录失败: {e}"))?;

    let json_path = version_dir.join(format!("{}.json", install_name));
    let json_str = serde_json::to_string_pretty(&profile)
        .map_err(|e| format!("序列化失败: {e}"))?;
    fs::write(&json_path, &json_str).map_err(|e| format!("写入失败: {e}"))?;

    Ok(())
}