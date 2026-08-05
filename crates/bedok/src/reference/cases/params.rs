//! The `params` struct a case constructor takes in and hands back, and the
//! transient/kinetics data the transient cases attach to it.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source files | the `params` fields set by `main_exec_diff3d.m`, `run_neacrpd1t.m`, `iaea3ds.m`, `neacrpa2.m`, `neacrpa2t.m`, `neacrpa1t.m`, `neacrpd1.m`, `neacrpd1t.m` |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//!
//! # The in-out convention, and why it matters
//!
//! Every MATLAB case constructor has the signature
//! `[params, …] = case(params)`: the caller passes a user set-up struct and
//! gets a *modified* one back. Some cases overwrite the grid the caller asked
//! for — `iaea3ds.m` forces 17 × 17 × **19**, `neacrpd1.m` forces
//! 17 × 17 × **14** — so **the grid must always be read back from the returned
//! params, never from what was requested.** The Rust constructors keep that
//! shape (`&CaseParams` in, a new [`CaseParams`] out) precisely so the mistake
//! is hard to make.

use crate::reference::grid::Grid;

/// Which thermal-hydraulic model the coupled solver should run.
///
/// MATLAB `params.th_model`, a string, set only by `neacrpd1t.m`
/// (`params.th_model='hem'`). Absent means "the case's default path".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalHydraulicModel {
    /// Homogeneous-equilibrium enthalpy march
    /// (`singleflow1devap` / `singleflow1devaptime`). MATLAB `'hem'`.
    ///
    /// `neacrpd1t.m` selects it for both the steady state and the transient so
    /// that the two use the *same* model: seeding the HEM transient from the
    /// two-fluid steady solver would hand it a slip-void density mismatch,
    /// i.e. a spurious reactivity step at `t = 0`.
    HomogeneousEquilibrium,
}

/// Radial discretisation of the fuel pin, for the 1-D cylindrical conduction
/// model.
///
/// MATLAB `params.fuel`. Node counts, not lengths — the radii live in
/// [`FuelGeometry`](super::fuel::FuelGeometry).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuelDiscretisation {
    /// Radial nodes across the pellet–clad gap. MATLAB `params.fuel.gapn`;
    /// `1` in every case.
    pub gap_nodes: usize,
    /// Radial nodes across the cladding. MATLAB `params.fuel.cladn`; `1` in
    /// every case.
    pub clad_nodes: usize,
    /// Radial nodes across the fuel pellet. MATLAB `params.fuel.fueln`; `20`
    /// in every case.
    pub fuel_nodes: usize,
    /// Total radial nodes, `fuel + gap + clad`. MATLAB `params.fuel.maxir`.
    pub total_nodes: usize,
}

impl FuelDiscretisation {
    /// The discretisation both NEACRP cases use: 20 pellet nodes, 1 gap node,
    /// 1 cladding node.
    #[must_use]
    pub const fn neacrp_default() -> Self {
        Self {
            gap_nodes: 1,
            clad_nodes: 1,
            fuel_nodes: 20,
            total_nodes: 22,
        }
    }
}

/// Prompt-neutron speeds and delayed-neutron precursor data for the transient
/// solver.
///
/// MATLAB `params.velocities`, `params.beta_dnp`, `params.lambda_dnp`, set by
/// `neacrpa2t.m`, `neacrpa1t.m` and `neacrpd1t.m`. Absent in the steady cases.
#[derive(Debug, Clone, PartialEq)]
pub struct KineticsData {
    /// Prompt neutron speed per energy group \[cm/s\]. MATLAB
    /// `params.velocities`.
    ///
    /// The PWR cases give speeds directly (`0.28e8`, `0.44e6`); the BWR case
    /// gives them as reciprocals of the Table 5.1 inverse velocities
    /// (`1/3.57e-8`, `1/2.27e-6`).
    pub velocities: Vec<f64>,
    /// Delayed-neutron fraction per precursor group \[dimensionless\]. MATLAB
    /// `params.beta_dnp`. Six groups, summing to 0.0076.
    pub beta: Vec<f64>,
    /// Precursor decay constant per group \[1/s\]. MATLAB `params.lambda_dnp`.
    pub lambda: Vec<f64>,
}

