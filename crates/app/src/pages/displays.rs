//! Displays: drag monitors into place, pick modes, scale and rotation, with a timed revert.

use doc::{NodeRef, Seg, Val};
use egui::{pos2, vec2, Rect, RichText, Sense, Stroke, StrokeKind};
use niri_ipc::Output;
use schema::Kind;

use crate::theme::icons;
use crate::app::Ctx;
use crate::widgets;

const TRANSFORMS: [&str; 8] = ["normal", "90", "180", "270", "flipped", "flipped-90", "flipped-180", "flipped-270"];
/// Snap distance in logical pixels.
const SNAP: f32 = 48.0;

#[derive(Default)]
pub struct State {
    selected: Option<String>,
    /// Output being dragged and its current logical position.
    drag: Option<(String, egui::Pos2)>,
}

/// The config node for an output, matched by connector or by "make model serial" like niri.
fn output_node(cx: &Ctx, o: &Output) -> Option<NodeRef> {
    let full = format!("{} {} {}", o.make, o.model, o.serial.as_deref().unwrap_or("Unknown"));
    cx.doc.find_all(&[Seg::Name("output".into())]).into_iter().rev().find(|r| {
        cx.doc.node(r).and_then(|n| doc::node_args(n).first().cloned()).and_then(|v| v.as_str().map(str::to_owned)).is_some_and(|name| {
            name.eq_ignore_ascii_case(&o.name) || name.eq_ignore_ascii_case(&full)
        })
    })
}

fn ensure_output(cx: &mut Ctx, o: &Output) -> anyhow::Result<NodeRef> {
    match output_node(cx, o) {
        Some(r) => Ok(r),
        None => cx.doc.ensure(&[Seg::Keyed("output".into(), o.name.clone())]),
    }
}

fn mode_string(m: &niri_ipc::Mode) -> String {
    format!("{}x{}@{}.{:03}", m.width, m.height, m.refresh_rate / 1000, m.refresh_rate % 1000)
}

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State) {
    widgets::heading(ui, "Displays", "Drag monitors to arrange them. Applying keeps the change for 15 seconds unless you confirm it.");
    let outputs = cx.live.state().outputs.clone();
    if outputs.is_empty() {
        widgets::card(ui, "No live outputs", |ui| {
            ui.weak("niri isn't reachable, so monitors can't be listed. Output settings can still be edited in the raw config.");
        });
        return;
    }
    if st.selected.as_ref().is_none_or(|s| !outputs.iter().any(|o| &o.name == s)) {
        st.selected = Some(outputs[0].name.clone());
    }

    widgets::card(ui, "Arrangement", |ui| arrangement(ui, cx, st, &outputs));

    ui.horizontal(|ui| {
        let dirty = cx.doc.is_dirty();
        if ui
            .add_enabled(dirty, egui::Button::new(RichText::new(format!("{}  Apply display changes…", icons::MONITOR)).strong()))
            .on_hover_text("Saves the config; you have 15 seconds to keep the result or it reverts")
            .clicked()
        {
            *cx.request_trial = true;
        }
        if ui.button(format!("{} Refresh from niri", icons::ARROW_CLOCKWISE)).clicked() {
            cx.live.refresh_outputs();
        }
    });

    let Some(o) = outputs.iter().find(|o| Some(&o.name) == st.selected.as_ref()).cloned() else { return };
    let title = format!("{}  —  {} {}", o.name, o.make, o.model);
    widgets::card(ui, &title, |ui| settings(ui, cx, &o));
}

