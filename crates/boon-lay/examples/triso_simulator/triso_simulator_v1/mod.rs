use std::time::SystemTime;
use std::{sync::Arc, thread, time::Duration};

use std::sync::{Barrier, Mutex};

use boon_lay::prelude::decay_library::DecayLibrary;
use boon_lay::prelude::SingleNuclideSimulatorMC;
use boon_lay::Nuclide;
use rayon::prelude::*;

use crate::triso_simulator_v1::backend::simulator_state::SimulatorState;
use crate::triso_simulator_v1::front_end::Panel;

pub fn triso_decay_diffusion_simulator_v1() -> eframe::Result<()> {

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "TRISO Decay and Diffusion Simulator v1 Powered by Boon Lay",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(TRISOSimApp::new(cc)))
        }),
    )
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TRISOSimApp {
    label: String,

    #[serde(skip)]
    value: f64,

    open_panel: Panel,

    #[serde(skip)]
    decay_sim_thread_1_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>,DecayLibrary)>>,
    #[serde(skip)]
    decay_sim_thread_2_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>,DecayLibrary)>>,
    #[serde(skip)]
    decay_sim_thread_3_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>,DecayLibrary)>>,
    #[serde(skip)]
    decay_sim_thread_4_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>,DecayLibrary)>>,

    #[serde(skip)]
    simulator_state: Arc<Mutex<SimulatorState>>,

    #[serde(skip)]
    csv_simulator_state: SimulatorState,

    user_wants_fast_fwd_on: bool,
    user_wants_slow_motion_on: bool,
}

impl TRISOSimApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let new_decay_sim_app: TRISOSimApp = Default::default();

        let decay_sim_thread_1_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>> =
            new_decay_sim_app.decay_sim_thread_1_ptr.clone();
        let decay_sim_thread_2_ptr =
            new_decay_sim_app.decay_sim_thread_2_ptr.clone();
        let decay_sim_thread_3_ptr =
            new_decay_sim_app.decay_sim_thread_3_ptr.clone();
        let decay_sim_thread_4_ptr =
            new_decay_sim_app.decay_sim_thread_4_ptr.clone();

        let simulator_state_thread_1_ptr: Arc<Mutex<SimulatorState>> =
            new_decay_sim_app.simulator_state.clone();
        let simulator_state_thread_2_ptr: Arc<Mutex<SimulatorState>> =
            new_decay_sim_app.simulator_state.clone();
        let simulator_state_thread_3_ptr: Arc<Mutex<SimulatorState>> =
            new_decay_sim_app.simulator_state.clone();
        let simulator_state_thread_4_ptr: Arc<Mutex<SimulatorState>> =
            new_decay_sim_app.simulator_state.clone();

        let decay_sim_plotting_thread_1_ptr =
            new_decay_sim_app.decay_sim_thread_1_ptr.clone();
        let decay_sim_plotting_thread_2_ptr =
            new_decay_sim_app.decay_sim_thread_2_ptr.clone();
        let decay_sim_plotting_thread_3_ptr =
            new_decay_sim_app.decay_sim_thread_3_ptr.clone();
        let decay_sim_plotting_thread_4_ptr =
            new_decay_sim_app.decay_sim_thread_4_ptr.clone();

        simulator_state_thread_4_ptr.lock().unwrap().update_fractions_using_decay_sim_thread_ptrs(
            decay_sim_plotting_thread_1_ptr.clone(),
            decay_sim_plotting_thread_2_ptr.clone(),
            decay_sim_plotting_thread_3_ptr.clone(),
            decay_sim_plotting_thread_4_ptr.clone(),
        );

        let num_threads = 4;
        let barrier: Arc<Barrier> = Arc::new(Barrier::new(num_threads));
        let barrier_1 = Arc::clone(&barrier);
        let barrier_2 = Arc::clone(&barrier);
        let barrier_3 = Arc::clone(&barrier);
        let barrier_4 = Arc::clone(&barrier);

        thread::spawn(move ||{
            let thread_number = 1;
            Self::run_decay_chain_simulation(
                decay_sim_thread_1_ptr,
                simulator_state_thread_1_ptr,
                thread_number,
                barrier_1,
            );
        });
        thread::spawn(move ||{
            let thread_number = 2;
            Self::run_decay_chain_simulation(
                decay_sim_thread_2_ptr,
                simulator_state_thread_2_ptr,
                thread_number,
                barrier_2,
            );
        });
        thread::spawn(move ||{
            let thread_number = 3;
            Self::run_decay_chain_simulation(
                decay_sim_thread_3_ptr,
                simulator_state_thread_3_ptr,
                thread_number,
                barrier_3,
            );
        });
        thread::spawn(move ||{
            let thread_number = 4;
            Self::run_decay_chain_simulation(
                decay_sim_thread_4_ptr,
                simulator_state_thread_4_ptr,
                thread_number,
                barrier_4,
            );
        });
        let simulator_state_thread_5_ptr: Arc<Mutex<SimulatorState>> =
            new_decay_sim_app.simulator_state.clone();

        thread::spawn(move ||{
            loop {
                simulator_state_thread_5_ptr.lock().unwrap().update_fractions_using_decay_sim_thread_ptrs(
                    decay_sim_plotting_thread_1_ptr.clone(),
                    decay_sim_plotting_thread_2_ptr.clone(),
                    decay_sim_plotting_thread_3_ptr.clone(),
                    decay_sim_plotting_thread_4_ptr.clone(),
                );

                let time_to_sleep_seconds =
                    simulator_state_thread_5_ptr.lock().unwrap().graph_data_record_interval_seconds;

                let time_to_sleep_milliseconds: u64 =
                    (time_to_sleep_seconds*1000.0).round() as u64;
                let time_to_sleep_non_realtime: Duration =
                    Duration::from_millis(time_to_sleep_milliseconds);
                thread::sleep(time_to_sleep_non_realtime);
            };
        });

        new_decay_sim_app
    }
}

