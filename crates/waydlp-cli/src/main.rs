//! `waydlp-cli`: scriptable access to the waydlp engine.
//!
//! It uses the same settings, presets, managed tools and history as the GUI.

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use waydlp_config::{Settings, SettingsStore};
use waydlp_core::args::{DownloadRequest, EngineContext, build_probe_args};
use waydlp_core::humanize;
use waydlp_core::media::ProbeResult;
use waydlp_core::options::*;
use waydlp_core::site::site_name;
use waydlp_core::urls::{extract_urls, validate_url};
use waydlp_engine::managed::ManagedTools;
use waydlp_engine::queue::*;
use waydlp_engine::{ToolSearch, ToolStatus, detect_tools, http_client};
use waydlp_history::{EntryStatus, History, HistoryFilter, NewEntry};
use waydlp_platform::AppPaths;

#[derive(Parser)]
#[command(name = "waydlp-cli", version, about = "waydlp: your media, your format, your way.")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum AudioFmt {
    Original,
    Mp3,
    M4a,
    Flac,
    Wav,
    Opus,
    Ogg,
}

#[derive(Clone, Copy, ValueEnum)]
enum VideoFmt {
    Mp4,
    Mkv,
    Webm,
    Mov,
    Original,
}

#[derive(Clone, Copy, ValueEnum)]
enum Tool {
    YtDlp,
    Ffmpeg,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show real metadata and available formats for a URL.
    Analyze {
        url: String,
        /// Print the raw parsed metadata as JSON.
        #[arg(long)]
        json: bool,
        /// Treat URLs that point at both a video and a playlist as the video.
        #[arg(long)]
        no_playlist: bool,
    },
    /// Download one or more URLs (or `-` to read URLs from stdin).
    Download {
        urls: Vec<String>,
        /// Preset id (see `waydlp-cli presets`).
        #[arg(long, short)]
        preset: Option<String>,
        /// Extract audio in this format.
        #[arg(long, value_enum, conflicts_with = "video")]
        audio: Option<AudioFmt>,
        /// Audio bitrate target in kbps (lossy formats only).
        #[arg(long)]
        bitrate: Option<u32>,
        /// Video container.
        #[arg(long, value_enum)]
        video: Option<VideoFmt>,
        /// Maximum video height, e.g. 1080.
        #[arg(long)]
        max_height: Option<u32>,
        /// Download this exact yt-dlp format id (from `analyze`).
        #[arg(long)]
        format_id: Option<String>,
        /// Output directory (default: settings, else Downloads/waydlp).
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// Filename template, e.g. "%(title)s.%(ext)s".
        #[arg(long)]
        template: Option<String>,
        /// Download the whole playlist when the URL is one.
        #[arg(long)]
        playlist: bool,
        /// Playlist items to download, e.g. 1,3,5-7.
        #[arg(long)]
        items: Option<String>,
        /// Concurrent downloads (default: settings).
        #[arg(long, short = 'j')]
        jobs: Option<usize>,
        /// Do not record the downloads in history.
        #[arg(long)]
        no_history: bool,
    },
    /// Show detected yt-dlp / FFmpeg.
    Tools,
    /// Install or update a waydlp-managed copy of a tool (checksum-verified).
    InstallTool {
        #[arg(value_enum)]
        tool: Tool,
    },
    /// List presets.
    Presets,
    /// Show download history.
    History {
        #[arg(long, short)]
        search: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// List the extractors (sites) supported by the installed yt-dlp.
    Extractors,
    /// Print the paths waydlp uses.
    Paths,
}

struct Env {
    paths: AppPaths,
    settings: Settings,
}

impl Env {
    fn load() -> Result<Self> {
        let paths = AppPaths::resolve();
        paths.ensure().context("creating waydlp directories")?;
        let (settings, warning) = SettingsStore::new(paths.settings_file()).load();
        if let Some(w) = warning {
            eprintln!("warning: {w}");
        }
        Ok(Self { paths, settings })
    }

    fn tool_search(&self) -> ToolSearch {
        ToolSearch {
            custom_yt_dlp: self.settings.advanced.yt_dlp_path.clone(),
            custom_ffmpeg: self.settings.advanced.ffmpeg_path.clone(),
            managed_dir: Some(self.paths.bin_dir.clone()),
            bundled_dir: waydlp_platform::exe_dir(),
        }
    }

    async fn tools(&self) -> ToolStatus {
        detect_tools(&self.tool_search()).await
    }

