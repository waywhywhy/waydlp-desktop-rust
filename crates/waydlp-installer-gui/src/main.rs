//! waydlp graphical installer.
//!
//! The same binary is the Windows `setup.exe` (with the application archive
//! appended, see `waydlp-installer-core::payload`) and the uninstaller.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui;
use egui::{RichText, Ui};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use waydlp_core::artifacts::{Os, PackageKind};
use waydlp_installer_core::payload::{self, Payload};
use waydlp_installer_core::plan::{InstallOptions, Progress, Source};
use waydlp_installer_core::system::Elevation;
use waydlp_installer_core::{Installer, Receipt, available_methods};
use waydlp_theme::{Accent, palette::*};

const LICENSE: &str = include_str!("../../../LICENSE");
const ICON_PNG: &[u8] = include_bytes!("../../../assets/icons/hicolor/256x256/apps/io.github.waywhywhy.waydlp.png");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Welcome,
    License,
    Options,
    Dependencies,
    Working,
    Done,
}

enum Msg {
    Progress(Progress),
    Finished(Result<Option<PathBuf>, String>),
}

struct SetupApp {
    step: Step,
    installer: Option<Installer>,
    init_error: Option<String>,
    payload: Option<Payload>,
    existing: Option<Receipt>,
    uninstall_mode: bool,
    accepted: bool,
    methods: Vec<(PackageKind, Result<(), String>)>,
    method: PackageKind,
    install_dir: PathBuf,
    desktop_shortcut: bool,
    menu_entry: bool,
    launch_after: bool,
    log: Vec<String>,
    download: Option<(u64, Option<u64>)>,
    files: Option<(usize, usize)>,
    rx: Option<Receiver<Msg>>,
    result: Option<Result<Option<PathBuf>, String>>,
}

