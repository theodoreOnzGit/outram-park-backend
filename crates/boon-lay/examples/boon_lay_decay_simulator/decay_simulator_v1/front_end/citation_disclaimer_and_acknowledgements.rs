use egui::Ui;

use crate::decay_simulator_v1::DecaySimApp;

impl DecaySimApp {

        pub fn citation_disclaimer_and_acknowledgements(&mut self, ui: &mut Ui){

        ui.heading("DISCLAIMER");

        ui.label(" ");

        ui.label("This is an educational simulator under testing and development");

        ui.label(" ");
        ui.label(" ");

        ui.heading("COPYRIGHT");

        ui.label(" ");

        ui.label("Theodore Kay Chen Ong, SiCong Xiao, SNRSI, and Per F. Peterson");

        ui.label(" ");
        ui.label(" ");
        ui.heading("CREDITS");

        ui.label(" ");

        ui.label("Note: Some code was vibe coded with help from ChatGPT5, esp boilerplate code");

        ui.label(" ");
        ui.label(" ");

        ui.heading("Citations appreciated:");
        ui.label(" ");

        ui.label("TBC");

        ui.label(" ");

    }

}
