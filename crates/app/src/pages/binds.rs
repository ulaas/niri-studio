use crate::app::Ctx;

#[derive(Default)]
pub struct State;

pub fn show(ui: &mut egui::Ui, _cx: &mut Ctx, _st: &mut State) {
    ui.label("todo");
}
