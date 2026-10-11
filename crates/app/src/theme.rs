//! The look of the app: color palettes, the egui style built from them, fonts and the
//! user's appearance preferences (saved to `$XDG_CONFIG_HOME/niri-studio/studio.conf`).
//!
//! Widgets read colors through [`pal`] instead of hard-coding them, so every preset and
//! accent applies everywhere.

use std::path::PathBuf;

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Theme};

/// Semantic color tokens.
#[derive(Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    /// The navigation sidebar and top bar.
    pub chrome: Color32,
    /// Behind the pages.
    pub bg: Color32,
    /// Cards and popups.
    pub surface: Color32,
    /// Buttons, inputs and slider rails at rest.
    pub control: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub error: Color32,
    /// Two extra hues for syntax highlighting.
    pub blue: Color32,
    pub purple: Color32,
}

impl Palette {
    pub fn control_hover(&self) -> Color32 {
        mix(self.control, self.text, 0.07)
    }
    pub fn control_active(&self) -> Color32 {
        mix(self.control, self.text, 0.14)
    }
    pub fn border_strong(&self) -> Color32 {
        mix(self.border, self.text, 0.18)
    }
    /// Accent usable as text or thin lines on the page background.
    pub fn accent_fg(&self) -> Color32 {
        readable(self.accent, self.bg, 3.2)
    }
    /// Text drawn on a solid accent fill.
    pub fn on_accent(&self) -> Color32 {
        if luminance(self.accent) > 0.4 { Color32::from_rgb(16, 16, 20) } else { Color32::WHITE }
    }
    /// A translucent wash of `c` over the background, for selected rows and badges.
    pub fn tint(&self, c: Color32, amount: f32) -> Color32 {
        mix(self.surface, c, amount)
    }
    pub fn shadow(&self) -> Color32 {
        Color32::from_black_alpha(if self.dark { 110 } else { 28 })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    System,
    StudioDark,
    StudioLight,
    CatppuccinMocha,
    CatppuccinLatte,
    TokyoNight,
    RosePine,
    Nord,
    GruvboxDark,
}

impl Preset {
    pub const ALL: [Preset; 9] = [
        Preset::System,
        Preset::StudioDark,
        Preset::StudioLight,
        Preset::CatppuccinMocha,
        Preset::CatppuccinLatte,
        Preset::TokyoNight,
        Preset::RosePine,
        Preset::Nord,
        Preset::GruvboxDark,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Preset::System => "Follow system",
            Preset::StudioDark => "Studio Dark",
            Preset::StudioLight => "Studio Light",
            Preset::CatppuccinMocha => "Catppuccin Mocha",
            Preset::CatppuccinLatte => "Catppuccin Latte",
            Preset::TokyoNight => "Tokyo Night",
            Preset::RosePine => "Rosé Pine",
            Preset::Nord => "Nord",
            Preset::GruvboxDark => "Gruvbox Dark",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Preset::System => "system",
            Preset::StudioDark => "studio-dark",
            Preset::StudioLight => "studio-light",
            Preset::CatppuccinMocha => "catppuccin-mocha",
            Preset::CatppuccinLatte => "catppuccin-latte",
            Preset::TokyoNight => "tokyo-night",
            Preset::RosePine => "rose-pine",
            Preset::Nord => "nord",
            Preset::GruvboxDark => "gruvbox-dark",
        }
    }

