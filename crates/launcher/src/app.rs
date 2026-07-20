//! egui application shell.
//!
//! Deliberately plain: this is a working skeleton, and the visual design is expected
//! to be specified separately. What is load-bearing here is the *structure* — the UI
//! thread never blocks, all install work lives on a worker, and every state the user
//! can reach is reflected in [`LauncherApp::phase`].

use std::sync::mpsc;
use std::thread;

use crate::config::Config;
use crate::install::{self, Cancel, InstallRequest};
use crate::launch::launch_game;
use crate::progress::{Phase, ProgressRx, Reporter};

pub struct LauncherApp {
    config: Config,
    /// `None` = idle. Otherwise the most recent report from the worker.
    phase: Option<Phase>,
    rx: Option<ProgressRx>,
    cancel: Cancel,
    /// Surfaced under the buttons; cleared on the next successful action.
    status: Option<String>,
}

impl LauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Slightly larger default text — the stock egui size is cramped for a
        // consumer-facing app.
        cc.egui_ctx.all_styles_mut(|s| {
            for font in s.text_styles.values_mut() {
                font.size *= 1.15;
            }
        });

        Self {
            config: Config::load(),
            phase: None,
            rx: None,
            cancel: Cancel::default(),
            status: None,
        }
    }

    fn busy(&self) -> bool {
        matches!(
            self.phase,
            Some(Phase::CheckingManifest | Phase::Downloading { .. } | Phase::Extracting { .. })
        )
    }

    fn start_install(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.cancel = Cancel::default();
        self.phase = Some(Phase::CheckingManifest);
        self.status = None;

        let reporter = Reporter::new(tx, Some(ctx.clone()));
        let req = InstallRequest {
            base_url: self.config.base_url.clone(),
            install_dir: self.config.install_dir.clone(),
            cancel: self.cancel.clone(),
        };
        thread::spawn(move || install::run(req, reporter));
    }

    /// Drains everything queued since the last frame; only the final message matters
    /// because each one is a full snapshot of worker state.
    fn poll_worker(&mut self) {
        let Some(rx) = &self.rx else { return };
        let mut latest = None;
        while let Ok(phase) = rx.try_recv() {
            latest = Some(phase);
        }
        let Some(phase) = latest else { return };

        match &phase {
            Phase::Ready => {
                // Record the install so the next launch skips straight to Play. The
                // version is filled in by the worker's manifest; until the update
                // flow lands, presence is what matters.
                self.config.installed_version = Some("installed".to_string());
                if let Err(e) = self.config.save() {
                    tracing::warn!("could not persist config: {e:#}");
                }
                self.rx = None;
                self.status = Some("Installation complete.".into());
            }
            Phase::Failed(msg) => {
                self.rx = None;
                self.status = Some(msg.clone());
            }
            _ => {}
        }
        self.phase = Some(phase);
    }

    fn pick_install_dir(&mut self) {
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("Choose install folder")
            .set_directory(&self.config.install_dir)
            .pick_folder()
        {
            self.config.install_dir = dir;
            // A new folder is a different install; re-check whether a client is
            // already there rather than assuming the previous state carries over.
            self.config.installed_version = None;
            if let Err(e) = self.config.save() {
                tracing::warn!("could not persist config: {e:#}");
            }
        }
    }

    fn play(&mut self) {
        match launch_game(&self.config.game_exe(), &self.config.server_ip) {
            Ok(()) => self.status = Some("Starting game…".into()),
            Err(e) => self.status = Some(format!("{e:#}")),
        }
    }
}

impl eframe::App for LauncherApp {
    /// Runs before every repaint, including repaints the worker thread requests
    /// while the window is hidden — so progress keeps advancing when minimised.
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("BattleCrab");
            ui.label(egui::RichText::new("Lineage II Interlude Classic").weak());
            ui.add_space(16.0);

            self.install_dir_row(ui);
            ui.add_space(12.0);
            self.progress_section(ui);
            ui.add_space(12.0);
            self.action_row(ui, &ctx);

            if let Some(status) = &self.status {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(status).weak());
            }
        });
    }
}

impl LauncherApp {
    fn install_dir_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Install folder:");
            // Truncating keeps a deep path from stretching the window.
            let shown = self.config.install_dir.display().to_string();
            ui.add(
                egui::Label::new(egui::RichText::new(shown).monospace())
                    .truncate(),
            );
        });
        ui.horizontal(|ui| {
            // Changing the target mid-download would strand a partial install.
            if ui
                .add_enabled(!self.busy(), egui::Button::new("Change folder…"))
                .clicked()
            {
                self.pick_install_dir();
            }
        });
    }

    fn progress_section(&mut self, ui: &mut egui::Ui) {
        let Some(phase) = &self.phase else {
            return;
        };

        let caption = match phase {
            Phase::CheckingManifest => "Checking for updates…".to_string(),
            Phase::Downloading { done, total } => match total {
                Some(t) => format!("Downloading — {} / {}", human(*done), human(*t)),
                None => format!("Downloading — {}", human(*done)),
            },
            Phase::Extracting { done, total } => {
                format!("Unpacking — {} / {}", human(*done), human(*total))
            }
            Phase::Ready => "Ready to play".to_string(),
            Phase::Failed(_) => "Failed".to_string(),
        };

        ui.label(caption);
        let mut bar = egui::ProgressBar::new(phase.fraction().unwrap_or(0.0)).desired_width(f32::INFINITY);
        if phase.fraction().is_none() && self.busy() {
            // Indeterminate: a server with no Content-Length, or the manifest fetch.
            bar = bar.animate(true);
        }
        ui.add(bar);
    }

    fn action_row(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            if self.busy() {
                if ui.button("Cancel").clicked() {
                    self.cancel.cancel();
                }
                return;
            }

            if self.config.is_installed() {
                if ui
                    .add(egui::Button::new(egui::RichText::new("Play").strong()))
                    .clicked()
                {
                    self.play();
                }
                if ui.button("Reinstall").clicked() {
                    self.start_install(ctx);
                }
            } else {
                let label = if matches!(self.phase, Some(Phase::Failed(_))) {
                    "Retry"
                } else {
                    "Install"
                };
                if ui
                    .add(egui::Button::new(egui::RichText::new(label).strong()))
                    .clicked()
                {
                    self.start_install(ctx);
                }
            }
        });
    }
}

/// Byte count in the units a player expects to see.
fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_readable_sizes() {
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1024), "1.0 KB");
        assert_eq!(human(9_300_000_000), "8.7 GB");
    }

    #[test]
    fn fraction_is_none_when_total_unknown() {
        assert!(Phase::Downloading { done: 10, total: None }.fraction().is_none());
        assert_eq!(
            Phase::Extracting { done: 5, total: 10 }.fraction(),
            Some(0.5)
        );
    }
}
