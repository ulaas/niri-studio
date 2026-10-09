//! Animations: spring and easing editors with a plotted curve and a looping preview.
//!
//! The curve maths mirrors niri's `src/animation` so what you see is what niri does.

use std::time::Instant;

use doc::{p, Seg, Val};
use egui::{pos2, vec2, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};
use niri_config::animations::{Animation, Curve, Kind as AnimKind};
use schema::Page;

use crate::app::Ctx;
use crate::pages::forms;
use crate::widgets;

const NAMES: [(&str, &str); 11] = [
    ("workspace-switch", "Workspace switch"),
    ("window-open", "Window open"),
    ("window-close", "Window close"),
    ("horizontal-view-movement", "Horizontal view movement"),
    ("window-movement", "Window movement"),
    ("window-resize", "Window resize"),
    ("config-notification-open-close", "Config notification"),
    ("exit-confirmation-open-close", "Exit confirmation"),
    ("screenshot-ui-open", "Screenshot UI open"),
    ("overview-open-close", "Overview open/close"),
    ("recent-windows-close", "Recent windows close"),
];

const CURVES: [&str; 5] = ["linear", "ease-out-quad", "ease-out-cubic", "ease-out-expo", "cubic-bezier"];

pub struct State {
    selected: usize,
    started: Instant,
}

impl Default for State {
    fn default() -> Self {
        Self { selected: 0, started: Instant::now() }
    }
}

fn effective(cfg: &niri_config::Config, name: &str) -> Animation {
    let a = &cfg.animations;
    match name {
        "workspace-switch" => a.workspace_switch.0,
        "window-open" => a.window_open.anim,
        "window-close" => a.window_close.anim,
        "horizontal-view-movement" => a.horizontal_view_movement.0,
        "window-movement" => a.window_movement.0,
        "window-resize" => a.window_resize.anim,
        "config-notification-open-close" => a.config_notification_open_close.0,
        "exit-confirmation-open-close" => a.exit_confirmation_open_close.0,
        "screenshot-ui-open" => a.screenshot_ui_open.0,
        "overview-open-close" => a.overview_open_close.0,
        _ => a.recent_windows_close.0,
    }
}

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State) {
    widgets::heading(ui, "Animations", "Springs follow your touchpad's velocity; easings run for a fixed duration.");
    forms::sections(ui, cx, Page::Animations, &[]);

    let slowdown = if cx.cfg.animations.off { 0.0 } else { cx.cfg.animations.slowdown };
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        widgets::card(ui, "Animation", |ui| {
            for (i, (key, title)) in NAMES.iter().enumerate() {
                let custom = cx.doc.has(&p(&format!("animations/{key}")));
                let off = effective(cx.cfg, key).off;
                let mut text = RichText::new(*title);
                if off {
                    text = text.strikethrough().weak();
                }
                ui.horizontal(|ui| {
                    if ui.selectable_label(st.selected == i, text).clicked() {
                        st.selected = i;
                        st.started = Instant::now();
                    }
                    if custom {
                        ui.weak("customised");
                    }
                });
            }
        });
        let ui = &mut cols[1];
        let (key, title) = NAMES[st.selected];
        widgets::card(ui, title, |ui| editor(ui, cx, st, key, slowdown));
    });
}

