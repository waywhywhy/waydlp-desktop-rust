//! Offline installers: the application archive appended to the installer
//! executable.
//!
//! Layout: `[installer exe][payload zip][version (utf-8)][u32 LE version len][u64 LE payload len][8-byte MAGIC]`.
//! The payload is still verified: its SHA-256 is stored next to it and
//! checked before extraction to detect corruption.

use crate::InstallError;
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const MAGIC: &[u8; 8] = b"WAYDLPPL";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload {
    pub version: String,
    pub sha256: String,
    pub data: Vec<u8>,
}

/// Append `payload` (a zip) to a copy of `stub`, producing an offline installer.
pub fn bundle(stub: &Path, payload: &Path, version: &str, out: &Path) -> Result<(), InstallError> {
    let mut o = std::fs::File::create(out)?;
    std::io::copy(&mut std::fs::File::open(stub)?, &mut o)?;
    let data = std::fs::read(payload)?;
    let sha = hex::encode(Sha256::digest(&data));
    let meta = format!("{version}\n{sha}");
    o.write_all(&data)?;
    o.write_all(meta.as_bytes())?;
    o.write_all(&(meta.len() as u32).to_le_bytes())?;
    o.write_all(&(data.len() as u64).to_le_bytes())?;
    o.write_all(MAGIC)?;
    o.sync_all()?;
    Ok(())
}

/// Read a payload appended to `exe`, if any.
pub fn read_from(exe: &Path) -> Result<Option<Payload>, InstallError> {
    let mut f = std::fs::File::open(exe)?;
    let len = f.metadata()?.len();
    if len < 20 {
        return Ok(None);
    }
    let mut trailer = [0u8; 20];
    f.seek(SeekFrom::End(-20))?;
    f.read_exact(&mut trailer)?;
    if &trailer[12..] != MAGIC {
        return Ok(None);
    }
    let meta_len = u32::from_le_bytes(trailer[0..4].try_into().expect("4 bytes")) as u64;
    let data_len = u64::from_le_bytes(trailer[4..12].try_into().expect("8 bytes"));
    let total = data_len
        .checked_add(meta_len)
        .and_then(|v| v.checked_add(20))
        .ok_or_else(|| InstallError::Archive("corrupt payload header".into()))?;
    if total > len || meta_len > 4096 {
        return Err(InstallError::Archive("corrupt payload header".into()));
    }
    f.seek(SeekFrom::End(-(total as i64)))?;
    let mut data = vec![0u8; data_len as usize];
    f.read_exact(&mut data)?;
    let mut meta = vec![0u8; meta_len as usize];
    f.read_exact(&mut meta)?;
    let meta = String::from_utf8_lossy(&meta).to_string();
    let (version, sha) =
        meta.split_once('\n').ok_or_else(|| InstallError::Archive("corrupt payload metadata".into()))?;
    let actual = hex::encode(Sha256::digest(&data));
    if actual != sha {
        return Err(InstallError::ChecksumMismatch { file: "embedded payload".into(), expected: sha.into(), actual });
    }
    Ok(Some(Payload { version: version.to_string(), sha256: actual, data }))
}

/// Payload of the running executable.
pub fn current() -> Result<Option<Payload>, InstallError> {
    read_from(&std::env::current_exe()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_and_read() {
        let d = tempfile::tempdir().unwrap();
        let stub = d.path().join("stub.exe");
        let payload = d.path().join("p.zip");
        let out = d.path().join("setup.exe");
        std::fs::write(&stub, b"MZ fake stub").unwrap();
        std::fs::write(&payload, b"PK fake zip data").unwrap();
        assert!(read_from(&stub).unwrap().is_none());
        bundle(&stub, &payload, "1.2.3", &out).unwrap();
        let p = read_from(&out).unwrap().unwrap();
        assert_eq!(p.version, "1.2.3");
        assert_eq!(p.data, b"PK fake zip data");
    }

    #[test]
    fn detects_corruption() {
        let d = tempfile::tempdir().unwrap();
        let stub = d.path().join("stub.exe");
        let payload = d.path().join("p.zip");
        let out = d.path().join("setup.exe");
        std::fs::write(&stub, b"MZ").unwrap();
        std::fs::write(&payload, b"PK payload").unwrap();
        bundle(&stub, &payload, "1.0.0", &out).unwrap();
        let mut bytes = std::fs::read(&out).unwrap();
        bytes[3] ^= 0xff; // flip a payload byte
        std::fs::write(&out, bytes).unwrap();
        assert!(matches!(read_from(&out), Err(InstallError::ChecksumMismatch { .. })));
    }
}
