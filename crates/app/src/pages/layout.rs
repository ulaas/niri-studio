//! Layout: gaps, struts, columns and sizing presets, with a live preview.

use doc::{p, NodeRef, Val};
use egui::RichText;
use schema::Page;

use crate::theme::icons;
use crate::app::Ctx;
use crate::pages::forms;
use crate::{preview, widgets};

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx) {
    widgets::heading(ui, "Layout", "Gaps, struts and how columns are sized. The preview is drawn from niri's own reading of your config.");
    preview_card(ui, cx, true);
    forms::only(ui, cx, Page::Layout, &["Spacing", "Columns"]);
    widgets::card(ui, "Column widths", |ui| {
        ui.weak("Presets cycled by switch-preset-column-width. Proportions are of the screen width.");
        presets(ui, cx, "preset-column-widths");
        ui.add_space(6.0);
        default_width(ui, cx);
    });
    widgets::card(ui, "Window heights", |ui| {
        ui.weak("Presets cycled by switch-preset-window-height.");
        presets(ui, cx, "preset-window-heights");
    });
}

/// The focused workspace as niri lays it out now, column by column, or a sample when niri isn't
/// running or the workspace is empty.
fn scene(cx: &Ctx) -> preview::Scene {
    use preview::{AppKind, Column, ColumnWidth, Floating, Scene, Tile};
    let st = cx.live.state();
    let ws = st.workspaces.iter().find(|w| w.is_focused).or_else(|| st.workspaces.iter().find(|w| w.is_active));
    let output = ws
        .and_then(|w| w.output.as_ref())
        .and_then(|name| st.outputs.iter().find(|o| &o.name == name))
        .or(st.outputs.first())
        .and_then(|o| o.logical)
        .map(|l| (l.width as f32, l.height as f32));
    let Some(ws) = ws else { return Scene::sample(cx.cfg, output) };
    let windows: Vec<_> = st.windows.iter().filter(|w| w.workspace_id == Some(ws.id)).collect();
    if windows.is_empty() {
        return Scene::sample(cx.cfg, output);
    }
    let (ow, oh) = output.unwrap_or((1920.0, 1080.0));
    let l = &cx.cfg.layout;
    let gap = l.gaps as f32;
    let work_w = ow - l.struts.left.0 as f32 - l.struts.right.0 as f32;
    let tile = |w: &niri_ipc::Window, share: f32| Tile {
        kind: AppKind::guess(w.app_id.as_deref().unwrap_or("")),
        title: match (&w.title, &w.app_id) {
            (Some(t), Some(a)) => format!("{t}  ({a})"),
            (Some(t), None) => t.clone(),
            (None, Some(a)) => a.clone(),
            (None, None) => String::new(),
        },
        share,
        focused: w.is_focused || ws.active_window_id == Some(w.id) && !st.windows.iter().any(|x| x.is_focused && x.workspace_id == Some(ws.id)),
        urgent: w.is_urgent,
    };

    let mut columns: Vec<(usize, Vec<(usize, &niri_ipc::Window)>)> = Vec::new();
    let mut floating = Vec::new();
    for w in &windows {
        match w.layout.pos_in_scrolling_layout {
            Some((c, t)) => match columns.iter_mut().find(|(ci, _)| *ci == c) {
                Some((_, tiles)) => tiles.push((t, w)),
                None => columns.push((c, vec![(t, w)])),
            },
            None => {
                if let Some((x, y)) = w.layout.tile_pos_in_workspace_view {
                    let (tw, th) = w.layout.tile_size;
                    floating.push(Floating {
                        tile: tile(w, 1.0),
                        rect: egui::Rect::from_min_size(egui::pos2(x as f32, y as f32), egui::vec2(tw as f32, th as f32)),
                    });
                }
            }
        }
    }
    columns.sort_by_key(|(c, _)| *c);
    let mut focused_column = 0;
    let mut focused_x = None;
    let cols: Vec<Column> = columns
        .iter_mut()
        .enumerate()
        .map(|(i, (_, tiles))| {
            tiles.sort_by_key(|(t, _)| *t);
            let first = tiles[0].1;
            // Keep each column's share of the working area, so new gaps and struts re-flow it.
            let tw = first.layout.tile_size.0 as f32;
            let width = ColumnWidth::Proportion(((tw + gap) / (work_w - gap).max(1.0)).clamp(0.05, 1.0));
            // Stacked tiles share the column's height; tabs each take all of it.
            let total: f32 = tiles.iter().map(|(_, w)| w.layout.tile_size.1 as f32).sum();
            let tabbed = tiles.len() > 1 && total > oh * 1.05;
            let tiles: Vec<Tile> = tiles.iter().map(|(_, w)| tile(w, w.layout.tile_size.1 as f32)).collect();
            if tiles.iter().any(|t| t.focused) {
                focused_column = i;
                focused_x = columns_x(first);
            }
            Column { width, tiles, tabbed }
        })
        .collect();
    let name = ws.name.clone().unwrap_or_else(|| ws.idx.to_string());
    let on = ws.output.as_deref().map(|o| format!(" on {o}")).unwrap_or_default();
    let n = windows.len();
    Scene {
        output: output.unwrap_or((1920.0, 1080.0)),
        columns: if cols.is_empty() { Scene::sample(cx.cfg, output).columns } else { cols },
        floating,
        focused_column,
        focused_x,
        caption: format!("Workspace {name}{on} · {n} window{}", crate::app::plural(n)),
        live: true,
    }
}

