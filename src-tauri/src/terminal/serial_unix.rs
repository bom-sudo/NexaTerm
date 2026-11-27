//! Serial ports on Linux / Unix via termios (libc). Device paths such as /dev/ttyUSB0.
//!
//! พอร์ตอนุกรมบน Linux: เปิดอุปกรณ์แบบ exclusive, ตั้งค่า termios ตามที่ผู้ใช้เลือก แล้วอ่าน/เขียน
//! ใน thread แยกเพื่อไม่ให้ UI ค้าง

use super::{FlowControl, Parity, PortInfo, SerialSpec, StopBits};
use crate::error::{AppError, AppResult};
use crate::terminal::{TermEvent, TermSink, TerminalBackend};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

/// Accept only serial-like device paths so the value can never open an arbitrary file:
/// /dev/ttyS*, ttyUSB*, ttyACM*, ttyAMA*, ttyXRUSB*, rfcomm*, /dev/serial/by-id|by-path/*, /dev/pts/N.
pub fn validate_unix_port(path: &str) -> AppResult<String> {
    let p = path.trim();
    let ok = if let Some(rest) = p.strip_prefix("/dev/serial/by-id/").or_else(|| p.strip_prefix("/dev/serial/by-path/")) {
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || "._:+-".contains(c))
    } else if let Some(n) = p.strip_prefix("/dev/pts/") {
        !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
    } else if let Some(dev) = p.strip_prefix("/dev/") {
        ["ttyS", "ttyUSB", "ttyACM", "ttyAMA", "ttyXRUSB", "rfcomm"].iter().any(|pre| {
            dev.strip_prefix(pre).map(|n| !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit())).unwrap_or(false)
        })
    } else {
        false
    };
    if !ok {
        return Err(AppError::Validation(format!("Invalid serial device '{p}' (expected e.g. /dev/ttyUSB0 or /dev/ttyACM0)")));
    }
    Ok(p.to_string())
}

fn baud_constant(baud: u32) -> AppResult<libc::speed_t> {
    Ok(match baud {
        1200 => libc::B1200,
        2400 => libc::B2400,
        4800 => libc::B4800,
        9600 => libc::B9600,
        19200 => libc::B19200,
        38400 => libc::B38400,
        57600 => libc::B57600,
        115200 => libc::B115200,
        230400 => libc::B230400,
        #[cfg(target_os = "linux")]
        460800 => libc::B460800,
        #[cfg(target_os = "linux")]
        921600 => libc::B921600,
        other => return Err(AppError::Validation(format!("Unsupported speed {other} baud"))),
    })
}

/// Apply the spec onto a termios structure (raw mode). Pure, unit-tested.
pub fn configure(t: &mut libc::termios, spec: &SerialSpec) -> AppResult<()> {
    unsafe { libc::cfmakeraw(t) };
    t.c_cflag |= libc::CLOCAL | libc::CREAD;
    t.c_cflag &= !libc::CSIZE;
    t.c_cflag |= match spec.data_bits {
        5 => libc::CS5,
        6 => libc::CS6,
        7 => libc::CS7,
        8 => libc::CS8,
        n => return Err(AppError::Validation(format!("Data bits must be 5-8, not {n}"))),
    };
    t.c_cflag &= !(libc::PARENB | libc::PARODD);
    #[cfg(target_os = "linux")]
    {
        t.c_cflag &= !libc::CMSPAR;
    }
    match spec.parity {
        Parity::None => {}
        Parity::Odd => t.c_cflag |= libc::PARENB | libc::PARODD,
        Parity::Even => t.c_cflag |= libc::PARENB,
        #[cfg(target_os = "linux")]
        Parity::Mark => t.c_cflag |= libc::PARENB | libc::CMSPAR | libc::PARODD,
        #[cfg(target_os = "linux")]
        Parity::Space => t.c_cflag |= libc::PARENB | libc::CMSPAR,
        #[cfg(not(target_os = "linux"))]
        Parity::Mark | Parity::Space => return Err(AppError::Validation("Mark/space parity is not supported on this system".into())),
    }
    match spec.stop_bits {
        StopBits::One => t.c_cflag &= !libc::CSTOPB,
        StopBits::Two => t.c_cflag |= libc::CSTOPB,
        StopBits::OnePointFive => return Err(AppError::Validation("1.5 stop bits are not supported on Linux; use 1 or 2".into())),
    }
    t.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
    t.c_cflag &= !libc::CRTSCTS;
    match spec.flow {
        FlowControl::None => {}
        FlowControl::XonXoff => t.c_iflag |= libc::IXON | libc::IXOFF,
        FlowControl::RtsCts => t.c_cflag |= libc::CRTSCTS,
    }
    // Reads are driven by poll(); never block inside read().
    t.c_cc[libc::VMIN] = 0;
    t.c_cc[libc::VTIME] = 0;
    let speed = baud_constant(spec.baud)?;
    unsafe {
        libc::cfsetispeed(t, speed);
        libc::cfsetospeed(t, speed);
    }
    Ok(())
}

