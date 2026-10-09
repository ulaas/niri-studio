//! Input: keyboard layouts (add from the XKB catalogue, reorder, try live), the layout-switch
//! shortcut, and the remaining device settings.

use doc::{p, Seg, Val};
use egui::RichText;
use schema::Page;

use crate::app::Ctx;
use crate::pages::forms;
use crate::widgets;
use crate::xkb::{self, Catalog};

const LAYOUT: &str = "input/keyboard/xkb/layout";
const VARIANT: &str = "input/keyboard/xkb/variant";
const OPTIONS: &str = "input/keyboard/xkb/options";
const FILE: &str = "input/keyboard/xkb/file";

/// The switchers most people want, shown as one-click choices.
const COMMON_SWITCHERS: [(&str, &str); 6] = [
    ("grp:alt_shift_toggle", "Alt+Shift"),
    ("grp:win_space_toggle", "Super+Space"),
    ("grp:ctrl_shift_toggle", "Ctrl+Shift"),
    ("grp:caps_toggle", "Caps Lock"),
    ("grp:shifts_toggle", "Both Shifts"),
    ("grp:alt_space_toggle", "Alt+Space"),
];

#[derive(Default)]
pub struct State {
    catalog: Option<Catalog>,
    query: String,
    adding: bool,
    test_text: String,
}

fn str_arg(cx: &Ctx, path: &str) -> String {
    cx.doc.arg(&p(path)).and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}

/// (code, variant) pairs from the config.
fn configured(cx: &Ctx) -> Vec<(String, String)> {
    let codes = xkb::split(&str_arg(cx, LAYOUT));
    let variants = xkb::split(&str_arg(cx, VARIANT));
    codes.into_iter().enumerate().map(|(i, c)| (c, variants.get(i).cloned().unwrap_or_default())).collect()
}

fn write_layouts(cx: &mut Ctx, layouts: &[(String, String)]) {
    let res = if layouts.is_empty() {
        cx.doc.remove_all(&p(LAYOUT)).and_then(|_| cx.doc.remove_all(&p(VARIANT)))
    } else {
        let codes = layouts.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>().join(",");
        let r = cx.doc.set_arg(&p(LAYOUT), Val::Str(codes));
        if layouts.iter().any(|(_, v)| !v.is_empty()) {
            let vars = layouts.iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>().join(",");
            r.and_then(|_| cx.doc.set_arg(&p(VARIANT), Val::Str(vars)))
        } else {
            r.and_then(|_| cx.doc.remove_all(&p(VARIANT)))
        }
    };
    cx.doc.break_coalescing();
    if let Err(e) = res {
        cx.toasts.error(format!("{e:#}"));
    }
}

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State) {
    widgets::heading(ui, "Input", "Keyboard layouts, pointer devices and focus behaviour.");
    let cat = st.catalog.take().unwrap_or_else(Catalog::load);
    widgets::card(ui, "Keyboard layouts", |ui| layouts(ui, cx, st, &cat));
    widgets::card(ui, "Layout switching", |ui| switcher(ui, cx, &cat));
    widgets::card(ui, "Try it", |ui| try_it(ui, cx, st, &cat));
    st.catalog = Some(cat);
    widgets::card(ui, "Keyboard", |ui| {
        for o in schema::OPTIONS.iter().filter(|o| o.page == Page::Input && o.section == "Keyboard") {
            if ![LAYOUT, VARIANT, OPTIONS].contains(&o.path) {
                widgets::option(ui, cx, o);
            }
        }
    });
    forms::sections(ui, cx, Page::Input, &["Keyboard"]);
}

fn layouts(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State, cat: &Catalog) {
    if cx.doc.has(&p(FILE)) {
        ui.colored_label(ui.visuals().warn_fg_color, "A keymap file is set below, so niri ignores the layouts here.");
    }
    let mut list = configured(cx);
    let from_system = list.is_empty();
    if from_system {
        // niri falls back to the system settings (localectl); start from what's active.
        let live = cx.live.state().keyboard.clone();
        let names = live.map(|k| k.names).unwrap_or_default();
        list = names.iter().filter_map(|n| cat.by_description(n)).map(|l| (l.code.clone(), l.variant.clone())).collect();
        ui.weak(if list.is_empty() {
            "No layouts set: niri uses the system keyboard settings."
        } else {
            "Not set in your config: these come from the system keyboard settings (localectl). Editing saves them to the config."
        });
    } else {
        ui.weak("The first layout is active at startup. Order is the switching order.");
    }

    let mut new = list.clone();
    let n = list.len();
    for (i, (code, variant)) in list.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}", i + 1)).weak().monospace());
            let desc = cat.describe(code, variant).unwrap_or("Unknown layout");
            ui.label(RichText::new(desc).strong());
            let code_text = if variant.is_empty() { code.clone() } else { format!("{code}({variant})") };
            ui.label(RichText::new(code_text).monospace().weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✖").on_hover_text("Remove").clicked() {
                    new.remove(i);
                }
                if ui.add_enabled(i + 1 < n, egui::Button::new("⏷").small()).on_hover_text("Move down").clicked() {
                    new.swap(i, i + 1);
                }
                if ui.add_enabled(i > 0, egui::Button::new("⏶").small()).on_hover_text("Move up (first is the default)").clicked() {
                    new.swap(i, i - 1);
                }
            });
        });
    }

    ui.add_space(4.0);
    ui.toggle_value(&mut st.adding, "➕ Add layout");
    if st.adding {
        ui.add(egui::TextEdit::singleline(&mut st.query).hint_text("Search: language, country or code (e.g. \"turkish\", \"de\", \"dvorak\")").desired_width(f32::INFINITY));
        egui::ScrollArea::vertical().id_salt("xkb-search").max_height(240.0).show(ui, |ui| {
            let results = cat.search(&st.query);
            if results.is_empty() {
                ui.weak("No layouts match.");
            }
            for l in results.into_iter().take(300) {
                let already = new.iter().any(|(c, v)| *c == l.code && *v == l.variant);
                let code = if l.variant.is_empty() { l.code.clone() } else { format!("{}({})", l.code, l.variant) };
                ui.horizontal(|ui| {
                    let text = if l.variant.is_empty() { RichText::new(&l.description).strong() } else { RichText::new(&l.description) };
                    if ui.add_enabled(!already, egui::Button::selectable(false, text)).clicked() {
                        new.push((l.code.clone(), l.variant.clone()));
                    }
                    ui.label(RichText::new(code).monospace().weak());
                });
            }
        });
    }

    if new != list {
        write_layouts(cx, &new);
    }
}

