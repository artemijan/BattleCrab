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
        v.panel_fill = egui::Color32::TRANSPARENT;
        v.window_fill = egui::Color32::TRANSPARENT;
        // Widgets draw their own glass backgrounds; the default opaque greys would
        // punch holes in it.
        v.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
        v.widgets.noninteractive.weak_bg_fill = egui::Color32::TRANSPARENT;
        v.widgets.inactive.bg_fill = glass_fill(18);
        v.widgets.inactive.weak_bg_fill = glass_fill(18);
        v.widgets.hovered.bg_fill = glass_fill(34);
        v.widgets.hovered.weak_bg_fill = glass_fill(34);
        v.widgets.active.bg_fill = glass_fill(46);
        v.widgets.active.weak_bg_fill = glass_fill(46);

        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, glass_edge(40));
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, glass_edge(90));
        v.widgets.active.bg_stroke = egui::Stroke::new(1.0, glass_edge(120));

        for w in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.noninteractive,
        ] {
            w.corner_radius = egui::CornerRadius::same(8);
        }

        v.selection.bg_fill = palette::GLOW.gamma_multiply(0.35);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    });
}

/// Translucent white with a cool tint — the body of a glass pane.
pub fn glass_fill(alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(0x9E, 0xC6, 0xE6, alpha)
}

/// The lit rim of a glass pane.
pub fn glass_edge(alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(0xCF, 0xE6, 0xFA, alpha)
}

/// Pre-rendered gradients, uploaded once at startup.
///
/// ## Why textures rather than meshes
///
/// A gradient drawn as a `Mesh` is a **sharp-cornered quad**. Painted over a rounded
/// rect it spills past the rounding, leaving a few stray semi-transparent pixels at
/// each corner — visible as a smudge or halo just outside the curve. The radial glows
/// had the same problem against the window edge, only worse, since a circle centred
/// near an edge extends beyond it.
///
/// epaint can fill a *rounded* rect from a texture (`RectShape::with_texture`), and a
/// rounded rect is clipped to its own rounding. So the gradients are baked into small
/// textures and drawn as single rounded rects, which cannot spill by construction.
#[derive(Clone)]
pub struct Surfaces {
    /// The whole window background: vertical gradient plus both glows, baked in.
    backdrop: egui::TextureHandle,
    /// One column: the glass fill with its specular sheen fading down from the top.
    glass: egui::TextureHandle,
}

impl Surfaces {
    pub fn load(ctx: &egui::Context) -> Self {
        Self {
            backdrop: ctx.load_texture("backdrop", backdrop_image(), egui::TextureOptions::LINEAR),
            glass: ctx.load_texture("glass", glass_image(), egui::TextureOptions::LINEAR),
        }
    }
}

/// Half the window's resolution. Everything here is low-frequency, so linear
/// upscaling is invisible and this keeps the upload trivial.
const BACKDROP_W: usize = 450;
const BACKDROP_H: usize = 260;

