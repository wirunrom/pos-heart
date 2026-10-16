//! Encrypted cloud backups (multi-device phase 2). A recovery bundle (the
//! same `.posbackup` folder as local backups) is packed and sealed on this
//! computer with a key derived from the owner's backup password, so the
//! cloud stores only ciphertext. Restoring decrypts back into a folder and
//! goes through the ordinary recovery review.
//!
//! Format `.phcloud`: "PHCLOUD1" | Argon2id m (KiB), t, p as u32 LE | salt
//! (16) | nonce (12) | ChaCha20-Poly1305(packed files) with the header as
//! associated data. Packed files: repeated (name length u16 LE, UTF-8 name,
//! size u64 LE, bytes). Only the derived key is kept on the main register
//! (`cloud.json`); the password itself is never stored.
use ring::{
    aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};
use std::{fs, io, path::Path};

pub const EXTENSION: &str = "phcloud";
const MAGIC: &[u8; 8] = b"PHCLOUD1";
/// Argon2id cost: 64 MiB, 3 passes, 1 lane (about a second on a laptop).
const M_KIB: u32 = 64 * 1024;
const T: u32 = 3;
const P: u32 = 1;
const SALT: usize = 16;
const HEADER: usize = 8 + 12 + SALT + NONCE_LEN;
pub const MIN_PASSWORD: usize = 10;

fn bad(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

/// A derived backup key and the salt it was derived with.
#[derive(Clone, PartialEq, Eq)]
pub struct Key {
    pub salt: [u8; SALT],
    key: [u8; 32],
}
impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key(…)")
    }
}
fn derive_with(password: &str, salt: &[u8], m: u32, t: u32, p: u32) -> io::Result<[u8; 32]> {
    let params = argon2::Params::new(m, t, p, Some(32)).map_err(|_| bad("bad key parameters"))?;
    let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| bad("cannot derive the key"))?;
    Ok(key)
}
impl Key {
    /// A new key from the owner's password (slow on purpose: run it off the UI thread).
    pub fn new(password: &str) -> io::Result<Self> {
        if password.chars().count() < MIN_PASSWORD {
            return Err(bad("the backup password needs at least 10 characters"));
        }
        let mut salt = [0u8; SALT];
        SystemRandom::new()
            .fill(&mut salt)
            .map_err(|_| bad("no random source"))?;
        Ok(Self {
            key: derive_with(password, &salt, M_KIB, T, P)?,
            salt,
        })
    }
    /// For `cloud.json`: salt and key as hex.
    pub fn to_hex(&self) -> (String, String) {
        (hex(&self.salt), hex(&self.key))
    }
    pub fn from_hex(salt: &str, key: &str) -> Option<Self> {
        Some(Self {
            salt: unhex(salt)?.try_into().ok()?,
            key: unhex(key)?.try_into().ok()?,
        })
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}
fn aead(key: &[u8; 32]) -> io::Result<LessSafeKey> {
    Ok(LessSafeKey::new(
        UnboundKey::new(&CHACHA20_POLY1305, key).map_err(|_| bad("bad key"))?,
    ))
}

/// Packs and seals every file of the recovery bundle `dir`.
pub fn seal(key: &Key, dir: &Path) -> io::Result<Vec<u8>> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    let mut plain = Vec::new();
    for entry in entries {
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let bytes = fs::read(entry.path())?;
        let len = u16::try_from(name.len()).map_err(|_| bad("file name too long"))?;
        plain.extend_from_slice(&len.to_le_bytes());
        plain.extend_from_slice(name.as_bytes());
        plain.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        plain.extend_from_slice(&bytes);
    }
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| bad("no random source"))?;
    let mut header = Vec::with_capacity(HEADER);
    header.extend_from_slice(MAGIC);
    for n in [M_KIB, T, P] {
        header.extend_from_slice(&n.to_le_bytes());
    }
    header.extend_from_slice(&key.salt);
    header.extend_from_slice(&nonce);
    aead(&key.key)?
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(&header),
            &mut plain,
        )
        .map_err(|_| bad("cannot encrypt"))?;
    header.extend_from_slice(&plain);
    Ok(header)
}

/// True when `bytes` start like a cloud backup.
pub fn is_backup(bytes: &[u8]) -> bool {
    bytes.len() > HEADER && bytes.starts_with(MAGIC)
}

