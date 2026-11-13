//! The download queue: an actor running on Tokio that owns every job,
//! enforces the concurrency limit, retries transient failures and reports
//! real progress to the UI through a callback.
//!
//! Each job gets its own temporary directory (`--paths temp:`). Partial files
//! live there, which makes the lifecycle simple and leak-free:
//! - pause keeps the directory, so resuming continues the `.part` files;
//! - cancel/remove deletes it;
//! - completion deletes it after yt-dlp moved the final files into place.

use crate::run::{RunOutcome, download};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use waydlp_core::args::{DownloadRequest, EngineContext, NetworkOptions, build_download_args};
use waydlp_core::errors::{ErrorKind, FailureReport};
use waydlp_core::progress::{DownloadProgress, ProgressEvent, postprocessor_label};

pub type JobId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Queued,
    Running,
    Paused,
    WaitingRetry,
    Completed,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn is_finished(self) -> bool {
        matches!(self, JobState::Completed | JobState::Failed | JobState::Cancelled)
    }

    pub fn is_active(self) -> bool {
        matches!(self, JobState::Running | JobState::Queued | JobState::WaitingRetry)
    }

    pub fn label(self) -> &'static str {
        match self {
            JobState::Queued => "Queued",
            JobState::Running => "Downloading",
            JobState::Paused => "Paused",
            JobState::WaitingRetry => "Waiting to retry",
            JobState::Completed => "Completed",
            JobState::Failed => "Failed",
            JobState::Cancelled => "Cancelled",
        }
    }
}

/// Everything needed to run one download.
#[derive(Debug, Clone)]
pub struct JobSpec {
    pub request: DownloadRequest,
    pub ctx: EngineContext,
    pub net: NetworkOptions,
    pub yt_dlp: PathBuf,
    /// Known metadata for display (from analysis), if any.
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub site: String,
    /// Opaque id the UI uses to link a job to its history row.
    pub history_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct JobSnapshot {
    pub id: JobId,
    pub spec: Arc<JobSpec>,
    pub state: JobState,
    /// Human-readable current stage, e.g. "Merging video and audio".
    pub stage: String,
    /// Real progress of the current stream; `None` when unknown.
    pub progress: Option<DownloadProgress>,
    /// Current playlist item `(index, count)`.
    pub item: Option<(Option<u32>, Option<u32>)>,
    pub current_title: Option<String>,
    pub files: Vec<PathBuf>,
    pub warnings: Vec<String>,
    pub error: Option<FailureReport>,
    /// Number of times the job was started.
    pub attempts: u32,
    pub created_at: SystemTime,
    pub finished_at: Option<SystemTime>,
}

impl JobSnapshot {
    pub fn display_title(&self) -> String {
        self.current_title
            .clone()
            .or_else(|| self.spec.title.clone())
            .unwrap_or_else(|| self.spec.request.url.to_string())
    }
}

/// Snapshots are only sent on state changes (and throttled progress), so the
/// size difference between variants doesn't matter here.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum QueueEvent {
    Updated(JobSnapshot),
    Removed(JobId),
    Order(Vec<JobId>),
}

pub type EventSink = Arc<dyn Fn(QueueEvent) + Send + Sync>;

#[derive(Debug, Clone, Copy)]
pub struct QueueSettings {
    pub max_concurrent: usize,
    /// Automatic retries for transient failures (network, rate limits...).
    pub auto_retries: u32,
    pub retry_base_delay: Duration,
}

impl Default for QueueSettings {
    fn default() -> Self {
        Self { max_concurrent: 3, auto_retries: 2, retry_base_delay: Duration::from_secs(5) }
    }
}

enum Cmd {
    Add(JobId, Box<JobSpec>),
    Cancel(JobId),
    Pause(JobId),
    Resume(JobId),
    Retry(JobId),
    Remove(JobId),
    Move(JobId, i32),
    Settings(QueueSettings),
    ClearFinished,
    RetryFailed,
    PauseAll,
    ResumeAll,
    Shutdown(oneshot::Sender<()>),
}

enum Internal {
    Event(JobId, u64, ProgressEvent),
    Exited(JobId, u64, RunOutcome),
    /// The run sequence the retry was scheduled for: a stale timer (the job
    /// was paused, resumed and failed again meanwhile) must not fire early.
    RetryDue(JobId, u64),
}

/// Cheap, clonable handle used by the UI and CLI.
#[derive(Clone)]
pub struct QueueHandle {
    tx: mpsc::UnboundedSender<Cmd>,
    next_id: Arc<AtomicU64>,
}

