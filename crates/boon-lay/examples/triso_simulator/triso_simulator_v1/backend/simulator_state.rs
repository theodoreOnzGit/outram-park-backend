use boon_lay::{Nuclide, lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell};
use uom::si::f64::{*};
use uom::ConstZero;
use uom::si::time::second;
use uom::si::ratio::ratio;

use crate::triso_simulator_v1::front_end::triso_particle::TrisoParticleUi;

#[derive(Debug, Clone, PartialEq)]
pub struct SimulatorState {
    is_running: bool,
    restart_button_pressed: bool,
    change_nuclide_button_pressed: bool,
    user_selected_nuclide: Nuclide,
    user_selected_timestep: Time,
    elapsed_time: Time,
    simulated_time: Time,
    nuclide_fraction_remaining: Ratio,
    should_change_nuclide_to_graph_plot: bool,
    nuclide_fraction_vector: Vec<(Nuclide, f64)>,
    nuclides_to_plot: Vec<Nuclide>,
    nuclide_fractions_over_time: Vec<(Time, Vec<f64>)>,

    release_fraction: Ratio,
    pub release_fractions_over_time: Vec<(Time, f64)>,

    pub graph_data_record_interval_seconds: f64,
    pub csv_display_interval_seconds: f64,
    pub plot_width_pixels: f64,

    pub triso_cell: TrisoCell,

    pub user_selected_temperature: ThermodynamicTemperature,
}

impl Default for SimulatorState {

    fn default() -> Self {

        let is_running = true;
        let restart_button_pressed = false;
        let change_nuclide_button_pressed = false;
        let user_selected_timestep = Time::new::<second>(0.01);
        let elapsed_time = Time::ZERO;
        let simulated_time = Time::ZERO;
        let release_fraction = Ratio::ZERO;

        let should_change_nuclide_to_graph_plot = true;
        let nuclide_fraction_remaining = Ratio::new::<ratio>(1.0);
        let nuclide_fraction_vector: Vec<(Nuclide, f64)> = vec![];

        let nuclides_to_plot: Vec<Nuclide> = vec![];
        let nuclide_fractions_over_time: Vec<(Time, Vec<f64>)> = vec![];
        let release_fractions_over_time: Vec<(Time, f64)> = vec![];

        let graph_data_record_interval_seconds = 0.1;
        let csv_display_interval_seconds = 1.0;
        let plot_width_pixels = 800.0;

        let new_triso_particle_ui = TrisoParticleUi::default();
        let fuel_radius = new_triso_particle_ui.get_diameter_after_fuel() * 0.5;
        let buffer_radius = new_triso_particle_ui.get_diameter_after_buffer() * 0.5;
        let ipyc_radius = new_triso_particle_ui.get_diameter_after_ipyc() * 0.5;
        let sic_radius = new_triso_particle_ui.get_diameter_after_sic() * 0.5;
        let opyc_radius = new_triso_particle_ui.get_diameter_after_opyc() * 0.5;

        let triso_cell = TrisoCell::new(
            fuel_radius,
            buffer_radius,
            ipyc_radius,
            sic_radius,
            opyc_radius);

        let user_selected_temperature = triso_cell.get_uniform_temperature();

        Self {
            is_running,
            restart_button_pressed,
            user_selected_nuclide: Nuclide::Cs137,
            change_nuclide_button_pressed,
            user_selected_timestep,
            elapsed_time,
            simulated_time,
            nuclide_fraction_remaining,
            nuclide_fraction_vector,
            nuclides_to_plot,
            nuclide_fractions_over_time,
            should_change_nuclide_to_graph_plot,
            graph_data_record_interval_seconds,
            csv_display_interval_seconds,
            plot_width_pixels,
            release_fraction,
            triso_cell,
            user_selected_temperature,
            release_fractions_over_time,
        }
    }
}

impl SimulatorState {

    pub fn is_paused(&self) -> bool {
        return !self.is_running;
    }

    pub fn get_timestep(&self) -> Time {
        return self.user_selected_timestep;
    }
    pub fn set_timestep(&mut self, timestep: Time){
        self.user_selected_timestep = timestep;
    }

