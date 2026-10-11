//! Window and layer rules: click-a-window quick rules, a rule list you can rename, reorder,
//! enable/disable and delete, and an editor that shows which open windows each rule matches.

use std::sync::mpsc::Receiver;

use doc::{Disabled, NodeRef, Seg, Val};
use egui::RichText;
use niri_ipc::Window;
use regex::Regex;
use schema::{Kind, RuleProp};

use crate::theme::icons;
use crate::app::Ctx;
use crate::widgets;

#[derive(Clone, Copy, PartialEq, Default)]
enum Tab {
    #[default]
    Window,
    Layer,
}

impl Tab {
    fn node(self) -> &'static str {
        match self {
            Tab::Window => "window-rule",
            Tab::Layer => "layer-rule",
        }
    }
    fn props(self) -> &'static [RuleProp] {
        match self {
            Tab::Window => schema::WINDOW_RULE_PROPS,
            Tab::Layer => schema::LAYER_RULE_PROPS,
        }
    }
}

#[derive(Clone, PartialEq)]
enum Sel {
    On(usize),
    Off(usize),
}

enum Purpose {
    NewRule(usize),
    AddMatch { rule: usize, exclude: bool },
}

#[derive(Default)]
pub struct State {
    tab: Tab,
    selected: Option<Sel>,
    /// Debug/screenshot aid: select the n-th enabled rule.
    pub preselect: Option<usize>,
    picking: Option<(Purpose, Receiver<anyhow::Result<Option<Window>>>)>,
    with_title: bool,
    renaming: Option<(Sel, String)>,
    namespaces: Option<Vec<String>>,
}

enum Entry {
    On { idx: usize, r: NodeRef },
    Off { idx: usize, d: Disabled },
}

impl Entry {
    fn sel(&self) -> Sel {
        match self {
            Entry::On { idx, .. } => Sel::On(*idx),
            Entry::Off { idx, .. } => Sel::Off(*idx),
        }
    }
}

fn entries(cx: &Ctx, tab: Tab) -> Vec<(usize, usize, Entry)> {
    let on = cx.doc.find_all(&[Seg::Name(tab.node().into())]);
    let mut v: Vec<(usize, usize, Entry)> = on
        .into_iter()
        .enumerate()
        .filter_map(|(idx, r)| {
            let off = cx.doc.node(&r)?.span().offset();
            Some((r.file, off, Entry::On { idx, r }))
        })
        .collect();
    for (idx, d) in cx.doc.disabled_nodes(tab.node()).into_iter().enumerate() {
        v.push((d.file, d.range.start, Entry::Off { idx, d }));
    }
    v.sort_by_key(|(f, o, _)| (*f, *o));
    v
}

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State) {
    widgets::heading(ui, "Window rules", "Rules apply in order; later rules override earlier ones for the same property.");
    ui.horizontal(|ui| {
        if let Some(v) = crate::widgets::segmented(ui, ui.id().with("seg-tab"), Some(&st.tab), &[(Tab::Window, "Window rules"), (Tab::Layer, "Layer rules (bars, launchers…)")], false) {
            st.tab = v;
        }
    });
    poll_pick(cx, st);

    if st.tab == Tab::Window {
        quick_rules(ui, cx, st);
    } else if st.namespaces.is_none() {
        st.namespaces = Some(live::layer_namespaces());
    }

    let list = entries(cx, st.tab);
    if let Some(i) = st.preselect.take() {
        st.selected = Some(Sel::On(i));
    }
    if let Some(sel) = &st.selected {
        if !list.iter().any(|(_, _, e)| e.sel() == *sel) {
            st.selected = None;
        }
    }
    let windows = cx.live.state().windows.clone();

    ui.columns(2, |cols| {
        widgets::card(&mut cols[0], "Rules", |ui| rule_list(ui, cx, st, &list, &windows));
        let ui = &mut cols[1];
        match st.selected.clone() {
            Some(Sel::On(i)) => {
                if let Some((_, _, Entry::On { r, .. })) = list.iter().find(|(_, _, e)| e.sel() == Sel::On(i)) {
                    let title = cx.doc.label(r).unwrap_or_else(|| format!("Rule {}", i + 1));
                    widgets::card(ui, &title, |ui| editor(ui, cx, st, r, i, &windows));
                }
            }
            Some(Sel::Off(i)) => {
                if let Some((_, _, Entry::Off { d, .. })) = list.iter().find(|(_, _, e)| e.sel() == Sel::Off(i)) {
                    widgets::card(ui, "Disabled rule", |ui| {
                        ui.weak("This rule is commented out with /- and has no effect. Enable it to edit.");
                        ui.add_space(4.0);
                        ui.label(RichText::new(&d.body).monospace());
                        if ui.button("Enable").clicked() {
                            cx.doc.enable(d);
                            st.selected = None;
                        }
                    });
                }
            }
            None => {
                widgets::card(ui, "Editor", |ui| {
                    ui.weak("Select a rule to edit it, or create one with a quick rule above.");
                });
            }
        }
    });
}

