//! Application state, background message handling and the main layout.

use crate::notify;
use eframe::egui;
use egui::{RichText, Ui};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use url::Url;
use waydlp_config::{Page, Settings, SettingsStore};
use waydlp_core::args::{DownloadRequest, EngineContext, build_probe_args};
use waydlp_core::errors::{ErrorKind, FailureReport};
use waydlp_core::media::ProbeResult;
use waydlp_core::options::DownloadOptions;
use waydlp_core::site::site_name;
use waydlp_engine::queue::{
    EventSink, JobId, JobSnapshot, JobSpec, JobState, QueueEvent, QueueHandle, QueueSettings, spawn_queue,
};
use waydlp_engine::updates::ReleaseInfo;
use waydlp_engine::{ToolSearch, ToolStatus, detect_tools};
use waydlp_history::{EntryStatus, History, HistoryEntry, NewEntry};
use waydlp_platform::{AppPaths, InstallKind};
use waydlp_theme::{Accent, ThemeOptions, palette::*};

/// Messages from background tasks to the UI thread.
pub enum BgMsg {
    Queue(QueueEvent),
    Tools(ToolStatus),
    Analysis { seq: u64, result: Result<ProbeResult, FailureReport> },
    Extractors(Result<Vec<String>, FailureReport>),
    InstallProgress { tool: &'static str, done: u64, total: Option<u64> },
    InstallDone { tool: &'static str, result: Result<String, String> },
    Update(Result<ReleaseInfo, String>),
    YtDlpLatest(Result<String, String>),
}

pub enum Analysis {
    Idle,
    Running { url: Url, seq: u64, cancel: CancellationToken, started: Instant },
    Failed { url: Url, error: FailureReport },
    Single { url: Url, info: Box<waydlp_core::MediaInfo> },
    Playlist { url: Url, info: waydlp_core::PlaylistInfo, selected: Vec<bool> },
}

pub struct Toast {
    pub text: String,
    pub color: egui::Color32,
    pub until: Instant,
}

pub struct Install {
    pub tool: &'static str,
    pub done: u64,
    pub total: Option<u64>,
}

/// What the confirmation modal is asking about.
pub enum Confirm {
    Quit,
    ClearHistory,
    DeleteFiles(i64),
    LargePlaylist { count: usize },
    ResetSettings,
}

pub struct WaydlpApp {
    pub rt: tokio::runtime::Runtime,
    pub paths: AppPaths,
    pub store: SettingsStore,
    pub settings: Settings,
    pub settings_problems: Vec<String>,
    pub history: Option<History>,
    pub queue: QueueHandle,
    pub jobs: HashMap<JobId, JobSnapshot>,
    pub job_order: Vec<JobId>,
    finalized: HashMap<JobId, (JobState, u32)>,
    pub tools: Option<ToolStatus>,
    pub tools_checking: bool,
    pub install_kind: InstallKind,
    pub bg_tx: Sender<BgMsg>,
    bg_rx: Receiver<BgMsg>,
    pub ctx: egui::Context,
    pub page: Page,

    // Home / downloader
    pub url_input: String,
    pub url_error: Option<String>,
    pub analysis: Analysis,
    analysis_seq: u64,
    pub dl_options: DownloadOptions,
    pub dl_preset: String,
    pub dl_output_dir: Option<PathBuf>,
    pub dl_template: String,
    pub new_preset_name: String,
    pub batch_input: String,
    pub batch_open: bool,
    pub batch_allow_playlists: bool,

    // Pages
    pub history_search: String,
    pub history_status: Option<EntryStatus>,
    pub history_site: Option<String>,
    pub history_rows: Vec<HistoryEntry>,
    pub history_dirty: bool,
    pub sites: Option<Result<Vec<String>, FailureReport>>,
    pub sites_loading: bool,
    pub sites_search: String,
    pub update: Option<Result<ReleaseInfo, String>>,
    pub update_checking: bool,
    pub yt_dlp_latest: Option<Result<String, String>>,
    pub install: Option<Install>,
    pub install_error: Option<String>,

    pub toasts: Vec<Toast>,
    pub confirm: Option<Confirm>,
    allow_quit: bool,
    clipboard: Option<arboard::Clipboard>,
    clipboard_last: String,
    clipboard_checked: Instant,
    pub clipboard_offer: Option<String>,
    applied_theme: Option<(ThemeOptions, f32)>,
    pending_url: Option<String>,
    settings_dirty_at: Option<Instant>,
}

impl WaydlpApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        rt: tokio::runtime::Runtime,
        paths: AppPaths,
        store: SettingsStore,
        settings: Settings,
        startup_warning: Option<String>,
    ) -> Self {
        let ctx = cc.egui_ctx.clone();
        egui_extras::install_image_loaders(&ctx);
        let (bg_tx, bg_rx) = channel();

        let mut toasts = Vec::new();
        let history = match History::open(&paths.history_db()) {
            Ok(h) => {
                let _ = h.mark_interrupted();
                let _ = h.prune_older_than(settings.privacy.history_retention_days);
                Some(h)
            }
            Err(e) => {
                toasts.push(Toast {
                    text: format!("History is unavailable: {e}"),
                    color: ERROR,
                    until: Instant::now() + Duration::from_secs(10),
                });
                None
            }
        };
        if let Some(w) = startup_warning {
            toasts.push(Toast { text: w, color: WARNING, until: Instant::now() + Duration::from_secs(12) });
        }

        let sink_tx = bg_tx.clone();
        let sink_ctx = ctx.clone();
        let sink: EventSink = std::sync::Arc::new(move |ev| {
            let _ = sink_tx.send(BgMsg::Queue(ev));
            sink_ctx.request_repaint();
        });
        let queue = spawn_queue(rt.handle(), paths.jobs_temp_dir.clone(), queue_settings(&settings), sink);

        let mut app = Self {
            dl_options: settings.default_options(),
            dl_preset: settings.downloads.default_preset.clone(),
            dl_template: settings.downloads.filename_template.clone(),
            page: settings.general.landing_page,
            settings_problems: settings.validate(),
            rt,
            paths,
            store,
            settings,
            history,
            queue,
            jobs: HashMap::new(),
            job_order: Vec::new(),
            finalized: HashMap::new(),
            tools: None,
            tools_checking: false,
            install_kind: waydlp_platform::detect_install_kind(),
            bg_tx,
            bg_rx,
            ctx,
            url_input: String::new(),
            url_error: None,
            analysis: Analysis::Idle,
            analysis_seq: 0,
            dl_output_dir: None,
            new_preset_name: String::new(),
            batch_input: String::new(),
            batch_open: false,
            batch_allow_playlists: false,
            history_search: String::new(),
            history_status: None,
            history_site: None,
            history_rows: Vec::new(),
            history_dirty: true,
            sites: None,
            sites_loading: false,
            sites_search: String::new(),
            update: None,
            update_checking: false,
            yt_dlp_latest: None,
            install: None,
            install_error: None,
            toasts,
            confirm: None,
            allow_quit: false,
            clipboard: None,
            clipboard_last: String::new(),
            clipboard_checked: Instant::now(),
            clipboard_offer: None,
            applied_theme: None,
            pending_url: None,
            settings_dirty_at: None,
        };
        app.apply_theme_if_needed();
        app.refresh_tools();
        // `waydlp <url>` (e.g. from a desktop entry or another program): analyse it once tools are known.
        app.pending_url = std::env::args().skip(1).find(|a| waydlp_core::urls::validate_url(a).is_ok());
        if app.settings.privacy.check_updates_on_start {
            app.check_for_updates();
        }
        app
    }

