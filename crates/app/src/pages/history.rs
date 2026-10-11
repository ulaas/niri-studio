//! Review unsaved changes and restore earlier saved versions.

use doc::Backup;
use egui::RichText;
use similar::{ChangeTag, TextDiff};

use crate::theme::icons;
use crate::app::Ctx;
use crate::widgets;

#[derive(Default)]
pub struct State {
    backups: Option<Vec<Backup>>,
    /// `None` = the unsaved changes; `Some(i)` = backup i.
    selected: Option<usize>,
}

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State) {
    widgets::heading(
        ui,
        "History",
        "Every save keeps a backup of the previous file. Review what changed and restore any version.",
    );
    let files: Vec<_> = cx.doc.files.iter().map(|f| f.path.clone()).collect();
    if st.backups.is_none() {
        st.backups = Some(doc::list_backups().into_iter().filter(|b| files.contains(&b.of)).collect());
    }

    egui::Panel::left("history-list").default_size(280.0).show(ui, |ui| {
        if ui.button(format!("{} Refresh", icons::ARROW_CLOCKWISE)).clicked() {
            st.backups = None;
        }
        ui.add_space(4.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            let dirty = cx.doc.is_dirty();
            let label = if dirty { "• Unsaved changes" } else { "No unsaved changes" };
            if ui.selectable_label(st.selected.is_none(), label).clicked() {
                st.selected = None;
            }
            ui.separator();
            for (i, b) in st.backups.iter().flatten().enumerate() {
                let name = b.of.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let text = format!("{}  {name}", doc::format_time(b.time));
                if ui.selectable_label(st.selected == Some(i), text).on_hover_text("The file as it was before this save").clicked() {
                    st.selected = Some(i);
                }
            }
            if st.backups.as_ref().is_some_and(Vec::is_empty) {
                ui.weak("No backups yet. One is made each time you save.");
            }
        });
    });

    egui::CentralPanel::default_margins().show(ui, |ui| match st.selected {
        None => {
            for f in &cx.doc.files {
                if f.is_dirty() {
                    ui.label(RichText::new(f.path.display().to_string()).strong());
                    diff(ui, f.saved_text(), &f.text);
                }
            }
            if !cx.doc.is_dirty() {
                ui.weak("Nothing to review: the editor matches what's on disk.");
            }
        }
        Some(i) => {
            let Some(b) = st.backups.as_ref().and_then(|v| v.get(i)).cloned() else { return };
            let Some(fi) = cx.doc.files.iter().position(|f| f.path == b.of) else { return };
            let old = match b.read() {
                Ok(t) => t,
                Err(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, format!("{e:#}"));
                    return;
                }
            };
            ui.horizontal(|ui| {
                ui.label(format!("Changes from this backup to the current editor text ({})", doc::format_time(b.time)));
                if ui.button(format!("{} Restore this version", icons::ARROW_COUNTER_CLOCKWISE)).on_hover_text("Loads it into the editor; save to apply. Undoable.").clicked() {
                    cx.doc.set_text(fi, old.clone());
                    cx.toasts.info("Backup loaded into the editor. Save to apply it.");
                }
            });
            diff(ui, &old, &cx.doc.files[fi].text);
        }
    });
}

fn diff(ui: &mut egui::Ui, old: &str, new: &str) {
    let d = TextDiff::from_lines(old, new);
    let font = egui::FontId::monospace(13.0);
    let t = crate::theme::pal(ui.ctx());
    let (add, del) = (t.ok, t.error);
    egui::ScrollArea::both().id_salt(old.len() ^ new.len()).auto_shrink([false, true]).show(ui, |ui| {
        let mut any = false;
        for group in d.grouped_ops(3) {
            any = true;
            ui.separator();
            for op in group {
                for change in d.iter_changes(&op) {
                    let (sign, color) = match change.tag() {
                        ChangeTag::Delete => ("-", del),
                        ChangeTag::Insert => ("+", add),
                        ChangeTag::Equal => (" ", ui.visuals().weak_text_color()),
                    };
                    let line = change.value().trim_end_matches('\n');
                    ui.label(RichText::new(format!("{sign} {line}")).font(font.clone()).color(color));
                }
            }
        }
        if !any {
            ui.weak("Identical.");
        }
    });
}