fn quick_rules(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State) {
    let connected = cx.live.state().connected;
    widgets::card(ui, "Quick rules", |ui| {
        if let Some((purpose, _)) = &st.picking {
            ui.horizontal(|ui| {
                ui.spinner();
                let what = match purpose {
                    Purpose::NewRule(i) => schema::QUICK_RULES[*i].0,
                    Purpose::AddMatch { .. } => "add a match",
                };
                ui.label(RichText::new(format!("Click a window to {}… (Esc in niri cancels)", what.to_lowercase())).strong());
            });
            return;
        }
        ui.weak("Pick a rule, then click the window it should apply to. The rule is matched on the app id.");
        ui.horizontal_wrapped(|ui| {
            for (i, (button, _, _)) in schema::QUICK_RULES.iter().enumerate() {
                if ui.add_enabled(connected, egui::Button::new(*button)).on_disabled_hover_text("Needs a running niri").clicked() {
                    start_pick(cx, st, Purpose::NewRule(i));
                }
            }
        });
        widgets::checkbox(ui, &mut st.with_title, "Also match the window title (only that exact window)");
    });
}

fn start_pick(cx: &Ctx, st: &mut State, purpose: Purpose) {
    let ctx = egui::Context::clone(&ctx_of(cx));
    let rx = live::pick_window(move || ctx.request_repaint());
    st.picking = Some((purpose, rx));
}

// The repaint context: the global egui context is reachable through any Ui, but pages only
// keep `Ctx`; egui keeps one context per app, so fetch it lazily.
fn ctx_of(_cx: &Ctx) -> egui::Context {
    CTX.with(|c| c.borrow().clone().expect("egui context set"))
}

thread_local! {
    static CTX: std::cell::RefCell<Option<egui::Context>> = const { std::cell::RefCell::new(None) };
}

fn poll_pick(cx: &mut Ctx, st: &mut State) {
    let Some((_, rx)) = &st.picking else { return };
    let Ok(res) = rx.try_recv() else { return };
    let (purpose, _) = st.picking.take().unwrap();
    let w = match res {
        Ok(Some(w)) => w,
        Ok(None) => return cx.toasts.info("Picking cancelled."),
        Err(e) => return cx.toasts.error(format!("Couldn't pick a window: {e:#}")),
    };
    let conds = match_props(&w, st.with_title);
    if conds.is_empty() {
        return cx.toasts.warn("That window has neither an app id nor a title to match on.");
    }
    let what = w.app_id.clone().or(w.title.clone()).unwrap_or_default();
    match purpose {
        Purpose::NewRule(i) => {
            let (_, prefix, lines) = schema::QUICK_RULES[i];
            let mut text = format!("// {prefix}: {what}\nwindow-rule {{\n    match {conds}\n");
            for l in lines {
                text += &format!("    {l}\n");
            }
            text += "}";
            let before = cx.doc.find_all(&[Seg::Name("window-rule".into())]).len();
            if let Err(e) = cx.doc.append(None, &text) {
                return cx.toasts.error(format!("{e:#}"));
            }
            if let Some(err) = cx.doc.validation().errors.first().map(|e| e.message.clone()) {
                cx.doc.undo();
                return cx.toasts.error(format!("niri rejected the generated rule, so it was removed: {err}"));
            }
            st.selected = Some(Sel::On(before));
            cx.toasts.ok(format!("Added \"{prefix}: {what}\". Save to apply."));
        }
        Purpose::AddMatch { rule, exclude } => {
            let Some(r) = cx.doc.find_all(&[Seg::Name("window-rule".into())]).get(rule).cloned() else { return };
            let kw = if exclude { "exclude" } else { "match" };
            if let Err(e) = cx.doc.append(Some(&r), &format!("{kw} {conds}")) {
                cx.toasts.error(format!("{e:#}"));
            }
        }
    }
}

