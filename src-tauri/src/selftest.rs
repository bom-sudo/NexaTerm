//! `NexaTerm.exe --self-test`: headless verification of a packaged build on the target machine.
//! Checks the data location, database (integrity, CRUD, persistence across runs), Windows
//! Credential Manager, shell detection, ConPTY, logging and the WebView2 runtime.
//! Writes the report to stdout (when redirected) and to `<data dir>\logs\self-test.txt`.
//! It creates no lasting data except the `selfTest.lastRun` setting used to prove persistence.

use crate::credentials::SecretStore;
use crate::database::{self, models::SessionInput, repo};
use std::io::Write;
use std::time::Duration;

pub fn run() -> i32 {
    let mut report = String::new();
    let mut failures = 0;
    let mut check = |name: &str, r: Result<String, String>| {
        let line = match &r {
            Ok(d) => format!("PASS  {name}: {d}"),
            Err(e) => {
                failures += 1;
                format!("FAIL  {name}: {e}")
            }
        };
        report.push_str(&line);
        report.push('\n');
    };

    let loc = database::resolve_data_location();
    crate::applog::init(&loc.dir);
    check(
        "data location",
        Ok(format!("{} (portable={}){}", loc.dir.display(), loc.portable, loc.notice.as_ref().map(|n| format!(" NOTICE: {n}")).unwrap_or_default())),
    );
    check("data folder writable", database::ensure_writable(&loc.dir).map(|_| "ok".into()).map_err(|e| e.to_string()));

    match database::open(&loc.dir.join("nexaterm.db")) {
        Ok(conn) => {
            check("database open + integrity", database::schema_version(&conn).map(|v| format!("schema v{v}")).map_err(|e| e.to_string()));
            let prev = repo::get_settings(&conn).ok().and_then(|s| s.get("selfTest.lastRun").cloned());
            let now = chrono::Local::now().to_rfc3339();
            check(
                "persistence across runs",
                repo::set_setting(&conn, "selfTest.lastRun", &serde_json::json!(now))
                    .map(|_| match prev {
                        Some(p) => format!("previous run found ({p}); stored {now}"),
                        None => format!("first run; stored {now}"),
                    })
                    .map_err(|e| e.to_string()),
            );
            let crud = (|| -> Result<String, String> {
                let input = SessionInput { name: "__nexaterm_self_test__".into(), protocol: "local".into(), ..Default::default() };
                let s = repo::insert_session(&conn, &input, None).map_err(|e| e.to_string())?;
                let back = repo::get_session(&conn, &s.id).map_err(|e| e.to_string())?;
                repo::delete_session(&conn, &s.id).map_err(|e| e.to_string())?;
                let total = repo::list_sessions(&conn).map_err(|e| e.to_string())?.len();
                Ok(format!("insert/read/delete ok ({}); {total} saved session(s)", back.name))
            })();
            check("session CRUD", crud);
        }
        Err(e) => check("database open + integrity", Err(e.to_string())),
    }

    #[cfg(windows)]
    {
        let store = crate::credentials::WindowsCredentialStore;
        let key = format!("NexaTerm-selftest/{}", uuid::Uuid::new_v4());
        let r = (|| -> Result<String, String> {
            store.set(&key, "self-test-secret").map_err(|e| e.to_string())?;
            let got = store.get(&key).map_err(|e| e.to_string())?;
            store.delete(&key).map_err(|e| e.to_string())?;
            if got.as_deref() == Some("self-test-secret") { Ok("write/read/delete ok".into()) } else { Err("value mismatch".into()) }
        })();
        check("Windows Credential Manager", r);
    }

    let shells = crate::terminal::shells::detect();
    check(
        "shell detection",
        if shells.is_empty() { Err("no shells found".into()) } else { Ok(shells.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")) },
    );
    check(if cfg!(windows) { "ConPTY local terminal" } else { "PTY local terminal" }, conpty_check());

    let log_path = loc.dir.join("logs").join("nexaterm.log");
    log::info!("self-test executed");
    log::logger().flush();
    check(
        "application log",
        std::fs::metadata(&log_path).map(|m| format!("{} ({} bytes)", log_path.display(), m.len())).map_err(|e| format!("{}: {e}", log_path.display())),
    );
    #[cfg(windows)]
    check("WebView2 runtime", webview2_version());

    let summary = if failures == 0 { "SELFTEST PASS".to_string() } else { format!("SELFTEST FAIL ({failures} failed)") };
    report = format!("NexaTerm {} self-test — {}\n{report}{summary}\n", env!("CARGO_PKG_VERSION"), chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    let _ = std::io::stdout().write_all(report.as_bytes());
    let _ = std::fs::write(loc.dir.join("logs").join("self-test.txt"), &report);
    if failures == 0 { 0 } else { 1 }
}

fn conpty_check() -> Result<String, String> {
    use crate::terminal::local::{LocalPty, LocalSpec};
    use crate::terminal::{TermSink, TerminalBackend};
    use std::sync::{mpsc, Arc, Mutex};
    #[cfg(windows)]
    let (program, args, cmd, expect, name) = (
        std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into()),
        vec!["/Q".to_string(), "/K".to_string()],
        &b"echo NEXATERM_%OS%_OK\r\n"[..],
        "NEXATERM_Windows_NT_OK",
        "cmd.exe",
    );
    #[cfg(not(windows))]
    let (program, args, cmd, expect, name) = ("/bin/sh".to_string(), Vec::<String>::new(), &b"echo NEXATERM_$((6*7))_OK\n"[..], "NEXATERM_42_OK", "/bin/sh");
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let tx = Mutex::new(tx);
    let sink = TermSink::new(move |d| { let _ = tx.lock().unwrap().send(d); }, |_| {});
    let pty: Arc<LocalPty> = LocalPty::spawn(LocalSpec { program, args, cwd: None, cols: 80, rows: 24 }, sink).map_err(|e| e.to_string())?;
    pty.write(cmd).map_err(|e| e.to_string())?;
    let mut out = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    let mut answered = false;
    while std::time::Instant::now() < deadline {
        if let Ok(d) = rx.recv_timeout(Duration::from_millis(200)) {
            out.push_str(&String::from_utf8_lossy(&d));
            if !answered && out.contains("\x1b[6n") {
                let _ = pty.write(b"\x1b[1;1R"); // answer ConPTY's cursor query like xterm.js does
                answered = true;
            }
            if out.contains(expect) {
                pty.close();
                return Ok(format!("{name} spawned, command executed, output received"));
            }
        }
    }
    pty.close();
    Err(format!("no output from {name} within 15 s"))
}

#[cfg(windows)]
fn webview2_version() -> Result<String, String> {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let keys: [(HKEY, windows::core::PCWSTR); 3] = [
        (HKEY_LOCAL_MACHINE, w!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}")),
        (HKEY_LOCAL_MACHINE, w!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}")),
        (HKEY_CURRENT_USER, w!(r"Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}")),
    ];
    for (root, sub) in keys {
        let mut buf = [0u16; 64];
        let mut len = (buf.len() * 2) as u32;
        let r = unsafe { RegGetValueW(root, sub, w!("pv"), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut len)) };
        if r.is_ok() {
            let s = String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]);
            if !s.is_empty() && s != "0.0.0.0" {
                return Ok(format!("installed, version {s}"));
            }
        }
    }
    Err("WebView2 Runtime not found — install it from https://developer.microsoft.com/microsoft-edge/webview2/".into())
}

