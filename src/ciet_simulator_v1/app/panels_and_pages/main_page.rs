use egui::{Color32, TextStyle, Ui};
use egui_extras::{Size, StripBuilder};

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

        let dark_mode = ui.visuals().dark_mode;
        let faded_color = ui.visuals().window_fill();
        let faded_color = |color: Color32| -> Color32 {
            use egui::Rgba;
            let t = if dark_mode { 0.95 } else { 0.8 };
            egui::lerp(Rgba::from(color)..=Rgba::from(faded_color), t).into()
        };

        
        let body_text_size = TextStyle::Body.resolve(ui.style()).size;
        StripBuilder::new(ui)
            .size(Size::exact(50.0))
            .size(Size::remainder())
            .size(Size::relative(0.5).at_least(60.0))
            .size(Size::exact(body_text_size))
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    ui.painter().rect_filled(
                        ui.available_rect_before_wrap(),
                        0.0,
                        faded_color(Color32::BLUE),
                    );
                    ui.label("width: 100%\nheight: 50px");
                });
                strip.strip(|builder| {
                    builder.sizes(Size::remainder(), 2).horizontal(|mut strip| {
                        strip.cell(|ui| {
                            ui.painter().rect_filled(
                                ui.available_rect_before_wrap(),
                                0.0,
                                faded_color(Color32::RED),
                            );
                            ui.label("width: 50%\nheight: remaining");
                        });
                        strip.strip(|builder| {
                            builder.sizes(Size::remainder(), 3).vertical(|mut strip| {
                                strip.empty();
                                strip.cell(|ui| {
                                    ui.painter().rect_filled(
                                        ui.available_rect_before_wrap(),
                                        0.0,
                                        faded_color(Color32::YELLOW),
                                    );
                                    ui.label("width: 50%\nheight: 1/3 of the red region");
                                });
                                strip.empty();
                            });
                        });
                    });
                });
                strip.strip(|builder| {
                    builder
                        .size(Size::remainder())
                        .size(Size::exact(120.0))
                        .size(Size::remainder())
                        .size(Size::exact(70.0))
                        .horizontal(|mut strip| {
                            strip.empty();
                            strip.strip(|builder| {
                                builder
                                    .size(Size::remainder())
                                    .size(Size::exact(60.0))
                                    .size(Size::remainder())
                                    .vertical(|mut strip| {
                                        strip.empty();
                                        strip.cell(|ui| {
                                            ui.painter().rect_filled(
                                                ui.available_rect_before_wrap(),
                                                0.0,
                                                faded_color(Color32::GOLD),
                                            );
                                            ui.label("width: 120px\nheight: 60px");
                                        });
                                    });
                            });
                            strip.empty();
                            strip.cell(|ui| {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.0,
                                    faded_color(Color32::GREEN),
                                );
                                ui.label("width: 70px\n\nheight: 50%, but at least 60px.");
                            });
                        });
                });
                strip.cell(|ui| {
                    ui.add(egui::github_link_file!(
                            "https://github.com/theodoreOnzGit/outram-park-backend/blob/develop/",
                            "outram-park-backend Github Repo (develop)"
                    ));
                });
            });
    }
}
