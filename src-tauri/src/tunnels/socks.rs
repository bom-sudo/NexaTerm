//! Minimal SOCKS5 (RFC 1928) server side for dynamic forwarding: no-auth, CONNECT only.

use std::io::{Error, ErrorKind};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

fn bad(msg: &str) -> Error {
    Error::new(ErrorKind::InvalidData, msg.to_string())
}

/// Returns the requested (host, port). The caller must then send `reply_ok` / `reply_fail`.
pub async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(s: &mut S) -> std::io::Result<(String, u16)> {
    let ver = s.read_u8().await?;
    match ver {
        5 => {
            let n = s.read_u8().await? as usize;
            let mut methods = vec![0u8; n];
            s.read_exact(&mut methods).await?;
            if !methods.contains(&0) {
                s.write_all(&[5, 0xFF]).await?;
                return Err(bad("SOCKS5 client requires authentication"));
            }
            s.write_all(&[5, 0]).await?;
            let mut hdr = [0u8; 4];
            s.read_exact(&mut hdr).await?;
            if hdr[0] != 5 || hdr[1] != 1 {
                s.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
                return Err(bad("only SOCKS5 CONNECT is supported"));
            }
            let host = match hdr[3] {
                1 => {
                    let mut a = [0u8; 4];
                    s.read_exact(&mut a).await?;
                    std::net::Ipv4Addr::from(a).to_string()
                }
                3 => {
                    let len = s.read_u8().await? as usize;
                    let mut d = vec![0u8; len];
                    s.read_exact(&mut d).await?;
                    String::from_utf8(d).map_err(|_| bad("bad domain"))?
                }
                4 => {
                    let mut a = [0u8; 16];
                    s.read_exact(&mut a).await?;
                    std::net::Ipv6Addr::from(a).to_string()
                }
                _ => return Err(bad("bad address type")),
            };
            let port = s.read_u16().await?;
            Ok((host, port))
        }
        _ => Err(bad("not a SOCKS5 request (SOCKS4 is not supported)")),
    }
}

pub async fn reply_ok<S: AsyncWrite + Unpin>(s: &mut S) -> std::io::Result<()> {
    s.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await
}

pub async fn reply_fail<S: AsyncWrite + Unpin>(s: &mut S) -> std::io::Result<()> {
    s.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn socks5_domain_connect() {
        let (mut client, mut server) = tokio::io::duplex(256);
        let t = tokio::spawn(async move { handshake(&mut server).await });
        client.write_all(&[5, 1, 0]).await.unwrap();
        let mut r = [0u8; 2];
        client.read_exact(&mut r).await.unwrap();
        assert_eq!(r, [5, 0]);
        let mut req = vec![5, 1, 0, 3, 11];
        req.extend_from_slice(b"example.com");
        req.extend_from_slice(&443u16.to_be_bytes());
        client.write_all(&req).await.unwrap();
        assert_eq!(t.await.unwrap().unwrap(), ("example.com".to_string(), 443));
    }

    #[tokio::test]
    async fn socks5_rejects_auth_only_clients() {
        let (mut client, mut server) = tokio::io::duplex(64);
        let t = tokio::spawn(async move { handshake(&mut server).await });
        client.write_all(&[5, 1, 2]).await.unwrap();
        assert!(t.await.unwrap().is_err());
    }
}
