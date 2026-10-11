//! Shared widgets: option rows generated from the schema, cards, switches, color fields.

use doc::{p, NodeRef, Seg, Val};
use egui::{Color32, RichText, Sense, Ui};
use schema::{Kind, Opt};

use crate::app::Ctx;
use crate::theme::{self, icons};
use crate::color;

const LABEL_WIDTH: f32 = 250.0;

/// A titled card grouping related settings.
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_frame(ui, |ui| {
        ui.label(RichText::new(title).strong().size(15.5).color(theme::pal(ui.ctx()).text));
        ui.add_space(6.0);
        add(ui)
    })
}

/// The card surface without a title: a raised panel with a hairline border and soft shadow.
pub fn card_frame<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = theme::pal(ui.ctx());
    let radius = ui.visuals().window_corner_radius;
    ui.add_space(6.0);
    // Frame margins and stroke sit outside the content; size the content so the whole card fits.
    let margin = 18.0;
    let width = (ui.available_width() - 2.0 * margin - 4.0).max(100.0);
    let r = egui::Frame::new()
        .fill(p.surface)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(radius)
        .inner_margin(margin)
        .shadow(egui::epaint::Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(if p.dark { 40 } else { 10 }) })
        .show(ui, |ui| {
            ui.set_width(width);
            add(ui)
        })
        .inner;
    ui.add_space(6.0);
    r
}

/// A small status dot (filled or hollow); egui's fonts lack ● and ○.
pub fn dot(ui: &mut Ui, filled: bool, color: Color32) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
    if filled {
        ui.painter().circle_filled(rect.center(), 3.5, color);
    } else {
        ui.painter().circle_stroke(rect.center(), 3.0, egui::Stroke::new(1.0, color));
    }
    r
}

/// A toggle switch with a sliding knob.
pub fn switch(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let p = theme::pal(ui.ctx());
    let size = egui::vec2(38.0, 22.0);
    let (rect, mut r) = ui.allocate_exact_size(size, Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, ""));
    let t = ui.ctx().animate_bool_responsive(r.id, *on);
    let radius = rect.height() / 2.0;
    let off = if r.hovered() { p.control_active() } else { p.control_hover() };
    let bg = color::lerp(off, p.accent, t);
    let painter = ui.painter();
    if r.has_focus() {
        painter.rect_stroke(rect.expand(2.0), radius + 2.0, egui::Stroke::new(2.0, p.accent.gamma_multiply(0.5)), egui::StrokeKind::Outside);
    }
    painter.rect_filled(rect, radius, bg);
    let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
    let c = egui::pos2(x, rect.center().y);
    painter.circle_filled(c + egui::vec2(0.0, 1.0), radius - 3.0, Color32::from_black_alpha(50));
    let knob = color::lerp(if p.dark { p.text } else { Color32::WHITE }, p.on_accent(), t);
    painter.circle_filled(c, radius - 3.0, knob);
    r
}

/// A borderless button showing a Phosphor icon, with a background on hover.
pub fn icon_button(ui: &mut Ui, icon: &str, tip: &str) -> egui::Response {
    let r = ui.add(egui::Button::new(RichText::new(icon).size(16.0)).frame_when_inactive(false).min_size(egui::vec2(30.0, 28.0)));
    if tip.is_empty() { r } else { r.on_hover_text(tip) }
}

/// A small icon button, as used for per-row actions.
pub fn small_icon_button(ui: &mut Ui, icon: &str, tip: &str) -> egui::Response {
    let p = theme::pal(ui.ctx());
    ui.add(egui::Button::new(RichText::new(icon).size(14.0).color(p.muted)).frame_when_inactive(false).min_size(egui::vec2(24.0, 22.0)))
        .on_hover_text(tip)
}

/// A frameless icon button for use with `add_enabled`.
pub fn icon_only(icon: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(icon).size(14.0)).frame_when_inactive(false).min_size(egui::vec2(24.0, 22.0))
}

