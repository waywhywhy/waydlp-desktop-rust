//! Install / uninstall / verify orchestration.

use crate::payload::Payload;
use crate::receipt::{FileRecord, Receipt};
use crate::release::{self, Release};
use crate::system::{self, Elevation};
use crate::{InstallError, Target};
use std::path::{Path, PathBuf};
use waydlp_core::artifacts::{Os, PackageKind};

/// Where the application files come from.
#[derive(Debug, Clone)]
pub enum Source {
    /// Archive appended to the installer executable (offline setup.exe).
    Embedded(Payload),
    /// A local artifact file chosen by the user.
    File(PathBuf),
    /// Download from GitHub Releases (`None` = latest stable).
    Release { version: Option<String> },
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub method: PackageKind,
    pub source: Source,
    /// For user-level archive installs; `None` = default location.
    pub install_dir: Option<PathBuf>,
    pub desktop_shortcut: bool,
    pub menu_entry: bool,
    pub assume_yes: bool,
    pub elevation: Elevation,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    Status(String),
    Download { done: u64, total: Option<u64> },
    Files { done: usize, total: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub receipt: Receipt,
    pub ok: Vec<PathBuf>,
    pub missing: Vec<PathBuf>,
    pub modified: Vec<PathBuf>,
    pub package_installed: Option<bool>,
}

impl VerifyReport {
    pub fn is_ok(&self) -> bool {
        self.missing.is_empty() && self.modified.is_empty() && self.package_installed != Some(false)
    }
}

pub struct Installer {
    pub target: Target,
}

impl Installer {
    pub fn new() -> Result<Self, InstallError> {
        Ok(Self { target: Target::detect()? })
    }

    pub fn default_install_dir(&self) -> Result<PathBuf, InstallError> {
        match self.target.os {
            #[cfg(windows)]
            Os::Windows => crate::desktop::windows::default_install_dir(),
            #[cfg(not(windows))]
            Os::Windows => Err(InstallError::UnsupportedPlatform("windows".into())),
            Os::Linux => crate::desktop::linux::default_install_dir(),
        }
    }

    fn work_dir() -> Result<PathBuf, InstallError> {
        Ok(crate::receipt::receipt_dir()?.join("downloads"))
    }

    /// Obtain the artifact file for `kind` from the chosen source.
    fn obtain(
        &self,
        source: &Source,
        kind: PackageKind,
        progress: &dyn Fn(Progress),
    ) -> Result<(PathBuf, String), InstallError> {
        match source {
            Source::File(p) => {
                if !p.is_file() {
                    return Err(InstallError::Other(format!("{} does not exist", p.display())));
                }
                let version =
                    waydlp_core::artifacts::parse_artifact_name(&p.file_name().unwrap_or_default().to_string_lossy())
                        .map(|a| a.version)
                        .unwrap_or_else(|| "unknown".into());
                progress(Progress::Status(format!("Using local file {}", p.display())));
                Ok((p.clone(), version))
            }
            Source::Embedded(payload) => {
                let dir = Self::work_dir()?;
                std::fs::create_dir_all(&dir)?;
                let p = dir.join("embedded-payload.zip");
                std::fs::write(&p, &payload.data)?;
                Ok((p, payload.version.clone()))
            }
            Source::Release { version } => {
                progress(Progress::Status("Fetching release information from GitHub…".into()));
                let rel: Release = release::fetch(version.as_deref())?;
                let asset = rel.asset_for(kind, self.target.arch)?.clone();
                progress(Progress::Status(format!("Downloading {}…", asset.name)));
                let p = release::download_verified(&rel, &asset, &Self::work_dir()?, &|done, total| {
                    progress(Progress::Download { done, total })
                })?;
                progress(Progress::Status("Checksum verified.".into()));
                Ok((p, rel.version))
            }
        }
    }

