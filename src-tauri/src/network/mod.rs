//! Network diagnostics. External Windows tools are spawned with structured, validated arguments.

use crate::error::{AppError, AppResult};
use crate::security::{validate_host, validate_port};
use crate::state::tokio_util_like::CancelFlag;
use serde::Serialize;
use std::net::IpAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Run `ping.exe` or `tracert.exe`, streaming output lines; killed on cancel or timeout.
pub async fn run_tool(tool: &str, host: &str, count: u32, cancel: &CancelFlag, mut line: impl FnMut(String)) -> AppResult<i32> {
    let host = validate_host(host)?;
    let sysroot = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let (exe, args, limit): (String, Vec<String>, u64) = match tool {
        "ping" => {
            let n = count.clamp(1, 1000);
            (format!(r"{sysroot}\System32\PING.EXE"), vec!["-n".into(), n.to_string(), host], 10 + n as u64 * 6)
        }
        "traceroute" => (format!(r"{sysroot}\System32\TRACERT.EXE"), vec!["-d".into(), "-w".into(), "1500".into(), "-h".into(), "30".into(), host], 300),
        other => return Err(AppError::Validation(format!("Unknown tool {other}"))),
    };
    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(&args).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = cmd.spawn().map_err(|e| AppError::Io(format!("Cannot start {tool}: {e}")))?;
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).split(b'\n');
    let deadline = tokio::time::sleep(Duration::from_secs(limit));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => { let _ = child.kill().await; return Err(AppError::Cancelled); }
            _ = &mut deadline => { let _ = child.kill().await; return Err(AppError::Timeout); }
            l = lines.next_segment() => match l? {
                Some(bytes) => {
                    let s = String::from_utf8_lossy(&bytes).trim_end_matches('\r').to_string();
                    line(s);
                }
                None => break,
            }
        }
    }
    let status = child.wait().await?;
    Ok(status.code().unwrap_or(-1))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsResult {
    pub query: String,
    pub addresses: Vec<String>,
    pub elapsed_ms: u128,
}

pub async fn dns_lookup(host: &str) -> AppResult<DnsResult> {
    let host = validate_host(host)?;
    let start = Instant::now();
    let h = host.clone();
    let res = tokio::time::timeout(Duration::from_secs(10), tokio::task::spawn_blocking(move || dns_lookup::lookup_host(&h)))
        .await
        .map_err(|_| AppError::Timeout)?
        .map_err(|e| AppError::Other(e.to_string()))?
        .map_err(|e| AppError::Connection(format!("Lookup failed for {host}: {e}")))?;
    let mut addresses: Vec<String> = res.into_iter().map(|a| a.to_string()).collect();
    addresses.dedup();
    Ok(DnsResult { query: host, addresses, elapsed_ms: start.elapsed().as_millis() })
}

pub async fn reverse_dns(ip: &str) -> AppResult<String> {
    let addr: IpAddr = ip.trim().parse().map_err(|_| AppError::Validation("Enter a valid IPv4 or IPv6 address".into()))?;
    tokio::time::timeout(Duration::from_secs(10), tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&addr)))
        .await
        .map_err(|_| AppError::Timeout)?
        .map_err(|e| AppError::Other(e.to_string()))?
        .map_err(|e| AppError::Connection(format!("No PTR record: {e}")))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpProbe {
    pub host: String,
    pub port: u16,
    pub open: bool,
    pub address: Option<String>,
    pub elapsed_ms: u128,
    pub error: Option<String>,
}

/// Single-port reachability test (not a scanner: one host, one port per call).
pub async fn tcp_probe(host: &str, port: i64, timeout_ms: u64) -> AppResult<TcpProbe> {
    let host = validate_host(host)?;
    let port = validate_port(port)?;
    let start = Instant::now();
    let r = tokio::time::timeout(Duration::from_millis(timeout_ms.clamp(100, 30_000)), tokio::net::TcpStream::connect((host.as_str(), port))).await;
    let elapsed_ms = start.elapsed().as_millis();
    Ok(match r {
        Ok(Ok(s)) => TcpProbe { host, port, open: true, address: s.peer_addr().ok().map(|a| a.to_string()), elapsed_ms, error: None },
        Ok(Err(e)) => TcpProbe { host, port, open: false, address: None, elapsed_ms, error: Some(e.to_string()) },
        Err(_) => TcpProbe { host, port, open: false, address: None, elapsed_ms, error: Some("timed out (filtered or host down)".into()) },
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceInfo {
    pub name: String,
    pub address: String,
    pub is_loopback: bool,
    pub is_ipv6: bool,
    pub netmask: Option<String>,
}

pub fn interfaces() -> AppResult<Vec<InterfaceInfo>> {
    let list = if_addrs::get_if_addrs()?;
    Ok(list
        .into_iter()
        .map(|i| {
            let netmask = match &i.addr {
                if_addrs::IfAddr::V4(v) => Some(v.netmask.to_string()),
                if_addrs::IfAddr::V6(v) => Some(v.netmask.to_string()),
            };
            InterfaceInfo { is_loopback: i.is_loopback(), is_ipv6: i.ip().is_ipv6(), address: i.ip().to_string(), name: i.name, netmask }
        })
        .collect())
}

pub fn hostname() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_default()
}

/// Public IP lookup — only ever runs when the user clicks the button. Plain HTTP to ipify
/// (no identifiers sent beyond the TCP connection itself).
pub async fn public_ip() -> AppResult<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let fut = async {
        let mut s = tokio::net::TcpStream::connect(("api.ipify.org", 80)).await?;
        s.write_all(b"GET / HTTP/1.1\r\nHost: api.ipify.org\r\nUser-Agent: NexaTerm\r\nConnection: close\r\n\r\n").await?;
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).await?;
        Ok::<_, std::io::Error>(buf)
    };
    let buf = tokio::time::timeout(Duration::from_secs(10), fut).await.map_err(|_| AppError::Timeout)??;
    let text = String::from_utf8_lossy(&buf);
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").trim();
    body.parse::<IpAddr>().map(|ip| ip.to_string()).map_err(|_| AppError::Connection("Unexpected response from ipify".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tcp_probe_open_and_closed() {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let p = tcp_probe("127.0.0.1", port as i64, 2000).await.unwrap();
        assert!(p.open);
        drop(l);
        let p = tcp_probe("127.0.0.1", port as i64, 2000).await.unwrap();
        assert!(!p.open);
        assert!(tcp_probe("bad host", 22, 100).await.is_err());
        assert!(tcp_probe("localhost", 0, 100).await.is_err());
    }

    #[tokio::test]
    async fn localhost_dns_and_interfaces() {
        let r = dns_lookup("localhost").await.unwrap();
        assert!(!r.addresses.is_empty());
        assert!(interfaces().unwrap().iter().any(|i| i.is_loopback));
        assert!(reverse_dns("not-an-ip").await.is_err());
    }

    #[tokio::test]
    async fn ping_localhost_and_cancel() {
        let cancel = CancelFlag::default();
        let mut lines = vec![];
        let code = run_tool("ping", "127.0.0.1", 1, &cancel, |l| lines.push(l)).await.unwrap();
        assert_eq!(code, 0);
        assert!(lines.iter().any(|l| l.contains("127.0.0.1")));
        let c2 = CancelFlag::default();
        c2.cancel();
        assert!(matches!(run_tool("ping", "127.0.0.1", 50, &c2, |_| {}).await, Err(AppError::Cancelled)));
        assert!(run_tool("ping", "127.0.0.1 & calc", 1, &cancel, |_| {}).await.is_err());
    }
}