/// The main call to action: filled with the accent color.
pub fn primary_button(ui: &mut Ui, enabled: bool, text: impl Into<String>) -> egui::Response {
    let p = theme::pal(ui.ctx());
    let text = text.into();
    let (fill, fg) = if enabled { (p.accent, p.on_accent()) } else { (p.control, p.faint) };
    ui.add_enabled(enabled, egui::Button::new(RichText::new(text).strong().color(fg)).fill(fill).stroke(egui::Stroke::NONE).min_size(egui::vec2(0.0, 30.0)))
}

/// A rounded label with a tinted background, e.g. a status or a count.
pub fn pill(ui: &mut Ui, icon: &str, text: &str, color: Color32) -> egui::Response {
    let p = theme::pal(ui.ctx());
    let label = if icon.is_empty() { text.to_owned() } else if text.is_empty() { icon.to_owned() } else { format!("{icon} {text}") };
    let galley = ui.painter().layout_no_wrap(label, egui::FontId::proportional(12.5), color);
    let size = galley.size() + egui::vec2(18.0, 8.0);
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    let fill = p.tint(color, if r.hovered() { 0.22 } else { 0.14 });
    ui.painter().rect_filled(rect, rect.height() / 2.0, fill);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    r
}

/// A keyboard key cap, e.g. `Ctrl`.
pub fn kbd(ui: &mut Ui, key: &str) {
    let p = theme::pal(ui.ctx());
    let galley = ui.painter().layout_no_wrap(key.to_owned(), egui::FontId::proportional(11.0), p.muted);
    let size = galley.size() + egui::vec2(10.0, 4.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect(rect, 4.0, p.control, egui::Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, p.muted);
}

/// Background for a clickable list row: tinted when selected, faint on hover.
pub fn list_row_bg(ui: &Ui, r: &egui::Response, rect: egui::Rect, selected: bool) {
    let p = theme::pal(ui.ctx());
    let radius = ui.visuals().widgets.inactive.corner_radius;
    if selected {
        ui.painter().rect(rect, radius, p.tint(p.accent, 0.16), egui::Stroke::new(1.0, p.tint(p.accent, 0.45)), egui::StrokeKind::Inside);
    } else if r.hovered() {
        ui.painter().rect_filled(rect, radius, theme::mix(p.surface, p.text, 0.045));
    }
}

/// Paints a key combination such as `Mod+Shift+T` as key caps starting at `left`; returns the
/// x coordinate where they end.
pub fn keycaps(ui: &Ui, left: egui::Pos2, combo: &str, color: Color32) -> f32 {
    let p = theme::pal(ui.ctx());
    let painter = ui.painter();
    let mut x = left.x;
    for key in combo.split('+').filter(|k| !k.is_empty()) {
        let galley = painter.layout_no_wrap(key.to_owned(), egui::FontId::monospace(12.0), color);
        let size = egui::vec2(galley.size().x + 12.0, 20.0);
        let cap = egui::Rect::from_min_size(egui::pos2(x, left.y - size.y / 2.0), size);
        painter.rect_filled(cap.translate(egui::vec2(0.0, 1.5)), 4.0, p.border);
        painter.rect(cap, 4.0, p.control, egui::Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
        painter.galley(cap.center() - galley.size() / 2.0, galley, color);
        x = cap.right() + 4.0;
    }
    x
}

/// A checkbox in the app's style: a rounded box filled with the accent and a drawn check when on.
pub fn checkbox(ui: &mut Ui, on: &mut bool, text: impl Into<egui::WidgetText>) -> egui::Response {
    let p = theme::pal(ui.ctx());
    let text = text.into();
    let galley = text.into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body);
    let size = egui::vec2(18.0 + 8.0 + galley.size().x, ui.spacing().interact_size.y.max(galley.size().y));
    let (rect, mut r) = ui.allocate_exact_size(size, Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, galley.text()));
    let t = ui.ctx().animate_bool_responsive(r.id, *on);
    let bx = egui::Rect::from_center_size(egui::pos2(rect.left() + 9.0, rect.center().y), egui::vec2(18.0, 18.0));
    let painter = ui.painter();
    let off_fill = if r.hovered() { p.control_active() } else { p.control_hover() };
    painter.rect(
        bx,
        5.0,
        color::lerp(off_fill, p.accent, t),
        egui::Stroke::new(1.0, color::lerp(p.border_strong(), p.accent, t)),
        egui::StrokeKind::Inside,
    );
    if t > 0.0 {
        // The check draws itself in as the box fills.
        let a = bx.left_center() + egui::vec2(4.5, 0.5);
        let b = bx.center_bottom() + egui::vec2(-1.5, -5.0);
        let c = bx.right_top() + egui::vec2(-4.0, 5.0);
        let stroke = egui::Stroke::new(2.0, p.on_accent());
        let first = t.min(0.5) * 2.0;
        painter.line_segment([a, a + (b - a) * first], stroke);
        if t > 0.5 {
            painter.line_segment([b, b + (c - b) * ((t - 0.5) * 2.0)], stroke);
        }
    }
    if r.has_focus() {
        painter.rect_stroke(bx.expand(2.0), 7.0, egui::Stroke::new(2.0, p.accent.gamma_multiply(0.5)), egui::StrokeKind::Outside);
    }
    let color = if ui.is_enabled() { p.text } else { p.faint };
    painter.galley(egui::pos2(bx.right() + 8.0, rect.center().y - galley.size().y / 2.0), galley, color);
    r
}