    fn ctx(&self, tools: &ToolStatus, output: Option<PathBuf>) -> EngineContext {
        EngineContext {
            output_dir: output
                .or_else(|| self.settings.general.download_dir.clone())
                .unwrap_or_else(waydlp_platform::default_download_dir),
            temp_dir: None,
            ffmpeg_location: tools.ffmpeg_location(),
            archive_file: self.settings.downloads.use_download_archive.then(|| self.paths.archive_file()),
            cookies_file: self.settings.advanced.cookies_file.clone(),
            use_yt_dlp_config: self.settings.advanced.use_yt_dlp_config,
        }
    }
}

fn require_yt_dlp(t: &ToolStatus) -> Result<PathBuf> {
    t.yt_dlp
        .as_ref()
        .map(|i| i.path.clone())
        .ok_or_else(|| anyhow!("yt-dlp was not found. Install it with: waydlp-cli install-tool yt-dlp"))
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let env = Env::load()?;
    match cli.cmd {
        Cmd::Paths => {
            let p = &env.paths;
            println!("portable:  {}", p.portable);
            println!("settings:  {}", p.settings_file().display());
            println!("history:   {}", p.history_db().display());
            println!("tools:     {}", p.bin_dir.display());
            println!("logs:      {}", p.log_dir.display());
            println!("install:   {}", waydlp_platform::detect_install_kind().label());
        }
        Cmd::Tools => {
            let t = env.tools().await;
            for (name, info) in [("yt-dlp", &t.yt_dlp), ("ffmpeg", &t.ffmpeg), ("ffprobe", &t.ffprobe)] {
                match info {
                    Some(i) => println!(
                        "{name:8} {} ({}) — {}",
                        i.version.as_deref().unwrap_or("unknown version"),
                        i.source.label(),
                        i.path.display()
                    ),
                    None => println!("{name:8} not found"),
                }
            }
            for p in &t.problems {
                println!("warning: {p}");
            }
        }
        Cmd::InstallTool { tool } => {
            let m = ManagedTools::new(http_client(), env.paths.bin_dir.clone());
            let last = Mutex::new(0u64);
            let progress = move |done: u64, total: Option<u64>| {
                let mut l = last.lock().unwrap();
                if done - *l >= 1_000_000 || Some(done) == total {
                    *l = done;
                    match total {
                        Some(t) => eprint!("\r  {} / {}   ", humanize::bytes(done), humanize::bytes(t)),
                        None => eprint!("\r  {}   ", humanize::bytes(done)),
                    }
                }
            };
            match tool {
                Tool::YtDlp => {
                    let p = m.install_yt_dlp(&progress).await?;
                    eprintln!();
                    println!("Installed and verified yt-dlp at {}", p.display());
                }
                Tool::Ffmpeg => {
                    let (a, b) = m.install_ffmpeg(&progress).await?;
                    eprintln!();
                    println!("Installed and verified {} and {}", a.display(), b.display());
                }
            }
        }
        Cmd::Presets => {
            for p in env.settings.all_presets() {
                println!(
                    "{:14} {:22} {}{}",
                    p.id,
                    p.name,
                    p.options.summary(),
                    if p.id == env.settings.downloads.default_preset { "  (default)" } else { "" }
                );
            }
        }
        Cmd::History { search, limit } => {
            let h = History::open(&env.paths.history_db())?;
            let rows = h.list(&HistoryFilter {
                search: search.unwrap_or_default(),
                limit: Some(limit),
                ..Default::default()
            })?;
            if rows.is_empty() {
                println!("No downloads yet.");
            }
            for r in rows {
                println!(
                    "{}  {:10} {:12} {}  [{}]",
                    r.created_at.format("%Y-%m-%d %H:%M"),
                    r.status.label(),
                    r.site,
                    r.title,
                    r.format
                );
            }
        }
        Cmd::Extractors => {
            let t = env.tools().await;
            let yt = require_yt_dlp(&t)?;
            let list = waydlp_engine::run::list_extractors(&yt).await.map_err(|e| anyhow!(e.summary))?;
            for l in &list {
                println!("{l}");
            }
            eprintln!("{} extractors", list.len());
        }
        Cmd::Analyze { url, json, no_playlist } => {
            let url = validate_url(&url).map_err(|e| anyhow!(e))?;
            let t = env.tools().await;
            let yt = require_yt_dlp(&t)?;
            let args = build_probe_args(&url, &env.ctx(&t, None), &env.settings.network_options(), no_playlist)?;
            eprintln!("Analysing {url} …");
            let r = waydlp_engine::run::probe(&yt, &args, Duration::from_secs(180), CancellationToken::new())
                .await
                .map_err(|e| anyhow!("{}: {}\n{}", e.kind.title(), e.summary, e.kind.hint()))?;
            print_probe(&r, json)?;
        }
        Cmd::Download {
            urls,
            preset,
            audio,
            bitrate,
            video,
            max_height,
            format_id,
            output,
            template,
            playlist,
            items,
            jobs,
            no_history,
        } => {
            let mut text = urls.iter().filter(|u| *u != "-").cloned().collect::<Vec<_>>().join("\n");
            if urls.iter().any(|u| u == "-") {
                let mut s = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)?;
                text.push('\n');
                text.push_str(&s);
            }
            let (urls, bad) = extract_urls(&text);
            for b in bad {
                eprintln!("skipping invalid URL: {b}");
            }
            if urls.is_empty() {
                bail!("no valid URLs given");
            }
            let mut opts = match preset {
                Some(id) => env.settings.preset(&id).ok_or_else(|| anyhow!("unknown preset \"{id}\""))?.options,
                None => env.settings.default_options(),
            };
            if let Some(a) = audio {
                opts.mode = DownloadMode::AudioOnly;
                opts.audio.format = match a {
                    AudioFmt::Original => AudioFormat::Original,
                    AudioFmt::Mp3 => AudioFormat::Mp3,
                    AudioFmt::M4a => AudioFormat::M4a,
                    AudioFmt::Flac => AudioFormat::Flac,
                    AudioFmt::Wav => AudioFormat::Wav,
                    AudioFmt::Opus => AudioFormat::Opus,
                    AudioFmt::Ogg => AudioFormat::Ogg,
                };
            }
            if let Some(b) = bitrate {
                opts.audio.quality = AudioQuality::Kbps(b);
            }
            if let Some(v) = video {
                opts.mode = DownloadMode::VideoAndAudio;
                opts.video.container = match v {
                    VideoFmt::Mp4 => Container::Mp4,
                    VideoFmt::Mkv => Container::Mkv,
                    VideoFmt::Webm => Container::Webm,
                    VideoFmt::Mov => Container::Mov,
                    VideoFmt::Original => Container::Original,
                };
            }
            if max_height.is_some() {
                opts.video.max_height = max_height;
            }
            if let Some(id) = format_id {
                opts.exact_format = Some(ExactFormat { format_id: id, video_only: false });
            }
            opts.playlist.items = items;

            let tools = env.tools().await;
            let yt = require_yt_dlp(&tools)?;
            if opts.needs_ffmpeg() && tools.ffmpeg.is_none() {
                eprintln!(
                    "warning: FFmpeg was not found; merging/conversion will fail. Install it with `waydlp-cli install-tool ffmpeg` (Windows) or your package manager."
                );
            }
            let ctx = env.ctx(&tools, output);
            let history = if no_history || !env.settings.privacy.save_history {
                None
            } else {
                Some(History::open(&env.paths.history_db())?)
            };

            let state: Arc<Mutex<HashMap<JobId, JobSnapshot>>> = Default::default();
            let notify = Arc::new(tokio::sync::Notify::new());
            let (s2, n2) = (state.clone(), notify.clone());
            let sink: EventSink = Arc::new(move |ev| {
                if let QueueEvent::Updated(snap) = ev {
                    s2.lock().unwrap().insert(snap.id, snap);
                    n2.notify_one();
                }
            });
            let q = spawn_queue(
                &tokio::runtime::Handle::current(),
                env.paths.jobs_temp_dir.join(format!("cli-{}", std::process::id())),
                QueueSettings {
                    max_concurrent: jobs.unwrap_or(env.settings.downloads.max_concurrent).clamp(1, 16),
                    auto_retries: env.settings.downloads.auto_retries,
                    ..Default::default()
                },
                sink,
            );
            let mut ids = Vec::new();
            for url in urls {
                let site = site_name(&url);
                let hid = history.as_ref().and_then(|h| {
                    h.insert(&NewEntry {
                        title: url.to_string(),
                        url: url.to_string(),
                        site: site.clone(),
                        format: opts.summary(),
                        options_json: serde_json::to_string(&opts).unwrap_or_default(),
                        is_playlist: playlist,
                        thumbnail: None,
                    })
                    .ok()
                });
                let id = q.add(JobSpec {
                    request: DownloadRequest {
                        url,
                        options: opts.clone(),
                        filename_template: Some(
                            template.clone().unwrap_or_else(|| env.settings.downloads.filename_template.clone()),
                        ),
                        is_playlist: playlist,
                        custom_args: env.settings.advanced.custom_args.clone(),
                    },
                    ctx: ctx.clone(),
                    net: env.settings.network_options(),
                    yt_dlp: yt.clone(),
                    title: None,
                    thumbnail: None,
                    site,
                    history_id: hid,
                });
                ids.push((id, hid));
            }

            let cancel = CancellationToken::new();
            let c2 = cancel.clone();
            tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                c2.cancel();
            });

