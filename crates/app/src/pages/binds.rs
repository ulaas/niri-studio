//! Key bindings: search, conflict detection, a key recorder, an action picker and "run now".

use std::collections::HashMap;

use doc::{NodeRef, Seg, Val};
use egui::{Key, RichText};

use crate::app::Ctx;
use crate::widgets;

const MODIFIERS: [&str; 5] = ["Mod", "Super", "Ctrl", "Alt", "Shift"];

#[derive(Default)]
pub struct State {
    filter: String,
    pub selected: Option<usize>,
    recording: bool,
}

#[derive(Clone)]
struct Bind {
    r: NodeRef,
    key: String,
    props: Vec<(String, Val)>,
    action: String,
    args: Vec<Val>,
    /// Action properties (like `show-pointer=false`), kept as written.
    action_props: Vec<String>,
}

impl Bind {
    fn to_kdl(&self) -> String {
        let mut s = doc::value::ident(&self.key);
        for (k, v) in &self.props {
            s += &format!(" {}={}", doc::value::ident(k), v.to_kdl());
        }
        s += &format!(" {{ {}", doc::value::ident(&self.action));
        for a in &self.args {
            s += &format!(" {}", a.to_kdl());
        }
        for p in &self.action_props {
            s += &format!(" {p}");
        }
        s + "; }"
    }

    fn prop(&self, k: &str) -> Option<&Val> {
        self.props.iter().find(|(n, _)| n == k).map(|(_, v)| v)
    }

    fn set_prop(&mut self, k: &str, v: Option<Val>) {
        self.props.retain(|(n, _)| n != k);
        if let Some(v) = v {
            self.props.push((k.to_owned(), v));
        }
    }

    fn summary(&self) -> String {
        let args: Vec<String> = self.args.iter().map(|a| a.as_str().map_or_else(|| a.to_kdl(), str::to_owned)).collect();
        if args.is_empty() { self.action.clone() } else { format!("{} {}", self.action, args.join(" ")) }
    }
}

fn binds(cx: &Ctx) -> Vec<Bind> {
    let Some(binds) = cx.doc.find(&[Seg::Name("binds".into())]) else { return Vec::new() };
    cx.doc
        .children(&binds)
        .into_iter()
        .filter_map(|r| {
            let n = cx.doc.node(&r)?;
            let props = n.entries().iter().filter_map(|e| Some((e.name()?.value().to_owned(), Val::from_kdl(e.value())))).collect();
            let action = n.children().and_then(|c| c.nodes().first());
            let (name, args, aprops) = match action {
                Some(a) => {
                    let text = &cx.doc.files[r.file].text;
                    let aprops = a
                        .entries()
                        .iter()
                        .filter(|e| e.name().is_some())
                        .map(|e| text[e.span().offset()..e.span().offset() + e.span().len()].to_owned())
                        .collect();
                    (a.name().value().to_owned(), doc::node_args(a), aprops)
                }
                None => (String::new(), Vec::new(), Vec::new()),
            };
            Some(Bind { key: n.name().value().to_owned(), r, props, action: name, args, action_props: aprops })
        })
        .collect()
}