/// A slider in the app's style: a thin track filled with the accent up to a round knob, and an
/// editable value beside it. Mirrors the parts of `egui::Slider`'s builder the pages use.
pub struct Slider<'a, N: egui::emath::Numeric> {
    value: &'a mut N,
    range: std::ops::RangeInclusive<N>,
    suffix: String,
    max_decimals: Option<usize>,
    fixed_decimals: Option<usize>,
    clamp: bool,
    log: bool,
}

impl<'a, N: egui::emath::Numeric> Slider<'a, N> {
    pub fn new(value: &'a mut N, range: std::ops::RangeInclusive<N>) -> Self {
        Self { value, range, suffix: String::new(), max_decimals: None, fixed_decimals: None, clamp: true, log: false }
    }
    pub fn suffix(mut self, s: impl Into<String>) -> Self {
        self.suffix = s.into();
        self
    }
    pub fn max_decimals(mut self, n: usize) -> Self {
        self.max_decimals = Some(n);
        self
    }
    pub fn fixed_decimals(mut self, n: usize) -> Self {
        self.fixed_decimals = Some(n);
        self
    }
    /// `Never` lets the value box take values past the track's range, as niri does.
    pub fn clamping(mut self, c: egui::SliderClamping) -> Self {
        self.clamp = !matches!(c, egui::SliderClamping::Never);
        self
    }
    pub fn logarithmic(mut self, on: bool) -> Self {
        self.log = on;
        self
    }

    fn decimals(&self) -> Option<usize> {
        if N::INTEGRAL { Some(0) } else { self.fixed_decimals.or(self.max_decimals) }
    }
}

