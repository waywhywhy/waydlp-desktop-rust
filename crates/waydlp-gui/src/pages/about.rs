use crate::app::WaydlpApp;
use egui::{RichText, Ui};
use waydlp_platform::InstallKind;
use waydlp_theme::palette::*;

const REPO: &str = "https://github.com/waywhywhy/waydlp-desktop-rust";

impl WaydlpApp {
    pub fn page_about(&mut self, ui: &mut Ui) {
        let accent = self.accent();
        ui.horizontal(|ui| {
            waydlp_theme::logo(ui, 72.0, accent);
            ui.vertical(|ui| {
                waydlp_theme::title(ui, "waydlp");
                ui.label(waydlp_theme::secondary("Your media, your format, your way."));
            });
        });
        ui.add_space(14.0);

        waydlp_theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("about-grid").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
                ui.label(waydlp_theme::secondary("Version"));
                ui.label(RichText::new(waydlp_core::VERSION).strong());
                ui.end_row();
                ui.label(waydlp_theme::secondary("Installation"));
                ui.label(format!(
                    "{}{}",
                    self.install_kind.label(),
                    if self.paths.portable { " (portable data)" } else { "" }
                ));
                ui.end_row();
                ui.label(waydlp_theme::secondary("yt-dlp"));
                ui.label(self.tool_line(|t| &t.yt_dlp));
                ui.end_row();
                ui.label(waydlp_theme::secondary("FFmpeg"));
                ui.label(self.tool_line(|t| &t.ffmpeg));
                ui.end_row();
                ui.label(waydlp_theme::secondary("FFprobe"));
                ui.label(self.tool_line(|t| &t.ffprobe));
                ui.end_row();
                ui.label(waydlp_theme::secondary("Platform"));
                ui.label(format!("{} {}", std::env::consts::OS, std::env::consts::ARCH));
                ui.end_row();
                ui.label(waydlp_theme::secondary("License"));
                ui.label("GPL-3.0-or-later");
                ui.end_row();
            });
        });
        ui.add_space(12.0);

        waydlp_theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            waydlp_theme::subheading(ui, "Updates");
            ui.label(waydlp_theme::muted("waydlp only contacts GitHub when you press this button (or if you enabled automatic checks in Settings » Privacy)."));
            ui.horizontal(|ui| {
                if waydlp_theme::primary_button_enabled(ui, accent, "Check for updates", !self.update_checking).clicked() {
                    self.check_for_updates();
                }
                if self.update_checking {
                    ui.spinner();
                }
            });
            match &self.update {
                None => {}
                Some(Err(e)) => {
                    ui.label(RichText::new(e).color(TEXT_SECONDARY));
                }
                Some(Ok(r)) if r.is_newer => {
                    ui.label(RichText::new(format!("waydlp {} is available.", r.version)).color(SUCCESS).strong());
                    ui.label(waydlp_theme::secondary(self.install_kind.update_advice()));
                    if !r.notes.trim().is_empty() {
                        egui::CollapsingHeader::new("What's new").show(ui, |ui| {
                            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                ui.label(RichText::new(r.notes.trim()).small().color(TEXT_SECONDARY));
                            });
                        });
                    }
                    if !matches!(self.install_kind, InstallKind::Flatpak | InstallKind::Deb | InstallKind::Rpm | InstallKind::Arch)
                        && ui.button("Open release page").clicked()
                    {
                        let _ = waydlp_platform::open_url(&r.url);
                    }
                }
                Some(Ok(r)) => {
                    ui.label(RichText::new(format!("You're up to date (latest release: {}).", r.version)).color(TEXT_SECONDARY));
                }
            }
        });
        ui.add_space(12.0);

        waydlp_theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            waydlp_theme::subheading(ui, "Credits");
            ui.label(waydlp_theme::secondary(
                "waydlp is free and open-source software. No ads, no accounts, no tracking.",
            ));
            ui.add_space(4.0);
            for (name, what, url) in [
                ("yt-dlp", "media extraction (Unlicense)", "https://github.com/yt-dlp/yt-dlp"),
                ("FFmpeg", "merging, conversion and embedding (LGPL/GPL)", "https://ffmpeg.org"),
                ("egui / eframe", "native user interface (MIT/Apache-2.0)", "https://github.com/emilk/egui"),
                ("Tokio, SQLite, rustls and many more Rust crates", "see the repository for the full list", REPO),
            ] {
                ui.horizontal(|ui| {
                    if ui.link(name).clicked() {
                        let _ = waydlp_platform::open_url(url);
                    }
                    ui.label(waydlp_theme::muted(format!("— {what}")));
                });
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Source code on GitHub").clicked() {
                    let _ = waydlp_platform::open_url(REPO);
                }
                if ui.button("Report a problem").clicked() {
                    let _ = waydlp_platform::open_url(&format!("{REPO}/issues/new/choose"));
                }
            });
            ui.add_space(6.0);
            ui.label(waydlp_theme::muted(
                "Only download media you have the right to access. waydlp does not circumvent DRM or access controls.",
            ));
        });
    }

    fn tool_line(&self, f: impl Fn(&waydlp_engine::ToolStatus) -> &Option<waydlp_engine::ToolInfo>) -> RichText {
        match self.tools.as_ref() {
            None => RichText::new("checking…").color(TEXT_MUTED),
            Some(t) => match f(t) {
                Some(i) => RichText::new(format!(
                    "{} ({})",
                    i.version.as_deref().unwrap_or("unknown version"),
                    i.source.label()
                )),
                None => RichText::new("not found").color(WARNING),
            },
        }
    }
}
