//! Delegation to system package managers. waydlp never writes into
//! package-managed locations itself; it runs the package manager with
//! explicit argument lists (no shell) and reports its exit status.

use crate::{APP_ID, InstallError};
use std::path::Path;
use std::process::Command;
use waydlp_core::artifacts::PackageKind;

/// How to obtain administrator rights for package managers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    /// `sudo`, prompting in the terminal (CLI).
    Sudo,
    /// `pkexec`, showing a graphical polkit prompt (GUI).
    Pkexec,
    /// Already root, or not needed.
    None,
}

fn has(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

pub fn check_native(kind: PackageKind) -> Result<(), String> {
    match kind {
        PackageKind::Deb => {
            if has("dpkg") && (has("apt-get") || has("apt")) {
                Ok(())
            } else {
                Err("dpkg/apt is not available".into())
            }
        }
        PackageKind::Rpm => {
            if has("rpm") && (has("dnf") || has("zypper") || has("yum")) {
                Ok(())
            } else {
                Err("rpm with dnf/zypper/yum is not available".into())
            }
        }
        PackageKind::ArchPkg => {
            if has("pacman") {
                Ok(())
            } else {
                Err("pacman is not available".into())
            }
        }
        PackageKind::Flatpak => {
            if has("flatpak") {
                Ok(())
            } else {
                Err("flatpak is not installed".into())
            }
        }
        other => Err(format!("{other} is not a system package format")),
    }
}

/// The command (program + args) that installs `file`. Pure, for testing.
pub fn install_command(kind: PackageKind, file: &Path, yes: bool, elevation: Elevation, rpm_tool: &str) -> Vec<String> {
    let f = file.display().to_string();
    let mut v: Vec<String> = match kind {
        PackageKind::Deb => {
            // apt needs a path containing '/' to treat the argument as a file.
            let f = if f.contains('/') { f } else { format!("./{f}") };
            let mut v = vec!["apt-get".into(), "install".into()];
            if yes {
                v.push("-y".into());
            }
            v.push(f);
            v
        }
        PackageKind::Rpm => match rpm_tool {
            "zypper" => {
                let mut v = vec!["zypper".into()];
                if yes {
                    v.push("--non-interactive".into());
                }
                v.extend(["install".into(), "--allow-unsigned-rpm".into(), f]);
                v
            }
            tool => {
                let mut v = vec![tool.to_string(), "install".into()];
                if yes {
                    v.push("-y".into());
                }
                v.push(f);
                v
            }
        },
        PackageKind::ArchPkg => {
            let mut v = vec!["pacman".into(), "-U".into()];
            if yes {
                v.push("--noconfirm".into());
            }
            v.push(f);
            v
        }
        PackageKind::Flatpak => {
            // User installation: no administrator rights needed.
            let mut v = vec!["flatpak".into(), "install".into(), "--user".into()];
            if yes {
                v.push("-y".into());
            }
            v.extend(["--bundle".into(), f]);
            return v;
        }
        _ => return Vec::new(),
    };
    match elevation {
        Elevation::Sudo => v.insert(0, "sudo".into()),
        Elevation::Pkexec => v.insert(0, "pkexec".into()),
        Elevation::None => {}
    }
    v
}

pub fn uninstall_command(
    kind: PackageKind,
    yes: bool,
    elevation: Elevation,
    rpm_tool: &str,
    arch_package: &str,
) -> Vec<String> {
    let mut v: Vec<String> = match kind {
        PackageKind::Deb => {
            let mut v = vec!["apt-get".into(), "remove".into()];
            if yes {
                v.push("-y".into());
            }
            v.push("waydlp".into());
            v
        }
        PackageKind::Rpm => {
            let mut v = vec![rpm_tool.to_string()];
            if rpm_tool == "zypper" {
                if yes {
                    v.push("--non-interactive".into());
                }
                v.push("remove".into());
            } else {
                v.push("remove".into());
                if yes {
                    v.push("-y".into());
                }
            }
            v.push("waydlp".into());
            v
        }
        PackageKind::ArchPkg => {
            let mut v = vec!["pacman".into(), "-R".into()];
            if yes {
                v.push("--noconfirm".into());
            }
            v.push(arch_package.into());
            v
        }
        PackageKind::Flatpak => {
            let mut v = vec!["flatpak".into(), "uninstall".into(), "--user".into()];
            if yes {
                v.push("-y".into());
            }
            v.push(APP_ID.into());
            return v;
        }
        _ => return Vec::new(),
    };
    match elevation {
        Elevation::Sudo => v.insert(0, "sudo".into()),
        Elevation::Pkexec => v.insert(0, "pkexec".into()),
        Elevation::None => {}
    }
    v
}

pub fn rpm_tool() -> &'static str {
    if has("dnf") {
        "dnf"
    } else if has("zypper") {
        "zypper"
    } else {
        "yum"
    }
}

pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        std::env::var("USER").map(|u| u == "root").unwrap_or(false)
            || std::env::var("EUID").map(|u| u == "0").unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Run a command with inherited stdio (so sudo/apt can prompt).
pub fn run(cmd: &[String]) -> Result<(), InstallError> {
    let (prog, args) = cmd.split_first().ok_or_else(|| InstallError::Other("empty command".into()))?;
    let status = Command::new(prog)
        .args(args)
        .status()
        .map_err(|e| InstallError::Other(format!("could not run {prog}: {e}")))?;
    if status.success() { Ok(()) } else { Err(InstallError::CommandFailed(cmd.join(" "), status.code().unwrap_or(-1))) }
}

/// Is the package currently installed according to the package manager?
pub fn is_installed(kind: PackageKind) -> Option<bool> {
    let arch_package = if kind == PackageKind::ArchPkg { arch_package_owner() } else { None };
    let (prog, args): (&str, Vec<&str>) = match kind {
        PackageKind::Deb => ("dpkg-query", vec!["-W", "-f=${Status}", "waydlp"]),
        PackageKind::Rpm => ("rpm", vec!["-q", "waydlp"]),
        PackageKind::ArchPkg => ("pacman", vec!["-Q", arch_package.as_deref().unwrap_or("waydlp")]),
        PackageKind::Flatpak => ("flatpak", vec!["info", "--user", APP_ID]),
        _ => return None,
    };
    let out = Command::new(prog).args(args).output().ok()?;
    if kind == PackageKind::Deb {
        return Some(out.status.success() && String::from_utf8_lossy(&out.stdout).contains("install ok installed"));
    }
    Some(out.status.success())
}

fn parse_arch_owner(output: &str) -> Option<String> {
    match output.trim() {
        name @ ("waydlp" | "waydlp-bin" | "waydlp-git") => Some(name.into()),
        _ => None,
    }
}

/// Source, prebuilt and git Arch packages have different package names.
pub fn arch_package_owner() -> Option<String> {
    let out = Command::new("pacman").args(["-Qoq", "/usr/bin/waydlp"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_arch_owner(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn deb_command() {
        let c = install_command(PackageKind::Deb, &PathBuf::from("waydlp.deb"), true, Elevation::Sudo, "dnf");
        assert_eq!(c, vec!["sudo", "apt-get", "install", "-y", "./waydlp.deb"]);
    }

    #[test]
    fn rpm_commands() {
        let c = install_command(PackageKind::Rpm, &PathBuf::from("/tmp/w.rpm"), false, Elevation::Pkexec, "dnf");
        assert_eq!(c, vec!["pkexec", "dnf", "install", "/tmp/w.rpm"]);
        let c = install_command(PackageKind::Rpm, &PathBuf::from("/tmp/w.rpm"), true, Elevation::None, "zypper");
        assert_eq!(c, vec!["zypper", "--non-interactive", "install", "--allow-unsigned-rpm", "/tmp/w.rpm"]);
    }

    #[test]
    fn arch_and_flatpak_commands() {
        let c = install_command(PackageKind::ArchPkg, &PathBuf::from("/t/w.pkg.tar.zst"), true, Elevation::Sudo, "");
        assert_eq!(c, vec!["sudo", "pacman", "-U", "--noconfirm", "/t/w.pkg.tar.zst"]);
        let c = install_command(PackageKind::Flatpak, &PathBuf::from("/t/w.flatpak"), true, Elevation::Sudo, "");
        assert_eq!(c, vec!["flatpak", "install", "--user", "-y", "--bundle", "/t/w.flatpak"], "flatpak never elevates");
    }

    #[test]
    fn uninstall_commands() {
        assert_eq!(
            uninstall_command(PackageKind::Deb, true, Elevation::Sudo, "", "waydlp"),
            vec!["sudo", "apt-get", "remove", "-y", "waydlp"]
        );
        assert_eq!(
            uninstall_command(PackageKind::Rpm, true, Elevation::None, "zypper", "waydlp"),
            vec!["zypper", "--non-interactive", "remove", "waydlp"]
        );
        assert_eq!(
            uninstall_command(PackageKind::Flatpak, false, Elevation::Pkexec, "", "waydlp"),
            vec!["flatpak", "uninstall", "--user", APP_ID]
        );
    }

    #[test]
    fn arch_package_variants() {
        for name in ["waydlp", "waydlp-bin", "waydlp-git"] {
            assert_eq!(parse_arch_owner(&format!("{name}\n")), Some(name.into()));
            assert_eq!(
                uninstall_command(PackageKind::ArchPkg, true, Elevation::Sudo, "", name),
                vec!["sudo", "pacman", "-R", "--noconfirm", name]
            );
        }
        assert_eq!(parse_arch_owner("unrelated-package"), None);
        assert_eq!(parse_arch_owner("waydlp-bin\nother"), None);
    }
}