/// Canonical form of a key combo for conflict detection.
fn normalize(key: &str) -> String {
    let parts: Vec<String> = key.split('+').map(|p| p.trim().to_lowercase()).collect();
    let (key, mods) = parts.split_last().map_or((String::new(), &[][..]), |(k, m)| (k.clone(), m));
    let mut mods: Vec<String> = mods
        .iter()
        .map(|m| match m.as_str() {
            "control" => "ctrl".to_owned(),
            "win" | "logo" => "super".to_owned(),
            "mod1" => "alt".to_owned(),
            "mod4" => "super".to_owned(),
            other => other.to_owned(),
        })
        .collect();
    mods.sort();
    mods.dedup();
    format!("{}+{key}", mods.join("+"))
}

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State) {
    widgets::heading(ui, "Key bindings", "Mod is Super normally, and Alt when niri runs nested. Click ▶ to try an action right now.");
    let all = binds(cx);
    let mut counts: HashMap<String, usize> = HashMap::new();
    for b in &all {
        *counts.entry(normalize(&b.key)).or_default() += 1;
    }
    let conflicts = counts.values().filter(|&&n| n > 1).count();

    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut st.filter).hint_text("Filter by key, action or title…").desired_width(320.0));
        ui.weak(format!("{} bindings", all.len()));
        if conflicts > 0 {
            ui.colored_label(ui.visuals().warn_fg_color, format!("⚠ {conflicts} key combination{} bound more than once", crate::app::plural(conflicts)));
        }
        if ui.button("➕ New binding").clicked() {
            add_bind(cx, st, &all);
        }
        if ui.button("⌨ Show hotkey overlay").on_hover_text("niri's overlay, as of the last saved config").clicked() {
            if let Err(e) = live::run_action("show-hotkey-overlay", &[], &[]) {
                cx.toasts.error(format!("{e:#}"));
            }
        }
    });
    ui.add_space(4.0);

    ui.columns(2, |cols| {
        widgets::card(&mut cols[0], "Bindings", |ui| {
            let f = st.filter.to_lowercase();
            egui::ScrollArea::vertical().id_salt("binds-list").max_height((ui.ctx().content_rect().height() - 240.0).max(300.0)).show(ui, |ui| {
                for (i, b) in all.iter().enumerate() {
                    let title = b.prop("hotkey-overlay-title").and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
                    if !f.is_empty() && ![&b.key, &b.summary(), &title].iter().any(|s| s.to_lowercase().contains(&f)) {
                        continue;
                    }
                    let dup = counts.get(&normalize(&b.key)).copied().unwrap_or(0) > 1;
                    ui.horizontal(|ui| {
                        let key = RichText::new(&b.key).monospace().strong();
                        let key = if dup { key.color(ui.visuals().warn_fg_color) } else { key };
                        let r = ui.add_sized([170.0, 20.0], egui::Button::selectable(st.selected == Some(i), key));
                        if dup {
                            r.clone().on_hover_text("This combination is bound more than once; niri uses the last one.");
                        }
                        if r.clicked() {
                            st.selected = Some(i);
                            st.recording = false;
                        }
                        let room = ui.available_width() - 30.0;
                        let text = if title.is_empty() { b.summary() } else { title.clone() };
                        ui.label(widgets::ellipsize(&text, room)).on_hover_text(b.summary());
                    });
                }
            });
        });
        let ui = &mut cols[1];
        match st.selected.and_then(|i| all.get(i)).cloned() {
            Some(b) => widgets::card(ui, &b.key.clone(), |ui| editor(ui, cx, st, b, &counts)),
            None => widgets::card(ui, "Editor", |ui| {
                ui.weak("Select a binding to edit it.");
            }),
        }
    });
}

fn add_bind(cx: &mut Ctx, st: &mut State, all: &[Bind]) {
    let taken: Vec<String> = all.iter().map(|b| normalize(&b.key)).collect();
    let key = (1..=12).map(|n| format!("Mod+Shift+F{n}")).find(|k| !taken.contains(&normalize(k))).unwrap_or_else(|| "Mod+Shift+F12".into());
    let res = cx
        .doc
        .ensure(&[Seg::Name("binds".into())])
        .and_then(|b| cx.doc.append(Some(&b), &format!("{key} {{ spawn \"foot\"; }}")));
    match res {
        Ok(_) => st.selected = Some(all.len()),
        Err(e) => cx.toasts.error(format!("{e:#}")),
    }
}