            let mut failed = 0;
            loop {
                tokio::select! {
                    _ = notify.notified() => {}
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                    _ = cancel.cancelled() => {
                        eprintln!("\nStopping…");
                        for (id, _) in &ids { q.cancel(*id); }
                        q.shutdown().await;
                        bail!("interrupted");
                    }
                }
                let snaps = state.lock().unwrap().clone();
                let mut line = String::new();
                let mut all_done = true;
                for (id, _) in &ids {
                    let Some(s) = snaps.get(id) else {
                        all_done = false;
                        continue;
                    };
                    if !s.state.is_finished() {
                        all_done = false;
                    }
                    if s.state == JobState::Running {
                        let pct = s.progress.as_ref().and_then(|p| p.fraction()).map(|f| format!("{:5.1}%", f * 100.0));
                        let speed = s.progress.as_ref().and_then(|p| p.speed).map(humanize::speed);
                        let eta = s.progress.as_ref().and_then(|p| p.eta).map(humanize::eta);
                        line.push_str(&format!(
                            "[{}] {} {} {} {}  ",
                            id,
                            s.stage,
                            pct.unwrap_or_default(),
                            speed.unwrap_or_default(),
                            eta.map(|e| format!("ETA {e}")).unwrap_or_default()
                        ));
                    }
                }
                if !line.is_empty() {
                    let width = 150;
                    let shown: String = line.chars().take(width).collect();
                    eprint!("\r{shown:<width$}");
                    let _ = std::io::stderr().flush();
                }
                if all_done {
                    eprintln!();
                    for (id, hid) in &ids {
                        let s = &snaps[id];
                        match s.state {
                            JobState::Completed => {
                                if s.files.is_empty() {
                                    println!("✓ {} — {}", s.display_title(), s.stage);
                                }
                                for f in &s.files {
                                    println!("✓ {}", f.display());
                                }
                            }
                            _ => {
                                failed += 1;
                                let e = s.error.clone();
                                println!(
                                    "✗ {} — {}{}",
                                    s.display_title(),
                                    e.as_ref().map(|e| e.kind.title()).unwrap_or("failed"),
                                    e.as_ref()
                                        .map(|e| format!("\n    {}\n    {}", e.summary, e.kind.hint()))
                                        .unwrap_or_default()
                                );
                            }
                        }
                        if let (Some(h), Some(hid)) = (&history, hid) {
                            let status = match s.state {
                                JobState::Completed => EntryStatus::Completed,
                                JobState::Cancelled => EntryStatus::Cancelled,
                                _ => EntryStatus::Failed,
                            };
                            if let Some(t) = &s.current_title {
                                let _ = h.update_title(*hid, t);
                            }
                            let _ = h.finish(*hid, status, &s.files, s.error.as_ref().map(|e| e.summary.as_str()));
                        }
                    }
                    break;
                }
            }
            q.shutdown().await;
            if failed > 0 {
                bail!("{failed} download(s) failed");
            }
        }
    }
    Ok(())
}

