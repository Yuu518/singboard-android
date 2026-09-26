use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::Manager;

use super::update::{GhRelease, UPDATE_LOCK, apply_mirror, download_asset, emit_progress, github_get};

const PANEL_REPO: &str = "Yuu518/singboard-android";
const PROGRESS_EVENT: &str = "panel-update-progress";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelUpdateInfo {
    current_version: String,
    latest_version: String,
    has_update: bool,
    out_of_sync: bool,
    published_at: String,
    asset_url: String,
    asset_size: u64,
    asset_digest: String,
}

fn abi() -> Result<&'static str, String> {
    match std::env::consts::ARCH {
        "aarch64" => Ok("arm64-v8a"),
        "arm" => Ok("armeabi-v7a"),
        "x86_64" => Ok("x86_64"),
        "x86" => Ok("x86"),
        other => Err(format!("不支持的 CPU 架构: {}", other)),
    }
}

fn asset_candidates() -> Result<[String; 2], String> {
    Ok([
        format!("singboard-{}.apk", abi()?),
        "singboard-universal.apk".to_string(),
    ])
}

fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim().trim_start_matches(['v', 'V']);
    let mut parts = text.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch_raw = parts.next().unwrap_or("0");
    let digits: String = patch_raw.chars().take_while(char::is_ascii_digit).collect();
    let patch = digits.parse().ok()?;
    Some((major, minor, patch))
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => latest.trim().trim_start_matches(['v', 'V']) != current.trim(),
    }
}

fn digest_hash(asset_digest: &str) -> Option<&str> {
    asset_digest
        .trim()
        .strip_prefix("sha256:")
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| format!("读取文件失败: {}", e))?;
    Ok(format!("{:x}", Sha256::digest(&data)))
}

fn installed_out_of_sync(app: &tauri::AppHandle, asset_digest: &str) -> bool {
    let Some(expected) = digest_hash(asset_digest) else {
        return false;
    };
    let Ok(info) = crate::native::apk_info(app) else {
        return false;
    };
    match sha256_file(Path::new(&info.path)) {
        Ok(actual) => !actual.eq_ignore_ascii_case(expected),
        Err(_) => false,
    }
}

#[tauri::command]
pub async fn check_panel_update(app: tauri::AppHandle) -> Result<PanelUpdateInfo, String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        PANEL_REPO
    );
    let release: GhRelease = github_get(&url)
        .await?
        .json()
        .await
        .map_err(|e| format!("解析 GitHub API 响应失败: {}", e))?;

    let candidates = asset_candidates()?;
    let asset = candidates
        .iter()
        .find_map(|name| {
            release
                .assets
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case(name))
        })
        .ok_or_else(|| format!("该版本未提供 {} 安装包", candidates[0]))?;

    let current_version = app.package_info().version.to_string();
    let latest_version = release.tag_name.trim_start_matches(['v', 'V']).to_string();
    let asset_digest = asset.digest.clone().unwrap_or_default();
    let has_update = is_newer(&latest_version, &current_version);

    let out_of_sync = if has_update {
        false
    } else {
        let digest = asset_digest.clone();
        let app = app.clone();
        tokio::task::spawn_blocking(move || installed_out_of_sync(&app, &digest))
            .await
            .unwrap_or(false)
    };

    Ok(PanelUpdateInfo {
        has_update,
        out_of_sync,
        current_version,
        latest_version,
        published_at: release.published_at.unwrap_or_default(),
        asset_url: asset.browser_download_url.clone(),
        asset_size: asset.size,
        asset_digest,
    })
}

fn verify_digest(file: &Path, asset_digest: &str, mirror: &Option<String>) -> Result<(), String> {
    let Some(expected) = digest_hash(asset_digest) else {
        let mirrored = mirror
            .as_deref()
            .map(str::trim)
            .is_some_and(|m| !m.is_empty());
        if mirrored {
            return Err("该版本缺少校验信息，已中止更新".into());
        }
        return Ok(());
    };

    let actual = sha256_file(file)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err("文件校验失败，已中止更新".into())
    }
}

#[tauri::command]
pub async fn perform_panel_update(
    app: tauri::AppHandle,
    asset_url: String,
    asset_size: u64,
    asset_digest: String,
    mirror: Option<String>,
) -> Result<(), String> {
    let _guard = UPDATE_LOCK
        .try_lock()
        .map_err(|_| "更新正在进行中".to_string())?;

    let staging = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("获取缓存目录失败: {e}"))?
        .join("panel-update");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| format!("创建临时目录失败: {}", e))?;
    let cleanup = |msg: String| {
        let _ = std::fs::remove_dir_all(&staging);
        msg
    };

    let apk = staging.join("singboard.apk");
    let download_url = apply_mirror(&mirror, &asset_url);
    download_asset(&app, PROGRESS_EVENT, &download_url, asset_size, &apk)
        .await
        .map_err(cleanup)?;

    emit_progress(&app, PROGRESS_EVENT, "verify", 0, 0);
    {
        let apk = apk.clone();
        tokio::task::spawn_blocking(move || verify_digest(&apk, &asset_digest, &mirror))
            .await
            .map_err(|e| format!("任务执行失败: {}", e))
            .and_then(|r| r)
            .map_err(cleanup)?;
    }

    emit_progress(&app, PROGRESS_EVENT, "replace", 0, 0);
    crate::native::install_apk(&app, apk.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn cleanup_panel_update(app: tauri::AppHandle) {
    if let Ok(dir) = app.path().app_cache_dir() {
        let _ = std::fs::remove_dir_all(dir.join("panel-update"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_ignores_prefix_and_suffix() {
        assert!(is_newer("v1.2.0", "1.1.9"));
        assert!(!is_newer("1.2.0", "1.2.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(!is_newer("1.0.0-beta", "1.0.0"));
    }
}
