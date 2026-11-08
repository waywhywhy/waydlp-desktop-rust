//! Safe extraction of `.zip` and `.tar.gz` archives.
//!
//! Every entry path is validated: absolute paths, `..` components and
//! symlinks pointing outside the destination are rejected ("zip slip").

use crate::InstallError;
use std::path::{Component, Path, PathBuf};

/// Validate an archive entry path and return it relative to the destination.
pub fn safe_relative(name: &Path) -> Result<PathBuf, InstallError> {
    let mut out = PathBuf::new();
    for c in name.components() {
        match c {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(InstallError::Archive(format!("unsafe path in archive: {}", name.display())));
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(InstallError::Archive("empty path in archive".into()));
    }
    Ok(out)
}

/// Strip a single common top-level directory (e.g. `waydlp-1.0.0/…`).
fn strip_prefix(paths: &[PathBuf]) -> Option<PathBuf> {
    let first = paths.first()?.components().next()?;
    let Component::Normal(top) = first else { return None };
    let all_under = paths.iter().all(|p| {
        let mut c = p.components();
        c.next() == Some(Component::Normal(top)) && c.next().is_some() || p == Path::new(top)
    });
    if all_under && paths.iter().any(|p| p.components().count() > 1) { Some(PathBuf::from(top)) } else { None }
}

/// Extract a zip archive into `dest`. Returns the extracted files (relative).
pub fn extract_zip(data: &[u8], dest: &Path) -> Result<Vec<PathBuf>, InstallError> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| InstallError::Archive(e.to_string()))?;
    let mut names = Vec::new();
    for i in 0..z.len() {
        let f = z.by_index(i).map_err(|e| InstallError::Archive(e.to_string()))?;
        let raw = f.name().map_err(|e| InstallError::Archive(e.to_string()))?.to_string();
        names.push(safe_relative(Path::new(&raw.replace('\\', "/")))?);
    }
    let prefix = strip_prefix(&names);
    let mut out = Vec::new();
    for (i, rel) in names.iter().enumerate() {
        let mut f = z.by_index(i).map_err(|e| InstallError::Archive(e.to_string()))?;
        let rel = match &prefix {
            Some(p) => match rel.strip_prefix(p) {
                Ok(r) if !r.as_os_str().is_empty() => r.to_path_buf(),
                _ => continue,
            },
            None => rel.clone(),
        };
        let target = dest.join(&rel);
        if f.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut w = std::fs::File::create(&target)?;
        std::io::copy(&mut f, &mut w)?;
        #[cfg(unix)]
        if let Some(mode) = f.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode & 0o755))?;
        }
        out.push(rel);
    }
    Ok(out)
}

/// Extract a `.tar.gz` archive into `dest`. Returns the extracted files.
pub fn extract_tar_gz(data: &[u8], dest: &Path) -> Result<Vec<PathBuf>, InstallError> {
    let mut paths = Vec::new();
    {
        let mut a = tar::Archive::new(flate2::read::GzDecoder::new(data));
        for e in a.entries()? {
            let e = e?;
            paths.push(safe_relative(&e.path()?)?);
        }
    }
    let prefix = strip_prefix(&paths);
    let mut a = tar::Archive::new(flate2::read::GzDecoder::new(data));
    a.set_preserve_permissions(true);
    let mut out = Vec::new();
    for (i, e) in a.entries()?.enumerate() {
        let mut e = e?;
        let rel = &paths[i];
        let rel = match &prefix {
            Some(p) => match rel.strip_prefix(p) {
                Ok(r) if !r.as_os_str().is_empty() => r.to_path_buf(),
                _ => continue,
            },
            None => rel.clone(),
        };
        let kind = e.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            // Links are not needed for waydlp and could escape `dest`.
            continue;
        }
        let target = dest.join(&rel);
        if kind.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        e.unpack(&target)?;
        out.push(rel);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn rejects_traversal() {
        assert!(safe_relative(Path::new("../evil")).is_err());
        assert!(safe_relative(Path::new("a/../../evil")).is_err());
        assert!(safe_relative(Path::new("/etc/passwd")).is_err());
        assert_eq!(safe_relative(Path::new("./a/b")).unwrap(), PathBuf::from("a/b"));
    }

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            for (n, d) in entries {
                z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(d).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn zip_strips_top_folder() {
        let data = make_zip(&[("waydlp-1.0.0/waydlp.exe", b"x"), ("waydlp-1.0.0/README.txt", b"r")]);
        let d = tempfile::tempdir().unwrap();
        let files = extract_zip(&data, d.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(d.path().join("waydlp.exe").exists());
    }

    #[test]
    fn zip_without_folder() {
        let data = make_zip(&[("waydlp.exe", b"x"), ("docs/a.txt", b"r")]);
        let d = tempfile::tempdir().unwrap();
        extract_zip(&data, d.path()).unwrap();
        assert!(d.path().join("waydlp.exe").exists());
        assert!(d.path().join("docs/a.txt").exists());
    }

    #[test]
    fn zip_rejects_slip() {
        let data = make_zip(&[("../../evil.txt", b"x")]);
        let d = tempfile::tempdir().unwrap();
        assert!(extract_zip(&data, d.path()).is_err());
    }

    #[test]
    fn tar_gz_roundtrip() {
        let mut gz = Vec::new();
        {
            let enc = flate2::write::GzEncoder::new(&mut gz, flate2::Compression::fast());
            let mut t = tar::Builder::new(enc);
            for (n, d) in
                [("waydlp-1.0.0/waydlp", b"bin".as_slice()), ("waydlp-1.0.0/share/icon.png", b"png".as_slice())]
            {
                let mut h = tar::Header::new_gnu();
                h.set_size(d.len() as u64);
                h.set_mode(0o755);
                h.set_cksum();
                t.append_data(&mut h, n, d).unwrap();
            }
            t.into_inner().unwrap().finish().unwrap();
        }
        let d = tempfile::tempdir().unwrap();
        let files = extract_tar_gz(&gz, d.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(std::fs::read(d.path().join("waydlp")).unwrap(), b"bin");
    }
}
