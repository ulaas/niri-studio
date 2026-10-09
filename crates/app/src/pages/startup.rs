//! Startup: programs to launch, shell commands, environment variables and session options.

use doc::{NodeRef, Seg, Val};
use egui::RichText;
use schema::Page;

use crate::app::Ctx;
use crate::pages::forms;
use crate::widgets;

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx) {
    widgets::heading(ui, "Startup", "What niri launches when it starts, and the environment it gives every program.");
    widgets::card(ui, "Programs", |ui| {
        ui.weak("spawn-at-startup: runs the program directly, without a shell.");
        spawn_list(ui, cx, "spawn-at-startup", false);
    });
    widgets::card(ui, "Shell commands", |ui| {
        ui.weak("spawn-sh-at-startup: runs through sh -c, so pipes, && and $VARS work.");
        spawn_list(ui, cx, "spawn-sh-at-startup", true);
    });
    widgets::card(ui, "Environment variables", |ui| environment(ui, cx));
    forms::sections(ui, cx, Page::Startup, &[]);
}

fn spawn_list(ui: &mut egui::Ui, cx: &mut Ctx, name: &str, shell: bool) {
    let nodes = cx.doc.find_all(&[Seg::Name(name.into())]);
    let mut edit: Option<(NodeRef, String)> = None;
    let mut remove = None;
    for (i, r) in nodes.iter().enumerate() {
        let args: Vec<String> = cx.doc.node(r).map(doc::node_args).unwrap_or_default().into_iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
        let mut line = if shell { args.first().cloned().unwrap_or_default() } else { widgets::join_args(&args) };
        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut line)
                    .id(ui.id().with((name, i)))
                    .font(egui::TextStyle::Monospace)
                    .desired_width(ui.available_width() - 90.0),
            );
            if resp.changed() {
                let text = if shell {
                    format!("{name} {}", Val::Str(line.clone()).to_kdl())
                } else {
                    let parts: Vec<String> = widgets::split_args(&line).iter().map(|a| Val::str(a.as_str()).to_kdl()).collect();
                    format!("{name} {}", parts.join(" "))
                };
                edit = Some((r.clone(), text));
            }
            if ui.small_button("▶").on_hover_text("Run it now through niri").clicked() {
                let (action, args) = if shell { ("spawn-sh", vec![line.clone()]) } else { ("spawn", args.clone()) };
                if let Err(e) = live::run_action(action, &args, &[]) {
                    cx.toasts.error(format!("{e:#}"));
                }
            }
            if ui.small_button("🗑").on_hover_text("Remove").clicked() {
                remove = Some(r.clone());
            }
        });
    }
    if nodes.is_empty() {
        ui.weak("Nothing yet.");
    }
    if ui.button("➕ Add").clicked() {
        let placeholder = if shell { "\"echo hello\"" } else { "\"program\"" };
        if let Err(e) = cx.doc.append(None, &format!("{name} {placeholder}")) {
            cx.toasts.error(format!("{e:#}"));
        }
    }
    let res = match (edit, remove) {
        (Some((r, t)), _) => cx.doc.replace_node(&r, &t),
        (_, Some(r)) => cx.doc.remove(&r),
        _ => Ok(()),
    };
    if let Err(e) = res {
        cx.toasts.error(format!("{e:#}"));
    }
}

fn environment(ui: &mut egui::Ui, cx: &mut Ctx) {
    let env = cx.doc.find(&[Seg::Name("environment".into())]);
    let vars: Vec<(NodeRef, String, Option<String>)> = env
        .as_ref()
        .map(|e| {
            cx.doc
                .children(e)
                .into_iter()
                .filter_map(|r| {
                    let n = cx.doc.node(&r)?;
                    let v = doc::node_args(n).first().and_then(|v| v.as_str().map(str::to_owned));
                    Some((r.clone(), n.name().value().to_owned(), v))
                })
                .collect()
        })
        .unwrap_or_default();

    ui.weak("A null value unsets the variable for programs niri starts.");
    let mut edit: Option<(NodeRef, String)> = None;
    let mut remove = None;
    egui::Grid::new("env").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
        for (i, (r, name, value)) in vars.iter().enumerate() {
            let mut n = name.clone();
            let mut v = value.clone().unwrap_or_default();
            let mut unset = value.is_none();
            let a = ui.add(egui::TextEdit::singleline(&mut n).id(ui.id().with(("env-n", i))).font(egui::TextStyle::Monospace).desired_width(220.0));
            let b = ui.add_enabled(!unset, egui::TextEdit::singleline(&mut v).id(ui.id().with(("env-v", i))).font(egui::TextStyle::Monospace).desired_width(300.0));
            let c = ui.checkbox(&mut unset, "unset");
            if ui.small_button("🗑").clicked() {
                remove = Some(r.clone());
            }
            ui.end_row();
            if (a.changed() || b.changed() || c.changed()) && !n.trim().is_empty() {
                let val = if unset { Val::Null } else { Val::Str(v) };
                edit = Some((r.clone(), format!("{} {}", doc::value::ident(n.trim()), val.to_kdl())));
            }
        }
    });
    if vars.is_empty() {
        ui.label(RichText::new("No variables set.").weak());
    }
    if ui.button("➕ Add variable").clicked() {
        let res = cx.doc.ensure(&[Seg::Name("environment".into())]).and_then(|e| cx.doc.append(Some(&e), "NEW_VARIABLE \"value\""));
        if let Err(e) = res {
            cx.toasts.error(format!("{e:#}"));
        }
    }
    let res = match (edit, remove) {
        (Some((r, t)), _) => cx.doc.replace_node(&r, &t),
        (_, Some(r)) => cx.doc.remove(&r),
        _ => Ok(()),
    };
    if let Err(e) = res {
        cx.toasts.error(format!("{e:#}"));
    }
}
