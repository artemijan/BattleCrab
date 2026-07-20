//! egui application shell.
//!
//! The window is undecorated and transparent so the frosted panels and rounded
//! corners are not framed by an opaque OS title bar — which means this file also
//! owns the title bar: dragging, minimise, and close.
//!
//! Structurally: the UI thread never blocks. All install work lives on a worker
//! thread, and every state the user can reach is reflected in [`LauncherApp::phase`].

use std::sync::mpsc;
use std::thread;

use egui::{Align, Color32, Layout, Pos2, Rect, Vec2, ViewportCommand};

use crate::assets::Assets;
use crate::config::Config;
use crate::install::{self, Cancel, InstallRequest};
use crate::launch::launch_game;
use crate::progress::{Phase, ProgressRx, Reporter};
use crate::theme::{self, palette};

/// Native aspect ratio of the logo art (1408x768).
const LOGO_ASPECT: f32 = 1408.0 / 768.0;
const LOGO_WIDTH: f32 = 380.0;
const TITLE_BAR_HEIGHT: f32 = 38.0;

/// The window is a fixed size, so this is shared with the render test — which
/// asserts the layout fits inside it.
pub const WINDOW_SIZE: [f32; 2] = [900.0, 520.0];

/// Fixed height of the glass panel's contents.
///
/// The window is a fixed size and cannot scroll, so the panel must not be
/// content-sized — a long error message would push its bottom edge past the bottom
/// of the window. Every state reserves the same space instead, which also stops the
/// layout jumping as an install progresses.
const PANEL_CONTENT_HEIGHT: f32 = 234.0;

/// One line, reserved whether or not there is anything to say.
const STATUS_HEIGHT: f32 = 20.0;

pub struct LauncherApp {
    config: Config,
    assets: Assets,
    /// `None` = idle. Otherwise the most recent report from the worker.
    phase: Option<Phase>,
    rx: Option<ProgressRx>,
    cancel: Cancel,
    /// Surfaced under the action button; cleared on the next successful action.
    status: Option<String>,
    /// Bottom edge of the glass panel as last laid out. Recorded so a test can
    /// assert the panel never grows past the bottom of the fixed-size window.
    panel_bottom: f32,
}

