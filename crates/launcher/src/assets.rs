//! Embedded artwork.
//!
//! Baked into the binary with `include_bytes!` rather than shipped alongside it —
//! the whole reason for choosing egui was a launcher that is one self-contained
//! `.exe` with nothing to install and nothing to lose.

use egui::{Color32, ColorImage, TextureHandle, TextureOptions};

const LOGO_PNG: &[u8] = include_bytes!("../assets/logo.png");
const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

/// Icon for the live window — title bar, taskbar, Alt-Tab.
///
/// Distinct from the icon `build.rs` compiles into the PE resource table: that one is
/// what Explorer shows for the *file*, this one is what Windows shows for the
/// *running window*. Both are needed; neither substitutes for the other.
pub fn window_icon() -> egui::IconData {
    let img = image::load_from_memory(ICON_PNG)
        .expect("embedded icon.png is not a valid image")
        .to_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData { rgba: img.into_raw(), width, height }
}

/// Decodes the logo and keys its black background out to transparency.
///
/// The source art is bright line-work on solid black with no alpha channel, so
/// drawing it directly would put a black rectangle over the backdrop. Setting
/// `alpha = max(r, g, b)` and leaving RGB alone is the classic screen-blend key:
/// black pixels become fully transparent, bright pixels fully opaque, and the
/// logo's cyan glow fades out smoothly instead of ending at a hard edge.
///
/// The result is *premultiplied* by construction — every channel is `<= alpha` —
/// which is exactly what egui's texture upload expects.
fn decode_logo() -> ColorImage {
    let img = image::load_from_memory(LOGO_PNG)
        .expect("embedded logo.png is not a valid image")
        .to_rgb8();
    let (w, h) = img.dimensions();

    let pixels = img
        .pixels()
        .map(|p| {
            let [r, g, b] = p.0;
            let alpha = r.max(g).max(b);
            Color32::from_rgba_premultiplied(r, g, b, alpha)
        })
        .collect();

    ColorImage::new([w as usize, h as usize], pixels)
}

/// Exposed so the alpha-keying can be tested without an egui context (loading a
/// texture needs a live `Context`, which a unit test has no business creating).
#[cfg(test)]
pub fn decode_logo_for_test() -> ColorImage {
    decode_logo()
}

pub struct Assets {
    pub logo: TextureHandle,
}

impl Assets {
    pub fn load(ctx: &egui::Context) -> Self {
        // The logo is scaled well below its native 1408px width, so linear
        // filtering matters here — nearest would alias the fine gold lettering.
        let logo = ctx.load_texture("logo", decode_logo(), TextureOptions::LINEAR);
        Self { logo }
    }
}