/// Bakes the vertical gradient and both radial glows into one opaque image.
fn backdrop_image() -> egui::ColorImage {
    let (w, h) = (BACKDROP_W as f32, BACKDROP_H as f32);

    // Behind the logo, echoing its own cyan rim-light; and a warmer, weaker one
    // low-left so the composition is not symmetrical.
    let glows = [
        (w * 0.5, h * 0.24, w * 0.46, palette::GLOW, 0.22_f32),
        (w * 0.22, h * 0.88, w * 0.34, palette::GOLD, 0.10),
    ];

    let mut pixels = Vec::with_capacity(BACKDROP_W * BACKDROP_H);
    for y in 0..BACKDROP_H {
        let t = y as f32 / (h - 1.0);
        for x in 0..BACKDROP_W {
            let mut rgb = lerp_rgb(palette::BG_TOP, palette::BG_BOTTOM, t);
            for (cx, cy, radius, colour, intensity) in glows {
                let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                if d < radius {
                    // Squared falloff: a linear one leaves a visible rim where the
                    // glow ends.
                    let f = (1.0 - d / radius).powi(2) * intensity;
                    rgb = add_rgb(rgb, colour, f);
                }
            }
            pixels.push(egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
        }
    }
    egui::ColorImage::new([BACKDROP_W, BACKDROP_H], pixels)
}

/// Fraction of a panel's height over which the sheen fades out.
const SHEEN_FRACTION: f32 = 0.18;
const GLASS_H: usize = 256;

/// One column of glass: base translucency plus the sheen fading down from the top.
///
/// Two pixels wide because a single column can sample oddly at the horizontal edges
/// under linear filtering.
fn glass_image() -> egui::ColorImage {
    let mut pixels = Vec::with_capacity(2 * GLASS_H);
    for y in 0..GLASS_H {
        let t = y as f32 / (GLASS_H as f32 - 1.0);
        let sheen = (1.0 - (t / SHEEN_FRACTION)).max(0.0);
        let alpha = 16.0 + 26.0 * sheen;
        // Premultiplied, which is what the texture upload expects.
        let a = alpha.round() as u8;
        let c = egui::Color32::from_rgba_unmultiplied(0x9E, 0xC6, 0xE6, a);
        let premul = egui::Color32::from_rgba_premultiplied(
            (c.r() as u16 * a as u16 / 255) as u8,
            (c.g() as u16 * a as u16 / 255) as u8,
            (c.b() as u16 * a as u16 / 255) as u8,
            a,
        );
        pixels.push(premul);
        pixels.push(premul);
    }
    egui::ColorImage::new([2, GLASS_H], pixels)
}

fn lerp_rgb(a: egui::Color32, b: egui::Color32, t: f32) -> [u8; 3] {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b())]
}

fn add_rgb(base: [u8; 3], glow: egui::Color32, amount: f32) -> [u8; 3] {
    let f = |b: u8, g: u8| (b as f32 + g as f32 * amount).min(255.0) as u8;
    [
        f(base[0], glow.r()),
        f(base[1], glow.g()),
        f(base[2], glow.b()),
    ]
}

/// Full texture, as `RectShape::with_texture` expects.
fn full_uv() -> egui::Rect {
    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0))
}

/// Paints the window backdrop as one rounded, texture-filled rect.
///
/// Rounded because the window is undecorated — outside this shape the window is fully
/// transparent, and nothing may bleed past the curve.
pub fn paint_backdrop(painter: &egui::Painter, rect: egui::Rect, surfaces: &Surfaces) {
    painter.add(egui::Shape::Rect(
        egui::epaint::RectShape::filled(
            rect,
            egui::CornerRadius::same(WINDOW_RADIUS),
            egui::Color32::WHITE,
        )
        .with_texture(surfaces.backdrop.id(), full_uv()),
    ));
}

/// A frosted pane: texture-filled rounded rect plus a lit rim.
///
/// The sheen is what sells it as glass — a flat translucent box with no gradient just
/// reads as a grey rectangle — and baking it into the texture keeps it inside the
/// rounded corners.
pub fn glass_panel_shape(rect: egui::Rect, radius: u8, surfaces: &Surfaces) -> egui::Shape {
    let cr = egui::CornerRadius::same(radius);
    egui::Shape::Vec(vec![
        egui::Shape::Rect(
            egui::epaint::RectShape::filled(rect, cr, egui::Color32::WHITE)
                .with_texture(surfaces.glass.id(), full_uv()),
        ),
        egui::Shape::rect_stroke(
            rect,
            cr,
            egui::Stroke::new(1.0, glass_edge(46)),
            egui::StrokeKind::Inside,
        ),
    ])
}

