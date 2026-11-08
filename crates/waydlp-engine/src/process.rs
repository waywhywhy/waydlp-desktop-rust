//! Safe subprocess handling.
//!
//! - Arguments are passed as argv; no shell is involved.
//! - On Windows no console window is shown.
//! - On Unix the child gets its own process group so the whole tree
//!   (yt-dlp plus the FFmpeg it spawns) can be stopped together.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a command with waydlp's standard process settings.
pub fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        // Make Python-based yt-dlp emit UTF-8 regardless of the console code page.
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .env("PYTHONUNBUFFERED", "1");
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    cmd
}

/// Run a program to completion and capture its output.
pub async fn run_capture<I, S>(
    program: &Path,
    args: I,
    timeout: std::time::Duration,
) -> std::io::Result<std::process::Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = command(program);
    cmd.args(args);
    let child = cmd.spawn()?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(r) => r,
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("{} did not finish within {}s", program.display(), timeout.as_secs()),
        )),
    }
}

/// Terminate a child and all of its descendants.
pub async fn kill_tree(child: &mut Child) {
    let Some(pid) = child.id() else {
        return; // already exited
    };
    #[cfg(windows)]
    {
        // yt-dlp.exe is a PyInstaller bootstrapper that spawns a second
        // process, which in turn may spawn ffmpeg; /T kills the whole tree.
        let mut tk = Command::new("taskkill");
        tk.args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        let _ = tk.status().await;
    }
    #[cfg(unix)]
    {
        let pgid = pid as libc::pid_t;
        // SAFETY: sending a signal to a process group we created; no memory involved.
        unsafe {
            libc::kill(-pgid, libc::SIGTERM);
        }
        if tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await.is_err() {
            // SAFETY: as above.
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
        }
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}
