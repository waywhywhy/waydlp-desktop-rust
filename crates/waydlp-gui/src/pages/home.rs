use crate::app::{WaydlpApp, truncate};
use crate::widgets::{progress_text, state_color};
use egui::{RichText, Ui};
use waydlp_config::Page;
use waydlp_core::humanize;
use waydlp_engine::JobState;
use waydlp_history::HistoryFilter;
use waydlp_theme::palette::*;

impl WaydlpApp {
    pub fn page_home(&mut self, ui: &mut Ui) {
        let accent = self.accent();
        waydlp_theme::title(ui, "Download anything you're allowed to.");
        ui.label(waydlp_theme::secondary(
            "Paste a link from YouTube, SoundCloud, Vimeo and thousands of other sites supported by yt-dlp.",
        ));
        ui.add_space(16.0);

        // URL bar.
        waydlp_theme::accent_card(accent).show(ui, |ui| {
            ui.set_width(ui.available_width());
            self.url_bar(ui, true);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(waydlp_theme::muted("Quick download uses your default preset:"));
                let name =
                    self.settings.preset(&self.settings.downloads.default_preset).map(|p| p.name).unwrap_or_default();
                egui::ComboBox::from_id_salt("home-default-preset").selected_text(name).show_ui(ui, |ui| {
                    let mut changed = false;
                    for p in self.settings.all_presets() {
                        changed |= ui
                            .selectable_value(
                                &mut self.settings.downloads.default_preset,
                                p.id.clone(),
                                format!("{} — {}", p.name, p.options.summary()),
                            )
                            .changed();
                    }
                    if changed {
                        self.settings_changed();
                    }
                });
                if ui.link("Paste several links…").clicked() {
                    self.batch_open = !self.batch_open;
                }
            });
            if self.batch_open {
                ui.add_space(8.0);
                self.batch_ui(ui);
            }
        });
        ui.add_space(16.0);

        // Active downloads + stats.
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            waydlp_theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(210.0);
                ui.horizontal(|ui| {
                    waydlp_theme::subheading(ui, "Active downloads");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.link("Open queue »").clicked() {
                            self.page = Page::Queue;
                        }
                    });
                });
                let active: Vec<_> = self
                    .job_order
                    .iter()
                    .filter_map(|id| self.jobs.get(id))
                    .filter(|j| j.state.is_active() || j.state == JobState::Paused)
                    .take(4)
                    .cloned()
                    .collect();
                if active.is_empty() {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| ui.label(waydlp_theme::muted("Nothing downloading right now.")));
                }
                for j in active {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        waydlp_theme::pill(ui, j.state.label(), state_color(j.state));
                        ui.label(RichText::new(truncate(&j.display_title(), 48)).color(TEXT_PRIMARY));
                    });
                    let frac = j.progress.as_ref().and_then(|p| p.fraction());
                    let text = match &j.progress {
                        Some(p) if j.state == JobState::Running => format!("{} · {}", j.stage, progress_text(p)),
                        _ => j.stage.clone(),
                    };
                    let indeterminate = j.state == JobState::Running && frac.is_none();
                    waydlp_theme::progress_bar(
                        ui,
                        accent,
                        if indeterminate { None } else { Some(frac.unwrap_or(0.0)) },
                        Some(text),
                        self.animate(),
                    );
                }
            });

            let ui = &mut cols[1];
            waydlp_theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(210.0);
                waydlp_theme::subheading(ui, "At a glance");
                ui.add_space(6.0);
                let stats = self.history.as_ref().and_then(|h| h.stats().ok()).unwrap_or_default();
                egui::Grid::new("stats").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
                    ui.label(waydlp_theme::secondary("Completed downloads"));
                    ui.label(RichText::new(stats.completed.to_string()).strong());
                    ui.end_row();
                    ui.label(waydlp_theme::secondary("Failed downloads"));
                    ui.label(RichText::new(stats.failed.to_string()).strong());
                    ui.end_row();
                    ui.label(waydlp_theme::secondary("Downloaded data"));
                    ui.label(RichText::new(humanize::bytes(stats.total_bytes)).strong());
                    ui.end_row();
                    ui.label(waydlp_theme::secondary("yt-dlp"));
                    match self.tools.as_ref().map(|t| &t.yt_dlp) {
                        None => ui.label(waydlp_theme::muted("checking…")),
                        Some(Some(t)) => ui.label(
                            RichText::new(t.version.clone().unwrap_or_else(|| "installed".into())).color(SUCCESS),
                        ),
                        Some(None) => ui.label(RichText::new("not installed").color(ERROR)),
                    };
                    ui.end_row();
                    ui.label(waydlp_theme::secondary("FFmpeg"));
                    match self.tools.as_ref().map(|t| &t.ffmpeg) {
                        None => ui.label(waydlp_theme::muted("checking…")),
                        Some(Some(t)) => ui.label(
                            RichText::new(truncate(t.version.as_deref().unwrap_or("installed"), 24)).color(SUCCESS),
                        ),
                        Some(None) => ui.label(RichText::new("not installed").color(WARNING)),
                    };
                    ui.end_row();
                    ui.label(waydlp_theme::secondary("Save to"));
                    ui.label(RichText::new(truncate(&self.download_dir().display().to_string(), 34)).small());
                    ui.end_row();
                });
                if self.tools.as_ref().is_some_and(|t| t.yt_dlp.is_none() || t.ffmpeg.is_none())
                    && waydlp_theme::primary_button(ui, accent, "Set up missing tools").clicked()
                {
                    self.page = Page::Settings;
                }
            });
        });
        ui.add_space(16.0);

        // Recent downloads.
        waydlp_theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                waydlp_theme::subheading(ui, "Recent downloads");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.link("Full history »").clicked() {
                        self.page = Page::History;
                        self.history_dirty = true;
                    }
                });
            });
            let recent = self
                .history
                .as_ref()
                .and_then(|h| h.list(&HistoryFilter { limit: Some(5), ..Default::default() }).ok())
                .unwrap_or_default();
            if recent.is_empty() {
                waydlp_theme::empty_state(ui, "⬇", "No downloads yet", "Paste a link above to get started.");
            }
            for e in recent {
                ui.horizontal(|ui| {
                    waydlp_theme::pill(ui, e.status.label(), crate::widgets::entry_color(e.status));
                    ui.label(RichText::new(truncate(&e.title, 60)).color(TEXT_PRIMARY));
                    ui.label(waydlp_theme::muted(format!("{} · {}", e.site, e.format)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(f) = e.primary_file() {
                            if ui.small_button("📂").on_hover_text("Show in folder").clicked() {
                                let _ = waydlp_platform::reveal_path(f);
                            }
                            if ui.small_button("▶").on_hover_text("Open file").clicked() {
                                let _ = waydlp_platform::open_path(f);
                            }
                        }
                    });
                });
            }
        });
    }

    /// URL input with Paste / Analyze / Quick download.
    pub fn url_bar(&mut self, ui: &mut Ui, with_quick: bool) {
        let accent = self.accent();
        ui.horizontal(|ui| {
            let w = ui.available_width() - if with_quick { 420.0 } else { 260.0 };
            let edit = egui::TextEdit::singleline(&mut self.url_input)
                .hint_text("https://…  paste a video, song or playlist link")
                .desired_width(w.max(200.0))
                .margin(egui::Margin::symmetric(12, 10))
                .font(egui::TextStyle::Body);
            let r = ui.add(edit);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Media URL"));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.url_input.trim().is_empty() {
                let u = self.url_input.clone();
                self.analyze(&u, false);
            }
            if ui.button("📋 Paste").on_hover_text("Paste a link from the clipboard").clicked()
                && let Some(t) = self.read_clipboard()
            {
                self.url_input = t;
                self.url_error = None;
            }
            let has = !self.url_input.trim().is_empty();
            if waydlp_theme::primary_button_enabled(ui, accent, "Analyze", has).clicked() {
                let u = self.url_input.clone();
                self.analyze(&u, false);
            }
            if with_quick
                && ui
                    .add_enabled(has, egui::Button::new("⬇ Quick download"))
                    .on_hover_text("Download immediately with your default preset")
                    .clicked()
            {
                let u = self.url_input.clone();
                self.quick_download(&u);
            }
        });
        if let Some(e) = &self.url_error {
            ui.label(RichText::new(e).color(ERROR));
        }
    }

    fn batch_ui(&mut self, ui: &mut Ui) {
        let accent = self.accent();
        ui.label(waydlp_theme::secondary("One link per line. Each is downloaded with the default preset."));
        ui.add(
            egui::TextEdit::multiline(&mut self.batch_input)
                .desired_rows(5)
                .desired_width(f32::INFINITY)
                .hint_text("https://…\nhttps://…"),
        );
        let (urls, bad) = waydlp_core::urls::extract_urls(&self.batch_input);
        let playlists = urls
            .iter()
            .filter(|u| waydlp_core::urls::looks_like_playlist(u) && !u.query_pairs().any(|(k, _)| k == "v"))
            .count();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} valid link(s)", urls.len())).color(TEXT_SECONDARY));
            if !bad.is_empty() {
                ui.label(RichText::new(format!("{} ignored", bad.len())).color(WARNING)).on_hover_text(bad.join("\n"));
            }
            if playlists > 0 {
                ui.checkbox(&mut self.batch_allow_playlists, format!("Include {playlists} playlist link(s) in full"));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if waydlp_theme::primary_button_enabled(
                    ui,
                    accent,
                    &format!("Add {} to queue", urls.len()),
                    !urls.is_empty(),
                )
                .clicked()
                {
                    let opts = self.settings.default_options();
                    let mut added = 0;
                    for u in urls {
                        let is_pl =
                            waydlp_core::urls::looks_like_playlist(&u) && !u.query_pairs().any(|(k, _)| k == "v");
                        if is_pl && !self.batch_allow_playlists {
                            continue;
                        }
                        if self.enqueue(u, opts.clone(), is_pl, None, None, None, None) {
                            added += 1;
                        }
                    }
                    self.toast(format!("Added {added} download(s) to the queue"), SUCCESS);
                    self.batch_input.clear();
                    self.batch_open = false;
                }
            });
        });
    }
}