fn editor(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State, key: &str, slowdown: f64) {
    let base = p(&format!("animations/{key}"));
    let anim = effective(cx.cfg, key);
    let custom = cx.doc.has(&base);

    ui.horizontal(|ui| {
        let mut off = cx.doc.has(&child(&base, "off"));
        ui.label("Disabled");
        if widgets::switch(ui, &mut off).changed() {
            let _ = cx.doc.set_flag(&child(&base, "off"), off);
        }
        if custom && ui.button("⟲ Reset to niri default").clicked() {
            let _ = cx.doc.remove_all(&base);
        }
    });

    let is_spring = matches!(anim.kind, AnimKind::Spring(_));
    let mut want_spring = is_spring;
    ui.horizontal(|ui| {
        ui.selectable_value(&mut want_spring, true, "Spring");
        ui.selectable_value(&mut want_spring, false, "Easing");
    });
    if want_spring != is_spring {
        let res = if want_spring {
            cx.doc
                .remove_all(&child(&base, "duration-ms"))
                .and_then(|_| cx.doc.remove_all(&child(&base, "curve")))
                .and_then(|_| cx.doc.set_prop(&child(&base, "spring"), "damping-ratio", Some(Val::Float(1.0))))
                .and_then(|_| cx.doc.set_prop(&child(&base, "spring"), "stiffness", Some(Val::Int(800))))
                .and_then(|_| cx.doc.set_prop(&child(&base, "spring"), "epsilon", Some(Val::Float(0.0001))))
        } else {
            cx.doc
                .remove_all(&child(&base, "spring"))
                .and_then(|_| cx.doc.set_arg(&child(&base, "duration-ms"), Val::Int(250)))
                .and_then(|_| cx.doc.set_arg(&child(&base, "curve"), Val::str("ease-out-cubic")))
        };
        if let Err(e) = res {
            cx.toasts.error(format!("{e:#}"));
        }
        return;
    }

    match anim.kind {
        AnimKind::Spring(sp) => {
            let path = child(&base, "spring");
            let mut d = sp.damping_ratio;
            let mut k = sp.stiffness as f64;
            let mut eps = sp.epsilon;
            let mut changed = None;
            row(ui, "Damping ratio", "Below 1 bounces; 1 settles fastest without bouncing.", |ui| {
                if ui.add(egui::Slider::new(&mut d, 0.1..=10.0).logarithmic(true).max_decimals(2)).changed() {
                    changed = Some(("damping-ratio", Val::Float((d * 100.0).round() / 100.0)));
                }
            });
            row(ui, "Stiffness", "Higher is faster.", |ui| {
                if ui.add(egui::Slider::new(&mut k, 1.0..=5000.0).logarithmic(true).max_decimals(0)).changed() {
                    changed = Some(("stiffness", Val::Int(k.round() as i64)));
                }
            });
            row(ui, "Epsilon", "Lower if the animation jumps at the end.", |ui| {
                if ui.add(egui::Slider::new(&mut eps, 0.00001..=0.1).logarithmic(true).max_decimals(5)).changed() {
                    changed = Some(("epsilon", Val::Float(eps)));
                }
            });
            if let Some((key, v)) = changed {
                let r = cx.doc.set_prop(&path, "damping-ratio", Some(Val::Float(sp.damping_ratio)))
                    .and_then(|_| cx.doc.set_prop(&path, "stiffness", Some(Val::Int(sp.stiffness as i64))))
                    .and_then(|_| cx.doc.set_prop(&path, "epsilon", Some(Val::Float(sp.epsilon))))
                    .and_then(|_| cx.doc.set_prop(&path, key, Some(v)));
                if let Err(e) = r {
                    cx.toasts.error(format!("{e:#}"));
                }
            }
        }
        AnimKind::Easing(e) => {
            let mut ms = e.duration_ms as i64;
            let mut dur_changed = false;
            row(ui, "Duration", "", |ui| {
                dur_changed = ui.add(egui::Slider::new(&mut ms, 0..=2000).suffix(" ms")).changed();
            });
            if dur_changed {
                let _ = cx.doc.set_arg(&child(&base, "duration-ms"), Val::Int(ms));
            }
            let cur = curve_name(e.curve);
            let mut pick = None;
            row(ui, "Curve", "", |ui| {
                egui::ComboBox::from_id_salt(("curve", key)).selected_text(cur).show_ui(ui, |ui| {
                    for c in CURVES {
                        if ui.selectable_label(cur == c, c).clicked() {
                            pick = Some(c);
                        }
                    }
                });
            });
            if let Some(c) = pick.filter(|c| *c != cur) {
                let args = if c == "cubic-bezier" {
                    vec![Val::str(c), Val::Float(0.05), Val::Float(0.7), Val::Float(0.1), Val::Float(1.0)]
                } else {
                    vec![Val::str(c)]
                };
                let _ = cx.doc.set_args(&child(&base, "curve"), &args);
            }
        }
    }

    ui.add_space(6.0);
    let duration = duration_secs(&anim);
    let bezier_edit = plot(ui, &anim, duration);
    if let Some([x1, y1, x2, y2]) = bezier_edit {
        let r = |v: f32| Val::Float(((v as f64) * 100.0).round() / 100.0);
        let _ = cx.doc.set_args(&child(&base, "curve"), &[Val::str("cubic-bezier"), r(x1), r(y1), r(x2), r(y2)]);
    }
    ui.horizontal(|ui| {
        ui.weak(match duration {
            Some(d) => format!("Settles in {:.0} ms", d * 1000.0 * slowdown.max(0.0)),
            None => "Never settles with these parameters.".to_owned(),
        });
        if slowdown != 1.0 && slowdown > 0.0 {
            ui.weak(format!("(includes slowdown ×{slowdown})"));
        }
    });
    demo(ui, st, &anim, duration, slowdown);
}

fn child(base: &[Seg], name: &str) -> Vec<Seg> {
    let mut v = base.to_vec();
    v.push(Seg::Name(name.to_owned()));
    v
}

fn row(ui: &mut egui::Ui, label: &str, help: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        let l = ui.add_sized([110.0, 20.0], egui::Label::new(label));
        if !help.is_empty() {
            l.on_hover_text(help);
        }
        add(ui);
    });
}

