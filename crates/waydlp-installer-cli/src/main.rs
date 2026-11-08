//! `waydlp-installer`: install, upgrade, verify and uninstall waydlp.
//!
//! Usable interactively and in scripts (`--yes`). Exit code is non-zero on
//! any failure.

use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::PathBuf;
use waydlp_core::artifacts::PackageKind;
use waydlp_installer_core::plan::{InstallOptions, Progress, Source};
use waydlp_installer_core::system::Elevation;
use waydlp_installer_core::{Distro, Installer, Receipt, Target, available_methods, payload};

#[derive(Parser)]
#[command(name = "waydlp-installer", version, about = "Install, update, verify and remove waydlp.")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install or upgrade waydlp.
    Install {
        /// Package format: portable (Windows user install), msi, tar (Linux user install),
        /// appimage, deb, rpm, arch, flatpak. Default: the best one for this system.
        #[arg(long, short)]
        format: Option<String>,
        /// Answer yes to all prompts (for scripts).
        #[arg(long, short)]
        yes: bool,
        /// Install a specific version instead of the latest.
        #[arg(long)]
        version: Option<String>,
        /// Install from a local artifact file instead of downloading.
        #[arg(long)]
        file: Option<PathBuf>,
        /// Installation directory for user-level installs.
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Create a desktop shortcut.
        #[arg(long)]
        desktop_shortcut: bool,
        /// Do not add waydlp to the application menu.
        #[arg(long)]
        no_menu: bool,
    },
    /// Remove waydlp (settings and history are kept).
    Uninstall {
        #[arg(long, short)]
        yes: bool,
    },
    /// Check that the installed files are intact.
    Verify,
    /// Show the installed version and installer version.
    Version,
    /// Show detected platform and supported installation methods.
    Detect,
    /// (Packaging) append an application archive to an installer stub,
    /// producing an offline installer.
    #[command(hide = true)]
    Bundle {
        #[arg(long)]
        stub: PathBuf,
        #[arg(long)]
        payload: PathBuf,
        #[arg(long)]
        version: String,
        #[arg(long)]
        out: PathBuf,
    },
}

