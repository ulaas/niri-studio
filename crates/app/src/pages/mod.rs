mod animations;
mod appearance;
mod binds;
mod displays;
mod forms;
mod history;
mod layout;
pub mod raw;
pub mod rules;
mod startup;

use schema::Page;

use crate::app::Ctx;

#[derive(Default)]
pub struct State {
    pub raw: raw::State,
    history: history::State,
    displays: displays::State,
    binds: binds::State,
    rules: rules::State,
    animations: animations::State,
}

pub fn show(page: Page, ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State) {
    // Visual pages edit through kdl-rs spans; with a syntax error there are none.
    let structured_ok = cx.doc.files.iter().all(|f| f.kdl.is_ok());
    match page {
        Page::Raw => return raw::show(ui, cx, &mut st.raw),
        Page::History => return history::show(ui, cx, &mut st.history),
        _ if !structured_ok => return,
        _ => {}
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(ui.available_width().min(1100.0));
        match page {
            Page::Input | Page::Gestures | Page::Misc => forms::show(ui, cx, page),
            Page::Layout => layout::show(ui, cx),
            Page::Appearance => appearance::show(ui, cx),
            Page::Animations => animations::show(ui, cx, &mut st.animations),
            Page::Displays => displays::show(ui, cx, &mut st.displays),
            Page::Keybinds => binds::show(ui, cx, &mut st.binds),
            Page::Rules => rules::show(ui, cx, &mut st.rules),
            Page::Startup => startup::show(ui, cx),
            Page::Raw | Page::History => unreachable!(),
        }
    });
}
