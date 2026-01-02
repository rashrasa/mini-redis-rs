#[derive(PartialEq)]
enum Operations {
    Read,
    Insert,
    Delete,
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
        ..Default::default()
    };
    let mut operation = Operations::Read;
    let mut key = String::new();
    let mut value = String::new();
    eframe::run_simple_native("Mini Redis Client", options, move |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.style_mut().spacing.item_spacing = (8.0, 8.0).into();
            ui.heading(egui::RichText::from("Mini Redis Client"));

            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing = (12.0, 12.0).into();
                ui.radio_value(&mut operation, Operations::Read, "Read");
                ui.radio_value(&mut operation, Operations::Insert, "Insert");
                ui.radio_value(&mut operation, Operations::Delete, "Delete");
            });
            ui.horizontal(|ui| {
                ui.label("Key: ");
                ui.text_edit_singleline(&mut key);
            });
            ui.vertical(|ui| {
                ui.label("Value");
                ui.text_edit_multiline(&mut value);
            });

            if ui.button("Send").clicked() {
                // Send request
            };
        });
    })
}