impl<N: egui::emath::Numeric> egui::Widget for Slider<'_, N> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let p = theme::pal(ui.ctx());
        let (min, max) = (self.range.start().to_f64(), self.range.end().to_f64());
        let log = self.log && min > 0.0 && max > min;
        let to_t = |v: f64| -> f64 {
            let t = if log { (v / min).ln() / (max / min).ln() } else { (v - min) / (max - min).max(f64::EPSILON) };
            t.clamp(0.0, 1.0)
        };
        let from_t = |t: f64| -> f64 { if log { min * (max / min).powf(t) } else { min + t * (max - min) } };
        let decimals = self.decimals();
        let round = |v: f64| match decimals {
            Some(d) => {
                let m = 10f64.powi(d as i32);
                (v * m).round() / m
            }
            None => v,
        };
        let mut v = self.value.to_f64();

        let inner = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let size = egui::vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
            let (rect, mut r) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let knob_r = 8.0;
            let track = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width() - 2.0 * knob_r, 4.0));
            if let Some(pos) = r.interact_pointer_pos() {
                if r.dragged() || r.clicked() {
                    let t = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) as f64;
                    let nv = round(from_t(t));
                    if nv != v {
                        v = nv;
                        r.mark_changed();
                    }
                }
            }
            if r.has_focus() {
                let (left, right) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
                if left || right {
                    let step = if N::INTEGRAL { 1.0 } else { (max - min) / 100.0 };
                    let nv = if log {
                        from_t((to_t(v) + if right { 0.01 } else { -0.01 }).clamp(0.0, 1.0))
                    } else {
                        (v + if right { step } else { -step }).clamp(min, max)
                    };
                    v = round(nv);
                    r.mark_changed();
                }
            }
            r.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), v, ""));

            let enabled = ui.is_enabled();
            let t = to_t(v) as f32;
            let x = egui::lerp(track.left()..=track.right(), t);
            let hot = ui.ctx().animate_bool_responsive(r.id.with("hot"), enabled && (r.hovered() || r.dragged() || r.has_focus()));
            let painter = ui.painter();
            let accent = if enabled { p.accent } else { p.faint };
            painter.rect_filled(track, 2.0, p.control_active());
            painter.rect_filled(egui::Rect::from_min_max(track.left_top(), egui::pos2(x, track.bottom())), 2.0, accent);
            let c = egui::pos2(x, rect.center().y);
            if hot > 0.0 {
                painter.circle_filled(c, knob_r + 5.0 * hot, accent.gamma_multiply(0.18 * hot));
            }
            painter.circle_filled(c + egui::vec2(0.0, 1.0), knob_r, Color32::from_black_alpha(60));
            painter.circle(c, knob_r, if p.dark { p.text } else { Color32::WHITE }, egui::Stroke::new(2.0, accent), );

            let mut dv = egui::DragValue::new(&mut v).speed(if log { 0.0 } else { (max - min) / 300.0 }).suffix(self.suffix.clone());
            if self.clamp {
                dv = dv.range(min..=max);
            }
            if let Some(d) = decimals {
                dv = if self.fixed_decimals.is_some() || N::INTEGRAL { dv.fixed_decimals(d) } else { dv.max_decimals(d) };
            }
            let dr = ui.add_sized([62.0, size.y], dv);
            r.union(dr)
        });
        let mut r = inner.inner;
        if v != self.value.to_f64() {
            *self.value = N::from_f64(v);
            r.mark_changed();
        }
        r
    }
}

/// A segmented control: options side by side in one pill, the selected one raised on a sliding
/// highlight. `dim` draws the highlight faintly, for a value that is niri's default rather than
/// set. Returns the option clicked, if any.
pub fn segmented<T: PartialEq + Clone>(ui: &mut Ui, id: egui::Id, selected: Option<&T>, options: &[(T, &str)], dim: bool) -> Option<T> {
    let p = theme::pal(ui.ctx());
    let font = egui::FontId::proportional(13.5);
    let pad = 14.0;
    let h = ui.spacing().interact_size.y;
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, l)| ui.painter().layout_no_wrap((*l).to_owned(), font.clone(), p.text).size().x + 2.0 * pad)
        .collect();
    let total = widths.iter().sum::<f32>() + 6.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(total, h), Sense::hover());
    let radius = ui.visuals().widgets.inactive.corner_radius.nw as f32 + 2.0;
    ui.painter().rect(rect, radius, p.control, egui::Stroke::new(1.0, theme::mix(p.control, p.border, 0.6)), egui::StrokeKind::Inside);

    let mut x = rect.left() + 3.0;
    let mut slots = Vec::with_capacity(options.len());
    for w in &widths {
        slots.push(egui::Rect::from_min_size(egui::pos2(x, rect.top() + 3.0), egui::vec2(*w, h - 6.0)));
        x += w;
    }
    let sel = selected.and_then(|s| options.iter().position(|(v, _)| v == s));
    if let Some(i) = sel {
        let target = slots[i];
        let left = ui.ctx().animate_value_with_time(id.with("x"), target.left(), 0.16);
        let width = ui.ctx().animate_value_with_time(id.with("w"), target.width(), 0.16);
        let hl = egui::Rect::from_min_size(egui::pos2(left, target.top()), egui::vec2(width, target.height()));
        let fill = if dim { theme::mix(p.control, p.surface, 0.5) } else if p.dark { p.control_active() } else { p.surface };
        if !dim {
            ui.painter().rect_filled(hl.translate(egui::vec2(0.0, 1.0)), radius - 2.0, Color32::from_black_alpha(if p.dark { 70 } else { 22 }));
        }
        ui.painter().rect_filled(hl, radius - 2.0, fill);
    }
    let mut out = None;
    for (i, ((value, label), slot)) in options.iter().zip(&slots).enumerate() {
        let r = ui.interact(*slot, id.with(i), Sense::click());
        let is_sel = sel == Some(i);
        let color = if is_sel && !dim { p.text } else if r.hovered() { p.text } else { p.muted };
        ui.painter().text(slot.center(), egui::Align2::CENTER_CENTER, *label, font.clone(), color);
        if r.clicked() {
            out = Some(value.clone());
        }
    }
    out
}

