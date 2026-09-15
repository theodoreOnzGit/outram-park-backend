//! GUI-facing plant state: the snapshot the physics thread writes and the
//! panels read, and the bounded plot-history ring buffers. Mirrors
//! `htgr_sim_v1::app::state`'s shape (`HtgrSnapshot`/`HtgrPlotData`) for a
//! column instead of a reactor.

/// Ring-buffer cap per plot series -- same bound `htgr_sim_v1` uses.
const MAX_PLOT_SAMPLES: usize = 4000;

/// The whole plant's state as the GUI sees it: operator setpoints (written by
/// the panels, read by the physics thread as [`crate::physics::PlantCommands`])
/// and computed per-stage/scalar outputs (written by
/// [`crate::physics::DistillationPlant::write_snapshot`]).
///
/// Plain `f64`/`usize`/`Vec<f64>` throughout, no `uom` -- this is the
/// GUI/physics-thread hand-off boundary, not a physics API, matching
/// `HtgrSnapshot`'s convention.
#[derive(Clone, Debug)]
pub struct ColumnSnapshot {
    // ---- Operator setpoints (written by the panels; read as commands) ----
    /// Reflux ratio `R = L_0 / D` \[-\], `> 0`.
    pub reflux_ratio: f64,
    /// Reboiler heat duty \[W\], `> 0`.
    pub reboiler_duty_watts: f64,

    // ---- Computed per-stage outputs, stage 0 = condenser .. N-1 = reboiler ----
    /// Number of equilibrium stages.
    pub n_stages: usize,
    /// Stage temperatures \[K\].
    pub stage_temperature_k: Vec<f64>,
    /// Liquid benzene (light-key) mole fraction per stage \[-\].
    pub liquid_benzene_fraction: Vec<f64>,
    /// Vapor benzene mole fraction per stage \[-\] (index 0 unused -- total
    /// condenser performs no vapor-liquid split).
    pub vapor_benzene_fraction: Vec<f64>,
    /// Liquid molar flow leaving each stage \[mol/s\] (index 0 is the reflux).
    pub liquid_flow_mol_s: Vec<f64>,
    /// Vapor molar flow leaving each stage \[mol/s\] (index 0 is always 0).
    pub vapor_flow_mol_s: Vec<f64>,
    /// Total molar holdup on each stage \[mol\].
    pub stage_holdup_mol: Vec<f64>,

    // ---- Computed scalar outputs ----
    /// Distillate product molar flow \[mol/s\].
    pub distillate_mol_s: f64,
    /// Bottoms product molar flow \[mol/s\].
    pub bottoms_mol_s: f64,

    // ---- Clock ----
    /// Accumulated plant simulation time \[s\] -- the TRUE plant clock, not
    /// wall-clock time (see `physics`'s module doc on time acceleration).
    pub sim_time_s: f64,
}

impl Default for ColumnSnapshot {
    /// The default operating point with **flat, unresolved profiles** (every
    /// stage temperature 0 K, every flow/holdup 0) -- deliberately not the
    /// converged startup values, because computing those needs a full
    /// [`crate::physics::DistillationPlant`] construction (a steady solve),
    /// which this `Default` impl must not do. This is a "first frame only"
    /// placeholder exactly like `HtgrSnapshot::default`'s documented
    /// convention: the physics thread overwrites every field within the
    /// first tick after launch.
    fn default() -> Self {
        let n = crate::physics::column_config::N_STAGES;
        Self {
            reflux_ratio: crate::physics::column_config::DEFAULT_REFLUX_RATIO,
            reboiler_duty_watts: 0.0,
            n_stages: n,
            stage_temperature_k: vec![0.0; n],
            liquid_benzene_fraction: vec![0.0; n],
            vapor_benzene_fraction: vec![0.0; n],
            liquid_flow_mol_s: vec![0.0; n],
            vapor_flow_mol_s: vec![0.0; n],
            stage_holdup_mol: vec![0.0; n],
            distillate_mol_s: 0.0,
            bottoms_mol_s: 0.0,
            sim_time_s: 0.0,
        }
    }
}

/// Bounded time-series history for the plots panel. Each series is
/// `[t_seconds, value]`, directly consumable by `egui_plot::PlotPoints`,
/// exactly like `HtgrPlotData`.
#[derive(Clone, Debug, Default)]
pub struct ColumnPlotData {
    /// Distillate benzene purity (light-key liquid mole fraction at stage 0)
    /// vs time.
    pub distillate_purity: Vec<[f64; 2]>,
    /// Bottoms benzene purity (light-key liquid mole fraction at the reboiler
    /// stage) vs time.
    pub bottoms_purity: Vec<[f64; 2]>,
    /// Reboiler-stage temperature vs time.
    pub reboiler_temperature_k: Vec<[f64; 2]>,
    /// Condenser-stage (stage 0) temperature vs time.
    pub condenser_temperature_k: Vec<[f64; 2]>,
    /// Reboiler heat duty vs time.
    pub reboiler_duty_watts: Vec<[f64; 2]>,
}

impl ColumnPlotData {
    /// Append one sample from `snapshot` to every series, trimming each to
    /// [`MAX_PLOT_SAMPLES`].
    pub fn push_sample(&mut self, snapshot: &ColumnSnapshot) {
        let t = snapshot.sim_time_s;
        let last = snapshot.n_stages.saturating_sub(1);
        push_capped(
            &mut self.distillate_purity,
            [
                t,
                snapshot
                    .liquid_benzene_fraction
                    .first()
                    .copied()
                    .unwrap_or(0.0),
            ],
        );
        push_capped(
            &mut self.bottoms_purity,
            [
                t,
                snapshot
                    .liquid_benzene_fraction
                    .get(last)
                    .copied()
                    .unwrap_or(0.0),
            ],
        );
        push_capped(
            &mut self.reboiler_temperature_k,
            [
                t,
                snapshot
                    .stage_temperature_k
                    .get(last)
                    .copied()
                    .unwrap_or(0.0),
            ],
        );
        push_capped(
            &mut self.condenser_temperature_k,
            [
                t,
                snapshot.stage_temperature_k.first().copied().unwrap_or(0.0),
            ],
        );
        push_capped(
            &mut self.reboiler_duty_watts,
            [t, snapshot.reboiler_duty_watts],
        );
    }
}

fn push_capped(buf: &mut Vec<[f64; 2]>, sample: [f64; 2]) {
    if buf.len() >= MAX_PLOT_SAMPLES {
        buf.remove(0);
    }
    buf.push(sample);
}
