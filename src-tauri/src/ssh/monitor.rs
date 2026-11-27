//! Remote resource monitor for the status bar (like MobaXterm's monitoring bar).
//!
//! อ่านสถานะเครื่องปลายทาง (CPU, RAM, ดิสก์, เน็ต, uptime) ผ่าน exec channel แยกบนการเชื่อมต่อ
//! SSH เดิม ไม่พิมพ์อะไรลงในเทอร์มินัลของผู้ใช้ และอ่านอย่างเดียว ไม่แก้ไขอะไรบนเซิร์ฟเวอร์
//!
//! Only Linux exposes /proc in this format; other systems return `supported: false`.

use super::SshConnection;
use crate::error::{AppError, AppResult};
use russh::ChannelMsg;
use serde::Serialize;
use std::time::Duration;

/// Read-only probe. Section markers keep parsing independent of missing tools.
pub const PROBE: &str = "LC_ALL=C; export LC_ALL; \
echo '##os'; uname -s 2>/dev/null; \
echo '##stat'; head -n 1 /proc/stat 2>/dev/null; \
echo '##cpus'; grep -c '^processor' /proc/cpuinfo 2>/dev/null; \
echo '##mem'; grep -E '^(MemTotal|MemAvailable|SwapTotal|SwapFree):' /proc/meminfo 2>/dev/null; \
echo '##df'; df -kP / 2>/dev/null | tail -n 1; \
echo '##uptime'; cat /proc/uptime 2>/dev/null; \
echo '##load'; cat /proc/loadavg 2>/dev/null; \
echo '##net'; tail -n +3 /proc/net/dev 2>/dev/null; \
echo '##end'";

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStats {
    /// False when the server is not Linux (no /proc); the status bar then hides the monitor.
    pub supported: bool,
    pub os: String,
    /// Cumulative CPU jiffies (all and idle+iowait); the UI computes usage from two samples.
    pub cpu_total: u64,
    pub cpu_idle: u64,
    pub cpus: u32,
    pub mem_total_kb: u64,
    pub mem_available_kb: u64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
    pub disk_total_kb: u64,
    pub disk_used_kb: u64,
    pub uptime_secs: u64,
    pub load1: f64,
    /// Cumulative bytes over all interfaces except loopback.
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// Parse the probe output (pure; unit-tested).
pub fn parse(out: &str) -> RemoteStats {
    let mut s = RemoteStats::default();
    let mut section = "";
    for line in out.lines() {
        let line = line.trim_end();
        if let Some(name) = line.strip_prefix("##") {
            section = match name {
                "os" => "os",
                "stat" => "stat",
                "cpus" => "cpus",
                "mem" => "mem",
                "df" => "df",
                "uptime" => "uptime",
                "load" => "load",
                "net" => "net",
                _ => "",
            };
            continue;
        }
        if line.is_empty() {
            continue;
        }
        match section {
            "os" => s.os = line.trim().to_string(),
            "stat" => {
                let v: Vec<u64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
                if v.len() >= 4 {
                    // user nice system idle iowait irq softirq steal (guest is already in user)
                    s.cpu_total = v.iter().take(8).sum();
                    s.cpu_idle = v[3] + v.get(4).copied().unwrap_or(0);
                }
            }
            "cpus" => s.cpus = line.trim().parse().unwrap_or(0),
            "mem" => {
                let mut it = line.split_whitespace();
                let key = it.next().unwrap_or("");
                let val: u64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                match key {
                    "MemTotal:" => s.mem_total_kb = val,
                    "MemAvailable:" => s.mem_available_kb = val,
                    "SwapTotal:" => s.swap_total_kb = val,
                    "SwapFree:" => s.swap_free_kb = val,
                    _ => {}
                }
            }
            "df" => {
                // Filesystem 1024-blocks Used Available Capacity Mounted-on
                let f: Vec<&str> = line.split_whitespace().collect();
                if f.len() >= 6 {
                    s.disk_total_kb = f[1].parse().unwrap_or(0);
                    s.disk_used_kb = f[2].parse().unwrap_or(0);
                }
            }
            "uptime" => s.uptime_secs = line.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) as u64,
            "load" => s.load1 = line.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            "net" => {
                if let Some((iface, rest)) = line.split_once(':') {
                    if iface.trim() == "lo" {
                        continue;
                    }
                    let v: Vec<u64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                    if v.len() >= 9 {
                        s.rx_bytes += v[0];
                        s.tx_bytes += v[8];
                    }
                }
            }
            _ => {}
        }
    }
    s.supported = s.cpu_total > 0 && s.mem_total_kb > 0;
    s
}