impl KineticsData {
    /// Total delayed-neutron fraction, `sum(beta)` \[dimensionless\].
    ///
    /// Both NEACRP data sets are specified to total 0.76 %.
    #[must_use]
    pub fn total_beta(&self) -> f64 {
        self.beta.iter().sum()
    }
}

/// A control-rod bank ejection, the forcing of the two PWR transient cases.
///
/// MATLAB splits this across two structs: `geometry.crodeject` (which bank),
/// `geometry.crodejectto` (final position) and `params.ejectduration` (how
/// long). It is kept together here; the doc comments name both origins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RodEjection {
    /// Index into `geometry.crod` of the ejected bank, **1-based** as in the
    /// MATLAB. MATLAB `geometry.crodeject`; `1` (the central CA) in both PWR
    /// transients.
    ///
    /// `neacrpd1t.m` sets `geometry.crodeject = 0` meaning *no rod motion*;
    /// that case is represented by `rod_ejection: None` rather than by a zero
    /// here.
    pub bank: usize,
    /// Final bank position \[withdrawal steps\]. MATLAB
    /// `geometry.crodejectto`; `228` = fully withdrawn.
    pub target_steps: f64,
    /// Ejection time \[s\]. MATLAB `params.ejectduration`; `0.1` s, and the
    /// benchmark states it is independent of insertion depth.
    pub duration: f64,
}

/// The time window and output grid of a transient run.
///
/// MATLAB `params.tend`, `params.tgrid`, `params.outprefix`, plus the rod
/// motion. Present only on the transient cases.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientSchedule {
    /// End of the transient \[s\]. MATLAB `params.tend`.
    pub t_end: f64,
    /// Output/step times \[s\], ascending, starting at 0. MATLAB
    /// `params.tgrid`.
    ///
    /// Built in the MATLAB by concatenating uniform ranges, which **repeats
    /// the junction times** — e.g. `[0:0.0025:0.2, 0.2:0.01:1, …]` contains
    /// `0.2` twice. The duplicates are reproduced here rather than removed:
    /// they are what the reference solver actually steps over. See
    /// [`has_duplicate_times`](Self::has_duplicate_times).
    pub time_grid: Vec<f64>,
    /// Prefix for the solver's history CSVs. MATLAB `params.outprefix`.
    pub output_prefix: String,
    /// The control-rod ejection driving the transient, if any. `None` for the
    /// BWR cold-water-injection case, whose forcing is a coolant inlet
    /// condition instead.
    pub rod_ejection: Option<RodEjection>,
}

impl TransientSchedule {
    /// Whether the time grid repeats a time, as the MATLAB concatenations do.
    ///
    /// Reported rather than repaired — see `docs/bedok-port-scoping.md` §1.0.
    /// A repeated time means the solver takes a zero-length step there.
    #[must_use]
    pub fn has_duplicate_times(&self) -> bool {
        self.time_grid.windows(2).any(|w| w[0] == w[1])
    }
}

/// Build the MATLAB expression `a:step:b` — a closed range that stops at or
/// before `b`.
///
/// Used by the transient cases to reproduce `params.tgrid` exactly. Note
/// MATLAB's colon operator accumulates as `a + k*step`, which is what is done
/// here, so the rounding matches.
///
/// # Panics
///
/// If `step` is not strictly positive.
#[must_use]
pub fn colon(a: f64, step: f64, b: f64) -> Vec<f64> {
    assert!(step > 0.0, "colon step must be positive");
    let n = ((b - a) / step).floor() as i64;
    (0..=n.max(-1)).map(|k| a + (k as f64) * step).collect()
}

