//! Integration tests for the queue and runner, using the fake yt-dlp binary.
//! Run with: `cargo test -p waydlp-engine --features fake-yt-dlp`
#![cfg(feature = "fake-yt-dlp")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use url::Url;
use waydlp_core::args::{DownloadRequest, EngineContext, NetworkOptions, build_probe_args};
use waydlp_core::errors::ErrorKind;
use waydlp_core::media::ProbeResult;
use waydlp_core::options::DownloadOptions;
use waydlp_engine::queue::*;

fn fake() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_waydlp-fake-yt-dlp"))
}

struct Harness {
    handle: QueueHandle,
    state: Arc<Mutex<HashMap<JobId, JobSnapshot>>>,
    progress_seen: Arc<Mutex<HashMap<JobId, usize>>>,
    out: tempfile::TempDir,
    temp_root: PathBuf,
}

impl Harness {
    fn new(max_concurrent: usize, auto_retries: u32) -> Self {
        let out = tempfile::tempdir().unwrap();
        let temp_root = out.path().join("tmp");
        let state: Arc<Mutex<HashMap<JobId, JobSnapshot>>> = Default::default();
        let progress_seen: Arc<Mutex<HashMap<JobId, usize>>> = Default::default();
        let (s, p) = (state.clone(), progress_seen.clone());
        let sink: EventSink = Arc::new(move |ev| {
            if let QueueEvent::Updated(snap) = ev {
                if snap.progress.is_some() {
                    *p.lock().unwrap().entry(snap.id).or_default() += 1;
                }
                s.lock().unwrap().insert(snap.id, snap);
            }
        });
        let handle = spawn_queue(
            &tokio::runtime::Handle::current(),
            temp_root.clone(),
            QueueSettings { max_concurrent, auto_retries, retry_base_delay: Duration::from_millis(20) },
            sink,
        );
        Self { handle, state, progress_seen, out, temp_root }
    }

    fn spec(&self, path: &str) -> JobSpec {
        JobSpec {
            request: DownloadRequest {
                url: Url::parse(&format!("https://fake.test/{path}")).unwrap(),
                options: DownloadOptions::default(),
                filename_template: None,
                is_playlist: false,
                custom_args: String::new(),
            },
            ctx: EngineContext { output_dir: self.out.path().join("downloads"), ..Default::default() },
            net: NetworkOptions { retries: 1, fragment_retries: 1, ..Default::default() },
            yt_dlp: fake(),
            title: None,
            thumbnail: None,
            site: "fake".into(),
            history_id: None,
        }
    }

    fn snap(&self, id: JobId) -> Option<JobSnapshot> {
        self.state.lock().unwrap().get(&id).cloned()
    }