/// Lays out `add_contents` inside a frosted pane of fixed content height.
///
/// The pane cannot be measured until its contents are laid out, but it has to be
/// painted *behind* them. So a slot in the paint list is reserved up front and
/// filled in once the final rect is known — the standard egui idiom for
/// content-sized backgrounds.
///
/// `content_height` is pinned rather than left to the content because the window is
/// a fixed size: a pane that grows with its contents — a long error message, say —
/// grows straight past the bottom edge of the window, which cannot scroll or
/// resize. Callers must keep their contents within it.
/// Returns the pane's rect alongside the inner value so callers can assert it stays
/// within the window.
pub fn glass_group<R>(
    ui: &mut egui::Ui,
    radius: u8,
    content_height: f32,
    surfaces: &Surfaces,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let bg = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_min_height(content_height);
            add_contents(ui)
        });
    ui.painter()
        .set(bg, glass_panel_shape(inner.response.rect, radius, surfaces));
    inner
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
        egui::Vec2::new(ui.available_width(), BAR_HEIGHT),
        egui::Sense::hover(),
    );
    let cr = egui::CornerRadius::same((BAR_HEIGHT / 2.0) as u8);
    let painter = ui.painter();

    painter.rect_filled(rect, cr, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 90));
    painter.rect_stroke(
        rect,
        cr,
        egui::Stroke::new(1.0, glass_edge(30)),
        egui::StrokeKind::Inside,
    );

    let fill_rect = match fraction {
        Some(f) => {
            let f = f.clamp(0.0, 1.0);
            if f <= f32::EPSILON {
                return;
            }
            egui::Rect::from_min_size(rect.min, egui::Vec2::new(rect.width() * f, rect.height()))
        }
        None => {
            // Sweep a band roughly a fifth of the track wide, looping every 1.6s.
            let t = ui.input(|i| i.time) as f32;
            let period = 1.6;
            let phase = (t % period) / period;
            let band = rect.width() * 0.22;
            // Travel from fully off the left to fully off the right, then clip.
            let x = rect.left() - band + phase * (rect.width() + band * 2.0);
            let band_rect = egui::Rect::from_min_size(
                egui::Pos2::new(x, rect.top()),
                egui::Vec2::new(band, rect.height()),
            );
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
        let grad = egui::Rect::from_min_max(
            egui::Pos2::new(fill_rect.left() + inset, fill_rect.top()),
            egui::Pos2::new(fill_rect.right() - inset, fill_rect.bottom()),
        );
        let mut mesh = egui::Mesh::default();
        let clear = egui::Color32::from_rgba_unmultiplied(
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
        painter.add(egui::Shape::mesh(mesh));
    }
}

/// How long the hover tint eases in and out.
const HOVER_ANIM_SECS: f32 = 0.15;
/// How long the press squish takes — fast, so the button feels attached to the
/// mouse rather than lagging behind it.
const PRESS_ANIM_SECS: f32 = 0.08;
/// Fraction of its size a fully pressed button shrinks toward its centre.
const PRESS_SQUISH: f32 = 0.045;
/// Lifetime of the ring that ripples outward on click.
const FLASH_SECS: f64 = 0.4;

/// Everything that distinguishes one button family from another; the motion is
/// shared by all of them in [`animated_button`].
struct ButtonLook {
    fill: egui::Color32,
    fill_hovered: egui::Color32,
    stroke: egui::Color32,
    stroke_hovered: egui::Color32,
    /// Applied to the label explicitly — the style's `override_text_color` would
    /// otherwise be baked into the galley, defeating the disabled dimming.
    text: egui::Color32,
    radius: u8,
    /// Colour of the click ripple.
    flash: egui::Color32,
}

/// The one prominent call to action — Play, or Install. Gold, lit, and larger than
/// anything else on screen so there is never a question of what to click.
pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let text = egui::RichText::new(label).size(19.0).strong();
    let look = ButtonLook {
        fill: palette::GOLD,
        fill_hovered: mix(palette::GOLD, egui::Color32::WHITE, 0.16),
        stroke: palette::GOLD_DIM,
        stroke_hovered: palette::GOLD,
        text: egui::Color32::from_rgb(0x1A, 0x12, 0x04),
        radius: 10,
        flash: palette::GOLD,
    };
    animated_button(ui, Some(egui::Vec2::new(190.0, 46.0)), text, &look, true)
}

/// A quieter secondary action that sits on the glass without competing with it.
pub fn ghost_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    animated_button(ui, None, egui::RichText::new(label), &ghost_look(), enabled)
}

/// [`ghost_button`] at an explicit size, for rows that must line up.
pub fn ghost_button_sized(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    size: egui::Vec2,
) -> egui::Response {
    animated_button(
        ui,
        Some(size),
        egui::RichText::new(label),
        &ghost_look(),
        enabled,
    )
}

fn ghost_look() -> ButtonLook {
    ButtonLook {
        fill: glass_fill(20),
        fill_hovered: glass_fill(40),
        stroke: glass_edge(50),
        stroke_hovered: glass_edge(120),
        text: palette::TEXT,
        radius: 8,
        flash: palette::GLOW,
    }
}

