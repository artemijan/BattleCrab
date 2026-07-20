//! Visual language: palette, backdrop, and the frosted-glass surfaces.
//!
//! ## On "glass"
//!
//! egui has no backdrop blur — there is no way to sample what is behind a widget
//! and blur it. Real glassmorphism is therefore not achievable directly, so this
//! module fakes it the way it is faked in any renderer without a blur pass:
//!
//! 1. The backdrop is a **smooth gradient plus soft radial glows**. Blur only
//!    visibly changes high-frequency content; against a low-frequency background,
//!    plain translucency is indistinguishable from a blurred one.
//! 2. Panels are translucent, with a **brighter top edge and dimmer bottom** —
//!    the specular gradient is what actually reads as "pane of glass" to the eye,
//!    more than the transparency does.
//! 3. A hairline light stroke gives the pane a lit rim.
//!
//! Windows 11 can do the real thing via `DwmSetWindowAttribute`
//! (`DWMSBT_ACRYLICWINDOW` / Mica), which blurs the actual desktop behind the
//! window. That needs the `windows` crate and a raw window handle, and cannot be
//! tested from macOS — left as a future upgrade. What is here is platform-neutral
//! and self-contained.

use egui::{Color32, CornerRadius, Mesh, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

/// Colours sampled from the BattleCrab logo so the shell and the art agree.
pub mod palette {
    use egui::Color32;

    /// Backdrop gradient, top to bottom.
    pub const BG_TOP: Color32 = Color32::from_rgb(0x0E, 0x1B, 0x2B);
    pub const BG_BOTTOM: Color32 = Color32::from_rgb(0x04, 0x08, 0x0F);

    /// The cyan rim-light that surrounds the crab in the logo.
    pub const GLOW: Color32 = Color32::from_rgb(0x4F, 0xC8, 0xF0);
    /// The gold of the lettering and the shield.
    pub const GOLD: Color32 = Color32::from_rgb(0xE8, 0xB5, 0x4A);
    pub const GOLD_DIM: Color32 = Color32::from_rgb(0x9A, 0x76, 0x2C);

    pub const TEXT: Color32 = Color32::from_rgb(0xDE, 0xE8, 0xF2);
    pub const TEXT_WEAK: Color32 = Color32::from_rgb(0x8C, 0x9E, 0xB2);
    pub const DANGER: Color32 = Color32::from_rgb(0xE0, 0x6C, 0x6C);
}

pub const PANEL_RADIUS: u8 = 14;
pub const WINDOW_RADIUS: u8 = 16;

/// Applies the dark, low-contrast base style the glass surfaces sit on.
pub fn install(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        for font in style.text_styles.values_mut() {
            font.size *= 1.15;
        }

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = Some(palette::TEXT);
        v.panel_fill = Color32::TRANSPARENT;
        v.window_fill = Color32::TRANSPARENT;
        // Widgets draw their own glass backgrounds; the default opaque greys would
        // punch holes in it.
        v.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
        v.widgets.noninteractive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_fill = glass_fill(18);
        v.widgets.inactive.weak_bg_fill = glass_fill(18);
        v.widgets.hovered.bg_fill = glass_fill(34);
        v.widgets.hovered.weak_bg_fill = glass_fill(34);
        v.widgets.active.bg_fill = glass_fill(46);
        v.widgets.active.weak_bg_fill = glass_fill(46);

        v.widgets.inactive.bg_stroke = Stroke::new(1.0, glass_edge(40));
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, glass_edge(90));
        v.widgets.active.bg_stroke = Stroke::new(1.0, glass_edge(120));

        for w in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.noninteractive,
        ] {
            w.corner_radius = CornerRadius::same(8);
        }

        v.selection.bg_fill = palette::GLOW.gamma_multiply(0.35);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    });
}

/// Translucent white with a cool tint — the body of a glass pane.
pub fn glass_fill(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(0x9E, 0xC6, 0xE6, alpha)
}

/// The lit rim of a glass pane.
pub fn glass_edge(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(0xCF, 0xE6, 0xFA, alpha)
}

/// Paints the window backdrop: vertical gradient plus two soft glows that give the
/// translucent panels something with depth to sit against.
pub fn paint_backdrop(painter: &egui::Painter, rect: Rect) {
    // Rounded, because the window is undecorated — the corners outside this shape
    // stay fully transparent.
    painter.rect_filled(rect, CornerRadius::same(WINDOW_RADIUS), palette::BG_BOTTOM);

    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), palette::BG_TOP);
    mesh.colored_vertex(rect.right_top(), palette::BG_TOP);
    mesh.colored_vertex(rect.right_bottom(), palette::BG_BOTTOM);
    mesh.colored_vertex(rect.left_bottom(), palette::BG_BOTTOM);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));

    // Behind where the logo sits, echoing its own cyan rim-light.
    paint_glow(
        painter,
        rect.center_top() + Vec2::new(0.0, rect.height() * 0.24),
        rect.width() * 0.46,
        palette::GLOW.gamma_multiply(0.22),
    );
    // A warmer, weaker one low-left to keep the composition from being symmetrical.
    paint_glow(
        painter,
        rect.left_bottom() + Vec2::new(rect.width() * 0.22, -rect.height() * 0.12),
        rect.width() * 0.34,
        palette::GOLD.gamma_multiply(0.10),
    );
}

/// A soft radial falloff, drawn as a triangle fan from an opaque centre to a
/// transparent rim. Cheaper and smoother than stacking translucent circles.
fn paint_glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
    const SEGMENTS: usize = 64;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    let edge = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0);
    for i in 0..=SEGMENTS {
        let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(
            center + Vec2::new(angle.cos(), angle.sin()) * radius,
            edge,
        );
    }
    for i in 1..=SEGMENTS as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    painter.add(Shape::mesh(mesh));
}

