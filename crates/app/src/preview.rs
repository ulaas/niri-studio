//! A live mock-up of a niri workspace, drawn from niri's own parsed layout config.

use egui::epaint::{Mesh, Shadow};
use egui::{pos2, vec2, Color32, CornerRadius, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui};
use niri_config::{Config, Gradient, GradientRelativeTo, PresetSize};

use crate::color::{self, from_niri};

/// Which element the user is hovering in the preview, so pages can explain it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Gap,
    Strut,
    FocusRing,
    Border,
    Window,
    TabIndicator,
}

impl Part {
    pub fn describe(self) -> &'static str {
        match self {
            Part::Gap => "gaps: space between windows and around the working area",
            Part::Strut => "struts: extra outer space reserved on each side",
            Part::FocusRing => "focus ring: drawn around the active window, outside it",
            Part::Border => "border: drawn around every window and takes up layout space",
            Part::Window => "a window; column widths come from presets and default-column-width",
            Part::TabIndicator => "tab indicator of a tabbed column",
        }
    }
}

/// What a window looks like inside, guessed from its app id.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppKind {
    Terminal,
    Browser,
    Editor,
    Files,
    Chat,
    /// niri studio itself, drawn in the app's theme.
    Studio,
    Generic,
}

impl AppKind {
    pub fn guess(app_id: &str) -> Self {
        let a = app_id.to_lowercase();
        let any = |list: &[&str]| list.iter().any(|k| a.contains(k));
        if a == "niri-studio" {
            AppKind::Studio
        } else if any(&["kitty", "foot", "alacritty", "wezterm", "ghostty", "konsole", "terminal", "ptyxis", "xterm", "rio"]) {
            AppKind::Terminal
        } else if any(&["firefox", "chrom", "brave", "zen", "librewolf", "epiphany", "vivaldi", "browser", "qutebrowser"]) {
            AppKind::Browser
        } else if any(&["code", "zed", "neovide", "emacs", "jetbrains", "gedit", "kate", "helix", "sublime", "texteditor"]) {
            AppKind::Editor
        } else if any(&["nautilus", "thunar", "dolphin", "nemo", "files", "pcmanfm", "yazi"]) {
            AppKind::Files
        } else if any(&["discord", "slack", "telegram", "signal", "element", "vesktop", "fractal", "thunderbird", "mail"]) {
            AppKind::Chat
        } else {
            AppKind::Generic
        }
    }
}

/// One tile of a column.
#[derive(Clone)]
pub struct Tile {
    pub kind: AppKind,
    pub title: String,
    /// Share of the column's height.
    pub share: f32,
    pub focused: bool,
    pub urgent: bool,
}

#[derive(Clone, Copy)]
pub enum ColumnWidth {
    /// Of the working area, as niri's proportions are.
    Proportion(f32),
    /// The window's own width in logical pixels.
    Fixed(f32),
}

#[derive(Clone)]
pub struct Column {
    pub width: ColumnWidth,
    pub tiles: Vec<Tile>,
    /// Shown as tabs rather than stacked.
    pub tabbed: bool,
}

#[derive(Clone)]
pub struct Floating {
    pub tile: Tile,
    /// Position and size in the workspace view, logical pixels.
    pub rect: Rect,
}

/// What to draw: the user's focused workspace when niri is running, else a sample.
#[derive(Clone)]
pub struct Scene {
    /// Logical size of the output.
    pub output: (f32, f32),
    pub columns: Vec<Column>,
    pub floating: Vec<Floating>,
    pub focused_column: usize,
    /// Where niri currently shows the focused column's left edge, in logical pixels.
    pub focused_x: Option<f32>,
    pub caption: String,
    pub live: bool,
}

