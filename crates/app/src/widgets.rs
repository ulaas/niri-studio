//! Shared widgets: option rows generated from the schema, cards, switches, color fields.

use doc::{p, NodeRef, Seg, Val};
use egui::{Color32, RichText, Sense, Ui};
use schema::{Kind, Opt};

use crate::app::Ctx;
use crate::color;

const LABEL_WIDTH: f32 = 250.0;

/// A titled card grouping related settings.
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(4.0);
    // Frame margins and stroke sit outside the content; size the content so the whole card fits.
    let width = (ui.available_width() - 2.0 * 12.0 - 4.0).max(100.0);
    let r = egui::Frame::group(ui.style())
        .inner_margin(12.0)
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.set_width(width);
            ui.label(RichText::new(title).strong().size(15.0));
            ui.add_space(4.0);
            add(ui)
        })
        .inner;
    ui.add_space(4.0);
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

/// An iOS-style switch.
pub fn switch(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let size = egui::vec2(36.0, 20.0);
    let (rect, mut r) = ui.allocate_exact_size(size, Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(r.id, *on);
    let v = ui.style().interact_selectable(&r, *on);
    let radius = rect.height() / 2.0;
    let bg = color::lerp(ui.visuals().widgets.inactive.bg_fill, ui.visuals().selection.bg_fill, t);
    ui.painter().rect(rect, radius, bg, v.bg_stroke, egui::StrokeKind::Inside);
    let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), radius - 3.0, v.fg_stroke.color);
    r
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
    let inner = ui
        .horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(LABEL_WIDTH, 22.0), Sense::hover());
            let text = if is_set { RichText::new(label) } else { RichText::new(label).weak() };
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
            if is_set {
                let c = child.visuals().selection.stroke.color;
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
            if is_set && ui.small_button("⟲").on_hover_text("Remove from config (back to niri's default)").clicked() {
                reset = true;
            }
            out
        });
    if highlight {
        ui.painter().rect_stroke(
            inner.response.rect.expand(3.0),
            4.0,
            egui::Stroke::new(2.0, ui.visuals().selection.stroke.color),
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
            let mut on = current.as_ref().and_then(Val::as_bool).unwrap_or(false);
            if switch(ui, &mut on).changed() {
                result = Some(Edit::Set(Val::Bool(on)));
            }
        }
        Kind::Int { min, max } => {
            let mut v = current.as_ref().and_then(Val::as_i64).unwrap_or(min.max(0).min(max));
            let r = ui.add(egui::Slider::new(&mut v, min..=max).clamping(egui::SliderClamping::Never));
            if r.changed() {
                result = Some(Edit::Set(Val::Int(v)));
            }
        }
        Kind::Float { min, max } => {
            let fallback = if (min..=max).contains(&1.0) && max <= 10.0 { 1.0 } else { min.max(0.0) };
            let mut v = current.as_ref().and_then(Val::as_f64).unwrap_or(fallback);
            let r = ui.add(
                egui::Slider::new(&mut v, min..=max)
                    .clamping(egui::SliderClamping::Never)
                    .max_decimals(if max - min > 50.0 { 1 } else { 3 }),
            );
            if r.changed() {
                result = Some(Edit::Set(Val::Float(v)));
            }
        }
        Kind::Str | Kind::Path => {
            let mut s = current.as_ref().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(280.0).id(id));
            if r.changed() {
                result = Some(Edit::Set(Val::Str(s)));
            }
        }
        Kind::Enum(values) => {
            let cur = current.as_ref().and_then(|v| v.as_str().map(str::to_owned));
            egui::ComboBox::from_id_salt(id)
                .selected_text(cur.clone().unwrap_or_else(|| "default".into()))
                .show_ui(ui, |ui| {
                    if ui.selectable_label(cur.is_none(), "default").clicked() {
                        result = Some(Edit::Remove);
                    }
                    for v in values {
                        if ui.selectable_label(cur.as_deref() == Some(v), *v).clicked() {
                            result = Some(Edit::Set(Val::str(*v)));
                        }
                    }
                });
        }
        Kind::Color => {
            let s = current.as_ref().and_then(|v| v.as_str().map(str::to_owned));
            if let Some(new) = color_field(ui, id, s.as_deref()) {
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
        ui.colored_label(ui.visuals().warn_fg_color, "⚠").on_hover_text("Not a color niri understands");
    }
    out
}

/// Page heading with an optional subtitle.
pub fn heading(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(title).size(22.0).strong());
    if !subtitle.is_empty() {
        ui.weak(subtitle);
    }
    ui.add_space(6.0);
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