fn os_err(what: &str, path: &str) -> AppError {
    let e = std::io::Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::ENOENT) => AppError::NotFound(format!("serial device {path}")),
        Some(libc::EACCES) | Some(libc::EPERM) => AppError::Terminal(format!(
            "Permission denied for {path}. Add your user to the 'dialout' (or 'uucp') group and log in again."
        )),
        Some(libc::EBUSY) => AppError::Terminal(format!("{path} is in use by another program")),
        _ => AppError::Terminal(format!("{what} {path}: {e}")),
    }
}

enum Cmd {
    Data(Vec<u8>),
    Close,
}

pub struct SerialSession {
    tx: Mutex<Option<mpsc::Sender<Cmd>>>,
    stop: Arc<AtomicBool>,
}

impl SerialSession {
    pub fn open(spec: SerialSpec, sink: TermSink) -> AppResult<Arc<SerialSession>> {
        let path = validate_unix_port(&spec.port)?;
        let cpath = std::ffi::CString::new(path.clone()).map_err(|_| AppError::Validation("Invalid device path".into()))?;
        // O_NONBLOCK so open() does not wait for carrier detect; switched back to blocking below.
        let fd = unsafe { libc::open(cpath.as_ptr(), libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(os_err("Cannot open", &path));
        }
        let file = unsafe { File::from_raw_fd(fd) };
        unsafe {
            // Exclusive access (fails silently on devices that do not support it, e.g. some pty).
            libc::ioctl(fd, libc::TIOCEXCL as _);
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK);
        }
        let mut t: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut t) } != 0 {
            return Err(os_err("Not a serial device:", &path));
        }
        configure(&mut t, &spec)?;
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &t) } != 0 {
            return Err(os_err("Cannot configure", &path));
        }
        unsafe { libc::tcflush(fd, libc::TCIOFLUSH) };

        let stop = Arc::new(AtomicBool::new(false));
        let reader = file.try_clone()?;
        let mut writer = file;

        // Reader: poll with a short timeout so close() is honoured promptly.
        let rstop = stop.clone();
        let rsink = sink.clone();
        std::thread::Builder::new().name("serial-reader".into()).spawn(move || {
            let mut reader = reader;
            let mut buf = vec![0u8; 8192];
            let mut reason = None;
            while !rstop.load(Ordering::SeqCst) {
                let mut pfd = libc::pollfd { fd: reader.as_raw_fd(), events: libc::POLLIN, revents: 0 };
                let r = unsafe { libc::poll(&mut pfd, 1, 100) };
                if r < 0 {
                    if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    reason = Some("Serial device error".to_string());
                    break;
                }
                if r == 0 {
                    continue;
                }
                if pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 && pfd.revents & libc::POLLIN == 0 {
                    reason = Some("Serial device was disconnected".to_string());
                    break;
                }
                match reader.read(&mut buf) {
                    Ok(0) => continue,
                    Ok(n) => rsink.data(buf[..n].to_vec()),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted || e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(e) => {
                        reason = Some(format!("Serial read failed: {e}"));
                        break;
                    }
                }
            }
            rstop.store(true, Ordering::SeqCst);
            rsink.event(TermEvent::Exit { code: None, reason });
        })?;

        // Writer: owns the write half; exits on Close or when the channel is dropped.
        let (tx, rx) = mpsc::channel::<Cmd>();
        let wstop = stop.clone();
        std::thread::Builder::new().name("serial-writer".into()).spawn(move || {
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    Cmd::Data(d) => {
                        if writer.write_all(&d).and_then(|_| writer.flush()).is_err() {
                            break;
                        }
                    }
                    Cmd::Close => break,
                }
            }
            wstop.store(true, Ordering::SeqCst);
        })?;

        Ok(Arc::new(SerialSession { tx: Mutex::new(Some(tx)), stop }))
    }
}

impl TerminalBackend for SerialSession {
    fn write(&self, data: &[u8]) -> AppResult<()> {
        let g = self.tx.lock().unwrap();
        let tx = g.as_ref().ok_or_else(|| AppError::Terminal("Serial port is closed".into()))?;
        tx.send(Cmd::Data(data.to_vec())).map_err(|_| AppError::Terminal("Serial port is closed".into()))
    }
    fn resize(&self, _: u16, _: u16) -> AppResult<()> {
        Ok(())
    }
    fn close(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(tx) = self.tx.lock().unwrap().take() {
            let _ = tx.send(Cmd::Close);
        }
    }
}

