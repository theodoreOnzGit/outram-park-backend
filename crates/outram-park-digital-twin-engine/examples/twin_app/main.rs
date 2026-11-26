//! # The multifidelity twin template's app (gh:#1745)
//!
//! `TwinApp` (`outram_park_digital_twin_engine::twin::app`) over a native
//! `LocalHost`, for the twins built on the template so far: the MSRE
//! (`msre::twin`, gh:#1748, the default) and EBR-II (`ebr2::twin`, gh:#1749,
//! `--ebr2`). Five pages: Plant, Control volumes, Coarse- and Fine-mesh
//! multiphysics, Monte Carlo. A page with no model says so and names its
//! issue.
//!
//! ```text
//! cargo run --release -p outram-park-digital-twin-engine --example twin_app [-- --ebr2]
//! cargo run --release -p outram-park-digital-twin-engine --example twin_app -- --headless [end_s] [--ebr2]
//! cargo run --release -p outram-park-digital-twin-engine --example twin_app -- --dump-cvs [steps] [--ebr2]
//! cargo run --release -p outram-park-digital-twin-engine --example twin_app -- --coefficients   # MSRE's live split, many minutes
//! ```
//!
//! `--headless` prints the template's plant CSV of the reactor's
//! demonstration script in 1 s steps (default 600 s); the GUI runs the same
//! plant on a thread at 10× real time. The headless outputs are the
//! library's `MultiFidelityTwin::{plant_csv, dump_cvs, coefficient_report}`,
//! tested there; `main` and the control panels are GUI glue.
//!
//! Research, education and V&V only. NOT VALIDATED. Offline demonstration.

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn main() {
    // Native only: the browser twins are the msre_web / ebr2_web demos.
}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod native {
    use outram_park_digital_twin_engine::ebr2::session::{Command as ECmd, SessionConfig as ECfg, Snapshot as ESnap};
    use outram_park_digital_twin_engine::ebr2::twin::{self as ebr2, Ebr2Plant};
    use outram_park_digital_twin_engine::msre::session::{Command as MCmd, SessionConfig as MCfg, Snapshot as MSnap};
    use outram_park_digital_twin_engine::msre::twin::{self as msre, MsrePlant};
    use outram_park_digital_twin_engine::twin::app::{TwinApp, TwinHost};
    use outram_park_digital_twin_engine::twin::local::{LocalHost, PlantControls};
    use uom::si::f64::Time;
    use uom::si::time::second;

    /// The MSRE's operator controls (GUI code).
    #[derive(Default)]
    pub struct MsreControls {
        rods_pcm: f64,
        air: f64,
    }

    impl PlantControls<MsrePlant> for MsreControls {
        fn controls(&mut self, ui: &mut egui::Ui, latest: Option<&MSnap>) -> Vec<MCmd> {
            let mut out = Vec::new();
            if let Some(s) = latest {
                ui.label(format!("t = {:.0} s · {:.2} MW · fuel {:.0} °F", s.time_s, s.power_mw, s.fuel_f));
            }
            if ui.add(egui::Slider::new(&mut self.rods_pcm, -50.0..=50.0).text("rods, pcm")).changed() {
                out.push(MCmd::RodsPcm(self.rods_pcm));
            }
            if self.air == 0.0 {
                self.air = 0.8;
            }
            if ui.add(egui::Slider::new(&mut self.air, 0.01..=1.0).text("radiator air")).changed() {
                out.push(MCmd::AirFraction(self.air));
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("Servo on").clicked() {
                    out.push(MCmd::Servo(true));
                }
                if ui.button("Trip fuel pump").clicked() {
                    out.push(MCmd::TripFuelPump);
                }
                if ui.button("Start fuel pump").clicked() {
                    out.push(MCmd::StartFuelPump);
                }
            });
            out
        }
    }

    /// EBR-II's operator controls (GUI code).
    #[derive(Default)]
    pub struct Ebr2Controls {
        external_pcm: f64,
    }

    impl PlantControls<Ebr2Plant> for Ebr2Controls {
        fn controls(&mut self, ui: &mut egui::Ui, latest: Option<&ESnap>) -> Vec<ECmd> {
            let mut out = Vec::new();
            if let Some(s) = latest {
                ui.label(format!("t = {:.0} s · {:.2} MW · flow {:.0} kg/s", s.time_s, s.total_mw, s.primary_flow_kg_s));
            }
            if ui.add(egui::Slider::new(&mut self.external_pcm, -100.0..=20.0).text("external reactivity, pcm")).changed() {
                out.push(ECmd::ExternalPcm(self.external_pcm));
            }
            out
        }
    }

    /// The window around a host (GUI glue).
    struct Window<H: TwinHost> {
        app: TwinApp<H>,
    }

    impl<H: TwinHost> eframe::App for Window<H> {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.app.ui(ui);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// `--window WxH`: the window's inner size, px (a phone is 390x844).
    fn window_size() -> Option<[f32; 2]> {
        let args: Vec<String> = std::env::args().collect();
        let i = args.iter().position(|a| a == "--window")?;
        let (w, h) = args.get(i + 1)?.split_once('x')?;
        Some([w.parse().ok()?, h.parse().ok()?])
    }

    fn gui<H: TwinHost + 'static>(title: &str, host: H) -> Result<(), String> {
        let mut options = eframe::NativeOptions::default();
        if let Some(size) = window_size() {
            options.viewport = egui::ViewportBuilder::default().with_inner_size(size);
        }
        eframe::run_native(title, options, Box::new(|_| Ok(Box::new(Window { app: TwinApp::new(host) }))))
            .map_err(|e| e.to_string())
    }

    /// `main` (GUI and CLI glue).
    pub fn main() -> Result<(), String> {
        let args: Vec<String> = std::env::args().collect();
        let is_ebr2 = args.iter().any(|a| a == "--ebr2");
        let one = Time::new::<second>(1.0);
        let headless = args.iter().position(|a| a == "--headless");
        let text_only = headless.is_some() || args.iter().any(|a| a == "--dump-cvs" || a == "--coefficients");
        let end = |d: f64| match headless {
            Some(i) => args.get(i + 1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(d),
            None => d,
        };
        if is_ebr2 {
            let mut twin = ebr2::ebr2_twin(ECfg::default()).map_err(|e| e.to_string())?;
            if text_only {
                let mut a = args.clone();
                if headless.is_some() {
                    a = vec![a[0].clone(), "--seconds".into(), end(600.0).to_string()];
                }
                print!("{}", twin.headless(&a, &ebr2::demo_script(), one, Time::new::<second>(600.0)).map_err(|e| e.to_string())?);
                return Ok(());
            }
            let host = LocalHost::start(twin, Ebr2Controls::default(), "EBR-II twin".into(), Time::new::<second>(0.1), 10.0);
            return gui("EBR-II twin", host);
        }
        let mut twin = msre::msre_twin(MCfg::default(), msre::quick_split()).map_err(|e| e.to_string())?;
        if text_only {
            let mut a = args.clone();
            if headless.is_some() {
                a = vec![a[0].clone(), "--seconds".into(), end(600.0).to_string()];
            }
            print!("{}", twin.headless(&a, &msre::demo_script(), one, Time::new::<second>(600.0)).map_err(|e| e.to_string())?);
            return Ok(());
        }
        let host = LocalHost::start(twin, MsreControls::default(), "MSRE twin".into(), Time::new::<second>(0.1), 10.0);
        gui("MSRE twin", host)
    }
}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    native::main()
}
