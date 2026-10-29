//! Key bindings: search, conflict detection, a key recorder, an action picker and "run now".

use std::collections::HashMap;

use doc::{NodeRef, Seg, Val};
use egui::{Key, RichText};

use crate::theme::icons;
use crate::app::Ctx;
use crate::widgets;

const MODIFIERS: [&str; 5] = ["Mod", "Super", "Ctrl", "Alt", "Shift"];

#[derive(Default)]
pub struct State {
    filter: String,
    pub selected: Option<usize>,
    recording: bool,
    /// Modifiers held so far while recording; releasing them all without a key records a
    /// modifier-only combination.
    rec_mods: egui::Modifiers,
}

/// One action, as in `close-window` or `spawn "foot"`.
#[derive(Clone, Default, PartialEq)]
struct Act {
    name: String,
    args: Vec<Val>,
    /// Action properties (like `show-pointer=false`), kept as written.
    props: Vec<String>,
}

impl Act {
    fn parse(a: &kdl::KdlNode, text: &str) -> Self {
        let props = a
            .entries()
            .iter()
            .filter(|e| e.name().is_some())
            .map(|e| text[e.span().offset()..e.span().offset() + e.span().len()].to_owned())
            .collect();
        Act { name: a.name().value().to_owned(), args: doc::node_args(a), props }
    }

    fn to_kdl(&self) -> String {
        let mut s = doc::value::ident(&self.name);
        for a in &self.args {
            s += &format!(" {}", a.to_kdl());
        }
        for p in &self.props {
            s += &format!(" {p}");
        }
        s + ";"
    }

    fn summary(&self) -> String {
        let args: Vec<String> = self.args.iter().map(|a| a.as_str().map_or_else(|| a.to_kdl(), str::to_owned)).collect();
        if args.is_empty() { self.name.clone() } else { format!("{} {}", self.name, args.join(" ")) }
    }

    fn run(&self) -> anyhow::Result<()> {
        let args: Vec<String> = self.args.iter().map(|a| a.as_str().map_or_else(|| a.to_kdl(), str::to_owned)).collect();
        let props: Vec<(String, String)> =
            self.props.iter().filter_map(|p| p.split_once('=').map(|(k, v)| (k.to_owned(), v.trim_matches('"').to_owned()))).collect();
        live::run_action(&self.name, &args, &props)
    }
}

/// When a bind fires: on key press (niri's default), on release, or both.
#[derive(Clone, Copy, PartialEq)]
enum When {
    Press,
    Release,
    Both,
}

#[derive(Clone)]
struct Bind {
    r: NodeRef,
    key: String,
    props: Vec<(String, Val)>,
    press: Option<Act>,
    release: Option<Act>,
    /// Written with `press { }` / `release { }` blocks rather than a bare action.
    blocks: bool,
}

impl Bind {
    fn to_kdl(&self) -> String {
        let mut s = doc::value::ident(&self.key);
        for (k, v) in &self.props {
            s += &format!(" {}={}", doc::value::ident(k), v.to_kdl());
        }
        if !self.blocks && self.release.is_none() {
            return s + &format!(" {{ {} }}", self.press.as_ref().map(Act::to_kdl).unwrap_or_default());
        }
        s += " {";
        for (block, act) in [("press", &self.press), ("release", &self.release)] {
            if let Some(act) = act {
                s += &format!("\n    {block} {{ {} }}", act.to_kdl());
            }
        }
        s + "\n}"
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

    fn when(&self) -> When {
        match (&self.press, &self.release) {
            (Some(_), Some(_)) => When::Both,
            (None, Some(_)) => When::Release,
            _ => When::Press,
        }
    }

    fn set_when(&mut self, when: When) {
        match when {
            When::Press => {
                self.press = self.press.take().or_else(|| self.release.clone());
                self.release = None;
            }
            When::Release => {
                self.release = self.release.take().or_else(|| self.press.clone());
                self.press = None;
            }
            When::Both => {
                self.press = self.press.take().or_else(|| self.release.clone());
                self.release = self.release.take().or_else(|| self.press.clone());
            }
        }
        self.blocks = when != When::Press;
    }

    fn summary(&self) -> String {
        match (&self.press, &self.release) {
            (Some(p), Some(r)) => format!("{} · on release: {}", p.summary(), r.summary()),
            (None, Some(r)) => format!("on release: {}", r.summary()),
            (Some(p), None) => p.summary(),
            (None, None) => String::new(),
        }
    }
}

fn binds(cx: &Ctx) -> Vec<Bind> {
    let Some(binds) = cx.doc.find(&[Seg::Name("binds".into())]) else { return Vec::new() };
    cx.doc
        .children(&binds)
        .into_iter()
        .filter_map(|r| {
            let n = cx.doc.node(&r)?;
            let text = &cx.doc.files[r.file].text;
            let props = n.entries().iter().filter_map(|e| Some((e.name()?.value().to_owned(), Val::from_kdl(e.value())))).collect();
            let children = n.children().map(|c| c.nodes()).unwrap_or_default();
            let blocks = children.first().is_some_and(|c| matches!(c.name().value(), "press" | "release"));
            let block = |name: &str| {
                children
                    .iter()
                    .find(|c| c.name().value() == name)
                    .map(|c| c.children().and_then(|c| c.nodes().first()).map(|a| Act::parse(a, text)).unwrap_or_default())
            };
            let (press, release) = if blocks {
                (block("press"), block("release"))
            } else {
                (Some(children.first().map(|a| Act::parse(a, text)).unwrap_or_default()), None)
            };
            Some(Bind { key: n.name().value().to_owned(), r, props, press, release, blocks })
        })
        .collect()
}

/// Canonical name of a generic modifier (one that can be held while pressing another key).
fn generic_modifier(m: &str) -> Option<&'static str> {
    Some(match m.to_lowercase().as_str() {
        "mod" => "mod",
        "ctrl" | "control" => "ctrl",
        "shift" => "shift",
        "alt" | "mod1" => "alt",
        "super" | "win" | "logo" | "mod4" => "super",
        "iso_level3_shift" | "mod5" => "mod5",
        "iso_level5_shift" => "iso_level5_shift",
        _ => return None,
    })
}