impl SetupApp {
    fn new(uninstall_mode: bool) -> Self {
        let (installer, init_error) = match Installer::new() {
            Ok(i) => (Some(i), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let payload = payload::current().ok().flatten();
        let existing = Receipt::load().ok().flatten();
        let methods = installer.as_ref().map(|i| available_methods(i.target)).unwrap_or_default();
        let method = methods.iter().find(|(_, r)| r.is_ok()).map(|(k, _)| *k).unwrap_or(PackageKind::TarGz);
        let install_dir = installer.as_ref().and_then(|i| i.default_install_dir().ok()).unwrap_or_default();
        Self {
            step: Step::Welcome,
            installer,
            init_error,
            payload,
            existing,
            uninstall_mode,
            accepted: false,
            methods,
            method,
            install_dir,
            desktop_shortcut: false,
            menu_entry: true,
            launch_after: true,
            log: Vec::new(),
            download: None,
            files: None,
            rx: None,
            result: None,
        }
    }

    fn accent(&self) -> Accent {
        Accent::default()
    }

    fn start(&mut self, ctx: &egui::Context) {
        let Some(inst) = self.installer.as_ref() else { return };
        let target = inst.target;
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.step = Step::Working;
        self.log.clear();
        let ctx2 = ctx.clone();
        let uninstall = self.uninstall_mode;
        let source = match (&self.payload, self.method) {
            (Some(p), PackageKind::WindowsPortable | PackageKind::TarGz) => Source::Embedded(p.clone()),
            _ => Source::Release { version: None },
        };
        let opts = InstallOptions {
            method: self.method,
            source,
            install_dir: Some(self.install_dir.clone()).filter(|d| !d.as_os_str().is_empty()),
            desktop_shortcut: self.desktop_shortcut,
            menu_entry: self.menu_entry,
            assume_yes: true,
            elevation: Elevation::Pkexec,
        };
        std::thread::spawn(move || {
            let inst = Installer { target };
            let ptx = tx.clone();
            let pctx = ctx2.clone();
            let progress = move |p: Progress| {
                let _ = ptx.send(Msg::Progress(p));
                pctx.request_repaint();
            };
            let result = if uninstall {
                inst.uninstall(true, Elevation::Pkexec, &progress).map(|_| None).map_err(|e| e.to_string())
            } else {
                inst.install(&opts, &progress)
                    .map(|r| {
                        r.install_dir
                            .map(|d| d.join(if cfg!(windows) { "waydlp.exe" } else { "waydlp" }))
                            .or_else(|| r.files.first().map(|f| f.path.clone()))
                    })
                    .map_err(|e| e.to_string())
            };
            let _ = tx.send(Msg::Finished(result));
            ctx2.request_repaint();
        });
    }

    fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Progress(Progress::Status(s)) => self.log.push(s),
                Msg::Progress(Progress::Download { done, total }) => self.download = Some((done, total)),
                Msg::Progress(Progress::Files { done, total }) => self.files = Some((done, total)),
                Msg::Finished(r) => {
                    self.result = Some(r);
                    self.step = Step::Done;
                }
            }
        }
    }

    fn nav(&mut self, ui: &mut Ui, back: Option<Step>, next: Option<(&str, bool)>) -> bool {
        let accent = self.accent();
        let mut go_next = false;
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((label, enabled)) = next
                        && waydlp_theme::primary_button_enabled(ui, accent, label, enabled).clicked()
                    {
                        go_next = true;
                    }
                    if let Some(b) = back
                        && waydlp_theme::ghost_button(ui, "Back").clicked()
                    {
                        self.step = b;
                    }
                });
            });
        });
        go_next
    }

    fn page(&mut self, ui: &mut Ui) {
        let accent = self.accent();
        if let Some(e) = &self.init_error {
            waydlp_theme::heading(ui, "Unsupported system");
            ui.label(RichText::new(e).color(ERROR));
            return;
        }
        match self.step {
            Step::Welcome => {
                ui.horizontal(|ui| {
                    waydlp_theme::logo(ui, 64.0, accent);
                    ui.vertical(|ui| {
                        waydlp_theme::title(
                            ui,
                            if self.uninstall_mode { "Uninstall waydlp" } else { "Welcome to waydlp" },
                        );
                        ui.label(waydlp_theme::secondary("Your media, your format, your way."));
                    });
                });
                ui.add_space(16.0);
                if self.uninstall_mode {
                    match &self.existing {
                        Some(r) => {
                            ui.label(format!("waydlp {} is installed ({}).", r.version, r.method.display_name()));
                            ui.label(waydlp_theme::muted("Uninstalling removes the application. Your settings, history and downloaded files are kept."));
                        }
                        None => {
                            ui.label(RichText::new("No installation made by this installer was found.").color(WARNING));
                        }
                    }
                    let can = self.existing.is_some();
                    if self.nav(ui, None, Some(("Uninstall", can))) {
                        let ctx = ui.ctx().clone();
                        self.start(&ctx);
                    }
                    return;
                }
                ui.label("waydlp is a free, open-source media downloader powered by yt-dlp and FFmpeg.");
                ui.label(waydlp_theme::muted("No ads. No accounts. No tracking."));
                ui.add_space(10.0);
                waydlp_theme::card().show(ui, |ui| {
                    match &self.payload {
                        Some(p) => ui.label(format!("This installer contains waydlp {}.", p.version)),
                        None => ui.label(
                            "This installer downloads the latest waydlp release from GitHub and verifies its checksum.",
                        ),
                    };
                    if let Some(r) = &self.existing {
                        ui.label(
                            RichText::new(format!(
                                "waydlp {} is already installed ({}); it will be upgraded.",
                                r.version,
                                r.method.display_name()
                            ))
                            .color(ACCENT_HOVER),
                        );
                    }
                });
                if self.nav(ui, None, Some(("Next", true))) {
                    self.step = Step::License;
                }
            }
            Step::License => {
                waydlp_theme::heading(ui, "License");
                ui.label(waydlp_theme::secondary(
                    "waydlp is licensed under the GNU General Public License v3.0 or later.",
                ));
                ui.add_space(6.0);
                waydlp_theme::card().show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        ui.label(RichText::new(LICENSE).monospace().small().color(TEXT_SECONDARY));
                    });
                });
                ui.checkbox(&mut self.accepted, "I accept the license terms");
                if self.nav(ui, Some(Step::Welcome), Some(("Next", self.accepted))) {
                    self.step = Step::Options;
                }
            }
            Step::Options => {
                waydlp_theme::heading(ui, "Installation options");
                ui.add_space(6.0);
                waydlp_theme::card().show(ui, |ui| {
                    ui.label(RichText::new("Package format").strong());
                    for (k, r) in self.methods.clone() {
                        ui.add_enabled_ui(r.is_ok(), |ui| {
                            let label = match k {
                                PackageKind::WindowsPortable => "Install for me (recommended, no administrator rights)".to_string(),
                                PackageKind::TarGz => "Install for me in ~/.local (no administrator rights)".to_string(),
                                other => other.display_name().to_string(),
                            };
                            let resp = ui.radio_value(&mut self.method, k, label);
                            if let Err(why) = &r {
                                resp.on_disabled_hover_text(why);
                            }
                        });
                    }
                    if matches!(self.method, PackageKind::Deb | PackageKind::Rpm | PackageKind::ArchPkg) {
                        ui.label(waydlp_theme::muted("Your system will ask for your password; the package manager installs and later updates waydlp."));
                    }
                });
                ui.add_space(8.0);
                let user_level =
                    matches!(self.method, PackageKind::WindowsPortable | PackageKind::TarGz | PackageKind::AppImage);
                if user_level {
                    waydlp_theme::card().show(ui, |ui| {
                        if self.method != PackageKind::AppImage {
                            ui.horizontal(|ui| {
                                ui.label("Install location");
                                ui.label(RichText::new(self.install_dir.display().to_string()).color(TEXT_SECONDARY));
                                if ui.button("Change…").clicked()
                                    && let Some(d) = rfd::FileDialog::new().pick_folder()
                                {
                                    self.install_dir = d.join("waydlp");
                                }
                            });
                        }
                        ui.checkbox(
                            &mut self.menu_entry,
                            if cfg!(windows) { "Add to the Start menu" } else { "Add to the application menu" },
                        );
                        ui.checkbox(&mut self.desktop_shortcut, "Create a desktop shortcut");
                        ui.checkbox(&mut self.launch_after, "Launch waydlp when finished");
                    });
                }
                if self.nav(ui, Some(Step::License), Some(("Next", true))) {
                    self.step = Step::Dependencies;
                }
            }
            Step::Dependencies => {
                waydlp_theme::heading(ui, "Dependencies");
                ui.label(waydlp_theme::secondary("waydlp uses two free tools. You don't need them now: waydlp can install verified copies on first start."));
                ui.add_space(8.0);
                waydlp_theme::card().show(ui, |ui| {
                    for (name, what) in [("yt-dlp", "extracts media from websites"), ("ffmpeg", "merges, converts and embeds")] {
                        let found = which::which(name).is_ok();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(if found { "✔" } else { "•" }).color(if found { SUCCESS } else { TEXT_MUTED }));
                            ui.label(RichText::new(name).strong());
                            ui.label(waydlp_theme::muted(format!("— {what}")));
                            ui.label(if found {
                                RichText::new("found on this system").color(SUCCESS)
                            } else {
                                RichText::new("not found — install from waydlp » Settings").color(TEXT_SECONDARY)
                            });
                        });
                    }
                    if self.installer.as_ref().is_some_and(|i| i.target.os == Os::Linux) {
                        ui.add_space(4.0);
                        ui.label(waydlp_theme::muted("On Linux, installing FFmpeg with your package manager is recommended (e.g. sudo apt install ffmpeg)."));
                    }
                });
                if self.nav(ui, Some(Step::Options), Some(("Install", true))) {
                    let ctx = ui.ctx().clone();
                    self.start(&ctx);
                }
            }
            Step::Working => {
                waydlp_theme::heading(ui, if self.uninstall_mode { "Uninstalling…" } else { "Installing…" });
                ui.add_space(8.0);
                let (frac, text) = match (self.files, self.download) {
                    (Some((d, t)), _) => (Some(d as f32 / t.max(1) as f32), format!("{d} of {t} files")),
                    (None, Some((d, Some(t)))) => (
                        Some(d as f32 / t.max(1) as f32),
                        format!("{:.1} of {:.1} MB", d as f64 / 1048576.0, t as f64 / 1048576.0),
                    ),
                    (None, Some((d, None))) => (None, format!("{:.1} MB", d as f64 / 1048576.0)),
                    _ => (None, "Working…".into()),
                };
                waydlp_theme::progress_bar(ui, accent, frac, Some(text), true);
                ui.add_space(8.0);
                for l in &self.log {
                    ui.label(waydlp_theme::secondary(l));
                }
            }
            Step::Done => match &self.result {
                Some(Ok(exe)) => {
                    waydlp_theme::title(ui, if self.uninstall_mode { "waydlp was removed" } else { "waydlp is ready" });
                    for l in &self.log {
                        ui.label(waydlp_theme::secondary(l));
                    }
                    let exe = exe.clone();
                    if self.nav(ui, None, Some(("Finish", true))) {
                        if self.launch_after
                            && !self.uninstall_mode
                            && let Some(e) = exe.filter(|e| e.exists())
                        {
                            let _ = std::process::Command::new(e).spawn();
                        }
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                Some(Err(e)) => {
                    waydlp_theme::heading(ui, "Something went wrong");
                    ui.label(RichText::new(e).color(ERROR));
                    for l in &self.log {
                        ui.label(waydlp_theme::muted(l));
                    }
                    if self.nav(ui, Some(Step::Options), Some(("Close", true))) {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                None => {}
            },
        }
    }
}

impl eframe::App for SetupApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let steps = ["Welcome", "License", "Options", "Dependencies", "Install", "Done"];
        let current = self.step as usize;
        egui::Panel::left("steps")
            .exact_size(190.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(BG_SECONDARY).inner_margin(egui::Margin::same(18)))
            .show(ui, |ui| {
                waydlp_theme::logo(ui, 40.0, self.accent());
                ui.label(RichText::new("waydlp setup").strong().color(TEXT_PRIMARY));
                ui.add_space(18.0);
                if !self.uninstall_mode {
                    for (i, s) in steps.iter().enumerate() {
                        let (c, mark) = if i < current {
                            (SUCCESS, "✔")
                        } else if i == current {
                            (ACCENT_HOVER, "•")
                        } else {
                            (TEXT_MUTED, "○")
                        };
                        ui.label(RichText::new(format!("{mark}  {s}")).color(c));
                        ui.add_space(4.0);
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.label(waydlp_theme::muted(format!("installer {}", env!("CARGO_PKG_VERSION"))));
                });
            });
        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(BG_PRIMARY).inner_margin(egui::Margin::same(26)))
            .show(ui, |ui| self.page(ui));
        if self.step == Step::Working {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

fn icon() -> egui::IconData {
    match image::load_from_memory(ICON_PNG) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::IconData { rgba: rgba.into_raw(), width: w, height: h }
        }
        Err(_) => egui::IconData::default(),
    }
}