    pub fn palette(self, system: Theme) -> Palette {
        let h = hex;
        match self {
            Preset::System => match system {
                Theme::Dark => Preset::StudioDark.palette(system),
                Theme::Light => Preset::StudioLight.palette(system),
            },
            Preset::StudioDark => Palette {
                dark: true,
                chrome: h(0x111113),
                bg: h(0x161618),
                surface: h(0x1d1d20),
                control: h(0x29292e),
                border: h(0x2c2c31),
                text: h(0xececf1),
                muted: h(0xa1a1aa),
                faint: h(0x71717a),
                accent: h(0x7c9cff),
                ok: h(0x4ade80),
                warn: h(0xfbbf24),
                error: h(0xf87171),
                blue: h(0x7cb4ff),
                purple: h(0xc89cff),
            },
            Preset::StudioLight => Palette {
                dark: false,
                chrome: h(0xf3f3f5),
                bg: h(0xf8f8fa),
                surface: h(0xffffff),
                control: h(0xefeff2),
                border: h(0xe2e2e7),
                text: h(0x18181b),
                muted: h(0x52525b),
                faint: h(0x9a9aa3),
                accent: h(0x4f6bed),
                ok: h(0x15803d),
                warn: h(0xb45309),
                error: h(0xdc2626),
                blue: h(0x1d5fc9),
                purple: h(0x8a3bc2),
            },
            Preset::CatppuccinMocha => Palette {
                dark: true,
                chrome: h(0x181825),
                bg: h(0x1e1e2e),
                surface: h(0x24243a),
                control: h(0x313244),
                border: h(0x313244),
                text: h(0xcdd6f4),
                muted: h(0xa6adc8),
                faint: h(0x6c7086),
                accent: h(0xcba6f7),
                ok: h(0xa6e3a1),
                warn: h(0xf9e2af),
                error: h(0xf38ba8),
                blue: h(0x89b4fa),
                purple: h(0xf5c2e7),
            },
            Preset::CatppuccinLatte => Palette {
                dark: false,
                chrome: h(0xe6e9ef),
                bg: h(0xeff1f5),
                surface: h(0xf8f9fb),
                control: h(0xdce0e8),
                border: h(0xccd0da),
                text: h(0x4c4f69),
                muted: h(0x6c6f85),
                faint: h(0x9ca0b0),
                accent: h(0x8839ef),
                ok: h(0x40a02b),
                warn: h(0xdf8e1d),
                error: h(0xd20f39),
                blue: h(0x1e66f5),
                purple: h(0xea76cb),
            },
            Preset::TokyoNight => Palette {
                dark: true,
                chrome: h(0x16161e),
                bg: h(0x1a1b26),
                surface: h(0x1f2335),
                control: h(0x292e42),
                border: h(0x292e42),
                text: h(0xc0caf5),
                muted: h(0xa9b1d6),
                faint: h(0x565f89),
                accent: h(0x7aa2f7),
                ok: h(0x9ece6a),
                warn: h(0xe0af68),
                error: h(0xf7768e),
                blue: h(0x7dcfff),
                purple: h(0xbb9af7),
            },
            Preset::RosePine => Palette {
                dark: true,
                chrome: h(0x16141f),
                bg: h(0x191724),
                surface: h(0x1f1d2e),
                control: h(0x26233a),
                border: h(0x2a273f),
                text: h(0xe0def4),
                muted: h(0x908caa),
                faint: h(0x6e6a86),
                accent: h(0xc4a7e7),
                ok: h(0x9ccfd8),
                warn: h(0xf6c177),
                error: h(0xeb6f92),
                blue: h(0x31748f),
                purple: h(0xebbcba),
            },
            Preset::Nord => Palette {
                dark: true,
                chrome: h(0x2b303b),
                bg: h(0x2e3440),
                surface: h(0x353c4a),
                control: h(0x434c5e),
                border: h(0x434c5e),
                text: h(0xeceff4),
                muted: h(0xd8dee9),
                faint: h(0x8a94a8),
                accent: h(0x88c0d0),
                ok: h(0xa3be8c),
                warn: h(0xebcb8b),
                error: h(0xbf616a),
                blue: h(0x81a1c1),
                purple: h(0xb48ead),
            },
            Preset::GruvboxDark => Palette {
                dark: true,
                chrome: h(0x1d2021),
                bg: h(0x282828),
                surface: h(0x302e2c),
                control: h(0x3c3836),
                border: h(0x3c3836),
                text: h(0xebdbb2),
                muted: h(0xbdae93),
                faint: h(0x7c6f64),
                accent: h(0xfe8019),
                ok: h(0xb8bb26),
                warn: h(0xfabd2f),
                error: h(0xfb4934),
                blue: h(0x83a598),
                purple: h(0xd3869b),
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Accent {
    /// Follow niri's focus ring color, so the app matches the desktop.
    FocusRing,
    /// The preset's own accent.
    Theme,
    Custom(Color32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Density {
    Comfortable,
    Compact,
}

/// Appearance preferences.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Settings {
    pub preset: Preset,
    pub accent: Accent,
    pub density: Density,
    /// Corner radius of controls; cards and popups scale from it.
    pub radius: u8,
    /// Use the desktop's sans-serif font (via fontconfig) instead of egui's built-in one.
    pub system_font: bool,
    /// Draw our own title bar with window controls instead of asking the compositor for
    /// decorations.
    pub custom_titlebar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { preset: Preset::System, accent: Accent::FocusRing, density: Density::Comfortable, radius: 7, system_font: true, custom_titlebar: true }
    }
}

impl Settings {
    fn path() -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| doc::home().join(".config"));
        base.join("niri-studio/studio.conf")
    }

    pub fn load() -> Self {
        let mut s = Self::default();
        let Ok(text) = std::fs::read_to_string(Self::path()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "theme" => {
                    if let Some(p) = Preset::ALL.into_iter().find(|p| p.key() == v) {
                        s.preset = p;
                    }
                }
                "accent" => {
                    s.accent = match v {
                        "focus-ring" => Accent::FocusRing,
                        "theme" => Accent::Theme,
                        c => crate::color::parse(c).map_or(s.accent, Accent::Custom),
                    }
                }
                "density" => s.density = if v == "compact" { Density::Compact } else { Density::Comfortable },
                "radius" => s.radius = v.parse().unwrap_or(s.radius).min(14),
                "system-font" => s.system_font = v != "false",
                "titlebar" => s.custom_titlebar = v != "system",
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) -> std::io::Result<()> {
        let accent = match self.accent {
            Accent::FocusRing => "focus-ring".to_owned(),
            Accent::Theme => "theme".to_owned(),
            Accent::Custom(c) => crate::color::format(c),
        };
        let density = match self.density {
            Density::Comfortable => "comfortable",
            Density::Compact => "compact",
        };
        let text = format!(
            "# niri studio appearance\ntheme = {}\naccent = {accent}\ndensity = {density}\nradius = {}\nsystem-font = {}\ntitlebar = {}\n",
            self.preset.key(),
            self.radius,
            self.system_font,
            if self.custom_titlebar { "app" } else { "system" },
        );
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, text)
    }

    /// The palette to use, given the system theme and niri's focus ring color.
    pub fn resolve(&self, system: Theme, focus_ring: Color32) -> Palette {
        let mut p = self.preset.palette(system);
        p.accent = match self.accent {
            Accent::FocusRing => focus_ring,
            Accent::Theme => p.accent,
            Accent::Custom(c) => c,
        };
        p
    }
}

/// The palette in use; widgets call this instead of hard-coding colors.
pub fn pal(ctx: &egui::Context) -> Palette {
    ctx.data(|d| d.get_temp::<Palette>(egui::Id::NULL.with("palette")))
        .unwrap_or_else(|| Preset::StudioDark.palette(Theme::Dark))
}

/// Applies a palette and settings to the egui style (both its dark and light slots, since the
/// app resolves "follow system" itself).
pub fn apply(ctx: &egui::Context, p: &Palette, s: &Settings) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL.with("palette"), *p));
    let style = style(p, s);
    ctx.set_style_of(Theme::Dark, style.clone());
    ctx.set_style_of(Theme::Light, style);
}