/// The generic modifier a side-specific modifier key like `Alt_L` implies.
fn implied_modifier(key: &str) -> Option<&'static str> {
    let (base, side) = key.rsplit_once('_')?;
    if !matches!(side.to_lowercase().as_str(), "l" | "r") {
        return None;
    }
    generic_modifier(base)
}

/// Canonical form of a key combo for conflict detection. Binds made only of generic modifiers
/// don't depend on press order, and `Alt+Alt_L` is the same as `Alt_L`.
fn normalize(key: &str) -> String {
    let parts: Vec<String> = key.split('+').map(|p| p.trim().to_lowercase()).collect();
    let (mut key, mods) = parts.split_last().map_or((String::new(), &[][..]), |(k, m)| (k.clone(), m));
    let mut mods: Vec<String> = mods.iter().map(|m| generic_modifier(m).map_or_else(|| m.clone(), str::to_owned)).collect();
    if let Some(m) = generic_modifier(&key) {
        mods.push(m.to_owned());
        key.clear();
    } else if let Some(m) = implied_modifier(&key) {
        mods.retain(|x| x != m);
    }
    mods.sort();
    mods.dedup();
    format!("{}+{key}", mods.join("+"))
}

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State) {
    widgets::heading(ui, "Key bindings", &format!("Mod is Super normally, and Alt when niri runs nested. Click {} to try an action right now.", icons::PLAY));
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
            ui.colored_label(ui.visuals().warn_fg_color, format!("{} {conflicts} key combination{} bound more than once", icons::WARNING, crate::app::plural(conflicts)));
        }
        if ui.button(format!("{} New binding", icons::PLUS)).clicked() {
            add_bind(cx, st, &all);
        }
        if ui.button(format!("{} Show hotkey overlay", icons::KEYBOARD)).on_hover_text("niri's overlay, as of the last saved config").clicked() {
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
                    let (rect, r) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
                    widgets::list_row_bg(ui, &r, rect, st.selected == Some(i));
                    let p = crate::theme::pal(ui.ctx());
                    let end = widgets::keycaps(ui, rect.left_center() + egui::vec2(8.0, 0.0), &b.key, if dup { p.warn } else { p.text });
                    let x = end.max(rect.left() + 200.0);
                    let text = if title.is_empty() { b.summary() } else { title.clone() };
                    let text = widgets::ellipsize(&text, rect.right() - x - 8.0);
                    let color = if title.is_empty() { p.muted } else { p.text };
                    ui.painter().text(egui::pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(13.5), color);
                    let r = if dup { r.on_hover_text("This combination is bound more than once; niri uses the last one.") } else { r.on_hover_text(b.summary()) };
                    if r.clicked() {
                        st.selected = Some(i);
                        st.recording = false;
                    }
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

    // Key combination. A combination ending in a generic modifier (`Mod`, `Alt+Shift`) is
    // modifier-only and has no separate trigger key.
    let parts: Vec<&str> = b.key.split('+').collect();
    let (main_key, mods) = match parts.split_last() {
        Some((k, _)) if generic_modifier(k).is_some() => ("", &parts[..]),
        Some((k, m)) => (*k, m),
        None => ("", &[][..]),
    };
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
        key_changed |= ui
            .add(egui::TextEdit::singleline(&mut key).hint_text("modifiers only").desired_width(110.0).font(egui::TextStyle::Monospace))
            .on_hover_text("An XKB key name, or a modifier key like Alt_L. Leave empty to bind the modifiers alone.")
            .changed();
        let rec = if st.recording { format!("{} press a key… (Esc cancels)", icons::RECORD) } else { format!("{} Record", icons::RECORD) };
        if ui
            .toggle_value(&mut st.recording, rec)
            .on_hover_text("Press the key to bind. Ctrl/Alt/Shift held while pressing are captured; press and release modifiers alone to bind just them. Toggle Mod/Super above")
            .changed()
        {
            st.rec_mods = egui::Modifiers::NONE;
        }
    });
    if st.recording {
        let (pressed, held) = ui.input(|i| {
            let pressed = i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            });
            (pressed, i.modifiers)
        });
        let mut record = |key_name: &str, m: egui::Modifiers, mods: &mut Vec<String>| {
            key = key_name.to_owned();
            key_changed = true;
            for (on, n) in [(m.ctrl, "Ctrl"), (m.alt, "Alt"), (m.shift, "Shift")] {
                if on && !mods.iter().any(|x| x.eq_ignore_ascii_case(n)) {
                    mods.push(n.to_owned());
                }
            }
        };
        if let Some((k, m)) = pressed {
            if k == Key::Escape && !m.any() {
                st.recording = false;
            } else if let Some(name) = xkb_name(k) {
                record(name, m, &mut mods);
                st.recording = false;
            }
        } else if held.any() {
            st.rec_mods |= held;
        } else if st.rec_mods.any() {
            // Modifiers pressed and released with no key in between.
            record("", st.rec_mods, &mut mods);
            st.recording = false;
        }
        if !st.recording {
            st.rec_mods = egui::Modifiers::NONE;
        }
    }
    // Only rebuild the combination when the user changed it, so selecting a bind never
    // rewrites how it's spelled.
    if key_changed {
        let order = |m: &String| MODIFIERS.iter().position(|x| x.eq_ignore_ascii_case(m)).unwrap_or(9);
        mods.sort_by_key(order);
        let combo = mods.iter().map(String::as_str).chain([key.as_str()]).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("+");
        if !combo.is_empty() {
            nb.key = combo;
        }
    }
    if counts.get(&normalize(&nb.key)).copied().unwrap_or(0) > usize::from(normalize(&nb.key) == normalize(&b.key)) {
        ui.colored_label(ui.visuals().warn_fg_color, format!("{} This combination is already bound elsewhere.", icons::WARNING));
    }

    ui.add_space(6.0);
    // When it fires.
    let mut when = nb.when();
    ui.horizontal(|ui| {
        ui.label("Fires on");
        for (w, label) in [(When::Press, "Press"), (When::Release, "Release"), (When::Both, "Press and release")] {
            ui.selectable_value(&mut when, w, label);
        }
    });
    if when != nb.when() {
        nb.set_when(when);
    }
    match when {
        When::Press => {}
        When::Release => {
            ui.weak("Fires only if nothing else is pressed while the keys are held, so a modifier can still be used in other shortcuts.");
        }
        When::Both => {
            ui.weak("If the press action fires, the release action is guaranteed to fire too (e.g. for push-to-talk).");
        }
    }

    // Actions.
    let both = when == When::Both;
    if let Some(act) = nb.press.as_mut() {
        action_editor(ui, cx, "bind-action-press", if both { "On press" } else { "Action" }, act);
    }
    if let Some(act) = nb.release.as_mut() {
        action_editor(ui, cx, "bind-action-release", if both { "On release" } else { "Action" }, act);
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
    // Release-only binds never repeat; binds with both press and release don't by default.
    if when != When::Release {
        let help = if both { "Off by default for binds with a release action." } else { "" };
        flag(&mut nb, ui, "repeat", "Repeat while held", !both, help);
    }
    flag(&mut nb, ui, "allow-when-locked", "Works on the lock screen", false, "Only for spawn actions.");
    flag(&mut nb, ui, "allow-inhibiting", "Apps can inhibit it", true, "Off keeps it working in VMs and remote desktops that grab the keyboard.");
    let mut cooldown = nb.prop("cooldown-ms").and_then(Val::as_i64).unwrap_or(0);
    let (changed, _) = widgets::row(ui, "Cooldown (ms)", "Ignore repeats within this time; useful for wheel binds.", nb.prop("cooldown-ms").is_some(), false, |ui| {
        ui.add(widgets::Slider::new(&mut cooldown, 0..=2000)).changed()
    });
    if changed {
        nb.set_prop("cooldown-ms", (cooldown > 0).then_some(Val::Int(cooldown)));
    }

    ui.add_space(6.0);
    if ui.button("🗑 Delete binding").clicked() {
        let _ = cx.doc.remove(&b.r);
        st.selected = None;
    }
    ui.add_space(4.0);
    ui.label(RichText::new(nb.to_kdl()).monospace().weak());

    if nb.to_kdl() != b.to_kdl() {
        if let Err(e) = cx.doc.replace_node(&b.r, &nb.to_kdl()) {
            cx.toasts.error(format!("{e:#}"));
        }
    }
}

