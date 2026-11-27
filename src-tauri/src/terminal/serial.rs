//! Serial (COM port) terminal backend for Windows, implemented directly on Win32
//! (CreateFileW / DCB / COMMTIMEOUTS) through the `windows` crate.
//!
//! Threading model: the port is opened for synchronous I/O. One reader thread loops on
//! `ReadFile`, which returns within ~50 ms when the line is idle (see [`read_timeouts`]), and one
//! writer thread drains an mpsc channel, so `write` never blocks the caller. Synchronous I/O on a
//! single handle is serialized by the I/O manager, so a write may wait for the current read to
//! return (bounded by the 50 ms read timeout). `close` sets a stop flag, closes the channel and
//! calls `CancelIoEx` so both threads exit promptly; the handle is closed when the last thread
//! releases it.
//!
//! Settings validation and DCB field computation are pure functions ([`validate_port_name`],
//! [`dcb_fields`]) so they can be unit-tested on machines without serial hardware.

use super::{TermSink, TerminalBackend};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Parity {
    #[default]
    None,
    Odd,
    Even,
    Mark,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum StopBits {
    /// "one"
    #[default]
    One,
    /// "onePointFive"
    OnePointFive,
    /// "two"
    Two,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum FlowControl {
    /// "none"
    #[default]
    None,
    /// "xonXoff"
    XonXoff,
    /// "rtsCts"
    RtsCts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialSpec {
    /// e.g. "COM3"
    pub port: String,
    pub baud: u32,
    /// 5-8
    pub data_bits: u8,
    pub parity: Parity,
    pub stop_bits: StopBits,
    pub flow: FlowControl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub name: String,
    pub description: Option<String>,
}

pub const MIN_BAUD: u32 = 50;
pub const MAX_BAUD: u32 = 12_000_000;
/// Driver queue sizes requested with SetupComm (advisory; some USB drivers ignore it).
pub const QUEUE_SIZE: u32 = 4096;
pub const XON_CHAR: u8 = 0x11;
pub const XOFF_CHAR: u8 = 0x13;

// Win32 DCB constants (winbase.h), duplicated here so the pure mapping compiles on every target.
pub const DTR_CONTROL_ENABLE: u32 = 1;
pub const RTS_CONTROL_ENABLE: u32 = 1;
pub const RTS_CONTROL_HANDSHAKE: u32 = 2;

// DCB bitfield layout (winbase.h).
pub const F_BINARY: u32 = 1 << 0;
pub const F_PARITY: u32 = 1 << 1;
pub const F_OUTX_CTS_FLOW: u32 = 1 << 2;
pub const F_OUTX_DSR_FLOW: u32 = 1 << 3;
pub const F_DTR_CONTROL_SHIFT: u32 = 4;
pub const F_DSR_SENSITIVITY: u32 = 1 << 6;
pub const F_TX_CONTINUE_ON_XOFF: u32 = 1 << 7;
pub const F_OUTX: u32 = 1 << 8;
pub const F_INX: u32 = 1 << 9;
pub const F_ERROR_CHAR: u32 = 1 << 10;
pub const F_NULL: u32 = 1 << 11;
pub const F_RTS_CONTROL_SHIFT: u32 = 12;
pub const F_ABORT_ON_ERROR: u32 = 1 << 14;

/// Validate a COM port name (`^COM[1-9][0-9]{0,2}$`, case-insensitive) and return it uppercased.
/// Anything else (device paths, `..`, `NUL`, other DOS devices) is rejected so the value can never
/// address something other than a serial port.
pub fn validate_port_name(name: &str) -> AppResult<String> {
    let n = name.trim().to_ascii_uppercase();
    let digits = n.strip_prefix("COM").unwrap_or("");
    let ok = n.starts_with("COM")
        && (1..=3).contains(&digits.len())
        && digits.bytes().all(|b| b.is_ascii_digit())
        && !digits.starts_with('0');
    if !ok {
        return Err(AppError::Validation(format!("Invalid serial port name '{}' (expected COM1-COM999)", name.trim())));
    }
    Ok(n)
}

/// Win32 device path for a validated port name (required for COM10 and above).
pub fn device_path(port: &str) -> AppResult<String> {
    Ok(format!(r"\\.\{}", validate_port_name(port)?))
}

/// DCB values derived from a [`SerialSpec`]; applied onto the driver's current DCB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DcbFields {
    pub baud_rate: u32,
    pub byte_size: u8,
    /// NOPARITY=0, ODDPARITY=1, EVENPARITY=2, MARKPARITY=3, SPACEPARITY=4
    pub parity: u8,
    /// ONESTOPBIT=0, ONE5STOPBITS=1, TWOSTOPBITS=2
    pub stop_bits: u8,
    pub bitfield: u32,
    pub xon_lim: u16,
    pub xoff_lim: u16,
    pub xon_char: u8,
    pub xoff_char: u8,
}

/// Validate the line settings and compute the DCB fields (pure).
pub fn dcb_fields(spec: &SerialSpec) -> AppResult<DcbFields> {
    if !(MIN_BAUD..=MAX_BAUD).contains(&spec.baud) {
        return Err(AppError::Validation(format!("Baud rate must be between {MIN_BAUD} and {MAX_BAUD}")));
    }
    if !(5..=8).contains(&spec.data_bits) {
        return Err(AppError::Validation("Data bits must be 5, 6, 7 or 8".into()));
    }
    // ข้อจำกัดของ Windows: 1.5 stop bits ใช้ได้กับ 5 data bits เท่านั้น และ 5 data bits ใช้ 2 stop bits ไม่ได้
    match (spec.data_bits, spec.stop_bits) {
        (5, StopBits::Two) => {
            return Err(AppError::Validation("2 stop bits cannot be used with 5 data bits (use 1.5)".into()))
        }
        (6..=8, StopBits::OnePointFive) => {
            return Err(AppError::Validation("1.5 stop bits can only be used with 5 data bits".into()))
        }
        _ => {}
    }
    let parity = match spec.parity {
        Parity::None => 0,
        Parity::Odd => 1,
        Parity::Even => 2,
        Parity::Mark => 3,
        Parity::Space => 4,
    };
    let stop_bits = match spec.stop_bits {
        StopBits::One => 0,
        StopBits::OnePointFive => 1,
        StopBits::Two => 2,
    };

    // Binary mode is mandatory on Windows; DTR is asserted (many devices need it to talk) and
    // transmission continues after we send XOFF. Errors never abort I/O.
    let mut bits = F_BINARY | (DTR_CONTROL_ENABLE << F_DTR_CONTROL_SHIFT) | F_TX_CONTINUE_ON_XOFF;
    if spec.parity != Parity::None {
        bits |= F_PARITY;
    }
    match spec.flow {
        FlowControl::None => bits |= RTS_CONTROL_ENABLE << F_RTS_CONTROL_SHIFT,
        FlowControl::XonXoff => bits |= F_OUTX | F_INX | (RTS_CONTROL_ENABLE << F_RTS_CONTROL_SHIFT),
        FlowControl::RtsCts => bits |= F_OUTX_CTS_FLOW | (RTS_CONTROL_HANDSHAKE << F_RTS_CONTROL_SHIFT),
    }

    Ok(DcbFields {
        baud_rate: spec.baud,
        byte_size: spec.data_bits,
        parity,
        stop_bits,
        bitfield: bits,
        // Send XON when the input queue drains below 1/4, XOFF when fewer than 1/4 bytes are free.
        xon_lim: (QUEUE_SIZE / 4) as u16,
        xoff_lim: (QUEUE_SIZE / 4) as u16,
        xon_char: XON_CHAR,
        xoff_char: XOFF_CHAR,
    })
}

/// Read timeouts (ReadIntervalTimeout, ReadTotalTimeoutMultiplier, ReadTotalTimeoutConstant):
/// MAXDWORD/MAXDWORD/50 makes ReadFile return immediately with whatever is buffered, wait at most
/// 50 ms for the first byte when idle, and return as soon as one byte arrives.
pub const fn read_timeouts() -> (u32, u32, u32) {
    (u32::MAX, u32::MAX, 50)
}
/// Write timeouts (multiplier, constant): a write stuck behind flow control fails after 5 s
/// instead of hanging the writer thread forever.
pub const fn write_timeouts() -> (u32, u32) {
    (0, 5000)
}

/// Natural sort key: "COM2" < "COM10".
fn natural_key(name: &str) -> (String, u64, String) {
    let upper = name.to_ascii_uppercase();
    let split = upper.find(|c: char| c.is_ascii_digit()).unwrap_or(upper.len());
    let (prefix, rest) = upper.split_at(split);
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    let num = rest[..end].parse::<u64>().unwrap_or(0);
    (prefix.to_string(), num, rest[end..].to_string())
}

/// Sort ports naturally and drop duplicate names (pure; used by [`list_ports`]).
pub fn sort_ports(mut ports: Vec<PortInfo>) -> Vec<PortInfo> {
    ports.sort_by_key(|p| natural_key(&p.name));
    ports.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    ports
}

#[cfg(windows)]
pub use win::{list_ports, SerialSession};

#[cfg(not(windows))]
pub fn list_ports() -> Vec<PortInfo> {
    Vec::new()
}

#[cfg(not(windows))]
pub struct SerialSession;

#[cfg(not(windows))]
impl SerialSession {
    pub fn open(spec: SerialSpec, _sink: TermSink) -> AppResult<Arc<SerialSession>> {
        validate_port_name(&spec.port)?;
        dcb_fields(&spec)?;
        Err(AppError::Terminal("Serial ports are only supported on Windows".into()))
    }
}

#[cfg(not(windows))]
impl TerminalBackend for SerialSession {
    fn write(&self, _: &[u8]) -> AppResult<()> {
        Err(AppError::Terminal("Serial port is closed".into()))
    }
    fn resize(&self, _: u16, _: u16) -> AppResult<()> {
        Ok(())
    }
    fn close(&self) {}
}

#[cfg(windows)]
mod win {
    use super::*;
    use crate::terminal::TermEvent;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Mutex};
    use windows::core::{w, HSTRING, PWSTR};
    use windows::Win32::Devices::Communication::{
        GetCommState, PurgeComm, SetCommState, SetCommTimeouts, SetupComm, COMMTIMEOUTS, DCB, DCB_PARITY,
        DCB_STOP_BITS, PURGE_RXABORT, PURGE_RXCLEAR, PURGE_TXABORT, PURGE_TXCLEAR,
    };
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND,
        ERROR_SHARING_VIOLATION, ERROR_SUCCESS, GENERIC_READ, GENERIC_WRITE, HANDLE,
    };
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, ReadFile, WriteFile, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_NONE, OPEN_EXISTING,
    };
    use windows::Win32::System::Registry::{
        RegCloseKey, RegEnumValueW, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ,
    };
    use windows::Win32::System::IO::CancelIoEx;

    /// Owned COM port handle; closed on drop (after the last thread releases it).
    struct PortHandle(HANDLE);
    // SAFETY: a Win32 file handle may be used from any thread; Win32 serializes synchronous I/O.
    unsafe impl Send for PortHandle {}
    unsafe impl Sync for PortHandle {}
    impl Drop for PortHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    impl PortHandle {
        fn cancel_io(&self) {
            unsafe {
                let _ = CancelIoEx(self.0, None);
            }
        }
    }

    pub struct SerialSession {
        handle: Mutex<Option<Arc<PortHandle>>>,
        tx: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
        stop: Arc<AtomicBool>,
    }

    fn open_error(port: &str, e: &windows::core::Error) -> AppError {
        let code = e.code();
        if code == ERROR_FILE_NOT_FOUND.to_hresult() || code == ERROR_PATH_NOT_FOUND.to_hresult() {
            // ไม่พบพอร์ต: อุปกรณ์อาจถูกถอดออกหรือยังไม่ได้ติดตั้งไดรเวอร์
            AppError::NotFound(format!("serial port {port} (is the device connected?)"))
        } else if code == ERROR_ACCESS_DENIED.to_hresult() || code == ERROR_SHARING_VIOLATION.to_hresult() {
            // พอร์ตถูกโปรแกรมอื่นเปิดใช้งานอยู่
            AppError::Connection(format!("{port} is in use by another program"))
        } else {
            AppError::Connection(format!("Cannot open {port}: {}", e.message()))
        }
    }

    fn apply_fields(dcb: &mut DCB, f: &DcbFields) {
        dcb.DCBlength = std::mem::size_of::<DCB>() as u32;
        dcb.BaudRate = f.baud_rate;
        dcb.ByteSize = f.byte_size;
        dcb.Parity = DCB_PARITY(f.parity);
        dcb.StopBits = DCB_STOP_BITS(f.stop_bits);
        // Keep the reserved high bits (fDummy2) from the driver, replace the 15 defined flag bits.
        dcb._bitfield = (dcb._bitfield & !0x7FFF) | f.bitfield;
        dcb.XonLim = f.xon_lim;
        dcb.XoffLim = f.xoff_lim;
        dcb.XonChar = f.xon_char as i8;
        dcb.XoffChar = f.xoff_char as i8;
    }

    fn configure(h: HANDLE, port: &str, f: &DcbFields) -> AppResult<()> {
        unsafe {
            let _ = SetupComm(h, QUEUE_SIZE, QUEUE_SIZE);
            let mut dcb = DCB { DCBlength: std::mem::size_of::<DCB>() as u32, ..Default::default() };
            GetCommState(h, &mut dcb)
                .map_err(|e| AppError::Connection(format!("{port} is not a usable serial port: {}", e.message())))?;
            apply_fields(&mut dcb, f);
            SetCommState(h, &dcb).map_err(|e| {
                AppError::Validation(format!("{port} does not support these settings (baud/data/parity/stop): {}", e.message()))
            })?;
            let (ri, rm, rc) = read_timeouts();
            let (wm, wc) = write_timeouts();
            let t = COMMTIMEOUTS {
                ReadIntervalTimeout: ri,
                ReadTotalTimeoutMultiplier: rm,
                ReadTotalTimeoutConstant: rc,
                WriteTotalTimeoutMultiplier: wm,
                WriteTotalTimeoutConstant: wc,
            };
            SetCommTimeouts(h, &t).map_err(|e| AppError::Connection(format!("Cannot set timeouts on {port}: {}", e.message())))?;
            let _ = PurgeComm(h, PURGE_RXABORT | PURGE_RXCLEAR | PURGE_TXABORT | PURGE_TXCLEAR);
        }
        Ok(())
    }

    impl SerialSession {
        pub fn open(spec: SerialSpec, sink: TermSink) -> AppResult<Arc<SerialSession>> {
            let port = validate_port_name(&spec.port)?;
            let fields = dcb_fields(&spec)?;
            let path = HSTRING::from(device_path(&port)?);
            let raw = unsafe {
                CreateFileW(
                    &path,
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            }
            .map_err(|e| open_error(&port, &e))?;
            let handle = Arc::new(PortHandle(raw)); // closes on error paths below
            configure(handle.0, &port, &fields)?;

            let stop = Arc::new(AtomicBool::new(false));
            let (tx, rx) = mpsc::channel::<Vec<u8>>();

            let rh = handle.clone();
            let rstop = stop.clone();
            let rsink = sink.clone();
            std::thread::Builder::new()
                .name(format!("serial-reader-{port}"))
                .spawn(move || {
                    let mut buf = vec![0u8; 4096];
                    let reason = loop {
                        if rstop.load(Ordering::SeqCst) {
                            break None;
                        }
                        let mut n = 0u32;
                        match unsafe { ReadFile(rh.0, Some(&mut buf), Some(&mut n), None) } {
                            Ok(()) => {
                                if n > 0 {
                                    rsink.data(buf[..n as usize].to_vec());
                                }
                            }
                            Err(_) if rstop.load(Ordering::SeqCst) => break None,
                            // อุปกรณ์ถูกถอดออกหรือไดรเวอร์รายงานข้อผิดพลาด
                            Err(_) => break Some("Serial port disconnected".to_string()),
                        }
                    };
                    if reason.is_some() {
                        // Unblock and stop the writer as well.
                        rstop.store(true, Ordering::SeqCst);
                    }
                    drop(rh);
                    rsink.event(TermEvent::Exit { code: None, reason });
                })
                .map_err(|e| AppError::Terminal(e.to_string()))?;

            let wh = handle.clone();
            let wstop = stop.clone();
            let wsink = sink;
            std::thread::Builder::new()
                .name(format!("serial-writer-{port}"))
                .spawn(move || {
                    while let Ok(data) = rx.recv() {
                        if wstop.load(Ordering::SeqCst) {
                            break;
                        }
                        let mut off = 0usize;
                        while off < data.len() {
                            let mut n = 0u32;
                            let r = unsafe { WriteFile(wh.0, Some(&data[off..]), Some(&mut n), None) };
                            if r.is_err() || n == 0 {
                                if !wstop.load(Ordering::SeqCst) {
                                    // ส่งข้อมูลไม่สำเร็จ (เช่น flow control ค้างนานเกินไป)
                                    wsink.event(TermEvent::Status {
                                        state: "error".into(),
                                        message: Some("Serial write failed or timed out".into()),
                                    });
                                }
                                break;
                            }
                            off += n as usize;
                        }
                    }
                    drop(wh);
                })
                .map_err(|e| AppError::Terminal(e.to_string()))?;

            Ok(Arc::new(SerialSession { handle: Mutex::new(Some(handle)), tx: Mutex::new(Some(tx)), stop }))
        }
    }

    impl TerminalBackend for SerialSession {
        fn write(&self, data: &[u8]) -> AppResult<()> {
            let g = self.tx.lock().unwrap();
            let tx = g.as_ref().ok_or_else(|| AppError::Terminal("Serial port is closed".into()))?;
            tx.send(data.to_vec()).map_err(|_| AppError::Terminal("Serial port is closed".into()))
        }
        fn resize(&self, _cols: u16, _rows: u16) -> AppResult<()> {
            Ok(()) // a serial line has no window size
        }
        fn close(&self) {
            self.stop.store(true, Ordering::SeqCst);
            self.tx.lock().unwrap().take(); // writer thread exits once the channel is empty/closed
            if let Some(h) = self.handle.lock().unwrap().take() {
                h.cancel_io(); // abort an in-flight ReadFile / WriteFile right away
            }
        }
    }

    impl Drop for SerialSession {
        fn drop(&mut self) {
            self.close();
        }
    }

    /// Enumerate COM ports from `HKLM\HARDWARE\DEVICEMAP\SERIALCOMM` (value name = kernel device,
    /// data = "COMn"). Returns an empty list if the key does not exist (no ports present).
    pub fn list_ports() -> Vec<PortInfo> {
        let mut out = Vec::new();
        unsafe {
            let mut key = HKEY::default();
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, w!(r"HARDWARE\DEVICEMAP\SERIALCOMM"), None, KEY_READ, &mut key) != ERROR_SUCCESS {
                return out;
            }
            let mut index = 0u32;
            loop {
                let mut name = [0u16; 512];
                let mut name_len = name.len() as u32;
                let mut data = [0u16; 256];
                let mut data_len = (data.len() * 2) as u32;
                let mut kind = 0u32;
                let r = RegEnumValueW(
                    key,
                    index,
                    Some(PWSTR(name.as_mut_ptr())),
                    &mut name_len,
                    None,
                    Some(&mut kind),
                    Some(data.as_mut_ptr() as *mut u8),
                    Some(&mut data_len),
                );
                index += 1;
                if r == ERROR_NO_MORE_ITEMS {
                    break;
                }
                if r != ERROR_SUCCESS {
                    // Skip oversized / unreadable values; stop after an implausible number of them.
                    if index > 1024 {
                        break;
                    }
                    continue;
                }
                if kind != REG_SZ.0 {
                    continue;
                }
                let chars = (data_len as usize / 2).min(data.len());
                let value = String::from_utf16_lossy(&data[..chars]);
                let value = value.trim_end_matches('\0').trim();
                let Ok(port) = validate_port_name(value) else { continue };
                let device = String::from_utf16_lossy(&name[..(name_len as usize).min(name.len())]);
                let description = if device.is_empty() { None } else { Some(device) };
                out.push(PortInfo { name: port, description });
            }
            let _ = RegCloseKey(key);
        }
        sort_ports(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn dcb_mapping_onto_win32_struct() {
            let spec = SerialSpec {
                port: "COM1".into(),
                baud: 9600,
                data_bits: 7,
                parity: Parity::Even,
                stop_bits: StopBits::Two,
                flow: FlowControl::RtsCts,
            };
            let f = dcb_fields(&spec).unwrap();
            // Simulate a driver DCB with reserved high bits set and stale flags.
            let mut dcb = DCB { _bitfield: 0xFFFF_FFFF, ..Default::default() };
            apply_fields(&mut dcb, &f);
            assert_eq!(dcb.DCBlength as usize, std::mem::size_of::<DCB>());
            assert_eq!(dcb.BaudRate, 9600);
            assert_eq!(dcb.ByteSize, 7);
            assert_eq!(dcb.Parity, windows::Win32::Devices::Communication::EVENPARITY);
            assert_eq!(dcb.StopBits, windows::Win32::Devices::Communication::TWOSTOPBITS);
            assert_eq!(dcb._bitfield & 0x7FFF, f.bitfield);
            assert_eq!(dcb._bitfield & !0x7FFF, 0xFFFF_8000);
            assert_eq!(dcb.XonChar, 0x11);
            assert_eq!(dcb.XoffChar, 0x13);
        }

        #[test]
        fn open_nonexistent_port_is_not_found() {
            let spec = SerialSpec {
                port: "COM987".into(),
                baud: 115200,
                data_bits: 8,
                parity: Parity::None,
                stop_bits: StopBits::One,
                flow: FlowControl::None,
            };
            let sink = TermSink::new(|_| {}, |_| {});
            match SerialSession::open(spec, sink) {
                Err(AppError::NotFound(m)) => assert!(m.contains("COM987")),
                Err(e) => panic!("unexpected error: {e}"),
                Ok(_) => panic!("COM987 should not exist"),
            }
        }

        #[test]
        fn list_ports_does_not_panic() {
            let ports = list_ports();
            assert!(ports.iter().all(|p| validate_port_name(&p.name).is_ok()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> SerialSpec {
        SerialSpec {
            port: "COM3".into(),
            baud: 115200,
            data_bits: 8,
            parity: Parity::None,
            stop_bits: StopBits::One,
            flow: FlowControl::None,
        }
    }

    #[test]
    fn port_name_validation() {
        for ok in ["COM1", "com3", " Com10 ", "COM255", "COM999"] {
            assert!(validate_port_name(ok).is_ok(), "{ok}");
        }
        assert_eq!(validate_port_name("com7").unwrap(), "COM7");
        for bad in [
            "", "COM", "COM0", "COM01", "COM1000", "COM1a", "COM-1", "LPT1", "NUL", r"\\.\COM3", "COM3:", "COM 3",
            "../COM3", "COM3\0", "ＣＯＭ3", "COM١",
        ] {
            assert!(validate_port_name(bad).is_err(), "{bad:?}");
        }
        assert_eq!(device_path("com12").unwrap(), r"\\.\COM12");
        assert!(device_path("C:\\x").is_err());
    }

    #[test]
    fn dcb_basic_8n1() {
        let f = dcb_fields(&spec()).unwrap();
        assert_eq!(f.baud_rate, 115200);
        assert_eq!(f.byte_size, 8);
        assert_eq!(f.parity, 0);
        assert_eq!(f.stop_bits, 0);
        assert_eq!(f.bitfield & F_BINARY, F_BINARY);
        assert_eq!(f.bitfield & F_PARITY, 0);
        assert_eq!((f.bitfield >> F_DTR_CONTROL_SHIFT) & 3, DTR_CONTROL_ENABLE);
        assert_eq!((f.bitfield >> F_RTS_CONTROL_SHIFT) & 3, RTS_CONTROL_ENABLE);
        assert_eq!(f.bitfield & (F_OUTX | F_INX | F_OUTX_CTS_FLOW | F_OUTX_DSR_FLOW), 0);
        assert_eq!(f.bitfield & (F_ABORT_ON_ERROR | F_NULL | F_ERROR_CHAR | F_DSR_SENSITIVITY), 0);
        assert_eq!(f.bitfield & !0x7FFF, 0, "only defined flag bits are set");
    }

    #[test]
    fn dcb_parity_stop_and_flow() {
        let cases = [
            (Parity::Odd, 1u8),
            (Parity::Even, 2),
            (Parity::Mark, 3),
            (Parity::Space, 4),
        ];
        for (p, v) in cases {
            let f = dcb_fields(&SerialSpec { parity: p, ..spec() }).unwrap();
            assert_eq!(f.parity, v);
            assert_eq!(f.bitfield & F_PARITY, F_PARITY);
        }
        assert_eq!(dcb_fields(&SerialSpec { stop_bits: StopBits::Two, ..spec() }).unwrap().stop_bits, 2);
        let f = dcb_fields(&SerialSpec { data_bits: 5, stop_bits: StopBits::OnePointFive, ..spec() }).unwrap();
        assert_eq!((f.byte_size, f.stop_bits), (5, 1));

        let x = dcb_fields(&SerialSpec { flow: FlowControl::XonXoff, ..spec() }).unwrap();
        assert_eq!(x.bitfield & (F_OUTX | F_INX), F_OUTX | F_INX);
        assert_eq!(x.bitfield & F_OUTX_CTS_FLOW, 0);
        assert_eq!((x.xon_char, x.xoff_char), (0x11, 0x13));
        assert!(x.xon_lim as u32 + x.xoff_lim as u32 <= QUEUE_SIZE);

        let r = dcb_fields(&SerialSpec { flow: FlowControl::RtsCts, ..spec() }).unwrap();
        assert_eq!(r.bitfield & F_OUTX_CTS_FLOW, F_OUTX_CTS_FLOW);
        assert_eq!((r.bitfield >> F_RTS_CONTROL_SHIFT) & 3, RTS_CONTROL_HANDSHAKE);
        assert_eq!(r.bitfield & (F_OUTX | F_INX), 0);
    }

    #[test]
    fn dcb_rejects_invalid_settings() {
        assert!(dcb_fields(&SerialSpec { baud: 0, ..spec() }).is_err());
        assert!(dcb_fields(&SerialSpec { baud: MAX_BAUD + 1, ..spec() }).is_err());
        assert!(dcb_fields(&SerialSpec { data_bits: 4, ..spec() }).is_err());
        assert!(dcb_fields(&SerialSpec { data_bits: 9, ..spec() }).is_err());
        assert!(dcb_fields(&SerialSpec { data_bits: 5, stop_bits: StopBits::Two, ..spec() }).is_err());
        assert!(dcb_fields(&SerialSpec { data_bits: 8, stop_bits: StopBits::OnePointFive, ..spec() }).is_err());
        for b in [300, 9600, 115200, 921600] {
            assert!(dcb_fields(&SerialSpec { baud: b, ..spec() }).is_ok());
        }
    }

    #[test]
    fn timeouts_return_quickly_when_idle() {
        assert_eq!(read_timeouts(), (u32::MAX, u32::MAX, 50));
        let (_, wc) = write_timeouts();
        assert!(wc > 0);
    }

    #[test]
    fn natural_port_sort() {
        let ports = ["COM10", "COM2", "COM1", "com3", "COM2"]
            .iter()
            .map(|n| PortInfo { name: n.to_string(), description: None })
            .collect();
        let names: Vec<_> = sort_ports(ports).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["COM1", "COM2", "com3", "COM10"]);
    }

    #[test]
    fn serde_names() {
        let json = serde_json::to_value(SerialSpec {
            parity: Parity::Mark,
            stop_bits: StopBits::OnePointFive,
            flow: FlowControl::RtsCts,
            ..spec()
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"port":"COM3","baud":115200,"dataBits":8,"parity":"mark","stopBits":"onePointFive","flow":"rtsCts"})
        );
        let back: SerialSpec = serde_json::from_value(serde_json::json!({
            "port":"COM4","baud":9600,"dataBits":7,"parity":"even","stopBits":"two","flow":"xonXoff"
        }))
        .unwrap();
        assert_eq!((back.parity, back.stop_bits, back.flow), (Parity::Even, StopBits::Two, FlowControl::XonXoff));
    }
}