/// An uppercase caption above a group of items.
pub fn caption(ui: &mut Ui, text: &str) {
    let p = theme::pal(ui.ctx());
    ui.label(RichText::new(text.to_uppercase()).size(10.5).strong().color(p.faint).extra_letter_spacing(0.8));
}

/// Row layout: label on the left (with help on hover), control on the right, reset at the end.
pub fn row<R>(
    ui: &mut Ui,
    label: &str,
    help: &str,
    is_set: bool,
    highlight: bool,
    control: impl FnOnce(&mut Ui) -> R,
) -> (R, bool) {
    let mut reset = false;
    let bg = ui.painter().add(egui::Shape::Noop);
    let inner = ui
        .horizontal(|ui| {
            let label_width = LABEL_WIDTH.min(ui.available_width() * 0.4);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(label_width, ui.spacing().interact_size.y), Sense::hover());
            let text = if is_set { RichText::new(label) } else { RichText::new(label).weak() };
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
            if is_set {
                let c = theme::pal(child.ctx()).accent_fg();
                dot(&mut child, true, c).on_hover_text("Set in your config");
            } else {
                let c = child.visuals().weak_text_color();
                dot(&mut child, false, c).on_hover_text("Not set: niri's default applies");
            }
            let l = child.label(text);
            if !help.is_empty() {
                l.on_hover_text(help);
            }
            let out = control(ui);
            if is_set && small_icon_button(ui, icons::ARROW_COUNTER_CLOCKWISE, "Remove from config (back to niri's default)").clicked() {
                reset = true;
            }
            out
        });
    // A faint band behind the hovered row ties the label to its control.
    let band = inner.response.rect.expand2(egui::vec2(8.0, 2.0));
    if ui.rect_contains_pointer(band) {
        let p = theme::pal(ui.ctx());
        ui.painter().set(bg, egui::Shape::rect_filled(band, 6.0, theme::mix(p.surface, p.text, 0.035)));
    }
    if highlight {
        ui.painter().rect_stroke(
            inner.response.rect.expand(3.0),
            4.0,
            egui::Stroke::new(2.0, theme::pal(ui.ctx()).accent_fg()),
            egui::StrokeKind::Outside,
        );
        inner.response.scroll_to_me(Some(egui::Align::Center));
    }
    (inner.inner, reset)
}

/// Renders an option from the schema, reading and writing the document.
pub fn option(ui: &mut Ui, cx: &mut Ctx, o: &Opt) {
    option_at(ui, cx, None, &p(o.path), o.kind, o.label, o.help, cx.highlight == Some(o.path));
}