    // ----------------------------------------------------------------- helpers

    pub fn accent(&self) -> Accent {
        Accent::from_rgb(self.settings.appearance.accent)
    }

    pub fn animate(&self) -> bool {
        self.settings.appearance.animations && !self.settings.appearance.reduced_motion
    }

    pub fn toast(&mut self, text: impl Into<String>, color: egui::Color32) {
        self.toasts.push(Toast { text: text.into(), color, until: Instant::now() + Duration::from_secs(5) });
    }

    pub fn download_dir(&self) -> PathBuf {
        self.settings.general.download_dir.clone().unwrap_or_else(waydlp_platform::default_download_dir)
    }

    /// Call after changing `self.settings`; saves shortly after the last change.
    pub fn settings_changed(&mut self) {
        self.settings_problems = self.settings.validate();
        self.settings_dirty_at = Some(Instant::now());
        self.queue.set_settings(queue_settings(&self.settings));
    }

    fn save_settings_if_due(&mut self, force: bool) {
        if let Some(t) = self.settings_dirty_at
            && (force || t.elapsed() > Duration::from_millis(600))
        {
            self.settings_dirty_at = None;
            let mut s = self.settings.clone();
            s.normalise();
            if let Err(e) = self.store.save(&s) {
                self.toast(format!("Could not save settings: {e}"), ERROR);
            }
        }
    }

    pub fn tool_search(&self) -> ToolSearch {
        ToolSearch {
            custom_yt_dlp: self.settings.advanced.yt_dlp_path.clone(),
            custom_ffmpeg: self.settings.advanced.ffmpeg_path.clone(),
            managed_dir: Some(self.paths.bin_dir.clone()),
            bundled_dir: waydlp_platform::exe_dir(),
        }
    }

    pub fn refresh_tools(&mut self) {
        if self.tools_checking {
            return;
        }
        self.tools_checking = true;
        let search = self.tool_search();
        let tx = self.bg_tx.clone();
        let ctx = self.ctx.clone();
        self.rt.spawn(async move {
            let status = detect_tools(&search).await;
            let _ = tx.send(BgMsg::Tools(status));
            ctx.request_repaint();
        });
    }