impl Default for TRISOSimApp {
    fn default() -> Self {
        let time_start = SystemTime::now();

        let num_of_nuclides = 62_500;
        let nuclide = Nuclide::Cs137;

        fn build_four_vec(
            num_of_nuclides: usize,
            nuclide: Nuclide,
        ) -> (Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>,
        Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>,
        Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>,
        Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>) {
            let seeds = [550_u64, 47, 58, 1414];

            let sims: Vec<Arc<Mutex<(Vec<SingleNuclideSimulatorMC>, DecayLibrary)>>> = seeds
                .par_iter()
                .map(|&seed| {
                    TRISOSimApp::construct_new_single_thread_multi_particle_simulation(
                        num_of_nuclides.try_into().unwrap(),
                        nuclide,
                        seed,
                    )
                })
            .collect();

            let mut it = sims.into_iter();
            (
                it.next().unwrap(),
                it.next().unwrap(),
                it.next().unwrap(),
                it.next().unwrap(),
            )
        }
        let (decay_sim_thread_1_ptr,
            decay_sim_thread_2_ptr,
            decay_sim_thread_3_ptr,
            decay_sim_thread_4_ptr,)
            = build_four_vec(num_of_nuclides.try_into().unwrap(), nuclide);

        let simulator_state = Arc::new(Mutex::new(SimulatorState::default()));
        let csv_simulator_state: SimulatorState = simulator_state.lock().unwrap().clone();

        let initiation_time_secs = time_start.elapsed().unwrap().as_secs();
        dbg!(&initiation_time_secs);

        Self {
            label: "Boon Lay Decay Simulator v1".to_owned(),
            value: 3.6,
            open_panel: Panel::MainPage,
            user_wants_fast_fwd_on: false,
            user_wants_slow_motion_on: false,
            decay_sim_thread_1_ptr,
            decay_sim_thread_2_ptr,
            decay_sim_thread_3_ptr,
            decay_sim_thread_4_ptr,
            simulator_state,
            csv_simulator_state,
        }
    }
}

impl eframe::App for TRISOSimApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        egui::TopBottomPanel::top("top_panel").show(&ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
            });

            egui::ScrollArea::both().show(ui, |ui| {
                ui.heading("TRISO Diffusion and Decay Simulator v1");
                ui.separator();
                ui.horizontal(
                    |ui| {
                        ui.selectable_value(&mut self.open_panel, Panel::MainPage, "Main Page");
                        ui.selectable_value(&mut self.open_panel, Panel::GraphPage, "Graph Page");
                        ui.selectable_value(&mut self.open_panel, Panel::PeriodicTable, "Periodic Table (Legend)");
                    }
                );
                ui.separator();
            });
        });

        egui::SidePanel::right("Supplementary Info").show(&ctx, |ui|{
            match self.open_panel {
                Panel::MainPage => {
                    egui::ScrollArea::both().show(ui, |ui| {
                        self.side_panel(ui);
                        self.citation_disclaimer_and_acknowledgements(ui);
                    });
                },
                Panel::GraphPage => {
                    egui::ScrollArea::both().show(ui, |ui| {
                        self.graph_page_side_panel(ui);
                        self.citation_disclaimer_and_acknowledgements(ui);
                    });
                },
                _ => {
                    egui::ScrollArea::both().show(ui, |ui| {
                        self.side_panel(ui);
                        self.citation_disclaimer_and_acknowledgements(ui);
                    });
                },
            }
        });

        egui::TopBottomPanel::bottom("github").show(&ctx, |ui|{
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });

        egui::CentralPanel::default().show(&ctx, |ui| {
            match self.open_panel {
                Panel::MainPage => {
                    egui::ScrollArea::both().show(ui, |ui| {
                        self.main_page(ui);
                    });
                },
                Panel::PeriodicTable => {
                    egui::ScrollArea::both().show(ui, |ui| {
                        self.periodic_table(ui);
                    });
                },
                Panel::GraphPage => {
                    egui::ScrollArea::both().show(ui, |ui| {
                        self.graph_page(ui);
                    });
                },
            }

            ui.add(egui::github_link_file!(
                    "https://github.com/theodoreOnzGit/boon-lay/blob/develop/",
                    "Boon Lay Github Repo"
            ));
        });

        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}

pub mod front_end;
pub mod backend;
