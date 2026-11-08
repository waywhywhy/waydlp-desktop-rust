use crate::app::WaydlpApp;
use crate::widgets::failure_view;
use egui::{RichText, Ui};
use waydlp_theme::palette::*;

impl WaydlpApp {
    pub fn page_sites(&mut self, ui: &mut Ui) {
        waydlp_theme::heading(ui, "Supported websites");
        let version = self.tools.as_ref().and_then(|t| t.yt_dlp.as_ref()).and_then(|y| y.version.clone());
        ui.label(waydlp_theme::secondary(match &version {
            Some(v) => format!(
                "waydlp supports every site your installed yt-dlp ({v}) supports. This list comes straight from it."
            ),
            None => "waydlp supports every site your installed yt-dlp supports.".to_string(),
        }));
        ui.label(waydlp_theme::muted(
            "Being listed doesn't guarantee a page works: sites change, some need sign-in, and DRM-protected services are never supported.",
        ));
        ui.add_space(10.0);

        if self.yt_dlp_path().is_none() {
            waydlp_theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                waydlp_theme::empty_state(
                    ui,
                    "🌐",
                    "yt-dlp is not installed",
                    "Install it in Settings » yt-dlp & FFmpeg to see the list.",
                );
            });
            return;
        }
        if self.sites.is_none() && !self.sites_loading {
            self.load_sites();
        }
        if self.sites_loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Asking yt-dlp for its extractor list…");
            });
            return;
        }
        let Some(result) = self.sites.clone() else { return };
        match result {
            Err(e) => {
                waydlp_theme::card().show(ui, |ui| failure_view(ui, "sites", &e));
                if ui.button("Try again").clicked() {
                    self.sites = None;
                }
            }
            Ok(list) => {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.sites_search)
                            .hint_text("🔍 Filter, e.g. vimeo")
                            .desired_width(300.0),
                    );
                    if ui.button("Refresh").clicked() {
                        self.sites = None;
                    }
                });
                let q = self.sites_search.trim().to_ascii_lowercase();
                let filtered: Vec<&String> =
                    list.iter().filter(|s| q.is_empty() || s.to_ascii_lowercase().contains(&q)).collect();
                ui.label(waydlp_theme::muted(format!("{} of {} extractors", filtered.len(), list.len())));
                ui.add_space(6.0);
                waydlp_theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let row_h = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
                    egui::ScrollArea::vertical()
                        .max_height(520.0)
                        .id_salt("sites-list")
                        .auto_shrink([false, true])
                        .show_rows(ui, row_h, filtered.len(), |ui, range| {
                            for s in &filtered[range] {
                                let broken = s.contains("(CURRENTLY BROKEN)");
                                ui.label(RichText::new(s.as_str()).color(if broken {
                                    TEXT_MUTED
                                } else {
                                    TEXT_PRIMARY
                                }));
                            }
                        });
                });
            }
        }
    }
}