impl Scene {
    /// A sample workspace following the config's column widths: a stacked column, the default
    /// width and the widest preset, one of them tabbed.
    pub fn sample(cfg: &Config, output: Option<(f32, f32)>) -> Self {
        let l = &cfg.layout;
        let width = |p: Option<PresetSize>| match p {
            Some(PresetSize::Proportion(p)) => ColumnWidth::Proportion(p as f32),
            Some(PresetSize::Fixed(px)) => ColumnWidth::Fixed(px as f32),
            None => ColumnWidth::Proportion(0.5),
        };
        let presets = &l.preset_column_widths;
        let tile = |kind, title: &str, share, focused| Tile { kind, title: title.into(), share, focused, urgent: false };
        Self {
            output: output.unwrap_or((1920.0, 1080.0)),
            columns: vec![
                Column {
                    width: width(presets.first().copied()),
                    tiles: vec![tile(AppKind::Terminal, "Terminal", 0.5, false), tile(AppKind::Files, "Files", 0.5, false)],
                    tabbed: false,
                },
                Column { width: width(l.default_column_width), tiles: vec![tile(AppKind::Editor, "Editor", 1.0, true)], tabbed: false },
                Column {
                    width: width(presets.last().copied()),
                    tiles: vec![tile(AppKind::Browser, "Browser", 1.0, false), tile(AppKind::Chat, "Chat", 1.0, false)],
                    tabbed: true,
                },
            ],
            floating: Vec::new(),
            focused_column: 1,
            focused_x: None,
            caption: "Sample layout".into(),
            live: false,
        }
    }
}