    pub fn install(&self, opts: &InstallOptions, progress: &dyn Fn(Progress)) -> Result<Receipt, InstallError> {
        if opts.method.os() != self.target.os {
            return Err(InstallError::UnsupportedMethod(
                opts.method,
                format!("it is for {}", opts.method.os().as_str()),
            ));
        }
        if matches!(opts.source, Source::Embedded(_))
            && !matches!(opts.method, PackageKind::WindowsPortable | PackageKind::TarGz)
        {
            return Err(InstallError::UnsupportedMethod(
                opts.method,
                "the embedded payload is the portable archive".into(),
            ));
        }
        match opts.method {
            PackageKind::WindowsPortable | PackageKind::TarGz => self.install_archive(opts, progress),
            PackageKind::AppImage => self.install_appimage(opts, progress),
            PackageKind::Msi => self.install_msi(opts, progress),
            PackageKind::Deb | PackageKind::Rpm | PackageKind::ArchPkg | PackageKind::Flatpak => {
                self.install_system(opts, progress)
            }
            PackageKind::WindowsSetup => Err(InstallError::UnsupportedMethod(
                opts.method,
                "run the setup .exe directly; it is this installer with the app embedded".into(),
            )),
        }
    }

    /// Remove a previous user-level install recorded in a receipt.
    fn remove_previous_files(r: &Receipt) {
        for f in &r.files {
            let _ = std::fs::remove_file(&f.path);
        }
        for p in &r.extra_paths {
            let _ = std::fs::remove_file(p);
        }
    }

    fn install_archive(&self, opts: &InstallOptions, progress: &dyn Fn(Progress)) -> Result<Receipt, InstallError> {
        let dir = match &opts.install_dir {
            Some(d) => d.clone(),
            None => self.default_install_dir()?,
        };
        if let Some(old) = Receipt::load()? {
            if !matches!(old.method, PackageKind::WindowsPortable | PackageKind::TarGz) {
                return Err(InstallError::Other(format!(
                    "waydlp is already installed as {}. Uninstall it first with `waydlp-installer uninstall`.",
                    old.method.display_name()
                )));
            }
            progress(Progress::Status(format!("Upgrading existing installation ({})…", old.version)));
            Self::remove_previous_files(&old);
        }
        let (file, version) = self.obtain(&opts.source, opts.method, progress)?;
        let data = std::fs::read(&file)?;

        progress(Progress::Status("Extracting…".into()));
        let staging = Self::work_dir()?.join("staging");
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging)?;
        let files = if file.extension().is_some_and(|e| e == "zip") {
            crate::archive::extract_zip(&data, &staging)?
        } else {
            crate::archive::extract_tar_gz(&data, &staging)?
        };

