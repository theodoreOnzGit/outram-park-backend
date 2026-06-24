use boon_lay::Nuclide;
use uom::{ConstZero, si::{f64::{Ratio, Time}, ratio::ratio, time::second}};

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
    // these are plotting settings
    should_change_nuclide_to_graph_plot:bool,
    nuclide_fraction_vector: Vec<(Nuclide, f64)>,
    nuclides_to_plot: Vec<Nuclide>,
    nuclide_fractions_over_time: Vec<(Time, Vec<f64>)>,

    // graph settings
    pub graph_data_record_interval_seconds: f64,
    pub csv_display_interval_seconds: f64,
    pub plot_width_pixels: f64,
}

impl Default for SimulatorState {

    fn default() -> Self {

        let is_running = true;
        let restart_button_pressed = false;
        let change_nuclide_button_pressed = false;
        let user_selected_timestep = Time::new::<second>(900000.0);
        let elapsed_time = Time::ZERO;
        let simulated_time = Time::ZERO;

        // these are for plotting
        let should_change_nuclide_to_graph_plot = true;
        let nuclide_fraction_remaining = Ratio::new::<ratio>(1.0);
        let nuclide_fraction_vector: Vec<(Nuclide, f64)> =
            vec![];

        let nuclides_to_plot: Vec<Nuclide> = vec![];
        // the nuclide fractions over time
        //
        // are of a tuple
        // first, time is recorded,
        //
        // second, the fractions of the nuclides to plot are arranged in a vector
        // (we don't know at compile time which nuclides are in the
        // decay chain)
        //
        // note that these must be in order of the nuclides supplied in
        // nuclides to plot
        //
        let nuclide_fractions_over_time: Vec<(Time, Vec<f64>)> = vec![];

        let graph_data_record_interval_seconds = 0.1;
        let csv_display_interval_seconds = 1.0;
        let plot_width_pixels = 800.0;

        Self {
            is_running,
            restart_button_pressed,
            user_selected_nuclide: Nuclide::Sr90,
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
        }
    }
}

impl SimulatorState {

    pub fn is_paused(&self) -> bool {
        return !self.is_running;
    }


    // timestep settings
    pub fn get_timestep(&self) -> Time {
        return self.user_selected_timestep;
    }
    pub fn set_timestep(&mut self, timestep: Time){
        self.user_selected_timestep = timestep;
    }

    // these are for elapsed time

    pub fn set_elapsed_time(&mut self, elapsed_time: Time){
        self.elapsed_time = elapsed_time;
    }

    pub fn get_elapsed_time(&self) -> Time {
        self.elapsed_time
    }


    // these are for simulated time

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


    // for getting and setting nuclide
    pub fn set_user_selected_nuclide(&mut self, user_selected_nuclide: Nuclide){
        self.user_selected_nuclide = user_selected_nuclide;
    }

    pub fn get_user_selected_nuclide(&self) -> Nuclide {
        self.user_selected_nuclide
    }


    /// for restart button
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

    // nuclide fraction
    pub fn set_nuclide_fraction(&mut self, fraction: f64){
        self.nuclide_fraction_remaining = Ratio::new::<ratio>(fraction);
    }

    pub fn get_nuclide_fraction(&self) -> Ratio{
        self.nuclide_fraction_remaining
    }

    // nuclide fraction vector
    pub fn get_nuclide_fraction_vector(&self) -> Vec<(Nuclide, f64)>{
        self.nuclide_fraction_vector.clone()
    }

    pub fn set_nuclide_fraction_vector(&mut self, nuclide_fraction_vector: Vec<(Nuclide, f64)>){
        self.nuclide_fraction_vector = nuclide_fraction_vector;
    }

    // the change nuclide to plot

    pub fn turn_on_change_nuclide_to_plot_button(&mut self){
        self.should_change_nuclide_to_graph_plot = true;
    }
    pub fn turn_off_change_nuclide_to_plot_button(&mut self){
        self.should_change_nuclide_to_graph_plot = false;
    }
    pub fn is_change_nuclide_to_plot_button_pressed(&self) -> bool{
        return self.should_change_nuclide_to_graph_plot;
    }

    // nuclides to plot a nuclide fraction over time
    pub fn get_nuclides_to_plot(&self) -> Vec<Nuclide> {
        self.nuclides_to_plot.clone()
    }
    pub fn get_nuclides_fractions_over_time(&self) -> Vec<(Time, Vec<f64>)> {
        self.nuclide_fractions_over_time.clone()
    }
}

pub mod graphing;