/// The `params` struct: solver controls, grid shape, and the state feedback
/// variables' initial averages.
///
/// # Units
///
/// Temperatures are kelvin, boron is ppm by weight, coolant density is
/// g/cm³ — the units the NEACRP cross-section feedback tables are expressed
/// against.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseParams {
    /// Node counts and energy-group count: MATLAB `params.maxix`,
    /// `params.maxiy`, `params.maxiz` and `params.G` in one place.
    ///
    /// **Read this back from the constructed case.** Three of the four case
    /// constructors overwrite what the caller asked for.
    pub grid: Grid,
    /// Maximum outer (power-iteration) cycles. MATLAB
    /// `params.max_num_cycles`.
    pub max_num_cycles: usize,
    /// Cycles between semi-analytic nodal corrections; `0` selects the
    /// solver's default. MATLAB `params.nodalupd`.
    pub nodal_update: usize,
    /// Force a stop after this many cycles; `0` disables. MATLAB
    /// `params.stop`.
    pub stop: usize,
    /// Verbosity level. MATLAB `params.verb`.
    pub verbosity: u8,
    /// Whether the driver draws figures. MATLAB `params.plotfig` (0/1).
    pub plot_figures: bool,
    /// Whether the driver draws the 3-D power plot. MATLAB `params.plot3d`.
    pub plot_3d: bool,
    /// Whether the solver dumps debug state. MATLAB `params.debugdump`.
    pub debug_dump: bool,
    /// Number of extra (precursor) unknowns carried alongside the flux per
    /// node. MATLAB `params.Nc`; **`0` in every case in the snapshot**, so the
    /// `Nc /= 0` branches of the index utilities are untested dead code.
    pub num_extra_unknowns: usize,
    /// Prompt fission fraction \[dimensionless\]. MATLAB `params.frac_p`; `1`
    /// wherever it is set (`iaea3ds.m`, `geom2dxycase1.m`).
    ///
    /// The NEACRP cases never set it — `params.frac_p` is simply absent there,
    /// represented as `None`.
    pub prompt_fraction: Option<f64>,
    /// Whether the solver preconditions its JFNK iterations. MATLAB
    /// `params.jfnkprecon`.
    pub jfnk_preconditioner: bool,
    /// JFNK under-relaxation factor \[dimensionless\]. MATLAB
    /// `params.jfnkrel`.
    pub jfnk_relaxation: f64,
    /// JFNK verbosity. MATLAB `params.jfnkverb`.
    pub jfnk_verbosity: u8,
    /// Boron concentration \[ppm\]. MATLAB `params.boron`.
    pub boron_ppm: Option<f64>,
    /// Core-average fuel temperature used to initialise the Doppler feedback
    /// \[K\]. MATLAB `params.fueltempavg`.
    pub fuel_temperature_average: Option<f64>,
    /// Core-average coolant temperature used to initialise the moderator
    /// temperature feedback \[K\]. MATLAB `params.cooltempavg`.
    pub coolant_temperature_average: Option<f64>,
    /// Core-average coolant density used to initialise the density feedback
    /// \[g/cm³\]. MATLAB `params.cooldenavg`.
    pub coolant_density_average: Option<f64>,
    /// Fuel-pin radial discretisation, when the case runs thermal hydraulics.
    /// MATLAB `params.fuel`.
    pub fuel: Option<FuelDiscretisation>,
    /// Transient window, time grid and rod motion. MATLAB `params.tend` /
    /// `tgrid` / `outprefix` (+ the ejection fields). `None` for a steady case.
    pub transient: Option<TransientSchedule>,
    /// Prompt velocities and delayed-neutron data. MATLAB
    /// `params.velocities` / `beta_dnp` / `lambda_dnp`. `None` for a steady
    /// case.
    pub kinetics: Option<KineticsData>,
    /// Thermal-hydraulic model override. MATLAB `params.th_model`.
    pub thermal_hydraulic_model: Option<ThermalHydraulicModel>,
    /// Path the transient driver caches its converged steady state in. MATLAB
    /// `params.steadyfile`, set by `run_neacrpd1t.m`.
    pub steady_state_file: Option<String>,
}