    pub fn yt_dlp_path(&self) -> Option<PathBuf> {
        self.tools.as_ref()?.yt_dlp.as_ref().map(|t| t.path.clone())
    }

    pub fn has_ffmpeg(&self) -> bool {
        self.tools.as_ref().is_some_and(|t| t.ffmpeg.is_some())
    }

    pub fn engine_ctx(&self, output_dir: Option<PathBuf>) -> EngineContext {
        EngineContext {
            output_dir: output_dir.unwrap_or_else(|| self.download_dir()),
            temp_dir: None,
            ffmpeg_location: self.tools.as_ref().and_then(|t| t.ffmpeg_location()),
            archive_file: self.settings.downloads.use_download_archive.then(|| self.paths.archive_file()),
            cookies_file: self.settings.advanced.cookies_file.clone(),
            use_yt_dlp_config: self.settings.advanced.use_yt_dlp_config,
        }
    }

    // ----------------------------------------------------------------- actions

    /// Start analysing a URL (cancels a running analysis).
    pub fn analyze(&mut self, raw: &str, single_only: bool) {
        let url = match waydlp_core::urls::validate_url(raw) {
            Ok(u) => u,
            Err(e) => {
                self.url_error = Some(e.to_string());
                return;
            }
        };
        self.url_error = None;
        self.url_input = url.to_string();
        let Some(yt) = self.yt_dlp_path() else {
            self.analysis = Analysis::Failed {
                url,
                error: FailureReport::new(ErrorKind::YtDlpMissing, "yt-dlp is not installed", ""),
            };
            self.page = Page::Downloader;
            return;
        };
        if let Analysis::Running { cancel, .. } = &self.analysis {
            cancel.cancel();
        }
        // A link to a video inside a playlist: analyse the video unless asked otherwise.
        let has_video_and_list =
            url.query_pairs().any(|(k, _)| k == "v") && url.query_pairs().any(|(k, _)| k == "list");
        let no_playlist = single_only || has_video_and_list;
        let args = match build_probe_args(&url, &self.engine_ctx(None), &self.settings.network_options(), no_playlist) {
            Ok(a) => a,
            Err(e) => {
                self.url_error = Some(e.to_string());
                return;
            }
        };
        self.analysis_seq += 1;
        let seq = self.analysis_seq;
        let cancel = CancellationToken::new();
        let (tx, ctx, c2) = (self.bg_tx.clone(), self.ctx.clone(), cancel.clone());
        self.rt.spawn(async move {
            let result = waydlp_engine::run::probe(&yt, &args, Duration::from_secs(180), c2).await;
            let _ = tx.send(BgMsg::Analysis { seq, result });
            ctx.request_repaint();
        });
        self.analysis = Analysis::Running { url, seq, cancel, started: Instant::now() };
        self.dl_output_dir = None;
        self.page = Page::Downloader;
    }

    pub fn cancel_analysis(&mut self) {
        if let Analysis::Running { cancel, .. } = &self.analysis {
            cancel.cancel();
        }
        self.analysis = Analysis::Idle;
    }

    /// Queue a download. Returns false (with a toast) if it can't be queued.
    #[allow(clippy::too_many_arguments)]
    pub fn enqueue(
        &mut self,
        url: Url,
        options: DownloadOptions,
        is_playlist: bool,
        title: Option<String>,
        thumbnail: Option<String>,
        output_dir: Option<PathBuf>,
        template: Option<String>,
    ) -> bool {
        let Some(yt) = self.yt_dlp_path() else {
            self.toast("yt-dlp is not installed. Install it in Settings » yt-dlp & FFmpeg.", ERROR);
            return false;
        };
        let template = template.unwrap_or_else(|| self.settings.downloads.filename_template.clone());
        let request = DownloadRequest {
            url: url.clone(),
            options: options.clone(),
            filename_template: Some(template),
            is_playlist,
            custom_args: self.settings.advanced.custom_args.clone(),
        };
        let ctx = self.engine_ctx(output_dir);
        // Validate now so the user gets immediate feedback.
        if let Err(e) = waydlp_core::args::build_download_args(&request, &ctx, &self.settings.network_options()) {
            self.toast(e.to_string(), ERROR);
            return false;
        }
        let site = site_name(&url);
        let history_id = if self.settings.privacy.save_history {
            self.history.as_ref().and_then(|h| {
                h.insert(&NewEntry {
                    title: title.clone().unwrap_or_else(|| url.to_string()),
                    url: url.to_string(),
                    site: site.clone(),
                    format: options.summary(),
                    options_json: serde_json::to_string(&options).unwrap_or_default(),
                    is_playlist,
                    thumbnail: thumbnail.clone(),
                })
                .ok()
            })
        } else {
            None
        };
        self.history_dirty = true;
        self.queue.add(JobSpec {
            request,
            ctx,
            net: self.settings.network_options(),
            yt_dlp: yt,
            title,
            thumbnail,
            site,
            history_id,
        });
        true
    }