/// Renders a typed setting at `path` relative to `base`.
#[allow(clippy::too_many_arguments)]
pub fn option_at(
    ui: &mut Ui,
    cx: &mut Ctx,
    base: Option<&NodeRef>,
    path: &[Seg],
    kind: Kind,
    label: &str,
    help: &str,
    highlight: bool,
) {
    let doc = &mut *cx.doc;
    let is_set = doc.has_in(base, path);
    let current = doc.arg_in(base, path);
    // niri's default, shown while the option is unset (only for top-level schema paths).
    let (default, default_note) = match base.is_none().then(|| schema::default_of(&doc::path::display(path))).flatten() {
        Some((text, note)) => (parse_default(text), note),
        None => (None, ""),
    };
    let unset_note = |ui: &mut Ui| {
        if !is_set {
            let text = if default_note.is_empty() { "default".to_owned() } else { default_note.to_owned() };
            ui.weak(text);
        }
    };
    let id = ui.id().with(doc::path::display(path)).with(base.map(|b| (b.file, b.idx.clone())));
    let mut result: Option<Edit> = None;

    let (_, reset) = row(ui, label, help, is_set, highlight, |ui| match kind {
        Kind::Flag => {
            let mut on = is_set;
            if switch(ui, &mut on).changed() {
                result = Some(Edit::Flag(on));
            }
        }
        Kind::Bool => {
            let mut on = current.as_ref().or(default.as_ref()).and_then(Val::as_bool).unwrap_or(false);
            if switch(ui, &mut on).changed() {
                result = Some(Edit::Set(Val::Bool(on)));
            }
        }
        Kind::Int { min, max } => {
            let mut v = current.as_ref().or(default.as_ref()).and_then(Val::as_i64).unwrap_or(min.max(0).min(max));
            let r = ui.add(Slider::new(&mut v, min..=max).clamping(egui::SliderClamping::Never));
            if r.changed() {
                result = Some(Edit::Set(Val::Int(v)));
            }
            unset_note(ui);
        }
        Kind::Float { min, max } => {
            let fallback = if (min..=max).contains(&1.0) && max <= 10.0 { 1.0 } else { min.max(0.0) };
            let mut v = current.as_ref().or(default.as_ref()).and_then(Val::as_f64).unwrap_or(fallback);
            let r = ui.add(
                Slider::new(&mut v, min..=max)
                    .clamping(egui::SliderClamping::Never)
                    .max_decimals(if max - min > 50.0 { 1 } else { 3 }),
            );
            if r.changed() {
                result = Some(Edit::Set(Val::Float(v)));
            }
            unset_note(ui);
        }
        Kind::Str | Kind::Path => {
            let mut s = current.as_ref().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            let hint = default.as_ref().and_then(|d| d.as_str().map(str::to_owned)).unwrap_or_default();
            let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(280.0).id(id).hint_text(hint));
            if r.changed() {
                result = Some(Edit::Set(Val::Str(s)));
            }
        }
        Kind::Enum(values) => {
            let cur = current.as_ref().and_then(|v| v.as_str().map(str::to_owned));
            let default_label = match default.as_ref().and_then(|d| d.as_str()) {
                Some(d) => format!("default ({d})"),
                None => "default".to_owned(),
            };
            // A few short choices read best side by side; niri's default shows faintly when unset.
            if values.len() <= 4 && values.iter().map(|v| v.len()).sum::<usize>() <= 36 {
                let default_value = default.as_ref().and_then(|d| d.as_str().map(str::to_owned));
                let shown = cur.clone().or(default_value);
                let options: Vec<(String, &str)> = values.iter().map(|v| ((*v).to_owned(), *v)).collect();
                if let Some(v) = segmented(ui, id, shown.as_ref(), &options, cur.is_none()) {
                    result = Some(Edit::Set(Val::str(v)));
                }
                unset_note(ui);
            } else {
            egui::ComboBox::from_id_salt(id)
                .selected_text(cur.clone().unwrap_or_else(|| default_label.clone()))
                .show_ui(ui, |ui| {
                    if ui.selectable_label(cur.is_none(), &default_label).clicked() {
                        result = Some(Edit::Remove);
                    }
                    for v in values {
                        if ui.selectable_label(cur.as_deref() == Some(v), *v).clicked() {
                            result = Some(Edit::Set(Val::str(*v)));
                        }
                    }
                });
            }
        }
        Kind::Color => {
            let s = current.as_ref().and_then(|v| v.as_str().map(str::to_owned));
            let fallback = default.as_ref().and_then(|d| d.as_str()).and_then(color::parse);
            if let Some(new) = color_field_or(ui, id, s.as_deref(), fallback) {
                result = Some(Edit::Set(Val::Str(new)));
            }
        }
    });
    if reset {
        result = Some(Edit::Remove);
    }

    let res = match result {
        None => return,
        Some(Edit::Flag(on)) => doc.set_flag_in(base, path, on),
        Some(Edit::Set(v)) => doc.set_args_in(base, path, &[v]),
        Some(Edit::Remove) => doc.remove_all_in(base, path),
    };
    if let Err(e) = res {
        cx.toasts.error(format!("{e:#}"));
    }
}