impl LauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_context(&cc.egui_ctx)
    }

    /// Construction from a bare [`egui::Context`], so the UI can be built by the
    /// headless render harness — `CreationContext` cannot be constructed outside
    /// eframe's own startup.
    pub fn with_context(ctx: &egui::Context) -> Self {
        theme::install(ctx);
        Self {
            config: Config::load(),
            assets: Assets::load(ctx),
            phase: None,
            rx: None,
            cancel: Cancel::default(),
            status: None,
            panel_bottom: 0.0,
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
            // A new folder is a different install; re-check rather than assuming the
            // previous state carries over.
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
    /// The window is transparent; the backdrop is painted by us, not cleared to a
    /// solid colour, so the rounded corners stay see-through.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    /// Runs before every repaint, including repaints the worker requests while the
    /// window is hidden — so progress keeps advancing when minimised.
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl LauncherApp {
    /// The whole composition. Separate from the `eframe::App` impl so the headless
    /// render test can drive it without an `eframe::Frame`.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        // The whole window, not `ui.max_rect()` — that is inset by the root margin,
        // which would leave an unpainted strip along the top edge showing through to
        // the transparent background.
        let screen = ctx.viewport_rect();
        theme::paint_backdrop(ui.painter(), screen);

        self.title_bar(ui, &ctx, screen);

        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(34, 8))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| self.logo(ui));
                ui.add_space(6.0);
                let panel =
                    theme::glass_group(ui, theme::PANEL_RADIUS, PANEL_CONTENT_HEIGHT, |ui| {
                        self.body(ui, &ctx)
                    });
                self.panel_bottom = panel.response.rect.bottom();
            });
    }

    /// Forces a state for the render test, which has no worker thread.
    #[cfg(test)]
    pub fn set_state_for_test(&mut self, phase: Option<Phase>, status: Option<String>) {
        self.phase = phase;
        self.status = status;
    }

    /// Bottom edge of the glass panel, for the overflow test.
    #[cfg(test)]
    pub fn panel_bottom(&self) -> f32 {
        self.panel_bottom
    }

    /// Undecorated windows have no OS title bar, so this provides the drag region
    /// and the window buttons.
    fn title_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, screen: Rect) {
        let bar = Rect::from_min_size(screen.min, Vec2::new(screen.width(), TITLE_BAR_HEIGHT));
        let response = ui.interact(
            bar,
            ui.id().with("title_bar"),
            egui::Sense::click_and_drag(),
        );
        if response.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        let mut bar_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(bar.shrink2(Vec2::new(12.0, 6.0)))
                .layout(Layout::right_to_left(Align::Center)),
        );
        // `×` (U+00D7) rather than `✕` (U+2715): the latter is not in egui's default
        // font and renders as a tofu box.
        if window_button(&mut bar_ui, "×", palette::DANGER).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if window_button(&mut bar_ui, "—", palette::TEXT_WEAK).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
    }

    fn logo(&mut self, ui: &mut egui::Ui) {
        let size = Vec2::new(LOGO_WIDTH, LOGO_WIDTH / LOGO_ASPECT);
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter().image(
            self.assets.logo.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    fn body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.install_dir_row(ui);
        ui.add_space(12.0);
        self.progress_section(ui);
        ui.add_space(14.0);

        ui.vertical_centered(|ui| {
            self.action_row(ui, ctx);
            ui.add_space(6.0);
            self.status_line(ui);
        });
    }

    /// Always occupies [`STATUS_HEIGHT`], message or not.
    ///
    /// The text is truncated to a single line: failures carry a full `anyhow`
    /// context chain ("install failed: requesting https://…: connect timed out"),
    /// which wraps to several lines and would otherwise overflow the panel. The
    /// whole message stays available on hover.
    fn status_line(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let Some(status) = self.status.clone() else {
            ui.allocate_exact_size(Vec2::new(width, STATUS_HEIGHT), egui::Sense::hover());
            return;
        };

        let colour = if matches!(self.phase, Some(Phase::Failed(_))) {
            palette::DANGER
        } else {
            palette::TEXT_WEAK
        };
        ui.add_sized(
            Vec2::new(width, STATUS_HEIGHT),
            egui::Label::new(egui::RichText::new(&status).size(13.0).color(colour)).truncate(),
        )
        .on_hover_text(&status);
    }

    fn install_dir_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("INSTALL FOLDER")
                    .size(11.0)
                    .color(palette::TEXT_WEAK),
            );
            // Right-aligned so the button keeps its position regardless of path length.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Changing the target mid-download would strand a partial install.
                if theme::ghost_button(ui, "Change…", !self.busy()).clicked() {
                    self.pick_install_dir();
                }
            });
        });
        ui.add_space(2.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(self.config.install_dir.display().to_string())
                    .monospace()
                    .size(13.0),
            )
            .truncate(),
        );
    }

    fn progress_section(&mut self, ui: &mut egui::Ui) {
        let Some(phase) = &self.phase else {
            // Keep the layout from jumping when an install starts.
            ui.add_space(theme::progress_section_height());
            return;
        };

        let (caption, detail) = match phase {
            Phase::CheckingManifest => ("Checking for updates".to_string(), String::new()),
            Phase::Downloading { done, total } => (
                "Downloading".to_string(),
                match total {
                    Some(t) => format!("{} / {}", human(*done), human(*t)),
                    None => human(*done),
                },
            ),
            Phase::Extracting { done, total } => (
                "Unpacking".to_string(),
                format!("{} / {}", human(*done), human(*total)),
            ),
            Phase::Ready => ("Ready to play".to_string(), String::new()),
            Phase::Failed(_) => ("Failed".to_string(), String::new()),
        };

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(caption).size(13.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(detail)
                        .size(13.0)
                        .monospace()
                        .color(palette::TEXT_WEAK),
                );
            });
        });
        ui.add_space(6.0);
        // A failed install must not animate — an indeterminate sweep reads as "still
        // working", which is the opposite of what happened. Show an empty track.
        let fraction = match phase {
            Phase::Failed(_) => Some(0.0),
            _ => phase.fraction(),
        };
        theme::glass_progress(ui, fraction);
    }

    fn action_row(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.busy() {
            if theme::ghost_button(ui, "Cancel", true).clicked() {
                self.cancel.cancel();
            }
            return;
        }

        if self.config.is_installed() {
            if theme::primary_button(ui, "Play").clicked() {
                self.play();
            }
            ui.add_space(6.0);
            if theme::ghost_button(ui, "Reinstall", true).clicked() {
                self.start_install(ctx);
            }
        } else {
            let label = if matches!(self.phase, Some(Phase::Failed(_))) {
                "Retry"
            } else {
                "Install"
            };
            if theme::primary_button(ui, label).clicked() {
                self.start_install(ctx);
            }
        }
    }
}

