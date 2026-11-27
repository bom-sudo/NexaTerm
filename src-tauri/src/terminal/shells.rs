//! Detect locally available shells.

use serde::Serialize;
#[allow(unused_imports)]
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellProfile {
    pub id: String,
    pub name: String,
    pub path: String,
    pub args: Vec<String>,
    pub kind: String,
}

#[cfg(windows)]
fn env_path(var: &str, rel: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(|v| Path::new(&v).join(rel)).filter(|p| p.exists())
}

fn which(exe: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(exe)).find(|p| p.is_file())
}

/// Shells available on this machine; the first entry is the platform default.
pub fn detect() -> Vec<ShellProfile> {
    #[cfg(windows)]
    return detect_windows();
    #[cfg(unix)]
    return detect_unix();
}

/// Unix: the login shell from $SHELL first, then common shells listed in /etc/shells or on PATH.
#[cfg(unix)]
fn detect_unix() -> Vec<ShellProfile> {
    let mut v: Vec<ShellProfile> = Vec::new();
    let listed: Vec<PathBuf> = std::fs::read_to_string("/etc/shells")
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('/'))
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .collect();
    let find = |name: &str| listed.iter().find(|p| p.file_name().map(|f| f == name).unwrap_or(false)).cloned().or_else(|| which(name));
    let mut add = |path: PathBuf, label: Option<&str>| {
        let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        if file.is_empty() || v.iter().any(|s| s.id == file) {
            return;
        }
        let name = match file.as_str() {
            "bash" => "Bash",
            "zsh" => "Zsh",
            "fish" => "Fish",
            "sh" => "sh",
            "dash" => "Dash",
            other => other,
        }
        .to_string();
        v.push(ShellProfile {
            id: file.clone(),
            name: label.map(|l| format!("{name} ({l})")).unwrap_or(name),
            path: path.display().to_string(),
            args: if matches!(file.as_str(), "bash" | "zsh" | "fish") { vec!["-l".into()] } else { vec![] },
            kind: file,
        });
    };
    if let Some(sh) = std::env::var_os("SHELL").map(PathBuf::from).filter(|p| p.is_file()) {
        add(sh, Some("login shell"));
    }
    for name in ["bash", "zsh", "fish", "sh"] {
        if let Some(p) = find(name) {
            add(p, None);
        }
    }
    v
}

#[cfg(windows)]
fn detect_windows() -> Vec<ShellProfile> {
    let mut v = Vec::new();
    let mut push = |id: &str, name: &str, path: PathBuf, args: &[&str], kind: &str| {
        v.push(ShellProfile {
            id: id.into(),
            name: name.into(),
            path: path.display().to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            kind: kind.into(),
        })
    };

    if let Some(p) = env_path("SystemRoot", r"System32\WindowsPowerShell\v1.0\powershell.exe") {
        push("powershell", "Windows PowerShell", p, &["-NoLogo"], "powershell");
    }
    let pwsh = which("pwsh.exe").or_else(|| env_path("ProgramFiles", r"PowerShell\7\pwsh.exe"));
    if let Some(p) = pwsh {
        push("pwsh", "PowerShell 7", p, &["-NoLogo"], "powershell");
    }
    let cmd = std::env::var_os("ComSpec").map(PathBuf::from).filter(|p| p.exists()).or_else(|| env_path("SystemRoot", r"System32\cmd.exe"));
    if let Some(p) = cmd {
        push("cmd", "Command Prompt", p, &[], "cmd");
    }
    let git_bash = env_path("ProgramFiles", r"Git\bin\bash.exe")
        .or_else(|| env_path("ProgramFiles(x86)", r"Git\bin\bash.exe"))
        .or_else(|| env_path("LOCALAPPDATA", r"Programs\Git\bin\bash.exe"));
    if let Some(p) = git_bash {
        push("gitbash", "Git Bash", p, &["--login", "-i"], "bash");
    }
    if let Some(wsl) = env_path("SystemRoot", r"System32\wsl.exe") {
        for distro in wsl_distros(&wsl) {
            v.push(ShellProfile {
                id: format!("wsl:{distro}"),
                name: format!("WSL: {distro}"),
                path: wsl.display().to_string(),
                args: vec!["-d".into(), distro.clone(), "--cd".into(), "~".into()],
                kind: "wsl".into(),
            });
        }
    }
    v
}

/// `wsl.exe -l -q` prints UTF-16LE. Runs with a timeout so a broken WSL install cannot hang startup.
#[cfg(windows)]
fn wsl_distros(wsl: &Path) -> Vec<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let child = std::process::Command::new(wsl)
        .args(["-l", "-q"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn();
    let Ok(child) = child else { return vec![] };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let Ok(Ok(out)) = rx.recv_timeout(std::time::Duration::from_secs(4)) else { return vec![] };
    if !out.status.success() {
        return vec![];
    }
    parse_wsl_list(&out.stdout)
}

pub fn parse_wsl_list(bytes: &[u8]) -> Vec<String> {
    let text = if bytes.len() >= 2 && bytes.len() % 2 == 0 && bytes.iter().skip(1).step_by(2).all(|b| *b == 0) {
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    text.lines()
        .map(|l| l.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}' || c == '\0').to_string())
        .filter(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn detects_a_posix_shell() {
        let shells = detect();
        assert!(!shells.is_empty(), "no shell found");
        assert!(shells.iter().all(|s| std::path::Path::new(&s.path).is_file()));
        assert!(shells.iter().any(|s| s.id == "sh" || s.id == "bash"));
    }

    #[cfg(windows)]
    #[test]
    fn detects_cmd_and_powershell() {
        let shells = detect();
        assert!(shells.iter().any(|s| s.id == "cmd"));
        assert!(shells.iter().any(|s| s.id == "powershell"));
    }

    #[test]
    fn parses_utf16_wsl_list() {
        let s = "Ubuntu\r\ndocker-desktop\r\n";
        let bytes: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(parse_wsl_list(&bytes), vec!["Ubuntu", "docker-desktop"]);
        assert!(parse_wsl_list(b"").is_empty());
    }
}