/// Draws the scene with the config's gaps, struts, borders, focus ring, shadows and corner radius,
/// filling the available width up to `max_height`. Returns the hovered part.
pub fn workspace(ui: &mut Ui, cfg: &Config, scene: &Scene, max_height: f32) -> Option<Part> {
    let (ow, oh) = scene.output;
    let width = ui.available_width().min(max_height * ow / oh);
    let (resp, painter) = ui.allocate_painter(vec2(width, width * oh / ow), Sense::hover());
    let screen = resp.rect;
    let s = screen.width() / ow; // logical px -> points
    let l = &cfg.layout;
    let mut hovered = None;
    let mut hovered_title: Option<&str> = None;
    let hover = resp.hover_pos();

    // A stand-in wallpaper shows through transparent backgrounds, as yours does on screen.
    wallpaper(&painter, screen);
    painter.rect_filled(screen, 8.0, from_niri(l.background_color));
    let st = &l.struts;
    let work = Rect::from_min_max(
        pos2(screen.left() + st.left.0 as f32 * s, screen.top() + st.top.0 as f32 * s),
        pos2(screen.right() - st.right.0 as f32 * s, screen.bottom() - st.bottom.0 as f32 * s),
    );
    let hatch = Color32::from_white_alpha(10);
    for r in [
        Rect::from_min_max(screen.min, pos2(work.left(), screen.bottom())),
        Rect::from_min_max(pos2(work.right(), screen.top()), screen.max),
        Rect::from_min_max(screen.min, pos2(screen.right(), work.top())),
        Rect::from_min_max(pos2(screen.left(), work.bottom()), screen.max),
    ] {
        if r.width() > 0.5 && r.height() > 0.5 {
            painter.rect_filled(r, 0.0, hatch);
            if hover.is_some_and(|h| r.contains(h)) {
                hovered = Some(Part::Strut);
            }
        }
    }

    let gap = l.gaps as f32 * s;
    let border = if l.border.off { 0.0 } else { l.border.width as f32 * s };
    let radius = corner_radius(cfg) * s;

    let view_w = work.width() - gap;
    let col_w = |w: ColumnWidth| match w {
        ColumnWidth::Proportion(p) => (view_w * p - gap).max(8.0),
        ColumnWidth::Fixed(px) => px * s + 2.0 * border,
    };
    let widths: Vec<f32> = scene.columns.iter().map(|c| col_w(c.width)).collect();
    let fc = scene.focused_column.min(widths.len().saturating_sub(1));
    let before: f32 = widths[..fc].iter().map(|w| w + gap).sum();
    let focused_w = widths.get(fc).copied().unwrap_or(0.0);
    // Where the focused column starts: where niri shows it now, centered when the config says so,
    // and always fully on screen, as niri keeps it.
    let center = match l.center_focused_column {
        niri_config::CenterFocusedColumn::Always => true,
        niri_config::CenterFocusedColumn::OnOverflow => widths.iter().map(|w| w + gap).sum::<f32>() + gap > work.width(),
        niri_config::CenterFocusedColumn::Never => false,
    };
    let mut fx = match (center, scene.focused_x) {
        (true, _) => work.center().x - focused_w / 2.0,
        (false, Some(x)) => screen.left() + x * s,
        (false, None) => work.left() + gap + before,
    };
    fx = fx.min(work.right() - gap - focused_w).max(work.left() + gap);
    let mut x = fx - before;
    let top = work.top() + gap;
    let bottom = work.bottom() - gap;

    let painter = painter.with_clip_rect(screen);
    if hover.is_some_and(|h| work.contains(h)) && hovered.is_none() {
        hovered = Some(Part::Gap);
    }

    for (col, w) in scene.columns.iter().zip(&widths) {
        let rect = Rect::from_min_max(pos2(x, top), pos2(x + w, bottom));
        x += w + gap;
        if rect.right() < screen.left() || rect.left() > screen.right() {
            continue;
        }
        let tiles: Vec<(Rect, &Tile)> = if col.tabbed || col.tiles.len() <= 1 {
            // A tabbed column shows its focused tab (or the first).
            let t = col.tiles.iter().find(|t| t.focused).or(col.tiles.first());
            t.map(|t| vec![(rect, t)]).unwrap_or_default()
        } else {
            let total: f32 = col.tiles.iter().map(|t| t.share).sum::<f32>().max(0.001);
            let avail = rect.height() - gap * (col.tiles.len() - 1) as f32;
            let mut y = rect.top();
            col.tiles
                .iter()
                .map(|t| {
                    let h = avail * t.share / total;
                    let r = Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), h));
                    y += h + gap;
                    (r, t)
                })
                .collect()
        };
        for (win, t) in tiles {
            if hover.is_some_and(|h| win.expand(border).contains(h)) {
                hovered = Some(Part::Window);
                hovered_title = Some(&t.title);
            }
            window(&painter, cfg, win, screen, t, radius, border, s, hover, &mut hovered);
        }
        if col.tabbed && !l.tab_indicator.off {
            tab_indicator(&painter, cfg, rect, screen, col.tiles.len().max(2), s, hover, &mut hovered);
        }
    }

    // Floating windows sit above the tiles, where niri has them.
    for f in &scene.floating {
        let win = Rect::from_min_size(screen.min + f.rect.min.to_vec2() * s, f.rect.size() * s).shrink(border);
        if hover.is_some_and(|h| win.expand(border).contains(h)) {
            hovered = Some(Part::Window);
            hovered_title = Some(&f.tile.title);
        }
        window(&painter, cfg, win.expand(border), screen, &f.tile, radius, border, s, hover, &mut hovered);
    }

    // Caption: whose workspace this is.
    let p = crate::theme::pal(ui.ctx());
    let font = egui::FontId::proportional(11.5);
    let galley = painter.layout_no_wrap(scene.caption.clone(), font, Color32::WHITE);
    let pill = Rect::from_min_size(screen.left_bottom() + vec2(8.0, -8.0 - galley.size().y - 6.0), galley.size() + vec2(18.0, 6.0));
    painter.rect_filled(pill, pill.height() / 2.0, Color32::from_black_alpha(150));
    let dot = pos2(pill.left() + 8.0, pill.center().y);
    painter.circle_filled(dot, 3.0, if scene.live { p.ok } else { Color32::from_gray(150) });
    painter.galley(pos2(pill.left() + 14.0, pill.top() + 3.0), galley, Color32::WHITE);

    painter.rect_stroke(screen, 8.0, Stroke::new(1.0, p.border), StrokeKind::Outside);
    if let (Some(Part::Window), Some(title)) = (hovered, hovered_title) {
        if !title.is_empty() {
            resp.on_hover_text_at_pointer(title);
        }
    }
    hovered
}

