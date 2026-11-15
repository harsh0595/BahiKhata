//! Encrypted vault file.
//!
//! Layout: "BKV1" | m_cost u32 | t_cost u32 | p_cost u32 | salt[16] | nonce[24] | ciphertext.
//! The key is derived from the app password with Argon2id; the data (JSON) is sealed with
//! XChaCha20-Poly1305 and the header is authenticated as associated data.
//! Nothing is ever written to disk unencrypted.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use std::path::Path;
use zeroize::Zeroizing;

const MAGIC: &[u8; 4] = b"BKV1";
const HEADER_LEN: usize = 4 + 12 + 16 + 24;
pub const WRONG_PASSWORD: &str = "Wrong password (or the data file is damaged).";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KdfParams {
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

impl KdfParams {
    pub const DEFAULT: KdfParams = KdfParams { m_kib: 64 * 1024, t: 3, p: 1 };
    /// Cheap parameters for unit tests only.
    pub const TEST: KdfParams = KdfParams { m_kib: 64, t: 1, p: 1 };
}

pub struct VaultKey {
    key: Zeroizing<[u8; 32]>,
    salt: [u8; 16],
    params: KdfParams,
}

impl VaultKey {
    pub fn derive(password: &str, salt: [u8; 16], params: KdfParams) -> Result<Self, String> {
        let p = Params::new(params.m_kib, params.t, params.p, Some(32)).map_err(|e| e.to_string())?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
        let mut key = Zeroizing::new([0u8; 32]);
        argon
            .hash_password_into(password.as_bytes(), &salt, &mut key[..])
            .map_err(|e| e.to_string())?;
        Ok(VaultKey { key, salt, params })
    }

    /// New key with a fresh random salt.
    pub fn create(password: &str, params: KdfParams) -> Result<Self, String> {
        let mut salt = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        Self::derive(password, salt, params)
    }

    fn header(&self, nonce: &[u8; 24]) -> Vec<u8> {
        let mut h = Vec::with_capacity(HEADER_LEN);
        h.extend_from_slice(MAGIC);
        h.extend_from_slice(&self.params.m_kib.to_le_bytes());
        h.extend_from_slice(&self.params.t.to_le_bytes());
        h.extend_from_slice(&self.params.p.to_le_bytes());
        h.extend_from_slice(&self.salt);
        h.extend_from_slice(nonce);
        h
    }

    pub fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce = [0u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let header = self.header(&nonce);
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&self.key[..]));
        let ct = cipher
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: plain, aad: &header })
            .map_err(|_| "Encryption failed".to_string())?;
        let mut out = header;
        out.extend_from_slice(&ct);
        Ok(out)
    }

    pub fn open(&self, data: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
        let (params, salt, nonce) = parse_header(data)?;
        if params != self.params || salt != self.salt {
            return Err(WRONG_PASSWORD.into());
        }
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&self.key[..]));
        let plain = cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload { msg: &data[HEADER_LEN..], aad: &data[..HEADER_LEN] },
            )
            .map_err(|_| WRONG_PASSWORD.to_string())?;
        Ok(Zeroizing::new(plain))
    }
}

fn parse_header(data: &[u8]) -> Result<(KdfParams, [u8; 16], [u8; 24]), String> {
    if data.len() < HEADER_LEN + 16 || &data[..4] != MAGIC {
        return Err("Not a Bahi-Khata data file.".into());
    }
    let u = |i: usize| u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
    let params = KdfParams { m_kib: u(4), t: u(8), p: u(12) };
    if params.m_kib == 0 || params.m_kib > 4 * 1024 * 1024 || params.t == 0 || params.t > 64 || params.p == 0 || params.p > 64 {
        return Err("Data file header is damaged.".into());
    }
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&data[16..32]);
    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&data[32..56]);
    Ok((params, salt, nonce))
}

/// Derive the key from the file's own header parameters and decrypt.
pub fn unlock(data: &[u8], password: &str) -> Result<(VaultKey, Zeroizing<Vec<u8>>), String> {
    let (params, salt, _) = parse_header(data)?;
    let key = VaultKey::derive(password, salt, params)?;
    let plain = key.open(data)?;
    Ok((key, plain))
}

/// Write atomically: write a temp file, keep the previous version as `.bak`, then rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create data folder: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| format!("Cannot write data file: {e}"))?;
        f.write_all(bytes).map_err(|e| format!("Cannot write data file: {e}"))?;
        f.sync_all().map_err(|e| format!("Cannot write data file: {e}"))?;
    }
    if path.exists() {
        let bak = path.with_extension("bak");
        let _ = std::fs::remove_file(&bak);
        std::fs::rename(path, &bak).map_err(|e| format!("Cannot keep backup: {e}"))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("Cannot save data file: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_wrong_password() {
        let key = VaultKey::create("correct horse", KdfParams::TEST).unwrap();
        let sealed = key.seal(b"{\"hello\":1}").unwrap();
        assert!(!sealed.windows(5).any(|w| w == b"hello"));
        let (_, plain) = unlock(&sealed, "correct horse").unwrap();
        assert_eq!(plain.as_slice(), b"{\"hello\":1}");
        assert_eq!(unlock(&sealed, "wrong").err().unwrap(), WRONG_PASSWORD);
        assert_eq!(key.open(&sealed).unwrap().as_slice(), b"{\"hello\":1}");
    }

    #[test]
    fn tamper_detected() {
        let key = VaultKey::create("pw", KdfParams::TEST).unwrap();
        let mut sealed = key.seal(b"some data here").unwrap();
        let n = sealed.len();
        sealed[n - 1] ^= 1;
        assert!(unlock(&sealed, "pw").is_err());
        assert!(unlock(b"garbage", "pw").is_err());
    }

    #[test]
    fn atomic_write_keeps_backup() {
        let dir = std::env::temp_dir().join(format!("bk-test-{}", std::process::id()));
        let path = dir.join("vault.bkv");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        assert_eq!(std::fs::read(path.with_extension("bak")).unwrap(), b"one");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
