use boon_lay::Nuclide;
use egui::Ui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use uom::si::{f64::Time, time::{nanosecond, second}};

use crate::decay_simulator_v1::{backend::simulator_state::SimulatorState, DecaySimApp};

impl DecaySimApp {

    pub fn graph_page(&self, ui: &mut Ui){


        let mut simulator_state_clone =
            self.simulator_state.lock().unwrap().clone();

        let nuclides_to_plot = simulator_state_clone.get_nuclides_to_plot();
        let nuclide_fractions_over_time: Vec<(Time, Vec<f64>)> =
            simulator_state_clone.get_nuclides_fractions_over_time();

        let plot_width_slider = egui::Slider::new(
            &mut simulator_state_clone.plot_width_pixels,
            800.0..=1800.0
        ) .logarithmic(false) .text("Plot Width (Pixels)") .drag_value_speed(0.001);
        ui.add(plot_width_slider);

        self.simulator_state.lock().unwrap().plot_width_pixels =
            simulator_state_clone.plot_width_pixels;

        let mut nuclide_plot = Plot::new("Nuclide Fractions over time").legend(Legend::default());

        // sets the aspect for plot
        nuclide_plot = nuclide_plot.width(simulator_state_clone.plot_width_pixels as f32);
        nuclide_plot = nuclide_plot.view_aspect(16.0/9.0);

        nuclide_plot = nuclide_plot.x_axis_label(
            "time (seconds), current time (seconds): ".to_owned()
        );
        nuclide_plot = nuclide_plot.y_axis_label(
            "nuclide fractions".to_owned());


        // let's make the time and bt11 vector

        ui.heading("Nuclide Fractions over Time");
        nuclide_plot.show(ui, |plot_ui| {
            // outer loop, ie do for every nuclide
            for (nuclide_index,nuclide) in nuclides_to_plot.iter().enumerate() {
                let nuclide_name: String = format!("{:?}",nuclide);

                let mut plot_vector_time_and_nuclide_fraction: Vec<[f64;2]>
                    = vec![];

                let mut current_nuclide_fraction = 0.0;
                // we construct the plot vector for every nuclide
                for (_time_index, (simulation_time, nuclide_fractions_vector)) in
                    nuclide_fractions_over_time.iter().enumerate() {

                        let time_seconds = simulation_time.get::<second>();
                        let nuclide_fraction = nuclide_fractions_vector[nuclide_index];

                        plot_vector_time_and_nuclide_fraction.
                            push([time_seconds,nuclide_fraction]);

                        current_nuclide_fraction = nuclide_fraction;

                    }


                plot_ui.line(Line::new(
                    nuclide_name + " fraction: " + &current_nuclide_fraction.to_string(),
                    PlotPoints::from(plot_vector_time_and_nuclide_fraction.clone()),
                ));

            }

            //
        });



        ui.separator();


    }


