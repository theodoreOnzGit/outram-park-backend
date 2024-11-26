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

        // for dhx
        let dhx_x = tchx_x + 250.0;
        let dhx_y = tchx_y + 250.0;
        let dhx_x_width = tchx_x_width;
        let dhx_y_width = tchx_y_width;

        self.put_widget_with_size_and_centre(
            ui, 
            dhx_pic, 
            dhx_x, 
            dhx_y, 
            dhx_x_width, 
            dhx_y_width);

        // for heater
        let heater_x = dhx_x + 350.0;
        let heater_y = dhx_y + 50.0;
        let heater_x_width = dhx_x_width;
        let heater_y_width = dhx_y_width;

        self.put_widget_with_size_and_centre(
            ui, 
            heater_pic, 
            heater_x, 
            heater_y, 
            heater_x_width, 
            heater_y_width);

        // for ctah
        let ctah_x = heater_x + 750.0;
        let ctah_y = tchx_y;
        let ctah_x_width = dhx_x_width;
        let ctah_y_width = dhx_y_width;

        self.put_widget_with_size_and_centre(
            ui, 
            ctah_pic, 
            ctah_x, 
            ctah_y, 
            ctah_x_width, 
            ctah_y_width);

        // for ctah_pump
        let ctah_pump_x = ctah_x - 50.0;
        let ctah_pump_y = heater_y + 270.0;
        let ctah_pump_x_width = dhx_x_width;
        let ctah_pump_y_width = dhx_y_width;

        self.put_widget_with_size_and_centre(
            ui, 
            ctah_pump_pic, 
            ctah_pump_x, 
            ctah_pump_y, 
            ctah_pump_x_width, 
            ctah_pump_y_width);

        ui.separator();
    }
}