        progress(Progress::Status(format!("Installing to {}…", dir.display())));
        std::fs::create_dir_all(&dir)?;
        let mut records = Vec::new();
        let total = files.len();
        for (i, rel) in files.iter().enumerate() {
            if rel.as_os_str() == waydlp_platform_marker() {
                continue; // installed copies are not portable
            }
            let src = staging.join(rel);
            let dst = dir.join(rel);
            if let Some(p) = dst.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(&src, &dst).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    InstallError::Other(format!(
                        "Cannot write {} — is waydlp still running? Close it and try again.",
                        dst.display()
                    ))
                } else {
                    e.into()
                }
            })?;
            records.push(FileRecord { path: dst.clone(), sha256: release::sha256_file(&dst)? });
            progress(Progress::Files { done: i + 1, total });
        }
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_file(Self::work_dir()?.join("embedded-payload.zip"));

        // Mark how this copy was installed (used by waydlp's update advice).
        let marker = dir.join("install-kind");
        std::fs::write(&marker, if self.target.os == Os::Windows { "installer" } else { "tarball" })?;
        records.push(FileRecord { path: marker.clone(), sha256: release::sha256_file(&marker)? });

        let mut extra = Vec::new();
        #[cfg(windows)]
        let mut registry_key = None;
        #[cfg(not(windows))]
        let registry_key = None;
        match self.target.os {
            Os::Windows => {
                #[cfg(windows)]
                {
                    use crate::desktop::windows as w;
                    let exe = dir.join("waydlp.exe");
                    // Keep a copy of this installer as the uninstaller.
                    if let Ok(me) = std::env::current_exe() {
                        let un = dir.join("waydlp-installer.exe");
                        if me != un {
                            // Copy only the installer stub, not an appended payload.
                            copy_without_payload(&me, &un)?;
                            // The archive may already contain a different build of
                            // the installer. Record only the final installed bytes.
                            records.retain(|record| record.path != un);
                            records.push(FileRecord { path: un.clone(), sha256: release::sha256_file(&un)? });
                        }
                    }
                    if opts.menu_entry {
                        let lnk = w::start_menu_dir()?.join("waydlp.lnk");
                        w::create_shortcut(&exe, &lnk)?;
                        extra.push(lnk);
                    }
                    if opts.desktop_shortcut
                        && let Some(d) = crate::desktop::desktop_dir()
                    {
                        let lnk = d.join("waydlp.lnk");
                        w::create_shortcut(&exe, &lnk)?;
                        extra.push(lnk);
                    }
                    let size_kb =
                        records.iter().filter_map(|r| std::fs::metadata(&r.path).ok()).map(|m| m.len()).sum::<u64>()
                            / 1024;
                    registry_key = Some(w::register_uninstall(&dir, &version, size_kb)?);
                }
            }
            Os::Linux => {
                let exe = dir.join("waydlp");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    for b in ["waydlp", "waydlp-cli", "waydlp-installer"] {
                        let p = dir.join(b);
                        if p.exists() {
                            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755))?;
                        }
                    }
                    extra.push(crate::desktop::linux::link_binary(&exe)?);
                }
                if opts.menu_entry {
                    extra.extend(crate::desktop::linux::integrate(&exe, opts.desktop_shortcut)?);
                }
            }
        }

        let receipt = Receipt {
            version,
            method: opts.method,
            install_dir: Some(dir),
            files: records,
            extra_paths: extra,
            registry_key,
            package: None,
            msi_path: None,
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        receipt.save()?;
        progress(Progress::Status("waydlp is installed.".into()));
        Ok(receipt)
    }

    fn install_appimage(&self, opts: &InstallOptions, progress: &dyn Fn(Progress)) -> Result<Receipt, InstallError> {
        let (file, version) = self.obtain(&opts.source, PackageKind::AppImage, progress)?;
        if let Some(old) = Receipt::load()? {
            if old.method != PackageKind::AppImage {
                return Err(InstallError::Other(format!(
                    "waydlp is already installed as {}. Uninstall it first.",
                    old.method.display_name()
                )));
            }
            Self::remove_previous_files(&old);
        }
        let dir = opts.install_dir.clone().map_or_else(crate::desktop::linux::applications_dir, Ok)?;
        std::fs::create_dir_all(&dir)?;
        let dest = dir.join("waydlp.AppImage");
        std::fs::copy(&file, &dest)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
        }
        let mut extra = Vec::new();
        if opts.menu_entry {
            extra.extend(crate::desktop::linux::integrate(&dest, opts.desktop_shortcut)?);
        }
        let receipt = Receipt {
            version,
            method: PackageKind::AppImage,
            install_dir: Some(dir),
            files: vec![FileRecord { sha256: release::sha256_file(&dest)?, path: dest }],
            extra_paths: extra,
            registry_key: None,
            package: None,
            msi_path: None,
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        receipt.save()?;
        progress(Progress::Status("waydlp AppImage installed.".into()));
        Ok(receipt)
    }

    fn install_msi(&self, opts: &InstallOptions, progress: &dyn Fn(Progress)) -> Result<Receipt, InstallError> {
        let (file, version) = self.obtain(&opts.source, PackageKind::Msi, progress)?;
        // Keep the MSI so it can be used for uninstall.
        let keep = crate::receipt::receipt_dir()?.join("waydlp.msi");
        std::fs::create_dir_all(keep.parent().expect("parent"))?;
        if file != keep {
            std::fs::copy(&file, &keep)?;
        }
        progress(Progress::Status("Starting Windows Installer…".into()));
        let mut cmd = vec!["msiexec".to_string(), "/i".into(), keep.display().to_string()];
        if opts.assume_yes {
            cmd.push("/qn".into());
        }
        system::run(&cmd)?;
        let receipt = Receipt {
            version,
            method: PackageKind::Msi,
            install_dir: None,
            files: vec![],
            extra_paths: vec![],
            registry_key: None,
            package: Some("waydlp".into()),
            msi_path: Some(keep),
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        receipt.save()?;
        Ok(receipt)
    }

    fn install_system(&self, opts: &InstallOptions, progress: &dyn Fn(Progress)) -> Result<Receipt, InstallError> {
        system::check_native(opts.method).map_err(|e| InstallError::UnsupportedMethod(opts.method, e))?;
        let (file, version) = self.obtain(&opts.source, opts.method, progress)?;
        let elevation = if system::is_root() { Elevation::None } else { opts.elevation };
        let cmd = system::install_command(opts.method, &file, opts.assume_yes, elevation, system::rpm_tool());
        progress(Progress::Status(format!("Running: {}", cmd.join(" "))));
        system::run(&cmd)?;
        let receipt = Receipt {
            version,
            method: opts.method,
            install_dir: None,
            files: vec![],
            extra_paths: vec![],
            registry_key: None,
            package: Some(if opts.method == PackageKind::Flatpak { crate::APP_ID.into() } else { "waydlp".into() }),
            msi_path: None,
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        receipt.save()?;
        progress(Progress::Status(format!(
            "Installed with {}. Future updates come from your package manager.",
            opts.method.display_name()
        )));
        Ok(receipt)
    }

    pub fn uninstall(
        &self,
        assume_yes: bool,
        elevation: Elevation,
        progress: &dyn Fn(Progress),
    ) -> Result<Receipt, InstallError> {
        let r = Receipt::load()?.ok_or_else(|| {
            InstallError::NotInstalled(
                crate::receipt::receipt_path().map(|p| p.display().to_string()).unwrap_or_default(),
            )
        })?;
        match r.method {
            PackageKind::Deb | PackageKind::Rpm | PackageKind::ArchPkg | PackageKind::Flatpak => {
                let elevation = if system::is_root() { Elevation::None } else { elevation };
                let arch_package = if r.method == PackageKind::ArchPkg { system::arch_package_owner() } else { None };
                let cmd = system::uninstall_command(
                    r.method,
                    assume_yes,
                    elevation,
                    system::rpm_tool(),
                    arch_package.as_deref().unwrap_or("waydlp"),
                );
                progress(Progress::Status(format!("Running: {}", cmd.join(" "))));
                system::run(&cmd)?;
            }
            PackageKind::Msi => {
                let msi = r.msi_path.clone().ok_or_else(|| {
                    InstallError::Other("the cached MSI is missing; uninstall waydlp from Settings → Apps".into())
                })?;
                let mut cmd = vec!["msiexec".to_string(), "/x".into(), msi.display().to_string()];
                if assume_yes {
                    cmd.push("/qn".into());
                }
                system::run(&cmd)?;
                let _ = std::fs::remove_file(msi);
            }
            _ => {
                progress(Progress::Status("Removing files…".into()));
                let total = r.files.len() + r.extra_paths.len();
                let me = std::env::current_exe().ok();
                let mut deferred_self = false;
                for (i, f) in r.files.iter().map(|f| &f.path).chain(r.extra_paths.iter()).enumerate() {
                    if me.as_deref() == Some(f.as_path()) {
                        deferred_self = true;
                        continue;
                    }
                    match std::fs::remove_file(f) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => {
                            return Err(InstallError::Other(format!(
                                "Cannot remove {}: {e}. Is waydlp still running?",
                                f.display()
                            )));
                        }
                    }
                    progress(Progress::Files { done: i + 1, total });
                }
                #[cfg(windows)]
                if let Some(k) = &r.registry_key {
                    crate::desktop::windows::unregister_uninstall(k)?;
                }
                if let Some(dir) = &r.install_dir {
                    remove_empty_dirs(dir);
                    #[cfg(windows)]
                    if deferred_self || dir.exists() {
                        crate::desktop::windows::schedule_dir_removal(dir)?;
                    }
                }
                let _ = deferred_self;
            }
        }
        Receipt::delete()?;
        if let Ok(d) = crate::receipt::receipt_dir() {
            remove_empty_dirs(&d);
        }
        progress(Progress::Status("waydlp was uninstalled. Your settings and download history were kept.".into()));
        Ok(r)
    }

    pub fn verify(&self) -> Result<VerifyReport, InstallError> {
        let r = Receipt::load()?.ok_or_else(|| {
            InstallError::NotInstalled(
                crate::receipt::receipt_path().map(|p| p.display().to_string()).unwrap_or_default(),
            )
        })?;
        let (mut ok, mut missing, mut modified) = (Vec::new(), Vec::new(), Vec::new());
        for f in &r.files {
            if !f.path.exists() {
                missing.push(f.path.clone());
            } else if release::sha256_file(&f.path)? != f.sha256 {
                modified.push(f.path.clone());
            } else {
                ok.push(f.path.clone());
            }
        }
        for p in &r.extra_paths {
            if p.symlink_metadata().is_err() {
                missing.push(p.clone());
            }
        }
        let package_installed = system::is_installed(r.method);
        Ok(VerifyReport { receipt: r, ok, missing, modified, package_installed })
    }
}