    /// a truncated side panel for the graph page
    pub fn graph_page_side_panel(&mut self, ui: &mut Ui){

        ui.heading("Timestep and Time");

        // basically, get the simulator state first
        ui.label(" ");

        // this is the snapshot of the current simulator state
        let current_simulator_state_clone : SimulatorState
            = self.simulator_state.lock().unwrap().clone();

        // this is the simulator state used to plot csv data
        let csv_simulator_state_clone =
            self.csv_simulator_state.clone();


        // display elapsed time
        let elapsed_time = current_simulator_state_clone.get_elapsed_time();
        let mut elapsed_time_string: String = "Elapsed Time (seconds):".to_string();
        elapsed_time_string += &elapsed_time.get::<second>().to_string();

        ui.label(elapsed_time_string);
        // timestep settings

        let mut user_set_timestep_seconds
            = current_simulator_state_clone.get_timestep().get::<second>();

        let timestep_slider_seconds = egui::Slider::new(
            &mut user_set_timestep_seconds,
            0.00001..=1e20
        ) .logarithmic(true) .text("Timestep Control (s)") .drag_value_speed(0.001);

        // set timestep
        ui.add(timestep_slider_seconds);
        let timestep = Time::new::<second>(user_set_timestep_seconds);
        self.simulator_state.lock().unwrap().set_timestep(timestep);
        ui.separator();

        ui.label(" ");

        ui.label("Select nuclide :");

        let mut nuclide = current_simulator_state_clone.get_user_selected_nuclide();

        egui::ComboBox::from_label("User Selected Nuclide")
            .selected_text(format!("{:?}", nuclide))
            .show_ui(ui, |ui| {
                // Fuel / transuranics
                ui.selectable_value(&mut nuclide, Nuclide::U238, "U-238");
                ui.selectable_value(&mut nuclide, Nuclide::U235, "U-235");
                ui.selectable_value(&mut nuclide, Nuclide::Pu238, "Pu-238");
                ui.selectable_value(&mut nuclide, Nuclide::Pu239, "Pu-239");
                ui.selectable_value(&mut nuclide, Nuclide::Pu240, "Pu-240");
                ui.selectable_value(&mut nuclide, Nuclide::Am241, "Am-241");

                ui.separator();

                // Iodine chain (early dose drivers)
                ui.selectable_value(&mut nuclide, Nuclide::I131, "I-131");
                ui.selectable_value(&mut nuclide, Nuclide::I133, "I-133");
                ui.selectable_value(&mut nuclide, Nuclide::I135, "I-135");
                ui.selectable_value(&mut nuclide, Nuclide::I129, "I-129");
                ui.selectable_value(&mut nuclide, Nuclide::I132, "I-132"); // from Te-132

                ui.separator();

                // Cesium (long-term contamination)
                ui.selectable_value(&mut nuclide, Nuclide::Cs137, "Cs-137");
                ui.selectable_value(&mut nuclide, Nuclide::Cs134, "Cs-134");

                ui.separator();

                // Strontium
                ui.selectable_value(&mut nuclide, Nuclide::Sr90, "Sr-90");
                ui.selectable_value(&mut nuclide, Nuclide::Sr89, "Sr-89");

                ui.separator();

                // Noble gases (plume dose)
                ui.selectable_value(&mut nuclide, Nuclide::Xe133, "Xe-133");
                ui.selectable_value(&mut nuclide, Nuclide::Xe135, "Xe-135");
                ui.selectable_value(&mut nuclide, Nuclide::Kr85,  "Kr-85");

                ui.separator();

                // Other important fission products
                ui.selectable_value(&mut nuclide, Nuclide::Te132, "Te-132");
                ui.selectable_value(&mut nuclide, Nuclide::Ba140, "Ba-140");
                ui.selectable_value(&mut nuclide, Nuclide::La140, "La-140");
                ui.selectable_value(&mut nuclide, Nuclide::Zr95,  "Zr-95");
                ui.selectable_value(&mut nuclide, Nuclide::Nb95,  "Nb-95");
                ui.selectable_value(&mut nuclide, Nuclide::Ru103, "Ru-103");
                ui.selectable_value(&mut nuclide, Nuclide::Ru106, "Ru-106");
                ui.selectable_value(&mut nuclide, Nuclide::Mo99,  "Mo-99");
                ui.selectable_value(&mut nuclide, Nuclide::Tc99m, "Tc-99m");
                ui.selectable_value(&mut nuclide, Nuclide::Tc99,  "Tc-99");
                ui.selectable_value(&mut nuclide, Nuclide::Ce144, "Ce-144");
                ui.selectable_value(&mut nuclide, Nuclide::Sb125, "Sb-125");
                ui.selectable_value(&mut nuclide, Nuclide::Ag110m,"Ag-110m");
                ui.selectable_value(&mut nuclide, Nuclide::Eu154, "Eu-154");

                ui.separator();

                // Activation products
                ui.selectable_value(&mut nuclide, Nuclide::H3,    "H-3 (Tritium)");
                ui.selectable_value(&mut nuclide, Nuclide::C14,   "C-14");
                ui.selectable_value(&mut nuclide, Nuclide::Co60,  "Co-60");
                ui.selectable_value(&mut nuclide, Nuclide::Mn54,  "Mn-54");
                ui.selectable_value(&mut nuclide, Nuclide::Fe59,  "Fe-59");
                ui.selectable_value(&mut nuclide, Nuclide::Ar41,  "Ar-41");
                ui.selectable_value(&mut nuclide, Nuclide::N16,   "N-16");
            });

        ui.label(format!("User Selected Nuclide: {:?}", nuclide));
        // vibe coded for extra speed
        // Restart toggle button
        let label = if self.simulator_state.lock().unwrap().is_restart_button_pressed()
        { "Restart (pending...)" } else { "Restart (OFF)" };
        if ui.button(label).clicked() {
            self.simulator_state.lock().unwrap().turn_on_restart_button();
        }

        // Change nuclide toggle button
        let label = if self.simulator_state.lock().unwrap().is_change_nuclide_button_pressed()
        { "Change Nuclide (pending...)" } else { "Change Nuclide (OFF)" };
        if ui.button(label).clicked() {
            self.simulator_state.lock().unwrap().turn_on_change_nuclide_button();
        }
        ui.separator();

        // just for convenience



        ui.heading("CSV Data");
        ui.label("Press Update CSV Data if you want to copy/paste csv data");
        if ui.button("Update CSV Data").clicked(){
            // spawn a new window with csv data
            self.csv_simulator_state = current_simulator_state_clone;

        };
        // allows user to control recording interval
        let record_interval_seconds_slider = egui::Slider::new(
            &mut self.csv_simulator_state.graph_data_record_interval_seconds,
            0.05..=1000.0)
            .logarithmic(true)
            .text("Graph Data Recording Elapsed Time Interval (Seconds)")
            .drag_value_speed(0.001);

        ui.add(record_interval_seconds_slider);

        // allows user to control csv display interval

        let csv_display_interval_seconds_slider = egui::Slider::new(
            &mut self.csv_simulator_state.csv_display_interval_seconds,
            0.1..=1000.0)
            .logarithmic(true)
            .text("CSV Display Elapsed Time Interval (Seconds)")
            .drag_value_speed(0.001);

        ui.add(csv_display_interval_seconds_slider);

        // update the main simulator state with this new info
        self.simulator_state.lock().unwrap().csv_display_interval_seconds =
            self.csv_simulator_state.csv_display_interval_seconds;
        self.simulator_state.lock().unwrap().graph_data_record_interval_seconds =
            self.csv_simulator_state.graph_data_record_interval_seconds;

        let csv_display_interval_seconds =
            self.csv_simulator_state.csv_display_interval_seconds;
        let graph_data_record_interval_seconds =
            self.csv_simulator_state.graph_data_record_interval_seconds;


        // now, we filter data every x number of rows based on the ratio
        // of these two

        let csv_data_display_interval: i32 =
            (csv_display_interval_seconds/graph_data_record_interval_seconds)
            .ceil() as i32;


        // now we display rows every
        // csv_display_interval_seconds
        // rows

        let mut display_counter: i32 = 0;

        // now, let us construct proper labels,
        let mut label_string = "Time (s), ".to_string();

        // let's add all the nuclides to it

        let nuclides_to_plot = csv_simulator_state_clone.get_nuclides_to_plot();
        let nuclide_fractions_over_time: Vec<(Time, Vec<f64>)> =
            csv_simulator_state_clone.get_nuclides_fractions_over_time();

        for nuclide in nuclides_to_plot {
            let nuclide_string = format!("{:?}", nuclide);

            label_string += &nuclide_string;
            label_string += " Fraction, ";

        }


        ui.label(label_string);

        // now we can print the main csv data
        for (time, nuclide_fraction_vector) in nuclide_fractions_over_time {

            let mut data_string = "".to_string();

            // first we add the time in seconds, correct to 9dp (nearest nanosecond)

            let time_nanoseconds = time.get::<nanosecond>().round();
            let time_seconds = Time::new::<nanosecond>(time_nanoseconds).get::<second>();

            data_string += &time_seconds.to_string();
            data_string += ", ";

            // now, for each nuclide we must do the same

            for nuclide_fraction in nuclide_fraction_vector {
                data_string += &nuclide_fraction.to_string();
                data_string += ", ";

            }

            // this allows us to selectively show data for csv
            let blank_data_row =
                time_seconds.round() as u32 != 0;
            let data_display_remainder =
                display_counter.rem_euclid(csv_data_display_interval);

            let data_display_modulus_zero: bool =
                data_display_remainder == 0;

            if blank_data_row && data_display_modulus_zero {
                ui.label(data_string);
            }

            display_counter += 1;
        }


        self.simulator_state.lock().unwrap().set_user_selected_nuclide(
            nuclide
        );

        ui.separator();




    }
}