fn arrangement(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State, outputs: &[Output]) {
    // Logical rects, with the dragged one at its drag position.
    let mut rects: Vec<(String, Rect)> = outputs
        .iter()
        .filter_map(|o| {
            let l = o.logical?;
            let mut min = pos2(l.x as f32, l.y as f32);
            if let Some((name, p)) = &st.drag {
                if *name == o.name {
                    min = *p;
                }
            }
            Some((o.name.clone(), Rect::from_min_size(min, vec2(l.width as f32, l.height as f32))))
        })
        .collect();
    let disabled: Vec<&Output> = outputs.iter().filter(|o| o.logical.is_none()).collect();

    let bounds = rects.iter().fold(Rect::NOTHING, |b, (_, r)| b.union(*r)).expand(400.0);
    let width = ui.available_width();
    let (resp, painter) = ui.allocate_painter(vec2(width, 320.0), Sense::hover());
    let canvas = resp.rect.shrink(16.0);
    let scale = (canvas.width() / bounds.width()).min(canvas.height() / bounds.height());
    let offset = canvas.center() - bounds.center().to_vec2() * scale;
    let to_screen = |p: egui::Pos2| pos2(p.x * scale + offset.x, p.y * scale + offset.y);
    painter.rect_filled(resp.rect, 8.0, ui.visuals().extreme_bg_color);

    let accent = ui.visuals().selection.stroke.color;
    let mut release: Option<String> = None;
    for (i, (name, r)) in rects.clone().into_iter().enumerate() {
        let sr = Rect::from_min_max(to_screen(r.min), to_screen(r.max));
        let id = resp.id.with(("out", &name));
        let h = ui.interact(sr, id, Sense::click_and_drag());
        let selected = st.selected.as_deref() == Some(name.as_str());
        let fill = if selected { accent.gamma_multiply(0.35) } else { ui.visuals().widgets.inactive.bg_fill };
        painter.rect_filled(sr, 6.0, fill);
        painter.rect_stroke(sr, 6.0, Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { accent } else { ui.visuals().widgets.inactive.fg_stroke.color }), StrokeKind::Inside);
        let o = &outputs.iter().find(|o| o.name == name).unwrap();
        let l = o.logical.unwrap();
        painter.text(sr.center() - vec2(0.0, 10.0), egui::Align2::CENTER_CENTER, &name, egui::FontId::proportional(16.0), ui.visuals().strong_text_color());
        painter.text(
            sr.center() + vec2(0.0, 10.0),
            egui::Align2::CENTER_CENTER,
            format!("{}×{} @ {}×  ({}, {})", l.width, l.height, l.scale, r.min.x as i32, r.min.y as i32),
            egui::FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        );
        if h.clicked() || h.drag_started() {
            st.selected = Some(name.clone());
        }
        if h.dragged() {
            let delta = h.drag_delta() / scale;
            let mut p = st.drag.as_ref().filter(|(n, _)| *n == name).map_or(r.min, |(_, p)| *p) + delta;
            // Snap edges to the other outputs.
            let me = Rect::from_min_size(p, r.size());
            for (j, (_, other)) in rects.iter().enumerate() {
                if j == i {
                    continue;
                }
                for (a, b) in [(me.left(), other.right()), (me.left(), other.left()), (me.right(), other.left()), (me.right(), other.right())] {
                    if (a - b).abs() < SNAP {
                        p.x += b - a;
                        break;
                    }
                }
                for (a, b) in [(me.top(), other.bottom()), (me.top(), other.top()), (me.bottom(), other.top()), (me.bottom(), other.bottom())] {
                    if (a - b).abs() < SNAP {
                        p.y += b - a;
                        break;
                    }
                }
            }
            st.drag = Some((name.clone(), p));
            rects[i].1 = Rect::from_min_size(p, r.size());
        }
        if h.drag_stopped() {
            release = Some(name.clone());
        }
        if h.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
    if !disabled.is_empty() {
        painter.text(
            resp.rect.left_bottom() + vec2(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            format!("Off: {}", disabled.iter().map(|o| o.name.as_str()).collect::<Vec<_>>().join(", ")),
            egui::FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
    }

    if release.is_some() {
        // Normalise so the arrangement starts at 0,0, then write every position: niri places
        // outputs without one automatically, which would fight the arrangement.
        let rects: Vec<(String, Rect)> = rects
            .iter()
            .map(|(n, r)| {
                let r = if st.drag.as_ref().is_some_and(|(d, _)| d == n) { Rect::from_min_size(st.drag.as_ref().unwrap().1, r.size()) } else { *r };
                (n.clone(), r)
            })
            .collect();
        let min = rects.iter().fold(pos2(f32::MAX, f32::MAX), |m, (_, r)| pos2(m.x.min(r.min.x), m.y.min(r.min.y)));
        for (name, r) in rects {
            let Some(o) = outputs.iter().find(|o| o.name == name) else { continue };
            let res = ensure_output(cx, o).and_then(|node| {
                let pos = cx.doc.ensure_in(Some(&node), &[Seg::Name("position".into())])?;
                cx.doc.set_prop_ref(&pos, "x", Some(Val::Int((r.min.x - min.x).round() as i64)), "arrange-x")?;
                cx.doc.set_prop_ref(&pos, "y", Some(Val::Int((r.min.y - min.y).round() as i64)), "arrange-y")
            });
            if let Err(e) = res {
                cx.toasts.error(format!("{e:#}"));
            }
        }
        cx.doc.break_coalescing();
        st.drag = None;
    }
}

fn settings(ui: &mut egui::Ui, cx: &mut Ctx, o: &Output) {
    let node = output_node(cx, o);
    if node.is_none() {
        ui.weak("Not in your config yet; changing anything adds an output section for it.");
    }

    // Enabled.
    let mut off = node.as_ref().is_some_and(|n| cx.doc.has_in(Some(n), &[Seg::Name("off".into())]));
    let (changed, _) = widgets::row(ui, "Disable this output", "", off, false, |ui| widgets::switch(ui, &mut off).changed());
    if changed {
        let r = ensure_output(cx, o).and_then(|n| cx.doc.set_flag_in(Some(&n), &[Seg::Name("off".into())], off));
        if let Err(e) = r {
            cx.toasts.error(format!("{e:#}"));
        }
    }

    // Mode.
    let cur_cfg = node.as_ref().and_then(|n| cx.doc.arg_in(Some(n), &[Seg::Name("mode".into())])).and_then(|v| v.as_str().map(str::to_owned));
    let live_mode = o.current_mode.and_then(|i| o.modes.get(i)).map(mode_string);
    let mut pick: Option<Option<String>> = None;
    let (_, reset) = widgets::row(ui, "Mode", "Resolution and refresh rate.", cur_cfg.is_some(), false, |ui| {
        let shown = cur_cfg.clone().or(live_mode.clone()).unwrap_or_else(|| "automatic".into());
        egui::ComboBox::from_id_salt(("mode", &o.name)).width(240.0).selected_text(shown).show_ui(ui, |ui| {
            if ui.selectable_label(cur_cfg.is_none(), "automatic (preferred)").clicked() {
                pick = Some(None);
            }
            for m in &o.modes {
                let s = mode_string(m);
                let label = if m.is_preferred { format!("{s}  {}", icons::STAR) } else { s.clone() };
                if ui.selectable_label(cur_cfg.as_deref() == Some(&s), label).clicked() {
                    pick = Some(Some(s));
                }
            }
        });
    });
    if reset {
        pick = Some(None);
    }
    if let Some(p) = pick {
        let r = ensure_output(cx, o).and_then(|n| match p {
            Some(m) => cx.doc.set_arg_in(Some(&n), &[Seg::Name("mode".into())], m),
            None => cx.doc.remove_all_in(Some(&n), &[Seg::Name("mode".into())]),
        });
        if let Err(e) = r {
            cx.toasts.error(format!("{e:#}"));
        }
    }

    // Scalar settings, written into this output's node.
    let Some(n) = node else {
        // Render with the live values; the node is created on first change.
        return simple_rows(ui, cx, o);
    };
    let fields: [(&str, Kind, &str, &str); 4] = [
        ("scale", Kind::Float { min: 0.5, max: 4.0 }, "Scale", "Fractional scales are fine."),
        ("transform", Kind::Enum(&TRANSFORMS), "Rotation", "Counter-clockwise."),
        ("focus-at-startup", Kind::Flag, "Focus at startup", ""),
        ("backdrop-color", Kind::Color, "Backdrop color", "Shown in the overview around workspaces."),
    ];
    for (name, kind, label, help) in fields {
        widgets::option_at(ui, cx, Some(&n), &[Seg::Name(name.into())], kind, label, help, false);
    }
    vrr(ui, cx, o, Some(&n));
}

/// Before an output has a node: show live values; the first change creates the node.
fn simple_rows(ui: &mut egui::Ui, cx: &mut Ctx, o: &Output) {
    let live_scale = o.logical.map_or(1.0, |l| l.scale);
    let mut scale = live_scale;
    let (changed, _) = widgets::row(ui, "Scale", "", false, false, |ui| ui.add(egui::Slider::new(&mut scale, 0.5..=4.0)).changed());
    if changed {
        let r = ensure_output(cx, o).and_then(|n| cx.doc.set_arg_in(Some(&n), &[Seg::Name("scale".into())], Val::Float(scale)));
        if let Err(e) = r {
            cx.toasts.error(format!("{e:#}"));
        }
    }
    vrr(ui, cx, o, None);
}

fn vrr(ui: &mut egui::Ui, cx: &mut Ctx, o: &Output, node: Option<&NodeRef>) {
    let path = [Seg::Name("variable-refresh-rate".into())];
    let set = node.is_some_and(|n| cx.doc.has_in(Some(n), &path));
    let on_demand = node.and_then(|n| cx.doc.prop_in(Some(n), &path, "on-demand")).and_then(|v| v.as_bool()).unwrap_or(false);
    let mut mode = if !set { 0 } else if on_demand { 2 } else { 1 };
    let before = mode;
    widgets::row(ui, "Variable refresh rate", if o.vrr_supported { "" } else { "This output doesn't report VRR support." }, set, false, |ui| {
        ui.add_enabled_ui(o.vrr_supported, |ui| {
            ui.selectable_value(&mut mode, 0, "Off");
            ui.selectable_value(&mut mode, 1, "Always");
            ui.selectable_value(&mut mode, 2, "On demand").on_hover_text("Only for windows with a variable-refresh-rate window rule");
        });
    });
    if mode != before {
        let r = ensure_output(cx, o).and_then(|n| match mode {
            0 => cx.doc.remove_all_in(Some(&n), &path),
            1 => cx.doc.set_flag_in(Some(&n), &path, true).and_then(|_| cx.doc.set_prop_in(Some(&n), &path, "on-demand", None)),
            _ => cx.doc.set_prop_in(Some(&n), &path, "on-demand", Some(Val::Bool(true))),
        });
        if let Err(e) = r {
            cx.toasts.error(format!("{e:#}"));
        }
    }
}
