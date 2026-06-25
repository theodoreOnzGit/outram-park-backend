
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;
use boon_lay::Nuclide;
use boon_lay::prelude::SingleNuclideSimulatorMC;
use eframe::egui;
use eframe::egui::Widget;
use eframe::egui::Pos2;
use egui::Color32;
use egui::Rect;
use egui::Ui;
use uom::si::f64::*;
use uom::si::ratio::ratio;
use uom::si::length::millimeter;

use crate::triso_simulator_v1::TRISOSimApp;


#[derive(Clone,Copy, Debug)]
pub struct TrisoParticleUi {
    triso_cell: TrisoCell,
}

impl Default for TrisoParticleUi {
    fn default() -> Self {

        let triso_cell = TrisoCell::new_crp6_geometry();

        Self {
            triso_cell,
        }
    }
}

impl Widget for TrisoParticleUi {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let desired = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::hover());

        let min_dim = rect.width().min(rect.height());
        let outer_radius: f32 = 0.5 * min_dim;
        let center: Pos2 = rect.center();

        let painter = ui.painter_at(rect);

        let inner_kernel_radius: Length = 0.5 * self.get_diameter_after_fuel();
        let buffer_radius: Length = 0.5 * self.get_diameter_after_buffer();
        let ipyc_radius: Length = 0.5 * self.get_diameter_after_ipyc();
        let sic_radius: Length = 0.5 * self.get_diameter_after_sic();
        let opyc_radius: Length = 0.5 * self.get_diameter_after_opyc();

        let scale_length_per_pixel: Length = opyc_radius/(outer_radius as f64);
        let inner_kernel_radius_pixels: f32
            = (inner_kernel_radius/scale_length_per_pixel).get::<ratio>() as f32;
        let buffer_radius_pixels: f32
            = (buffer_radius/scale_length_per_pixel).get::<ratio>() as f32;
        let ipyc_radius_pixels: f32
            = (ipyc_radius/scale_length_per_pixel).get::<ratio>() as f32;
        let sic_radius_pixels: f32
            = (sic_radius/scale_length_per_pixel).get::<ratio>() as f32;
        let opyc_radius_pixels: f32
            = (opyc_radius/scale_length_per_pixel).get::<ratio>() as f32;

        let fuel_kernel_nuclide = Nuclide::U235;
        let buffer_nuclide = Nuclide::C12;
        let ipyc_nuclide = Nuclide::C12;
        let sic_nuclide = Nuclide::Si28;
        let opyc_nuclide = Nuclide::C12;

        let fuel_kernel_colour = TRISOSimApp::element_color(fuel_kernel_nuclide);
        let _buffer_colour = TRISOSimApp::element_color(buffer_nuclide);
        let buffer_colour = Color32::from_rgb(100, 200, 100);
        let ipyc_colour = TRISOSimApp::element_color(ipyc_nuclide);
        let sic_colour = TRISOSimApp::element_color(sic_nuclide);
        let opyc_colour = TRISOSimApp::element_color(opyc_nuclide);

        painter.circle_filled(center, opyc_radius_pixels, opyc_colour);
        painter.circle_filled(center, sic_radius_pixels, sic_colour);
        painter.circle_filled(center, ipyc_radius_pixels, ipyc_colour);
        painter.circle_filled(center, buffer_radius_pixels, buffer_colour);
        painter.circle_filled(center, inner_kernel_radius_pixels, fuel_kernel_colour);

        response
    }
}


impl TrisoParticleUi {
    pub fn get_triso_cell(&self) -> &TrisoCell {
        &self.triso_cell
    }

    pub fn get_triso_cell_mut(&mut self) -> &mut TrisoCell {
        &mut self.triso_cell
    }

    pub fn set_triso_cell(&mut self, cell: TrisoCell) {
        self.triso_cell = cell;
    }