fn editor(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State, b: Bind, counts: &HashMap<String, usize>) {
    let mut nb = b.clone();

    // Key combination.
    let parts: Vec<&str> = b.key.split('+').collect();
    let (main_key, mods) = parts.split_last().map_or(("", &[][..]), |(k, m)| (*k, m));
    let mut mods: Vec<String> = mods.iter().map(|s| s.to_string()).collect();
    let mut key = main_key.to_owned();
    let mut key_changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.label("Keys");
        for m in MODIFIERS {
            let mut on = mods.iter().any(|x| x.eq_ignore_ascii_case(m) || (m == "Ctrl" && x.eq_ignore_ascii_case("Control")));
            if ui.toggle_value(&mut on, m).changed() {
                key_changed = true;
                mods.retain(|x| !(x.eq_ignore_ascii_case(m) || (m == "Ctrl" && x.eq_ignore_ascii_case("Control"))));
                if on {
                    mods.push(m.to_owned());
                }
            }
        }
        ui.label("+");
        key_changed |= ui.add(egui::TextEdit::singleline(&mut key).desired_width(110.0).font(egui::TextStyle::Monospace)).changed();
        let rec = if st.recording { "⏺ press a key… (Esc cancels)" } else { "⏺ Record" };
        if ui.toggle_value(&mut st.recording, rec).on_hover_text("Press the key to bind. Ctrl/Alt/Shift held while pressing are captured; toggle Mod/Super above").changed() {}
    });
    if st.recording {
        let pressed = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        if let Some((k, m)) = pressed {
            if k == Key::Escape && !m.any() {
                st.recording = false;
            } else if let Some(name) = xkb_name(k) {
                key = name.to_owned();
                key_changed = true;
                for (on, n) in [(m.ctrl, "Ctrl"), (m.alt, "Alt"), (m.shift, "Shift")] {
                    if on && !mods.iter().any(|x| x.eq_ignore_ascii_case(n)) {
                        mods.push(n.to_owned());
                    }
                }
                st.recording = false;
            }
        }
    }
    // Only rebuild the combination when the user changed it, so selecting a bind never
    // rewrites how it's spelled.
    if key_changed {
        let order = |m: &String| MODIFIERS.iter().position(|x| x.eq_ignore_ascii_case(m)).unwrap_or(9);
        mods.sort_by_key(order);
        nb.key = mods.iter().map(String::as_str).chain([key.as_str()]).collect::<Vec<_>>().join("+");
    }
    if counts.get(&normalize(&nb.key)).copied().unwrap_or(0) > usize::from(normalize(&nb.key) == normalize(&b.key)) {
        ui.colored_label(ui.visuals().warn_fg_color, "⚠ This combination is already bound elsewhere.");
    }

    ui.add_space(6.0);
    // Action.
    ui.horizontal(|ui| {
        ui.label("Action");
        egui::ComboBox::from_id_salt("bind-action").width(260.0).selected_text(&nb.action).height(400.0).show_ui(ui, |ui| {
            for (name, _) in schema::ACTIONS {
                if ui.selectable_label(nb.action == *name, *name).clicked() && nb.action != *name {
                    nb.action = (*name).to_owned();
                    nb.args.clear();
                    nb.action_props.clear();
                }
            }
        });
    });
    let hint = schema::ACTIONS.iter().find(|(n, _)| *n == nb.action).map_or("", |(_, h)| *h);
    if nb.action == "spawn" {
        let mut line = widgets::join_args(&nb.args.iter().filter_map(|a| a.as_str().map(str::to_owned)).collect::<Vec<_>>());
        ui.horizontal(|ui| {
            ui.label("Command");
            if ui.add(egui::TextEdit::singleline(&mut line).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace)).changed() {
                nb.args = widgets::split_args(&line).into_iter().map(Val::Str).collect();
            }
        });
        ui.weak("Arguments are split like a shell would, but no shell runs. Use spawn-sh for pipes and variables.");
    } else if !hint.is_empty() || !nb.args.is_empty() {
        let mut arg = nb.args.first().map(|a| a.as_str().map_or_else(|| a.to_kdl(), str::to_owned)).unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Argument");
            if ui.add(egui::TextEdit::singleline(&mut arg).hint_text(hint).desired_width(f32::INFINITY)).changed() {
                nb.args = if arg.is_empty() {
                    Vec::new()
                } else if nb.action != "spawn-sh" && arg.parse::<i64>().is_ok() {
                    vec![Val::Int(arg.parse().unwrap())]
                } else {
                    vec![Val::Str(arg)]
                };
            }
        });
    }

    ui.add_space(6.0);
    // Bind properties.
    let mut title = nb.prop("hotkey-overlay-title").and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Overlay title");
        if ui.add(egui::TextEdit::singleline(&mut title).hint_text("shown in the hotkey overlay").desired_width(f32::INFINITY)).changed() {
            nb.set_prop("hotkey-overlay-title", (!title.is_empty()).then(|| Val::Str(title.clone())));
        }
    });
    let flag = |nb: &mut Bind, ui: &mut egui::Ui, key: &str, label: &str, default: bool, help: &str| {
        let mut on = nb.prop(key).and_then(Val::as_bool).unwrap_or(default);
        let (changed, _) = widgets::row(ui, label, help, nb.prop(key).is_some(), false, |ui| widgets::switch(ui, &mut on).changed());
        if changed {
            nb.set_prop(key, (on != default).then_some(Val::Bool(on)));
        }
    };
    flag(&mut nb, ui, "repeat", "Repeat while held", true, "");
    flag(&mut nb, ui, "allow-when-locked", "Works on the lock screen", false, "Only for spawn actions.");
    flag(&mut nb, ui, "allow-inhibiting", "Apps can inhibit it", true, "Off keeps it working in VMs and remote desktops that grab the keyboard.");
    let mut cooldown = nb.prop("cooldown-ms").and_then(Val::as_i64).unwrap_or(0);
    let (changed, _) = widgets::row(ui, "Cooldown (ms)", "Ignore repeats within this time; useful for wheel binds.", nb.prop("cooldown-ms").is_some(), false, |ui| {
        ui.add(egui::Slider::new(&mut cooldown, 0..=2000)).changed()
    });
    if changed {
        nb.set_prop("cooldown-ms", (cooldown > 0).then_some(Val::Int(cooldown)));
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("▶ Run now").on_hover_text("Runs this action in niri right away, without saving").clicked() {
            let args: Vec<String> = nb.args.iter().map(|a| a.as_str().map_or_else(|| a.to_kdl(), str::to_owned)).collect();
            let props: Vec<(String, String)> = nb
                .action_props
                .iter()
                .filter_map(|p| p.split_once('=').map(|(k, v)| (k.to_owned(), v.trim_matches('"').to_owned())))
                .collect();
            if let Err(e) = live::run_action(&nb.action, &args, &props) {
                cx.toasts.error(format!("{e:#}"));
            }
        }
        if ui.button("🗑 Delete binding").clicked() {
            let _ = cx.doc.remove(&b.r);
            st.selected = None;
        }
    });
    ui.add_space(4.0);
    ui.label(RichText::new(nb.to_kdl()).monospace().weak());

    if nb.to_kdl() != b.to_kdl() {
        if let Err(e) = cx.doc.replace_node(&b.r, &nb.to_kdl()) {
            cx.toasts.error(format!("{e:#}"));
        }
    }
}