/// `app-id="^…$"` (and the title when asked), escaped for niri's regexes.
fn match_props(w: &Window, with_title: bool) -> String {
    let mut parts = Vec::new();
    if let Some(a) = w.app_id.as_deref().filter(|a| !a.is_empty()) {
        parts.push(format!("app-id={}", Val::str(format!("^{}$", regex::escape(a))).to_kdl()));
    }
    if with_title || parts.is_empty() {
        if let Some(t) = w.title.as_deref().filter(|t| !t.is_empty()) {
            parts.push(format!("title={}", Val::str(format!("^{}$", regex::escape(t))).to_kdl()));
        }
    }
    parts.join(" ")
}

fn rule_list(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State, list: &[(usize, usize, Entry)], windows: &[Window]) {
    if list.is_empty() {
        ui.weak("No rules yet.");
        return;
    }
    let on_refs: Vec<NodeRef> = list
        .iter()
        .filter_map(|(_, _, e)| match e {
            Entry::On { r, .. } => Some(r.clone()),
            _ => None,
        })
        .collect();
    let mut action: Option<Box<dyn FnOnce(&mut Ctx, &mut State)>> = None;

    for (_, _, e) in list {
        let sel = e.sel();
        let selected = st.selected.as_ref() == Some(&sel);
        let (label, enabled) = match e {
            Entry::On { idx, r } => (cx.doc.label(r).unwrap_or_else(|| summary(cx, r, *idx)), true),
            Entry::Off { d, .. } => (cx.doc.label_at(d.file, d.range.start).unwrap_or_else(|| first_line(&d.body)), false),
        };
        ui.horizontal(|ui| {
            let mut on = enabled;
            if widgets::switch(ui, &mut on).on_hover_text("Enable or disable (keeps the rule in the file)").changed() {
                action = Some(match e {
                    Entry::On { r, .. } => {
                        let r = r.clone();
                        Box::new(move |cx: &mut Ctx, st: &mut State| {
                            let _ = cx.doc.disable(&r);
                            st.selected = None;
                        })
                    }
                    Entry::Off { d, .. } => {
                        let d = d.clone();
                        Box::new(move |cx: &mut Ctx, st: &mut State| {
                            cx.doc.enable(&d);
                            st.selected = None;
                        })
                    }
                });
            }

            if let Some((rsel, name)) = st.renaming.as_mut().filter(|(s, _)| *s == sel) {
                let resp = ui.add(egui::TextEdit::singleline(name).desired_width(200.0).hint_text("Rule name"));
                resp.request_focus();
                let _ = rsel;
                if resp.lost_focus() {
                    let name = name.clone();
                    let target = match e {
                        Entry::On { r, .. } => cx.doc.node(r).map(|n| (r.file, n.span().offset())),
                        Entry::Off { d, .. } => Some((d.file, d.range.start)),
                    };
                    if !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        if let Some((f, off)) = target {
                            cx.doc.set_label_at(f, off, Some(name.as_str()).filter(|n| !n.trim().is_empty()));
                        }
                    }
                    st.renaming = None;
                }
            } else {
                let room = ui.available_width() - 230.0;
                let mut text = RichText::new(widgets::ellipsize(&label, room));
                if !enabled {
                    text = text.strikethrough().weak();
                }
                let r = ui.selectable_label(selected, text).on_hover_text(&label);
                if r.clicked() {
                    st.selected = Some(sel.clone());
                }
                if r.double_clicked() {
                    st.renaming = Some((sel.clone(), label.clone()));
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::small_icon_button(ui, icons::TRASH, "Delete").clicked() {
                    action = Some(match e {
                        Entry::On { r, .. } => {
                            let r = r.clone();
                            Box::new(move |cx: &mut Ctx, st: &mut State| {
                                let _ = cx.doc.set_label(&r, None);
                                let _ = cx.doc.remove(&r);
                                st.selected = None;
                            })
                        }
                        Entry::Off { d, .. } => {
                            let d = d.clone();
                            Box::new(move |cx: &mut Ctx, st: &mut State| {
                                cx.doc.remove_disabled(&d);
                                st.selected = None;
                            })
                        }
                    });
                }
                if widgets::small_icon_button(ui, icons::PENCIL_SIMPLE, "Rename").clicked() {
                    st.renaming = Some((sel.clone(), label.clone()));
                }
                if let Entry::On { idx, r } = e {
                    let pos = on_refs.iter().position(|x| x == r).unwrap_or(0);
                    let next = on_refs.get(pos + 1).filter(|n| n.file == r.file).cloned();
                    let prev = pos.checked_sub(1).and_then(|p| on_refs.get(p)).filter(|n| n.file == r.file).cloned();
                    if ui.add_enabled(next.is_some(), widgets::icon_only(icons::CARET_DOWN)).on_hover_text("Move down").clicked() {
                        let (a, b, i) = (r.clone(), next.clone().unwrap(), *idx);
                        action = Some(Box::new(move |cx: &mut Ctx, st: &mut State| {
                            let _ = cx.doc.swap_blocks(&a, &b);
                            st.selected = Some(Sel::On(i + 1));
                        }));
                    }
                    if ui.add_enabled(prev.is_some(), widgets::icon_only(icons::CARET_UP)).on_hover_text("Move up").clicked() {
                        let (a, b, i) = (r.clone(), prev.clone().unwrap(), *idx);
                        action = Some(Box::new(move |cx: &mut Ctx, st: &mut State| {
                            let _ = cx.doc.swap_blocks(&a, &b);
                            st.selected = Some(Sel::On(i - 1));
                        }));
                    }
                    if widgets::small_icon_button(ui, icons::COPY, "Duplicate").clicked() {
                        let r = r.clone();
                        action = Some(Box::new(move |cx: &mut Ctx, _| {
                            let text = cx.doc.node_text(&r).unwrap_or_default().to_owned();
                            let label = cx.doc.label(&r).map(|l| format!("// {l} (copy)\n")).unwrap_or_default();
                            let _ = cx.doc.append(None, &format!("{label}{text}"));
                        }));
                    }
                    if st.tab == Tab::Window {
                        let n = windows.iter().filter(|w| rule_matches(cx, r, w) == Some(true)).count();
                        if n > 0 {
                            ui.label(RichText::new(format!("{n} open")).small().color(crate::theme::pal(ui.ctx()).accent_fg()))
                                .on_hover_text(format!("Applies to {n} open window{}", crate::app::plural(n)));
                        }
                    }
                }
            });
        });
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button(format!("{} New {}", icons::PLUS, st.tab.node())).clicked() {
            let node = st.tab.node();
            action = Some(Box::new(move |cx: &mut Ctx, st: &mut State| {
                let before = cx.doc.find_all(&[Seg::Name(node.into())]).len();
                let _ = cx.doc.append(None, &format!("// New rule\n{node} {{\n}}"));
                st.selected = Some(Sel::On(before));
            }));
        }
        ui.weak("Double-click a rule to rename it.");
    });
    if let Some(a) = action {
        a(cx, st);
    }
}

