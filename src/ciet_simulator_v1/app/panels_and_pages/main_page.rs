use egui::{include_image, Color32, Image, TextStyle, Ui};
use egui_extras::{Size, StripBuilder};

use crate::ciet_simulator_v1::CIETApp;

use super::ciet_data::CIETState;

impl CIETApp {

    pub fn ciet_sim_main_page(&mut self, ui: &mut Ui){

        // obtain a lock first to display the information 

        self.insert_read_and_update_widgets(ui);

        self.insert_pictures(ui);

    }

    /// inserts sliders and other widgets for ciet 
    fn insert_read_and_update_widgets(&mut self, ui: &mut Ui,){

        // obtain a lock for ciet state first, clone it
        // and drop the lock
        let mut ciet_state_local: CIETState 
            = self.ciet_state.lock().unwrap().clone();

        // manually set coordinates
        let (tchx_x, tchx_y): (f32, f32) = (150.0, 260.0);
        let (tchx_x_width, tchx_y_width): (f32, f32) = (150.0, 150.0);
        let dhx_x = tchx_x + 250.0;
        let dhx_y = tchx_y + 250.0;
        let dhx_x_width = tchx_x_width;
        let dhx_y_width = tchx_y_width;
        let heater_x = dhx_x + 350.0;
        let heater_y = dhx_y + 50.0;
        let heater_x_width = dhx_x_width;
        let heater_y_width = dhx_y_width;
        let ctah_x = heater_x + 750.0;
        let ctah_y = tchx_y;
        let ctah_x_width = dhx_x_width;
        let ctah_y_width = dhx_y_width;
        let ctah_pump_x = ctah_x - 50.0;
        let ctah_pump_y = heater_y + 270.0;
        let ctah_pump_x_width = dhx_x_width;
        let ctah_pump_y_width = dhx_y_width;

        // for user to set heater power
        let heater_set_pt_slider_kw = egui::Slider::new(
            &mut ciet_state_local.heater_power_kilowatts, 0.0..=10.0)
            .vertical()
            .text("Heater Power (kW)");

        let heater_slider_x = heater_x + 0.7 * heater_x_width;
        let heater_slider_y = heater_y + 10.0;
        let heater_slider_x_width = 30.0;
        let heater_slider_y_width = heater_y_width;

        self.put_widget_with_size_and_centre(
            ui, 
            heater_set_pt_slider_kw, 
            heater_slider_x, 
            heater_slider_y, 
            heater_slider_x_width, 
            heater_slider_y_width);

        // heater outlet temp and inlet temp
        let heater_out_temp_degc: f64 = 
            ciet_state_local.get_heater_outlet_temp_degc();

        let heater_display_text_outlet: String = 
            "Outlet BT-12 (degC): ".to_string() + &heater_out_temp_degc.to_string();

        let heater_outlet_label = egui::Label::new(&heater_display_text_outlet);

        self.put_widget_with_size_and_centre(
            ui, 
            heater_outlet_label, 
            heater_slider_x + 45.0, 
            heater_slider_y - 90.0, 
            heater_slider_x_width + 120.0, 
            heater_slider_y_width * 0.2);

        let heater_in_temp_degc: f64 = 
            ciet_state_local.get_heater_inlet_temp_degc();

        let heater_display_text_inlet: String = 
            "Inlet BT-11 (degC): ".to_string() + &heater_in_temp_degc.to_string();

        let heater_inlet_label = egui::Label::new(
            &heater_display_text_inlet);

        self.put_widget_with_size_and_centre(
            ui, 
            heater_inlet_label, 
            heater_slider_x + 45.0, 
            heater_slider_y + 90.0, 
            heater_slider_x_width + 120.0, 
            heater_slider_y_width*0.2);


        // for user to set CTAH and TCHX cooler set points
        let tchx_slider_outlet_set_pt_degc = egui::Slider::new(
            &mut ciet_state_local.bt_66_tchx_outlet_set_pt_deg_c, 25.0..=110.0)
            .vertical()
            .text("TCHX Outlet Set Pt (degC)");
        let tchx_slider_x = tchx_x + 0.7 * tchx_x_width;
        let tchx_slider_y = tchx_y + 10.0;
        let tchx_slider_x_width = 30.0;
        let tchx_slider_y_width = tchx_y_width;

        self.put_widget_with_size_and_centre(
            ui, 
            tchx_slider_outlet_set_pt_degc, 
            tchx_slider_x, 
            tchx_slider_y, 
            tchx_slider_x_width, 
            tchx_slider_y_width);

        let tchx_top_temp = ciet_state_local.get_tchx_inlet_temp_degc();
        let tchx_bottom_temp = ciet_state_local.get_tchx_outlet_temp_degc();

        let tchx_top_label = egui::Label::new(
            "Inlet BT-65 (degC): ".to_string() 
            + &tchx_top_temp.to_string()
            );

        let tchx_bottom_label = egui::Label::new(
            "Outlet BT-66 (degC): ".to_string() 
            + &tchx_bottom_temp.to_string()
            );

        self.put_widget_with_size_and_centre(
            ui, 
            tchx_top_label, 
            tchx_slider_x + 55.0, 
            tchx_slider_y - 90.0, 
            tchx_slider_x_width + 120.0, 
            tchx_slider_y_width * 0.2);

        self.put_widget_with_size_and_centre(
            ui, 
            tchx_bottom_label, 
            tchx_slider_x + 55.0, 
            tchx_slider_y + 90.0, 
            tchx_slider_x_width + 120.0, 
            tchx_slider_y_width * 0.2);

        let ctah_slider_outlet_set_pt_degc = egui::Slider::new(
            &mut ciet_state_local.bt_41_ctah_outlet_set_pt_deg_c, 25.0..=110.0)
            .vertical()
            .text("CTAH Outlet Set Pt (degC)");

        let ctah_slider_x = ctah_x + 0.7 * ctah_x_width;
        let ctah_slider_y = ctah_y + 10.0;
        let ctah_slider_x_width = 30.0;
        let ctah_slider_y_width = ctah_y_width;

        self.put_widget_with_size_and_centre(
            ui, 
            ctah_slider_outlet_set_pt_degc, 
            ctah_slider_x, 
            ctah_slider_y, 
            ctah_slider_x_width, 
            ctah_slider_y_width);

        let ctah_top_temp = ciet_state_local.get_ctah_inlet_temp_degc();
        let ctah_bottom_temp = ciet_state_local.get_ctah_outlet_temp_degc();

        let ctah_top_label = egui::Label::new(
            "Inlet BT-43 (degC): ".to_string() 
            + &ctah_top_temp.to_string()
            );

        let ctah_bottom_label = egui::Label::new(
            "Outlet BT-41 (degC): ".to_string() 
            + &ctah_bottom_temp.to_string()
            );

        self.put_widget_with_size_and_centre(
            ui, 
            ctah_top_label, 
            ctah_slider_x + 55.0, 
            ctah_slider_y - 90.0, 
            ctah_slider_x_width + 120.0, 
            ctah_slider_y_width * 0.2);

        self.put_widget_with_size_and_centre(
            ui, 
            ctah_bottom_label, 
            ctah_slider_x + 55.0, 
            ctah_slider_y + 90.0, 
            ctah_slider_x_width + 120.0, 
            ctah_slider_y_width * 0.2);

        // obtain a lock for ciet state, set it 
        self.ciet_state.lock().unwrap().overwrite_state(ciet_state_local);
    }

    /// inserts static image widgets for ciet
    fn insert_pictures(&mut self, ui: &mut Ui,){

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
