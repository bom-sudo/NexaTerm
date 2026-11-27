//! Telnet client backend (RFC 854 / 855, NAWS RFC 1073, TERMINAL-TYPE RFC 1091).
//!
//! The protocol logic lives in [`TelnetProtocol`], a pure sans-IO state machine that is fed raw
//! socket bytes and returns (terminal data, bytes to send back). [`TelnetSession`] owns the TCP
//! socket in a single tokio task driven by an mpsc command channel, so `write` / `resize` never
//! block the caller.
//!
//! Enter key / NVT choice: xterm.js sends a lone CR for Enter. In NVT (non-binary) mode RFC 854
//! requires a CR to be followed by LF or NUL, so a lone CR is sent as `CR NUL` (the BSD telnet
//! default; the server's line discipline turns it into Enter). A CR that is already followed by
//! LF in the same write (e.g. pasted text) is sent unchanged as `CR LF`. Only if we had agreed to
//! transmit BINARY would a raw CR be sent; the option policy below refuses BINARY, so in practice
//! Enter is always `CR NUL`. A CR at the very end of one write followed by an LF at the start of
//! the next is sent as `CR NUL LF` (harmless; the extra NUL is ignored by servers).
//!
//! Negotiation is reactive only (the client never offers options first); typical servers
//! (Linux telnetd, network equipment) open with their own DO/WILL requests.

use super::{TermEvent, TermSink, TerminalBackend};
use crate::error::{AppError, AppResult};
use crate::security::{validate_host, validate_port};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

// Telnet commands (RFC 854).
pub const IAC: u8 = 255;
pub const DONT: u8 = 254;
pub const DO: u8 = 253;
pub const WONT: u8 = 252;
pub const WILL: u8 = 251;
pub const SB: u8 = 250;
pub const SE: u8 = 240;

// Telnet options.
pub const OPT_BINARY: u8 = 0;
pub const OPT_ECHO: u8 = 1;
pub const OPT_SGA: u8 = 3;
pub const OPT_TTYPE: u8 = 24;
pub const OPT_NAWS: u8 = 31;

// TERMINAL-TYPE subnegotiation codes (RFC 1091).
const TTYPE_IS: u8 = 0;
const TTYPE_SEND: u8 = 1;

/// Subnegotiation payloads larger than this are truncated (a malicious server cannot make us
/// buffer unbounded memory).
const MAX_SB_LEN: usize = 4096;
const DEFAULT_TERM: &str = "xterm-256color";
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;

#[derive(Debug, Clone)]
pub struct TelnetSpec {
    pub host: String,
    pub port: u16,
    pub cols: u16,
    pub rows: u16,
    pub term: String,
    pub connect_timeout_secs: u64,
}

/// Per-option negotiation state for one side (RFC 1143 "Q method", without the queue bit since
/// the client never initiates a change).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum OptState {
    #[default]
    No,
    Yes,
    /// We refused this option once; identical repeated requests are ignored so a misbehaving
    /// server cannot drive a negotiation loop.
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    Data,
    /// Previous byte was IAC.
    Iac,
    /// Previous bytes were IAC WILL/WONT/DO/DONT; waiting for the option byte.
    Neg(u8),
    /// Inside IAC SB ... collecting payload.
    Sb,
    /// Inside SB and the previous byte was IAC.
    SbIac,
}

/// Output of [`TelnetProtocol::feed`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Feed {
    /// Bytes for the terminal (all telnet commands stripped, IAC IAC unescaped).
    pub data: Vec<u8>,
    /// Bytes that must be sent back to the server (negotiation replies).
    pub reply: Vec<u8>,
}

/// Sans-IO Telnet protocol state machine. State persists across `feed` calls, so commands split
/// between TCP reads are handled correctly.
#[derive(Debug)]
pub struct TelnetProtocol {
    state: ParseState,
    sb: Vec<u8>,
    /// Options the server performs (answered with DO/DONT).
    him: [OptState; 256],
    /// Options we perform (answered with WILL/WONT).
    us: [OptState; 256],
    /// Last byte delivered as data was a CR (used to drop the NUL of an incoming CR NUL).
    last_cr: bool,
    term: String,
    cols: u16,
    rows: u16,
}