/// A short description of a disabled rule's text: its first matcher, else its first line.
fn first_line(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .find(|l| l.starts_with("match "))
        .map(|l| l.trim_start_matches("match ").to_owned())
        .unwrap_or_else(|| s.lines().next().unwrap_or("").trim_end_matches('{').trim().to_owned())
}

/// A readable name for an unlabelled rule: its first matcher and a property or two.
fn summary(cx: &Ctx, r: &NodeRef, idx: usize) -> String {
    let mut parts = Vec::new();
    for c in cx.doc.children(r) {
        let Some(n) = cx.doc.node(&c) else { continue };
        match n.name().value() {
            "match" if parts.is_empty() => {
                let p: Vec<String> = n
                    .entries()
                    .iter()
                    .filter_map(|e| Some(format!("{}={}", e.name()?.value(), Val::from_kdl(e.value()).as_str().map(str::to_owned).unwrap_or_else(|| e.value().to_string()))))
                    .collect();
                parts.push(p.join(" "));
            }
            "match" | "exclude" => {}
            other if parts.len() < 3 => parts.push(other.to_owned()),
            _ => {}
        }
    }
    if parts.is_empty() { format!("Rule {} (empty)", idx + 1) } else { parts.join(" · ") }
}

/// Matcher conditions of a rule as (is_exclude, [(key, value)]).
fn conditions(cx: &Ctx, r: &NodeRef) -> Vec<(bool, Vec<(String, Val)>)> {
    cx.doc
        .children(r)
        .into_iter()
        .filter_map(|c| {
            let n = cx.doc.node(&c)?;
            let ex = match n.name().value() {
                "match" => false,
                "exclude" => true,
                _ => return None,
            };
            let props = n.entries().iter().filter_map(|e| Some((e.name()?.value().to_owned(), Val::from_kdl(e.value())))).collect();
            Some((ex, props))
        })
        .collect()
}

