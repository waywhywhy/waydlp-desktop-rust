use crate::app::{Confirm, WaydlpApp, truncate};
use crate::widgets::entry_color;
use egui::{RichText, Ui};
use waydlp_core::humanize;
use waydlp_history::{EntryStatus, HistoryFilter};
use waydlp_theme::palette::*;

impl WaydlpApp {
    fn reload_history(&mut self) {
        let Some(h) = &self.history else { return };
        let filter = HistoryFilter {
            search: self.history_search.clone(),
            status: self.history_status,
            site: self.history_site.clone(),
            limit: Some(2000),
        };
        match h.list(&filter) {
            Ok(rows) => self.history_rows = rows,
            Err(e) => self.toast(e.to_string(), ERROR),
        }
        self.history_dirty = false;
    }

    pub fn page_history(&mut self, ui: &mut Ui) {
        if self.history.is_none() {
            waydlp_theme::heading(ui, "History");
            ui.label(RichText::new("The history database could not be opened. See the logs for details.").color(ERROR));
            return;
        }
        if self.history_dirty {
            self.reload_history();
        }
        ui.horizontal(|ui| {
            waydlp_theme::heading(ui, "History");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Clear history…").clicked() {
                    self.confirm = Some(Confirm::ClearHistory);
                }
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.history_search)
                    .hint_text("🔍 Search title, link or site")
                    .desired_width(320.0),
            );
            if r.changed() {
                self.history_dirty = true;
            }
            let label = self.history_status.map(|s| s.label()).unwrap_or("Any status");
            egui::ComboBox::from_id_salt("hist-status").selected_text(label).show_ui(ui, |ui| {
                for (v, l) in [
                    (None, "Any status"),
                    (Some(EntryStatus::Completed), "Completed"),
                    (Some(EntryStatus::Failed), "Failed"),
                    (Some(EntryStatus::Cancelled), "Cancelled"),
                ] {
                    if ui.selectable_value(&mut self.history_status, v, l).changed() {
                        self.history_dirty = true;
                    }
                }
            });
            let sites = self.history.as_ref().and_then(|h| h.sites().ok()).unwrap_or_default();
            let label = self.history_site.clone().unwrap_or_else(|| "Any site".into());
            egui::ComboBox::from_id_salt("hist-site").selected_text(label).show_ui(ui, |ui| {
                if ui.selectable_value(&mut self.history_site, None, "Any site").changed() {
                    self.history_dirty = true;
                }
                for s in sites {
                    if ui.selectable_value(&mut self.history_site, Some(s.clone()), s).changed() {
                        self.history_dirty = true;
                    }
                }
            });
        });
        ui.add_space(10.0);

        if self.history_rows.is_empty() {
            waydlp_theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                if self.history_search.is_empty() && self.history_status.is_none() && self.history_site.is_none() {
                    waydlp_theme::empty_state(ui, "🕘", "No history yet", "Finished downloads appear here.");
                } else {
                    waydlp_theme::empty_state(ui, "🔍", "No matches", "Try a different search or filter.");
                }
            });
            return;
        }
        ui.label(waydlp_theme::muted(format!(
            "{} entr{}",
            self.history_rows.len(),
            if self.history_rows.len() == 1 { "y" } else { "ies" }
        )));

        let rows = self.history_rows.clone();
        let accent = self.accent();
        for e in rows {
            waydlp_theme::card().inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    waydlp_theme::pill(ui, e.status.label(), entry_color(e.status));
                    ui.label(RichText::new(truncate(&e.title, 80)).color(TEXT_PRIMARY).strong());
                });
                let local = e.created_at.with_timezone(&chrono::Local);
                let mut meta = format!("{} · {} · {}", e.site, e.format, local.format("%Y-%m-%d %H:%M"));
                if let Some(s) = e.total_size {
                    meta.push_str(&format!(" · {}", humanize::bytes(s)));
                }
                if e.is_playlist {
                    meta.push_str(&format!(" · playlist ({} files)", e.files.len()));
                }
                ui.label(waydlp_theme::muted(meta));
                if let Some(err) = &e.error
                    && e.status != EntryStatus::Completed
                {
                    ui.label(RichText::new(truncate(err, 160)).small().color(ERROR));
                }
                let file_exists = e.primary_file().is_some_and(|f| f.exists());
                if e.status == EntryStatus::Completed && !e.files.is_empty() && !file_exists {
                    ui.label(RichText::new("The file has been moved or deleted.").small().color(WARNING));
                }
                ui.horizontal(|ui| {
                    if let Some(f) = e.primary_file().filter(|_| file_exists) {
                        if waydlp_theme::primary_button(ui, accent, "▶ Open").clicked()
                            && let Err(err) = waydlp_platform::open_path(f)
                        {
                            self.toast(err, ERROR);
                        }
                        if ui.button("📂 Folder").clicked()
                            && let Err(err) = waydlp_platform::reveal_path(f)
                        {
                            self.toast(err, ERROR);
                        }
                    }
                    if ui.button("Copy link").clicked() {
                        ui.ctx().copy_text(e.url.clone());
                        self.toast("Link copied", SUCCESS);
                    }
                    if ui.button("↻ Download again").clicked() {
                        self.retry_history(e.id);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if e.files.iter().any(|f| f.exists())
                            && ui
                                .small_button(RichText::new("Delete files…").color(ERROR))
                                .on_hover_text("Permanently delete the downloaded files (asks first)")
                                .clicked()
                        {
                            self.confirm = Some(Confirm::DeleteFiles(e.id));
                        }
                        if ui
                            .small_button("Remove entry")
                            .on_hover_text("Removes it from history; files are kept")
                            .clicked()
                        {
                            if let Some(h) = &self.history {
                                let _ = h.remove(e.id);
                            }
                            self.history_dirty = true;
                        }
                    });
                });
            });
            ui.add_space(6.0);
        }
    }
}
