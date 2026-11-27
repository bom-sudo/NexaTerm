//! X11 forwarding plumbing, the way OpenSSH does it: the server only ever sees a random fake
//! MIT-MAGIC-COOKIE-1; each forwarded connection must present that fake cookie, and it is
//! replaced with the local display's real cookie (if any) before reaching the X server.
//!
//! X11 forwarding: เซิร์ฟเวอร์ปลายทางจะไม่เห็นคุกกี้จริงของจอเครื่องผู้ใช้

use crate::error::{AppError, AppResult};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const COOKIE_PROTO: &str = "MIT-MAGIC-COOKIE-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X11Target {
    Tcp(String, u16),
    #[cfg(unix)]
    Unix(std::path::PathBuf),
}

pub struct X11Forward {
    pub target: X11Target,
    pub fake_cookie: [u8; 16],
    /// The local display's real cookie, from `xauth`; None when the X server needs none.
    pub real_cookie: Option<Vec<u8>>,
}

impl X11Forward {
    pub fn new(display: Option<&str>) -> AppResult<Self> {
        let (display, target) = parse_display(display)?;
        Ok(Self { target, fake_cookie: rand::random(), real_cookie: real_cookie(&display) })
    }
    pub fn fake_cookie_hex(&self) -> String {
        self.fake_cookie.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// The display to use when the session does not name one.
fn default_display() -> String {
    #[cfg(unix)]
    if let Ok(d) = std::env::var("DISPLAY") {
        if !d.trim().is_empty() {
            return d;
        }
    }
    #[cfg(unix)]
    return ":0".into();
    #[cfg(not(unix))]
    return "localhost:0".into();
}

/// Parse an X display ("localhost:0", ":1", "127.0.0.1:10.0", "unix:0") into a connect target.
/// On Unix a display without a host is the local socket /tmp/.X11-unix/X<n>; on Windows it is TCP.
pub fn parse_display(display: Option<&str>) -> AppResult<(String, X11Target)> {
    let d = display.map(str::trim).filter(|d| !d.is_empty()).map(String::from).unwrap_or_else(default_display);
    let (host, rest) = d.rsplit_once(':').ok_or_else(|| AppError::Validation(format!("Invalid X display '{d}' (expected host:number)")))?;
    let num: u16 = rest.split('.').next().unwrap_or("").parse().map_err(|_| AppError::Validation(format!("Invalid X display number in '{d}'")))?;
    if num > 99 {
        return Err(AppError::Validation("X display number must be 0-99".into()));
    }
    #[cfg(unix)]
    if host.is_empty() || host == "unix" {
        return Ok((d.clone(), X11Target::Unix(format!("/tmp/.X11-unix/X{num}").into())));
    }
    let host = if host.is_empty() { "127.0.0.1".to_string() } else { crate::security::validate_host(host)? };
    Ok((d.clone(), X11Target::Tcp(host, 6000 + num)))
}

/// Real cookie for the display via `xauth list` (Unix only; Windows X servers run with -ac).
fn real_cookie(display: &str) -> Option<Vec<u8>> {
    #[cfg(unix)]
    {
        let out = std::process::Command::new("xauth").args(["list", display]).output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let line = text.lines().find(|l| l.contains(COOKIE_PROTO))?;
        let hex = line.split_whitespace().last()?;
        return decode_hex(hex);
    }
    #[cfg(not(unix))]
    {
        let _ = display;
        None
    }
}

#[cfg_attr(not(unix), allow(dead_code))]
fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

fn pad4(n: usize) -> usize {
    (n + 3) & !3
}

/// Read the client's X11 connection setup from `remote`, check the fake cookie, and return the
/// rewritten setup bytes carrying the real cookie (or no auth) for the local X server.
pub async fn rewrite_setup<R: AsyncRead + Unpin>(remote: &mut R, fake: &[u8], real: Option<&[u8]>) -> AppResult<Vec<u8>> {
    let mut hdr = [0u8; 12];
    remote.read_exact(&mut hdr).await?;
    let le = match hdr[0] {
        b'l' => true,
        b'B' => false,
        _ => return Err(AppError::Connection("X11: bad byte order in setup".into())),
    };
    let rd = |a: u8, b: u8| if le { u16::from_le_bytes([a, b]) } else { u16::from_be_bytes([a, b]) };
    let name_len = rd(hdr[6], hdr[7]) as usize;
    let data_len = rd(hdr[8], hdr[9]) as usize;
    let mut body = vec![0u8; pad4(name_len) + pad4(data_len)];
    remote.read_exact(&mut body).await?;
    let name = &body[..name_len];
    let data = &body[pad4(name_len)..pad4(name_len) + data_len];
    if name != COOKIE_PROTO.as_bytes() || data != fake {
        return Err(AppError::Connection("X11: connection rejected (wrong authentication cookie)".into()));
    }
    let (new_name, new_data): (&[u8], &[u8]) = match real {
        Some(c) => (COOKIE_PROTO.as_bytes(), c),
        None => (&[], &[]),
    };
    let wr = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let mut out = hdr.to_vec();
    out[6..8].copy_from_slice(&wr(new_name.len() as u16));
    out[8..10].copy_from_slice(&wr(new_data.len() as u16));
    out.extend_from_slice(new_name);
    out.resize(12 + pad4(new_name.len()), 0);
    out.extend_from_slice(new_data);
    out.resize(12 + pad4(new_name.len()) + pad4(new_data.len()), 0);
    Ok(out)
}

/// Connect a forwarded X11 channel to the local display.
pub async fn proxy<S>(fwd: &X11Forward, mut remote: S) -> AppResult<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let setup = rewrite_setup(&mut remote, &fwd.fake_cookie, fwd.real_cookie.as_deref()).await?;
    match &fwd.target {
        X11Target::Tcp(host, port) => {
            let mut local = tokio::net::TcpStream::connect((host.as_str(), *port)).await?;
            local.write_all(&setup).await?;
            tokio::io::copy_bidirectional(&mut local, &mut remote).await?;
        }
        #[cfg(unix)]
        X11Target::Unix(path) => {
            let mut local = tokio::net::UnixStream::connect(path).await?;
            local.write_all(&setup).await?;
            tokio::io::copy_bidirectional(&mut local, &mut remote).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(le: bool, name: &[u8], data: &[u8]) -> Vec<u8> {
        let w = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut v = vec![if le { b'l' } else { b'B' }, 0];
        v.extend(w(11));
        v.extend(w(0));
        v.extend(w(name.len() as u16));
        v.extend(w(data.len() as u16));
        v.extend([0, 0]);
        v.extend(name);
        v.resize(12 + pad4(name.len()), 0);
        v.extend(data);
        v.resize(12 + pad4(name.len()) + pad4(data.len()), 0);
        v
    }

    #[tokio::test]
    async fn cookie_is_replaced_or_rejected() {
        let fake = [7u8; 16];
        let real = [9u8; 16];
        for le in [true, false] {
            let input = setup(le, COOKIE_PROTO.as_bytes(), &fake);
            let out = rewrite_setup(&mut input.as_slice(), &fake, Some(&real)).await.unwrap();
            assert_eq!(out, setup(le, COOKIE_PROTO.as_bytes(), &real));
            let out = rewrite_setup(&mut input.as_slice(), &fake, None).await.unwrap();
            assert_eq!(out, setup(le, b"", b""));
        }
        let wrong = setup(true, COOKIE_PROTO.as_bytes(), &[1u8; 16]);
        assert!(rewrite_setup(&mut wrong.as_slice(), &fake, Some(&real)).await.is_err());
        let none = setup(true, b"", b"");
        assert!(rewrite_setup(&mut none.as_slice(), &fake, None).await.is_err());
    }

    #[test]
    fn display_parsing() {
        assert_eq!(parse_display(Some("localhost:10.0")).unwrap().1, X11Target::Tcp("localhost".into(), 6010));
        assert_eq!(parse_display(Some("127.0.0.1:1")).unwrap().1, X11Target::Tcp("127.0.0.1".into(), 6001));
        #[cfg(unix)]
        assert_eq!(parse_display(Some(":1")).unwrap().1, X11Target::Unix("/tmp/.X11-unix/X1".into()));
        #[cfg(windows)]
        assert_eq!(parse_display(Some(":1")).unwrap().1, X11Target::Tcp("127.0.0.1".into(), 6001));
        assert!(parse_display(Some("nohost")).is_err());
        assert!(parse_display(Some(":100")).is_err());
        assert_eq!(decode_hex("0aff"), Some(vec![0x0a, 0xff]));
    }
}