/// XKB keysym names for egui keys.
fn xkb_name(k: Key) -> Option<&'static str> {
    use Key::*;
    Some(match k {
        A => "A", B => "B", C => "C", D => "D", E => "E", F => "F", G => "G", H => "H", I => "I",
        J => "J", K => "K", L => "L", M => "M", N => "N", O => "O", P => "P", Q => "Q", R => "R",
        S => "S", T => "T", U => "U", V => "V", W => "W", X => "X", Y => "Y", Z => "Z",
        Num0 => "0", Num1 => "1", Num2 => "2", Num3 => "3", Num4 => "4",
        Num5 => "5", Num6 => "6", Num7 => "7", Num8 => "8", Num9 => "9",
        F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6",
        F7 => "F7", F8 => "F8", F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12",
        ArrowLeft => "Left", ArrowRight => "Right", ArrowUp => "Up", ArrowDown => "Down",
        Enter => "Return", Escape => "Escape", Tab => "Tab", Space => "Space",
        Backspace => "BackSpace", Delete => "Delete", Insert => "Insert",
        Home => "Home", End => "End", PageUp => "Page_Up", PageDown => "Page_Down",
        Minus => "Minus", Equals => "Equal", Plus => "Plus", Comma => "Comma", Period => "Period",
        Slash => "Slash", Backslash => "Backslash", Semicolon => "Semicolon", Quote => "Apostrophe",
        OpenBracket => "BracketLeft", CloseBracket => "BracketRight", Backtick => "grave",
        _ => return None,
    })
}