fn waydlp_platform_marker() -> &'static std::ffi::OsStr {
    std::ffi::OsStr::new("waydlp.portable")
}

fn remove_empty_dirs(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                remove_empty_dirs(&e.path());
            }
        }
    }
    let _ = std::fs::remove_dir(dir);
}

/// Copy an executable but drop an appended offline payload, if present.
pub fn copy_without_payload(src: &Path, dst: &Path) -> Result<(), InstallError> {
    let bytes = std::fs::read(src)?;
    let len = match crate::payload::read_from(src) {
        Ok(Some(p)) => {
            let meta = format!("{}\n{}", p.version, p.sha256).len();
            bytes.len() - p.data.len() - meta - 20
        }
        _ => bytes.len(),
    };
    std::fs::write(dst, &bytes[..len])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_strips_payload() {
        let d = tempfile::tempdir().unwrap();
        let stub = d.path().join("stub");
        let zip = d.path().join("p.zip");
        let setup = d.path().join("setup");
        let out = d.path().join("copy");
        std::fs::write(&stub, b"STUB-BINARY").unwrap();
        std::fs::write(&zip, b"PAYLOAD").unwrap();
        crate::payload::bundle(&stub, &zip, "1.0.0", &setup).unwrap();
        copy_without_payload(&setup, &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"STUB-BINARY");
    }

    #[test]
    fn verify_report_logic() {
        let r = Receipt {
            version: "1".into(),
            method: PackageKind::TarGz,
            install_dir: None,
            files: vec![],
            extra_paths: vec![],
            registry_key: None,
            package: None,
            msi_path: None,
            installed_at: String::new(),
        };
        let v =
            VerifyReport { receipt: r.clone(), ok: vec![], missing: vec![], modified: vec![], package_installed: None };
        assert!(v.is_ok());
        let v = VerifyReport {
            receipt: r,
            ok: vec![],
            missing: vec!["x".into()],
            modified: vec![],
            package_installed: None,
        };
        assert!(!v.is_ok());
    }
}