    pub fn put_self_with_size_and_centre(
        &mut self,
        ui: &mut Ui,
        centre_x_pixels: f32,
        centre_y_pixels: f32,
        x_width_pixels: f32,
        y_width_pixels: f32){

        let top_left_x: f32 = centre_x_pixels - 0.5 * x_width_pixels;
        let top_left_y: f32 = centre_y_pixels - 0.5 * y_width_pixels;
        let bottom_right_x: f32 = centre_x_pixels + 0.5 * x_width_pixels;
        let bottom_right_y: f32 = centre_y_pixels + 0.5 * y_width_pixels;

        let rect: Rect = Rect {
            min: Pos2 { x: top_left_x, y: top_left_y },
            max: Pos2 { x: bottom_right_x, y: bottom_right_y },
        };

        ui.put(rect, *self);

    }

    pub fn put_particle_vector_with_size_and_centre(
        &mut self,
        ui: &mut Ui,
        triso_centre_x_pixels: f32,
        triso_centre_y_pixels: f32,
        triso_width_pixels: f32,
        sampled_particle_sims_for_plotting: Vec<SingleNuclideSimulatorMC>,
    ){

        let triso_diameter: Length = self.get_diameter_after_opyc();

        let radionuclide_x_width_pixels = triso_width_pixels * 0.003;

        let scale_length_per_pixel: Length = triso_diameter / triso_width_pixels as f64;

        let painter = ui.painter();
        let z_max = Length::new::<millimeter>(0.1);
        let z_min = Length::new::<millimeter>(-0.1);

        for radionuclide_sim in sampled_particle_sims_for_plotting {

            let radionuclide_position = radionuclide_sim.position;

            let (_x, _y, z) = radionuclide_position;

            if z > z_max {
                continue;
            }
            if z < z_min {
                continue;
            }

            let (x_pixel,y_pixel, _z_pixel) =
                Self::convert_coordinate_to_pixel(
                    radionuclide_position,
                    scale_length_per_pixel
                );

            let radionuclide_center_x_pixels = triso_centre_x_pixels + x_pixel;
            let radionuclide_center_y_pixels = triso_centre_y_pixels + y_pixel;

            let nuclide = radionuclide_sim.get_current_nuclide();
            let colour = TRISOSimApp::element_color(nuclide);

            let center = Pos2::new(
                radionuclide_center_x_pixels,
                radionuclide_center_y_pixels
            );
            let fission_prod_radius = radionuclide_x_width_pixels;
            painter.circle_filled(center, fission_prod_radius, colour);
        }

    }

    pub fn get_diameter_after_buffer(&self) -> Length {
        self.triso_cell.get_buffer_radius() * 2.0
    }
    pub fn get_diameter_after_ipyc(&self) -> Length {
        self.triso_cell.get_ipyc_radius() * 2.0
    }
    pub fn get_diameter_after_sic(&self) -> Length {
        self.triso_cell.get_sic_radius() * 2.0
    }
    pub fn get_diameter_after_opyc(&self) -> Length {
        self.triso_cell.get_opyc_radius() * 2.0
    }
    pub fn get_diameter_after_fuel(&self) -> Length {
        self.triso_cell.get_fuel_radius() * 2.0
    }

    pub fn convert_coordinate_to_pixel(
        coordinate: (Length, Length, Length),
        scale_length_per_pixel: Length) -> (f32, f32, f32) {

        let (x,y,z) = coordinate;
        let x_pixel: f32 = (x / scale_length_per_pixel).get::<ratio>() as f32;
        let y_pixel: f32 = (y / scale_length_per_pixel).get::<ratio>() as f32;
        let z_pixel: f32 = (z / scale_length_per_pixel).get::<ratio>() as f32;

        (x_pixel, y_pixel, z_pixel)
    }

}

impl AsRef<TrisoCell> for TrisoParticleUi {
    fn as_ref(&self) -> &TrisoCell {
        &self.triso_cell
    }
}

impl AsMut<TrisoCell> for TrisoParticleUi {
    fn as_mut(&mut self) -> &mut TrisoCell {
        &mut self.triso_cell
    }
}
