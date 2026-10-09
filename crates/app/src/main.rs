fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_app_id("niri-studio"),
        ..Default::default()
    };
    eframe::run_ui_native("niri studio", options, |ui, _| {
        egui::CentralPanel::default_margins().show(ui, |ui| ui.label("hello"));
    })
}