fn curve_name(c: Curve) -> &'static str {
    match c {
        Curve::Linear => "linear",
        Curve::EaseOutQuad => "ease-out-quad",
        Curve::EaseOutCubic => "ease-out-cubic",
        Curve::EaseOutExpo => "ease-out-expo",
        Curve::CubicBezier(..) => "cubic-bezier",
    }
}

/// Progress (0 → 1, may overshoot) at `t` seconds.
fn value_at(anim: &Animation, t: f64) -> f64 {
    match anim.kind {
        AnimKind::Easing(e) => {
            let d = e.duration_ms as f64 / 1000.0;
            let x = if d <= 0.0 { 1.0 } else { (t / d).clamp(0.0, 1.0) };
            match e.curve {
                Curve::Linear => x,
                Curve::EaseOutQuad => 1.0 - (1.0 - x).powi(2),
                Curve::EaseOutCubic => 1.0 - (1.0 - x).powi(3),
                Curve::EaseOutExpo => 1.0 - 2f64.powf(-10.0 * x),
                Curve::CubicBezier(x1, y1, x2, y2) => bezier_y(x, x1, y1, x2, y2),
            }
        }
        AnimKind::Spring(sp) => spring(sp.damping_ratio, sp.stiffness as f64, t),
    }
}

/// niri's damped spring from 0 to 1 with zero initial velocity and mass 1.
fn spring(damping_ratio: f64, stiffness: f64, t: f64) -> f64 {
    let k = stiffness.max(0.0);
    let b = damping_ratio.max(0.0) * 2.0 * k.sqrt();
    let beta = b / 2.0;
    let w0 = k.sqrt();
    let x0 = -1.0;
    let env = (-beta * t).exp();
    if (beta - w0).abs() <= f32::EPSILON as f64 {
        1.0 + env * (x0 + beta * x0 * t)
    } else if beta < w0 {
        let w1 = (w0 * w0 - beta * beta).sqrt();
        1.0 + env * (x0 * (w1 * t).cos() + (beta * x0 / w1) * (w1 * t).sin())
    } else {
        let w2 = (beta * beta - w0 * w0).sqrt();
        1.0 + env * (x0 * (w2 * t).cosh() + (beta * x0 / w2) * (w2 * t).sinh())
    }
}

/// How long until the animation is done, in seconds (niri's clamped duration for springs).
fn duration_secs(anim: &Animation) -> Option<f64> {
    match anim.kind {
        AnimKind::Easing(e) => Some(e.duration_ms as f64 / 1000.0),
        AnimKind::Spring(sp) => (1..=3000)
            .map(|i| i as f64 / 1000.0)
            .find(|&t| 1.0 - spring(sp.damping_ratio, sp.stiffness as f64, t) <= sp.epsilon)
            .or_else(|| {
                // Bouncy springs: settle when the envelope stays within epsilon.
                (3001..=10000).map(|i| i as f64 / 1000.0).find(|&t| {
                    (0..50).all(|j| (1.0 - spring(sp.damping_ratio, sp.stiffness as f64, t + j as f64 / 1000.0)).abs() <= sp.epsilon)
                })
            }),
    }
}