    /// One-click download of a URL with the default preset.
    pub fn quick_download(&mut self, raw: &str) {
        let url = match waydlp_core::urls::validate_url(raw) {
            Ok(u) => u,
            Err(e) => {
                self.url_error = Some(e.to_string());
                return;
            }
        };
        self.url_error = None;
        let has_video = url.query_pairs().any(|(k, _)| k == "v");
        if waydlp_core::urls::looks_like_playlist(&url) && !has_video {
            // Never download a whole playlist without showing it first.
            self.toast("This looks like a playlist — showing a preview first.", ACCENT_HOVER);
            self.analyze(raw, false);
            return;
        }
        let opts = self.settings.default_options();
        if self.enqueue(url, opts, false, None, None, None, None) {
            self.url_input.clear();
            self.toast("Added to the queue", SUCCESS);
        }
    }

    pub fn retry_history(&mut self, id: i64) {
        let Some(e) = self.history.as_ref().and_then(|h| h.get(id).ok().flatten()) else { return };
        let Ok(url) = Url::parse(&e.url) else { return };
        let options: DownloadOptions =
            serde_json::from_str(&e.options_json).unwrap_or_else(|_| self.settings.default_options());
        let title = Some(e.title.clone()).filter(|t| *t != e.url);
        if self.enqueue(url, options, e.is_playlist, title, e.thumbnail.clone(), None, None) {
            self.toast("Added to the queue", SUCCESS);
        }
    }

    pub fn check_for_updates(&mut self) {
        if self.update_checking {
            return;
        }
        self.update_checking = true;
        let (tx, ctx) = (self.bg_tx.clone(), self.ctx.clone());
        self.rt.spawn(async move {
            let client = waydlp_engine::http_client();
            let r =
                waydlp_engine::updates::check_latest(&client, waydlp_core::VERSION).await.map_err(|e| e.to_string());
            let _ = tx.send(BgMsg::Update(r));
            ctx.request_repaint();
        });
    }

    pub fn check_yt_dlp_latest(&mut self) {
        let (tx, ctx) = (self.bg_tx.clone(), self.ctx.clone());
        let bin = self.paths.bin_dir.clone();
        self.rt.spawn(async move {
            let m = waydlp_engine::managed::ManagedTools::new(waydlp_engine::http_client(), bin);
            let r = m.latest_yt_dlp_version().await.map_err(|e| e.to_string());
            let _ = tx.send(BgMsg::YtDlpLatest(r));
            ctx.request_repaint();
        });
    }

    /// Install/update a managed tool ("yt-dlp" or "ffmpeg").
    pub fn install_tool(&mut self, tool: &'static str) {
        if self.install.is_some() {
            return;
        }
        self.install_error = None;
        self.install = Some(Install { tool, done: 0, total: None });
        let (tx, ctx) = (self.bg_tx.clone(), self.ctx.clone());
        let bin = self.paths.bin_dir.clone();
        self.rt.spawn(async move {
            let m = waydlp_engine::managed::ManagedTools::new(waydlp_engine::http_client(), bin);
            let ptx = tx.clone();
            let pctx = ctx.clone();
            let last = std::sync::Mutex::new(Instant::now());
            let progress = move |done: u64, total: Option<u64>| {
                let mut l = last.lock().unwrap();
                if l.elapsed() > Duration::from_millis(100) || Some(done) == total {
                    *l = Instant::now();
                    let _ = ptx.send(BgMsg::InstallProgress { tool, done, total });
                    pctx.request_repaint();
                }
            };
            let result = match tool {
                "yt-dlp" => m.install_yt_dlp(&progress).await.map(|p| p.display().to_string()),
                _ => m.install_ffmpeg(&progress).await.map(|(p, _)| p.display().to_string()),
            }
            .map_err(|e| e.to_string());
            let _ = tx.send(BgMsg::InstallDone { tool, result });
            ctx.request_repaint();
        });
    }

    pub fn load_sites(&mut self) {
        let Some(yt) = self.yt_dlp_path() else { return };
        if self.sites_loading {
            return;
        }
        self.sites_loading = true;
        let (tx, ctx) = (self.bg_tx.clone(), self.ctx.clone());
        self.rt.spawn(async move {
            let r = waydlp_engine::run::list_extractors(&yt).await;
            let _ = tx.send(BgMsg::Extractors(r));
            ctx.request_repaint();
        });
    }

    pub fn active_job_count(&self) -> usize {
        self.jobs.values().filter(|j| j.state.is_active()).count()
    }

    // ------------------------------------------------------- background events