#[allow(clippy::too_many_arguments)]
fn window(
    painter: &Painter,
    cfg: &Config,
    win: Rect,
    view: Rect,
    tile: &Tile,
    radius: f32,
    border: f32,
    s: f32,
    hover: Option<Pos2>,
    hovered: &mut Option<Part>,
) {
    let l = &cfg.layout;
    let active = tile.focused;
    let outer_radius = radius + border;
    // Shadow (behind everything else of this window).
    if l.shadow.on {
        let sh = &l.shadow;
        let col = if active { sh.color } else { sh.inactive_color.unwrap_or(sh.color) };
        let shadow = Shadow {
            offset: [(sh.offset.x.0 as f32 * s).round() as i8, (sh.offset.y.0 as f32 * s).round() as i8],
            blur: (sh.softness as f32 * s).clamp(0.0, 255.0) as u8,
            spread: (sh.spread as f32 * s).clamp(0.0, 255.0) as u8,
            color: from_niri(col),
        };
        painter.add(shadow.as_shape(win.expand(border), CornerRadius::same(outer_radius.round() as u8)));
    }

    // Border: part of the window's layout size.
    let content = win.shrink(border);
    if border > 0.0 {
        let b = &l.border;
        let (c, g) = if tile.urgent {
            (b.urgent_color, b.urgent_gradient)
        } else if active {
            (b.active_color, b.active_gradient)
        } else {
            (b.inactive_color, b.inactive_gradient)
        };
        ring(painter, content, border.max(1.0), radius, c, g, view);
        if hover.is_some_and(|h| win.contains(h) && !content.contains(h)) {
            *hovered = Some(Part::Border);
        }
    }

    app_content(painter, content, radius, tile.kind, active, s);

    // Focus ring: outside the window, only on the active one.
    if (active || tile.urgent) && !l.focus_ring.off {
        let f = &l.focus_ring;
        // Keep thin rings visible at preview scale.
        let w = (f.width as f32 * s).max(1.5);
        let (c, g) = if tile.urgent { (f.urgent_color, f.urgent_gradient) } else { (f.active_color, f.active_gradient) };
        ring(painter, win, w, outer_radius, c, g, view);
        if hover.is_some_and(|h| win.expand(w).contains(h) && !win.contains(h)) {
            *hovered = Some(Part::FocusRing);
        }
    }
}