fn columns_x(w: &niri_ipc::Window) -> Option<f32> {
    w.layout.tile_pos_in_workspace_view.map(|(x, _)| x as f32)
}

pub fn preview_card(ui: &mut egui::Ui, cx: &mut Ctx, link_to_appearance: bool) {
    let scene = scene(cx);
    let cfg = cx.cfg;
    widgets::card(ui, "Preview", |ui| {
        let hovered = preview::workspace(ui, cfg, &scene, 460.0);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            match hovered {
                Some(part) => ui.label(part.describe()),
                None => ui.weak(if scene.live {
                    "Your workspace as niri lays it out, redrawn with your edits. Hover to see what each part is."
                } else {
                    "Hover the preview to see what each part is."
                }),
            };
            if !cx.doc.validation().ok() {
                ui.colored_label(ui.visuals().warn_fg_color, "Showing the last valid config.");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if link_to_appearance && ui.link("Focus ring, border and shadow »").clicked() {
                    *cx.nav = Some(crate::app::Nav::Page(schema::Page::Appearance));
                }
            });
        });
    });
}

fn presets(ui: &mut egui::Ui, cx: &mut Ctx, name: &str) {
    let path = p(&format!("layout/{name}"));
    let parent = cx.doc.find(&path);
    let items: Vec<(NodeRef, String, f64)> = parent
        .as_ref()
        .map(|r| {
            cx.doc
                .children(r)
                .into_iter()
                .filter_map(|c| {
                    let n = cx.doc.node(&c)?;
                    let v = doc::node_args(n).first()?.as_f64()?;
                    Some((c.clone(), n.name().value().to_owned(), v))
                })
                .collect()
        })
        .unwrap_or_default();

    if parent.is_none() {
        ui.weak("Not set: niri uses ⅓, ½ and ⅔.");
    }
    let mut edit: Option<(NodeRef, String)> = None;
    let mut remove: Option<NodeRef> = None;
    for (i, (r, kind, v)) in items.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}.", i + 1)).weak());
            let mut kind_sel = kind.clone();
            egui::ComboBox::from_id_salt((name, i, "kind")).width(110.0).selected_text(&kind_sel).show_ui(ui, |ui| {
                if let Some(v) = crate::widgets::segmented(ui, ui.id().with("seg-kind_sel"), Some(&kind_sel), &[("proportion".to_owned(), "proportion"), ("fixed".to_owned(), "fixed")], false) {
                    kind_sel = v;
                }
            });
            let mut val = *v;
            let changed = if kind == "proportion" {
                ui.add(widgets::Slider::new(&mut val, 0.05..=1.0).fixed_decimals(3)).changed()
            } else {
                ui.add(egui::DragValue::new(&mut val).range(1.0..=10000.0).speed(4.0).suffix(" px")).changed()
            };
            if kind_sel != *kind {
                let nv = if kind_sel == "fixed" { 800.0 } else { 0.5 };
                edit = Some((r.clone(), format!("{kind_sel} {}", fmt(&kind_sel, nv))));
            } else if changed {
                edit = Some((r.clone(), format!("{kind} {}", fmt(kind, val))));
            }
            if widgets::small_icon_button(ui, icons::X, "Remove").clicked() {
                remove = Some(r.clone());
            }
        });
    }
    ui.horizontal(|ui| {
        if ui.button(format!("{} Proportion", icons::PLUS)).clicked() {
            add(cx, &path, "proportion 0.5");
        }
        if ui.button(format!("{} Fixed width", icons::PLUS)).clicked() {
            add(cx, &path, "fixed 800");
        }
        if parent.is_some() && ui.button("Reset to niri's presets").clicked() {
            let _ = cx.doc.remove_all(&path);
        }
    });
    let res = match (edit, remove) {
        (Some((r, text)), _) => cx.doc.replace_node(&r, &text),
        (_, Some(r)) => cx.doc.remove(&r),
        _ => Ok(()),
    };
    if let Err(e) = res {
        cx.toasts.error(format!("{e:#}"));
    }
}