/// Terminal type names must be short printable ASCII (RFC 1091); anything else falls back to the default.
fn sanitize_term(term: &str) -> String {
    let t = term.trim();
    if t.is_empty() || t.len() > 40 || !t.bytes().all(|b| b.is_ascii_graphic()) {
        DEFAULT_TERM.to_string()
    } else {
        t.to_string()
    }
}

/// Append `bytes` to `out`, doubling every 0xFF (IAC escaping, used both for data and SB payloads).
fn push_escaped(out: &mut Vec<u8>, bytes: &[u8]) {
    for &b in bytes {
        out.push(b);
        if b == IAC {
            out.push(IAC);
        }
    }
}

impl TelnetProtocol {
    pub fn new(term: &str, cols: u16, rows: u16) -> Self {
        Self {
            state: ParseState::Data,
            sb: Vec::new(),
            him: [OptState::No; 256],
            us: [OptState::No; 256],
            last_cr: false,
            term: sanitize_term(term),
            cols,
            rows,
        }
    }

    /// True if we agreed to perform `opt` (sent WILL and the server agreed / asked for it).
    pub fn local_enabled(&self, opt: u8) -> bool {
        self.us[opt as usize] == OptState::Yes
    }

    /// True if the server performs `opt` (we sent DO).
    pub fn remote_enabled(&self, opt: u8) -> bool {
        self.him[opt as usize] == OptState::Yes
    }

    /// Process bytes received from the server.
    pub fn feed(&mut self, input: &[u8]) -> Feed {
        let mut out = Feed::default();
        for &b in input {
            match self.state {
                ParseState::Data => {
                    if b == IAC {
                        self.state = ParseState::Iac;
                    } else {
                        self.push_data(&mut out.data, b);
                    }
                }
                ParseState::Iac => self.on_command(b, &mut out),
                ParseState::Neg(cmd) => {
                    self.state = ParseState::Data;
                    self.negotiate(cmd, b, &mut out.reply);
                }
                ParseState::Sb => {
                    if b == IAC {
                        self.state = ParseState::SbIac;
                    } else if self.sb.len() < MAX_SB_LEN {
                        self.sb.push(b);
                    }
                }
                ParseState::SbIac => match b {
                    IAC => {
                        if self.sb.len() < MAX_SB_LEN {
                            self.sb.push(IAC);
                        }
                        self.state = ParseState::Sb;
                    }
                    SE => {
                        self.state = ParseState::Data;
                        let payload = std::mem::take(&mut self.sb);
                        self.subnegotiation(&payload, &mut out.reply);
                    }
                    other => {
                        // Malformed SB (IAC followed by a command other than SE): drop the
                        // payload and treat the byte as a regular command, as most clients do.
                        self.sb.clear();
                        self.on_command(other, &mut out);
                    }
                },
            }
        }
        out
    }

    fn push_data(&mut self, data: &mut Vec<u8>, b: u8) {
        // In NVT mode the server sends a bare CR as CR NUL; drop the NUL.
        if self.last_cr && b == 0 && !self.remote_enabled(OPT_BINARY) {
            self.last_cr = false;
            return;
        }
        self.last_cr = b == b'\r';
        data.push(b);
    }

    /// Handle the byte following IAC.
    fn on_command(&mut self, b: u8, out: &mut Feed) {
        self.state = ParseState::Data;
        match b {
            IAC => self.push_data(&mut out.data, IAC),
            WILL | WONT | DO | DONT => self.state = ParseState::Neg(b),
            SB => {
                self.sb.clear();
                self.state = ParseState::Sb;
            }
            // NOP, DM, BRK, IP, AO, AYT, EC, EL, GA and stray SE carry no data for the terminal.
            _ => {}
        }
    }

