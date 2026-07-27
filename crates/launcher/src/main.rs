// Suppress the console window in release builds — a launcher that opens a terminal
// alongside itself looks broken to players. Debug builds keep it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod config;
mod install;
mod instance;
mod launch;
mod progress;
mod relocate;
mod theme;

use app::LauncherApp;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "launcher=info".into()),
        )
        .init();

    // Two launchers racing each other over one install directory would corrupt it,
    // so only the first instance runs. The lock is held for the whole of `main`
    // and the OS releases it however the process ends.
    let Some(_instance_lock) = instance::acquire() else {
        // A silent exit would read as "the launcher is broken" to whoever
        // double-clicked; say why nothing opened.
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Info)
            .set_title("BattleCrab Launcher")
            .set_description("The launcher is already running — check your taskbar.")
            .show();
        return Ok(());
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(app::WINDOW_SIZE)
            .with_icon(assets::window_icon())
            // Undecorated and transparent so the frosted panels and rounded corners
            // are not framed by an opaque OS title bar. The cost is that we own the
            // title bar — drag, minimise and close live in `app::title_bar`.
            .with_decorations(false)
            .with_transparent(true)
            // A launcher has one composition; resizing only stretches empty space,
            // and a fixed size avoids hand-rolling resize handles on an undecorated
            // window.
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        "BattleCrab Launcher",
        options,
        Box::new(|cc| Ok(Box::new(LauncherApp::new(cc)))),
    )
}