/// Whether a matcher fits a window. `None` when a regex doesn't compile.
fn fits(w: &Window, conds: &[(String, Val)]) -> Option<bool> {
    for (k, v) in conds {
        let ok = match (k.as_str(), v) {
            ("app-id", Val::Str(re)) => Regex::new(re).ok()?.is_match(w.app_id.as_deref().unwrap_or_default()) && w.app_id.is_some(),
            ("title", Val::Str(re)) => Regex::new(re).ok()?.is_match(w.title.as_deref().unwrap_or_default()) && w.title.is_some(),
            ("is-floating", Val::Bool(b)) => w.is_floating == *b,
            ("is-focused" | "is-active", Val::Bool(b)) => w.is_focused == *b,
            ("is-urgent", Val::Bool(b)) => w.is_urgent == *b,
            // Not observable over IPC; assume they hold.
            _ => true,
        };
        if !ok {
            return Some(false);
        }
    }
    Some(true)
}

fn rule_matches(cx: &Ctx, r: &NodeRef, w: &Window) -> Option<bool> {
    let conds = conditions(cx, r);
    let mut any_match = !conds.iter().any(|(ex, _)| !ex);
    for (ex, c) in &conds {
        let f = fits(w, c)?;
        if *ex && f {
            return Some(false);
        }
        if !ex && f {
            any_match = true;
        }
    }
    Some(any_match)
}

