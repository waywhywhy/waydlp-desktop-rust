//! One module per page. Each adds an `impl WaydlpApp` block with a
//! `page_*` method so pages share the app state without extra plumbing.

mod about;
mod downloader;
mod history;
mod home;
mod queue;
mod settings;
mod sites;