fn fmt(kind: &str, v: f64) -> String {
    if kind == "fixed" { (v.round() as i64).to_string() } else { Val::Float((v * 100000.0).round() / 100000.0).to_kdl() }
}

fn add(cx: &mut Ctx, path: &[doc::Seg], text: &str) {
    let res = cx.doc.ensure(path).and_then(|r| cx.doc.append(Some(&r), text).map(|_| ()));
    if let Err(e) = res {
        cx.toasts.error(format!("{e:#}"));
    }
}

fn default_width(ui: &mut egui::Ui, cx: &mut Ctx) {
    let path = p("layout/default-column-width");
    let node = cx.doc.find(&path);
    #[derive(PartialEq, Clone, Copy)]
    enum Mode {
        Unset,
        Window,
        Proportion(f64),
        Fixed(f64),
    }
    let mode = match &node {
        None => Mode::Unset,
        Some(r) => {
            let child = cx.doc.children(r).into_iter().last().and_then(|c| {
                let n = cx.doc.node(&c)?;
                Some((n.name().value().to_owned(), doc::node_args(n).first()?.as_f64()?))
            });
            match child {
                Some((k, v)) if k == "proportion" => Mode::Proportion(v),
                Some((k, v)) if k == "fixed" => Mode::Fixed(v),
                _ => Mode::Window,
            }
        }
    };
    let mut new = mode;
    ui.horizontal(|ui| {
        ui.label("New windows open at");
        ui.radio_value(&mut new, Mode::Unset, "niri default (½)");
        if ui.radio(mode == Mode::Window, "the window's choice").clicked() {
            new = Mode::Window;
        }
        if ui.radio(matches!(mode, Mode::Proportion(_)), "proportion").clicked() && !matches!(mode, Mode::Proportion(_)) {
            new = Mode::Proportion(0.5);
        }
        if ui.radio(matches!(mode, Mode::Fixed(_)), "fixed").clicked() && !matches!(mode, Mode::Fixed(_)) {
            new = Mode::Fixed(800.0);
        }
        match &mut new {
            Mode::Proportion(v) => {
                ui.add(widgets::Slider::new(v, 0.05..=1.0).fixed_decimals(3));
            }
            Mode::Fixed(v) => {
                ui.add(egui::DragValue::new(v).range(1.0..=10000.0).speed(4.0).suffix(" px"));
            }
            _ => {}
        }
    });
    if new != mode {
        let text = match new {
            Mode::Unset => None,
            Mode::Window => Some("default-column-width {}".to_owned()),
            Mode::Proportion(v) => Some(format!("default-column-width {{ proportion {}; }}", fmt("proportion", v))),
            Mode::Fixed(v) => Some(format!("default-column-width {{ fixed {}; }}", fmt("fixed", v))),
        };
        let res = match (text, node) {
            (None, _) => cx.doc.remove_all(&path),
            (Some(t), Some(r)) => cx.doc.replace_node(&r, &t),
            (Some(t), None) => cx.doc.ensure(&p("layout")).and_then(|l| cx.doc.append(Some(&l), &t).map(|_| ())),
        };
        if let Err(e) = res {
            cx.toasts.error(format!("{e:#}"));
        }
    }
}
