//! The server's passwords and channel secrets (bot token, SMTP password, webhook
//! addresses), each sealed with XChaCha20-Poly1305 in `vault.json`.
//!
//! The key is `vault.key` next to it, made on the first start, or `KLUE_VAULT_KEY`
//! (base64) to keep it out of the data volume. The server has to read passwords to sign in
//! by itself, so this protects copies and backups of the data, not a running server.

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use klue_core::store::{Account, VaultAccount};
use klue_engine::PasswordStore;
use serde::{Deserialize, Serialize};

const FILE: &str = "vault.json";
const KEY_FILE: &str = "vault.key";
const NONCE: usize = 24;

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    /// Sealed values by name: `line:<carrier:number>` for passwords, else a setting's name.
    entries: BTreeMap<String, String>,
    /// The lines with passwords, so they can come back if the data file is lost.
    #[serde(default)]
    index: Vec<VaultAccount>,
}

pub struct Vault {
    path: PathBuf,
    cipher: XChaCha20Poly1305,
    saved: Mutex<Saved>,
}

impl Vault {
    pub fn open(dir: &Path) -> io::Result<Self> {
        let key = match std::env::var("KLUE_VAULT_KEY") {
            Ok(text) => B64.decode(text.trim()).map_err(|_| io::Error::other("KLUE_VAULT_KEY isn't base64"))?,
            Err(_) => {
                let path = dir.join(KEY_FILE);
                match std::fs::read_to_string(&path) {
                    Ok(text) => B64.decode(text.trim()).map_err(|_| io::Error::other(format!("{} is damaged", path.display())))?,
                    Err(_) => {
                        let key: [u8; 32] = rand::random();
                        write_private(&path, B64.encode(key).as_bytes())?;
                        key.to_vec()
                    }
                }
            }
        };
        let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|_| io::Error::other("the vault key must be 32 bytes"))?;
        let path = dir.join(FILE);
        let saved = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| io::Error::other(format!("{} is damaged: {e}", path.display())))?,
            Err(_) => Saved::default(),
        };
        Ok(Self { path, cipher, saved: Mutex::new(saved) })
    }

    fn seal(&self, name: &str, value: &str) -> Option<String> {
        let nonce: [u8; NONCE] = rand::random();
        let sealed = self.cipher.encrypt(&XNonce::from(nonce), Payload { msg: value.as_bytes(), aad: name.as_bytes() }).ok()?;
        let mut out = nonce.to_vec();
        out.extend(sealed);
        Some(B64.encode(out))
    }

    fn unseal(&self, name: &str, sealed: &str) -> Option<String> {
        let bytes = B64.decode(sealed).ok()?;
        let (nonce, data) = bytes.split_at_checked(NONCE)?;
        let nonce: [u8; NONCE] = nonce.try_into().ok()?;
        let open = self.cipher.decrypt(&XNonce::from(nonce), Payload { msg: data, aad: name.as_bytes() }).ok()?;
        String::from_utf8(open).ok()
    }

    pub fn get(&self, name: &str) -> Option<String> {
        let sealed = self.saved.lock().ok()?.entries.get(name).cloned()?;
        self.unseal(name, &sealed)
    }

    /// `None` removes it.
    pub fn set(&self, name: &str, value: Option<&str>) -> bool {
        let sealed = match value {
            Some(value) => match self.seal(name, value) {
                Some(s) => Some(s),
                None => return false,
            },
            None => None,
        };
        {
            let Ok(mut saved) = self.saved.lock() else { return false };
            match sealed {
                Some(s) => saved.entries.insert(name.to_string(), s),
                None => saved.entries.remove(name),
            };
        }
        self.save()
    }

    /// The settings secrets that are set (not the passwords).
    pub fn names(&self) -> BTreeSet<String> {
        self.saved.lock().map(|s| s.entries.keys().filter(|k| !k.starts_with("line:")).cloned().collect()).unwrap_or_default()
    }

    fn save(&self) -> bool {
        let Ok(saved) = self.saved.lock() else { return false };
        let Ok(bytes) = serde_json::to_vec_pretty(&*saved) else { return false };
        match write_private(&self.path, &bytes) {
            Ok(()) => true,
            Err(e) => {
                tracing::error!("couldn't save {}: {e}", self.path.display());
                false
            }
        }
    }
}

/// Writes atomically, readable by the server's user only.
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(tmp, path)
}

/// The vault as the engine's password store.
pub struct Passwords(pub Arc<Vault>);

fn line_key(account: &Account) -> String {
    format!("line:{}", account.credential_user())
}

impl PasswordStore for Passwords {
    fn get(&self, account: &Account) -> Option<String> {
        self.0.get(&line_key(account))
    }

    fn set(&self, account: &Account, password: &str) -> bool {
        self.0.set(&line_key(account), Some(password))
    }

    fn delete(&self, account: &Account) {
        self.0.set(&line_key(account), None);
    }

    fn save_index(&self, accounts: &[Account]) {
        if let Ok(mut saved) = self.0.saved.lock() {
            saved.index = accounts.iter().map(VaultAccount::of).collect();
        }
        self.0.save();
    }

    fn load_index(&self) -> Vec<VaultAccount> {
        self.0.saved.lock().map(|s| s.index.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use klue_core::{Carrier, LineType};

    use super::*;

    #[test]
    fn secrets_are_sealed_and_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("klue-vault-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let vault = Arc::new(Vault::open(&dir).unwrap());
        let passwords = Passwords(vault.clone());
        let line = Account::new(Carrier::We, LineType::Landline, "0223584417".into());
        assert!(passwords.set(&line, "hunter2"));
        assert!(vault.set("telegram.token", Some("123:abc")));
        let file = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(!file.contains("hunter2") && !file.contains("123:abc"));
        assert_eq!(vault.names(), ["telegram.token".to_string()].into());

        let again = Passwords(Arc::new(Vault::open(&dir).unwrap()));
        assert_eq!(again.get(&line).as_deref(), Some("hunter2"));
        assert_eq!(again.0.get("telegram.token").as_deref(), Some("123:abc"));
        again.delete(&line);
        assert_eq!(again.get(&line), None);
    }
}
