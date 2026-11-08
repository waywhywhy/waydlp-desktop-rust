use crate::app::{WaydlpApp, truncate};
use crate::widgets::{failure_view, progress_text, state_color};
use egui::{RichText, Ui};
use waydlp_engine::JobState;
use waydlp_theme::palette::*;

impl WaydlpApp {
    pub fn page_queue(&mut self, ui: &mut Ui) {
        let accent = self.accent();
        ui.horizontal(|ui| {
            waydlp_theme::heading(ui, "Download queue");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Clear finished").clicked() {
                    self.queue.clear_finished();
                }
                if ui.button("▶ Resume all").clicked() {
                    self.queue.resume_all();
                }
                if ui.button("⏸ Pause all").clicked() {
                    self.queue.pause_all();
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label(waydlp_theme::secondary("Simultaneous downloads"));
            let mut n = self.settings.downloads.max_concurrent;
            if ui.add(egui::Slider::new(&mut n, 1..=8)).changed() {
                self.settings.downloads.max_concurrent = n;
                self.settings_changed();
            }
        });
        ui.add_space(10.0);

        if self.job_order.is_empty() {
            waydlp_theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                waydlp_theme::empty_state(
                    ui,
                    "☰",
                    "The queue is empty",
                    "Analyze a link or use Quick download on the Home page.",
                );
            });
            return;
        }

        let order = self.job_order.clone();
        let n = order.len();
        for (pos, id) in order.into_iter().enumerate() {
            let Some(j) = self.jobs.get(&id).cloned() else { continue };
            waydlp_theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if self.settings.privacy.load_thumbnails
                        && let Some(t) = &j.spec.thumbnail
                    {
                        ui.add(egui::Image::new(t.as_str()).fit_to_exact_size(egui::vec2(96.0, 54.0)).corner_radius(6));
                    }
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            waydlp_theme::pill(ui, j.state.label(), state_color(j.state));
                            ui.label(RichText::new(truncate(&j.display_title(), 80)).color(TEXT_PRIMARY).strong());
                        });
                        let mut meta = format!("{} · {}", j.spec.site, j.spec.request.options.summary());
                        if j.spec.request.is_playlist {
                            meta.push_str(" · playlist");
                            if let Some((Some(i), Some(c))) = j.item {
                                meta.push_str(&format!(" · item {i} of {c}"));
                            }
                        }
                        if j.attempts > 1 {
                            meta.push_str(&format!(" · attempt {}", j.attempts));
                        }
                        ui.label(waydlp_theme::muted(meta));
                    });
                });
                ui.add_space(4.0);

                match j.state {
                    JobState::Running => {
                        let frac = j.progress.as_ref().and_then(|p| p.fraction());
                        let text = match &j.progress {
                            Some(p) => {
                                let t = progress_text(p);
                                if t.is_empty() { j.stage.clone() } else { format!("{} · {t}", j.stage) }
                            }
                            None => j.stage.clone(),
                        };
                        waydlp_theme::progress_bar(ui, accent, frac, Some(text), self.animate());
                    }
                    JobState::Paused => {
                        let frac = j.progress.as_ref().and_then(|p| p.fraction()).unwrap_or(0.0);
                        waydlp_theme::progress_bar(ui, accent, Some(frac), Some(j.stage.clone()), false);
                    }
                    JobState::Queued | JobState::WaitingRetry => {
                        ui.label(waydlp_theme::secondary(&j.stage));
                    }
                    JobState::Completed => {
                        ui.label(RichText::new(&j.stage).color(SUCCESS));
                        for f in j.files.iter().take(3) {
                            ui.label(waydlp_theme::muted(f.display().to_string()));
                        }
                        if j.files.len() > 3 {
                            ui.label(waydlp_theme::muted(format!("…and {} more", j.files.len() - 3)));
                        }
                    }
                    JobState::Failed | JobState::Cancelled => {
                        if let Some(e) = &j.error {
                            if j.state == JobState::Failed {
                                failure_view(ui, id, e);
                            } else {
                                ui.label(waydlp_theme::muted(&j.stage));
                            }
                        }
                    }
                }
                if j.state == JobState::WaitingRetry
                    && let Some(e) = &j.error
                {
                    failure_view(ui, id, e);
                }
                if !j.warnings.is_empty() {
                    egui::CollapsingHeader::new(
                        RichText::new(format!("{} warning(s)", j.warnings.len())).small().color(WARNING),
                    )
                    .id_salt(("warn", id))
                    .show(ui, |ui| {
                        for w in &j.warnings {
                            ui.label(RichText::new(w).small().color(TEXT_MUTED));
                        }
                    });
                }

                ui.horizontal(|ui| {
                    match j.state {
                        JobState::Running | JobState::Queued | JobState::WaitingRetry => {
                            if ui
                                .button("⏸ Pause")
                                .on_hover_text("Stops the download and keeps partial data so it can resume")
                                .clicked()
                            {
                                self.queue.pause(id);
                            }
                            if ui
                                .button("✖ Cancel")
                                .on_hover_text("Stops the download and deletes partial data")
                                .clicked()
                            {
                                self.queue.cancel(id);
                            }
                        }
                        JobState::Paused => {
                            if waydlp_theme::primary_button(ui, accent, "▶ Resume").clicked() {
                                self.queue.resume(id);
                            }
                            if ui.button("✖ Cancel").clicked() {
                                self.queue.cancel(id);
                            }
                        }
                        JobState::Failed | JobState::Cancelled => {
                            if waydlp_theme::primary_button(ui, accent, "↻ Retry").clicked() {
                                self.queue.retry(id);
                            }
                        }
                        JobState::Completed => {
                            if let Some(f) = j.files.first() {
                                if waydlp_theme::primary_button(ui, accent, "▶ Open").clicked()
                                    && let Err(e) = waydlp_platform::open_path(f)
                                {
                                    self.toast(e, ERROR);
                                }
                                if ui.button("📂 Show in folder").clicked()
                                    && let Err(e) = waydlp_platform::reveal_path(f)
                                {
                                    self.toast(e, ERROR);
                                }
                            } else if ui.button("📂 Open folder").clicked() {
                                let _ = waydlp_platform::open_path(&j.spec.ctx.output_dir);
                            }
                        }
                    }
                    if ui.button("Copy link").clicked() {
                        ui.ctx().copy_text(j.spec.request.url.to_string());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("🗑").on_hover_text("Remove from queue (files on disk are kept)").clicked()
                        {
                            self.queue.remove(id);
                        }
                        if pos + 1 < n && ui.small_button("⏷").on_hover_text("Move down").clicked() {
                            self.queue.move_by(id, 1);
                        }
                        if pos > 0 && ui.small_button("⏶").on_hover_text("Move up").clicked() {
                            self.queue.move_by(id, -1);
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }
    }
}