fn print_probe(r: &ProbeResult, json: bool) -> Result<()> {
    match r {
        ProbeResult::Single(m) => {
            if json {
                println!("{}", serde_json::to_string_pretty(m)?);
                return Ok(());
            }
            println!("Title:    {}", m.display_title());
            if let Some(c) = m.creator() {
                println!("Creator:  {c}");
            }
            if let Some(d) = m.duration {
                println!("Duration: {}", humanize::duration(d));
            }
            if let Some(e) = &m.extractor_key {
                println!("Source:   {e}");
            }
            if m.is_live_now() {
                println!("Live:     yes (downloads record the live stream)");
            }
            let a = m.analysis();
            let res: Vec<String> = a.resolution_choices().into_iter().map(|(l, _)| l).collect();
            if !res.is_empty() {
                println!("Video:    {}", res.join(", "));
            }
            if let Some(k) = a.best_audio_kbps {
                println!("Audio:    up to ~{} kbps ({})", k.round(), a.audio_codecs.join(", "));
            }
            let subs = m.subtitle_languages();
            if !subs.is_empty() {
                println!("Subtitles: {}", subs.join(", "));
            }
            println!("\nFormats:");
            for f in m.usable_formats() {
                println!("  {:>8}  {}", f.format_id, f.label());
            }
        }
        ProbeResult::Playlist(p) => {
            if json {
                println!("{}", serde_json::to_string_pretty(p)?);
                return Ok(());
            }
            println!("Playlist: {}", p.display_title());
            if let Some(u) = p.uploader.as_ref().or(p.channel.as_ref()) {
                println!("By:       {u}");
            }
            println!("Items:    {}", p.entries.len());
            for e in &p.entries {
                println!(
                    "  {:>4}. {}{}",
                    e.index,
                    e.display_title(),
                    e.duration.map(|d| format!(" ({})", humanize::duration(d))).unwrap_or_default()
                );
            }
        }
    }
    Ok(())
}