/// Serial devices that really exist: /sys/class/tty entries with a backing device, plus the
/// stable /dev/serial/by-id names as descriptions.
pub fn list_ports() -> Vec<PortInfo> {
    let mut by_id = std::collections::HashMap::new();
    if let Ok(rd) = std::fs::read_dir("/dev/serial/by-id") {
        for e in rd.flatten() {
            if let Ok(target) = std::fs::canonicalize(e.path()) {
                by_id.insert(target.display().to_string(), e.file_name().to_string_lossy().to_string());
            }
        }
    }
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir("/sys/class/tty") {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !e.path().join("device").exists() {
                continue; // virtual consoles and ptys have no device
            }
            let dev = format!("/dev/{name}");
            if validate_unix_port(&dev).is_ok() {
                out.push(PortInfo { description: by_id.get(&dev).cloned(), name: dev });
            }
        }
    }
    super::sort_ports(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(port: &str) -> SerialSpec {
        SerialSpec { port: port.into(), baud: 9600, data_bits: 8, parity: Parity::None, stop_bits: StopBits::One, flow: FlowControl::None }
    }

    #[test]
    fn device_path_validation() {
        for ok in ["/dev/ttyUSB0", "/dev/ttyACM12", "/dev/ttyS3", "/dev/ttyAMA0", "/dev/pts/7", "/dev/serial/by-id/usb-FTDI_FT232R-if00-port0"] {
            assert!(validate_unix_port(ok).is_ok(), "{ok}");
        }
        for bad in ["/etc/passwd", "/dev/sda", "/dev/ttyUSB", "/dev/../etc/shadow", "COM1", "/dev/serial/by-id/../x", "/dev/pts/a"] {
            assert!(validate_unix_port(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn termios_mapping() {
        let mut t: libc::termios = unsafe { std::mem::zeroed() };
        let mut s = spec("/dev/ttyUSB0");
        s.data_bits = 7;
        s.parity = Parity::Even;
        s.stop_bits = StopBits::Two;
        s.flow = FlowControl::RtsCts;
        s.baud = 115200;
        configure(&mut t, &s).unwrap();
        assert_eq!(t.c_cflag & libc::CSIZE, libc::CS7);
        assert_ne!(t.c_cflag & libc::PARENB, 0);
        assert_eq!(t.c_cflag & libc::PARODD, 0);
        assert_ne!(t.c_cflag & libc::CSTOPB, 0);
        assert_ne!(t.c_cflag & libc::CRTSCTS, 0);
        assert_eq!(unsafe { libc::cfgetospeed(&t) }, libc::B115200);
        s.stop_bits = StopBits::OnePointFive;
        assert!(configure(&mut t, &s).is_err());
        let mut s2 = spec("/dev/ttyUSB0");
        s2.baud = 1234;
        assert!(configure(&mut t, &s2).is_err());
    }

    /// End-to-end through a pseudo-terminal pair: the slave side stands in for a serial device.
    #[test]
    fn pty_roundtrip() {
        let (mut master, mut slave) = (0, 0);
        assert_eq!(unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), std::ptr::null()) }, 0);
        let name = unsafe { std::ffi::CStr::from_ptr(libc::ttyname(slave)) }.to_string_lossy().to_string();
        let mut master = unsafe { File::from_raw_fd(master) };
        let (dtx, drx) = mpsc::channel::<Vec<u8>>();
        let (etx, erx) = mpsc::channel::<TermEvent>();
        let dtx = Mutex::new(dtx);
        let etx = Mutex::new(etx);
        let sink = TermSink::new(move |d| { let _ = dtx.lock().unwrap().send(d); }, move |e| { let _ = etx.lock().unwrap().send(e); });
        let session = SerialSession::open(spec(&name), sink).expect("open pty slave as serial port");
        unsafe { libc::close(slave) };

        master.write_all(b"hello from device\r\n").unwrap();
        let mut got = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !got.ends_with(b"\r\n") && std::time::Instant::now() < deadline {
            if let Ok(d) = drx.recv_timeout(std::time::Duration::from_millis(200)) {
                got.extend(d);
            }
        }
        assert_eq!(got, b"hello from device\r\n");

        session.write(b"AT\r").unwrap();
        let mut buf = [0u8; 16];
        let mut n = 0;
        while n < 3 {
            n += master.read(&mut buf[n..]).unwrap();
        }
        assert_eq!(&buf[..3], b"AT\r");

        session.close();
        assert!(matches!(erx.recv_timeout(std::time::Duration::from_secs(3)), Ok(TermEvent::Exit { .. })));
        assert!(session.write(b"x").is_err());
    }
}
