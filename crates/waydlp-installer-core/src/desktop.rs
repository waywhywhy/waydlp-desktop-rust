//! Desktop integration for user-level installs: Start menu / desktop
//! shortcuts and the "Apps & features" entry on Windows; `.desktop` files,
//! icons and a `~/.local/bin` link on Linux.

use crate::{APP_ID, InstallError};
use std::path::{Path, PathBuf};

pub const ICON_PNG_256: &[u8] =
    include_bytes!("../../../assets/icons/hicolor/256x256/apps/io.github.waywhywhy.waydlp.png");

pub fn desktop_dir() -> Option<PathBuf> {
    directories::UserDirs::new().and_then(|u| u.desktop_dir().map(Path::to_path_buf))
}

pub fn home_dir() -> Result<PathBuf, InstallError> {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .ok_or_else(|| InstallError::Other("cannot determine the home directory".into()))
}

// ------------------------------------------------------------------ Windows

#[cfg(windows)]
pub mod windows {
    use super::*;

    pub const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\waydlp";

    pub fn default_install_dir() -> Result<PathBuf, InstallError> {
        directories::BaseDirs::new()
            .map(|b| b.data_local_dir().join("Programs").join("waydlp"))
            .ok_or_else(|| InstallError::Other("cannot determine %LOCALAPPDATA%".into()))
    }

    pub fn start_menu_dir() -> Result<PathBuf, InstallError> {
        directories::BaseDirs::new()
            .map(|b| b.config_dir().join(r"Microsoft\Windows\Start Menu\Programs"))
            .ok_or_else(|| InstallError::Other("cannot determine the Start menu folder".into()))
    }

    pub fn create_shortcut(target: &Path, lnk: &Path) -> Result<(), InstallError> {
        if let Some(p) = lnk.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut sl = mslnk::ShellLink::new(target).map_err(|e| InstallError::Other(format!("shortcut: {e}")))?;
        sl.set_working_dir(target.parent().map(|p| p.display().to_string()));
        sl.set_icon_location(Some(target.display().to_string()));
        sl.set_name(Some("waydlp — media downloader".into()));
        sl.create_lnk(lnk).map_err(|e| InstallError::Other(format!("shortcut: {e}")))?;
        Ok(())
    }

    pub fn register_uninstall(install_dir: &Path, version: &str, size_kb: u64) -> Result<String, InstallError> {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(UNINSTALL_KEY)?;
        let exe = install_dir.join("waydlp.exe");
        let uninstaller = install_dir.join("waydlp-installer.exe");
        key.set_value("DisplayName", &"waydlp")?;
        key.set_value("DisplayVersion", &version)?;
        key.set_value("Publisher", &"waydlp contributors")?;
        key.set_value("InstallLocation", &install_dir.display().to_string())?;
        key.set_value("DisplayIcon", &exe.display().to_string())?;
        key.set_value("UninstallString", &format!("\"{}\" uninstall", uninstaller.display()))?;
        key.set_value("QuietUninstallString", &format!("\"{}\" uninstall --yes", uninstaller.display()))?;
        key.set_value("URLInfoAbout", &format!("https://github.com/{}", crate::REPO))?;
        key.set_value("NoModify", &1u32)?;
        key.set_value("NoRepair", &1u32)?;
        key.set_value("EstimatedSize", &(size_kb.min(u32::MAX as u64) as u32))?;
        key.set_value("InstallDate", &chrono::Local::now().format("%Y%m%d").to_string())?;
        Ok(UNINSTALL_KEY.to_string())
    }

    pub fn unregister_uninstall(key: &str) -> Result<(), InstallError> {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        match RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(key) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete a directory that contains the running executable: remove
    /// everything else now and let a short-lived `cmd` remove the rest
    /// after this process has exited.
    pub fn schedule_dir_removal(dir: &Path) -> Result<(), InstallError> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        let d = dir.display().to_string();
        if d.contains('"') || d.contains('&') || d.contains('|') || d.contains('%') {
            return Err(InstallError::Other(format!("refusing to schedule removal of unusual path {d}")));
        }
        // cmd.exe has its own quoting rules, so pass the command line verbatim:
        // `/S /C "<command>"` strips exactly the outer quotes.
        std::process::Command::new("cmd")
            .raw_arg(format!("/S /C \"timeout /T 3 /NOBREAK >NUL & rmdir /S /Q \"{d}\"\""))
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()?;
        Ok(())
    }
}

