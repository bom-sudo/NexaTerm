//! SSH key generation and inspection. Private keys live only in files the user controls;
//! the database stores the path and public metadata.

use crate::error::{AppError, AppResult};
use russh::keys::ssh_key::{self, private::RsaKeypair, Algorithm, LineEnding, PrivateKey};
use russh::keys::HashAlg;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub path: String,
    pub key_type: String,
    pub fingerprint: String,
    pub comment: String,
    pub encrypted: bool,
    /// Public key line; available even for encrypted keys (stored unencrypted in OpenSSH format).
    pub public_key: String,
}

fn info_from(path: &Path, k: &PrivateKey) -> AppResult<KeyInfo> {
    let public = k.public_key();
    Ok(KeyInfo {
        path: path.display().to_string(),
        key_type: k.algorithm().as_str().to_string(),
        fingerprint: public.fingerprint(HashAlg::Sha256).to_string(),
        comment: k.comment().to_string(),
        encrypted: k.is_encrypted(),
        public_key: public.to_openssh().map_err(|e| AppError::Other(e.to_string()))?,
    })
}

/// Parse a key file without needing its passphrase.
pub fn inspect(path: &Path) -> AppResult<KeyInfo> {
    let text = std::fs::read_to_string(path).map_err(|e| AppError::Validation(format!("Cannot read key file: {e}")))?;
    if text.contains("BEGIN RSA PRIVATE KEY") || text.contains("BEGIN EC PRIVATE KEY") || text.contains("BEGIN PRIVATE KEY") {
        // PEM keys are usable for login (russh decodes them) but have no embedded public part.
        let k = russh::keys::decode_secret_key(&text, None)
            .map_err(|_| AppError::Validation("PEM key is encrypted or malformed; convert it with ssh-keygen -p -f <file>".into()))?;
        return info_from(path, &k);
    }
    let k = PrivateKey::from_openssh(&text).map_err(|e| AppError::Validation(format!("Malformed or unsupported key file: {e}")))?;
    info_from(path, &k)
}

/// Generate a key pair at `path` (+ `path.pub`). Refuses to overwrite existing files.
pub fn generate(path: &Path, kind: &str, bits: usize, comment: &str, passphrase: Option<&str>) -> AppResult<KeyInfo> {
    if path.exists() || path.with_extension(ext_pub(path)).exists() {
        return Err(AppError::Validation(format!("{} already exists; choose another file name", path.display())));
    }
    let mut rng = rand::rng();
    let mut key = match kind {
        "ed25519" => PrivateKey::random(&mut rng, Algorithm::Ed25519).map_err(|e| AppError::Other(e.to_string()))?,
        "rsa" => {
            if !(2048..=8192).contains(&bits) {
                return Err(AppError::Validation("RSA key size must be 2048-8192 bits".into()));
            }
            let kp = RsaKeypair::random(&mut rng, bits).map_err(|e| AppError::Other(e.to_string()))?;
            PrivateKey::new(ssh_key::private::KeypairData::from(kp), comment).map_err(|e| AppError::Other(e.to_string()))?
        }
        other => return Err(AppError::Validation(format!("Unsupported key type {other}"))),
    };
    key.set_comment(comment);
    let public = key.public_key().to_openssh().map_err(|e| AppError::Other(e.to_string()))?;
    let to_write = match passphrase.filter(|p| !p.is_empty()) {
        Some(p) => key.encrypt(&mut rng, p).map_err(|e| AppError::Other(e.to_string()))?,
        None => key,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    to_write.write_openssh_file(path, LineEnding::LF).map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::write(pub_path(path), format!("{public}\n"))?;
    info_from(path, &to_write)
}

/// Change (or set / remove) the passphrase of an OpenSSH-format key in place.
pub fn change_passphrase(path: &Path, old: Option<&str>, new: Option<&str>) -> AppResult<KeyInfo> {
    let text = std::fs::read_to_string(path)?;
    let k = PrivateKey::from_openssh(&text).map_err(|e| AppError::Validation(format!("Malformed key: {e}")))?;
    let plain = if k.is_encrypted() {
        k.decrypt(old.unwrap_or("")).map_err(|_| AppError::AuthFailed("current passphrase is wrong".into()))?
    } else {
        k
    };
    let out = match new.filter(|p| !p.is_empty()) {
        Some(p) => plain.encrypt(&mut rand::rng(), p).map_err(|e| AppError::Other(e.to_string()))?,
        None => plain,
    };
    let tmp = path.with_extension("nexaterm-tmp");
    out.write_openssh_file(&tmp, LineEnding::LF).map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::rename(&tmp, path)?;
    info_from(path, &out)
}

fn ext_pub(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some(e) => format!("{e}.pub"),
        None => "pub".into(),
    }
}

pub fn pub_path(path: &Path) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".pub");
    s.into()
}

pub fn default_ssh_dir() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_default().join(".ssh")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_inspect_and_rekey() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("id_test");
        let info = generate(&p, "ed25519", 0, "me@test", Some("pw1")).unwrap();
        assert!(info.encrypted);
        assert_eq!(info.key_type, "ssh-ed25519");
        assert!(pub_path(&p).exists());
        assert!(generate(&p, "ed25519", 0, "x", None).is_err(), "must not overwrite");

        let i2 = inspect(&p).unwrap();
        assert_eq!(i2.fingerprint, info.fingerprint);
        // Login loader agrees: needs passphrase, wrong one fails, right one works.
        let ps = p.to_str().unwrap();
        assert!(matches!(crate::ssh::load_private_key(ps, None), Err(AppError::SecretRequired(_))));
        assert!(crate::ssh::load_private_key(ps, Some("bad")).is_err());
        assert!(crate::ssh::load_private_key(ps, Some("pw1")).is_ok());

        assert!(change_passphrase(&p, Some("wrong"), Some("pw2")).is_err());
        let i3 = change_passphrase(&p, Some("pw1"), None).unwrap();
        assert!(!i3.encrypted);
        assert!(crate::ssh::load_private_key(ps, None).is_ok());
        assert_eq!(i3.fingerprint, info.fingerprint);
    }

    #[test]
    fn rsa_generation_and_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("id_rsa_test");
        let info = generate(&p, "rsa", 2048, "", None).unwrap();
        assert_eq!(info.key_type, "ssh-rsa");
        assert!(generate(&dir.path().join("x"), "rsa", 512, "", None).is_err());
        let bad = dir.path().join("bad");
        std::fs::write(&bad, "hello").unwrap();
        assert!(inspect(&bad).is_err());
    }
}
