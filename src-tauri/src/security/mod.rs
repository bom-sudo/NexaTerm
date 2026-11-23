//! Input validation and log redaction helpers.

use crate::error::{AppError, AppResult};

/// Mask anything that looks like a secret assignment (`password=...`, `passphrase: ...`, `token=...`)
/// and PEM private-key bodies. Used for every error message and log line that leaves the core.
pub fn redact(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    if input.contains("PRIVATE KEY-----") {
        return "[redacted: private key material]".into();
    }
    let lower = input.to_ascii_lowercase();
    let keys = ["password", "passphrase", "passwd", "secret", "token"];
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let mut matched = None;
        for k in keys {
            if lower[i..].starts_with(k) {
                matched = Some(k.len());
                break;
            }
        }
        if let Some(klen) = matched {
            let mut j = i + klen;
            // skip optional whitespace and separator
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == b'=' || bytes[j] == b':') {
                j += 1;
                while j < bytes.len() && bytes[j] == b' ' {
                    j += 1;
                }
                out.push_str(&input[i..j]);
                out.push_str("***");
                while j < bytes.len() && !matches!(bytes[j], b' ' | b',' | b';' | b'&' | b'\n' | b'"') {
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        // copy one UTF-8 char
        let ch = input[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Hostname / IPv4 / IPv6 literal validation. Rejects whitespace, shell metacharacters and
/// option-like values (leading '-') so the value is always safe as a structured argument.
pub fn validate_host(host: &str) -> AppResult<String> {
    let h = host.trim();
    if h.is_empty() || h.len() > 253 {
        return Err(AppError::Validation("Host must be 1-253 characters".into()));
    }
    if h.starts_with('-') {
        return Err(AppError::Validation("Host must not start with '-'".into()));
    }
    if h.parse::<std::net::IpAddr>().is_ok() {
        return Ok(h.to_string());
    }
    let stripped = h.trim_start_matches('[').trim_end_matches(']');
    if stripped.parse::<std::net::Ipv6Addr>().is_ok() {
        return Ok(stripped.to_string());
    }
    let ok = h
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_');
    if !ok {
        return Err(AppError::Validation(format!("Invalid host name: {h}")));
    }
    Ok(h.to_string())
}

pub fn validate_port(port: i64) -> AppResult<u16> {
    if (1..=65535).contains(&port) {
        Ok(port as u16)
    } else {
        Err(AppError::Validation("Port must be between 1 and 65535".into()))
    }
}

/// Remote SFTP paths: reject NUL bytes; everything else is passed to the server verbatim.
pub fn validate_remote_path(p: &str) -> AppResult<&str> {
    if p.is_empty() || p.contains('\0') {
        return Err(AppError::Validation("Invalid remote path".into()));
    }
    Ok(p)
}

/// A single file/directory name (no separators, not `.`/`..`).
pub fn validate_file_name(name: &str) -> AppResult<&str> {
    let n = name.trim();
    if n.is_empty() || n == "." || n == ".." || n.contains(['/', '\\', '\0']) {
        return Err(AppError::Validation("Invalid file name".into()));
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secrets() {
        let r = redact("login failed password=hunter2 for user");
        assert!(!r.contains("hunter2"));
        assert!(r.contains("password=***"));
        let r = redact("Passphrase: abc def");
        assert!(!r.contains("abc"));
        assert_eq!(redact("-----BEGIN OPENSSH PRIVATE KEY-----\nabc"), "[redacted: private key material]");
        assert_eq!(redact("plain message ไทย"), "plain message ไทย");
    }

    #[test]
    fn host_validation() {
        assert!(validate_host("example.com").is_ok());
        assert!(validate_host("10.0.0.1").is_ok());
        assert!(validate_host("::1").is_ok());
        assert!(validate_host("[fe80::1]").is_ok());
        assert!(validate_host("-oProxyCommand=calc").is_err());
        assert!(validate_host("a b").is_err());
        assert!(validate_host("a;calc").is_err());
        assert!(validate_host("").is_err());
    }

    #[test]
    fn names() {
        assert!(validate_file_name("a.txt").is_ok());
        assert!(validate_file_name("../x").is_err());
        assert!(validate_file_name("..").is_err());
        assert!(validate_port(0).is_err());
        assert_eq!(validate_port(22).unwrap(), 22);
    }
}
