//! "Check for updates": asks GitHub for the latest release only when the user clicks, and on an
//! installed Windows build downloads the installer, verifies it against the release's
//! SHA256SUMS.txt and runs it in passive mode (progress bar only, app restarts afterwards).
//!
//! อัปเดตโปรแกรม: เช็กเฉพาะตอนผู้ใช้กดปุ่ม ไม่มีการเชื่อมต่อเองเบื้องหลัง และจะไม่ติดตั้งไฟล์ที่ค่า
//! SHA-256 ไม่ตรงกับที่ประกาศไว้ใน release

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

const REPO: &str = "bom-sudo/NexaTerm";
const RELEASES_PAGE: &str = "https://github.com/bom-sudo/NexaTerm/releases/latest";
pub const WINDOWS_INSTALLER: &str = "NexaTerm-Setup-x64-web.exe";
const MAX_INSTALLER_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    pub published_at: Option<String>,
    /// Release page (also used as "What's new").
    pub page_url: String,
    /// True when this build can download and run the installer itself (installed Windows build).
    pub can_install: bool,
    pub installer_size: Option<u64>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize, Clone)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("NexaTerm/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn net_err(e: ureq::Error) -> AppError {
    match e {
        ureq::Error::Status(403, _) | ureq::Error::Status(429, _) => {
            AppError::Connection("GitHub is limiting requests right now; try again in a few minutes".into())
        }
        ureq::Error::Status(404, _) => AppError::NotFound("published release".into()),
        ureq::Error::Status(code, _) => AppError::Connection(format!("GitHub answered with HTTP {code}")),
        ureq::Error::Transport(t) => AppError::Connection(format!("Cannot reach GitHub: {t}")),
    }
}

/// "v1.2.3" / "1.2.3" -> (1, 2, 3). Pre-release suffixes are ignored.
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_version(latest), parse_version(current)), (Some(l), Some(c)) if l > c)
}

/// Find `name` in a SHA256SUMS.txt body ("<hex> *<name>" or "<hex>  <name>").
pub fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (hash, file) = l.trim().split_once(char::is_whitespace)?;
        let file = file.trim().trim_start_matches('*');
        (file == name && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
    })
}

fn installed_windows_build(portable: bool) -> bool {
    cfg!(windows) && !portable
}

fn latest_release() -> AppResult<GhRelease> {
    let r: GhRelease = agent()
        .get(&format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(net_err)?
        .into_json()
        .map_err(|e| AppError::Connection(format!("Unexpected answer from GitHub: {e}")))?;
    if r.draft || r.prerelease {
        return Err(AppError::NotFound("published release".into()));
    }
    Ok(r)
}

pub fn check(portable: bool) -> AppResult<UpdateInfo> {
    let r = latest_release()?;
    let current = env!("CARGO_PKG_VERSION").to_string();
    let latest = r.tag_name.trim_start_matches(['v', 'V']).to_string();
    let installer = r.assets.iter().find(|a| a.name == WINDOWS_INSTALLER);
    Ok(UpdateInfo {
        newer: is_newer(&latest, &current),
        can_install: installed_windows_build(portable) && installer.is_some() && r.assets.iter().any(|a| a.name == "SHA256SUMS.txt"),
        installer_size: installer.map(|a| a.size),
        page_url: if r.html_url.starts_with("https://github.com/") { r.html_url } else { RELEASES_PAGE.into() },
        published_at: r.published_at,
        current,
        latest,
    })
}

/// Download the installer of the latest release, verify its SHA-256 and return its path.
/// `progress(done, total)` is called while downloading.
pub fn download_installer(portable: bool, progress: impl FnMut(u64, u64)) -> AppResult<PathBuf> {
    download_installer_from(portable, env!("CARGO_PKG_VERSION"), progress)
}

fn download_installer_from(portable: bool, current: &str, mut progress: impl FnMut(u64, u64)) -> AppResult<PathBuf> {
    if !installed_windows_build(portable) {
        return Err(AppError::Validation("Automatic install is only available for the installed Windows version".into()));
    }
    let r = latest_release()?;
    if !is_newer(&r.tag_name, current) {
        return Err(AppError::Validation("You already have the latest version".into()));
    }
    let asset = r.assets.iter().find(|a| a.name == WINDOWS_INSTALLER).cloned().ok_or_else(|| AppError::NotFound("installer in the latest release".into()))?;
    let sums_url = r.assets.iter().find(|a| a.name == "SHA256SUMS.txt").map(|a| a.browser_download_url.clone()).ok_or_else(|| AppError::NotFound("SHA256SUMS.txt in the latest release".into()))?;
    for u in [&asset.browser_download_url, &sums_url] {
        if !u.starts_with("https://github.com/") {
            return Err(AppError::Validation("Unexpected download location".into()));
        }
    }
    let sums = agent().get(&sums_url).call().map_err(net_err)?.into_string().map_err(|e| AppError::Io(e.to_string()))?;
    let want = expected_hash(&sums, WINDOWS_INSTALLER).ok_or_else(|| AppError::Validation("The release has no checksum for the installer".into()))?;
    if asset.size > MAX_INSTALLER_BYTES {
        return Err(AppError::Validation("Installer is unexpectedly large".into()));
    }

    let resp = agent().get(&asset.browser_download_url).call().map_err(net_err)?;
    let total = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(asset.size);
    let mut reader = resp.into_reader().take(MAX_INSTALLER_BYTES + 1);
    let mut bytes = Vec::with_capacity(total as usize);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| AppError::Connection(format!("Download interrupted: {e}")))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        progress(bytes.len() as u64, total);
    }
    let got: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if got != want {
        return Err(AppError::Validation("The downloaded installer does not match the published SHA-256 checksum; nothing was installed".into()));
    }
    let dir = std::env::temp_dir().join("nexaterm-update");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("NexaTerm-Setup-{}.exe", r.tag_name.trim_start_matches('v')));
    std::fs::write(&path, &bytes)?;
    Ok(path)
}

