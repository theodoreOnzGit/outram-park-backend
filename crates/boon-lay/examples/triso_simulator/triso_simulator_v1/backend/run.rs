use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

use boon_lay::Nuclide;
use boon_lay::compute::ComputeType;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{
    WalkParams, WoSWalker,
};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::cached_normals::DiffusionRandomCache;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;
use boon_lay::prelude::SingleNuclideSimulatorMC;
use boon_lay::prelude::decay_library::DecayLibrary;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::SingleParticleDiffusionSimulatorMC;
use rayon::prelude::*;
use uom::si::time::second;
use uom::si::f64::Time;
use uom::si::time::millisecond;

use crate::triso_simulator_v1::TRISOSimApp;
use crate::triso_simulator_v1::front_end::triso_particle::TrisoParticleUi;
use crate::triso_simulator_v1::backend::simulator_state::SimulatorState;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;

impl TRISOSimApp {
    pub fn run_decay_chain_simulation(
        thread_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>,
        simulator_state_ptr: Arc<Mutex<SimulatorState>>,
        thread_number: u8,
        barrier: Arc<Barrier>,
    ) {
        let loop_time = SystemTime::now();

        let mut particle_simulator_rng: OoRng64 = OoRng64::from_u64(thread_number as u64 * 7);

        let mut diffusion_simulator =
            SingleParticleDiffusionSimulatorMC::new_from_rng(&mut particle_simulator_rng);

        let new_triso_particle_ui = TrisoParticleUi::default();

        let cached_normals = DiffusionRandomCache::new(1e5 as usize);

        loop {
            let loop_time_start = loop_time.elapsed().unwrap();
            let simulator_state_clone: SimulatorState = simulator_state_ptr.lock().unwrap().clone();

            if simulator_state_clone.is_paused() {
                continue;
            }

            let triso_cell = simulator_state_clone.triso_cell;

            if simulator_state_clone.is_restart_button_pressed() {
                let (mut simulation_vector, mut decay_library): (
                    Vec<SingleNuclideSimulatorMC>,
                    DecayLibrary,
                ) = thread_ptr.lock().unwrap().clone();

                let user_set_nuclide: Nuclide = simulator_state_ptr
                    .lock()
                    .unwrap()
                    .get_user_selected_nuclide();

                let _buffer_radius = new_triso_particle_ui.get_diameter_after_buffer() * 0.5;
                let _ipyc_radius = new_triso_particle_ui.get_diameter_after_ipyc() * 0.5;
                let fuel_radius = new_triso_particle_ui.get_diameter_after_fuel() * 0.5;
                let opyc_radius = new_triso_particle_ui.get_diameter_after_opyc() * 0.5;
                let mut rng_for_position = OoRng64::from_u64(thread_number as u64 * 4);

                for simulation in simulation_vector.iter_mut() {
                    let mut new_simulation = SingleNuclideSimulatorMC::new_decay_chain_simulation(
                        user_set_nuclide,
                        &mut decay_library,
                    );

                    let coordinate = Self::random_point_in_triso(
                        fuel_radius,
                        opyc_radius,
                        &mut decay_library.random_number_generator,
                        &mut rng_for_position,
                    );
                    new_simulation.position = coordinate;

                    *simulation = new_simulation;
                }

                let timestep_based_on_diffusion: Time = Time::new::<second>(1.0);

                barrier.wait();

                *thread_ptr.lock().unwrap() = (simulation_vector, decay_library);
                if thread_number == 1 {
                    simulator_state_ptr
                        .lock()
                        .unwrap()
                        .turn_off_restart_button();
                    simulator_state_ptr
                        .lock()
                        .unwrap()
                        .turn_off_change_nuclide_button();
                    simulator_state_ptr.lock().unwrap().reset_simulated_time();
                    simulator_state_ptr
                        .lock()
                        .unwrap()
                        .set_timestep(timestep_based_on_diffusion);

                    simulator_state_ptr
                        .lock()
                        .unwrap()
                        .turn_on_change_nuclide_to_plot_button();
                }

                barrier.wait();
            }

            if simulator_state_clone.is_change_nuclide_button_pressed() {
                let (mut simulation_vector, mut decay_library): (
                    Vec<SingleNuclideSimulatorMC>,
                    DecayLibrary,
                ) = thread_ptr.lock().unwrap().clone();

                let user_set_nuclide: Nuclide = simulator_state_ptr
                    .lock()
                    .unwrap()
                    .get_user_selected_nuclide();

                barrier.wait();
                for simulation in simulation_vector.iter_mut() {
                    simulation.transmute_nuclide(user_set_nuclide, &mut decay_library);
                }
                *thread_ptr.lock().unwrap() = (simulation_vector, decay_library.clone());

                let timestep_based_on_diffusion: Time = Time::new::<second>(1.0);
                barrier.wait();

                if thread_number == 1 {
                    simulator_state_ptr
                        .lock()
                        .unwrap()
                        .turn_off_restart_button();
                    simulator_state_ptr
                        .lock()
                        .unwrap()
                        .turn_off_change_nuclide_button();
                    simulator_state_ptr
                        .lock()
                        .unwrap()
                        .set_timestep(timestep_based_on_diffusion);

                    simulator_state_ptr
                        .lock()
                        .unwrap()
                        .turn_on_change_nuclide_to_plot_button();
                }

                barrier.wait();
            }

            let (mut simulation_vector, decay_library): (
                Vec<SingleNuclideSimulatorMC>,
                DecayLibrary,
            ) = thread_ptr.lock().unwrap().clone();

            let timestep = simulator_state_clone.get_timestep();

            // Decay each atom on the CPU (unchanged), then advance its diffusion
            // with the fast, timestep-free Walk-on-Spheres engine via the selected
            // compute backend — replacing the old Fourier sub-timestepping path
            // that made the buffer layer crawl.
            for decay_simulation in simulation_vector.iter_mut() {
                decay_simulation.advance_timestep(timestep);
            }
            advance_diffusion_wos(
                &mut simulation_vector,
                &mut diffusion_simulator.rng,
                triso_cell,
                timestep,
                simulator_state_clone.get_compute_backend(),
                simulator_state_clone.get_user_selected_nuclide(),
            );
            let _ = &cached_normals; // retained for the legacy Fourier path
            *thread_ptr.lock().unwrap() = (simulation_vector, decay_library);

            let realtime = false;

            if realtime {
                let loop_time_end = loop_time.elapsed().unwrap();
                let time_taken_for_calculation_loop_milliseconds: f64 =
                    (loop_time_end - loop_time_start).as_millis() as f64;

                let time_to_sleep_milliseconds: u64 = (timestep.get::<millisecond>()
                    - time_taken_for_calculation_loop_milliseconds)
                    .round()
                    .abs() as u64;

                let time_to_sleep_realtime: Duration =
                    Duration::from_millis(time_to_sleep_milliseconds - 1);
                thread::sleep(time_to_sleep_realtime);
            } else {
                let time_to_sleep_non_realtime: Duration = Duration::from_millis(5);
                thread::sleep(time_to_sleep_non_realtime);
            }

            barrier.wait();

            if thread_number == 1 {
                simulator_state_ptr
                    .lock()
                    .unwrap()
                    .add_to_simulated_time(timestep);

                let elapsed_time_seconds =
                    (loop_time.elapsed().unwrap().as_secs_f64() * 100.0).round() / 100.0;
                let elapsed_time = Time::new::<second>(elapsed_time_seconds);

                let (simulation_vector, _decay_library): (
                    Vec<SingleNuclideSimulatorMC>,
                    DecayLibrary,
                ) = thread_ptr.lock().unwrap().clone();

                let user_set_nuclide: Nuclide = simulator_state_ptr
                    .lock()
                    .unwrap()
                    .get_user_selected_nuclide();

                let mut surviving_nuclide_counter = 0;

                for nuclide_simulation in simulation_vector.iter() {
                    if nuclide_simulation.check_if_current_nuclide_matches(user_set_nuclide) {
                        surviving_nuclide_counter += 1;
                    }
                }

                let nuclide_fraction_remaining =
                    surviving_nuclide_counter as f64 / simulation_vector.clone().len() as f64;

                simulator_state_ptr
                    .lock()
                    .unwrap()
                    .set_nuclide_fraction(nuclide_fraction_remaining);

                simulator_state_ptr
                    .lock()
                    .unwrap()
                    .set_elapsed_time(elapsed_time);
            }

            if thread_number == 2 {
                let (simulation_vector, _decay_library): (
                    Vec<SingleNuclideSimulatorMC>,
                    DecayLibrary,
                ) = thread_ptr.lock().unwrap().clone();

                let mut nuclide_vector: Vec<Nuclide> = vec![];

                for simulation in &simulation_vector {
                    let nuclide = simulation.get_current_nuclide();
                    nuclide_vector.push(nuclide);
                }

                let nuclide_fraction_vector: Vec<(Nuclide, f64)> =
                    TRISOSimApp::fractions_vec_map(&nuclide_vector);

                simulator_state_ptr
                    .lock()
                    .unwrap()
                    .set_nuclide_fraction_vector(nuclide_fraction_vector);
            }

            let debug = false;
            if thread_number == 3 && debug {
                let (simulation_vector, _decay_library): (
                    Vec<SingleNuclideSimulatorMC>,
                    DecayLibrary,
                ) = thread_ptr.lock().unwrap().clone();

                let decay_sim = simulation_vector[0].clone();

                let pos = decay_sim.position;
                let elapsed_time_seconds =
                    (loop_time.elapsed().unwrap().as_secs_f64() * 100.0).round() / 100.0;
                let elapsed_time = Time::new::<second>(elapsed_time_seconds);
                let nuclide = decay_sim.get_current_nuclide();

                dbg!(&(elapsed_time, nuclide, pos));
            }
            barrier.wait();
        }
    }
}