    /// Wait until the job has reported real progress (the process is up).
    async fn wait_progress(&self, id: JobId) {
        for _ in 0..400 {
            if *self.progress_seen.lock().unwrap().get(&id).unwrap_or(&0) > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("job {id} never reported progress");
    }

    async fn wait_for(&self, id: JobId, want: JobState) -> JobSnapshot {
        for _ in 0..400 {
            if let Some(s) = self.snap(id)
                && s.state == want
            {
                return s;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("job {id} never reached {want:?}; last = {:?}", self.snap(id).map(|s| (s.state, s.stage)));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn successful_download_reports_real_progress_and_file() {
    let h = Harness::new(2, 0);
    let id = h.handle.add(h.spec("ok"));
    let s = h.wait_for(id, JobState::Completed).await;
    assert_eq!(s.files.len(), 1);
    assert!(s.files[0].exists());
    assert_eq!(s.current_title.as_deref(), Some("Fake video"));
    assert!(s.warnings.iter().any(|w| w.contains("fake warning")));
    assert!(*h.progress_seen.lock().unwrap().get(&id).unwrap_or(&0) >= 1);
    // Temp dir cleaned up after success.
    assert!(std::fs::read_dir(&h.temp_root).map(|d| d.count()).unwrap_or(0) == 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permanent_failure_is_not_retried() {
    let h = Harness::new(2, 3);
    let id = h.handle.add(h.spec("private"));
    let s = h.wait_for(id, JobState::Failed).await;
    assert_eq!(s.attempts, 1);
    assert_eq!(s.error.unwrap().kind, ErrorKind::Private);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transient_failure_is_retried_then_fails() {
    let h = Harness::new(2, 2);
    let id = h.handle.add(h.spec("net"));
    let s = h.wait_for(id, JobState::Failed).await;
    assert_eq!(s.attempts, 3);
    assert_eq!(s.error.unwrap().kind, ErrorKind::Network);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flaky_download_succeeds_on_retry() {
    let h = Harness::new(2, 1);
    let id = h.handle.add(h.spec("flaky"));
    let s = h.wait_for(id, JobState::Completed).await;
    assert_eq!(s.attempts, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_kills_process_and_removes_partial_files() {
    let h = Harness::new(2, 0);
    let id = h.handle.add(h.spec("slow"));
    h.wait_for(id, JobState::Running).await;
    h.wait_progress(id).await;
    let job_dir = std::fs::read_dir(&h.temp_root).unwrap().next().unwrap().unwrap().path();
    assert!(job_dir.join("video.mp4.part").exists());
    h.handle.cancel(id);
    let s = h.wait_for(id, JobState::Cancelled).await;
    assert_eq!(s.error.unwrap().kind, ErrorKind::Cancelled);
    assert!(!job_dir.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pause_keeps_partial_data_and_resume_restarts() {
    let h = Harness::new(2, 0);
    let id = h.handle.add(h.spec("slow"));
    h.wait_for(id, JobState::Running).await;
    h.wait_progress(id).await;
    h.handle.pause(id);
    h.wait_for(id, JobState::Paused).await;
    let job_dir = std::fs::read_dir(&h.temp_root).unwrap().next().unwrap().unwrap().path();
    assert!(job_dir.join("video.mp4.part").exists(), "partial data kept while paused");
    h.handle.resume(id);
    let s = h.wait_for(id, JobState::Running).await;
    assert_eq!(s.attempts, 2);
    h.handle.cancel(id);
    h.wait_for(id, JobState::Cancelled).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrency_limit_is_respected_and_queue_advances() {
    let h = Harness::new(1, 0);
    let a = h.handle.add(h.spec("slow"));
    let b = h.handle.add(h.spec("ok"));
    h.wait_for(a, JobState::Running).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(h.snap(b).unwrap().state, JobState::Queued);
    h.handle.cancel(a);
    h.wait_for(b, JobState::Completed).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remove_and_clear_finished() {
    let h = Harness::new(2, 0);
    let id = h.handle.add(h.spec("ok"));
    h.wait_for(id, JobState::Completed).await;
    h.handle.clear_finished();
    h.handle.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_parses_and_classifies() {
    let ctx = EngineContext { output_dir: PathBuf::from("."), ..Default::default() };
    let net = NetworkOptions::default();
    let url = Url::parse("https://fake.test/video").unwrap();
    let args = build_probe_args(&url, &ctx, &net, true).unwrap();
    let r = waydlp_engine::run::probe(&fake(), &args, Duration::from_secs(20), CancellationToken::new()).await.unwrap();
    let ProbeResult::Single(m) = r else { panic!("expected single") };
    assert_eq!(m.display_title(), "Fake video");
    assert_eq!(m.analysis().heights, vec![360]);

    let url = Url::parse("https://fake.test/missing").unwrap();
    let args = build_probe_args(&url, &ctx, &net, true).unwrap();
    let e =
        waydlp_engine::run::probe(&fake(), &args, Duration::from_secs(20), CancellationToken::new()).await.unwrap_err();
    assert_eq!(e.kind, ErrorKind::UnsupportedUrl);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_binary_is_reported() {
    let e = waydlp_engine::run::probe(
        &PathBuf::from("/no/such/yt-dlp-binary"),
        &["--version".to_string()],
        Duration::from_secs(5),
        CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::YtDlpMissing);
}