fn confirm(question: &str, yes: bool) -> bool {
    if yes {
        return true;
    }
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).is_ok() && matches!(s.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn progress_printer() -> impl Fn(Progress) {
    let last = std::cell::Cell::new(0u64);
    move |p| match p {
        Progress::Status(s) => {
            if last.get() > 0 {
                eprintln!();
                last.set(0);
            }
            eprintln!("• {s}");
        }
        Progress::Download { done, total } => {
            if done - last.get() >= 512 * 1024 || Some(done) == total {
                last.set(done.max(1));
                match total {
                    Some(t) => eprint!("\r  {:.1} / {:.1} MB", done as f64 / 1048576.0, t as f64 / 1048576.0),
                    None => eprint!("\r  {:.1} MB", done as f64 / 1048576.0),
                }
            }
        }
        Progress::Files { done, total } => {
            if done == total {
                eprintln!("  {total} files");
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let cmd = cli.cmd.unwrap_or(Cmd::Detect);
    match cmd {
        Cmd::Bundle { stub, payload: p, version, out } => {
            payload::bundle(&stub, &p, &version, &out)?;
            println!("Wrote {}", out.display());
        }
        Cmd::Version => {
            println!("waydlp-installer {}", env!("CARGO_PKG_VERSION"));
            match Receipt::load()? {
                Some(r) => println!("installed: waydlp {} ({})", r.version, r.method.display_name()),
                None => println!("installed: none recorded by this installer"),
            }
            if let Ok(Some(p)) = payload::current() {
                println!("embedded payload: waydlp {}", p.version);
            }
        }
        Cmd::Detect => {
            let t = Target::detect()?;
            println!("platform: {}", t.describe());
            if let Some(d) = Distro::detect() {
                println!(
                    "distribution: {} ({}{})",
                    d.pretty_name,
                    d.id,
                    if d.id_like.is_empty() { String::new() } else { format!(", like {}", d.id_like.join(" ")) }
                );
            }
            println!("installation methods:");
            for (k, r) in available_methods(t) {
                match r {
                    Ok(()) => println!("  ✓ {:9} {}", k.cli_name(), k.display_name()),
                    Err(e) => println!("  ✗ {:9} {} — {e}", k.cli_name(), k.display_name()),
                }
            }
        }
        Cmd::Verify => {
            let inst = Installer::new()?;
            let r = inst.verify()?;
            println!("waydlp {} installed as {}", r.receipt.version, r.receipt.method.display_name());
            println!("{} file(s) intact", r.ok.len());
            for m in &r.missing {
                println!("  missing:  {}", m.display());
            }
            for m in &r.modified {
                println!("  modified: {}", m.display());
            }
            match r.package_installed {
                Some(true) => println!("package manager reports waydlp as installed"),
                Some(false) => println!("package manager does NOT report waydlp as installed"),
                None => {}
            }
            if !r.is_ok() {
                bail!("verification failed; reinstall with `waydlp-installer install`");
            }
            println!("OK");
        }
        Cmd::Uninstall { yes } => {
            let inst = Installer::new()?;
            let r = Receipt::load()?
                .ok_or_else(|| anyhow!("waydlp was not installed with this installer; nothing to do"))?;
            if !confirm(&format!("Uninstall waydlp {} ({})?", r.version, r.method.display_name()), yes) {
                bail!("cancelled");
            }
            inst.uninstall(yes, Elevation::Sudo, &progress_printer())?;
        }
        Cmd::Install { format, yes, version, file, dir, desktop_shortcut, no_menu } => {
            let inst = Installer::new()?;
            let embedded = payload::current().ok().flatten();
            let method = match &format {
                Some(f) => PackageKind::from_cli_name(f)
                    .ok_or_else(|| anyhow!("unknown format \"{f}\"; see `waydlp-installer detect`"))?,
                None => {
                    if let Some(f) = &file {
                        waydlp_core::artifacts::parse_artifact_name(
                            &f.file_name().unwrap_or_default().to_string_lossy(),
                        )
                        .map(|a| a.kind)
                        .ok_or_else(|| anyhow!("cannot tell the package format from the file name; pass --format"))?
                    } else {
                        let methods = available_methods(inst.target);
                        methods
                            .iter()
                            .find(|(_, r)| r.is_ok())
                            .map(|(k, _)| *k)
                            .ok_or_else(|| anyhow!("no supported installation method found"))?
                    }
                }
            };
            if let Some((_, Err(why))) = available_methods(inst.target).into_iter().find(|(k, _)| *k == method) {
                bail!("{} is not available here: {why}", method.display_name());
            }
            let source = match (file, embedded) {
                (Some(f), _) => Source::File(f),
                (None, Some(p))
                    if version.is_none() && matches!(method, PackageKind::WindowsPortable | PackageKind::TarGz) =>
                {
                    Source::Embedded(p)
                }
                _ => Source::Release { version },
            };
            let what = match &source {
                Source::File(f) => format!("from {}", f.display()),
                Source::Embedded(p) => format!("version {} (bundled)", p.version),
                Source::Release { version: Some(v) } => format!("version {v} from GitHub Releases"),
                Source::Release { version: None } => "the latest release from GitHub Releases".into(),
            };
            println!("Installing waydlp {what} as {}.", method.display_name());
            if matches!(method, PackageKind::Deb | PackageKind::Rpm | PackageKind::ArchPkg) {
                println!("Your package manager will ask for administrator rights.");
            }
            if !confirm("Continue?", yes) {
                bail!("cancelled");
            }
            let opts = InstallOptions {
                method,
                source,
                install_dir: dir,
                desktop_shortcut,
                menu_entry: !no_menu,
                assume_yes: yes,
                elevation: Elevation::Sudo,
            };
            let r = inst.install(&opts, &progress_printer())?;
            println!("Installed waydlp {}.", r.version);
            if let Some(d) = &r.install_dir {
                println!("Location: {}", d.display());
            }
            #[cfg(target_os = "linux")]
            if matches!(method, PackageKind::TarGz) && !waydlp_installer_core::desktop::linux::path_contains_local_bin()
            {
                println!(
                    "Note: ~/.local/bin is not on your PATH; start waydlp from the application menu or add it to PATH."
                );
            }
        }
    }
    Ok(())
}
