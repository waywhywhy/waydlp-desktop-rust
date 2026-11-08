//! Running yt-dlp: probing URLs and executing downloads.

use crate::process::{command, kill_tree};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use waydlp_core::errors::{ErrorKind, FailureReport, report_from_output};
use waydlp_core::media::{ProbeResult, parse_probe_json};
use waydlp_core::progress::{ProgressEvent, parse_line};

/// Keep at most this many lines of diagnostic output per run.
const LOG_LINES: usize = 400;

fn spawn_failure(program: &Path, e: &std::io::Error) -> FailureReport {
    if e.kind() == std::io::ErrorKind::NotFound {
        FailureReport::new(ErrorKind::YtDlpMissing, format!("{} was not found", program.display()), e.to_string())
    } else {
        FailureReport::new(ErrorKind::Unknown, format!("Could not start {}", program.display()), e.to_string())
    }
}

/// Probe a URL with `yt-dlp -J --flat-playlist`.
pub async fn probe(
    yt_dlp: &Path,
    args: &[String],
    timeout: Duration,
    cancel: CancellationToken,
) -> Result<ProbeResult, FailureReport> {
    let mut cmd = command(yt_dlp);
    cmd.args(args);
    let mut child = cmd.spawn().map_err(|e| spawn_failure(yt_dlp, &e))?;
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");

    let read_out = async {
        use tokio::io::AsyncReadExt;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let (a, b) = tokio::join!(stdout.read_to_end(&mut out), stderr.read_to_end(&mut err));
        a?;
        b?;
        Ok::<_, std::io::Error>((out, err))
    };

    let result = tokio::select! {
        r = tokio::time::timeout(timeout, read_out) => r,
        _ = cancel.cancelled() => {
            kill_tree(&mut child).await;
            return Err(FailureReport::new(ErrorKind::Cancelled, "Analysis cancelled", ""));
        }
    };
    let (out, err) = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            kill_tree(&mut child).await;
            return Err(FailureReport::new(ErrorKind::Unknown, "Failed to read yt-dlp output", e.to_string()));
        }
        Err(_) => {
            kill_tree(&mut child).await;
            return Err(FailureReport::new(
                ErrorKind::Timeout,
                format!("Analysis took longer than {} seconds", timeout.as_secs()),
                "",
            ));
        }
    };
    let status = child
        .wait()
        .await
        .map_err(|e| FailureReport::new(ErrorKind::Unknown, "Failed to wait for yt-dlp", e.to_string()))?;
    let stderr_text = String::from_utf8_lossy(&err).to_string();
    if !status.success() {
        return Err(report_from_output(&stderr_text));
    }
    let stdout_text = String::from_utf8_lossy(&out);
    parse_probe_json(&stdout_text).map_err(|e| {
        FailureReport::new(
            ErrorKind::Unknown,
            e.to_string(),
            format!("{stderr_text}\n--- stdout (truncated) ---\n{}", truncate(&stdout_text, 4000)),
        )
    })
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunOutcome {
    /// yt-dlp exited successfully. `files` lists the final output paths it
    /// reported (may be empty when everything was already downloaded).
    Success {
        files: Vec<PathBuf>,
        warnings: Vec<String>,
    },
    Failed(FailureReport),
    /// Stopped because the cancellation token fired.
    Stopped,
}

async fn pump_lines<R: AsyncRead + Unpin>(reader: R, tx: mpsc::Sender<String>) {
    let mut r = BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string();
                // yt-dlp may emit several \r-separated updates on one line.
                for part in line.split('\r') {
                    if !part.is_empty() && tx.send(part.to_string()).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
}

/// Run a yt-dlp download and stream parsed progress to `on_event`.
pub async fn download(
    yt_dlp: &Path,
    args: &[String],
    cancel: CancellationToken,
    mut on_event: impl FnMut(&ProgressEvent) + Send,
) -> RunOutcome {
    let mut cmd = command(yt_dlp);
    cmd.args(args);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return RunOutcome::Failed(spawn_failure(yt_dlp, &e)),
    };
    let (tx, mut rx) = mpsc::channel::<String>(256);
    let out_task = tokio::spawn(pump_lines(child.stdout.take().expect("piped"), tx.clone()));
    let err_task = tokio::spawn(pump_lines(child.stderr.take().expect("piped"), tx));

    let mut log: VecDeque<String> = VecDeque::with_capacity(LOG_LINES);
    let mut files = Vec::new();
    let mut warnings = Vec::new();

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                kill_tree(&mut child).await;
                out_task.abort();
                err_task.abort();
                return RunOutcome::Stopped;
            }
            line = rx.recv() => {
                let Some(line) = line else { break };
                let ev = parse_line(&line);
                match &ev {
                    ProgressEvent::FinalFile(p) => files.push(p.clone()),
                    ProgressEvent::Warning(w) => warnings.push(w.clone()),
                    _ => {}
                }
                if !matches!(ev, ProgressEvent::Download(_)) {
                    if log.len() == LOG_LINES {
                        log.pop_front();
                    }
                    log.push_back(line);
                }
                on_event(&ev);
            }
        }
    }

    let status = tokio::select! {
        s = child.wait() => s,
        _ = cancel.cancelled() => {
            kill_tree(&mut child).await;
            return RunOutcome::Stopped;
        }
    };
    let joined: String = log.iter().map(String::as_str).collect::<Vec<_>>().join("\n");
    match status {
        Ok(s) if s.success() => RunOutcome::Success { files, warnings },
        Ok(s) => {
            let mut report = report_from_output(&joined);
            if report.summary.is_empty() {
                report.summary = format!("yt-dlp exited with {s}");
            }
            RunOutcome::Failed(report)
        }
        Err(e) => {
            RunOutcome::Failed(FailureReport::new(ErrorKind::Unknown, "Failed to wait for yt-dlp", e.to_string()))
        }
    }
}

/// `yt-dlp --list-extractors`: the authoritative list of supported sites
/// for the installed version.
pub async fn list_extractors(yt_dlp: &Path) -> Result<Vec<String>, FailureReport> {
    let out = crate::process::run_capture(yt_dlp, ["--ignore-config", "--list-extractors"], Duration::from_secs(60))
        .await
        .map_err(|e| spawn_failure(yt_dlp, &e))?;
    if !out.status.success() {
        return Err(report_from_output(&String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}