    fn negotiate(&mut self, cmd: u8, opt: u8, reply: &mut Vec<u8>) {
        let i = opt as usize;
        match cmd {
            WILL => match self.him[i] {
                OptState::No => {
                    // ยอมรับให้ server ทำ echo และ suppress-go-ahead (โหมดพิมพ์ทีละตัวอักษร) เท่านั้น
                    if matches!(opt, OPT_ECHO | OPT_SGA) {
                        self.him[i] = OptState::Yes;
                        reply.extend_from_slice(&[IAC, DO, opt]);
                    } else {
                        self.him[i] = OptState::Refused;
                        reply.extend_from_slice(&[IAC, DONT, opt]);
                    }
                }
                OptState::Yes | OptState::Refused => {}
            },
            WONT => {
                if self.him[i] == OptState::Yes {
                    self.him[i] = OptState::No;
                    reply.extend_from_slice(&[IAC, DONT, opt]);
                }
            }
            DO => match self.us[i] {
                OptState::No => {
                    // รองรับการส่งขนาดหน้าต่าง (NAWS), ชนิดเทอร์มินัล (TTYPE) และ SGA
                    if matches!(opt, OPT_NAWS | OPT_TTYPE | OPT_SGA) {
                        self.us[i] = OptState::Yes;
                        reply.extend_from_slice(&[IAC, WILL, opt]);
                        if opt == OPT_NAWS {
                            reply.extend_from_slice(&self.naws_bytes());
                        }
                    } else {
                        self.us[i] = OptState::Refused;
                        reply.extend_from_slice(&[IAC, WONT, opt]);
                    }
                }
                OptState::Yes | OptState::Refused => {}
            },
            DONT => {
                if self.us[i] == OptState::Yes {
                    self.us[i] = OptState::No;
                    reply.extend_from_slice(&[IAC, WONT, opt]);
                }
            }
            _ => {}
        }
    }

    fn subnegotiation(&mut self, payload: &[u8], reply: &mut Vec<u8>) {
        if payload.len() >= 2 && payload[0] == OPT_TTYPE && payload[1] == TTYPE_SEND && self.local_enabled(OPT_TTYPE) {
            reply.extend_from_slice(&[IAC, SB, OPT_TTYPE, TTYPE_IS]);
            push_escaped(reply, self.term.as_bytes());
            reply.extend_from_slice(&[IAC, SE]);
        }
        // Other subnegotiations (or TTYPE SEND before we agreed to TTYPE) are ignored.
    }

    /// `IAC SB NAWS <w16> <h16> IAC SE` for the current size (0xFF bytes doubled).
    fn naws_bytes(&self) -> Vec<u8> {
        let mut v = vec![IAC, SB, OPT_NAWS];
        let mut size = Vec::with_capacity(4);
        size.extend_from_slice(&self.cols.to_be_bytes());
        size.extend_from_slice(&self.rows.to_be_bytes());
        push_escaped(&mut v, &size);
        v.extend_from_slice(&[IAC, SE]);
        v
    }

    /// Record a new window size; returns the NAWS message to send (empty if NAWS is not agreed).
    pub fn resize(&mut self, cols: u16, rows: u16) -> Vec<u8> {
        self.cols = cols;
        self.rows = rows;
        if self.local_enabled(OPT_NAWS) {
            self.naws_bytes()
        } else {
            Vec::new()
        }
    }

    /// Encode keyboard / paste input for the wire: escape 0xFF and apply the NVT CR rule
    /// (see the module docs).
    pub fn encode_input(&self, data: &[u8]) -> Vec<u8> {
        let binary = self.local_enabled(OPT_BINARY);
        let mut out = Vec::with_capacity(data.len() + 4);
        for (i, &b) in data.iter().enumerate() {
            match b {
                IAC => out.extend_from_slice(&[IAC, IAC]),
                b'\r' if !binary && data.get(i + 1) != Some(&b'\n') => out.extend_from_slice(b"\r\0"),
                _ => out.push(b),
            }
        }
        out
    }
}

