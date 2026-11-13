use crate::app::{Confirm, WaydlpApp, truncate};
use crate::widgets::setting_row;
use egui::{RichText, Ui};
use waydlp_config::{DEFAULT_ACCENT, Density, Page, SettingsStore};
use waydlp_core::humanize;
use waydlp_engine::ToolSource;
use waydlp_theme::palette::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    Downloads,
    Appearance,
    Tools,
    Privacy,
    Presets,
}

impl WaydlpApp {
    pub fn page_settings(&mut self, ui: &mut Ui) {
        waydlp_theme::heading(ui, "Settings");
        ui.label(waydlp_theme::muted("Changes are saved automatically."));
        ui.add_space(8.0);
        let id = egui::Id::unique("settings-tab");
        let mut tab = ui.data(|d| d.get_temp::<u8>(id)).map(tab_from).unwrap_or(Tab::General);
        ui.horizontal(|ui| {
            for (t, l) in [
                (Tab::General, "General"),
                (Tab::Downloads, "Downloads"),
                (Tab::Appearance, "Appearance"),
                (Tab::Tools, "yt-dlp & FFmpeg"),
                (Tab::Privacy, "Privacy"),
                (Tab::Presets, "Presets & backup"),
            ] {
                if ui.add(egui::Button::selectable(tab == t, l)).clicked() {
                    tab = t;
                }
            }
        });
        ui.data_mut(|d| d.insert_temp(id, tab as u8));
        ui.add_space(10.0);

        if !self.settings_problems.is_empty() {
            waydlp_theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                for p in &self.settings_problems {
                    ui.label(RichText::new(format!("⚠ {p}")).color(WARNING));
                }
            });
            ui.add_space(8.0);
        }

        let before = self.settings.clone();
        waydlp_theme::card().show(ui, |ui| match tab {
            Tab::General => self.tab_general(ui),
            Tab::Downloads => self.tab_downloads(ui),
            Tab::Appearance => self.tab_appearance(ui),
            Tab::Tools => self.tab_tools(ui),
            Tab::Privacy => self.tab_privacy(ui),
            Tab::Presets => self.tab_presets(ui),
        });
        if self.settings != before {
            self.settings_changed();
            if self.settings.privacy.auto_update_yt_dlp && !before.privacy.auto_update_yt_dlp {
                self.maybe_auto_update_yt_dlp();
            }
        }
    }

    fn tab_general(&mut self, ui: &mut Ui) {
        let s = &mut self.settings;
        let dir = s.general.download_dir.clone().unwrap_or_else(waydlp_platform::default_download_dir);
        setting_row(ui, "Download folder", &truncate(&dir.display().to_string(), 70), |ui| {
            if s.general.download_dir.is_some() && ui.button("Use default").clicked() {
                s.general.download_dir = None;
            }
            if ui.button("Change…").clicked()
                && let Some(d) = rfd::FileDialog::new().set_directory(&dir).pick_folder()
            {
                s.general.download_dir = Some(d);
            }
        });
        setting_row(ui, "Start page", "Shown when waydlp opens", |ui| {
            egui::ComboBox::from_id_salt("landing").selected_text(s.general.landing_page.label()).show_ui(ui, |ui| {
                for p in [Page::Home, Page::Downloader, Page::Queue, Page::History] {
                    ui.selectable_value(&mut s.general.landing_page, p, p.label());
                }
            });
        });
        setting_row(
            ui,
            "Notify when a download finishes",
            "Desktop notification while waydlp is in the background",
            |ui| {
                ui.checkbox(&mut s.general.notify_on_complete, "");
            },
        );
        setting_row(ui, "Notify when a download fails", "", |ui| {
            ui.checkbox(&mut s.general.notify_on_failure, "");
        });
        setting_row(ui, "Show the file in its folder when done", "", |ui| {
            ui.checkbox(&mut s.general.open_folder_when_done, "");
        });
        setting_row(ui, "Ask before quitting with active downloads", "", |ui| {
            ui.checkbox(&mut s.general.confirm_quit_with_active, "");
        });
    }

    fn tab_downloads(&mut self, ui: &mut Ui) {
        let presets = self.settings.all_presets();
        let s = &mut self.settings;
        setting_row(ui, "Default preset", "Used by Quick download, batch downloads and new analyses", |ui| {
            let name =
                presets.iter().find(|p| p.id == s.downloads.default_preset).map(|p| p.name.clone()).unwrap_or_default();
            egui::ComboBox::from_id_salt("def-preset").selected_text(name).width(220.0).show_ui(ui, |ui| {
                for p in &presets {
                    ui.selectable_value(
                        &mut s.downloads.default_preset,
                        p.id.clone(),
                        format!("{} — {}", p.name, p.options.summary()),
                    );
                }
            });
        });
        setting_row(
            ui,
            "File name template",
            "yt-dlp output template. Folders are allowed, e.g. %(uploader)s/%(title)s.%(ext)s",
            |ui| {
                if ui.small_button("Reset").clicked() {
                    s.downloads.filename_template = waydlp_core::sanitize::DEFAULT_TEMPLATE.into();
                }
                ui.add(
                    egui::TextEdit::singleline(&mut s.downloads.filename_template)
                        .desired_width(300.0)
                        .font(egui::TextStyle::Monospace),
                );
            },
        );
        setting_row(ui, "Simultaneous downloads", "", |ui| {
            ui.add(egui::Slider::new(&mut s.downloads.max_concurrent, 1..=8));
        });
        setting_row(ui, "Automatic retries", "For temporary problems such as network errors or rate limits", |ui| {
            ui.add(egui::Slider::new(&mut s.downloads.auto_retries, 0..=5));
        });
        setting_row(ui, "yt-dlp retries per request", "", |ui| {
            ui.add(egui::DragValue::new(&mut s.downloads.retries).range(0..=100));
        });
        setting_row(ui, "Speed limit", "Per download, e.g. 2M or 500K. Empty = unlimited", |ui| {
            ui.add(egui::TextEdit::singleline(&mut s.downloads.rate_limit).hint_text("unlimited").desired_width(120.0));
        });
        setting_row(ui, "Network timeout (seconds)", "", |ui| {
            ui.add(egui::DragValue::new(&mut s.downloads.socket_timeout_secs).range(1..=600));
        });
        setting_row(
            ui,
            "Skip already-downloaded items",
            "Keeps a download archive so playlists only fetch new items",
            |ui| {
                ui.checkbox(&mut s.downloads.use_download_archive, "");
            },
        );
        ui.separator();
        setting_row(ui, "Playlists: put files in a folder", "", |ui| {
            ui.checkbox(&mut s.downloads.playlist_subfolder, "");
        });
        setting_row(ui, "Playlists: number files", "", |ui| {
            ui.checkbox(&mut s.downloads.playlist_numbering, "");
        });
        setting_row(ui, "Playlists: confirm above", "Ask before downloading more than this many items", |ui| {
            ui.add(egui::DragValue::new(&mut s.downloads.playlist_confirm_threshold).range(1..=10000));
        });
    }

    fn tab_appearance(&mut self, ui: &mut Ui) {
        let s = &mut self.settings.appearance;
        setting_row(ui, "Accent colour", "The default is waydlp purple", |ui| {
            if s.accent != DEFAULT_ACCENT && ui.button("Default").clicked() {
                s.accent = DEFAULT_ACCENT;
            }
            ui.color_edit_button_srgb(&mut s.accent);
        });
        setting_row(ui, "Interface scale", "", |ui| {
            ui.add(
                egui::Slider::new(&mut s.ui_scale, 0.75..=2.0)
                    .step_by(0.05)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
            );
        });
        setting_row(ui, "Layout density", "", |ui| {
            ui.selectable_value(&mut s.density, Density::Compact, "Compact");
            ui.selectable_value(&mut s.density, Density::Comfortable, "Comfortable");
        });
        setting_row(ui, "Animations", "", |ui| {
            ui.checkbox(&mut s.animations, "");
        });
        setting_row(ui, "Reduce motion", "Disables animated progress and transitions", |ui| {
            ui.checkbox(&mut s.reduced_motion, "");
        });
    }

    fn tab_tools(&mut self, ui: &mut Ui) {
        let accent = self.accent();
        waydlp_theme::subheading(ui, "yt-dlp");
        ui.label(waydlp_theme::muted("Extracts media from websites. Sites change often, so keep it up to date."));
        let yt = self.tools.as_ref().and_then(|t| t.yt_dlp.clone());
        match &yt {
            Some(i) => {
                ui.label(format!(
                    "Installed: {} — {} ({})",
                    i.version.as_deref().unwrap_or("unknown version"),
                    i.source.label(),
                    i.path.display()
                ));
            }
            None if self.tools.is_some() => {
                ui.label(RichText::new("yt-dlp was not found.").color(ERROR));
            }
            None => {
                ui.label(waydlp_theme::muted("Checking…"));
            }
        }
        ui.horizontal(|ui| {
            let managed_or_missing = yt.as_ref().is_none_or(|i| i.source == ToolSource::Managed);
            let label = if yt.is_some() { "Update yt-dlp" } else { "Install yt-dlp" };
            if managed_or_missing {
                if waydlp_theme::primary_button_enabled(ui, accent, label, self.install.is_none()).clicked() {
                    self.install_tool("yt-dlp");
                }
            } else if ui
                .button("Install a managed copy")
                .on_hover_text("Downloads the official build into waydlp's own folder and uses it instead")
                .clicked()
            {
                self.install_tool("yt-dlp");
            }
            if ui.button("Check latest version").clicked() {
                self.check_yt_dlp_latest();
            }
            match &self.yt_dlp_latest {
                Some(Ok(v)) => {
                    let installed = yt.as_ref().and_then(|i| i.version.clone()).unwrap_or_default();
                    if waydlp_engine::managed::yt_dlp_version_is_newer(v, &installed) {
                        ui.label(RichText::new(format!("{v} is available")).color(SUCCESS));
                    } else {
                        ui.label(waydlp_theme::muted(format!("Latest is {v} — up to date")));
                    }
                }
                Some(Err(e)) => {
                    ui.label(RichText::new(e).color(ERROR));
                }
                None => {}
            }
        });
        if yt.as_ref().is_some_and(|i| i.source == ToolSource::System) {
            ui.label(waydlp_theme::muted("This copy belongs to your system (package manager, pip or winget). Update it there; waydlp won't modify it."));
        }
        ui.add_space(12.0);

        waydlp_theme::subheading(ui, "FFmpeg");
        ui.label(waydlp_theme::muted(
            "Merges video and audio, converts audio and embeds thumbnails, subtitles and metadata.",
        ));
        let ff = self.tools.as_ref().and_then(|t| t.ffmpeg.clone());
        match &ff {
            Some(i) => {
                ui.label(format!(
                    "Installed: {} — {} ({})",
                    truncate(i.version.as_deref().unwrap_or("unknown"), 40),
                    i.source.label(),
                    i.path.display()
                ));
            }
            None if self.tools.is_some() => {
                ui.label(
                    RichText::new("FFmpeg was not found. Downloads still work in “Original file” mode.").color(WARNING),
                );
            }
            None => {}
        }
        if self.tools.as_ref().is_some_and(|t| t.ffmpeg.is_some() && t.ffprobe.is_none()) {
            ui.label(
                RichText::new("FFprobe was not found next to FFmpeg; some post-processing may fail.").color(WARNING),
            );
        }
        if waydlp_engine::managed::ffmpeg_download_supported() {
            let managed_or_missing = ff.as_ref().is_none_or(|i| i.source == ToolSource::Managed);
            if managed_or_missing
                && waydlp_theme::primary_button_enabled(
                    ui,
                    accent,
                    if ff.is_some() { "Update FFmpeg" } else { "Install FFmpeg (~200 MB)" },
                    self.install.is_none(),
                )
                .clicked()
            {
                self.install_tool("ffmpeg");
            }
            ui.label(waydlp_theme::muted(
                "Downloads the GPL build published by the yt-dlp project and verifies its SHA-256 checksum.",
            ));
        } else {
            ui.label(waydlp_theme::secondary("Install FFmpeg with your package manager:"));
            for cmd in [
                "Debian/Ubuntu/Mint:  sudo apt install ffmpeg",
                "Fedora:  sudo dnf install ffmpeg-free   (or ffmpeg from RPM Fusion)",
                "openSUSE:  sudo zypper install ffmpeg",
                "Arch/CachyOS/EndeavourOS/Manjaro:  sudo pacman -S ffmpeg",
            ] {
                ui.label(RichText::new(cmd).monospace().small());
            }
        }
        if let Some(i) = &self.install {
            let frac = i.total.map(|t| i.done as f32 / t.max(1) as f32);
            let text = match i.total {
                Some(t) => format!("Downloading {} — {} of {}", i.tool, humanize::bytes(i.done), humanize::bytes(t)),
                None => format!("Downloading {} — {}", i.tool, humanize::bytes(i.done)),
            };
            waydlp_theme::progress_bar(ui, accent, frac, Some(text), self.animate());
        }
        if let Some(e) = &self.install_error {
            ui.label(RichText::new(e).color(ERROR));
        }
        ui.add_space(12.0);

        waydlp_theme::subheading(ui, "Custom locations");
        let s = &mut self.settings.advanced;
        let mut refresh = false;
        for (label, path, filter) in [
            ("yt-dlp executable", &mut s.yt_dlp_path, true),
            ("FFmpeg executable or folder", &mut s.ffmpeg_path, false),
        ] {
            let shown = path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "automatic".into());
            setting_row(ui, label, &truncate(&shown, 70), |ui| {
                if path.is_some() && ui.button("Automatic").clicked() {
                    *path = None;
                    refresh = true;
                }
                if ui.button("Browse…").clicked() {
                    let mut d = rfd::FileDialog::new();
                    if cfg!(windows) {
                        d = d.add_filter("Programs", &["exe"]);
                    }
                    let picked = if filter {
                        d.pick_file()
                    } else {
                        d.pick_file().or_else(|| rfd::FileDialog::new().pick_folder())
                    };
                    if let Some(p) = picked {
                        *path = Some(p);
                        refresh = true;
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            if ui.button("Re-detect tools").clicked() {
                refresh = true;
            }
            if self.tools_checking {
                ui.spinner();
            }
        });
        if refresh {
            self.tools = None;
            self.refresh_tools();
        }
        ui.add_space(12.0);

        waydlp_theme::subheading(ui, "Network & advanced");
        let s = &mut self.settings.advanced;
        setting_row(ui, "Proxy", "e.g. http://host:3128 or socks5://127.0.0.1:9050", |ui| {
            ui.add(egui::TextEdit::singleline(&mut s.proxy).hint_text("none").desired_width(260.0));
        });
        setting_row(
            ui,
            "Extra yt-dlp arguments",
            "For power users. Passed as separate arguments, never through a shell. Options that run commands are blocked.",
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut s.custom_args)
                        .hint_text("--sponsorblock-remove sponsor")
                        .desired_width(300.0)
                        .font(egui::TextStyle::Monospace),
                );
            },
        );
        let cookies = s.cookies_file.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "none".into());
        setting_row(
            ui,
            "Cookies file (sign-in)",
            &format!(
                "Netscape cookies.txt for sites that need you to be signed in. waydlp never reads your browser's cookies by itself. Current: {}",
                truncate(&cookies, 50)
            ),
            |ui| {
                if s.cookies_file.is_some() && ui.button("Remove").clicked() {
                    s.cookies_file = None;
                }
                if ui.button("Choose…").clicked()
                    && let Some(p) = rfd::FileDialog::new().add_filter("Cookies", &["txt"]).pick_file()
                {
                    s.cookies_file = Some(p);
                }
            },
        );
        if s.cookies_file.is_some() {
            ui.label(RichText::new("⚠ A cookies file gives access to your accounts. Keep it private and remove it when you no longer need it.").small().color(WARNING));
        }
        setting_row(ui, "Use my yt-dlp configuration files", "Off by default so waydlp behaves predictably", |ui| {
            ui.checkbox(&mut s.use_yt_dlp_config, "");
        });
        setting_row(ui, "Debug logging", "More detail in the log files (takes effect after restart)", |ui| {
            if ui.button("Open logs folder").clicked() {
                let _ = waydlp_platform::open_path(&self.paths.log_dir);
            }
            ui.checkbox(&mut s.debug_logging, "");
        });
    }

    fn tab_privacy(&mut self, ui: &mut Ui) {
        let s = &mut self.settings.privacy;
        ui.label(waydlp_theme::secondary(
            "waydlp has no accounts, ads or telemetry. These settings control what it stores and fetches.",
        ));
        ui.add_space(6.0);
        setting_row(ui, "Save download history", "", |ui| {
            ui.checkbox(&mut s.save_history, "");
        });
        setting_row(
            ui,
            "Keep history for (days)",
            "0 keeps it forever. Older entries are removed at start-up.",
            |ui| {
                ui.add(egui::DragValue::new(&mut s.history_retention_days).range(0..=3650));
            },
        );
        setting_row(ui, "Watch the clipboard for links", "Only while waydlp is focused. Off by default.", |ui| {
            ui.checkbox(&mut s.clipboard_watch, "");
        });
        setting_row(ui, "Load thumbnails", "Thumbnails are fetched from the media site", |ui| {
            ui.checkbox(&mut s.load_thumbnails, "");
        });
        setting_row(
            ui,
            "Check for waydlp updates at start-up",
            "Contacts GitHub once per start. Off by default.",
            |ui| {
                ui.checkbox(&mut s.check_updates_on_start, "");
            },
        );
        setting_row(
            ui,
            "Keep yt-dlp up to date automatically",
            "Updates the copy managed by waydlp when a new release exists. Contacts GitHub at most once a day. Off by default.",
            |ui| {
                ui.checkbox(&mut s.auto_update_yt_dlp, "");
            },
        );
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Clear history…").clicked() {
                self.confirm = Some(Confirm::ClearHistory);
            }
            if ui.button("Clear image cache").clicked() {
                ui.ctx().forget_all_images();
                self.toast("Image cache cleared", SUCCESS);
            }
            if ui.button("Clear download archive").on_hover_text("Forget which items were already downloaded").clicked()
            {
                let _ = std::fs::remove_file(self.paths.archive_file());
                self.toast("Download archive cleared", SUCCESS);
            }
        });
    }

    fn tab_presets(&mut self, ui: &mut Ui) {
        waydlp_theme::subheading(ui, "Your presets");
        if self.settings.presets.is_empty() {
            ui.label(waydlp_theme::muted("Save options as a preset from the Downloader page."));
        }
        let mut remove = None;
        for (i, p) in self.settings.presets.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(200.0));
                ui.label(waydlp_theme::muted(p.options.summary()));
                if ui.small_button("Delete").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            let p = self.settings.presets.remove(i);
            if self.settings.downloads.default_preset == p.id {
                self.settings.downloads.default_preset = "mp4-1080".into();
            }
        }
        ui.add_space(6.0);
        ui.label(waydlp_theme::muted("Built-in presets:"));
        for p in waydlp_core::presets::builtin_presets() {
            ui.label(RichText::new(format!("• {} — {}", p.name, p.options.summary())).small().color(TEXT_SECONDARY));
        }
        ui.add_space(12.0);
        waydlp_theme::subheading(ui, "Backup");
        ui.label(waydlp_theme::muted("Export your settings and presets to a file, or import them on another computer. Tool paths and cookies are never imported."));
        ui.horizontal(|ui| {
            if ui.button("Export settings…").clicked()
                && let Some(p) = rfd::FileDialog::new()
                    .set_file_name("waydlp-settings.toml")
                    .add_filter("TOML", &["toml"])
                    .save_file()
            {
                match SettingsStore::export(&self.settings, &p) {
                    Ok(()) => self.toast(format!("Exported to {}", p.display()), SUCCESS),
                    Err(e) => self.toast(e.to_string(), ERROR),
                }
            }
            if ui.button("Import settings…").clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("TOML", &["toml"]).pick_file()
            {
                match SettingsStore::import(&p, &self.settings) {
                    Ok(s) => {
                        self.settings = s;
                        self.toast("Settings imported", SUCCESS);
                    }
                    Err(e) => self.toast(format!("Import failed: {e}"), ERROR),
                }
            }
            if ui.button(RichText::new("Reset to defaults…").color(ERROR)).clicked() {
                self.confirm = Some(Confirm::ResetSettings);
            }
        });
        ui.add_space(6.0);
        ui.label(waydlp_theme::muted(format!("Settings file: {}", self.store.path().display())));
    }
}

fn tab_from(v: u8) -> Tab {
    match v {
        1 => Tab::Downloads,
        2 => Tab::Appearance,
        3 => Tab::Tools,
        4 => Tab::Privacy,
        5 => Tab::Presets,
        _ => Tab::General,
    }
}
