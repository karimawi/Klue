//! Who the server is: the certificate it makes for itself, and the key devices use. The key
//! carries the certificate's fingerprint, so the app checks it's talking to this server
//! with no domain or certificate authority involved.

use std::{io, path::Path};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64URL};
use klue_engine::remote::Key;
use sha2::{Digest, Sha256};

const CERT: &str = "cert.pem";
const PRIVATE_KEY: &str = "cert.key";
const SECRET: &str = "key";

pub struct Identity {
    pub cert_pem: String,
    pub key_pem: String,
    /// SHA-256 of the certificate, base64url.
    pub fingerprint: String,
}

/// The certificate in `dir`, made on the first start. It's valid for decades: the app trusts
/// it by its fingerprint, not by its dates or names.
pub fn certificate(dir: &Path) -> io::Result<Identity> {
    let (cert_path, key_path) = (dir.join(CERT), dir.join(PRIVATE_KEY));
    let (cert_pem, key_pem) = match (std::fs::read_to_string(&cert_path), std::fs::read_to_string(&key_path)) {
        (Ok(cert), Ok(key)) => (cert, key),
        _ => {
            let mut params = rcgen::CertificateParams::new(vec!["klue-server".to_string(), "localhost".to_string()]).map_err(io::Error::other)?;
            params.distinguished_name.push(rcgen::DnType::CommonName, "Klue Server");
            params.not_before = rcgen::date_time_ymd(2024, 1, 1);
            params.not_after = rcgen::date_time_ymd(2124, 1, 1);
            let key = rcgen::KeyPair::generate().map_err(io::Error::other)?;
            let cert = params.self_signed(&key).map_err(io::Error::other)?;
            let (cert, key) = (cert.pem(), key.serialize_pem());
            write_private(&cert_path, &cert)?;
            write_private(&key_path, &key)?;
            (cert, key)
        }
    };
    let fingerprint = fingerprint_of_pem(&cert_pem).ok_or_else(|| io::Error::other(format!("{} is damaged", cert_path.display())))?;
    Ok(Identity { cert_pem, key_pem, fingerprint })
}

fn fingerprint_of_pem(pem: &str) -> Option<String> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    let der = base64::engine::general_purpose::STANDARD.decode(body).ok()?;
    Some(fingerprint(&der))
}

/// SHA-256 of a certificate, base64url: what keys carry and the app checks.
pub fn fingerprint(der: &[u8]) -> String {
    B64URL.encode(Sha256::digest(der))
}

/// The secret devices send, made on the first start.
pub fn secret(dir: &Path) -> io::Result<String> {
    match std::fs::read_to_string(dir.join(SECRET)) {
        Ok(s) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        _ => new_secret(dir),
    }
}

/// Replaces the secret: every device has to be given the new key.
pub fn new_secret(dir: &Path) -> io::Result<String> {
    let secret = B64URL.encode(rand::random::<[u8; 24]>());
    write_private(&dir.join(SECRET), &secret)?;
    Ok(secret)
}

/// The key as people paste it, with the address when `KLUE_ADDRESS` gives it.
pub fn key(secret: &str, fingerprint: &str) -> Key {
    let address = std::env::var("KLUE_ADDRESS").ok().map(|a| a.trim().to_string()).filter(|a| !a.is_empty());
    Key { secret: secret.to_string(), fingerprint: fingerprint.to_string(), address }
}

/// What `docker logs` shows: the key in a box, and what to do with it.
pub fn banner(key: &Key, port: u16) -> String {
    let text = key.to_text();
    let width = text.chars().count().max(60) + 4;
    let line = "─".repeat(width);
    let pad = |s: &str| format!("│  {s}{}│", " ".repeat(width - 2 - s.chars().count()));
    let mut out = vec![format!("┌{line}┐"), pad("Klue Server is running."), pad(""), pad("Paste this key into Klue → Settings → Server:"), pad(""), pad(&text), pad("")];
    if key.address.is_none() {
        out.push(pad(&format!("Klue asks for this server's address too: its IP or domain, port {port}.")));
    }
    out.push(pad("Anyone with the key can use this server. `klue-server new-key` replaces it."));
    out.push(format!("└{line}┘"));
    out.join("\n")
}

fn write_private(path: &Path, text: &str) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_certificate_and_secret_are_kept() {
        let dir = std::env::temp_dir().join(format!("klue-identity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let first = certificate(&dir).unwrap();
        let again = certificate(&dir).unwrap();
        assert_eq!(first.fingerprint, again.fingerprint);
        assert_eq!(first.fingerprint.len(), 43);
        let s = secret(&dir).unwrap();
        assert_eq!(secret(&dir).unwrap(), s);
        assert_ne!(new_secret(&dir).unwrap(), s);
        let k = key(&s, &first.fingerprint);
        assert_eq!(Key::parse(&k.to_text()).map(|k| k.fingerprint), Some(first.fingerprint));
        assert!(banner(&k, 8443).contains(&k.to_text()));
    }
}