enum Cmd {
    Data(Vec<u8>),
    Resize(u16, u16),
    Close,
}

pub struct TelnetSession {
    tx: mpsc::UnboundedSender<Cmd>,
}

impl TelnetSession {
    pub async fn open(spec: TelnetSpec, sink: TermSink) -> AppResult<Arc<TelnetSession>> {
        let host = validate_host(&spec.host)?;
        let port = validate_port(spec.port as i64)?;
        let secs = if spec.connect_timeout_secs == 0 { DEFAULT_CONNECT_TIMEOUT_SECS } else { spec.connect_timeout_secs };

        let stream = match tokio::time::timeout(Duration::from_secs(secs), TcpStream::connect((host.as_str(), port))).await {
            Err(_) => return Err(AppError::Timeout),
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                // แจ้งผู้ใช้ว่าปลายทางปฏิเสธการเชื่อมต่อ (มักเป็นเพราะไม่มี Telnet server ที่พอร์ตนี้)
                return Err(AppError::Connection(format!(
                    "{host}:{port} refused the connection (is a Telnet server running on this port?)"
                )));
            }
            Ok(Err(e)) => return Err(AppError::Connection(format!("Cannot connect to {host}:{port}: {e}"))),
            Ok(Ok(s)) => s,
        };
        let _ = stream.set_nodelay(true);

        let proto = TelnetProtocol::new(&spec.term, spec.cols.max(1), spec.rows.max(1));
        let (tx, rx) = mpsc::unbounded_channel::<Cmd>();
        tokio::spawn(run(stream, proto, rx, sink));
        Ok(Arc::new(Self { tx }))
    }
}

/// Socket task: owns the stream and the protocol state; multiplexes socket reads and commands.
async fn run(mut stream: TcpStream, mut proto: TelnetProtocol, mut rx: mpsc::UnboundedReceiver<Cmd>, sink: TermSink) {
    let mut buf = vec![0u8; 16 * 1024];
    let lost = || TermEvent::Exit { code: None, reason: Some("Connection lost".into()) };
    let ended = TermEvent::Exit { code: None, reason: None };
    let exit = loop {
        tokio::select! {
            r = stream.read(&mut buf) => match r {
                Ok(0) => break ended,
                Err(_) => break lost(),
                Ok(n) => {
                    let feed = proto.feed(&buf[..n]);
                    if !feed.data.is_empty() {
                        sink.data(feed.data);
                    }
                    if !feed.reply.is_empty() && stream.write_all(&feed.reply).await.is_err() {
                        break lost();
                    }
                }
            },
            cmd = rx.recv() => {
                let bytes = match cmd {
                    Some(Cmd::Data(d)) => proto.encode_input(&d),
                    Some(Cmd::Resize(c, r)) => proto.resize(c, r),
                    // ผู้ใช้ปิดเทอร์มินัลเอง หรือ session ถูกทิ้ง: ปิดการเชื่อมต่ออย่างสุภาพ
                    Some(Cmd::Close) | None => {
                        let _ = stream.shutdown().await;
                        break ended;
                    }
                };
                if !bytes.is_empty() && stream.write_all(&bytes).await.is_err() {
                    break lost();
                }
            }
        }
    };
    sink.event(exit);
}

