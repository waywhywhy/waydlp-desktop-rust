//! App-specific UI helpers.

use egui::{Color32, RichText, Ui};
use waydlp_core::errors::FailureReport;
use waydlp_core::humanize;
use waydlp_core::progress::DownloadProgress;
use waydlp_engine::JobState;
use waydlp_history::EntryStatus;
use waydlp_theme::palette::*;

pub fn state_color(s: JobState) -> Color32 {
    match s {
        JobState::Running => ACCENT_HOVER,
        JobState::Queued | JobState::WaitingRetry => TEXT_SECONDARY,
        JobState::Paused => WARNING,
        JobState::Completed => SUCCESS,
        JobState::Failed => ERROR,
        JobState::Cancelled => TEXT_MUTED,
    }
}

pub fn entry_color(s: EntryStatus) -> Color32 {
    match s {
        EntryStatus::Completed => SUCCESS,
        EntryStatus::Failed => ERROR,
        EntryStatus::Cancelled => TEXT_MUTED,
        EntryStatus::InProgress => WARNING,
    }
}

/// "12.3 MB of ~45 MB · 2.1 MB/s · 15s left · fragment 3/40" — only the
/// parts yt-dlp actually reported.
pub fn progress_text(p: &DownloadProgress) -> String {
    let mut parts = Vec::new();
    match (p.downloaded, p.total) {
        (Some(d), Some(t)) => parts.push(format!(
            "{} of {}{}",
            humanize::bytes(d),
            if p.total_is_estimate { "~" } else { "" },
            humanize::bytes(t)
        )),
        (Some(d), None) => parts.push(humanize::bytes(d)),
        _ => {}
    }
    if let Some(s) = p.speed {
        parts.push(humanize::speed(s));
    }
    if let Some(e) = p.eta
        && p.status == "downloading"
    {
        parts.push(format!("{} left", humanize::eta(e)));
    }
    if let Some((i, n)) = p.fragment {
        parts.push(format!("fragment {i}/{n}"));
    }
    parts.join(" · ")
}

/// Readable error with an expandable technical section.
pub fn failure_view(ui: &mut Ui, id_salt: impl std::hash::Hash + std::fmt::Debug, f: &FailureReport) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(f.kind.title()).color(ERROR).strong());
        ui.label(RichText::new(f.kind.hint()).color(TEXT_SECONDARY));
    });
    egui::CollapsingHeader::new(RichText::new("Technical details").small().color(TEXT_MUTED))
        .id_salt(("failure", id_salt))
        .show(ui, |ui| {
            ui.label(RichText::new(&f.summary).monospace().color(TEXT_SECONDARY));
            if !f.details.trim().is_empty() {
                egui::ScrollArea::vertical().max_height(180.0).id_salt("details-scroll").show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(f.details.trim()).monospace().small().color(TEXT_MUTED))
                            .selectable(true),
                    );
                });
            }
            if ui.small_button("Copy details").clicked() {
                ui.ctx().copy_text(format!("{}\n\n{}", f.summary, f.details));
            }
        });
}

/// Section label used inside cards.
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).small().strong().color(TEXT_MUTED));
}

/// A label + right-aligned control row for settings.
pub fn setting_row(ui: &mut Ui, label: &str, help: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width() * 0.55);
            ui.label(RichText::new(label).color(TEXT_PRIMARY));
            if !help.is_empty() {
                ui.label(RichText::new(help).small().color(TEXT_MUTED));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
    ui.add_space(4.0);
}

/// Compress sorted 1-based indices into a yt-dlp `--playlist-items` spec.
pub fn items_spec(indices: &[usize]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < indices.len() {
        let start = indices[i];
        let mut end = start;
        while i + 1 < indices.len() && indices[i + 1] == end + 1 {
            i += 1;
            end = indices[i];
        }
        out.push(if start == end { start.to_string() } else { format!("{start}-{end}") });
        i += 1;
    }
    out.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_spec_compresses_ranges() {
        assert_eq!(items_spec(&[1, 2, 3, 5, 7, 8]), "1-3,5,7-8");
        assert_eq!(items_spec(&[4]), "4");
        assert_eq!(items_spec(&[]), "");
    }

    #[test]
    fn progress_text_only_reports_known_values() {
        let p = DownloadProgress {
            status: "downloading".into(),
            downloaded: Some(1024 * 1024),
            total: Some(4 * 1024 * 1024),
            total_is_estimate: true,
            speed: None,
            eta: Some(5),
            fragment: None,
            stream: waydlp_core::progress::StreamKind::Video,
        };
        assert_eq!(progress_text(&p), "1.0 MB of ~4.0 MB · 5s left");
    }
}
