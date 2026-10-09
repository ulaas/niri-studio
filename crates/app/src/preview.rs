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

/// Draws the preview into a `height`-tall area of the full available width.
/// `out_size` is the logical size of the output to mimic.
pub fn workspace(ui: &mut Ui, cfg: &Config, out_size: Option<(f32, f32)>, height: f32) -> Option<Part> {
    let (ow, oh) = out_size.unwrap_or((1920.0, 1080.0));
    let width = ui.available_width().min(height * ow / oh);
    let (resp, painter) = ui.allocate_painter(vec2(width, width * oh / ow), Sense::hover());
    let screen = resp.rect;
    let s = screen.width() / ow; // logical px -> points
    let l = &cfg.layout;
    let mut hovered = None;
    let hover = resp.hover_pos();

    // A stand-in wallpaper shows through transparent backgrounds, as yours does on screen.
    wallpaper(&painter, screen);
    painter.rect_filled(screen, 6.0, from_niri(l.background_color));
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

    // Columns: the default width, then the smallest and largest presets.
    let view_w = work.width() - gap;
    let col_w = |p: Option<PresetSize>| match p {
        Some(PresetSize::Proportion(p)) => (view_w * p as f32 - gap).max(8.0),
        Some(PresetSize::Fixed(px)) => px as f32 * s + 2.0 * border,
        None => (view_w * 0.5 - gap).max(8.0),
    };
    let presets = &l.preset_column_widths;
    let widths = [
        col_w(presets.first().copied()),
        col_w(l.default_column_width),
        col_w(presets.last().copied()),
    ];
    let focused = 1;
    // View offset: niri keeps the focused column on screen, centering it when configured.
    let center = match l.center_focused_column {
        niri_config::CenterFocusedColumn::Always => true,
        niri_config::CenterFocusedColumn::OnOverflow => widths[0] + widths[1] + 3.0 * gap > work.width(),
        niri_config::CenterFocusedColumn::Never => false,
    };
    let mut x = work.left() + gap;
    if center {
        x = work.center().x - widths[0] - gap - widths[1] / 2.0;
    }
    let top = work.top() + gap;
    let bottom = work.bottom() - gap;

    let painter = painter.with_clip_rect(screen);
    if hover.is_some_and(|h| work.contains(h)) && hovered.is_none() {
        hovered = Some(Part::Gap);
    }

    for (ci, w) in widths.into_iter().enumerate() {
        let col = Rect::from_min_max(pos2(x, top), pos2(x + w, bottom));
        let active_col = ci == focused;
        // Middle column holds two stacked windows; the last is tabbed.
        let windows: Vec<Rect> = if ci == 0 {
            let split = (col.top() + col.bottom()) / 2.0;
            vec![
                Rect::from_min_max(col.min, pos2(col.right(), split - gap / 2.0)),
                Rect::from_min_max(pos2(col.left(), split + gap / 2.0), col.max),
            ]
        } else {
            vec![col]
        };
        for (wi, win) in windows.iter().enumerate() {
            let active = active_col && wi == 0;
            if hover.is_some_and(|h| win.expand(border).contains(h)) {
                hovered = Some(Part::Window);
            }
            window(&painter, cfg, *win, screen, active, radius, border, s, hover, &mut hovered);
        }
        if ci == 2 && !l.tab_indicator.off {
            tab_indicator(&painter, cfg, col, screen, s, hover, &mut hovered);
        }
        x += w + gap;
    }

    painter.rect_stroke(screen, 6.0, Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color), StrokeKind::Outside);
    hovered
}

#[allow(clippy::too_many_arguments)]
fn window(
    painter: &Painter,
    cfg: &Config,
    win: Rect,
    view: Rect,
    active: bool,
    radius: f32,
    border: f32,
    s: f32,
    hover: Option<Pos2>,
    hovered: &mut Option<Part>,
) {
    let l = &cfg.layout;
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
        let (c, g) = if active { (b.active_color, b.active_gradient) } else { (b.inactive_color, b.inactive_gradient) };
        ring(painter, content, border.max(1.0), radius, c, g, view);
        if hover.is_some_and(|h| win.contains(h) && !content.contains(h)) {
            *hovered = Some(Part::Border);
        }
    }

    // Window body with a fake header bar.
    painter.rect_filled(content, radius, Color32::from_gray(if active { 52 } else { 40 }));
    let bar = Rect::from_min_size(content.min, vec2(content.width(), (28.0 * s).clamp(4.0, 18.0)));
    painter.rect_filled(
        bar,
        CornerRadius { nw: radius as u8, ne: radius as u8, sw: 0, se: 0 },
        Color32::from_gray(if active { 64 } else { 48 }),
    );
    for i in 0..4 {
        let y = bar.bottom() + (10.0 + i as f32 * 14.0) * s.max(0.35) * 2.0;
        if y + 4.0 < content.bottom() {
            let w = content.width() * [0.7, 0.5, 0.62, 0.35][i];
            painter.rect_filled(
                Rect::from_min_size(pos2(content.left() + 10.0 * s.max(0.5), y), vec2(w, 3.0)),
                1.5,
                Color32::from_gray(if active { 80 } else { 60 }),
            );
        }
    }

    // Focus ring: outside the window, only on the active one.
    if active && !l.focus_ring.off {
        let f = &l.focus_ring;
        // Keep thin rings visible at preview scale.
        let w = (f.width as f32 * s).max(1.5);
        ring(painter, win, w, outer_radius, f.active_color, f.active_gradient, view);
        if hover.is_some_and(|h| win.expand(w).contains(h) && !win.contains(h)) {
            *hovered = Some(Part::FocusRing);
        }
    }
}

fn tab_indicator(
    painter: &Painter,
    cfg: &Config,
    col: Rect,
    view: Rect,
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
    let tabs = 3;
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
