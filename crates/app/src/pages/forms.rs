//! Pages generated entirely from the schema.

use schema::Page;

use crate::app::Ctx;
use crate::widgets;

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx, page: Page) {
    let subtitle = match page {
        Page::Input => "Keyboard layouts, pointer devices and focus behaviour.",
        Page::Gestures => "Hot corners, drag-and-drop edge scrolling and the recent windows switcher.",
        Page::Misc => "Overlays, notifications and the clipboard.",
        _ => "",
    };
    widgets::heading(ui, page.title(), subtitle);
    if page == Page::Input {
        keyboard_status(ui, cx);
    }
    sections(ui, cx, page, &[]);
}

/// Renders the schema sections of `page`, skipping those named in `skip`.
pub fn sections(ui: &mut egui::Ui, cx: &mut Ctx, page: Page, skip: &[&str]) {
    for (title, opts) in schema::sections(page) {
        if skip.contains(&title) {
            continue;
        }
        widgets::card(ui, title, |ui| {
            for o in opts {
                widgets::option(ui, cx, o);
            }
        });
    }
}

/// Renders only the named sections of `page`.
pub fn only(ui: &mut egui::Ui, cx: &mut Ctx, page: Page, names: &[&str]) {
    for (title, opts) in schema::sections(page) {
        if names.contains(&title) {
            widgets::card(ui, title, |ui| {
                for o in opts {
                    widgets::option(ui, cx, o);
                }
            });
        }
    }
}

fn keyboard_status(ui: &mut egui::Ui, cx: &mut Ctx) {
    let kb = cx.live.state().keyboard.clone();
    if let Some(kb) = kb {
        ui.horizontal_wrapped(|ui| {
            ui.weak("Active layouts:");
            for (i, name) in kb.names.iter().enumerate() {
                let text = egui::RichText::new(name);
                if i == kb.current_idx as usize {
                    ui.label(text.strong()).on_hover_text("Current layout");
                } else {
                    ui.label(text);
                }
            }
        });
    }
}
