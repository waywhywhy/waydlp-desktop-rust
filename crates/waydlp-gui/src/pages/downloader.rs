use crate::app::{Analysis, WaydlpApp, truncate};
use crate::widgets::{failure_view, section_label};
use egui::{RichText, Ui};
use waydlp_core::humanize;
use waydlp_core::media::{FormatAnalysis, MediaInfo};
use waydlp_core::options::*;
use waydlp_core::presets::{Preset, slugify};
use waydlp_theme::palette::*;

const STANDARD_HEIGHTS: [u32; 10] = [4320, 2160, 1440, 1080, 720, 480, 360, 240, 144, 0];

impl WaydlpApp {
    pub fn page_downloader(&mut self, ui: &mut Ui) {
        waydlp_theme::heading(ui, "Downloader");
        ui.add_space(8.0);
        waydlp_theme::card().show(ui, |ui| self.url_bar(ui, false));
        ui.add_space(14.0);

        enum View {
            Idle,
            Running(String, f32),
            Failed,
            Single,
            Playlist,
        }
        let view = match &self.analysis {
            Analysis::Idle => View::Idle,
            Analysis::Running { url, started, .. } => View::Running(url.to_string(), started.elapsed().as_secs_f32()),
            Analysis::Failed { .. } => View::Failed,
            Analysis::Single { .. } => View::Single,
            Analysis::Playlist { .. } => View::Playlist,
        };
        match view {
            View::Idle => {
                waydlp_theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    waydlp_theme::empty_state(
                        ui,
                        "🔍",
                        "Analyze a link",
                        "waydlp fetches the real title, thumbnail and available qualities before you download.",
                    );
                });
            }
            View::Running(url, secs) => {
                let accent = self.accent();
                waydlp_theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(format!("Analyzing {}", truncate(&url, 80))).color(TEXT_PRIMARY));
                    });
                    waydlp_theme::progress_bar(
                        ui,
                        accent,
                        None,
                        Some(format!("Asking yt-dlp for metadata… {secs:.0}s")),
                        self.animate(),
                    );
                    if secs > 20.0 {
                        ui.label(waydlp_theme::muted("Large playlists and slow sites can take a while."));
                    }
                    if ui.button("Cancel").clicked() {
                        self.cancel_analysis();
                    }
                });
            }
            View::Failed => {
                let Analysis::Failed { url, error } = &self.analysis else { return };
                let (url, error) = (url.clone(), error.clone());
                waydlp_theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(truncate(url.as_str(), 90)).color(TEXT_MUTED));
                    failure_view(ui, "analysis", &error);
                    ui.horizontal(|ui| {
                        if ui.button("Try again").clicked() {
                            self.analyze(url.as_str(), false);
                        }
                        if error.kind == waydlp_core::ErrorKind::YtDlpMissing
                            && ui.button("Open tool settings").clicked()
                        {
                            self.page = waydlp_config::Page::Settings;
                        }
                    });
                });
            }
            View::Single => self.single_view(ui),
            View::Playlist => self.playlist_view(ui),
        }
    }

    fn single_view(&mut self, ui: &mut Ui) {
        let Analysis::Single { url, info } = &self.analysis else { return };
        let (url, info) = (url.clone(), info.clone());
        let accent = self.accent();
        let analysis = info.analysis();

        waydlp_theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                if self.settings.privacy.load_thumbnails
                    && let Some(t) = info.best_thumbnail()
                {
                    ui.add(egui::Image::new(t).max_width(300.0).max_height(170.0).corner_radius(10).show_loading_spinner(true));
                    ui.add_space(8.0);
                }
                ui.vertical(|ui| {
                    waydlp_theme::subheading(ui, info.display_title());
                    if let Some(c) = info.creator() {
                        ui.label(waydlp_theme::secondary(c));
                    }
                    ui.horizontal_wrapped(|ui| {
                        waydlp_theme::pill(ui, &waydlp_core::site::site_name(&url), ACCENT_HOVER);
                        if let Some(d) = info.duration {
                            waydlp_theme::pill(ui, &humanize::duration(d), TEXT_SECONDARY);
                        }
                        if info.is_live_now() {
                            waydlp_theme::pill(ui, "LIVE", ERROR);
                        }
                        if let Some(h) = analysis.heights.last() {
                            waydlp_theme::pill(ui, &format!("up to {h}p"), TEXT_SECONDARY);
                        }
                        if let Some(k) = analysis.best_audio_kbps {
                            waydlp_theme::pill(ui, &format!("audio ~{} kbps", k.round()), TEXT_SECONDARY);
                        }
                    });
                    if info.is_live_now() {
                        ui.label(RichText::new("This is a live stream: the download records from now until the stream ends or you cancel.").small().color(WARNING));
                    }
                    if analysis.format_count == 0 {
                        ui.label(RichText::new("yt-dlp did not list any formats for this page.").color(WARNING));
                    }
                });
            });
        });
        ui.add_space(12.0);
        self.options_card(ui, Some((&info, &analysis)), false);
        ui.add_space(12.0);
        self.save_location_card(ui);
        ui.add_space(12.0);

        let needs_ff = self.dl_options.needs_ffmpeg();
        let has_ff = self.has_ffmpeg();
        ui.horizontal(|ui| {
            let ok = !needs_ff || has_ff;
            if waydlp_theme::primary_button_enabled(ui, accent, "⬇  Download", ok).clicked() {
                let out = self.dl_output_dir.clone();
                let tpl = Some(self.dl_template.clone());
                let thumb = info.best_thumbnail().map(str::to_string);
                if self.enqueue(
                    url.clone(),
                    self.dl_options.clone(),
                    false,
                    Some(info.display_title().to_string()),
                    thumb,
                    out,
                    tpl,
                ) {
                    self.toast("Added to the queue", SUCCESS);
                    self.page = waydlp_config::Page::Queue;
                }
            }
            ui.label(waydlp_theme::muted(self.dl_options.summary()));
        });
        if needs_ff && !has_ff {
            ui.label(RichText::new("These options need FFmpeg (merging, conversion or embedding). Install it in Settings » yt-dlp & FFmpeg, or choose “Original file”.").color(WARNING));
        }
    }

    fn playlist_view(&mut self, ui: &mut Ui) {
        let accent = self.accent();
        let Analysis::Playlist { url, info, selected } = &mut self.analysis else { return };
        let url_s = url.to_string();
        let count = info.entries.len();
        let mut n_selected = 0usize;
        waydlp_theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                waydlp_theme::pill(ui, "PLAYLIST", ACCENT_HOVER);
                waydlp_theme::subheading(ui, &info.display_title());
            });
            if let Some(u) = info.uploader.as_ref().or(info.channel.as_ref()) {
                ui.label(waydlp_theme::secondary(u));
            }
            ui.label(waydlp_theme::muted(format!("{count} item(s) found. Choose which ones to download.")));
            ui.horizontal(|ui| {
                if ui.small_button("Select all").clicked() {
                    selected.iter_mut().for_each(|s| *s = true);
                }
                if ui.small_button("Select none").clicked() {
                    selected.iter_mut().for_each(|s| *s = false);
                }
                if ui.small_button("Invert").clicked() {
                    selected.iter_mut().for_each(|s| *s = !*s);
                }
            });
            ui.add_space(4.0);
            let row_h = 26.0;
            egui::ScrollArea::vertical().max_height(320.0).id_salt("playlist-entries").show_rows(
                ui,
                row_h,
                count,
                |ui, range| {
                    for i in range {
                        let e = &info.entries[i];
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut selected[i], "");
                            ui.label(RichText::new(format!("{:>4}.", e.index)).monospace().color(TEXT_MUTED));
                            ui.label(RichText::new(truncate(&e.display_title(), 90)).color(TEXT_PRIMARY));
                            if let Some(d) = e.duration {
                                ui.label(waydlp_theme::muted(humanize::duration(d)));
                            }
                        });
                    }
                },
            );
            n_selected = selected.iter().filter(|s| **s).count();
        });
        ui.add_space(12.0);
        self.options_card(ui, None, true);
        ui.add_space(12.0);
        self.save_location_card(ui);
        ui.add_space(12.0);
        let needs_ff = self.dl_options.needs_ffmpeg();
        let has_ff = self.has_ffmpeg();
        ui.horizontal(|ui| {
            if waydlp_theme::primary_button_enabled(
                ui,
                accent,
                &format!("⬇  Download {n_selected} item(s)"),
                n_selected > 0 && (!needs_ff || has_ff),
            )
            .clicked()
            {
                self.enqueue_playlist(false);
            }
            if ui.button("Analyze only the linked video").on_hover_text("Ignore the playlist").clicked() {
                self.analyze(&url_s, true);
            }
        });
        if needs_ff && !has_ff {
            ui.label(
                RichText::new(
                    "These options need FFmpeg. Install it in Settings » yt-dlp & FFmpeg, or choose “Original file”.",
                )
                .color(WARNING),
            );
        }
    }

    /// Format/quality options. `media` is present for analysed single items:
    /// then only real resolutions and formats are offered.
    fn options_card(&mut self, ui: &mut Ui, media: Option<(&MediaInfo, &FormatAnalysis)>, playlist: bool) {
        waydlp_theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                section_label(ui, "Preset");
                let current = self.settings.preset(&self.dl_preset).map(|p| p.name).unwrap_or_else(|| "Custom".into());
                egui::ComboBox::from_id_salt("dl-preset").selected_text(current).width(220.0).show_ui(ui, |ui| {
                    for p in self.settings.all_presets() {
                        if ui.selectable_label(self.dl_preset == p.id, format!("{} — {}", p.name, p.options.summary())).clicked() {
                            self.dl_preset = p.id.clone();
                            let keep_pl = self.dl_options.playlist.clone();
                            self.dl_options = p.options.clone();
                            self.dl_options.playlist.subfolder = keep_pl.subfolder;
                            self.dl_options.playlist.number_files = keep_pl.number_files;
                            if let Some((info, _)) = media {
                                let a = info.analysis();
                                if let Some(h) = self.dl_options.video.max_height
                                    && !a.heights.is_empty()
                                    && !a.heights.iter().any(|x| *x <= h)
                                {
                                    self.dl_options.video.max_height = None;
                                }
                            }
                        }
                    }
                });
            });
            ui.add_space(6.0);

            // Mode.
            ui.horizontal(|ui| {
                for m in DownloadMode::ALL {
                    let disabled = media.is_some_and(|(_, a)| match m {
                        DownloadMode::VideoAndAudio | DownloadMode::VideoOnly => !a.has_video,
                        DownloadMode::AudioOnly => !a.has_audio,
                        DownloadMode::Original => false,
                    });
                    let r = ui.add_enabled(!disabled, egui::Button::selectable(self.dl_options.mode == m, m.label()));
                    if r.clicked() {
                        self.dl_options.mode = m;
                        self.dl_options.exact_format = None;
                        self.dl_preset = "custom".into();
                    }
                    if disabled {
                        r.on_disabled_hover_text("Not available for this media");
                    }
                }
            });
            ui.add_space(8.0);
            let before = self.dl_options.clone();

            match self.dl_options.mode {
                DownloadMode::VideoAndAudio | DownloadMode::VideoOnly => self.video_options(ui, media.map(|m| m.1), playlist),
                DownloadMode::AudioOnly => self.audio_options(ui, media.map(|m| m.1)),
                DownloadMode::Original => {
                    ui.label(waydlp_theme::secondary("Downloads the best single file exactly as the site serves it. No merging, conversion or embedding — FFmpeg is not needed."));
                }
            }

            if self.dl_options.mode != DownloadMode::Original {
                ui.add_space(6.0);
                egui::CollapsingHeader::new("Metadata, thumbnails & subtitles").id_salt("embed").show(ui, |ui| {
                    ui.checkbox(&mut self.dl_options.embed_metadata, "Embed metadata (title, artist, date…)");
                    ui.checkbox(&mut self.dl_options.embed_thumbnail, "Embed thumbnail / cover art")
                        .on_hover_text("Not supported for WAV or WebM; skipped automatically there.");
                    if self.dl_options.mode != DownloadMode::AudioOnly {
                        ui.checkbox(&mut self.dl_options.embed_chapters, "Embed chapters");
                        self.subtitle_options(ui, media.map(|m| m.0));
                    }
                });
            }

            if let Some((info, _)) = media {
                egui::CollapsingHeader::new("Advanced: pick an exact format").id_salt("exact").show(ui, |ui| {
                    ui.label(waydlp_theme::muted("Overrides the rules above. Listed exactly as reported by yt-dlp."));
                    let mut clear = false;
                    if self.dl_options.exact_format.is_some() && ui.button("Use automatic selection").clicked() {
                        clear = true;
                    }
                    egui::ScrollArea::vertical().max_height(220.0).id_salt("formats").show(ui, |ui| {
                        for f in info.usable_formats() {
                            let sel = self.dl_options.exact_format.as_ref().is_some_and(|e| e.format_id == f.format_id);
                            let txt = format!("{:>6}  {}", f.format_id, f.label());
                            if ui.selectable_label(sel, RichText::new(txt).monospace()).clicked() {
                                self.dl_options.exact_format = Some(ExactFormat { format_id: f.format_id.clone(), video_only: f.is_video_only() });
                                if f.is_audio_only() && self.dl_options.mode != DownloadMode::Original {
                                    self.dl_options.mode = DownloadMode::AudioOnly;
                                }
                            }
                        }
                    });
                    if clear {
                        self.dl_options.exact_format = None;
                    }
                });
            }

            if self.dl_options != before {
                self.dl_preset = "custom".into();
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.new_preset_name).hint_text("Preset name").desired_width(180.0));
                if ui.add_enabled(!self.new_preset_name.trim().is_empty(), egui::Button::new("Save as preset")).clicked() {
                    let name = self.new_preset_name.trim().to_string();
                    let mut id = slugify(&name);
                    let existing: Vec<String> = self.settings.all_presets().into_iter().map(|p| p.id).collect();
                    let mut n = 2;
                    while existing.contains(&id) {
                        id = format!("{}-{n}", slugify(&name));
                        n += 1;
                    }
                    let mut options = self.dl_options.clone();
                    options.exact_format = None;
                    options.playlist.items = None;
                    self.settings.presets.push(Preset { id: id.clone(), name: name.clone(), builtin: false, options });
                    self.dl_preset = id;
                    self.new_preset_name.clear();
                    self.settings_changed();
                    self.toast(format!("Saved preset “{name}”"), SUCCESS);
                }
            });
        });
    }

    fn video_options(&mut self, ui: &mut Ui, a: Option<&FormatAnalysis>, playlist: bool) {
        let o = &mut self.dl_options;
        egui::Grid::new("video-opts").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
            ui.label("Quality");
            let label = |h: Option<u32>| match h {
                None => "Best available".to_string(),
                Some(h) => waydlp_core::media::resolution_label(h),
            };
            egui::ComboBox::from_id_salt("quality").selected_text(label(o.video.max_height)).width(220.0).show_ui(
                ui,
                |ui| {
                    ui.selectable_value(&mut o.video.max_height, None, "Best available");
                    match a.filter(|a| !a.heights.is_empty()) {
                        // Only real resolutions for analysed media.
                        Some(a) => {
                            for (l, h) in a.resolution_choices() {
                                ui.selectable_value(&mut o.video.max_height, Some(h), l);
                            }
                        }
                        None => {
                            for h in STANDARD_HEIGHTS.into_iter().filter(|h| *h > 0) {
                                ui.selectable_value(
                                    &mut o.video.max_height,
                                    Some(h),
                                    format!("Up to {}", waydlp_core::media::resolution_label(h)),
                                );
                            }
                        }
                    }
                },
            );
            ui.end_row();
            if a.is_none() && !playlist {
                ui.label("");
                ui.label(waydlp_theme::muted("Without analysis, the best quality at or below this limit is chosen."));
                ui.end_row();
            }

            ui.label("Container");
            egui::ComboBox::from_id_salt("container").selected_text(o.video.container.label()).show_ui(ui, |ui| {
                for c in Container::ALL {
                    ui.selectable_value(&mut o.video.container, c, c.label());
                }
            });
            ui.end_row();

            ui.label("Codec preference");
            egui::ComboBox::from_id_salt("codec").selected_text(o.video.codec.label()).width(220.0).show_ui(ui, |ui| {
                for c in CodecPreference::ALL {
                    let available = a.is_none_or(|a| {
                        c == CodecPreference::Any
                            || a.video_codecs.contains(&match c {
                                CodecPreference::H264 => "h264",
                                CodecPreference::H265 => "h265",
                                CodecPreference::Vp9 => "vp9",
                                CodecPreference::Av1 => "av1",
                                CodecPreference::Any => "",
                            })
                    });
                    let text = if available {
                        c.label().to_string()
                    } else {
                        format!("{} — not offered by this site", c.label())
                    };
                    ui.add_enabled_ui(available, |ui| ui.selectable_value(&mut o.video.codec, c, text));
                }
            });
            ui.end_row();

            if a.is_none_or(|a| a.max_fps.is_some_and(|f| f > 31.0)) {
                ui.label("Frame rate");
                egui::ComboBox::from_id_salt("fps").selected_text(o.video.fps.label()).show_ui(ui, |ui| {
                    for f in FpsPreference::ALL {
                        ui.selectable_value(&mut o.video.fps, f, f.label());
                    }
                });
                ui.end_row();
            }
        });
        ui.checkbox(&mut o.video.allow_recode, "Re-encode if the streams don't fit the container")
            .on_hover_text("Off: waydlp remuxes (no quality loss) and prefers streams that fit. On: FFmpeg re-encodes when remuxing is impossible, which is slower and can lose quality.");
        if playlist {
            ui.checkbox(&mut o.playlist.subfolder, "Save into a folder named after the playlist");
            ui.checkbox(&mut o.playlist.number_files, "Number files by playlist position");
        }
    }

    fn audio_options(&mut self, ui: &mut Ui, a: Option<&FormatAnalysis>) {
        let o = &mut self.dl_options;
        egui::Grid::new("audio-opts").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
            ui.label("Format");
            egui::ComboBox::from_id_salt("audio-format").selected_text(o.audio.format.label()).width(240.0).show_ui(
                ui,
                |ui| {
                    for f in AudioFormat::ALL {
                        ui.selectable_value(&mut o.audio.format, f, f.label());
                    }
                },
            );
            ui.end_row();
            if !o.audio.format.is_lossless() && o.audio.format != AudioFormat::Original {
                ui.label("Encoding quality");
                egui::ComboBox::from_id_salt("audio-quality")
                    .selected_text(o.audio.quality.label())
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for q in AudioQuality::CHOICES {
                            ui.selectable_value(&mut o.audio.quality, q, q.label());
                        }
                    });
                ui.end_row();
            }
        });
        let source = a
            .and_then(|a| a.best_audio_kbps)
            .map(|k| format!(" The best source audio here is about {} kbps.", k.round()))
            .unwrap_or_default();
        if o.audio.format == AudioFormat::Original {
            ui.label(waydlp_theme::muted(format!("Keeps the original audio stream without re-encoding.{source}")));
        } else {
            ui.label(waydlp_theme::muted(format!(
                "Bitrates are encoder targets: converting cannot restore quality that isn't in the source.{source}"
            )));
        }
        let can_norm = o.audio.format != AudioFormat::Original;
        ui.add_enabled_ui(can_norm, |ui| {
            ui.checkbox(&mut o.audio.normalize, "Normalize loudness (EBU R128)")
                .on_hover_text("Makes tracks play at a similar volume. Requires re-encoding.")
                .on_disabled_hover_text("Choose an output format other than Original to normalize.");
        });
        if !can_norm {
            o.audio.normalize = false;
        }
    }

    fn subtitle_options(&mut self, ui: &mut Ui, info: Option<&MediaInfo>) {
        let o = &mut self.dl_options;
        let (subs, autos) = info.map(|i| (i.subtitle_languages(), i.auto_caption_languages())).unwrap_or_default();
        if info.is_some() && subs.is_empty() && autos.is_empty() {
            ui.label(waydlp_theme::muted("No subtitles are available for this media."));
            o.subtitles.download = false;
            return;
        }
        ui.checkbox(&mut o.subtitles.download, "Download subtitles");
        if !o.subtitles.download {
            return;
        }
        ui.indent("subs", |ui| {
            if info.is_some() {
                ui.label(waydlp_theme::muted("Languages:"));
                ui.horizontal_wrapped(|ui| {
                    let mut all: Vec<(String, bool)> = subs.iter().map(|s| (s.clone(), false)).collect();
                    if o.subtitles.include_auto_generated {
                        all.extend(autos.iter().filter(|a| !subs.contains(a)).map(|a| (a.clone(), true)));
                    }
                    for (lang, auto) in all.into_iter().take(60) {
                        let mut on = o.subtitles.languages.contains(&lang);
                        let label = if auto { format!("{lang} (auto)") } else { lang.clone() };
                        if ui.toggle_value(&mut on, label).changed() {
                            if on {
                                o.subtitles.languages.push(lang);
                            } else {
                                o.subtitles.languages.retain(|l| *l != lang);
                            }
                        }
                    }
                });
            } else {
                let mut langs = o.subtitles.languages.join(",");
                ui.horizontal(|ui| {
                    ui.label("Languages");
                    if ui
                        .add(egui::TextEdit::singleline(&mut langs).hint_text("en,de or all").desired_width(160.0))
                        .changed()
                    {
                        o.subtitles.languages =
                            langs.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                    }
                });
            }
            if !autos.is_empty() || info.is_none() {
                ui.checkbox(&mut o.subtitles.include_auto_generated, "Include automatically generated captions");
            }
            ui.checkbox(&mut o.subtitles.embed, "Embed into the video file (MP4/MKV/WebM/MOV)");
            ui.checkbox(&mut o.subtitles.convert_to_srt, "Convert to SRT");
        });
    }

    fn save_location_card(&mut self, ui: &mut Ui) {
        waydlp_theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("save-loc").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                ui.label("Save to");
                ui.horizontal(|ui| {
                    let dir = self.dl_output_dir.clone().unwrap_or_else(|| self.download_dir());
                    ui.label(RichText::new(truncate(&dir.display().to_string(), 70)).color(TEXT_SECONDARY));
                    if ui.button("Change…").clicked()
                        && let Some(d) = rfd::FileDialog::new().set_directory(&dir).pick_folder()
                    {
                        self.dl_output_dir = Some(d);
                    }
                    if self.dl_output_dir.is_some() && ui.small_button("Reset").clicked() {
                        self.dl_output_dir = None;
                    }
                });
                ui.end_row();
                ui.label("File name");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.dl_template)
                            .desired_width(360.0)
                            .font(egui::TextStyle::Monospace),
                    )
                    .on_hover_text("yt-dlp output template, e.g. %(uploader)s/%(title)s.%(ext)s");
                    if self.dl_template != self.settings.downloads.filename_template
                        && ui.small_button("Default").clicked()
                    {
                        self.dl_template = self.settings.downloads.filename_template.clone();
                    }
                });
                ui.end_row();
            });
            if let Err(e) = waydlp_core::sanitize::validate_output_template(&self.dl_template) {
                ui.label(RichText::new(e.to_string()).color(ERROR));
            }
        });
    }
}
