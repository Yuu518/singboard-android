use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Emitter, Manager};
use tokio::io::AsyncWriteExt;

use crate::root::{self, SERVICE_HOME, quote};

pub(crate) static UPDATE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) const CORE_PROGRESS_EVENT: &str = "core-update-progress";

const CORE_BIN_NAME: &str = "sing-box";
const STAGED_MANIFEST: &str = "staged.json";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoreUpdateInfo {
    version: String,
    prerelease: bool,
    published_at: String,
    asset_name: String,
    asset_url: String,
    asset_size: u64,
    asset_digest: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreUpdateResult {
    version: String,
    restarted: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateProgress {
    phase: &'static str,
    downloaded: u64,
    total: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StagedCore {
    asset_url: String,
    asset_size: u64,
    asset_digest: String,
}

#[derive(Deserialize)]
pub(crate) struct GhAsset {
    pub(crate) name: String,
    pub(crate) browser_download_url: String,
    pub(crate) size: u64,
    #[serde(default)]
    pub(crate) digest: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct GhRelease {
    pub(crate) tag_name: String,
    pub(crate) prerelease: bool,
    #[serde(default)]
    pub(crate) draft: bool,
    #[serde(default)]
    pub(crate) published_at: Option<String>,
    pub(crate) assets: Vec<GhAsset>,
}

fn validate_repo(repo: &str) -> Result<(), String> {
    let parts: Vec<&str> = repo.split('/').collect();
    let valid = parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        });
    if valid {
        Ok(())
    } else {
        Err("仓库格式应为 owner/repo".into())
    }
}

fn android_arch_names() -> Result<&'static [&'static str], String> {
    match std::env::consts::ARCH {
        "aarch64" => Ok(&["arm64", "arm64-v8a"]),
        "arm" => Ok(&["arm", "armv7", "armeabi-v7a"]),
        "x86_64" => Ok(&["amd64", "x86_64"]),
        "x86" => Ok(&["386", "x86"]),
        other => Err(format!("不支持的 CPU 架构: {}", other)),
    }
}

fn staging_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map(|dir| dir.join("core-update"))
        .map_err(|e| format!("获取缓存目录失败: {e}"))
}

fn core_asset_hash(asset_digest: &str) -> Result<&str, String> {
    asset_digest
        .trim()
        .strip_prefix("sha256:")
        .map(str::trim)
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| "上游未提供有效的 SHA-256 校验信息，已中止核心更新".to_string())
}

