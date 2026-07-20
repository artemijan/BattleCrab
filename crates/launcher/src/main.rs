// Suppress the console window in release builds — a launcher that opens a terminal
// alongside itself looks broken to players. Debug builds keep it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod install;
mod launch;
mod manifest;
mod progress;

use app::LauncherApp;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "launcher=info".into()),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 420.0])
            .with_min_inner_size([560.0, 360.0])
            // A launcher has one fixed layout; letting it be maximised only creates
            // empty space. Revisit if the design gains a news panel.
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "BattleCrab Launcher",
        options,
        Box::new(|cc| Ok(Box::new(LauncherApp::new(cc)))),
    )
}