fn style(p: &Palette, s: &Settings) -> egui::Style {
    let compact = s.density == Density::Compact;
    let r = s.radius;
    let mut style = egui::Style::default();
    let mut v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };

    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(r.saturating_add(5));
    v.menu_corner_radius = CornerRadius::same(r.saturating_add(3));
    v.window_shadow = Shadow { offset: [0, 10], blur: 32, spread: 0, color: p.shadow() };
    v.popup_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: p.shadow() };
    v.faint_bg_color = mix(p.surface, p.text, 0.025);
    v.extreme_bg_color = if p.dark { mix(p.bg, Color32::BLACK, 0.25) } else { p.surface };
    v.text_edit_bg_color = Some(if p.dark { mix(p.bg, Color32::BLACK, 0.2) } else { p.surface });
    v.code_bg_color = p.control;
    v.hyperlink_color = p.accent_fg();
    v.warn_fg_color = p.warn;
    v.error_fg_color = p.error;
    v.weak_text_color = Some(p.faint);
    v.selection.bg_fill = mix(p.surface, p.accent, if p.dark { 0.42 } else { 0.30 });
    v.selection.stroke = Stroke::new(1.0, readable(p.accent, v.selection.bg_fill, 3.5));
    v.text_cursor.stroke = Stroke::new(2.0, p.accent_fg());
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    v.striped = false;
    v.indent_has_left_vline = true;
    v.collapsing_header_frame = false;

    let cr = CornerRadius::same(r);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = cr;

    w.inactive.bg_fill = p.control;
    w.inactive.weak_bg_fill = p.control;
    w.inactive.bg_stroke = Stroke::new(1.0, mix(p.control, p.border, 0.5));
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = cr;
    w.inactive.expansion = 0.0;

    w.hovered.bg_fill = p.control_hover();
    w.hovered.weak_bg_fill = p.control_hover();
    w.hovered.bg_stroke = Stroke::new(1.0, p.border_strong());
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.hovered.corner_radius = cr;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.control_active();
    w.active.weak_bg_fill = p.control_active();
    w.active.bg_stroke = Stroke::new(1.0, p.accent_fg());
    w.active.fg_stroke = Stroke::new(1.5, p.text);
    w.active.corner_radius = cr;
    w.active.expansion = 0.0;

    w.open.bg_fill = p.control_hover();
    w.open.weak_bg_fill = p.control_hover();
    w.open.bg_stroke = Stroke::new(1.0, p.accent_fg());
    w.open.fg_stroke = Stroke::new(1.0, p.text);
    w.open.corner_radius = cr;
    style.visuals = v;

    let sp = &mut style.spacing;
    sp.item_spacing = if compact { egui::vec2(6.0, 5.0) } else { egui::vec2(8.0, 8.0) };
    sp.button_padding = if compact { egui::vec2(8.0, 3.0) } else { egui::vec2(11.0, 5.0) };
    sp.interact_size.y = if compact { 24.0 } else { 30.0 };
    sp.window_margin = Margin::same(14);
    sp.menu_margin = Margin::same(6);
    sp.slider_width = 170.0;
    sp.slider_rail_height = 6.0;
    sp.combo_width = 180.0;
    sp.text_edit_width = 260.0;
    sp.icon_width = 16.0;
    sp.icon_width_inner = 9.0;
    sp.icon_spacing = 6.0;
    sp.tooltip_width = 420.0;
    sp.menu_spacing = 4.0;
    sp.indent = 18.0;
    sp.scroll = egui::style::ScrollStyle::floating();
    sp.scroll.bar_width = 8.0;
    sp.scroll.floating_width = 3.0;

    style.text_styles = [
        (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into();
    style.animation_time = 0.12;
    style.interaction.tooltip_delay = 0.35;
    style
}

/// Installs fonts: the desktop's UI and monospace fonts when wanted (looked up with
/// fontconfig), egui's built-in fonts as fallback, and the Phosphor icon font.
pub fn install_fonts(ctx: &egui::Context, system_font: bool) {
    let mut fonts = egui::FontDefinitions::default();
    if system_font {
        for (family, query) in [(FontFamily::Proportional, "sans-serif:style=Regular"), (FontFamily::Monospace, "monospace:style=Regular")] {
            if let Some(bytes) = fc_match(query) {
                let name = format!("system-{family:?}");
                fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes).into());
                fonts.families.entry(family).or_default().insert(0, name);
            }
        }
    }
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