/// A button that moves: the hover tint eases in and out, the whole button sinks
/// toward its centre while held, and a ring ripples outward on click.
///
/// Hand-rolled rather than styling `egui::Button` because a stock widget paints
/// itself during `add` — its visuals can only be the discrete hovered/active
/// states, never a value partway through an animation. Allocating the rect first
/// and painting after the interaction is what lets the visuals follow the
/// animated state.
///
/// `size` of `None` sizes the button to its label plus the style's button padding.
fn animated_button(
    ui: &mut egui::Ui,
    size: Option<egui::Vec2>,
    text: egui::RichText,
    look: &ButtonLook,
    enabled: bool,
) -> egui::Response {
    let text_colour = if enabled {
        look.text
    } else {
        look.text.gamma_multiply(0.45)
    };
    let galley = egui::WidgetText::from(text.color(text_colour)).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let size = size.unwrap_or_else(|| galley.size() + ui.spacing().button_padding * 2.0);
    // A disabled button must not eat clicks — sense hover only, so `clicked()`
    // can never fire while greyed out.
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, galley.text())
    });

    let ctx = ui.ctx().clone();
    // `animate_bool_with_time` owns the repaint scheduling for both of these.
    let hover = ctx.animate_bool_with_time(
        response.id.with("hover"),
        enabled && response.hovered(),
        HOVER_ANIM_SECS,
    );
    let press = ctx.animate_bool_with_time(
        response.id.with("press"),
        enabled && response.is_pointer_button_down_on(),
        PRESS_ANIM_SECS,
    );

    let flash_id = response.id.with("flash");
    let now = ui.input(|i| i.time);
    if response.clicked() {
        ctx.data_mut(|d| d.insert_temp(flash_id, now));
    }

    if ui.is_rect_visible(rect) {
        let rect =
            egui::Rect::from_center_size(rect.center(), rect.size() * (1.0 - PRESS_SQUISH * press));
        let cr = egui::CornerRadius::same(look.radius);

        let mut fill = mix(look.fill, look.fill_hovered, hover);
        let mut stroke = mix(look.stroke, look.stroke_hovered, hover);
        if !enabled {
            fill = fill.gamma_multiply(0.45);
            stroke = stroke.gamma_multiply(0.45);
        }

        let painter = ui.painter();
        painter.rect_filled(rect, cr, fill);
        painter.rect_stroke(
            rect,
            cr,
            egui::Stroke::new(1.0, stroke),
            egui::StrokeKind::Inside,
        );

        // The click ripple: an expanding, fading ring. Driven off the clock rather
        // than the press animation, so it plays out in full even though the press
        // that caused it may last a single frame.
        if let Some(clicked_at) = ctx.data(|d| d.get_temp::<f64>(flash_id)) {
            let t = ((now - clicked_at) / FLASH_SECS) as f32;
            if (0.0..1.0).contains(&t) {
                // Ease-out: the ring leaps away and coasts, like a ripple does.
                let ease = 1.0 - (1.0 - t) * (1.0 - t);
                let ring = rect.expand(1.0 + 6.0 * ease);
                painter.rect_stroke(
                    ring,
                    egui::CornerRadius::same(look.radius.saturating_add(3)),
                    egui::Stroke::new(2.0, look.flash.gamma_multiply(0.7 * (1.0 - t))),
                    egui::StrokeKind::Outside,
                );
                ctx.request_repaint();
            } else {
                ctx.data_mut(|d| d.remove::<f64>(flash_id));
            }
        }

        painter.galley(rect.center() - galley.size() * 0.5, galley, text_colour);
    }
    response
}

/// Linear blend in premultiplied space — correct for the translucent glass
/// colours, whose alpha changes between the resting and hovered states.
fn mix(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    egui::Color32::from_rgba_premultiplied(
        f(a.r(), b.r()),
        f(a.g(), b.g()),
        f(a.b(), b.b()),
        f(a.a(), b.a()),
    )
}

/// Lays out a horizontal row of known width, centred in the available space.
///
/// `ui.vertical_centered` centres each child by its own width, which a `horizontal`
/// block defeats by expanding to fill — so the padding is computed explicitly.
pub fn centered_row<R>(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let pad = ((ui.available_width() - width) * 0.5).max(0.0);
        ui.add_space(pad);
        add(ui)
    })
    .inner
}
