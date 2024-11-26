use egui::{include_image, Color32, Image, TextStyle, Ui};
use egui_extras::{Size, StripBuilder};

use crate::ciet_simulator_v1::CIETApp;

impl CIETApp {

    pub fn ciet_sim_main_page(&mut self, ui: &mut Ui){

        let tchx_pic = Image::new(
            include_image!("../../cooler.png")
            ).rounding(5.0);

        let dhx_pic = Image::new(
            include_image!("../../heat-exchanger.png")
            ).rounding(5.0);

        let heater_pic = Image::new(
            include_image!("../../heater.png")
            ).rounding(5.0);

        let ctah_pump_pic = Image::new(
            include_image!("../../pump.png")
            ).rounding(5.0);

        let ctah_pic = Image::new(
            include_image!("../../cooler.png")
            ).rounding(5.0);

        let (tchx_x, tchx_y): (f32, f32) = (150.0, 260.0);
        let (tchx_x_width, tchx_y_width): (f32, f32) = (150.0, 150.0);

        // for tchx
        self.put_widget_with_size_and_centre(
            ui, 
            tchx_pic, 
            tchx_x, 
            tchx_y, 
            tchx_x_width, 
            tchx_y_width);
    }
}