/// Parses a default from the schema's KDL text form.
fn parse_default(text: &str) -> Option<Val> {
    if let Some(s) = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return Some(Val::str(s));
    }
    match text {
        "true" => Some(Val::Bool(true)),
        "false" => Some(Val::Bool(false)),
        t if t.contains('.') => t.parse().ok().map(Val::Float),
        t => t.parse().ok().map(Val::Int),
    }
}

enum Edit {
    Flag(bool),
    Set(Val),
    Remove,
}

/// A color swatch with picker plus a text field accepting any CSS color. Returns the new string.
pub fn color_field(ui: &mut Ui, id: egui::Id, current: Option<&str>) -> Option<String> {
    color_field_or(ui, id, current, None)
}

/// Like [`color_field`]; when unset, the swatch shows `fallback` (niri's effective color).
pub fn color_field_or(ui: &mut Ui, id: egui::Id, current: Option<&str>, fallback: Option<Color32>) -> Option<String> {
    let parsed = current.and_then(color::parse);
    let mut c = parsed.or(fallback).unwrap_or(Color32::from_rgb(127, 200, 255));
    let mut out = None;
    let mut rgba = c.to_srgba_unmultiplied();
    if ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed() {
        c = Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
        out = Some(color::format(c));
    }
    // Text keeps whatever the user typed (e.g. "red") until it parses.
    let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| current.unwrap_or("").to_owned());
    let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(110.0).hint_text("#rrggbb"));
    if r.changed() {
        if color::parse(&text).is_some() {
            out = Some(text.clone());
        }
        ui.data_mut(|d| d.insert_temp(id, text));
    } else if !r.has_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    if current.is_some() && parsed.is_none() {
        ui.colored_label(ui.visuals().warn_fg_color, icons::WARNING).on_hover_text("Not a color niri understands");
    }
    out
}

/// Page heading with an optional subtitle.
pub fn heading(ui: &mut Ui, title: &str, subtitle: &str) {
    let p = theme::pal(ui.ctx());
    ui.add_space(10.0);
    ui.label(RichText::new(title).size(26.0).strong().color(p.text));
    if !subtitle.is_empty() {
        ui.add_space(2.0);
        ui.label(RichText::new(subtitle).size(14.0).color(p.muted));
    }
    ui.add_space(10.0);
}

/// Splits a command line into arguments, honoring single and double quotes and backslashes.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') | (None, '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
                in_word = true;
            }
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                in_word = true;
            }
            (None, c) if c.is_whitespace() => {
                if in_word {
                    out.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            (None, c) => {
                cur.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        out.push(cur);
    }
    out
}

/// Joins arguments into a command line that [`split_args`] reads back identically.
pub fn join_args(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if !a.is_empty() && !a.chars().any(|c| c.is_whitespace() || "\"'\\".contains(c)) {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', "'\"'\"'"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_round_trip() {
        let args = vec!["noctalia".to_owned(), "msg".into(), "a b".into(), "it's".into(), String::new()];
        assert_eq!(split_args(&join_args(&args)), args);
        assert_eq!(split_args(r#"sh -c "echo \"hi\"""#), vec!["sh", "-c", "echo \"hi\""]);
    }
}

/// Shortens text to roughly fit `width` points, adding an ellipsis.
pub fn ellipsize(s: &str, width: f32) -> String {
    let max = ((width / 7.2) as usize).max(4);
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>().trim_end())
    }
}