/// Start the downloaded installer: passive (progress only), update mode, restart the app after.
pub fn run_installer(path: &std::path::Path) -> AppResult<()> {
    let dir = std::env::temp_dir().join("nexaterm-update");
    if path.parent() != Some(dir.as_path()) || path.extension().and_then(|e| e.to_str()) != Some("exe") {
        return Err(AppError::Validation("Unexpected installer path".into()));
    }
    std::process::Command::new(path).args(["/P", "/R", "/UPDATE"]).spawn().map_err(|e| AppError::Io(format!("Cannot start the installer: {e}")))?;
    Ok(())
}

/// Open the release page in the default browser (only NexaTerm's GitHub pages are allowed).
pub fn open_page(url: &str) -> AppResult<()> {
    if !url.starts_with("https://github.com/bom-sudo/NexaTerm/") {
        return Err(AppError::Validation("Only NexaTerm's GitHub pages can be opened".into()));
    }
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32.exe").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    r.map(|_| ()).map_err(|e| AppError::Io(format!("Cannot open the browser: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.10"), Some((1, 10, 0)));
        assert_eq!(parse_version("2.0.0-beta.1"), Some((2, 0, 0)));
        assert_eq!(parse_version("garbage"), None);
        assert!(is_newer("v1.1.1", "1.1.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(!is_newer("1.1.0", "1.1.0"));
        assert!(!is_newer("1.0.9", "1.1.0"));
        assert!(!is_newer("nonsense", "1.1.0"));
    }

    #[test]
    fn checksum_lookup() {
        let sums = "811f42c68d8ac923e8f8fa4a0caa6ab9bc1f03803d5b1afc342e51afe253fcc2 *NexaTerm-Portable-x64.zip\n534D69EC0C3D9DAD0BA233D841C2863F23AC7CDB949A884B242814540A796BD4 *NexaTerm-Setup-x64-web.exe\nbad *NexaTerm-x64.deb\n";
        assert_eq!(expected_hash(sums, WINDOWS_INSTALLER).as_deref(), Some("534d69ec0c3d9dad0ba233d841c2863f23ac7cdb949a884b242814540a796bd4"));
        assert_eq!(expected_hash(sums, "NexaTerm-x64.deb"), None);
        assert_eq!(expected_hash(sums, "missing.exe"), None);
        assert!(expected_hash("abc  NexaTerm-Setup-x64-web.exe", WINDOWS_INSTALLER).is_none());
    }

    /// Talks to the real GitHub API; run with `cargo test -- --ignored live_check`.
    #[test]
    #[ignore]
    fn live_check() {
        let info = check(true).expect("latest release");
        println!("{info:?}");
        assert!(parse_version(&info.latest).is_some());
        assert!(info.page_url.starts_with("https://github.com/"));
        assert!(!info.can_install, "portable builds never self-install");
    }

    /// Downloads the real latest installer as if 1.0.0 were installed and checks the SHA-256 path.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn live_download_verifies_checksum() {
        let mut calls = 0;
        let path = download_installer_from(false, "1.0.0", |d, t| {
            calls += 1;
            assert!(d <= t.max(d));
        })
        .expect("download + checksum");
        assert!(calls > 0);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], b"MZ");
        println!("verified installer at {} ({} bytes)", path.display(), bytes.len());
        std::fs::remove_file(&path).ok();
        assert!(download_installer_from(true, "1.0.0", |_, _| {}).is_err(), "portable never self-installs");
    }

    #[test]
    fn only_project_pages_open() {
        assert!(open_page("https://evil.example.com/").is_err());
        assert!(open_page("file:///C:/Windows/System32/calc.exe").is_err());
    }
}