/// The font file fontconfig picks for `pattern`, if it's a TrueType/OpenType file.
fn fc_match(pattern: &str) -> Option<Vec<u8>> {
    let out = std::process::Command::new("fc-match").args(["-f", "%{file}", pattern]).output().ok()?;
    let path = String::from_utf8(out.stdout).ok()?;
    let lower = path.to_lowercase();
    if !(lower.ends_with(".ttf") || lower.ends_with(".otf")) {
        return None;
    }
    std::fs::read(path).ok()
}

// ---- color math ----

pub const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// Linear mix in sRGB space, keeping `a`'s alpha.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), a.a())
}

/// Relative luminance (WCAG).
pub fn luminance(c: Color32) -> f32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// `c`, lightened or darkened just enough to reach `min` contrast against `bg`.
pub fn readable(c: Color32, bg: Color32, min: f32) -> Color32 {
    let target = if luminance(bg) < 0.4 { Color32::WHITE } else { Color32::BLACK };
    let mut out = Color32::from_rgb(c.r(), c.g(), c.b());
    for i in 1..=10 {
        if contrast(out, bg) >= min {
            break;
        }
        out = mix(Color32::from_rgb(c.r(), c.g(), c.b()), target, i as f32 * 0.1);
    }
    out
}

/// Phosphor icons used across the app.
pub mod icons {
    pub use egui_phosphor::regular::*;

    pub fn page(p: schema::Page) -> &'static str {
        use schema::Page;
        match p {
            Page::Input => KEYBOARD,
            Page::Layout => SQUARES_FOUR,
            Page::Appearance => PAINT_BRUSH,
            Page::Animations => WAVE_SINE,
            Page::Displays => MONITOR,
            Page::Keybinds => COMMAND,
            Page::Rules => APP_WINDOW,
            Page::Startup => ROCKET_LAUNCH,
            Page::Gestures => HAND_SWIPE_RIGHT,
            Page::Misc => SLIDERS_HORIZONTAL,
            Page::Raw => BRACKETS_CURLY,
            Page::History => CLOCK_COUNTER_CLOCKWISE,
        }
    }
}