    fn process_messages(&mut self) {
        while let Ok(msg) = self.bg_rx.try_recv() {
            match msg {
                BgMsg::Queue(ev) => self.on_queue_event(ev),
                BgMsg::Tools(t) => {
                    self.tools_checking = false;
                    if let Some(u) = self.pending_url.take()
                        && t.yt_dlp.is_some()
                    {
                        self.tools = Some(t.clone());
                        self.analyze(&u, false);
                    }
                    for p in &t.problems {
                        self.toasts.push(Toast {
                            text: p.clone(),
                            color: WARNING,
                            until: Instant::now() + Duration::from_secs(8),
                        });
                    }
                    self.tools = Some(t);
                }
                BgMsg::Analysis { seq, result } => {
                    if let Analysis::Running { url, seq: s, .. } = &self.analysis
                        && *s == seq
                    {
                        let url = url.clone();
                        self.analysis = match result {
                            Ok(ProbeResult::Single(info)) => {
                                self.prepare_options_for(&info);
                                Analysis::Single { url, info }
                            }
                            Ok(ProbeResult::Playlist(info)) => {
                                let selected = vec![true; info.entries.len()];
                                Analysis::Playlist { url, info, selected }
                            }
                            Err(error) if error.kind == ErrorKind::Cancelled => Analysis::Idle,
                            Err(error) => Analysis::Failed { url, error },
                        };
                    }
                }
                BgMsg::Extractors(r) => {
                    self.sites_loading = false;
                    self.sites = Some(r);
                }
                BgMsg::InstallProgress { tool, done, total } => {
                    self.install = Some(Install { tool, done, total });
                }
                BgMsg::InstallDone { tool, result } => {
                    self.install = None;
                    match result {
                        Ok(path) => {
                            self.toast(format!("{tool} installed and verified ({path})"), SUCCESS);
                            self.yt_dlp_latest = None;
                            self.refresh_tools();
                            self.sites = None;
                        }
                        Err(e) => self.install_error = Some(format!("Installing {tool} failed: {e}")),
                    }
                }
                BgMsg::Update(r) => {
                    self.update_checking = false;
                    self.update = Some(r);
                }
                BgMsg::YtDlpLatest(r) => self.yt_dlp_latest = Some(r),
            }
        }
    }

    fn on_queue_event(&mut self, ev: QueueEvent) {
        match ev {
            QueueEvent::Updated(snap) => {
                let id = snap.id;
                if !self.job_order.contains(&id) {
                    self.job_order.push(id);
                }
                let terminal = snap.state.is_finished();
                let key = (snap.state, snap.attempts);
                if terminal && self.finalized.get(&id) != Some(&key) {
                    self.finalized.insert(id, key);
                    self.on_job_finished(&snap);
                }
                self.jobs.insert(id, snap);
            }
            QueueEvent::Removed(id) => {
                if let Some(j) = self.jobs.remove(&id)
                    && !j.state.is_finished()
                    && let (Some(h), Some(hid)) = (&self.history, j.spec.history_id)
                {
                    let _ = h.finish(hid, EntryStatus::Cancelled, &j.files, Some("Removed from the queue"));
                    self.history_dirty = true;
                }
                self.job_order.retain(|x| *x != id);
                self.finalized.remove(&id);
            }
            QueueEvent::Order(order) => self.job_order = order,
        }
    }

    fn on_job_finished(&mut self, s: &JobSnapshot) {
        let title = s.display_title();
        if let (Some(h), Some(hid)) = (&self.history, s.spec.history_id) {
            let status = match s.state {
                JobState::Completed => EntryStatus::Completed,
                JobState::Cancelled => EntryStatus::Cancelled,
                _ => EntryStatus::Failed,
            };
            if s.current_title.is_some() && s.spec.title.is_none() && !s.spec.request.is_playlist {
                let _ = h.update_title(hid, &title);
            }
            let err = s.error.as_ref().map(|e| format!("{}: {}", e.kind.title(), e.summary));
            let _ = h.finish(hid, status, &s.files, err.as_deref());
            self.history_dirty = true;
        }
        let focused = self.ctx.input(|i| i.focused);
        match s.state {
            JobState::Completed => {
                if self.settings.general.notify_on_complete && !focused {
                    notify::send("Download complete".into(), title.clone());
                }
                if self.settings.general.open_folder_when_done
                    && let Some(f) = s.files.first()
                {
                    let _ = waydlp_platform::reveal_path(f);
                }
                self.toast(format!("Downloaded: {title}"), SUCCESS);
            }
            JobState::Failed => {
                let why = s.error.as_ref().map(|e| e.kind.title()).unwrap_or("Failed");
                if self.settings.general.notify_on_failure && !focused {
                    notify::send(format!("Download failed: {why}"), title.clone());
                }
                self.toast(format!("Failed: {title} — {why}"), ERROR);
            }
            _ => {}
        }
    }

    /// Pre-select options that make sense for the analysed media.
    fn prepare_options_for(&mut self, info: &waydlp_core::MediaInfo) {
        let a = info.analysis();
        let o = &mut self.dl_options;
        o.exact_format = None;
        if !a.has_video
            && a.has_audio
            && matches!(o.mode, waydlp_core::DownloadMode::VideoAndAudio | waydlp_core::DownloadMode::VideoOnly)
        {
            o.mode = waydlp_core::DownloadMode::AudioOnly;
        }
        // Keep the preset's max height only if something at or below it exists.
        if let Some(h) = o.video.max_height
            && !a.heights.is_empty()
            && !a.heights.iter().any(|x| *x <= h)
        {
            o.video.max_height = None;
        }
        let subs = info.subtitle_languages();
        if !subs.is_empty() {
            o.subtitles.languages.retain(|l| subs.contains(l));
        }
    }

