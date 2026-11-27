//! Telnet backend against a scripted in-process telnet server.

use nexaterm_lib::terminal::telnet::{TelnetSession, TelnetSpec};
use nexaterm_lib::terminal::{TermEvent, TermSink, TerminalBackend};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

const IAC: u8 = 255;
const DO: u8 = 253;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;
const ECHO: u8 = 1;
const TTYPE: u8 = 24;
const NAWS: u8 = 31;

async fn read_until(sock: &mut tokio::net::TcpStream, buf: &mut Vec<u8>, pred: impl Fn(&[u8]) -> bool) {
    let mut tmp = [0u8; 512];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !pred(buf) {
        let n = tokio::time::timeout_at(deadline, sock.read(&mut tmp)).await.expect("timeout waiting for client").unwrap();
        assert!(n > 0, "client closed early");
        buf.extend_from_slice(&tmp[..n]);
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[tokio::test]
async fn negotiates_receives_data_resizes_and_exits() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let (dtx, mut drx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (etx, mut erx) = mpsc::unbounded_channel::<TermEvent>();
    let sink = TermSink::new(move |d| { let _ = dtx.send(d); }, move |e| { let _ = etx.send(e); });

    let client = tokio::spawn(TelnetSession::open(
        TelnetSpec { host: "127.0.0.1".into(), port, cols: 80, rows: 24, term: "xterm-256color".into(), connect_timeout_secs: 5 },
        sink,
    ));
    let (mut sock, _) = listener.accept().await.unwrap();
    let session = client.await.unwrap().expect("telnet connect");

    // Server negotiation + banner containing an escaped 0xFF byte.
    sock.write_all(&[IAC, DO, NAWS, IAC, WILL, ECHO, IAC, DO, TTYPE, IAC, SB, TTYPE, 1, IAC, SE]).await.unwrap();
    sock.write_all(b"login: ").await.unwrap();
    sock.write_all(&[IAC, IAC, b'\r', b'\n']).await.unwrap();

    let mut got = Vec::new();
    read_until(&mut sock, &mut got, |b| contains(b, b"xterm-256color") && contains(b, &[IAC, SB, NAWS])).await;
    assert!(contains(&got, &[IAC, WILL, NAWS]), "WILL NAWS: {got:?}");
    assert!(contains(&got, &[IAC, SB, NAWS, 0, 80, 0, 24, IAC, SE]), "NAWS 80x24: {got:?}");
    assert!(contains(&got, &[IAC, DO, ECHO]), "DO ECHO: {got:?}");
    assert!(contains(&got, &[IAC, WILL, TTYPE]), "WILL TTYPE: {got:?}");
    assert!(contains(&got, &[IAC, SB, TTYPE, 0]), "TTYPE IS: {got:?}");

    // Terminal receives the banner with IAC commands stripped and IAC IAC -> 0xFF.
    let mut screen = Vec::new();
    while !contains(&screen, &[0xFF, b'\r', b'\n']) {
        screen.extend(tokio::time::timeout(Duration::from_secs(5), drx.recv()).await.unwrap().unwrap());
    }
    assert_eq!(screen, b"login: \xFF\r\n".to_vec());

    // Typing reaches the server; resize sends a new NAWS.
    session.write(b"root\r").unwrap();
    session.resize(100, 40).unwrap();
    let mut got = Vec::new();
    read_until(&mut sock, &mut got, |b| contains(b, b"root") && contains(b, &[IAC, SB, NAWS, 0, 100, 0, 40, IAC, SE])).await;

    // Server closes -> Exit event.
    drop(sock);
    match tokio::time::timeout(Duration::from_secs(5), erx.recv()).await.unwrap().unwrap() {
        TermEvent::Exit { .. } => {}
        other => panic!("expected exit, got {other:?}"),
    }
    session.close();
}

#[tokio::test]
async fn refused_port_is_a_clear_error() {
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let sink = TermSink::new(|_| {}, |_| {});
    let r = TelnetSession::open(TelnetSpec { host: "127.0.0.1".into(), port, cols: 80, rows: 24, term: "vt100".into(), connect_timeout_secs: 3 }, sink).await;
    assert!(r.is_err());
    assert!(TelnetSession::open(
        TelnetSpec { host: "bad host;x".into(), port: 23, cols: 80, rows: 24, term: "vt100".into(), connect_timeout_secs: 3 },
        TermSink::new(|_| {}, |_| {})
    )
    .await
    .is_err());
}
