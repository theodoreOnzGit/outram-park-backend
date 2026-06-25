use std::sync::{Arc, Mutex};

use boon_lay::{Nuclide, lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoRegion, prelude::{SingleNuclideSimulatorMC, decay_library::DecayLibrary}};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::triso_simulator_v1::backend::simulator_state::SimulatorState;

impl SimulatorState {

    pub fn update_fractions_using_decay_sim_thread_ptrs(
        &mut self,
        decay_sim_plotting_thread_1_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>,
        decay_sim_plotting_thread_2_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>,
        decay_sim_plotting_thread_3_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>,
        decay_sim_plotting_thread_4_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>,
    ){

        let (nuclide_sim_vec_1, _decay_lib_1)
            = decay_sim_plotting_thread_1_ptr.lock().unwrap().clone();
        let (nuclide_sim_vec_2, _decay_lib_2)
            = decay_sim_plotting_thread_2_ptr.lock().unwrap().clone();
        let (nuclide_sim_vec_3, _decay_lib_3)
            = decay_sim_plotting_thread_3_ptr.lock().unwrap().clone();
        let (nuclide_sim_vec_4, _decay_lib_4)
            = decay_sim_plotting_thread_4_ptr.lock().unwrap().clone();

        let full_nuclide_sim_vec: Vec<SingleNuclideSimulatorMC> =
            nuclide_sim_vec_1.into_iter()
            .chain(nuclide_sim_vec_2)
            .chain(nuclide_sim_vec_3)
            .chain(nuclide_sim_vec_4)
            .collect();

        if self.should_change_nuclide_to_graph_plot {

            let nuclides_to_plot: Vec<Nuclide> =
                SingleNuclideSimulatorMC::all_chain_nuclides_unique_sorted(
                    &full_nuclide_sim_vec
                );

            self.nuclides_to_plot = nuclides_to_plot;
            self.nuclide_fractions_over_time = vec![];
            self.turn_off_change_nuclide_to_plot_button();

        }

        let nuclides_to_plot: Vec<Nuclide> = self.nuclides_to_plot.clone();

        let nuclide_count_vector: Vec<(Nuclide, u64)> =
            SingleNuclideSimulatorMC::count_nuclides_in_sims_linear(
                &full_nuclide_sim_vec,
                &nuclides_to_plot
            );

        fn to_fractions_consume(nucs: Vec<(Nuclide, u64)>) -> Vec<(Nuclide, f64)> {
            let total: u128 = nucs.iter().map(|&(_, c)| c as u128).sum();

            if total == 0 {
                return nucs.into_iter().map(|(n, _)| (n, 0.0)).collect();
            }

            let total_f = total as f64;
            nucs.into_iter()
                .map(|(n, c)| (n, (c as f64) / total_f))
                .collect()
        }

        let nuclide_fraction_vector: Vec<(Nuclide, f64)> =
            to_fractions_consume(nuclide_count_vector);

        fn reorder_exact_linear(
            nuclide_fraction_vector: &[(Nuclide, f64)],
            nuclides_to_plot: &[Nuclide],
        ) -> Vec<f64>
        where
            Nuclide: Clone + PartialEq,
        {
            nuclides_to_plot
                .iter()
                .cloned()
                .map(|n| {
                    nuclide_fraction_vector
                        .iter()
                        .find(|(m, _)| *m == n)
                        .map(|(_, f)| *f)
                        .unwrap_or(0.0)
                })
            .collect()
        }

        let nuclide_fraction_vector_float: Vec<f64> = reorder_exact_linear(
            &nuclide_fraction_vector,
            &nuclides_to_plot
        );

        let simulated_time_now = self.simulated_time;

        self.nuclide_fractions_over_time.push(
            (simulated_time_now, nuclide_fraction_vector_float)
        );

        fn keep_last_5000<T>(v: &mut Vec<T>) {
            let keep_from = v.len().saturating_sub(5000);
            if keep_from > 0 {
                v.drain(0..keep_from);
            }
        }

        keep_last_5000(&mut self.nuclide_fractions_over_time);

        let mut release_counter: f64 = 0.0;
        let mut fuel_region_counter: f64 = 0.0;
        let mut buffer_region_counter: f64 = 0.0;
        let mut ipyc_region_counter: f64 = 0.0;
        let mut sic_region_counter: f64 = 0.0;
        let mut opyc_region_counter: f64 = 0.0;
        let mut outside_region_counter: f64 = 0.0;

        let initial_nuclide = self.get_user_selected_nuclide();
        let number_of_particles = full_nuclide_sim_vec.len();
        for simulation in &full_nuclide_sim_vec {

            let nuclide_not_decayed: bool =
                simulation.get_current_nuclide() == initial_nuclide;

            let position = simulation.position;

            let particle_region: TrisoRegion
                = self.triso_cell.get_triso_region(position.into());

            if particle_region == TrisoRegion::Outside && nuclide_not_decayed {
                release_counter += 1.0;
            }

            match particle_region {
                TrisoRegion::Fuel => fuel_region_counter += 1.0,
                TrisoRegion::Buffer => buffer_region_counter += 1.0,
                TrisoRegion::IPyC => ipyc_region_counter += 1.0,
                TrisoRegion::SiC => sic_region_counter += 1.0,
                TrisoRegion::OPyC => opyc_region_counter += 1.0,
                TrisoRegion::Outside => outside_region_counter += 1.0,
            };

        }
        let release_fraction_value = release_counter / (number_of_particles as f64);
        let release_fraction: Ratio =
            Ratio::new::<ratio>(release_fraction_value);

        let total_region_counter = fuel_region_counter +
                buffer_region_counter +
                ipyc_region_counter +
                sic_region_counter +
                opyc_region_counter +
                outside_region_counter;
        assert_eq!(number_of_particles, total_region_counter as usize);
        self.set_release_fraction(release_fraction);

        self.release_fractions_over_time.push(
            (simulated_time_now, release_fraction_value)
        );
        keep_last_5000(&mut self.release_fractions_over_time);
    }

}