fn take_staged(staging: &Path, asset_url: &str, asset_size: u64, asset_digest: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(staging.join(STAGED_MANIFEST)) else {
        return false;
    };
    let Ok(staged) = serde_json::from_str::<StagedCore>(&text) else {
        return false;
    };
    let same_digest = match (
        core_asset_hash(&staged.asset_digest),
        core_asset_hash(asset_digest),
    ) {
        (Ok(a), Ok(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    };
    staged.asset_url == asset_url
        && staged.asset_size == asset_size
        && same_digest
        && extract_core(
            &staging.join("core.tar.gz"),
            &staging.join("files"),
            asset_digest,
        )
        .is_ok()
}

pub(crate) fn emit_progress(
    app: &tauri::AppHandle,
    event: &str,
    phase: &'static str,
    downloaded: u64,
    total: u64,
) {
    let _ = app.emit(
        event,
        UpdateProgress {
            phase,
            downloaded,
            total,
        },
    );
}

pub(crate) fn apply_mirror(mirror: &Option<String>, url: &str) -> String {
    match mirror.as_deref().map(str::trim) {
        Some(m) if !m.is_empty() => format!("{}/{}", m.trim_end_matches('/'), url),
        _ => url.to_string(),
    }
}

pub(crate) async fn github_get(url: &str) -> Result<reqwest::Response, String> {
    let client = super::network::build_client(Some(Duration::from_secs(30)))
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    let resp = client
        .get(url)
        .header("User-Agent", "singboard")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("请求 GitHub API 失败: {}", e))?;

    let status = resp.status();
    if status == reqwest::StatusCode::FORBIDDEN {
        let body = resp.text().await.unwrap_or_default();
        if body.contains("rate limit") {
            return Err("GitHub API 限流，请稍后再试".into());
        }
        return Err("GitHub API 拒绝访问 (403)".into());
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err("仓库不存在或没有发布版本".into());
    }
    if !status.is_success() {
        return Err(format!("GitHub API 返回错误: {}", status));
    }
    Ok(resp)
}

fn pick_android_asset(release: &GhRelease, arch_names: &[&str]) -> Result<CoreUpdateInfo, String> {
    let asset = arch_names
        .iter()
        .find_map(|arch| {
            let suffix = format!("-android-{arch}.tar.gz");
            release.assets.iter().find(|a| a.name.ends_with(&suffix))
        })
        .ok_or_else(|| {
            format!(
                "{} 未提供适用于 android-{} 的资产",
                release.tag_name,
                arch_names.join("/")
            )
        })?;
    Ok(CoreUpdateInfo {
        version: release.tag_name.clone(),
        prerelease: release.prerelease,
        published_at: release.published_at.clone().unwrap_or_default(),
        asset_name: asset.name.clone(),
        asset_url: asset.browser_download_url.clone(),
        asset_size: asset.size,
        asset_digest: asset.digest.clone().unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn check_core_update(repo: String, channel: String) -> Result<CoreUpdateInfo, String> {
    let repo = repo.trim().to_string();
    validate_repo(&repo)?;
    let arch_names = android_arch_names()?;

    let release = if channel == "testing" {
        let url = format!("https://api.github.com/repos/{}/releases?per_page=10", repo);
        let releases: Vec<GhRelease> = github_get(&url)
            .await?
            .json()
            .await
            .map_err(|e| format!("解析 GitHub API 响应失败: {}", e))?;
        releases
            .into_iter()
            .find(|r| !r.draft)
            .ok_or("该仓库暂无发布版本")?
    } else {
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
        github_get(&url)
            .await?
            .json()
            .await
            .map_err(|e| format!("解析 GitHub API 响应失败: {}", e))?
    };

    pick_android_asset(&release, arch_names)
}

pub(crate) async fn download_asset(
    app: &tauri::AppHandle,
    event: &str,
    url: &str,
    expected_size: u64,
    dest: &Path,
) -> Result<(), String> {
    let client =
        super::network::build_client(None).map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    let mut resp = client
        .get(url)
        .header("User-Agent", "singboard")
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(expected_size);
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("创建临时文件失败: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut last_emit = Instant::now();
    emit_progress(app, event, "download", 0, total);
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("下载中断: {}", e))? {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed() >= Duration::from_millis(200) {
            emit_progress(app, event, "download", downloaded, total);
            last_emit = Instant::now();
        }
    }
    file.flush()
        .await
        .map_err(|e| format!("写入临时文件失败: {}", e))?;
    emit_progress(app, event, "download", downloaded, total);

    if expected_size > 0 && downloaded != expected_size {
        return Err(format!(
            "下载文件不完整（{} / {} 字节）",
            downloaded, expected_size
        ));
    }
    Ok(())
}

fn extract_core(archive_path: &Path, files_dir: &Path, asset_digest: &str) -> Result<(), String> {
    let expected = core_asset_hash(asset_digest)?;
    let bytes = std::fs::read(archive_path).map_err(|e| format!("打开压缩包失败: {}", e))?;
    if !format!("{:x}", Sha256::digest(&bytes)).eq_ignore_ascii_case(expected) {
        return Err("核心文件 SHA-256 校验失败，已中止更新".into());
    }
    if files_dir.exists() {
        std::fs::remove_dir_all(files_dir).map_err(|e| format!("清理临时目录失败: {}", e))?;
    }
    std::fs::create_dir_all(files_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;

    let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| format!("读取压缩包失败: {}", e))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("读取压缩包失败: {}", e))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let name = entry
            .path()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        if name != CORE_BIN_NAME {
            continue;
        }
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| format!("解压失败: {}", e))?;
        std::fs::write(files_dir.join(CORE_BIN_NAME), data)
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
        return Ok(());
    }
    Err(format!("压缩包内未找到 {}", CORE_BIN_NAME))
}

async fn download_and_extract(
    app: &tauri::AppHandle,
    staging: &Path,
    asset_url: &str,
    asset_size: u64,
    asset_digest: &str,
    mirror: &Option<String>,
) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(staging);
    std::fs::create_dir_all(staging).map_err(|e| format!("创建临时目录失败: {}", e))?;
    let archive = staging.join("core.tar.gz");
    download_asset(
        app,
        CORE_PROGRESS_EVENT,
        &apply_mirror(mirror, asset_url),
        asset_size,
        &archive,
    )
    .await?;

    emit_progress(app, CORE_PROGRESS_EVENT, "extract", 0, 0);
    let staging = staging.to_path_buf();
    let asset_url = asset_url.to_string();
    let asset_digest = asset_digest.to_string();
    tokio::task::spawn_blocking(move || {
        extract_core(&archive, &staging.join("files"), &asset_digest)?;
        let manifest = serde_json::to_string(&StagedCore {
            asset_url,
            asset_size,
            asset_digest,
        })
        .map_err(|e| format!("写入清单失败: {}", e))?;
        std::fs::write(staging.join(STAGED_MANIFEST), manifest)
            .map_err(|e| format!("写入清单失败: {}", e))
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}

#[tauri::command]
pub async fn probe_asset_core_hash(
    app: tauri::AppHandle,
    asset_url: String,
    asset_size: u64,
    asset_digest: String,
    mirror: Option<String>,
) -> Result<String, String> {
    core_asset_hash(&asset_digest)?;
    let _guard = UPDATE_LOCK
        .try_lock()
        .map_err(|_| "更新正在进行中".to_string())?;

    let staging = staging_dir(&app)?;
    download_and_extract(&app, &staging, &asset_url, asset_size, &asset_digest, &mirror)
        .await
        .map_err(|e| {
            let _ = std::fs::remove_dir_all(&staging);
            e
        })?;
    let data = std::fs::read(staging.join("files").join(CORE_BIN_NAME))
        .map_err(|e| format!("读取文件失败: {}", e))?;
    Ok(format!("{:x}", Sha256::digest(&data)))
}

async fn swap_core(staged: &Path, target: &str) -> Result<(String, bool), String> {
    let pending = format!("{SERVICE_HOME}/.staging/{CORE_BIN_NAME}");
    let version_line = root::run_async(format!(
        "set -e\nmkdir -p {stage_dir}\ncp -f {src} {pending}\nchmod 755 {pending}\n{pending} version",
        stage_dir = quote(&format!("{SERVICE_HOME}/.staging")),
        src = quote(&staged.to_string_lossy()),
        pending = quote(&pending)
    ))
    .await
    .map_err(|e| format!("下载的核心无法运行: {e}"))?;
    let version = version_line.lines().next().unwrap_or("unknown").to_string();

    let was_running = super::service::is_running().await;
    if was_running {
        super::service::stop().await?;
    }

    let backup = format!("{target}.bak");
    root::run_async(format!(
        "set -e\nmkdir -p \"$(dirname {t})\"\nif [ -f {t} ]; then cp -f {t} {b}; fi\nmv -f {p} {t}\nchmod 755 {t}",
        t = quote(target),
        b = quote(&backup),
        p = quote(&pending)
    ))
    .await
    .map_err(|e| format!("替换核心失败: {e}"))?;

    if !was_running {
        let _ = root::run_async(format!("rm -f {}", quote(&backup))).await;
        return Ok((version, false));
    }

    let started = match super::service::start().await {
        Ok(()) => {
            tokio::time::sleep(Duration::from_secs(2)).await;
            super::service::is_running().await
        }
        Err(_) => false,
    };
    if started {
        let _ = root::run_async(format!("rm -f {}", quote(&backup))).await;
        return Ok((version, true));
    }

    let _ = root::run_async(format!(
        "if [ -f {b} ]; then mv -f {b} {t}; chmod 755 {t}; fi",
        b = quote(&backup),
        t = quote(target)
    ))
    .await;
    let _ = super::service::start().await;
    Err("新核心启动失败，已恢复旧版本".into())
}

#[tauri::command]
pub async fn perform_core_update(
    app: tauri::AppHandle,
    asset_url: String,
    asset_size: u64,
    asset_digest: String,
    mirror: Option<String>,
    singbox_path: String,
) -> Result<CoreUpdateResult, String> {
    core_asset_hash(&asset_digest)?;
    if !root::enabled() {
        return Err("核心更新需要启用 root 模式".into());
    }
    let _guard = UPDATE_LOCK
        .try_lock()
        .map_err(|_| "更新正在进行中".to_string())?;

    let target = singbox_path.trim().to_string();
    if target.is_empty() {
        return Err("请先在服务配置中设置 sing-box 路径".into());
    }

    let staging = staging_dir(&app)?;
    let cleanup = |msg: String| {
        let _ = std::fs::remove_dir_all(&staging);
        msg
    };

    let reusable = {
        let staging = staging.clone();
        let asset_url = asset_url.clone();
        let asset_digest = asset_digest.clone();
        tokio::task::spawn_blocking(move || {
            take_staged(&staging, &asset_url, asset_size, &asset_digest)
        })
        .await
        .map_err(|e| format!("任务执行失败: {}", e))?
    };
    if !reusable {
        download_and_extract(&app, &staging, &asset_url, asset_size, &asset_digest, &mirror)
            .await
            .map_err(cleanup)?;
    }

    emit_progress(&app, CORE_PROGRESS_EVENT, "replace", 0, 0);
    let staged = staging.join("files").join(CORE_BIN_NAME);
    let result = swap_core(&staged, &target).await;
    let _ = std::fs::remove_dir_all(&staging);
    let (version, restarted) = result?;
    Ok(CoreUpdateResult { version, restarted })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_digest_requires_a_complete_sha256_hash() {
        assert!(core_asset_hash("").is_err());
        assert!(core_asset_hash("sha256:abc").is_err());
        let full = format!("sha256:{}", "a".repeat(64));
        assert_eq!(core_asset_hash(&full).unwrap(), "a".repeat(64));
    }

    fn release_with(names: &[&str]) -> GhRelease {
        GhRelease {
            tag_name: "v1.0.0".into(),
            prerelease: false,
            draft: false,
            published_at: None,
            assets: names
                .iter()
                .map(|name| GhAsset {
                    name: name.to_string(),
                    browser_download_url: format!("https://example.com/{name}"),
                    size: 1,
                    digest: None,
                })
                .collect(),
        }
    }

    #[test]
    fn picks_android_asset_by_go_or_abi_arch_name() {
        let go_style = release_with(&[
            "sing-box-1.0.0-android-arm.tar.gz",
            "sing-box-1.0.0-android-arm64.tar.gz",
            "sing-box-1.0.0-android-amd64v3.tar.gz",
            "sing-box-1.0.0-android-amd64.tar.gz",
        ]);
        let abi_style = release_with(&[
            "sing-box-1.0.0-Yuu-android-armeabi-v7a.tar.gz",
            "sing-box-1.0.0-Yuu-android-arm64-v8a.tar.gz",
        ]);

        let pick = |release: &GhRelease, arch: &[&str]| {
            pick_android_asset(release, arch).map(|info| info.asset_name)
        };
        assert_eq!(
            pick(&go_style, &["arm64", "arm64-v8a"]).unwrap(),
            "sing-box-1.0.0-android-arm64.tar.gz"
        );
        assert_eq!(
            pick(&go_style, &["arm", "armv7", "armeabi-v7a"]).unwrap(),
            "sing-box-1.0.0-android-arm.tar.gz"
        );
        assert_eq!(
            pick(&go_style, &["amd64", "x86_64"]).unwrap(),
            "sing-box-1.0.0-android-amd64.tar.gz"
        );
        assert_eq!(
            pick(&abi_style, &["arm64", "arm64-v8a"]).unwrap(),
            "sing-box-1.0.0-Yuu-android-arm64-v8a.tar.gz"
        );
        assert_eq!(
            pick(&abi_style, &["arm", "armv7", "armeabi-v7a"]).unwrap(),
            "sing-box-1.0.0-Yuu-android-armeabi-v7a.tar.gz"
        );
        assert!(pick(&abi_style, &["amd64", "x86_64"]).is_err());
    }

    #[test]
    fn extracts_only_the_core_binary_from_a_verified_archive() {
        let dir = std::env::temp_dir().join(format!("singboard-core-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        for (name, body) in [
            ("sing-box-1.0.0-android-arm64/LICENSE", b"license".as_slice()),
            ("sing-box-1.0.0-android-arm64/sing-box", b"core-binary".as_slice()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append_data(&mut header, name, body).unwrap();
        }
        let archive = builder.into_inner().unwrap().finish().unwrap();
        let archive_path = dir.join("core.tar.gz");
        std::fs::write(&archive_path, &archive).unwrap();
        let digest = format!("sha256:{:x}", Sha256::digest(&archive));

        let files = dir.join("files");
        extract_core(&archive_path, &files, &digest).unwrap();
        assert_eq!(std::fs::read(files.join("sing-box")).unwrap(), b"core-binary");
        assert!(!files.join("LICENSE").exists());

        let wrong = format!("sha256:{}", "0".repeat(64));
        assert!(extract_core(&archive_path, &files, &wrong).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