fn bezier_y(x: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let bez = |t: f64, a: f64, b: f64| 3.0 * (1.0 - t).powi(2) * t * a + 3.0 * (1.0 - t) * t * t * b + t.powi(3);
    // Solve x(t) = x by bisection; x(t) is monotonic for x1, x2 in [0, 1].
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..50 {
        let mid = (lo + hi) / 2.0;
        if bez(mid, x1, x2) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez((lo + hi) / 2.0, y1, y2)
}

/// Plots progress over time; for cubic-bezier, the control points can be dragged.
fn plot(ui: &mut egui::Ui, anim: &Animation, duration: Option<f64>) -> Option<[f32; 4]> {
    let width = ui.available_width().min(520.0);
    let (resp, painter) = ui.allocate_painter(vec2(width, 220.0), Sense::hover());
    let r = resp.rect;
    let vis = ui.visuals();
    painter.rect_filled(r, 6.0, vis.extreme_bg_color);
    let inner = r.shrink2(vec2(14.0, 30.0));
    let t_max = duration.unwrap_or(1.0).clamp(0.05, 5.0) * 1.15;

    // Sample to find overshoot so the plot fits.
    let samples: Vec<(f64, f64)> = (0..=240).map(|i| {
        let t = t_max * i as f64 / 240.0;
        (t, value_at(anim, t))
    }).collect();
    let (ymin, ymax) = samples.iter().fold((0.0f64, 1.0f64), |(lo, hi), (_, y)| (lo.min(*y), hi.max(*y)));
    let to_screen = |t: f64, y: f64| -> Pos2 {
        pos2(
            inner.left() + (t / t_max) as f32 * inner.width(),
            inner.bottom() - ((y - ymin) / (ymax - ymin)) as f32 * inner.height(),
        )
    };

    // Grid: 0 and 1 lines, settle time.
    let grid = Stroke::new(1.0, vis.widgets.noninteractive.bg_stroke.color);
    for y in [0.0, 1.0] {
        painter.line_segment([to_screen(0.0, y), to_screen(t_max, y)], grid);
    }
    if let Some(d) = duration {
        let x = to_screen(d, 0.0).x;
        painter.line_segment([pos2(x, inner.top()), pos2(x, inner.bottom())], Stroke::new(1.0, vis.weak_text_color()));
        painter.text(pos2(x + 4.0, r.top() + 6.0), egui::Align2::LEFT_TOP, format!("{:.0} ms", d * 1000.0), egui::FontId::proportional(11.0), vis.weak_text_color());
    }
    let pts: Vec<Pos2> = samples.iter().map(|(t, y)| to_screen(*t, *y)).collect();
    painter.add(egui::Shape::line(pts, Stroke::new(2.5, vis.selection.stroke.color)));
    if ymax > 1.0001 {
        painter.text(to_screen(0.0, ymax) + vec2(4.0, -2.0), egui::Align2::LEFT_BOTTOM, format!("overshoot {:.1}%", (ymax - 1.0) * 100.0), egui::FontId::proportional(11.0), vis.warn_fg_color);
    }

    // Draggable cubic-bezier handles (in unit-square coordinates over the duration).
    let AnimKind::Easing(e) = anim.kind else { return None };
    let Curve::CubicBezier(x1, y1, x2, y2) = e.curve else { return None };
    let d = e.duration_ms as f64 / 1000.0;
    let unit = |x: f64, y: f64| to_screen(x * d, y);
    let from_screen = |p: Pos2| -> (f32, f32) {
        let t = ((p.x - inner.left()) / inner.width()) as f64 * t_max;
        let y = ymin + ((inner.bottom() - p.y) / inner.height()) as f64 * (ymax - ymin);
        (((t / d).clamp(0.0, 1.0)) as f32, y as f32)
    };
    let mut out = None;
    for (i, (x, y), anchor) in [(0, (x1, y1), unit(0.0, 0.0)), (1, (x2, y2), unit(1.0, 1.0))] {
        let p = unit(x, y);
        painter.line_segment([anchor, p], Stroke::new(1.0, vis.text_color()));
        let id = resp.id.with(("handle", i));
        let hr = ui.interact(Rect::from_center_size(p, vec2(14.0, 14.0)), id, Sense::drag());
        let color = if hr.hovered() || hr.dragged() { vis.selection.stroke.color } else { vis.text_color() };
        painter.circle(p, 5.0, color, Stroke::new(1.0, Color32::BLACK));
        if hr.dragged() {
            if let Some(pos) = hr.interact_pointer_pos() {
                let (nx, ny) = from_screen(pos);
                let mut v = [x1 as f32, y1 as f32, x2 as f32, y2 as f32];
                v[i * 2] = nx;
                v[i * 2 + 1] = ny;
                out = Some(v);
            }
        }
    }
    painter.rect_stroke(r, 6.0, grid, StrokeKind::Inside);
    out
}

/// A window sliding across, looping, at the real speed (with slowdown).
fn demo(ui: &mut egui::Ui, st: &mut State, anim: &Animation, duration: Option<f64>, slowdown: f64) {
    let width = ui.available_width().min(520.0);
    let (r, _) = ui.allocate_exact_size(vec2(width, 70.0), Sense::hover());
    let painter = ui.painter_at(r);
    painter.rect_filled(r, 6.0, ui.visuals().extreme_bg_color);
    let run = duration.unwrap_or(1.0).min(5.0) * slowdown.max(0.0001);
    let cycle = run + 0.7;
    let t = (st.started.elapsed().as_secs_f64() % cycle) / slowdown.max(0.0001);
    let v = if anim.off || slowdown == 0.0 { 1.0 } else { value_at(anim, t.min(run / slowdown.max(0.0001))) } as f32;
    let w = 90.0;
    let x0 = r.left() + 12.0;
    let x1 = r.right() - 12.0 - w;
    let x = x0 + (x1 - x0) * v;
    let win = Rect::from_min_size(pos2(x, r.top() + 12.0), vec2(w, r.height() - 24.0));
    painter.rect_filled(win, 6.0, ui.visuals().selection.bg_fill);
    painter.rect_stroke(win, 6.0, Stroke::new(2.0, ui.visuals().selection.stroke.color), StrokeKind::Inside);
    ui.ctx().request_repaint();
}