/// Advance every atom's diffusion by `timestep` with the Walk-on-Spheres engine,
/// using the selected compute backend, then write the new positions back.
///
/// Each atom becomes a transient [`WoSWalker`] at its current position and
/// nuclide, started at `t = 0`, and is diffused to `t = timestep`. The CPU
/// backends diffuse each atom with its *own* nuclide's diffusion coefficient
/// (exact); the GPU backend batches the whole population through the multilayer
/// kernel using the **user-selected** nuclide's coefficient (a single-`D`
/// approximation for the visualization — the kernel is one nuclide per dispatch),
/// and transparently falls back to the CPU when no adapter is present.
fn advance_diffusion_wos(
    sims: &mut [SingleNuclideSimulatorMC],
    rng: &mut OoRng64,
    triso_cell: TrisoCell,
    timestep: Time,
    backend: ComputeType,
    selected_nuclide: Nuclide,
) {
    let params = WalkParams::crp6_default();

    let mut walkers: Vec<WoSWalker> = sims
        .iter()
        .map(|s| {
            let (x, y, z) = s.position;
            let child = OoRng64::from_u64(rng.next_u64());
            WoSWalker::new([x, y, z], s.get_current_nuclide(), child)
        })
        .collect();
    let mut released = vec![false; walkers.len()];

    match backend {
        ComputeType::Gpu => {
            if !boon_lay::gpu::advance_multilayer_best_effort(
                &triso_cell,
                &params,
                &mut walkers,
                &mut released,
                selected_nuclide,
                timestep,
            ) {
                diffuse_cpu(
                    &mut walkers,
                    &mut released,
                    &triso_cell,
                    &params,
                    timestep,
                    true,
                );
            }
        }
        ComputeType::CpuMultiThread(_) => diffuse_cpu(
            &mut walkers,
            &mut released,
            &triso_cell,
            &params,
            timestep,
            true,
        ),
        ComputeType::CpuSingleThread => diffuse_cpu(
            &mut walkers,
            &mut released,
            &triso_cell,
            &params,
            timestep,
            false,
        ),
    }

    for (s, w) in sims.iter_mut().zip(walkers.iter()) {
        s.position = (w.position[0], w.position[1], w.position[2]);
    }
}

/// CPU diffusion of a walker batch to `until`, sequential or rayon-parallel.
fn diffuse_cpu(
    walkers: &mut [WoSWalker],
    released: &mut [bool],
    triso_cell: &TrisoCell,
    params: &WalkParams,
    until: Time,
    multi_thread: bool,
) {
    if multi_thread {
        walkers
            .par_iter_mut()
            .zip(released.par_iter_mut())
            .for_each(|(w, r)| {
                if !*r && !w.diffuse_until(triso_cell, params, until) {
                    *r = true;
                }
            });
    } else {
        for (w, r) in walkers.iter_mut().zip(released.iter_mut()) {
            if !*r && !w.diffuse_until(triso_cell, params, until) {
                *r = true;
            }
        }
    }
}