/// Action picker with its argument field and a button that runs it in niri right away.
fn action_editor(ui: &mut egui::Ui, cx: &mut Ctx, id: &str, label: &str, act: &mut Act) {
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(id).width(260.0).selected_text(&act.name).height(400.0).show_ui(ui, |ui| {
            for (name, _) in schema::ACTIONS {
                if ui.selectable_label(act.name == *name, *name).clicked() && act.name != *name {
                    act.name = (*name).to_owned();
                    act.args.clear();
                    act.props.clear();
                }
            }
        });
        if ui.button(icons::PLAY).on_hover_text("Runs this action in niri right away, without saving").clicked() {
            if let Err(e) = act.run() {
                cx.toasts.error(format!("{e:#}"));
            }
        }
    });
    let hint = schema::ACTIONS.iter().find(|(n, _)| *n == act.name).map_or("", |(_, h)| *h);
    if act.name == "spawn" {
        let mut line = widgets::join_args(&act.args.iter().filter_map(|a| a.as_str().map(str::to_owned)).collect::<Vec<_>>());
        ui.horizontal(|ui| {
            ui.label("Command");
            if ui.add(egui::TextEdit::singleline(&mut line).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace)).changed() {
                act.args = widgets::split_args(&line).into_iter().map(Val::Str).collect();
            }
        });
        ui.weak("Arguments are split like a shell would, but no shell runs. Use spawn-sh for pipes and variables.");
    } else if !hint.is_empty() || !act.args.is_empty() {
        let mut arg = act.args.first().map(|a| a.as_str().map_or_else(|| a.to_kdl(), str::to_owned)).unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Argument");
            if ui.add(egui::TextEdit::singleline(&mut arg).hint_text(hint).desired_width(f32::INFINITY)).changed() {
                act.args = if arg.is_empty() {
                    Vec::new()
                } else if act.name != "spawn-sh" && arg.parse::<i64>().is_ok() {
                    vec![Val::Int(arg.parse().unwrap())]
                } else {
                    vec![Val::Str(arg)]
                };
            }
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn act(name: &str) -> Option<Act> {
        Some(Act { name: name.to_owned(), ..Act::default() })
    }

    fn bind(key: &str, press: Option<Act>, release: Option<Act>, blocks: bool) -> Bind {
        Bind { r: NodeRef { file: 0, idx: Vec::new() }, key: key.to_owned(), props: Vec::new(), press, release, blocks }
    }

    fn assert_niri_accepts(b: &Bind) {
        let text = format!("binds {{\n    {}\n}}\n", b.to_kdl().replace('\n', "\n    "));
        let mut d = doc::Document::from_text(std::path::PathBuf::from("/nonexistent/config.kdl"), text.clone());
        let v = d.validation();
        assert!(v.ok(), "{}\n{text}", v.errors.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("; "));
    }

    #[test]
    fn modifier_only_binds_are_order_independent() {
        assert_eq!(normalize("Alt+Shift"), normalize("Shift+Alt"));
        assert_eq!(normalize("Control+Alt"), normalize("Alt+Ctrl"));
        assert_eq!(normalize("Alt+Alt_L"), normalize("Alt_L"));
        assert_ne!(normalize("Ctrl+Shift_L"), normalize("Shift+Control_L"));
        assert_ne!(normalize("Mod"), normalize("Mod+T"));
    }

    #[test]
    fn written_binds_are_accepted_by_niri() {
        assert_niri_accepts(&bind("Mod+T", act("close-window"), None, false));
        assert_niri_accepts(&bind("Mod", None, act("toggle-overview"), true));
        assert_niri_accepts(&bind("Alt+Shift", act("toggle-overview"), act("toggle-overview"), true));
        assert_niri_accepts(&bind("Ctrl+Alt_L", act("minimize-window"), None, true));
        assert_eq!(bind("Mod", None, act("toggle-overview"), true).to_kdl(), "Mod {\n    release { toggle-overview; }\n}");
    }

    #[test]
    fn switching_when_keeps_the_action() {
        let mut b = bind("Mod+O", act("toggle-overview"), None, false);
        b.set_when(When::Release);
        assert!(b.press.is_none() && b.release == act("toggle-overview") && b.blocks);
        b.set_when(When::Both);
        assert!(b.press == act("toggle-overview") && b.release == act("toggle-overview"));
        b.set_when(When::Press);
        assert_eq!(b.to_kdl(), "Mod+O { toggle-overview; }");
    }
}