impl CaseParams {
    /// The user set-up block of `main_exec_diff3d.m`, verbatim.
    ///
    /// 17 × 17 × 18 nodes, 2 energy groups, 150 outer cycles. Note that three
    /// of the four case constructors overwrite the axial node count.
    ///
    /// # Panics
    ///
    /// Never — the dimensions are non-zero constants.
    #[must_use]
    pub fn main_exec_defaults() -> Self {
        Self {
            // main_exec_diff3d.m does not set params.G; every 3-D case
            // constructor sets it to 2, so 2 is used here as the placeholder
            // the constructors will confirm.
            grid: Grid::new(17, 17, 18, 2).expect("17x17x18, 2 groups is a valid grid"),
            max_num_cycles: 150,
            nodal_update: 0,
            stop: 0,
            verbosity: 1,
            plot_figures: true,
            plot_3d: true,
            debug_dump: false,
            num_extra_unknowns: 0,
            prompt_fraction: None,
            jfnk_preconditioner: true,
            jfnk_relaxation: 0.5,
            jfnk_verbosity: 1,
            boron_ppm: None,
            fuel_temperature_average: None,
            coolant_temperature_average: None,
            coolant_density_average: None,
            fuel: None,
            transient: None,
            kinetics: None,
            thermal_hydraulic_model: None,
            steady_state_file: None,
        }
    }

    /// The user set-up block of `run_neacrpd1t.m`.
    ///
    /// Same as [`main_exec_defaults`](Self::main_exec_defaults) except that
    /// plotting is off, JFNK output is silenced, and a steady-state cache file
    /// is named. It still requests `maxiz = 18`, which `neacrpd1.m` then
    /// overwrites with 14.
    #[must_use]
    pub fn run_neacrpd1t_defaults() -> Self {
        Self {
            plot_figures: false,
            plot_3d: false,
            jfnk_verbosity: 0,
            steady_state_file: Some("neacrpd1t_steady.mat".to_string()),
            ..Self::main_exec_defaults()
        }
    }

    /// Node counts `(maxix, maxiy, maxiz)`. MATLAB `handle3dcoords(params)`.
    ///
    /// The MATLAB helper dispatches on which fields exist — `(maxir, maxitheta,
    /// maxiz)` for cylindrical, then `(maxix, maxiy, maxiz)`, then the generic
    /// `(maxi1, maxi2, maxi3)` — defaulting to `1,1,1` if none match. Only the
    /// Cartesian form occurs in the ported cases, so this is the identity.
    ///
    /// **Unfinished in the reference:** the generic branch of
    /// `handle3dcoords.m` reads `maxi3 = params.maxix`, plainly a typo for
    /// `params.maxi3`. Recorded, not fixed — no ported case reaches it.
    #[must_use]
    pub const fn coords_3d(&self) -> (usize, usize, usize) {
        (self.grid.nx, self.grid.ny, self.grid.nz)
    }

    /// Node counts `(maxi1, maxi2)` for a 2-D case. MATLAB
    /// `handle2dcoords(params)`.
    ///
    /// Same dispatch as [`coords_3d`](Self::coords_3d) minus the third axis:
    /// `(maxir, maxiz)`, then `(maxix, maxiy)`, then `(maxi1, maxi2)`.
    ///
    /// **Unfinished in the reference:** `handle2dcoords.m` has no default, so a
    /// `params` matching none of the three shapes leaves both outputs
    /// undefined and MATLAB errors. Recorded, not fixed.
    #[must_use]
    pub const fn coords_2d(&self) -> (usize, usize) {
        (self.grid.nx, self.grid.ny)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_exec_defaults_request_eighteen_axial_nodes() {
        let p = CaseParams::main_exec_defaults();
        assert_eq!(p.coords_3d(), (17, 17, 18));
        assert_eq!(p.grid.ngroups, 2);
        assert_eq!(p.max_num_cycles, 150);
    }

    #[test]
    fn colon_reproduces_the_matlab_range() {
        assert_eq!(colon(0.0, 0.5, 2.0), vec![0.0, 0.5, 1.0, 1.5, 2.0]);
        // Stops at or before the endpoint.
        let v = colon(0.0, 0.3, 1.0);
        assert_eq!(v.len(), 4);
        assert!((v[3] - 0.9).abs() < 1e-15);
    }

    #[test]
    fn kinetics_beta_totals_the_specified_fraction() {
        let k = KineticsData {
            velocities: vec![0.28e8, 0.44e6],
            beta: vec![0.034, 0.200, 0.183, 0.404, 0.145, 0.034]
                .into_iter()
                .map(|f| 0.0076 * f)
                .collect(),
            lambda: vec![0.0128, 0.0318, 0.1190, 0.3181, 1.4027, 3.9286],
        };
        assert!((k.total_beta() - 0.0076).abs() < 1e-12);
    }
}