/// Small square title-bar button. Tinted on hover only, so the bar stays quiet.
fn window_button(ui: &mut egui::Ui, glyph: &str, hover_colour: Color32) -> egui::Response {
    let response = ui.add(
        egui::Button::new(egui::RichText::new(glyph).size(15.0))
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE)
            .min_size(Vec2::new(26.0, 26.0)),
    );
    if response.hovered() {
        ui.painter().rect_filled(
            response.rect,
            egui::CornerRadius::same(6),
            hover_colour.gamma_multiply(0.22),
        );
    }
    response
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
        assert_eq!(Phase::Extracting { done: 5, total: 10 }.fraction(), Some(0.5));
    }

    /// Rasterizes the full window headlessly so the composition can actually be
    /// inspected — a theme that compiles is not a theme that looks right.
    ///
    /// Ignored by default: it needs a GPU adapter, which CI may not have. Run with
    /// `cargo test -p launcher -- --ignored render_window` and look at the PNGs in
    /// `target/ui-render/`.
    #[test]
    #[ignore = "requires a GPU adapter; run manually to inspect the UI"]
    fn render_window() {
        let out = std::path::Path::new("target/ui-render");
        std::fs::create_dir_all(out).unwrap();

        for (name, phase, status) in states() {
            let mut app: Option<LauncherApp> = None;
            // `Cell` rather than a plain `f32`: the closure is held by the harness
            // for its whole lifetime, so a mutable capture would still be borrowed
            // when the assertion below wants to read it.
            let bottom = std::cell::Cell::new(0.0_f32);
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(WINDOW_SIZE[0], WINDOW_SIZE[1]))
                .renderer(egui_kittest::wgpu::WgpuTestRenderer::default())
                .build_ui(|ui| {
                    let app = app.get_or_insert_with(|| LauncherApp::with_context(ui.ctx()));
                    app.set_state_for_test(phase.clone(), status.clone());
                    app.draw(ui);
                    bottom.set(app.panel_bottom());
                });

            // `run()` waits for repaints to settle, which never happens while the
            // indeterminate bar is animating. A fixed number of steps is enough to
            // lay out and paint.
            harness.run_steps(3);
            let image = harness.render().expect("headless render failed");
            image.save(out.join(format!("{name}.png"))).unwrap();

            assert!(
                bottom.get() <= WINDOW_SIZE[1],
                "[{name}] panel bottom {} overflows the {}px window",
                bottom.get(),
                WINDOW_SIZE[1]
            );
        }
    }

    /// The states worth rendering. A *long* failure message is included on purpose:
    /// the panel used to be content-sized, so a full `anyhow` context chain grew it
    /// straight past the bottom of the fixed-size window. Rendering the failed state
    /// with no status message at all is what let that ship.
    #[allow(clippy::type_complexity)]
    fn states() -> Vec<(&'static str, Option<Phase>, Option<String>)> {
        vec![
            ("idle", None, None),
            (
                "downloading",
                Some(Phase::Downloading {
                    done: 3_400_000_000,
                    total: Some(9_300_000_000),
                }),
                None,
            ),
            (
                "unpacking",
                Some(Phase::Extracting { done: 7_000_000_000, total: 9_300_000_000 }),
                None,
            ),
            (
                "failed",
                Some(Phase::Failed(LONG_ERROR.into())),
                Some(LONG_ERROR.into()),
            ),
            (
                "ready",
                Some(Phase::Ready),
                Some("Installation complete.".into()),
            ),
        ]
    }

    /// Representative of what `install()` actually produces via `{e:#}`.
    const LONG_ERROR: &str = "install failed: requesting \
        https://pub-example.r2.dev/chunks/textures.tar.zst: error sending request \
        for url: connect timed out after 30s";

    /// The logo is keyed to transparency at load; a regression here would put a
    /// black box over the backdrop.
    #[test]
    fn logo_black_is_keyed_transparent() {
        let img = crate::assets::decode_logo_for_test();
        let [w, h] = img.size;

        // Corners of the source art are solid black.
        assert_eq!(
            img.pixels[0].a(),
            0,
            "top-left pixel should be fully transparent"
        );

        // Note the centre must be indexed as row-major (`y * w + x`) — `len / 2`
        // lands on column 0 of the middle row, which is background, not artwork.
        let centre = img.pixels[(h / 2) * w + w / 2];
        assert!(
            centre.a() > 0,
            "centre of the logo should be visible, got alpha {}",
            centre.a()
        );

        // Guard against the key eating the art wholesale: a meaningful share of the
        // image must survive.
        let visible = img.pixels.iter().filter(|p| p.a() > 16).count();
        let ratio = visible as f32 / img.pixels.len() as f32;
        assert!(ratio > 0.10, "only {:.1}% of the logo survived keying", ratio * 100.0);
    }
}