fn editor(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State, r: &NodeRef, idx: usize, windows: &[Window]) {
    ui.label(RichText::new("Applies to").strong());
    let children = cx.doc.children(r);
    let matchers: Vec<NodeRef> = children
        .iter()
        .filter(|c| cx.doc.node(c).is_some_and(|n| matches!(n.name().value(), "match" | "exclude")))
        .cloned()
        .collect();
    if matchers.is_empty() {
        ui.weak("No matchers: this rule applies to every window.");
    }
    let keys: &[(&str, &str)] = if st.tab == Tab::Window {
        &[("app-id", "app id"), ("title", "title")]
    } else {
        &[("namespace", "namespace")]
    };
    let mut remove = None;
    for (mi, m) in matchers.iter().enumerate() {
        let Some(node) = cx.doc.node(m) else { continue };
        let is_ex = node.name().value() == "exclude";
        let props: Vec<(String, Val)> = node.entries().iter().filter_map(|e| Some((e.name()?.value().to_owned(), Val::from_kdl(e.value())))).collect();
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut ex = is_ex;
                if let Some(v) = crate::widgets::segmented(ui, ui.id().with("seg-ex"), Some(&ex), &[(false, "match"), (true, "exclude")], false) {
                    ex = v;
                }
                if ex != is_ex {
                    let text = cx.doc.node_text(m).unwrap_or_default();
                    let rest = text.split_once(char::is_whitespace).map_or("", |(_, r)| r);
                    let _ = cx.doc.replace_node(m, &format!("{} {rest}", if ex { "exclude" } else { "match" }).trim_end());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::small_icon_button(ui, icons::X, "Remove this matcher").clicked() {
                        remove = Some(m.clone());
                    }
                });
            });
            for (key, label) in keys {
                let cur = props.iter().rev().find(|(k, _)| k == key).and_then(|(_, v)| v.as_str().map(str::to_owned));
                ui.horizontal(|ui| {
                    ui.add_sized([80.0, 20.0], egui::Label::new(*label));
                    let mut s = cur.clone().unwrap_or_default();
                    let id = ui.id().with((idx, mi, *key));
                    let resp = ui.add(egui::TextEdit::singleline(&mut s).id(id).desired_width(220.0).hint_text("regex").font(egui::TextStyle::Monospace));
                    if resp.changed() {
                        let v = (!s.is_empty()).then(|| Val::Str(s.clone()));
                        let _ = cx.doc.set_prop_ref(m, key, v, &format!("rule{idx}m{mi}{key}"));
                    }
                    if let Some(re) = &cur {
                        if let Err(e) = Regex::new(re) {
                            ui.colored_label(ui.visuals().error_fg_color, "invalid").on_hover_text(e.to_string());
                        }
                    }
                });
            }
            if st.tab == Tab::Window {
                ui.horizontal_wrapped(|ui| {
                    for (flag, label) in schema::WINDOW_MATCH_FLAGS {
                        let cur = props.iter().rev().find(|(k, _)| k == flag).and_then(|(_, v)| v.as_bool());
                        let text = match cur {
                            None => RichText::new(*label).weak(),
                            Some(true) => RichText::new(format!("{label}")).strong(),
                            Some(false) => RichText::new(format!("not {label}")).strong(),
                        };
                        if ui.button(text).on_hover_text("Click to cycle: any → yes → no").clicked() {
                            let next = match cur {
                                None => Some(Val::Bool(true)),
                                Some(true) => Some(Val::Bool(false)),
                                Some(false) => None,
                            };
                            let _ = cx.doc.set_prop_ref(m, flag, next, &format!("rule{idx}m{mi}{flag}-click"));
                            cx.doc.break_coalescing();
                        }
                    }
                });
            } else {
                let at = props.iter().rev().find(|(k, _)| k == "at-startup").and_then(|(_, v)| v.as_bool());
                let mut on = at == Some(true);
                if widgets::checkbox(ui, &mut on, "only at startup").changed() {
                    let _ = cx.doc.set_prop_ref(m, "at-startup", on.then_some(Val::Bool(true)), "layer-at-startup");
                }
            }
        });
    }
    if let Some(m) = remove {
        let _ = cx.doc.remove(&m);
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button(format!("{} match", icons::PLUS)).clicked() {
            let _ = cx.doc.append(Some(r), "match");
        }
        if ui.button(format!("{} exclude", icons::PLUS)).clicked() {
            let _ = cx.doc.append(Some(r), "exclude");
        }
        if st.tab == Tab::Window && cx.live.state().connected {
            if ui.button(format!("{} Pick a window to match", icons::CURSOR_CLICK)).clicked() {
                start_pick(cx, st, Purpose::AddMatch { rule: idx, exclude: false });
            }
            if ui.button(format!("{} Pick a window to exclude", icons::CURSOR_CLICK)).clicked() {
                start_pick(cx, st, Purpose::AddMatch { rule: idx, exclude: true });
            }
        }
    });
    if st.tab == Tab::Layer {
        if let Some(ns) = &st.namespaces {
            if !ns.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.weak("Open layers:");
                    for n in ns {
                        ui.label(RichText::new(n).monospace());
                    }
                });
            }
        }
    }

    if st.tab == Tab::Window {
        let hits: Vec<&Window> = windows.iter().filter(|w| rule_matches(cx, r, w) == Some(true)).collect();
        ui.add_space(4.0);
        if windows.is_empty() {
            ui.weak("No open windows to test against.");
        } else if hits.is_empty() {
            ui.weak("Matches none of the open windows.");
        } else {
            ui.label(format!("Matches {} open window{}:", hits.len(), crate::app::plural(hits.len())));
            for w in hits.iter().take(8) {
                ui.label(
                    RichText::new(format!("  {}  —  {}", w.app_id.as_deref().unwrap_or("?"), w.title.as_deref().unwrap_or("")))
                        .small()
                        .color(crate::theme::pal(ui.ctx()).faint),
                );
            }
        }
    }

    ui.separator();
    ui.label(RichText::new("Properties").strong());
    let props = st.tab.props();
    let set: Vec<&RuleProp> = props.iter().filter(|(n, ..)| cx.doc.has_in(Some(r), &[Seg::Name((*n).into())])).collect();
    for (name, kind, label, help) in &set {
        widgets::option_at(ui, cx, Some(r), &[Seg::Name((*name).into())], *kind, label, help, false);
    }
    if set.is_empty() {
        ui.weak("No properties yet.");
    }
    let mut add = None;
    egui::ComboBox::from_id_salt(("add-prop", idx)).selected_text(format!("{} Add property", icons::PLUS)).show_ui(ui, |ui| {
        for p in props.iter().filter(|p| !set.iter().any(|s| s.0 == p.0)) {
            let r = ui.selectable_label(false, p.2);
            let r = if p.3.is_empty() { r } else { r.on_hover_text(p.3) };
            if r.clicked() {
                add = Some(*p);
            }
        }
    });
    if let Some((name, kind, ..)) = add {
        let v = match kind {
            Kind::Bool | Kind::Flag => Val::Bool(true),
            Kind::Float { min, max } => Val::Float(if (min..=max).contains(&1.0) { if max <= 1.0 { 0.9 } else { 12.0 } } else { min }),
            Kind::Int { .. } => Val::Int(400),
            Kind::Enum(v) => Val::str(v[0]),
            Kind::Str | Kind::Path | Kind::Color => Val::str(""),
        };
        let _ = cx.doc.set_arg_in(Some(r), &[Seg::Name(name.into())], v);
    }
    let _ = &st.renaming;
}

/// Registers the egui context for background pick threads to wake the UI.
pub fn set_context(ctx: &egui::Context) {
    CTX.with(|c| *c.borrow_mut() = Some(ctx.clone()));
}
