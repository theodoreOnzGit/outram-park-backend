use boon_lay::prelude::NuclideReactionAndDecayData;
use boon_lay::Nuclide;
use egui::Ui;
use uom::si::{f64::{ThermodynamicTemperature, Time}, ratio::ratio, thermodynamic_temperature::{degree_celsius, kelvin}, time::{day, hour, millisecond, second, year}};

use crate::triso_simulator_v1::{backend::simulator_state::SimulatorState, TRISOSimApp};


impl TRISOSimApp {

    pub fn side_panel(&mut self, ui: &mut Ui){

        ui.heading("Timestep and Time");

        ui.label(" ");

        let simulator_state_clone : SimulatorState
            = self.simulator_state.lock().unwrap().clone();

        let elapsed_time = simulator_state_clone.get_elapsed_time();
        let mut elapsed_time_string: String = "Elapsed Time (seconds):".to_string();
        elapsed_time_string += &elapsed_time.get::<second>().to_string();

        ui.label(elapsed_time_string);

        let mut user_set_timestep_seconds
            = simulator_state_clone.get_timestep().get::<second>();

        let timestep_slider_seconds = egui::Slider::new(
            &mut user_set_timestep_seconds,
            0.00001..=1500.0
        ) .logarithmic(false) .text("Timestep Control (s)") .drag_value_speed(0.001);

        ui.add(timestep_slider_seconds);
        let timestep = Time::new::<second>(user_set_timestep_seconds);
        self.simulator_state.lock().unwrap().set_timestep(timestep);
        ui.separator();

        ui.label(" ");

        let fmt5 = |x: f64| -> String { format!("{:.5}", x) };

        ui.separator();
        ui.label("Simulated time");

        let sim_t = simulator_state_clone.get_simulated_time();

        ui.label(format!("Simulated Time (seconds): {}",     fmt5(sim_t.get::<second>())));
        ui.label(format!("Simulated Time (days): {}",        fmt5(sim_t.get::<day>())));
        ui.label(format!("Simulated Time (hours): {}",        fmt5(sim_t.get::<hour>())));
        ui.label(format!("Simulated Time (years): {}",       fmt5(sim_t.get::<year>())));

        let sim_years = sim_t.get::<year>();
        ui.label(format!("Simulated Time (billion years): {}", fmt5(sim_years / 1.0e9)));

        ui.separator();
        ui.label("Timestep");

        let dt = simulator_state_clone.get_timestep();

        ui.label(format!("Timestep (milliseconds): {}", fmt5(dt.get::<millisecond>())));
        ui.label(format!("Timestep (seconds): {}",      fmt5(dt.get::<second>())));
        ui.label(format!("Timestep (days): {}",         fmt5(dt.get::<day>())));
        ui.label(format!("Timestep (hours): {}",         fmt5(dt.get::<hour>())));
        ui.label(format!("Timestep (years): {}",        fmt5(dt.get::<year>())));

        ui.separator();

        ui.heading("Fraction of Nuclides yet to Decay");
        let mut nuclide_fraction_remaining: f64 =
            simulator_state_clone.get_nuclide_fraction().get::<ratio>();

        nuclide_fraction_remaining =
            (nuclide_fraction_remaining * 1e5_f64).round() /
            1e5_f64;
        let mut surviving_fraction_string: String = "Surviving Fraction:".to_string();
        surviving_fraction_string += &nuclide_fraction_remaining.to_string();
        ui.label(surviving_fraction_string);
        ui.label(" ");

        ui.separator();

        ui.label("Select nuclide :");

        let mut nuclide = simulator_state_clone.get_user_selected_nuclide();

        egui::ComboBox::from_label("User Selected Nuclide")
            .selected_text(format!("{:?}", nuclide))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut nuclide, Nuclide::U238, "U-238");
                ui.selectable_value(&mut nuclide, Nuclide::U235, "U-235");
                ui.selectable_value(&mut nuclide, Nuclide::Pu238, "Pu-238");
                ui.selectable_value(&mut nuclide, Nuclide::Pu239, "Pu-239");
                ui.selectable_value(&mut nuclide, Nuclide::Pu240, "Pu-240");
                ui.selectable_value(&mut nuclide, Nuclide::Am241, "Am-241");
                ui.separator();
                ui.selectable_value(&mut nuclide, Nuclide::I131, "I-131");
                ui.selectable_value(&mut nuclide, Nuclide::I133, "I-133");
                ui.selectable_value(&mut nuclide, Nuclide::I135, "I-135");
                ui.selectable_value(&mut nuclide, Nuclide::I129, "I-129");
                ui.selectable_value(&mut nuclide, Nuclide::I132, "I-132");
                ui.separator();
                ui.selectable_value(&mut nuclide, Nuclide::Cs137, "Cs-137");
                ui.selectable_value(&mut nuclide, Nuclide::Cs134, "Cs-134");
                ui.separator();
                ui.selectable_value(&mut nuclide, Nuclide::Sr90, "Sr-90");
                ui.selectable_value(&mut nuclide, Nuclide::Sr89, "Sr-89");
                ui.separator();
                ui.selectable_value(&mut nuclide, Nuclide::Xe133, "Xe-133");
                ui.selectable_value(&mut nuclide, Nuclide::Xe135, "Xe-135");
                ui.selectable_value(&mut nuclide, Nuclide::Kr85,  "Kr-85");
                ui.separator();
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
                ui.selectable_value(&mut nuclide, Nuclide::H3,    "H-3 (Tritium)");
                ui.selectable_value(&mut nuclide, Nuclide::C14,   "C-14");
                ui.selectable_value(&mut nuclide, Nuclide::Co60,  "Co-60");
                ui.selectable_value(&mut nuclide, Nuclide::Mn54,  "Mn-54");
                ui.selectable_value(&mut nuclide, Nuclide::Fe59,  "Fe-59");
                ui.selectable_value(&mut nuclide, Nuclide::Ar41,  "Ar-41");
                ui.selectable_value(&mut nuclide, Nuclide::N16,   "N-16");
            });

        ui.label(format!("User Selected Nuclide: {:?}", nuclide));

        let label = if self.simulator_state.lock().unwrap().is_restart_button_pressed()
        { "Restart (pending...)" } else { "Restart (OFF)" };
        if ui.button(label).clicked() {
            self.simulator_state.lock().unwrap().turn_on_restart_button();
        }

        let label = if self.simulator_state.lock().unwrap().is_change_nuclide_button_pressed()
        { "Change Nuclide (pending...)" } else { "Change Nuclide (OFF)" };
        if ui.button(label).clicked() {
            self.simulator_state.lock().unwrap().turn_on_change_nuclide_button();
        }
        ui.separator();

        let (_,nuclide_library) =
            self.decay_sim_thread_1_ptr.lock().unwrap().clone();
        let nuclide_data: NuclideReactionAndDecayData
            = nuclide_library.try_match_nuclides_to_decay_data(
                nuclide
            ).unwrap();

        ui.label(" ");

        let half_life: Time = nuclide_data.try_get_half_life().unwrap();
        let hl_ms   = half_life.get::<millisecond>();
        let hl_s    = half_life.get::<second>();
        let hl_days = half_life.get::<day>();
        let hl_years = half_life.get::<year>();
        let hl_gyr   = hl_years / 1.0e9;

        ui.label(format!("Half-life (milliseconds): {:.5}", hl_ms));
        ui.label(format!("Half-life (seconds): {:.5}", hl_s));
        ui.label(format!("Half-life (days): {:.5}", hl_days));
        ui.label(format!("Half-life (years): {:.5}", hl_years));
        ui.label(format!("Half-life (billion years): {:.5}", hl_gyr));

        ui.separator();

        self.simulator_state.lock().unwrap().set_user_selected_nuclide(
            nuclide
        );

        ui.separator();

        ui.heading("TRISO Particle Temperature Control");
        let mut simulator_state_guard = self.simulator_state.lock().unwrap();

        ui.label(format!(
                "Current Active Temperature: {:.2} °C",
                simulator_state_guard.get_triso_uniform_temperature().get::<degree_celsius>()
        ));
        ui.label(format!(
                "Current Active Temperature: {:.2} K",
                simulator_state_guard.get_triso_uniform_temperature().get::<kelvin>()
        ));

        let mut user_selected_temp_celsius = simulator_state_guard.get_user_selected_temperature().get::<degree_celsius>();

        ui.add(egui::Slider::new(&mut user_selected_temp_celsius, 0.0..=2200.0)
            .text("Desired Temperature (Celsius)")
            .suffix(" °C")
            .logarithmic(false)
            .drag_value_speed(1.0)
        );

        let new_user_selected_temp = ThermodynamicTemperature::new::<degree_celsius>(user_selected_temp_celsius);
        if new_user_selected_temp != simulator_state_guard.get_user_selected_temperature() {
            simulator_state_guard.set_user_selected_temperature(new_user_selected_temp);
        }

        if ui.button("Change Temperature").clicked() {
            let temp_to_apply = simulator_state_guard.get_user_selected_temperature();
            simulator_state_guard.set_triso_uniform_temperature(temp_to_apply);
        }

        drop(simulator_state_guard);
        ui.separator();

        ui.heading("Nuclides in Simulation and their fraction");

        fn symbol_from_z(z: u16) -> &'static str {
            static SYMBOLS: [&str; 119] = [
                "", "H","He","Li","Be","B","C","N","O","F","Ne",
                "Na","Mg","Al","Si","P","S","Cl","Ar",
                "K","Ca","Sc","Ti","V","Cr","Mn","Fe","Co","Ni","Cu","Zn",
                "Ga","Ge","As","Se","Br","Kr",
                "Rb","Sr","Y","Zr","Nb","Mo","Tc","Ru","Rh","Pd","Ag","Cd",
                "In","Sn","Sb","Te","I","Xe",
                "Cs","Ba","La","Ce","Pr","Nd","Pm","Sm","Eu","Gd","Tb","Dy","Ho","Er","Tm","Yb","Lu",
                "Hf","Ta","W","Re","Os","Ir","Pt","Au","Hg",
                "Tl","Pb","Bi","Po","At","Rn",
                "Fr","Ra","Ac","Th","Pa","U","Np","Pu","Am","Cm","Bk","Cf","Es","Fm","Md","No","Lr",
                "Rf","Db","Sg","Bh","Hs","Mt","Ds","Rg","Cn",
                "Nh","Fl","Mc","Lv","Ts","Og",
            ];
            if z <= 118 { SYMBOLS[z as usize] } else { "?" }
        }

        fn nuclide_to_string(n: Nuclide) -> String {
            let (z, a) = n.get_z_a();
            let sym = symbol_from_z(z as u16);
            if a > 0 { format!("{sym}-{a}") } else { sym.to_string() }
        }

        fn ui_fraction_list(ui: &mut Ui, fractions: &[(Nuclide, f64)]) {
            let mut items = fractions.to_vec();
            items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (n, frac) in items.iter() {
                ui.label(format!(
                        "{} fraction : {:.5}",
                        nuclide_to_string(*n),
                        frac
                ));
            }
        }

        let nuclide_fraction_vec = &simulator_state_clone.get_nuclide_fraction_vector();

        ui_fraction_list(ui, nuclide_fraction_vec);

        let release_fraction: f64 = simulator_state_clone.get_release_fraction()
            .get::<ratio>();

        ui.label(format!("Release Fraction: {:.5}", release_fraction));

        ui.separator();

    }


    pub fn fractions_vec_map(nucs: &[Nuclide]) -> Vec<(Nuclide, f64)>
    where
        Nuclide: Clone,
        {
            let total = nucs.len() as f64;
            if total == 0.0 {
                return Vec::new();
            }

            let mut counts: Vec<(Nuclide, u64)> = Vec::new();

            for n in nucs {
                let key = n.get_z_a();
                if let Some((_, c)) = counts.iter_mut().find(|(rep, _)| rep.get_z_a() == key) {
                    *c += 1;
                } else {
                    counts.push((n.clone(), 1));
                }
            }

            counts
                .into_iter()
                .map(|(rep, c)| (rep, c as f64 / total))
                .collect()
        }

}