impl QueueHandle {
    pub fn add(&self, spec: JobSpec) -> JobId {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.tx.send(Cmd::Add(id, Box::new(spec)));
        id
    }
    pub fn cancel(&self, id: JobId) {
        let _ = self.tx.send(Cmd::Cancel(id));
    }
    pub fn pause(&self, id: JobId) {
        let _ = self.tx.send(Cmd::Pause(id));
    }
    pub fn resume(&self, id: JobId) {
        let _ = self.tx.send(Cmd::Resume(id));
    }
    pub fn retry(&self, id: JobId) {
        let _ = self.tx.send(Cmd::Retry(id));
    }
    pub fn remove(&self, id: JobId) {
        let _ = self.tx.send(Cmd::Remove(id));
    }
    pub fn move_by(&self, id: JobId, delta: i32) {
        let _ = self.tx.send(Cmd::Move(id, delta));
    }
    pub fn set_settings(&self, s: QueueSettings) {
        let _ = self.tx.send(Cmd::Settings(s));
    }
    pub fn clear_finished(&self) {
        let _ = self.tx.send(Cmd::ClearFinished);
    }
    /// Retry every failed job (each gets a fresh automatic-retry budget).
    pub fn retry_failed(&self) {
        let _ = self.tx.send(Cmd::RetryFailed);
    }
    pub fn pause_all(&self) {
        let _ = self.tx.send(Cmd::PauseAll);
    }
    pub fn resume_all(&self) {
        let _ = self.tx.send(Cmd::ResumeAll);
    }
    /// Stop every running process and wait until they are gone.
    pub async fn shutdown(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Cmd::Shutdown(tx)).is_ok() {
            let _ = rx.await;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StopReason {
    Pause,
    Cancel,
    Remove,
    Shutdown,
}

struct Job {
    snap: JobSnapshot,
    cancel: Option<CancellationToken>,
    stop_reason: Option<StopReason>,
    /// Incremented on every start so stale events from a previous run are ignored.
    run_seq: u64,
    last_emit: Instant,
    temp_dir: PathBuf,
}

/// Start the queue actor on the given runtime.
///
/// `temp_root` holds per-job temporary directories; it is wiped at start-up
/// because the queue itself is not persisted between sessions.
pub fn spawn_queue(
    rt: &tokio::runtime::Handle,
    temp_root: PathBuf,
    settings: QueueSettings,
    sink: EventSink,
) -> QueueHandle {
    let (tx, rx) = mpsc::unbounded_channel();
    let handle = QueueHandle { tx, next_id: Arc::new(AtomicU64::new(1)) };
    rt.spawn(async move {
        if temp_root.exists() {
            let _ = tokio::fs::remove_dir_all(&temp_root).await;
        }
        let actor = Actor::new(temp_root, settings, sink);
        actor.run(rx).await;
    });
    handle
}

struct Actor {
    jobs: Vec<Job>,
    settings: QueueSettings,
    sink: EventSink,
    temp_root: PathBuf,
    session: u128,
    itx: mpsc::UnboundedSender<Internal>,
    irx: Option<mpsc::UnboundedReceiver<Internal>>,
    running_tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Actor {
    fn new(temp_root: PathBuf, settings: QueueSettings, sink: EventSink) -> Self {
        let (itx, irx) = mpsc::unbounded_channel();
        let session = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        Self { jobs: Vec::new(), settings, sink, temp_root, session, itx, irx: Some(irx), running_tasks: Vec::new() }
    }

    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Cmd>) {
        let mut irx = self.irx.take().expect("internal receiver");
        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(Cmd::Shutdown(done)) => {
                            self.shutdown().await;
                            let _ = done.send(());
                            return;
                        }
                        Some(cmd) => self.handle_cmd(cmd).await,
                        None => {
                            self.shutdown().await;
                            return;
                        }
                    }
                }
                Some(msg) = irx.recv() => self.handle_internal(msg).await,
            }
            self.running_tasks.retain(|t| !t.is_finished());
            self.schedule().await;
        }
    }

    fn emit(&self, idx: usize) {
        (self.sink)(QueueEvent::Updated(self.jobs[idx].snap.clone()));
    }

    fn emit_order(&self) {
        (self.sink)(QueueEvent::Order(self.jobs.iter().map(|j| j.snap.id).collect()));
    }

    fn find(&self, id: JobId) -> Option<usize> {
        self.jobs.iter().position(|j| j.snap.id == id)
    }

    async fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Add(id, spec) => {
                let temp_dir = self.temp_root.join(format!("job-{}-{id}", self.session));
                let snap = JobSnapshot {
                    id,
                    spec: Arc::new(*spec),
                    state: JobState::Queued,
                    stage: "Waiting in queue".into(),
                    progress: None,
                    item: None,
                    current_title: None,
                    files: Vec::new(),
                    warnings: Vec::new(),
                    error: None,
                    attempts: 0,
                    created_at: SystemTime::now(),
                    finished_at: None,
                };
                self.jobs.push(Job {
                    snap,
                    cancel: None,
                    stop_reason: None,
                    run_seq: 0,
                    last_emit: Instant::now(),
                    temp_dir,
                });
                self.emit(self.jobs.len() - 1);
                self.emit_order();
            }
            Cmd::Cancel(id) => self.stop(id, StopReason::Cancel).await,
            Cmd::Pause(id) => self.stop(id, StopReason::Pause).await,
            Cmd::Remove(id) => self.stop(id, StopReason::Remove).await,
            Cmd::Resume(id) | Cmd::Retry(id) => {
                if let Some(i) = self.find(id) {
                    let st = self.jobs[i].snap.state;
                    if matches!(st, JobState::Paused | JobState::Failed | JobState::Cancelled | JobState::WaitingRetry)
                    {
                        let j = &mut self.jobs[i];
                        j.snap.state = JobState::Queued;
                        j.snap.stage = "Waiting in queue".into();
                        j.snap.error = None;
                        j.snap.finished_at = None;
                        if st != JobState::Paused {
                            // Manual retry gets a fresh automatic-retry budget.
                            j.snap.attempts = 0;
                        }
                        self.emit(i);
                    }
                }
            }
            Cmd::Move(id, delta) => {
                if let Some(i) = self.find(id) {
                    let target = (i as i64 + delta as i64).clamp(0, self.jobs.len() as i64 - 1) as usize;
                    if target != i {
                        let job = self.jobs.remove(i);
                        self.jobs.insert(target, job);
                        self.emit_order();
                    }
                }
            }
            Cmd::Settings(s) => self.settings = s,
            Cmd::ClearFinished => {
                let removed: Vec<JobId> =
                    self.jobs.iter().filter(|j| j.snap.state.is_finished()).map(|j| j.snap.id).collect();
                self.jobs.retain(|j| !j.snap.state.is_finished());
                for id in removed {
                    (self.sink)(QueueEvent::Removed(id));
                }
                self.emit_order();
            }
            Cmd::RetryFailed => {
                for i in 0..self.jobs.len() {
                    if self.jobs[i].snap.state == JobState::Failed {
                        let j = &mut self.jobs[i];
                        j.snap.state = JobState::Queued;
                        j.snap.stage = "Waiting in queue".into();
                        j.snap.error = None;
                        j.snap.finished_at = None;
                        j.snap.attempts = 0;
                        self.emit(i);
                    }
                }
            }
            Cmd::PauseAll => {
                let ids: Vec<JobId> =
                    self.jobs.iter().filter(|j| j.snap.state.is_active()).map(|j| j.snap.id).collect();
                for id in ids {
                    self.stop(id, StopReason::Pause).await;
                }
            }
            Cmd::ResumeAll => {
                for i in 0..self.jobs.len() {
                    if self.jobs[i].snap.state == JobState::Paused {
                        self.jobs[i].snap.state = JobState::Queued;
                        self.jobs[i].snap.stage = "Waiting in queue".into();
                        self.emit(i);
                    }
                }
            }
            Cmd::Shutdown(_) => unreachable!("handled in run loop"),
        }
    }

    async fn stop(&mut self, id: JobId, reason: StopReason) {
        let Some(i) = self.find(id) else { return };
        let state = self.jobs[i].snap.state;
        if state == JobState::Running {
            // The running task kills the process; the final state is applied
            // when it reports back in `Exited`.
            let j = &mut self.jobs[i];
            j.stop_reason = Some(reason);
            j.snap.stage = match reason {
                StopReason::Pause => "Pausing…".into(),
                _ => "Stopping…".into(),
            };
            if let Some(c) = &j.cancel {
                c.cancel();
            }
            self.emit(i);
            return;
        }
        match reason {
            StopReason::Pause => {
                if matches!(state, JobState::Queued | JobState::WaitingRetry) {
                    self.jobs[i].snap.state = JobState::Paused;
                    self.jobs[i].snap.stage = "Paused".into();
                    self.emit(i);
                }
            }
            StopReason::Cancel => {
                if !state.is_finished() {
                    self.finish_cancelled(i).await;
                }
            }
            StopReason::Remove | StopReason::Shutdown => {
                let job = self.jobs.remove(i);
                let _ = tokio::fs::remove_dir_all(&job.temp_dir).await;
                (self.sink)(QueueEvent::Removed(id));
                self.emit_order();
            }
        }
    }

    async fn finish_cancelled(&mut self, i: usize) {
        let j = &mut self.jobs[i];
        j.snap.state = JobState::Cancelled;
        j.snap.stage = "Cancelled".into();
        j.snap.progress = None;
        j.snap.finished_at = Some(SystemTime::now());
        j.snap.error = Some(FailureReport::new(ErrorKind::Cancelled, "Cancelled by you", ""));
        let dir = j.temp_dir.clone();
        let _ = tokio::fs::remove_dir_all(&dir).await;
        self.emit(i);
    }

    async fn schedule(&mut self) {
        loop {
            let running = self.jobs.iter().filter(|j| j.snap.state == JobState::Running).count();
            if running >= self.settings.max_concurrent.max(1) {
                return;
            }
            let Some(i) = self.jobs.iter().position(|j| j.snap.state == JobState::Queued) else {
                return;
            };
            self.start(i).await;
        }
    }

    async fn start(&mut self, i: usize) {
        let job = &mut self.jobs[i];
        let spec = job.snap.spec.clone();
        let mut ctx = spec.ctx.clone();
        ctx.temp_dir = Some(job.temp_dir.clone());

        let args = match build_download_args(&spec.request, &ctx, &spec.net) {
            Ok(a) => a,
            Err(e) => {
                job.snap.state = JobState::Failed;
                job.snap.stage = "Invalid options".into();
                job.snap.error = Some(FailureReport::new(ErrorKind::InvalidPath, e.to_string(), e.to_string()));
                job.snap.finished_at = Some(SystemTime::now());
                self.emit(i);
                return;
            }
        };
        if let Err(e) = tokio::fs::create_dir_all(&ctx.output_dir).await {
            let kind = if e.kind() == std::io::ErrorKind::PermissionDenied {
                ErrorKind::PermissionDenied
            } else {
                ErrorKind::InvalidPath
            };
            job.snap.state = JobState::Failed;
            job.snap.stage = "Cannot create download folder".into();
            job.snap.error =
                Some(FailureReport::new(kind, format!("Cannot create {}", ctx.output_dir.display()), e.to_string()));
            job.snap.finished_at = Some(SystemTime::now());
            self.emit(i);
            return;
        }
        let _ = tokio::fs::create_dir_all(&job.temp_dir).await;

        let token = CancellationToken::new();
        job.cancel = Some(token.clone());
        job.stop_reason = None;
        job.run_seq += 1;
        job.snap.attempts += 1;
        job.snap.state = JobState::Running;
        job.snap.stage = "Starting…".into();
        job.snap.progress = None;
        job.snap.error = None;
        let id = job.snap.id;
        let seq = job.run_seq;
        tracing::info!(job = id, url = %spec.request.url, "starting download");
        tracing::debug!(job = id, ?args, "yt-dlp arguments");
        self.emit(i);

        let itx = self.itx.clone();
        let yt = spec.yt_dlp.clone();
        let task = tokio::spawn(async move {
            let itx2 = itx.clone();
            let outcome = download(&yt, &args, token, move |ev| {
                let _ = itx2.send(Internal::Event(id, seq, ev.clone()));
            })
            .await;
            let _ = itx.send(Internal::Exited(id, seq, outcome));
        });
        self.running_tasks.push(task);
    }

    async fn handle_internal(&mut self, msg: Internal) {
        match msg {
            Internal::Event(id, seq, ev) => {
                let Some(i) = self.find(id) else { return };
                if self.jobs[i].run_seq != seq || self.jobs[i].stop_reason.is_some() {
                    return;
                }
                let throttle = matches!(ev, ProgressEvent::Download(ref p) if p.status == "downloading");
                let snap = &mut self.jobs[i].snap;
                match ev {
                    ProgressEvent::Download(p) => {
                        let mut stage = p.stream.label().to_string();
                        if let Some((Some(idx), Some(n))) = snap.item {
                            stage = format!("{stage} · item {idx} of {n}");
                        }
                        snap.stage = stage;
                        snap.progress = Some(p);
                    }
                    ProgressEvent::PostProcess { status, processor } => {
                        if status == "started" || status == "processing" {
                            snap.stage = postprocessor_label(&processor);
                            snap.progress = None;
                        }
                    }
                    ProgressEvent::Item { index, count, title, .. } => {
                        snap.item = Some((index, count));
                        if title.is_some() {
                            snap.current_title = title;
                        }
                    }
                    ProgressEvent::FinalFile(p) => snap.files.push(p),
                    ProgressEvent::Warning(w) => {
                        if snap.warnings.len() < 50 {
                            snap.warnings.push(w);
                        }
                    }
                    ProgressEvent::Error(_) | ProgressEvent::Log(_) => return,
                }
                let now = Instant::now();
                if !throttle || now.duration_since(self.jobs[i].last_emit) >= Duration::from_millis(150) {
                    self.jobs[i].last_emit = now;
                    self.emit(i);
                }
            }
            Internal::Exited(id, seq, outcome) => {
                let Some(i) = self.find(id) else { return };
                if self.jobs[i].run_seq != seq {
                    return;
                }
                self.jobs[i].cancel = None;
                let reason = self.jobs[i].stop_reason.take();
                match outcome {
                    RunOutcome::Stopped => match reason.unwrap_or(StopReason::Cancel) {
                        StopReason::Pause => {
                            let j = &mut self.jobs[i];
                            j.snap.state = JobState::Paused;
                            j.snap.stage = "Paused — partial data kept".into();
                            self.emit(i);
                        }
                        StopReason::Cancel => self.finish_cancelled(i).await,
                        StopReason::Remove | StopReason::Shutdown => {
                            let job = self.jobs.remove(i);
                            let _ = tokio::fs::remove_dir_all(&job.temp_dir).await;
                            (self.sink)(QueueEvent::Removed(id));
                            self.emit_order();
                        }
                    },
                    RunOutcome::Success { files, warnings } => {
                        let j = &mut self.jobs[i];
                        for f in files {
                            if !j.snap.files.contains(&f) {
                                j.snap.files.push(f);
                            }
                        }
                        for w in warnings {
                            if !j.snap.warnings.contains(&w) && j.snap.warnings.len() < 50 {
                                j.snap.warnings.push(w);
                            }
                        }
                        j.snap.state = JobState::Completed;
                        j.snap.stage = if j.snap.files.is_empty() {
                            "Nothing new to download (already downloaded or skipped)".into()
                        } else {
                            "Completed".into()
                        };
                        if let Some(p) = &mut j.snap.progress {
                            p.status = "finished".into();
                        }
                        j.snap.finished_at = Some(SystemTime::now());
                        let dir = j.temp_dir.clone();
                        let _ = tokio::fs::remove_dir_all(&dir).await;
                        self.emit(i);
                    }
                    RunOutcome::Failed(report) => {
                        let retries = self.settings.auto_retries;
                        let base = self.settings.retry_base_delay;
                        let j = &mut self.jobs[i];
                        let retry = report.kind.is_transient() && j.snap.attempts <= retries;
                        if retry {
                            let delay = base * j.snap.attempts.max(1);
                            j.snap.state = JobState::WaitingRetry;
                            j.snap.stage = format!(
                                "{} — retrying in {}s (attempt {} of {})",
                                report.kind.title(),
                                delay.as_secs(),
                                j.snap.attempts + 1,
                                retries + 1
                            );
                            j.snap.error = Some(report);
                            let itx = self.itx.clone();
                            let seq = j.run_seq;
                            tokio::spawn(async move {
                                tokio::time::sleep(delay).await;
                                let _ = itx.send(Internal::RetryDue(id, seq));
                            });
                        } else {
                            j.snap.state = JobState::Failed;
                            j.snap.stage = report.kind.title().into();
                            j.snap.error = Some(report);
                            j.snap.finished_at = Some(SystemTime::now());
                        }
                        self.emit(i);
                    }
                }
            }
            Internal::RetryDue(id, seq) => {
                if let Some(i) = self.find(id)
                    && self.jobs[i].run_seq == seq
                    && self.jobs[i].snap.state == JobState::WaitingRetry
                {
                    self.jobs[i].snap.state = JobState::Queued;
                    self.jobs[i].snap.stage = "Retrying…".into();
                    self.emit(i);
                }
            }
        }
    }

    async fn shutdown(&mut self) {
        for j in &mut self.jobs {
            if let Some(c) = &j.cancel {
                j.stop_reason = Some(StopReason::Shutdown);
                c.cancel();
            }
        }
        for t in self.running_tasks.drain(..) {
            let _ = tokio::time::timeout(Duration::from_secs(10), t).await;
        }
        let _ = tokio::fs::remove_dir_all(&self.temp_root).await;
    }
}