/// Run the probe on its own exec channel of an existing connection.
pub async fn sample(conn: &SshConnection) -> AppResult<RemoteStats> {
    let fut = async {
        let mut ch = conn.handle.channel_open_session().await?;
        ch.exec(true, PROBE).await?;
        let mut out: Vec<u8> = Vec::new();
        while let Some(msg) = ch.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    if out.len() < 256 * 1024 {
                        out.extend_from_slice(&data);
                    }
                }
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
        }
        Ok::<_, AppError>(String::from_utf8_lossy(&out).into_owned())
    };
    let text = tokio::time::timeout(Duration::from_secs(8), fut).await.map_err(|_| AppError::Timeout)??;
    Ok(parse(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "##os\nLinux\n##stat\ncpu  100 5 50 800 20 0 5 0 0 0\n##cpus\n4\n##mem\nMemTotal:        8000000 kB\nMemAvailable:    5000000 kB\nSwapTotal:       2000000 kB\nSwapFree:        1500000 kB\n##df\n/dev/sda2 49000000 9800000 37000000 21% /\n##uptime\n3725.55 14000.10\n##load\n1.10 0.90 0.80 2/320 12345\n##net\n    lo: 999 10 0 0 0 0 0 0 999 10 0 0 0 0 0 0\n  ens192: 1000 20 0 0 0 0 0 0 2000 30 0 0 0 0 0 0\n  docker0: 500 1 0 0 0 0 0 0 100 1 0 0 0 0 0 0\n##end\n";

    #[test]
    fn parses_linux_probe() {
        let s = parse(SAMPLE);
        assert!(s.supported);
        assert_eq!(s.os, "Linux");
        assert_eq!(s.cpu_total, 100 + 5 + 50 + 800 + 20 + 0 + 5 + 0);
        assert_eq!(s.cpu_idle, 820);
        assert_eq!(s.cpus, 4);
        assert_eq!((s.mem_total_kb, s.mem_available_kb), (8_000_000, 5_000_000));
        assert_eq!((s.swap_total_kb, s.swap_free_kb), (2_000_000, 1_500_000));
        assert_eq!((s.disk_total_kb, s.disk_used_kb), (49_000_000, 9_800_000));
        assert_eq!(s.uptime_secs, 3725);
        assert!((s.load1 - 1.10).abs() < 1e-9);
        assert_eq!((s.rx_bytes, s.tx_bytes), (1500, 2100)); // loopback excluded
    }

    /// The probe is plain POSIX sh; run it for real on the Linux CI runner.
    #[cfg(target_os = "linux")]
    #[test]
    fn probe_runs_on_a_real_linux_shell() {
        let out = std::process::Command::new("/bin/sh").arg("-c").arg(PROBE).output().unwrap();
        let s = parse(&String::from_utf8_lossy(&out.stdout));
        assert!(s.supported, "probe output not understood: {}", String::from_utf8_lossy(&out.stdout));
        assert_eq!(s.os, "Linux");
        assert!(s.cpus >= 1 && s.mem_total_kb > 0 && s.disk_total_kb > 0 && s.uptime_secs > 0);
    }

    #[test]
    fn non_linux_is_unsupported() {
        let s = parse("##os\nFreeBSD\n##stat\n##cpus\n##mem\n##df\n/dev/ada0 100 50 50 50% /\n##end\n");
        assert!(!s.supported);
        assert_eq!(s.os, "FreeBSD");
        assert!(!parse("").supported);
    }
}
