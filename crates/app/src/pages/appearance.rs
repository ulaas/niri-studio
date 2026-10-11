//! Focus ring, border, shadow and the other visual settings, with gradient editors.

use doc::{p, Val};
use egui::RichText;
use schema::{Kind, Page};

use crate::app::Ctx;
use crate::pages::{forms, layout};
use crate::widgets;

const STATES: [(&str, &str); 3] = [("active", "Active"), ("inactive", "Inactive"), ("urgent", "Urgent")];
const RELATIVE: [&str; 2] = ["window", "workspace-view"];
const INTERP: [&str; 7] = ["srgb", "srgb-linear", "oklab", "oklch shorter hue", "oklch longer hue", "oklch increasing hue", "oklch decreasing hue"];

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx) {
    widgets::heading(ui, "Appearance", "Window decorations, shadows, the cursor and the overview.");
    layout::preview_card(ui, cx, false);

    let ring_on = !cx.cfg.layout.focus_ring.off;
    let border_on = !cx.cfg.layout.border.off;
    widgets::card(ui, "Focus ring", |ui| {
        ui.weak("Drawn around the active window, outside it. Doesn't take up space.");
        decoration(ui, cx, "focus-ring", ring_on);
    });
    widgets::card(ui, "Border", |ui| {
        ui.weak("Drawn around every window. Unlike the focus ring, it takes up layout space.");
        decoration(ui, cx, "border", border_on);
    });
    widgets::card(ui, "Shadow", |ui| shadow(ui, cx));
    forms::sections(ui, cx, Page::Appearance, &[]);
}

/// Toggle written as `on`/`off` children, matching niri's conventions.
fn on_off(ui: &mut egui::Ui, cx: &mut Ctx, base: &str, effective: bool, label: &str) {
    let mut on = effective;
    let explicit = cx.doc.has(&p(&format!("{base}/on"))) || cx.doc.has(&p(&format!("{base}/off")));
    let (_, reset) = widgets::row(ui, label, "", explicit, false, |ui| widgets::switch(ui, &mut on).changed());
    let res = if reset {
        cx.doc.remove_all(&p(&format!("{base}/on"))).and_then(|_| cx.doc.remove_all(&p(&format!("{base}/off"))))
    } else if on != effective {
        // Always write the flag explicitly: for the border, any `border { }` block turns it on,
        // so merely removing `on` wouldn't turn it off.
        let (add, del) = if on { ("on", "off") } else { ("off", "on") };
        cx.doc
            .remove_all(&p(&format!("{base}/{del}")))
            .and_then(|_| cx.doc.set_flag(&p(&format!("{base}/{add}")), true))
    } else {
        Ok(())
    };
    if let Err(e) = res {
        cx.toasts.error(format!("{e:#}"));
    }
}

fn decoration(ui: &mut egui::Ui, cx: &mut Ctx, name: &str, effective: bool) {
    let base = format!("layout/{name}");
    on_off(ui, cx, &base, effective, "Enabled");
    if !effective {
        return;
    }
    let hl = cx.highlight;
    widgets::option_at(ui, cx, None, &p(&format!("{base}/width")), Kind::Float { min: 0.0, max: 32.0 }, "Width", "In logical pixels.", hl == Some("width"));
    for (state, title) in STATES {
        color_or_gradient(ui, cx, &base, state, title);
    }
}

fn color_or_gradient(ui: &mut egui::Ui, cx: &mut Ctx, base: &str, state: &str, title: &str) {
    let color_path = p(&format!("{base}/{state}-color"));
    let grad_path = p(&format!("{base}/{state}-gradient"));
    let has_grad = cx.doc.has(&grad_path);
    let id = ui.id().with((base, state));

    ui.horizontal(|ui| {
        ui.add_sized([120.0, 22.0], egui::Label::new(RichText::new(title).strong()));
        let mut grad = has_grad;
        if let Some(v) = crate::widgets::segmented(ui, ui.id().with("seg-grad"), Some(&grad), &[(false, "Solid"), (true, "Gradient")], false) {
            grad = v;
        }
        if grad != has_grad {
            let res = if grad {
                let from = cx.doc.arg(&color_path).and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_else(|| "#7fc8ff".into());
                let to = crate::color::parse(&from)
                    .map(|c| crate::color::format(crate::color::lerp(c, egui::Color32::WHITE, 0.45)))
                    .unwrap_or_else(|| "#bbddff".into());
                cx.doc
                    .set_prop(&grad_path, "from", Some(Val::Str(from)))
                    .and_then(|_| cx.doc.set_prop(&grad_path, "to", Some(Val::Str(to))))
                    .and_then(|_| cx.doc.set_prop(&grad_path, "angle", Some(Val::Int(45))))
            } else {
                cx.doc.remove_all(&grad_path)
            };
            if let Err(e) = res {
                cx.toasts.error(format!("{e:#}"));
            }
        }
    });

    ui.indent(id, |ui| {
        if !has_grad {
            let cur = cx.doc.arg(&color_path).and_then(|v| v.as_str().map(str::to_owned));
            let is_set = cur.is_some();
            let fallback = effective_color(cx.cfg, base, state);
            let (new, reset) = widgets::row(ui, "Color", "", is_set, false, |ui| widgets::color_field_or(ui, id.with("c"), cur.as_deref(), fallback));
            let res = if reset {
                cx.doc.remove_all(&color_path)
            } else if let Some(c) = new {
                cx.doc.set_arg(&color_path, c)
            } else {
                Ok(())
            };
            if let Err(e) = res {
                cx.toasts.error(format!("{e:#}"));
            }
            return;
        }

        for key in ["from", "to"] {
            let cur = cx.doc.prop(&grad_path, key).and_then(|v| v.as_str().map(str::to_owned));
            let label = if key == "from" { "From" } else { "To" };
            let (new, _) = widgets::row(ui, label, "", true, false, |ui| widgets::color_field(ui, id.with(key), cur.as_deref()));
            if let Some(c) = new {
                if let Err(e) = cx.doc.set_prop(&grad_path, key, Some(Val::Str(c))) {
                    cx.toasts.error(format!("{e:#}"));
                }
            }
        }
        let angle = cx.doc.prop(&grad_path, "angle").and_then(|v| v.as_i64()).unwrap_or(180);
        let (a, _) = widgets::row(ui, "Angle", "CSS convention: 0° points up, 90° right, 180° down.", true, false, |ui| {
            let mut a = angle;
            let r = ui.add(widgets::Slider::new(&mut a, 0..=360).suffix("°"));
            angle_dial(ui, a as f32);
            r.changed().then_some(a)
        });
        if let Some(a) = a {
            let _ = cx.doc.set_prop(&grad_path, "angle", Some(Val::Int(a)));
        }
        enum_prop(ui, cx, &grad_path, "relative-to", "Relative to", &RELATIVE, "window", id);
        enum_prop(ui, cx, &grad_path, "in", "Interpolation", &INTERP, "srgb", id);
    });
}

