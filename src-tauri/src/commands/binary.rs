use std::path::Path;

use crate::root::{self, fs, quote};

#[tauri::command]
pub async fn get_file_hash(path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || fs::sha256(Path::new(&path)))
        .await
        .map_err(|e| format!("任务执行失败: {}", e))?
}

pub(crate) async fn core_version(singbox_path: &str) -> Result<String, String> {
    let output = root::run_async(format!("{} version", quote(singbox_path)))
        .await
        .map_err(|e| format!("Failed to run sing-box version: {}", e))?;
    Ok(output.lines().next().unwrap_or("unknown").to_string())
}

#[tauri::command]
pub async fn get_singbox_version(singbox_path: String) -> Result<String, String> {
    core_version(&singbox_path).await
}