/// A window's insides, sketched like the app it is: enough to read as a real desktop.
fn app_content(painter: &Painter, r: Rect, radius: f32, kind: AppKind, active: bool, s: f32) {
    let cr = |nw: bool, ne: bool, sw: bool, se: bool| {
        let v = radius.round().clamp(0.0, 255.0) as u8;
        CornerRadius { nw: if nw { v } else { 0 }, ne: if ne { v } else { 0 }, sw: if sw { v } else { 0 }, se: if se { v } else { 0 } }
    };
    let u = (s * 2.2).clamp(0.6, 2.0); // a "line" unit that tracks the preview scale
    let line_h = 3.0 * u;
    let pad = 8.0 * u;
    let dim = |c: Color32| if active { c } else { color::lerp(c, Color32::from_gray(30), 0.25) };
    let bar = |painter: &Painter, x: f32, y: f32, w: f32, c: Color32| {
        if y + line_h < r.bottom() - pad * 0.5 && w > 1.0 {
            painter.rect_filled(Rect::from_min_size(pos2(x, y), vec2(w.min(r.right() - pad - x), line_h)), line_h / 2.0, dim(c));
        }
    };
    let rows = |from: f32| ((r.bottom() - from) / (line_h * 2.6)).max(0.0) as usize;
    match kind {
        AppKind::Terminal => {
            painter.rect_filled(r, radius, dim(Color32::from_rgb(24, 24, 30)));
            let mut y = r.top() + pad;
            let widths = [0.42, 0.7, 0.55, 0.3, 0.62, 0.48, 0.8, 0.36, 0.5, 0.66];
            for i in 0..rows(y).min(24) {
                let prompt = i % 4 == 0;
                if prompt {
                    bar(painter, r.left() + pad, y, 6.0 * u, Color32::from_rgb(120, 220, 150));
                    bar(painter, r.left() + pad + 9.0 * u, y, r.width() * 0.25, Color32::from_rgb(130, 170, 255));
                } else {
                    bar(painter, r.left() + pad, y, r.width() * widths[i % widths.len()], Color32::from_gray(150));
                }
                y += line_h * 2.6;
            }
            if active && y + line_h < r.bottom() {
                bar(painter, r.left() + pad, y, 6.0 * u, Color32::from_rgb(120, 220, 150));
                painter.rect_filled(Rect::from_min_size(pos2(r.left() + pad + 9.0 * u, y - u), vec2(4.0 * u, line_h + 2.0 * u)), 0.0, Color32::from_gray(220));
            }
        }
        AppKind::Browser => {
            let tool_h = (34.0 * s).clamp(6.0, 22.0);
            let tool = Rect::from_min_size(r.min, vec2(r.width(), tool_h));
            painter.rect_filled(r, radius, dim(Color32::from_rgb(246, 246, 249)));
            painter.rect_filled(tool, cr(true, true, false, false), dim(Color32::from_rgb(43, 43, 50)));
            let url = Rect::from_min_max(pos2(tool.left() + tool_h * 1.6, tool.top() + tool_h * 0.25), pos2(tool.right() - tool_h, tool.bottom() - tool_h * 0.25));
            if url.width() > 4.0 {
                painter.rect_filled(url, url.height() / 2.0, dim(Color32::from_rgb(70, 70, 80)));
            }
            let body = Rect::from_min_max(pos2(r.left(), tool.bottom()), r.max).shrink(pad);
            let hero = Rect::from_min_size(body.min, vec2(body.width(), (body.height() * 0.3).min(90.0 * s.max(0.3))));
            if hero.height() > 2.0 {
                painter.rect_filled(hero, 3.0 * u, dim(Color32::from_rgb(99, 124, 255)));
            }
            let mut y = hero.bottom() + pad;
            for i in 0..rows(y).min(14) {
                bar(painter, body.left(), y, body.width() * [0.9, 0.75, 0.82, 0.4][i % 4], Color32::from_gray(190));
                y += line_h * 2.6;
            }
        }
        AppKind::Editor => {
            painter.rect_filled(r, radius, dim(Color32::from_rgb(30, 33, 40)));
            let gutter = Rect::from_min_max(r.min, pos2(r.left() + (28.0 * u).min(r.width() * 0.2), r.bottom()));
            painter.rect_filled(gutter, cr(true, false, true, false), dim(Color32::from_rgb(25, 27, 33)));
            let colors = [
                Color32::from_rgb(198, 120, 221),
                Color32::from_rgb(97, 175, 239),
                Color32::from_rgb(152, 195, 121),
                Color32::from_rgb(229, 192, 123),
                Color32::from_gray(170),
            ];
            let indent = [0, 1, 1, 2, 2, 1, 0, 0, 1, 2, 1, 0];
            let mut y = r.top() + pad;
            for i in 0..rows(y).min(30) {
                let x0 = gutter.right() + pad + indent[i % indent.len()] as f32 * 10.0 * u;
                bar(painter, gutter.left() + pad * 0.6, y, 8.0 * u, Color32::from_gray(80));
                let w1 = r.width() * [0.12, 0.2, 0.08, 0.16][i % 4];
                bar(painter, x0, y, w1, colors[i % colors.len()]);
                bar(painter, x0 + w1 + 4.0 * u, y, r.width() * [0.25, 0.1, 0.32, 0.18][i % 4], colors[(i + 2) % colors.len()]);
                y += line_h * 2.6;
            }
        }
        AppKind::Files => {
            painter.rect_filled(r, radius, dim(Color32::from_rgb(40, 41, 48)));
            let side = Rect::from_min_max(r.min, pos2(r.left() + r.width() * 0.28, r.bottom()));
            painter.rect_filled(side, cr(true, false, true, false), dim(Color32::from_rgb(33, 34, 40)));
            let mut y = side.top() + pad;
            for i in 0..rows(y).min(10) {
                bar(painter, side.left() + pad, y, side.width() * 0.6, if i == 1 { Color32::from_rgb(130, 170, 255) } else { Color32::from_gray(140) });
                y += line_h * 2.6;
            }
            let cell = (36.0 * u).max(6.0);
            let area = Rect::from_min_max(pos2(side.right() + pad, r.top() + pad), r.max - vec2(pad, pad));
            let cols = (area.width() / (cell * 1.4)).floor().max(1.0) as usize;
            let rows_n = (area.height() / (cell * 1.5)).floor().max(0.0) as usize;
            for row in 0..rows_n.min(6) {
                for c in 0..cols.min(8) {
                    let p = pos2(area.left() + c as f32 * cell * 1.4, area.top() + row as f32 * cell * 1.5);
                    painter.rect_filled(Rect::from_min_size(p, vec2(cell, cell * 0.78)), 2.0 * u, dim(Color32::from_rgb(110, 150, 230)));
                }
            }
        }
        AppKind::Chat => {
            painter.rect_filled(r, radius, dim(Color32::from_rgb(49, 51, 60)));
            let side = Rect::from_min_max(r.min, pos2(r.left() + r.width() * 0.25, r.bottom()));
            painter.rect_filled(side, cr(true, false, true, false), dim(Color32::from_rgb(40, 42, 50)));
            let mut y = side.top() + pad;
            for _ in 0..rows(y).min(10) {
                painter.circle_filled(pos2(side.left() + pad + 2.5 * u, y + line_h / 2.0), 2.5 * u, dim(Color32::from_gray(120)));
                bar(painter, side.left() + pad + 8.0 * u, y, side.width() * 0.45, Color32::from_gray(150));
                y += line_h * 3.2;
            }
            let mut y = r.top() + pad;
            for i in 0..((r.bottom() - y) / (line_h * 7.0)).max(0.0) as usize {
                let mine = i % 3 == 1;
                let w = (r.width() - side.width()) * [0.5, 0.35, 0.6][i % 3];
                let x = if mine { r.right() - pad - w } else { side.right() + pad };
                let bubble = Rect::from_min_size(pos2(x, y), vec2(w, line_h * 4.0));
                if bubble.bottom() < r.bottom() - pad && w > 2.0 {
                    let c = if mine { Color32::from_rgb(88, 101, 242) } else { Color32::from_gray(75) };
                    painter.rect_filled(bubble, 4.0 * u, dim(c));
                }
                y += line_h * 6.0;
            }
        }
        AppKind::Studio => {
            let t = crate::theme::pal(painter.ctx());
            painter.rect_filled(r, radius, t.bg);
            let head = Rect::from_min_size(r.min, vec2(r.width(), (48.0 * s).clamp(4.0, 16.0)));
            painter.rect_filled(head, cr(true, true, false, false), t.chrome);
            let side = Rect::from_min_max(pos2(r.left(), head.bottom()), pos2(r.left() + r.width() * 0.2, r.bottom()));
            painter.rect_filled(side, cr(false, false, true, false), t.chrome);
            let mut y = side.top() + pad;
            for i in 0..rows(y).min(12) {
                let c = if i == 2 { t.accent } else { t.faint };
                bar(painter, side.left() + pad, y, side.width() * 0.6, c);
                y += line_h * 2.6;
            }
            let body = Rect::from_min_max(pos2(side.right() + pad, head.bottom() + pad), r.max - vec2(pad, pad));
            bar(painter, body.left(), body.top(), body.width() * 0.3, t.text);
            let mut y = body.top() + line_h * 3.0;
            while y + 20.0 * u < body.bottom() {
                let card = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), (40.0 * u).min(body.bottom() - y)));
                painter.rect(card, 3.0 * u, t.surface, Stroke::new(1.0, t.border), StrokeKind::Inside);
                bar(painter, card.left() + pad * 0.6, card.top() + pad * 0.6, card.width() * 0.25, t.muted);
                bar(painter, card.left() + pad * 0.6, card.top() + pad * 0.6 + line_h * 2.4, card.width() * 0.5, t.faint);
                y = card.bottom() + pad;
            }
        }
        AppKind::Generic => {
            painter.rect_filled(r, radius, dim(Color32::from_gray(52)));
            let head = Rect::from_min_size(r.min, vec2(r.width(), (28.0 * s).clamp(4.0, 18.0)));
            painter.rect_filled(head, cr(true, true, false, false), dim(Color32::from_gray(64)));
            let mut y = head.bottom() + pad;
            for i in 0..rows(y).min(12) {
                bar(painter, r.left() + pad, y, r.width() * [0.7, 0.5, 0.62, 0.35][i % 4], Color32::from_gray(95));
                y += line_h * 2.6;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn tab_indicator(
    painter: &Painter,
    cfg: &Config,
    col: Rect,
    view: Rect,
    tabs: usize,
    s: f32,
    hover: Option<Pos2>,
    hovered: &mut Option<Part>,
) {
    use niri_config::TabIndicatorPosition as P;
    let t = &cfg.layout.tab_indicator;
    let fr = &cfg.layout.focus_ring;
    let width = t.width as f32 * s;
    let gap = t.gap as f32 * s;
    let prop = t.length.total_proportion.unwrap_or(0.5) as f32;
    let between = t.gaps_between_tabs as f32 * s;
    let vertical = matches!(t.position, P::Left | P::Right);
    let span = if vertical { col.height() } else { col.width() } * prop;
    let tab_len = (span - between * (tabs - 1) as f32) / tabs as f32;
    let start = if vertical { col.center().y - span / 2.0 } else { col.center().x - span / 2.0 };
    let (inside, outside) = if t.place_within_column { (1.0, 0.0) } else { (0.0, 1.0) };
    for i in 0..tabs {
        let a = start + i as f32 * (tab_len + between);
        let r = match t.position {
            P::Left => {
                let x = col.left() - outside * (gap + width) + inside * gap;
                Rect::from_min_size(pos2(x, a), vec2(width, tab_len))
            }
            P::Right => {
                let x = col.right() + outside * gap - inside * (gap + width);
                Rect::from_min_size(pos2(x, a), vec2(width, tab_len))
            }
            P::Top => {
                let y = col.top() - outside * (gap + width) + inside * gap;
                Rect::from_min_size(pos2(a, y), vec2(tab_len, width))
            }
            P::Bottom => {
                let y = col.bottom() + outside * gap - inside * (gap + width);
                Rect::from_min_size(pos2(a, y), vec2(tab_len, width))
            }
        };
        let (c, g) = if i == 0 {
            (t.active_color.unwrap_or(fr.active_color), t.active_gradient.or(if t.active_color.is_none() { fr.active_gradient } else { None }))
        } else {
            (t.inactive_color.unwrap_or(fr.inactive_color), t.inactive_gradient)
        };
        let radius = (t.corner_radius as f32 * s).min(r.width().min(r.height()) / 2.0);
        fill_gradient(painter, r, radius, c, g, view);
        if hover.is_some_and(|h| r.expand(2.0).contains(h)) {
            *hovered = Some(Part::TabIndicator);
        }
    }
}

fn wallpaper(painter: &Painter, r: Rect) {
    let mut mesh = Mesh::default();
    let cols = [
        Color32::from_rgb(38, 52, 82),
        Color32::from_rgb(70, 48, 90),
        Color32::from_rgb(28, 70, 80),
        Color32::from_rgb(20, 28, 48),
    ];
    for (p, c) in [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].into_iter().zip(cols) {
        mesh.colored_vertex(p, c);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
}

/// The effective window corner radius: from the first window rule that applies to all windows.
fn corner_radius(cfg: &Config) -> f32 {
    cfg.window_rules
        .iter()
        .filter(|r| r.matches.is_empty())
        .filter_map(|r| r.geometry_corner_radius)
        .next_back()
        .map_or(0.0, |c| c.top_left)
}

/// A solid or gradient ring of `width` drawn outside `inner`.
fn ring(painter: &Painter, inner: Rect, width: f32, radius: f32, c: niri_config::Color, g: Option<Gradient>, view: Rect) {
    if width <= 0.0 {
        return;
    }
    let outer = inner.expand(width);
    let pts_in = rounded_rect_points(inner, radius);
    let pts_out = rounded_rect_points(outer, radius + width);
    let shade = shader(c, g, outer, view);
    let mut mesh = Mesh::default();
    for (a, b) in pts_out.iter().zip(&pts_in) {
        mesh.colored_vertex(*a, shade(*a));
        mesh.colored_vertex(*b, shade(*b));
    }
    let n = pts_in.len() as u32;
    for i in 0..n {
        let j = (i + 1) % n;
        let (o0, i0, o1, i1) = (2 * i, 2 * i + 1, 2 * j, 2 * j + 1);
        mesh.add_triangle(o0, i0, o1);
        mesh.add_triangle(i0, i1, o1);
    }
    painter.add(mesh);
}

fn fill_gradient(painter: &Painter, r: Rect, radius: f32, c: niri_config::Color, g: Option<Gradient>, view: Rect) {
    let pts = rounded_rect_points(r, radius);
    let shade = shader(c, g, r, view);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(r.center(), shade(r.center()));
    for p in &pts {
        mesh.colored_vertex(*p, shade(*p));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(mesh);
}

/// Color at a point: CSS linear-gradient semantics (0° points up, 180° is the default, down).
fn shader(c: niri_config::Color, g: Option<Gradient>, rect: Rect, view: Rect) -> impl Fn(Pos2) -> Color32 {
    let solid = from_niri(c);
    let g = g.map(|g| {
        let area = if g.relative_to == GradientRelativeTo::WorkspaceView { view } else { rect };
        let a = (g.angle as f32).to_radians();
        let dir = vec2(a.sin(), -a.cos());
        // Gradient line length so the corners hit 0 and 1 exactly, as CSS does.
        let half = (area.width() * dir.x.abs() + area.height() * dir.y.abs()) / 2.0;
        (from_niri(g.from), from_niri(g.to), area.center(), dir, half.max(0.001))
    });
    move |p: Pos2| match g {
        None => solid,
        Some((from, to, center, dir, half)) => {
            let t = ((p - center).dot(dir) / half * 0.5 + 0.5).clamp(0.0, 1.0);
            color::lerp(from, to, t)
        }
    }
}

/// Points around a rounded rectangle, clockwise from the top-left corner, with a fixed count
/// regardless of radius so inner and outer outlines pair up.
fn rounded_rect_points(r: Rect, radius: f32) -> Vec<Pos2> {
    const STEPS: usize = 8;
    let radius = radius.clamp(0.0, r.width().min(r.height()) / 2.0);
    let corners = [
        (pos2(r.left() + radius, r.top() + radius), 180.0f32),
        (pos2(r.right() - radius, r.top() + radius), 270.0),
        (pos2(r.right() - radius, r.bottom() - radius), 0.0),
        (pos2(r.left() + radius, r.bottom() - radius), 90.0),
    ];
    let mut pts = Vec::with_capacity(4 * (STEPS + 1));
    for (c, start) in corners {
        for i in 0..=STEPS {
            let a = (start + 90.0 * i as f32 / STEPS as f32).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * radius);
        }
    }
    pts
}