    fn handle_clipboard(&mut self) {
        if !self.settings.privacy.clipboard_watch {
            return;
        }
        if self.clipboard_checked.elapsed() < Duration::from_secs(1) || !self.ctx.input(|i| i.focused) {
            self.ctx.request_repaint_after(Duration::from_secs(1));
            return;
        }
        self.clipboard_checked = Instant::now();
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        let Some(cb) = &mut self.clipboard else { return };
        if let Ok(text) = cb.get_text() {
            let text = text.trim().to_string();
            if text != self.clipboard_last {
                self.clipboard_last = text.clone();
                if text.len() < 2048 && waydlp_core::urls::validate_url(&text).is_ok() && text != self.url_input {
                    self.clipboard_offer = Some(text);
                }
            }
        }
        self.ctx.request_repaint_after(Duration::from_secs(1));
    }

    pub fn read_clipboard(&mut self) -> Option<String> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        self.clipboard.as_mut()?.get_text().ok().map(|t| t.trim().to_string())
    }

    fn apply_theme_if_needed(&mut self) {
        let ap = &self.settings.appearance;
        let opts = ThemeOptions {
            accent: Accent::from_rgb(ap.accent),
            compact: ap.density == waydlp_config::Density::Compact,
            reduced_motion: ap.reduced_motion,
            animations: ap.animations,
        };
        let want = (opts, ap.ui_scale);
        if self.applied_theme != Some(want) {
            waydlp_theme::apply(&self.ctx, opts);
            self.ctx.set_zoom_factor(ap.ui_scale.clamp(0.75, 2.0));
            self.applied_theme = Some(want);
        }
    }

    // --------------------------------------------------------------------- UI

    fn sidebar(&mut self, ui: &mut Ui) {
        let accent = self.accent();
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            waydlp_theme::logo(ui, 34.0, accent);
            ui.vertical(|ui| {
                ui.label(RichText::new("waydlp").size(20.0).strong().color(TEXT_PRIMARY));
                ui.label(RichText::new(format!("v{}", waydlp_core::VERSION)).small().color(TEXT_MUTED));
            });
        });
        ui.add_space(18.0);
        let active = self.active_job_count();
        let nav = [
            (Page::Home, "🏠", "Home"),
            (Page::Downloader, "⬇", "Downloader"),
            (Page::Queue, "☰", "Queue"),
            (Page::History, "🕘", "History"),
            (Page::Sites, "🌐", "Supported Websites"),
            (Page::Settings, "⚙", "Settings"),
            (Page::About, "ℹ", "About"),
        ];
        for (page, icon, label) in nav {
            let selected = self.page == page;
            let text = if page == Page::Queue && active > 0 {
                format!("{icon}  {label}  ({active})")
            } else {
                format!("{icon}  {label}")
            };
            let rich = if selected {
                RichText::new(text).color(TEXT_PRIMARY).strong()
            } else {
                RichText::new(text).color(TEXT_SECONDARY)
            };
            let btn = egui::Button::new(rich)
                .fill(if selected {
                    waydlp_theme::mix(BG_PRIMARY, accent.primary, 0.22)
                } else {
                    egui::Color32::TRANSPARENT
                })
                .stroke(if selected { egui::Stroke::new(1.0, accent.dark) } else { egui::Stroke::NONE })
                .min_size(egui::vec2(ui.available_width() - 8.0, 34.0));
            let r = ui.add(btn);
            if r.clicked() {
                self.page = page;
                if page == Page::History {
                    self.history_dirty = true;
                }
            }
            ui.add_space(2.0);
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(12.0);
            match &self.tools {
                None => {
                    ui.label(waydlp_theme::muted("Checking tools…"));
                }
                Some(t) => {
                    let (c, txt) =
                        if t.yt_dlp.is_some() { (SUCCESS, "yt-dlp ready") } else { (ERROR, "yt-dlp missing") };
                    ui.label(RichText::new(format!("• {txt}")).small().color(c));
                    let (c, txt) =
                        if t.ffmpeg.is_some() { (SUCCESS, "FFmpeg ready") } else { (WARNING, "FFmpeg missing") };
                    ui.label(RichText::new(format!("• {txt}")).small().color(c));
                }
            }
        });
    }

    fn toasts_ui(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.toasts.retain(|t| t.until > now);
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::unique("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-18.0, -18.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let mut dismiss = None;
                for (i, t) in self.toasts.iter().enumerate().rev().take(4) {
                    let r = egui::Frame::new()
                        .fill(SURFACE_SECONDARY)
                        .stroke(egui::Stroke::new(1.0, waydlp_theme::mix(BORDER_PRIMARY, t.color, 0.6)))
                        .corner_radius(10)
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.set_max_width(420.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("•").color(t.color));
                                ui.label(RichText::new(&t.text).color(TEXT_PRIMARY));
                            });
                        })
                        .response
                        .interact(egui::Sense::click());
                    if r.clicked() {
                        dismiss = Some(i);
                    }
                    ui.add_space(6.0);
                }
                if let Some(i) = dismiss {
                    self.toasts.remove(i);
                }
            });
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn confirm_ui(&mut self, ctx: &egui::Context) {
        let Some(confirm) = &self.confirm else { return };
        let (title, body, ok_label, danger) = match confirm {
            Confirm::Quit => (
                "Quit waydlp?",
                format!("{} download(s) are still running or queued. Quitting stops them; partial files are removed.", self.active_job_count()),
                "Quit anyway",
                true,
            ),
            Confirm::ClearHistory => ("Clear download history?", "All history entries are removed. Downloaded files are not deleted.".to_string(), "Clear history", true),
            Confirm::DeleteFiles(id) => {
                let files = self
                    .history
                    .as_ref()
                    .and_then(|h| h.get(*id).ok().flatten())
                    .map(|e| e.files)
                    .unwrap_or_default();
                let list = files.iter().map(|f| f.display().to_string()).collect::<Vec<_>>().join("\n");
                ("Delete downloaded files?", format!("These files will be permanently deleted from your disk:\n\n{list}"), "Delete files", true)
            }
            Confirm::LargePlaylist { count } => (
                "Download a large playlist?",
                format!("You are about to download {count} items. Make sure you have enough disk space and bandwidth."),
                "Download all",
                false,
            ),
            Confirm::ResetSettings => ("Reset all settings?", "Every setting returns to its default. Your presets are removed. History and downloaded files are kept.".to_string(), "Reset", true),
        };
        let accent = self.accent();
        let mut result: Option<bool> = None;
        let modal = egui::Modal::new(egui::Id::unique("confirm-modal"))
            .frame(waydlp_theme::card().fill(BG_SECONDARY).inner_margin(egui::Margin::same(22)))
            .show(ctx, |ui| {
                ui.set_max_width(460.0);
                waydlp_theme::subheading(ui, title);
                ui.add_space(6.0);
                ui.label(RichText::new(body).color(TEXT_SECONDARY));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let ok = if danger {
                        ui.add(
                            egui::Button::new(RichText::new(ok_label).color(egui::Color32::WHITE).strong()).fill(ERROR),
                        )
                    } else {
                        waydlp_theme::primary_button(ui, accent, ok_label)
                    };
                    if ok.clicked() {
                        result = Some(true);
                    }
                    if waydlp_theme::ghost_button(ui, "Cancel").clicked() {
                        result = Some(false);
                    }
                });
            });
        if modal.should_close() && result.is_none() {
            result = Some(false);
        }
        if let Some(ok) = result {
            let c = self.confirm.take().expect("checked above");
            if ok {
                self.on_confirmed(c, ctx);
            }
        }
    }

    fn on_confirmed(&mut self, c: Confirm, ctx: &egui::Context) {
        match c {
            Confirm::Quit => {
                self.allow_quit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Confirm::ClearHistory => {
                if let Some(h) = &self.history {
                    match h.clear() {
                        Ok(n) => self.toast(format!("Removed {n} history entries"), SUCCESS),
                        Err(e) => self.toast(e.to_string(), ERROR),
                    }
                }
                self.history_dirty = true;
            }
            Confirm::DeleteFiles(id) => {
                let Some(e) = self.history.as_ref().and_then(|h| h.get(id).ok().flatten()) else { return };
                let mut deleted = 0;
                let mut errors = Vec::new();
                for f in &e.files {
                    match std::fs::remove_file(f) {
                        Ok(()) => deleted += 1,
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                        Err(err) => errors.push(format!("{}: {err}", f.display())),
                    }
                }
                if errors.is_empty() {
                    if let Some(h) = &self.history {
                        let _ = h.remove(id);
                    }
                    self.toast(format!("Deleted {deleted} file(s)"), SUCCESS);
                } else {
                    self.toast(format!("Some files could not be deleted: {}", errors.join("; ")), ERROR);
                }
                self.history_dirty = true;
            }
            Confirm::LargePlaylist { .. } => self.enqueue_playlist(true),
            Confirm::ResetSettings => {
                let paths_keep =
                    (self.settings.advanced.yt_dlp_path.clone(), self.settings.advanced.ffmpeg_path.clone());
                self.settings = Settings::default();
                self.settings.advanced.yt_dlp_path = paths_keep.0;
                self.settings.advanced.ffmpeg_path = paths_keep.1;
                self.settings_changed();
                self.toast("Settings reset to defaults", SUCCESS);
            }
        }
    }

    /// Queue the analysed playlist with the current selection.
    pub fn enqueue_playlist(&mut self, confirmed: bool) {
        let Analysis::Playlist { url, info, selected } = &self.analysis else { return };
        let indices: Vec<usize> =
            info.entries.iter().zip(selected).filter(|(_, s)| **s).map(|(e, _)| e.index).collect();
        if indices.is_empty() {
            self.toast("Select at least one item", WARNING);
            return;
        }
        if !confirmed && indices.len() > self.settings.downloads.playlist_confirm_threshold {
            self.confirm = Some(Confirm::LargePlaylist { count: indices.len() });
            return;
        }
        let all = indices.len() == info.entries.len();
        let mut opts = self.dl_options.clone();
        opts.exact_format = None;
        opts.playlist.items = if all { None } else { Some(crate::widgets::items_spec(&indices)) };
        let (url, title) = (url.clone(), Some(info.display_title()));
        let out = self.dl_output_dir.clone();
        let tpl = Some(self.dl_template.clone());
        if self.enqueue(url, opts, true, title, None, out, tpl) {
            self.toast(format!("Added {} item(s) to the queue", indices.len()), SUCCESS);
            self.page = Page::Queue;
        }
    }
}