impl TerminalBackend for TelnetSession {
    fn write(&self, data: &[u8]) -> AppResult<()> {
        self.tx.send(Cmd::Data(data.to_vec())).map_err(|_| AppError::Terminal("Telnet connection is closed".into()))
    }
    fn resize(&self, cols: u16, rows: u16) -> AppResult<()> {
        self.tx.send(Cmd::Resize(cols, rows)).map_err(|_| AppError::Terminal("Telnet connection is closed".into()))
    }
    fn close(&self) {
        let _ = self.tx.send(Cmd::Close);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proto() -> TelnetProtocol {
        TelnetProtocol::new("xterm-256color", 80, 24)
    }

    #[test]
    fn plain_data_passes_through() {
        let mut p = proto();
        let f = p.feed(b"hello\r\nworld");
        assert_eq!(f.data, b"hello\r\nworld");
        assert!(f.reply.is_empty());
    }

    #[test]
    fn iac_iac_unescapes_even_when_split() {
        let mut p = proto();
        let mut data = p.feed(b"a\xff").data;
        data.extend(p.feed(b"\xffb").data);
        assert_eq!(data, b"a\xffb");
    }

    #[test]
    fn split_negotiation_sequence() {
        let mut p = proto();
        let f1 = p.feed(&[b'x', IAC]);
        let f2 = p.feed(&[WILL]);
        let f3 = p.feed(&[OPT_ECHO, b'y']);
        assert_eq!(f1.data, b"x");
        assert!(f1.reply.is_empty() && f2.reply.is_empty() && f2.data.is_empty());
        assert_eq!(f3.reply, vec![IAC, DO, OPT_ECHO]);
        assert_eq!(f3.data, b"y");
        assert!(p.remote_enabled(OPT_ECHO));
    }

    #[test]
    fn option_policy_replies() {
        let mut p = proto();
        let f = p.feed(&[IAC, WILL, OPT_SGA, IAC, WILL, OPT_BINARY, IAC, WILL, 42]);
        assert_eq!(f.reply, vec![IAC, DO, OPT_SGA, IAC, DONT, OPT_BINARY, IAC, DONT, 42]);
        let f = p.feed(&[IAC, DO, OPT_TTYPE, IAC, DO, OPT_SGA, IAC, DO, OPT_BINARY, IAC, DO, 39]);
        assert_eq!(f.reply, vec![IAC, WILL, OPT_TTYPE, IAC, WILL, OPT_SGA, IAC, WONT, OPT_BINARY, IAC, WONT, 39]);
        assert!(f.data.is_empty());
    }

    #[test]
    fn do_naws_sends_size_and_resize_resends() {
        let mut p = TelnetProtocol::new("vt100", 80, 24);
        assert!(p.resize(90, 30).is_empty(), "no NAWS before agreement");
        let f = p.feed(&[IAC, DO, OPT_NAWS]);
        assert_eq!(f.reply, vec![IAC, WILL, OPT_NAWS, IAC, SB, OPT_NAWS, 0, 90, 0, 30, IAC, SE]);
        assert_eq!(p.resize(100, 40), vec![IAC, SB, OPT_NAWS, 0, 100, 0, 40, IAC, SE]);
        // 255 in the size is escaped.
        assert_eq!(p.resize(255, 256), vec![IAC, SB, OPT_NAWS, 0, 255, 255, 1, 0, IAC, SE]);
    }

    #[test]
    fn no_negotiation_loop_on_repeats() {
        let mut p = proto();
        assert_eq!(p.feed(&[IAC, WILL, OPT_ECHO]).reply, vec![IAC, DO, OPT_ECHO]);
        assert!(p.feed(&[IAC, WILL, OPT_ECHO]).reply.is_empty());
        assert_eq!(p.feed(&[IAC, DO, OPT_NAWS]).reply.len(), 3 + 9);
        assert!(p.feed(&[IAC, DO, OPT_NAWS]).reply.is_empty());
        // Refused options are refused once only.
        assert_eq!(p.feed(&[IAC, DO, 77]).reply, vec![IAC, WONT, 77]);
        assert!(p.feed(&[IAC, DO, 77, IAC, DO, 77]).reply.is_empty());
        assert_eq!(p.feed(&[IAC, WILL, 77]).reply, vec![IAC, DONT, 77]);
        assert!(p.feed(&[IAC, WILL, 77]).reply.is_empty());
        // Disabling acknowledges once, and disabling something already off is silent.
        assert_eq!(p.feed(&[IAC, WONT, OPT_ECHO]).reply, vec![IAC, DONT, OPT_ECHO]);
        assert!(p.feed(&[IAC, WONT, OPT_ECHO]).reply.is_empty());
        assert_eq!(p.feed(&[IAC, DONT, OPT_NAWS]).reply, vec![IAC, WONT, OPT_NAWS]);
        assert!(p.feed(&[IAC, DONT, OPT_NAWS]).reply.is_empty());
        assert!(p.resize(10, 10).is_empty());
    }

    #[test]
    fn ttype_subnegotiation_split_across_reads() {
        let mut p = proto();
        p.feed(&[IAC, DO, OPT_TTYPE]);
        let mut reply = Vec::new();
        for chunk in [&[IAC, SB][..], &[OPT_TTYPE, TTYPE_SEND, IAC][..], &[SE, b'!'][..]] {
            let f = p.feed(chunk);
            reply.extend(f.reply);
            if chunk.last() == Some(&b'!') {
                assert_eq!(f.data, b"!");
            } else {
                assert!(f.data.is_empty());
            }
        }
        let mut want = vec![IAC, SB, OPT_TTYPE, TTYPE_IS];
        want.extend_from_slice(b"xterm-256color");
        want.extend_from_slice(&[IAC, SE]);
        assert_eq!(reply, want);
    }

    #[test]
    fn ttype_send_ignored_unless_agreed_and_unknown_sb_stripped() {
        let mut p = proto();
        let f = p.feed(&[IAC, SB, OPT_TTYPE, TTYPE_SEND, IAC, SE]);
        assert!(f.reply.is_empty());
        // Unknown SB with an escaped IAC inside is consumed completely.
        let f = p.feed(&[b'a', IAC, SB, 99, 1, IAC, IAC, 2, IAC, SE, b'b']);
        assert_eq!(f.data, b"ab");
        assert!(f.reply.is_empty());
    }

    #[test]
    fn malformed_sb_recovers() {
        let mut p = proto();
        let f = p.feed(&[IAC, SB, 99, 1, IAC, WILL, OPT_ECHO, b'z']);
        assert_eq!(f.reply, vec![IAC, DO, OPT_ECHO]);
        assert_eq!(f.data, b"z");
    }

    #[test]
    fn oversized_sb_is_bounded() {
        let mut p = proto();
        let mut big = vec![IAC, SB, 99];
        big.extend(std::iter::repeat_n(7u8, MAX_SB_LEN * 3));
        assert!(p.feed(&big).data.is_empty());
        assert_eq!(p.sb.len(), MAX_SB_LEN);
        let f = p.feed(&[IAC, SE, b'k']);
        assert_eq!(f.data, b"k");
        assert!(p.sb.is_empty());
    }

    #[test]
    fn other_commands_are_stripped() {
        let mut p = proto();
        // NOP (241), GA (249), AYT (246), stray SE.
        let f = p.feed(&[b'1', IAC, 241, b'2', IAC, 249, IAC, 246, IAC, SE, b'3']);
        assert_eq!(f.data, b"123");
    }

    #[test]
    fn incoming_cr_nul_becomes_cr() {
        let mut p = proto();
        let mut d = p.feed(b"a\r").data;
        d.extend(p.feed(b"\0b\0").data);
        assert_eq!(d, b"a\rb\0");
    }

    #[test]
    fn outgoing_encoding() {
        let p = proto();
        assert_eq!(p.encode_input(b"root\r"), b"root\r\0");
        assert_eq!(p.encode_input(b"a\r\nb"), b"a\r\nb");
        assert_eq!(p.encode_input(&[1, 0xff, 2]), vec![1, 0xff, 0xff, 2]);
    }

    #[test]
    fn term_is_sanitized() {
        assert_eq!(TelnetProtocol::new("", 1, 1).term, DEFAULT_TERM);
        assert_eq!(TelnetProtocol::new("vt 100", 1, 1).term, DEFAULT_TERM);
        assert_eq!(TelnetProtocol::new(" VT100 ", 1, 1).term, "VT100");
    }
}