/// Decrypts `bytes` with `password` into the new folder `dest`. A wrong
/// password or a damaged file is refused and nothing is written.
pub fn open(bytes: &[u8], password: &str, dest: &Path) -> io::Result<()> {
    if !is_backup(bytes) {
        return Err(bad("this is not a POS Heart cloud backup"));
    }
    let word = |i: usize| -> io::Result<u32> {
        let b: [u8; 4] = bytes
            .get(8 + i * 4..12 + i * 4)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| bad("damaged backup"))?;
        Ok(u32::from_le_bytes(b))
    };
    let (m, t, p) = (word(0)?, word(1)?, word(2)?);
    // Refuse absurd costs from a crafted file (at most 1 GiB, 10 passes).
    if m > 1024 * 1024 || t > 10 || p > 8 {
        return Err(bad("damaged backup"));
    }
    let salt = &bytes[20..20 + SALT];
    let nonce: [u8; NONCE_LEN] = bytes[20 + SALT..HEADER]
        .try_into()
        .map_err(|_| bad("damaged backup"))?;
    let key = derive_with(password, salt, m, t, p)?;
    let mut sealed = bytes[HEADER..].to_vec();
    let plain = aead(&key)?
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(&bytes[..HEADER]),
            &mut sealed,
        )
        .map_err(|_| bad("wrong backup password or damaged file"))?;
    let mut files = Vec::new();
    let mut rest: &[u8] = plain;
    while !rest.is_empty() {
        let take = |rest: &mut &[u8], n: usize| -> io::Result<Vec<u8>> {
            if rest.len() < n {
                return Err(bad("damaged backup"));
            }
            let (head, tail) = rest.split_at(n);
            *rest = tail;
            Ok(head.to_vec())
        };
        let len = u16::from_le_bytes(
            take(&mut rest, 2)?
                .try_into()
                .map_err(|_| bad("damaged backup"))?,
        );
        let name = String::from_utf8(take(&mut rest, usize::from(len))?)
            .map_err(|_| bad("damaged backup"))?;
        // Plain file names only: nothing may land outside the folder.
        if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
            return Err(bad("damaged backup"));
        }
        let size = u64::from_le_bytes(
            take(&mut rest, 8)?
                .try_into()
                .map_err(|_| bad("damaged backup"))?,
        );
        let size = usize::try_from(size).map_err(|_| bad("damaged backup"))?;
        files.push((name, take(&mut rest, size)?));
    }
    fs::create_dir(dest)?;
    let written = files
        .iter()
        .try_for_each(|(name, bytes)| fs::write(dest.join(name), bytes));
    if written.is_err() {
        let _ = fs::remove_dir_all(dest);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    type Result = std::result::Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn a_sealed_bundle_opens_only_with_its_password() -> Result {
        let dir = tempfile::tempdir()?;
        let bundle = dir.path().join("b.posbackup");
        fs::create_dir(&bundle)?;
        fs::write(bundle.join("pos-heart.sqlite3"), b"database bytes")?;
        fs::write(bundle.join("shop.json"), "{\"name\":\"ร้าน\"}")?;
        let key = Key::new("correct horse battery")?;
        let sealed = seal(&key, &bundle)?;
        assert!(is_backup(&sealed));
        assert!(
            !sealed.windows(14).any(|w| w == b"database bytes"),
            "encrypted"
        );
        // The stored key round-trips through cloud.json.
        let (salt, k) = key.to_hex();
        assert_eq!(Key::from_hex(&salt, &k), Some(key.clone()));
        let out = dir.path().join("out.posbackup");
        assert!(open(&sealed, "wrong password!", &out).is_err());
        assert!(!out.exists());
        open(&sealed, "correct horse battery", &out)?;
        assert_eq!(fs::read(out.join("pos-heart.sqlite3"))?, b"database bytes");
        assert_eq!(
            fs::read_to_string(out.join("shop.json"))?,
            "{\"name\":\"ร้าน\"}"
        );
        // Any changed byte is refused.
        let mut tampered = sealed.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(open(&tampered, "correct horse battery", &dir.path().join("t")).is_err());
        assert!(Key::new("short").is_err());
        Ok(())
    }
}