// -------------------------------------------------------------------- Linux

pub mod linux {
    use super::*;

    pub fn data_home() -> Result<PathBuf, InstallError> {
        if let Some(x) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            return Ok(PathBuf::from(x));
        }
        Ok(home_dir()?.join(".local/share"))
    }

    pub fn default_install_dir() -> Result<PathBuf, InstallError> {
        Ok(home_dir()?.join(".local/opt/waydlp"))
    }

    pub fn bin_dir() -> Result<PathBuf, InstallError> {
        Ok(home_dir()?.join(".local/bin"))
    }

    pub fn applications_dir() -> Result<PathBuf, InstallError> {
        Ok(home_dir()?.join("Applications"))
    }

    /// Contents of the `.desktop` file pointing at `exec`.
    pub fn desktop_entry(exec: &Path, icon: &str) -> String {
        // Desktop Entry spec: quote the path and escape special characters.
        let quoted = exec
            .display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$");
        format!(
            "[Desktop Entry]\nType=Application\nName=waydlp\nGenericName=Media Downloader\nComment=Download video and audio from websites supported by yt-dlp\nExec=\"{quoted}\"\nIcon={icon}\nTerminal=false\nCategories=AudioVideo;Network;FileTransfer;\nKeywords=download;video;audio;music;youtube;yt-dlp;\nStartupWMClass={APP_ID}\n"
        )
    }

    /// Install icon + desktop entry (+ optional desktop shortcut). Returns created paths.
    pub fn integrate(exec: &Path, desktop_shortcut: bool) -> Result<Vec<PathBuf>, InstallError> {
        let mut created = Vec::new();
        let data = data_home()?;
        let icon = data.join(format!("icons/hicolor/256x256/apps/{APP_ID}.png"));
        std::fs::create_dir_all(icon.parent().expect("parent"))?;
        std::fs::write(&icon, ICON_PNG_256)?;
        created.push(icon);
        let entry = desktop_entry(exec, APP_ID);
        let file = data.join(format!("applications/{APP_ID}.desktop"));
        std::fs::create_dir_all(file.parent().expect("parent"))?;
        std::fs::write(&file, &entry)?;
        created.push(file);
        if desktop_shortcut && let Some(d) = desktop_dir() {
            let f = d.join("waydlp.desktop");
            std::fs::write(&f, &entry)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755))?;
            }
            created.push(f);
        }
        // Refresh menus where the tool exists; failures are harmless.
        let _ = std::process::Command::new("update-desktop-database").arg(data.join("applications")).status();
        Ok(created)
    }

    /// Create `~/.local/bin/waydlp` → `target`.
    #[cfg(unix)]
    pub fn link_binary(target: &Path) -> Result<PathBuf, InstallError> {
        let bin = bin_dir()?;
        std::fs::create_dir_all(&bin)?;
        let link = bin.join("waydlp");
        if link.symlink_metadata().is_ok() {
            std::fs::remove_file(&link)?;
        }
        std::os::unix::fs::symlink(target, &link)?;
        Ok(link)
    }

    pub fn path_contains_local_bin() -> bool {
        let Ok(bin) = bin_dir() else { return false };
        std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|x| x == bin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entry_escapes_exec() {
        let e = linux::desktop_entry(Path::new("/home/a b/$x/waydlp"), APP_ID);
        assert!(e.contains("Exec=\"/home/a b/\\$x/waydlp\""));
        assert!(e.contains("Icon=io.github.waywhywhy.waydlp"));
    }

    #[test]
    fn icon_is_png() {
        assert_eq!(&ICON_PNG_256[..8], b"\x89PNG\r\n\x1a\n");
    }
}
