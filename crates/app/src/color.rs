//! Conversions between niri's CSS color strings and egui colors.

use egui::Color32;

/// Parses anything niri accepts (`#rgb`, `#rrggbbaa`, `rgb(…)`, named colors…).
pub fn parse(s: &str) -> Option<Color32> {
    let c = csscolorparser::parse(s).ok()?;
    let [r, g, b, a] = c.to_rgba8();
    Some(Color32::from_rgba_unmultiplied(r, g, b, a))
}

/// Formats as `#rrggbb`, or `#rrggbbaa` when not opaque.
pub fn format(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

pub fn from_niri(c: niri_config::Color) -> Color32 {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(f(c.r), f(c.g), f(c.b), f(c.a))
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let (a, b) = (a.to_srgba_unmultiplied(), b.to_srgba_unmultiplied());
    let m = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(m(0), m(1), m(2), m(3))
}
