//! waydlp desktop application.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod icon;
mod notify;
mod pages;
mod widgets;

use waydlp_config::SettingsStore;
use waydlp_platform::AppPaths;

/// Build the native options for one renderer.
fn options(paths: &AppPaths, renderer: eframe::Renderer) -> eframe::NativeOptions {
    let viewport = egui::ViewportBuilder::default()
        .with_title("waydlp")
        .with_app_id("io.github.waywhywhy.waydlp")
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([860.0, 560.0])
        .with_icon(icon::app_icon(256));
    eframe::NativeOptions {
        viewport,
        renderer,
        persist_window: true,
        // Keep window state with the rest of waydlp's data (also in portable mode).
        persistence_path: Some(paths.data_dir.join("window-state.ron")),
        ..Default::default()
    }
}

/// Start the UI with `renderer`. Everything the app owns is created inside the
/// creator closure so a failed attempt can simply be retried.
fn launch(renderer: eframe::Renderer, startup_warning: Option<String>) -> eframe::Result {
    let paths = AppPaths::resolve();
    let opts = options(&paths, renderer);
    eframe::run_native(
        "waydlp",
        opts,
        Box::new(move |cc| {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .thread_name("waydlp-worker")
                .build()?;
            let store = SettingsStore::new(paths.settings_file());
            let (settings, warning) = store.load();
            let warning = warning.or(startup_warning);
            Ok(Box::new(app::WaydlpApp::new(cc, rt, paths, store, settings, warning)))
        }),
    )
}

fn main() -> eframe::Result {
    let paths = AppPaths::resolve();
    let paths_error = paths.ensure().err();
    let (settings, _) = SettingsStore::new(paths.settings_file()).load();
    let _log_guard = waydlp_platform::init_logging(&paths.log_dir, settings.advanced.debug_logging);
    tracing::info!(version = waydlp_core::VERSION, portable = paths.portable, "starting waydlp");
    let warning = paths_error.map(|e| format!("Could not create waydlp's data folders: {e}"));

    // wgpu (Direct3D 12 / Vulkan / Metal / GL) works on most systems, including
    // machines without modern OpenGL drivers. If it can't start, fall back to OpenGL.
    match launch(eframe::Renderer::Wgpu, warning.clone()) {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::warn!(error = %e, "wgpu renderer failed; retrying with OpenGL");
            launch(eframe::Renderer::Glow, warning)
        }
    }
}