fn main() -> eframe::Result {
    let uninstall = std::env::args().any(|a| a == "uninstall" || a == "--uninstall");
    // `waydlp-installer.exe uninstall --yes` (quiet uninstall from Apps & features).
    if uninstall && std::env::args().any(|a| a == "--yes") {
        let code = match Installer::new().and_then(|i| i.uninstall(true, Elevation::Pkexec, &|_| {})) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        };
        std::process::exit(code);
    }
    // `--self-test`: open the real window, draw a few frames, then exit 0.
    // CI uses it to prove the installer GUI starts on a clean machine.
    let self_test = std::env::args().any(|a| a == "--self-test");
    // wgpu first (works without modern OpenGL drivers), then OpenGL.
    match launch(eframe::Renderer::Wgpu, uninstall, self_test) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("wgpu renderer failed ({e}); retrying with OpenGL");
            launch(eframe::Renderer::Glow, uninstall, self_test)
        }
    }
}

fn launch(renderer: eframe::Renderer, uninstall: bool, self_test: bool) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(if uninstall { "Uninstall waydlp" } else { "waydlp setup" })
            .with_inner_size([760.0, 520.0])
            .with_resizable(false)
            .with_icon(icon()),
        renderer,
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "waydlp-setup",
        options,
        Box::new(move |cc| {
            waydlp_theme::apply(&cc.egui_ctx, waydlp_theme::ThemeOptions::default());
            let app = SetupApp::new(uninstall);
            if self_test { Ok(Box::new(SelfTest { app, frames: 0, renderer })) } else { Ok(Box::new(app)) }
        }),
    )
}

/// Wraps the real wizard for `--self-test`: renders it, reports, closes.
struct SelfTest {
    app: SetupApp,
    frames: u32,
    renderer: eframe::Renderer,
}

impl eframe::App for SelfTest {
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        eframe::App::ui(&mut self.app, ui, frame);
        self.frames += 1;
        if self.frames == 5 {
            println!(
                "self-test ok: renderer={:?}, embedded payload={}, installer init={}",
                self.renderer,
                self.app.payload.as_ref().map(|p| p.version.as_str()).unwrap_or("none"),
                self.app.init_error.as_deref().unwrap_or("ok"),
            );
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ui.ctx().request_repaint();
    }
}