fn enum_prop(ui: &mut egui::Ui, cx: &mut Ctx, path: &[doc::Seg], key: &str, label: &str, values: &[&str], default: &str, id: egui::Id) {
    let cur = cx.doc.prop(path, key).and_then(|v| v.as_str().map(str::to_owned));
    let (pick, _) = widgets::row(ui, label, "", cur.is_some(), false, |ui| {
        let mut pick = None;
        egui::ComboBox::from_id_salt(id.with(key)).selected_text(cur.clone().unwrap_or_else(|| default.to_owned())).show_ui(ui, |ui| {
            for v in values {
                if ui.selectable_label(cur.as_deref() == Some(v), *v).clicked() {
                    pick = Some(*v);
                }
            }
        });
        pick
    });
    if let Some(v) = pick {
        let v = (v != default).then(|| Val::str(v));
        let _ = cx.doc.set_prop(path, key, v);
    }
}

/// A small dial showing the gradient direction.
fn angle_dial(ui: &mut egui::Ui, deg: f32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
    let c = r.center();
    let stroke = ui.visuals().widgets.inactive.fg_stroke;
    ui.painter().circle_stroke(c, 10.0, stroke);
    let a = deg.to_radians();
    let tip = c + egui::vec2(a.sin(), -a.cos()) * 9.0;
    ui.painter().arrow(c - (tip - c), (tip - c) * 2.0, egui::Stroke::new(1.5, crate::theme::pal(ui.ctx()).accent_fg()));
}

fn shadow(ui: &mut egui::Ui, cx: &mut Ctx) {
    let on = cx.cfg.layout.shadow.on;
    on_off(ui, cx, "layout/shadow", on, "Enabled");
    if !on {
        return;
    }
    let f = |cx: &mut Ctx, ui: &mut egui::Ui, name: &str, kind, label, help| {
        widgets::option_at(ui, cx, None, &p(&format!("layout/shadow/{name}")), kind, label, help, false)
    };
    f(cx, ui, "softness", Kind::Float { min: 0.0, max: 100.0 }, "Softness", "Blur radius.");
    f(cx, ui, "spread", Kind::Float { min: -50.0, max: 100.0 }, "Spread", "Grows the shadow beyond the window.");
    let offset = p("layout/shadow/offset");
    for key in ["x", "y"] {
        let cur = cx.doc.prop(&offset, key).and_then(|v| v.as_f64());
        let label = if key == "x" { "Offset x" } else { "Offset y" };
        let (v, reset) = widgets::row(ui, label, "", cur.is_some(), false, |ui| {
            // niri's defaults: x=0, y=5.
            let mut v = cur.unwrap_or(if key == "y" { 5.0 } else { 0.0 });
            ui.add(widgets::Slider::new(&mut v, -50.0..=50.0).max_decimals(1)).changed().then_some(v)
        });
        if reset {
            let _ = cx.doc.set_prop(&offset, key, None);
        } else if let Some(v) = v {
            let _ = cx.doc.set_prop(&offset, key, Some(Val::Float(v)));
        }
    }
    f(cx, ui, "draw-behind-window", Kind::Bool, "Draw behind window", "Needed for windows with transparent backgrounds.");
    f(cx, ui, "color", Kind::Color, "Color", "");
    f(cx, ui, "inactive-color", Kind::Color, "Inactive color", "For windows that aren't focused.");
}

/// niri's effective solid color for a decoration state (its default when unset).
fn effective_color(cfg: &niri_config::Config, base: &str, state: &str) -> Option<egui::Color32> {
    let l = &cfg.layout;
    let (a, i, u) = match base {
        "layout/focus-ring" => (l.focus_ring.active_color, l.focus_ring.inactive_color, l.focus_ring.urgent_color),
        "layout/border" => (l.border.active_color, l.border.inactive_color, l.border.urgent_color),
        _ => return None,
    };
    let c = match state {
        "active" => a,
        "inactive" => i,
        _ => u,
    };
    Some(crate::color::from_niri(c))
}