/// A frosted pane, as a single composed shape: fill, specular sheen along the top
/// edge, and a lit rim. The sheen is what sells it as glass — a flat translucent
/// box with no gradient just reads as a grey rectangle.
pub fn glass_panel_shape(rect: Rect, radius: u8) -> Shape {
    let cr = CornerRadius::same(radius);

    let sheen_height = (rect.height() * 0.45).min(46.0);
    let sheen = Rect::from_min_size(rect.min, Vec2::new(rect.width(), sheen_height));
    let mut mesh = Mesh::default();
    mesh.colored_vertex(sheen.left_top(), glass_fill(26));
    mesh.colored_vertex(sheen.right_top(), glass_fill(26));
    mesh.colored_vertex(sheen.right_bottom(), glass_fill(0));
    mesh.colored_vertex(sheen.left_bottom(), glass_fill(0));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);

    Shape::Vec(vec![
        Shape::rect_filled(rect, cr, glass_fill(16)),
        Shape::mesh(mesh),
        Shape::rect_stroke(rect, cr, Stroke::new(1.0, glass_edge(46)), StrokeKind::Inside),
    ])
}

/// Lays out `add_contents` inside a frosted pane.
///
/// The pane cannot be measured until its contents are laid out, but it has to be
/// painted *behind* them. So a slot in the paint list is reserved up front and
/// filled in once the final rect is known — the standard egui idiom for
/// content-sized backgrounds.
pub fn glass_group<R>(
    ui: &mut egui::Ui,
    radius: u8,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let bg = ui.painter().add(Shape::Noop);
    let inner = egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, add_contents);
    ui.painter()
        .set(bg, glass_panel_shape(inner.response.rect, radius));
    inner.inner
}

const BAR_HEIGHT: f32 = 10.0;

/// Height the progress block occupies, reserved even when idle so the window does
/// not resize the instant an install starts.
pub fn progress_section_height() -> f32 {
    // caption line + gap + bar
    18.0 + 6.0 + BAR_HEIGHT
}

/// Progress bar in the same glass idiom: a recessed translucent track with a
/// glowing cyan-to-gold fill.
///
/// `fraction` of `None` means indeterminate — a band sweeps the track instead,
/// which is honest about not knowing the total rather than faking a position.
pub fn glass_progress(ui: &mut egui::Ui, fraction: Option<f32>) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), BAR_HEIGHT),
        egui::Sense::hover(),
    );
    let cr = CornerRadius::same((BAR_HEIGHT / 2.0) as u8);
    let painter = ui.painter();

    painter.rect_filled(rect, cr, Color32::from_rgba_unmultiplied(0, 0, 0, 90));
    painter.rect_stroke(rect, cr, Stroke::new(1.0, glass_edge(30)), StrokeKind::Inside);

    let fill_rect = match fraction {
        Some(f) => {
            let f = f.clamp(0.0, 1.0);
            if f <= f32::EPSILON {
                return;
            }
            Rect::from_min_size(rect.min, Vec2::new(rect.width() * f, rect.height()))
        }
        None => {
            // Sweep a band roughly a fifth of the track wide, looping every 1.6s.
            let t = ui.input(|i| i.time) as f32;
            let period = 1.6;
            let phase = (t % period) / period;
            let band = rect.width() * 0.22;
            // Travel from fully off the left to fully off the right, then clip.
            let x = rect.left() - band + phase * (rect.width() + band * 2.0);
            let band_rect =
                Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(band, rect.height()));
            ui.ctx().request_repaint();
            band_rect.intersect(rect)
        }
    };

    if fill_rect.width() <= 0.0 {
        return;
    }

    painter.rect_filled(fill_rect, cr, palette::GLOW);

    // Warm the leading end towards gold. Inset by the corner radius so the sharp
    // mesh cannot spill outside the rounded fill.
    let inset = BAR_HEIGHT / 2.0;
    if fill_rect.width() > BAR_HEIGHT {
        let grad = Rect::from_min_max(
            Pos2::new(fill_rect.left() + inset, fill_rect.top()),
            Pos2::new(fill_rect.right() - inset, fill_rect.bottom()),
        );
        let mut mesh = Mesh::default();
        let clear = Color32::from_rgba_unmultiplied(
            palette::GOLD.r(),
            palette::GOLD.g(),
            palette::GOLD.b(),
            0,
        );
        mesh.colored_vertex(grad.left_top(), clear);
        mesh.colored_vertex(grad.right_top(), palette::GOLD);
        mesh.colored_vertex(grad.right_bottom(), palette::GOLD);
        mesh.colored_vertex(grad.left_bottom(), clear);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(mesh));
    }
}

/// The one prominent call to action — Play, or Install. Gold, lit, and larger than
/// anything else on screen so there is never a question of what to click.
pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let text = egui::RichText::new(label)
        .size(19.0)
        .strong()
        .color(Color32::from_rgb(0x1A, 0x12, 0x04));
    ui.add_sized(
        Vec2::new(190.0, 46.0),
        egui::Button::new(text)
            .fill(palette::GOLD)
            .stroke(Stroke::new(1.0, palette::GOLD_DIM))
            .corner_radius(CornerRadius::same(10)),
    )
}

/// A quieter secondary action that sits on the glass without competing with it.
pub fn ghost_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).color(palette::TEXT))
            .fill(glass_fill(20))
            .stroke(Stroke::new(1.0, glass_edge(50)))
            .corner_radius(CornerRadius::same(8)),
    )
}