pub fn queue_settings(s: &Settings) -> QueueSettings {
    QueueSettings {
        max_concurrent: s.downloads.max_concurrent.clamp(1, 16),
        auto_retries: s.downloads.auto_retries,
        ..Default::default()
    }
}

impl eframe::App for WaydlpApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.process_messages();
        self.apply_theme_if_needed();
        self.handle_clipboard();
        self.save_settings_if_due(false);
        if self.settings_dirty_at.is_some() {
            ctx.request_repaint_after(Duration::from_millis(700));
        }

        // Quit confirmation while downloads are active.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_quit
            && self.settings.general.confirm_quit_with_active
            && self.active_job_count() > 0
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::Quit);
        }

        // Ctrl+V anywhere (outside a text field) pastes a link and analyses it.
        if !ctx.egui_wants_keyboard_input() {
            let pasted = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Paste(s) => Some(s.clone()),
                    _ => None,
                })
            });
            if let Some(p) = pasted
                && waydlp_core::urls::validate_url(&p).is_ok()
            {
                self.url_input = p.trim().to_string();
                let input = self.url_input.clone();
                self.analyze(&input, false);
            }
        }

        egui::Panel::left("nav")
            .exact_size(212.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(BG_SECONDARY)
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .stroke(egui::Stroke::new(1.0, BORDER_PRIMARY)),
            )
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(BG_PRIMARY).inner_margin(egui::Margin {
                left: 28,
                right: 28,
                top: 22,
                bottom: 16,
            }))
            .show(ui, |ui| {
                if let Some(link) = self.clipboard_offer.clone() {
                    let accent = self.accent();
                    waydlp_theme::accent_card(accent).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("📋 Link in clipboard:").color(TEXT_SECONDARY));
                            ui.label(RichText::new(truncate(&link, 70)).color(TEXT_PRIMARY));
                            if waydlp_theme::primary_button(ui, accent, "Analyze").clicked() {
                                self.clipboard_offer = None;
                                self.url_input = link.clone();
                                self.analyze(&link, false);
                            }
                            if ui.button("Dismiss").clicked() {
                                self.clipboard_offer = None;
                            }
                        });
                    });
                    ui.add_space(10.0);
                }
                egui::ScrollArea::vertical().id_salt(("page", self.page)).auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_max_width(ui.available_width().min(1100.0));
                    match self.page {
                        Page::Home => self.page_home(ui),
                        Page::Downloader => self.page_downloader(ui),
                        Page::Queue => self.page_queue(ui),
                        Page::History => self.page_history(ui),
                        Page::Sites => self.page_sites(ui),
                        Page::Settings => self.page_settings(ui),
                        Page::About => self.page_about(ui),
                    }
                    ui.add_space(24.0);
                });
            });

        self.confirm_ui(&ctx);
        self.toasts_ui(&ctx);

        // Keep progress bars and ETAs fresh while something is running.
        if self.active_job_count() > 0 || matches!(self.analysis, Analysis::Running { .. }) || self.install.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_settings_if_due(true);
        // Record interrupted downloads, then stop all processes and clean up.
        if let Some(h) = &self.history {
            for j in self.jobs.values().filter(|j| j.state.is_active() || j.state == JobState::Paused) {
                if let Some(hid) = j.spec.history_id {
                    let _ = h.finish(
                        hid,
                        EntryStatus::Cancelled,
                        &j.files,
                        Some("Interrupted: waydlp was closed before the download finished."),
                    );
                }
            }
        }
        let q = self.queue.clone();
        self.rt.block_on(async move {
            let _ = tokio::time::timeout(Duration::from_secs(15), q.shutdown()).await;
        });
        tracing::info!("waydlp exited cleanly");
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}