    pub fn set_elapsed_time(&mut self, elapsed_time: Time){
        self.elapsed_time = elapsed_time;
    }

    pub fn get_elapsed_time(&self) -> Time {
        self.elapsed_time
    }

    pub fn add_to_simulated_time(&mut self, timestep: Time){
        self.simulated_time += timestep;
    }
    pub fn get_simulated_time(&self) -> Time {
        self.simulated_time
    }
    pub fn get_simulated_time_seconds_2dp(&self) -> f64 {
        return (self.get_simulated_time().get::<second>()*100_f64).round()/100.0;
    }
    pub fn reset_simulated_time(&mut self){
        self.simulated_time = Time::ZERO;
    }

    pub fn set_user_selected_nuclide(&mut self, user_selected_nuclide: Nuclide){
        self.user_selected_nuclide = user_selected_nuclide;
    }

    pub fn get_user_selected_nuclide(&self) -> Nuclide {
        self.user_selected_nuclide
    }

    pub fn turn_on_restart_button(&mut self){
        self.restart_button_pressed = true;
    }
    pub fn turn_on_change_nuclide_button(&mut self){
        self.change_nuclide_button_pressed = true;
    }
    pub fn turn_off_restart_button(&mut self){
        self.restart_button_pressed = false;
    }
    pub fn turn_off_change_nuclide_button(&mut self){
        self.change_nuclide_button_pressed = false;
    }
    pub fn is_restart_button_pressed(&self) -> bool{
        return self.restart_button_pressed;
    }
    pub fn is_change_nuclide_button_pressed(&self) -> bool{
        return self.change_nuclide_button_pressed;
    }

    pub fn set_nuclide_fraction(&mut self, fraction: f64){
        self.nuclide_fraction_remaining = Ratio::new::<ratio>(fraction);
    }

    pub fn get_nuclide_fraction(&self) -> Ratio{
        self.nuclide_fraction_remaining
    }

    pub fn get_nuclide_fraction_vector(&self) -> Vec<(Nuclide, f64)>{
        self.nuclide_fraction_vector.clone()
    }

    pub fn set_nuclide_fraction_vector(&mut self, nuclide_fraction_vector: Vec<(Nuclide, f64)>){
        self.nuclide_fraction_vector = nuclide_fraction_vector;
    }

    pub fn turn_on_change_nuclide_to_plot_button(&mut self){
        self.should_change_nuclide_to_graph_plot = true;
    }
    pub fn turn_off_change_nuclide_to_plot_button(&mut self){
        self.should_change_nuclide_to_graph_plot = false;
    }
    pub fn is_change_nuclide_to_plot_button_pressed(&self) -> bool{
        return self.should_change_nuclide_to_graph_plot;
    }

    pub fn get_nuclides_to_plot(&self) -> Vec<Nuclide> {
        self.nuclides_to_plot.clone()
    }
    pub fn get_nuclides_fractions_over_time(&self) -> Vec<(Time, Vec<f64>)> {
        self.nuclide_fractions_over_time.clone()
    }
    pub fn get_release_fraction(&self) -> Ratio {
        self.release_fraction
    }
    pub fn set_release_fraction(&mut self, release_fraction: Ratio){
        self.release_fraction = release_fraction;
    }

    #[inline]
    pub fn get_triso_uniform_temperature(&self) -> ThermodynamicTemperature {
        self.triso_cell.get_uniform_temperature()
    }

    #[inline]
    pub fn set_triso_uniform_temperature(&mut self, temp: ThermodynamicTemperature) {
        self.triso_cell.set_uniform_temperature(temp);
        self.user_selected_temperature = temp;
    }
    pub fn get_user_selected_temperature(&self) -> ThermodynamicTemperature {
        self.user_selected_temperature
    }

    pub fn set_user_selected_temperature(&mut self, temp: ThermodynamicTemperature) {
        self.user_selected_temperature = temp;
    }

    pub fn get_release_fractions_over_time(&self) -> &Vec<(Time, f64)> {
        &self.release_fractions_over_time
    }
}

pub mod graphing;