fn options(cx: &Ctx) -> Vec<String> {
    xkb::split(&str_arg(cx, OPTIONS)).into_iter().filter(|o| !o.is_empty()).collect()
}

fn set_switcher(cx: &mut Ctx, grp: Option<&str>) {
    let mut opts: Vec<String> = options(cx).into_iter().filter(|o| !o.starts_with("grp:")).collect();
    if let Some(g) = grp {
        opts.push(g.to_owned());
    }
    let res = if opts.is_empty() { cx.doc.remove_all(&p(OPTIONS)) } else { cx.doc.set_arg(&p(OPTIONS), Val::Str(opts.join(","))) };
    cx.doc.break_coalescing();
    if let Err(e) = res {
        cx.toasts.error(format!("{e:#}"));
    }
}

fn switcher(ui: &mut egui::Ui, cx: &mut Ctx, cat: &Catalog) {
    let current = options(cx).into_iter().find(|o| o.starts_with("grp:"));
    let layouts = configured(cx).len().max(cx.live.state().keyboard.as_ref().map_or(0, |k| k.names.len()));
    ui.weak("A shortcut handled by XKB itself, so it works everywhere, even with modifier-only combos like Alt+Shift.");
    if layouts < 2 {
        ui.colored_label(ui.visuals().warn_fg_color, "Add at least two layouts for switching to do anything.");
    }
    let mut pick: Option<Option<String>> = None;
    ui.horizontal_wrapped(|ui| {
        if ui.selectable_label(current.is_none(), "None").clicked() {
            pick = Some(None);
        }
        for (opt, label) in COMMON_SWITCHERS {
            if ui.selectable_label(current.as_deref() == Some(opt), label).clicked() {
                pick = Some(Some(opt.to_owned()));
            }
        }
    });
    let shown = current.as_ref().map(|c| cat.switchers.iter().find(|(n, _)| n == c).map_or(c.clone(), |(_, d)| d.clone()));
    egui::ComboBox::from_id_salt("all-switchers")
        .width(380.0)
        .height(360.0)
        .selected_text(shown.unwrap_or_else(|| "More options…".into()))
        .show_ui(ui, |ui| {
            for (name, desc) in &cat.switchers {
                if ui.selectable_label(current.as_deref() == Some(name), desc).on_hover_text(name).clicked() {
                    pick = Some(Some(name.clone()));
                }
            }
        });
    if current.as_deref().is_some_and(|c| c.contains("alt_shift")) {
        ui.weak("Alt+Shift switches when the keys are released, so Alt+Shift+key shortcuts keep working.");
    }

    // niri bindings that switch layouts, as an alternative or complement.
    let binds: Vec<String> = cx
        .doc
        .find(&[Seg::Name("binds".into())])
        .map(|b| {
            cx.doc
                .children(&b)
                .into_iter()
                .filter_map(|r| {
                    let n = cx.doc.node(&r)?;
                    let a = n.children()?.nodes().first()?;
                    (a.name().value() == "switch-layout").then(|| n.name().value().to_owned())
                })
                .collect()
        })
        .unwrap_or_default();
    if !binds.is_empty() {
        ui.weak(format!("niri bindings that also switch layout: {}", binds.join(", ")));
    }
    if let Some(p) = pick {
        set_switcher(cx, p.as_deref());
    }
}

fn try_it(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State, cat: &Catalog) {
    let live = cx.live.state().keyboard.clone();
    let Some(kb) = live else {
        ui.weak("niri isn't reachable, so layouts can't be tried here.");
        return;
    };
    // Are the configured layouts what niri is running with?
    let cfg_names: Vec<String> = configured(cx).iter().map(|(c, v)| cat.describe(c, v).unwrap_or(c).to_owned()).collect();
    if !cfg_names.is_empty() && cfg_names != kb.names {
        ui.horizontal(|ui| {
            ui.colored_label(ui.visuals().warn_fg_color, "niri is still running the previous layouts.");
            if ui.button("💾 Save and apply").clicked() {
                *cx.request_save = true;
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Active:");
        for (i, name) in kb.names.iter().enumerate() {
            if ui.selectable_label(i == kb.current_idx as usize, name).on_hover_text("Switch to this layout").clicked() {
                if let Err(e) = live::run_action("switch-layout", &[i.to_string()], &[]) {
                    cx.toasts.error(format!("{e:#}"));
                }
            }
        }
    });
    ui.add(
        egui::TextEdit::multiline(&mut st.test_text)
            .hint_text("Type here to try the active layout, and press your switch shortcut to flip between them…")
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
}
