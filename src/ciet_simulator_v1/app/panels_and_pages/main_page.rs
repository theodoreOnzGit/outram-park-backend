use egui::Ui;

use crate::ciet_simulator_v1::CIETApp;

impl CIETApp {

    pub fn ciet_sim_main_page(&mut self, ui: &mut Ui){

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Main Page");
        });

        ui.horizontal(|ui| {
            ui.label("Write something: ");
            ui.text_edit_singleline(&mut self.label);
        });

        ui.add(egui::Slider::new(&mut self.value, 0.0..=10.0).text("Roentgen (just for the memes)"));
        if ui.button("Increment").clicked() {
            self.value += 1.0;
        }

    }
}
