//! Thermal hydraulics — faithful translation of Than Yan Ren's MATLAB.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Translated from the handed-over MATLAB snapshot
//! `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`, received 2026-08-05.
//! Permission to translate and to publish as open source under OUTRAM PARK was
//! given by the author and approved at project-lead level; see
//! `docs/bedok-port-scoping.md` §6.
//!
//! # What is in here
//!
//! | Rust module | MATLAB source |
//! |---|---|
//! | [`solver`] | `th_solverxyz.m` |
//! | [`solver_time`] | `th_solvertimexyz.m` |
//! | [`single_flow_evap`] | `singleflow1devap.m` |
//! | [`single_flow_evap_time`] | `singleflow1devaptime.m` |
//! | [`drift_flux_3d`] | `driftflux6_solverstatic3d.m` |
//! | [`fuel_rod`] | `fuelrodheat_1dcylnd.m` |
//! | [`fuel_rod_time`] | `fuelrodheattime_1dcylnd.m` |
//! | [`w3_chf`] | `w3chf.m`, `w3chfhottest.m` |
//! | [`steam`] | *substituted* — `tampines-steam-tables`, not `IAPWS_IF97.m` |
//! | [`linalg`] | *supporting* — replaces MATLAB's sparse `\` |
//!
//! # Units
//!
//! The MATLAB mixes unit systems and this translation keeps them **exactly as
//! they are**, because changing them would change the floating-point
//! arithmetic. Every public item states its units explicitly. The conventions
//! throughout are:
//!
//! - length **cm**, area **cm²**, volume **cm³**
//! - temperature **K** (never °C)
//! - pressure **MPa**
//! - specific enthalpy **kJ/kg** (= J/g)
//! - density **g/cm³**, mass flux **g/(s·cm²)**, velocity **cm/s**
//! - power **W**, linear power density **W/cm**, volumetric power **W/cm³**
//! - heat flux **W/cm²**, heat transfer coefficient **W/(cm²·K)**
//! - thermal conductivity **W/(cm·K)**, gap conductance **W/(cm²·K)**
//! - volumetric heat capacity **J/(cm³·K)**, kinematic viscosity **cm²/s**
//!
//! # Known gaps in the upstream snapshot
//!
//! Recorded here rather than repaired, per `docs/bedok-port-scoping.md` §1.0.
//! Each is also flagged at the point in the code where it occurs.
//!
//! 1. **`driftflux6_solverstatic1d.m` is missing from the snapshot.**
//!    `driftflux6_solverstatic3d.m` calls it at its line 157 and nothing else
//!    in the snapshot defines it. See [`drift_flux_3d`]. The single-phase
//!    homogeneous-equilibrium path ([`single_flow_evap`]) is complete and is
//!    what the benchmark cases actually exercise.
//! 2. **`fuelrodheat_1dcylnd` indexes past its own matrix** for any rod layout
//!    with no material→gap transition (e.g. an all-fuel rod). See
//!    [`fuel_rod::solve_static`].
//! 3. **A material→material interface with no gap between them assembles no
//!    conduction coefficient.** See [`fuel_rod::solve_static`].
//! 4. **The gap ring becomes an orphan row** fixed at `T = 1 K`. See
//!    [`fuel_rod::solve_static`].
//! 5. **`w3chfhottest.m` sets `highy = ix`** where it means `iy`. See
//!    [`w3_chf::hottest_channel`].
//! 6. **`w3chf.m`'s `enthshift` is not the inlet enthalpy** the W-3
//!    correlation calls for, and carries a stray factor of ½. See
//!    [`w3_chf::critical_heat_flux`].
//!
//! # Verification status
//!
//! **Unverified against the reference.** The MATLAB was not run (there is no
//! MATLAB or Octave on the build machine, and the snapshot ships no golden
//! outputs), so nothing here may be described as "reproducing Yan Ren's
//! results". The unit tests in this module check internal consistency and
//! hand-worked correlation values only.

use crate::reference::grid::Grid;
use thiserror::Error;

pub mod drift_flux_3d;
pub mod fuel_rod;
pub mod fuel_rod_time;
pub mod linalg;
pub mod single_flow_evap;
pub mod single_flow_evap_time;
pub mod solver;
pub mod solver_time;
pub mod steam;
pub mod w3_chf;

/// Result alias for the thermal-hydraulics reference translation.
pub type ThResult<T> = std::result::Result<T, ThError>;

/// Everything the ported thermal hydraulics can fail with.
///
/// This is deliberately a module-local error type rather than a new
/// [`crate::BedokError`] variant: `src/error.rs` is outside this module's
/// ownership. A `From<ThError> for BedokError` bridge should be added when the
/// coupling layer lands.
#[derive(Debug, Error)]
pub enum ThError {
    /// A field came out of the solve containing NaN.
    ///
    /// This is the translation of the MATLAB `pauseonnan` helper, which calls
    /// `error('NaN occured')`. The MATLAB also rejects complex values; that
    /// check has no Rust counterpart because every quantity here is `f64`.
    #[error("{field} contains NaN at flat node index {index} (MATLAB pauseonnan)")]
    NotANumber {
        /// Which field tripped the check.
        field: &'static str,
        /// Flat spatial-node index of the first offending entry.
        index: usize,
    },

    /// A MATLAB source file the snapshot depends on was not in the snapshot.
    ///
    /// Yan Ren handed the code over unfinished; this is not a translation
    /// oversight. Nothing is invented to fill the gap.
    #[error(
        "MATLAB source `{missing}` is absent from the handed-over BEDOK snapshot \
         (sha256 e45cd6f57be2087c…) but is called by `{caller}`; it was never \
         written, so there is nothing to translate"
    )]
    MissingUpstreamSource {
        /// The `.m` file that does not exist.
        missing: &'static str,
        /// The `.m` file that calls it.
        caller: &'static str,
    },

    /// An input vector was not the length the grid implies.
    #[error("{what}: expected length {expected}, got {got}")]
    LengthMismatch {
        /// Which input.
        what: &'static str,
        /// Length implied by the grid.
        expected: usize,
        /// Length supplied.
        got: usize,
    },

    /// A linear system could not be factorised.
    #[error("{what}: matrix is singular at pivot {pivot}")]
    SingularMatrix {
        /// Which solve.
        what: &'static str,
        /// Zero pivot position (0-based).
        pivot: usize,
    },

    /// A fuel-rod radial layout the upstream MATLAB cannot assemble.
    ///
    /// Not a Rust limitation — see [`fuel_rod::solve_static`] for the exact
    /// out-of-range write in the original.
    #[error(
        "fuel-rod radial layout is not assemblable by the upstream MATLAB: {reason}. \
         This is an unfinished-code gap in the snapshot, not a translation limitation"
    )]
    UnsupportedRodLayout {
        /// What about the layout the MATLAB cannot handle.
        reason: &'static str,
    },
}

/// Direction of coolant flow along the channel. MATLAB `th.flowdir`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowDirection {
    /// MATLAB `th.flowdir = 1` — inlet at the bottom of the channel.
    Upward,
    /// MATLAB `th.flowdir = -1` — inlet at the top of the channel.
    Downward,
}

/// Which channel model [`solver::solve_static`] dispatches to.
///
/// MATLAB `params.th_model`. The default in `th_solverxyz.m` is the two-fluid
/// path; `'hem'` selects the homogeneous-equilibrium enthalpy march. The
/// MATLAB comment records *why* the choice matters: `th_solvertimexyz` always
/// marches the HEM model, so a transient run needs its `t = 0` steady state
/// from the same model, or the density mismatch injects a spurious reactivity
/// step at `t = 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelModel {
    /// MATLAB default — `driftflux6_solverstatic3d`. **Not usable**: its
    /// single-channel kernel is absent from the snapshot, see
    /// [`drift_flux_3d`].
    TwoFluid,
    /// MATLAB `params.th_model = 'hem'` — `singleflow1devap`. Complete, and
    /// what the benchmark cases use.
    HomogeneousEquilibrium,
}

/// Material class of one radial ring of the fuel-pin conduction mesh.
///
/// MATLAB `geometry.fuel.whichk`, which stores `1` for fuel, `0` for the
/// gas gap and `2` for cladding, and uses that value to index the `tcon` cell
/// array of conductivity function handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RodMaterial {
    /// UO₂ fuel pellet. MATLAB `whichk == 1`.
    Fuel,
    /// Fuel-cladding gas gap. MATLAB `whichk == 0`. Carries a *conductance*
    /// (W/(cm²·K)), not a conductivity, and no heat capacity.
    Gap,
    /// Cladding. MATLAB `whichk == 2`.
    Clad,
}

impl RodMaterial {
    /// The MATLAB `whichk` integer for this material.
    #[must_use]
    pub const fn matlab_which_k(self) -> usize {
        match self {
            Self::Gap => 0,
            Self::Fuel => 1,
            Self::Clad => 2,
        }
    }

    /// Whether this ring generates fission power. MATLAB `whichf = (whichk == 1)`.
    #[must_use]
    pub const fn is_fuel(self) -> bool {
        matches!(self, Self::Fuel)
    }
}

/// Temperature-dependent thermal conductivity of a rod material, in W/(cm·K).
///
/// The MATLAB stores these as anonymous function handles in
/// `geometry.fuel.tcon{...}`. Workspace rules forbid trait objects and boxed
/// closures, so the closed set of correlations the benchmark cases use is an
/// enum instead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalConductivityModel {
    /// UO₂ as used by the NEACRP cases:
    /// `k(T) = (1.05 + 2150/(T - 73.15))/100` W/(cm·K), `T` in K.
    ///
    /// Valid for solid UO₂ roughly 300–3000 K. Singular at `T = 73.15 K`,
    /// which no reactor state reaches.
    Uo2Neacrp,
    /// Zircaloy cladding as used by the NEACRP cases:
    /// `k(T) = (7.51 + 2.09e-2 T - 1.45e-5 T² + 7.67e-9 T³)/100` W/(cm·K),
    /// `T` in K. Valid roughly 300–1500 K.
    ZircaloyNeacrp,
    /// A temperature-independent conductivity in W/(cm·K).
    Constant(f64),
}

impl ThermalConductivityModel {
    /// Thermal conductivity in W/(cm·K) at temperature `temperature_kelvin` \[K\].
    #[must_use]
    pub fn evaluate(&self, temperature_kelvin: f64) -> f64 {
        match *self {
            Self::Uo2Neacrp => (1.05 + 2150.0 / (temperature_kelvin - 73.15)) / 100.0,
            Self::ZircaloyNeacrp => {
                let t = temperature_kelvin;
                (7.51 + 2.09e-2 * t - 1.45e-5 * t * t + 7.67e-9 * t * t * t) / 100.0
            }
            Self::Constant(k) => k,
        }
    }
}

/// Temperature-dependent volumetric heat capacity `rho*cp`, in J/(cm³·K).
///
/// MATLAB `geometry.fuel.rhocp{...}`, used only by the transient rod solver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VolumetricHeatCapacityModel {
    /// UO₂ as used by `neacrpa2t.m`:
    /// `10.412*(1 - 0.01248)*(162.3 + 0.3038 T - 2.391e-4 T² + 6.404e-8 T³)/1000`
    /// J/(cm³·K), `T` in K. The leading factor is the theoretical density
    /// (g/cm³) reduced for porosity.
    Uo2Neacrp,
    /// Zircaloy as used by `neacrpa2t.m`:
    /// `6.6*(252.54 + 0.11474 T)/1000` J/(cm³·K), `T` in K.
    ZircaloyNeacrp,
    /// A temperature-independent volumetric heat capacity in J/(cm³·K).
    Constant(f64),
}

impl VolumetricHeatCapacityModel {
    /// Volumetric heat capacity in J/(cm³·K) at `temperature_kelvin` \[K\].
    #[must_use]
    pub fn evaluate(&self, temperature_kelvin: f64) -> f64 {
        match *self {
            Self::Uo2Neacrp => {
                let t = temperature_kelvin;
                10.412
                    * (1.0 - 0.01248)
                    * (162.3 + 0.3038 * t - 2.391e-4 * t * t + 6.404e-8 * t * t * t)
                    / 1000.0
            }
            Self::ZircaloyNeacrp => 6.6 * (252.54 + 0.11474 * temperature_kelvin) / 1000.0,
            Self::Constant(c) => c,
        }
    }
}

/// Radial node counts of the fuel-pin conduction mesh. MATLAB `params.fuel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuelRodParams {
    /// Radial rings inside the pellet. MATLAB `params.fuel.fueln`.
    pub fuel_rings: usize,
    /// Radial rings across the gas gap. MATLAB `params.fuel.gapn`.
    pub gap_rings: usize,
    /// Radial rings across the cladding. MATLAB `params.fuel.cladn`.
    pub clad_rings: usize,
    /// Total rings, `fuel_rings + gap_rings + clad_rings`.
    /// MATLAB `params.fuel.maxir`.
    pub max_ir: usize,
}

impl FuelRodParams {
    /// The conventional layout: `fuel_rings` pellet rings, then the gap, then
    /// the cladding, with `max_ir` set consistently.
    #[must_use]
    pub const fn new(fuel_rings: usize, gap_rings: usize, clad_rings: usize) -> Self {
        Self {
            fuel_rings,
            gap_rings,
            clad_rings,
            max_ir: fuel_rings + gap_rings + clad_rings,
        }
    }
}

/// Number of *solution* nodes in the rod-conduction matrix, MATLAB `maxid`.
///
/// The conduction mesh inserts one extra node at every material↔gap interface,
/// so `maxid = maxir + surfcount` where `surfcount` counts transitions between
/// a conducting ring and the gap in either direction. Translated from the
/// identical loop that appears in `fuelrodheat_1dcylnd.m`,
/// `fuelrodheattime_1dcylnd.m` and `thdiffusion_solverxyz.m`.
///
/// For the NEACRP layout (20 fuel rings, 1 gap, 1 clad) this is `22 + 2 = 24`.
#[must_use]
pub fn radial_solution_nodes(which_k: &[RodMaterial]) -> usize {
    let mut surface_count = 0usize;
    for window in which_k.windows(2) {
        let here_gap = matches!(window[0], RodMaterial::Gap);
        let next_gap = matches!(window[1], RodMaterial::Gap);
        if here_gap != next_gap {
            surface_count += 1;
        }
    }
    which_k.len() + surface_count
}

/// Radial geometry and material properties of one fuel pin.
/// MATLAB `geometry.fuel`.
///
/// All lengths in cm. The rod is treated as radially one-dimensional; the
/// integrated heat equation is divided through by `2*pi`, which is why the
/// "volumes" below are per unit rod length.
#[derive(Debug, Clone, PartialEq)]
pub struct FuelRodGeometry {
    /// Pellet outer radius \[cm\]. MATLAB `geometry.fuel.fuelrad`.
    pub fuel_radius: f64,
    /// Radial gap thickness \[cm\]. MATLAB `geometry.fuel.fuelgap`.
    pub gap_thickness: f64,
    /// Cladding thickness \[cm\]. MATLAB `geometry.fuel.clad`.
    pub clad_thickness: f64,
    /// Rod outer radius \[cm\], `fuel_radius + gap_thickness + clad_thickness`.
    /// MATLAB `geometry.fuel.Rtot`.
    pub outer_radius: f64,
    /// Square lattice pitch \[cm\]. MATLAB `geometry.fuel.pitch`.
    pub pitch: f64,
    /// Doppler weighting `alpha` \[-\], in `T_doppler = (1-alpha) T_centre +
    /// alpha T_surface`. Typically 0.7. MATLAB `geometry.fuel.doppleralpha`.
    pub doppler_alpha: f64,
    /// Radial thickness of each ring \[cm\], length `max_ir`.
    /// MATLAB `geometry.fuel.Lr`.
    pub ring_thickness: Vec<f64>,
    /// Radius of each ring's centre \[cm\], length `max_ir`.
    /// MATLAB `geometry.fuel.Ctr`.
    pub ring_centre_radius: Vec<f64>,
    /// Cross-sectional area of each ring \[cm²\] (MATLAB calls it a volume,
    /// `geometry.fuel.Vi`, because the rod is per unit length).
    pub ring_area: Vec<f64>,
    /// Material of each ring, length `max_ir`. MATLAB `geometry.fuel.whichk`.
    pub which_k: Vec<RodMaterial>,
    /// Coolant flow area per pin \[cm²\], `pitch² - pi*outer_radius²`.
    /// MATLAB `geometry.fuel.subarea`.
    pub subchannel_area: f64,
    /// Subchannel hydraulic diameter \[cm\]. MATLAB `geometry.fuel.hydia`.
    pub hydraulic_diameter: f64,
    /// Pellet conductivity. MATLAB `geometry.fuel.tcon{1}`.
    pub fuel_conductivity: ThermalConductivityModel,
    /// Cladding conductivity. MATLAB `geometry.fuel.tcon{2}`.
    pub clad_conductivity: ThermalConductivityModel,
    /// Gap **conductance** \[W/(cm²·K)\], not a conductivity.
    /// MATLAB `geometry.fuel.tcon{end}` (the NEACRP benchmark value is 1.0).
    pub gap_conductance: f64,
    /// Pellet volumetric heat capacity. MATLAB `geometry.fuel.rhocp{1}`.
    /// Transient solve only.
    pub fuel_heat_capacity: VolumetricHeatCapacityModel,
    /// Cladding volumetric heat capacity. MATLAB `geometry.fuel.rhocp{2}`.
    /// Transient solve only.
    pub clad_heat_capacity: VolumetricHeatCapacityModel,
}

impl FuelRodGeometry {
    /// Conductivity model of a conducting ring.
    ///
    /// Returns `None` for [`RodMaterial::Gap`], which carries a conductance
    /// rather than a conductivity — mirroring the MATLAB, where
    /// `tcon{whichk(ir)}` with `whichk == 0` would be an invalid cell index and
    /// the gap is special-cased instead.
    #[must_use]
    pub fn conductivity(&self, material: RodMaterial) -> Option<ThermalConductivityModel> {
        match material {
            RodMaterial::Fuel => Some(self.fuel_conductivity),
            RodMaterial::Clad => Some(self.clad_conductivity),
            RodMaterial::Gap => None,
        }
    }

    /// Volumetric heat capacity model of a conducting ring, J/(cm³·K).
    ///
    /// `None` for the gap, which carries no heat capacity in the MATLAB.
    #[must_use]
    pub fn heat_capacity(&self, material: RodMaterial) -> Option<VolumetricHeatCapacityModel> {
        match material {
            RodMaterial::Fuel => Some(self.fuel_heat_capacity),
            RodMaterial::Clad => Some(self.clad_heat_capacity),
            RodMaterial::Gap => None,
        }
    }

    /// Cumulative outer radius of ring `ring` \[cm\], 0-based.
    /// MATLAB `sumLr(ir) = sum(Lr(1:ir))`.
    #[must_use]
    pub fn cumulative_radius(&self, ring: usize) -> f64 {
        self.ring_thickness[..=ring].iter().sum()
    }
}

/// Axial geometry the thermal hydraulics reads. MATLAB `geometry` (the parts
/// `th_solverxyz` and friends touch).
///
/// # Note on `axial_height`
///
/// MATLAB `geometry.Lz` is a **full state-length column vector** — one entry
/// per spatial node, not one per `iz`. `neacrpa2.m:43` builds it as
/// `zeros(maxix*maxiy*maxiz,1)` and `driftflux6_solverstatic3d.m:63` reshapes
/// it to `(maxiz, nch)`. This struct keeps that shape.
#[derive(Debug, Clone, PartialEq)]
pub struct ThGeometry {
    /// Axial node height \[cm\], one per spatial node (`grid.nodes()` long).
    /// MATLAB `geometry.Lz`.
    pub axial_height: Vec<f64>,
    /// Lowest active axial node of each `(ix, iy)` channel, **0-based and
    /// inclusive**. MATLAB `geometry.zlows`, which is 1-based; the conversion
    /// happens once, here. Indexed `ix*ny + iy`.
    pub z_low: Vec<usize>,
    /// Highest active axial node of each channel, 0-based inclusive.
    /// MATLAB `geometry.zhis`. Indexed `ix*ny + iy`.
    pub z_high: Vec<usize>,
    /// Fuel-pin radial geometry. MATLAB `geometry.fuel`.
    pub fuel: FuelRodGeometry,
}

impl ThGeometry {
    /// Index into [`z_low`](Self::z_low) / [`z_high`](Self::z_high) for the
    /// channel at 0-based `(ix, iy)`.
    #[must_use]
    pub const fn channel_index(grid: &Grid, ix: usize, iy: usize) -> usize {
        ix * grid.ny + iy
    }

    /// A geometry whose every channel spans the full axial extent — the
    /// MATLAB fallback when `geometry.zlows` is absent
    /// (`zlows = ones(...)`, `zhis = maxiz*ones(...)`).
    #[must_use]
    pub fn with_full_axial_extent(
        grid: &Grid,
        axial_height: Vec<f64>,
        fuel: FuelRodGeometry,
    ) -> Self {
        let channels = grid.nx * grid.ny;
        Self {
            axial_height,
            z_low: vec![0; channels],
            z_high: vec![grid.nz - 1; channels],
            fuel,
        }
    }
}

/// Coolant state over the whole core. MATLAB `th.coolant`.
///
/// Every vector is `grid.nodes()` long and indexed by
/// [`Grid::index`](crate::reference::grid::Grid::index) with group `0`.
#[derive(Debug, Clone, PartialEq)]
pub struct CoolantState {
    /// Channel inlet temperature \[K\]. MATLAB `th.coolant.inlettemp`.
    pub inlet_temperature: f64,
    /// Channel pressure \[MPa\], held constant along the channel by the HEM
    /// model. MATLAB `th.coolant.inletpress`.
    pub inlet_pressure: f64,
    /// Volumetric inlet gas fraction \[-\]. MATLAB `th.coolant.inletvoid`.
    pub inlet_void: f64,
    /// Cell-centred mixture specific enthalpy \[kJ/kg\]. MATLAB `.enth`.
    pub enthalpy: Vec<f64>,
    /// Cell-**face** enthalpy \[kJ/kg\] from the transient march.
    /// MATLAB `.enthface`; the steady march does not set it.
    pub enthalpy_face: Vec<f64>,
    /// Mixture temperature \[K\] — `Tsat(p)` in the two-phase region.
    /// MATLAB `.temps`.
    pub temperature: Vec<f64>,
    /// Void fraction \[-\], 0 to 1. MATLAB `.alphag`.
    pub void_fraction: Vec<f64>,
    /// Equilibrium steam quality \[-\], clamped to 0–1. MATLAB `.quality`.
    pub quality: Vec<f64>,
    /// Nodal pressure \[MPa\]. MATLAB `.press`.
    pub pressure: Vec<f64>,
    /// Mixture density \[g/cm³\]. MATLAB `.dens`.
    pub density: Vec<f64>,
    /// Saturated/subcooled liquid density \[g/cm³\]. MATLAB `.ldens`.
    pub liquid_density: Vec<f64>,
    /// Vapour density \[g/cm³\]. MATLAB `.gdens`.
    pub gas_density: Vec<f64>,
    /// Mixture velocity \[cm/s\]. MATLAB `.vm`.
    pub mixture_velocity: Vec<f64>,
    /// Liquid thermal conductivity \[W/(cm·K)\]. MATLAB `.tcon`.
    pub thermal_conductivity: Vec<f64>,
    /// Liquid Prandtl number \[-\]. MATLAB `.pran`.
    pub prandtl: Vec<f64>,
    /// Liquid kinematic viscosity \[cm²/s\]. MATLAB `.kvis`.
    pub kinematic_viscosity: Vec<f64>,
    /// Liquid velocity \[cm/s\] — six-equation model only. MATLAB `.vliq`.
    pub liquid_velocity: Vec<f64>,
    /// Vapour velocity \[cm/s\] — six-equation model only. MATLAB `.vgas`.
    pub gas_velocity: Vec<f64>,
    /// Liquid temperature \[K\] — six-equation model only. MATLAB `.tempsliq`.
    pub liquid_temperature: Vec<f64>,
    /// Vapour temperature \[K\] — six-equation model only. MATLAB `.tempsgas`.
    pub gas_temperature: Vec<f64>,
}

impl CoolantState {
    /// A uniform initial coolant state over `nodes` spatial nodes.
    ///
    /// Mirrors `thdiffusion_solverxyz.m:60-61`, which initialises only
    /// `temps` and `dens` and lets the channel solver fill the rest.
    ///
    /// # Arguments
    ///
    /// - `inlet_temperature` \[K\], `inlet_pressure` \[MPa\], `inlet_void` \[-\]
    /// - `temperature` \[K\] — MATLAB `params.cooltempavg`
    /// - `density` \[g/cm³\] — MATLAB `params.cooldenavg`
    #[must_use]
    pub fn uniform(
        nodes: usize,
        inlet_temperature: f64,
        inlet_pressure: f64,
        inlet_void: f64,
        temperature: f64,
        density: f64,
    ) -> Self {
        Self {
            inlet_temperature,
            inlet_pressure,
            inlet_void,
            enthalpy: vec![0.0; nodes],
            enthalpy_face: vec![0.0; nodes],
            temperature: vec![temperature; nodes],
            void_fraction: vec![0.0; nodes],
            quality: vec![0.0; nodes],
            pressure: vec![inlet_pressure; nodes],
            density: vec![density; nodes],
            liquid_density: vec![density; nodes],
            gas_density: vec![0.0; nodes],
            mixture_velocity: vec![0.0; nodes],
            thermal_conductivity: vec![0.0; nodes],
            prandtl: vec![0.0; nodes],
            kinematic_viscosity: vec![0.0; nodes],
            liquid_velocity: vec![0.0; nodes],
            gas_velocity: vec![0.0; nodes],
            liquid_temperature: vec![temperature; nodes],
            gas_temperature: vec![temperature; nodes],
        }
    }
}

/// The whole thermal-hydraulic state passed through the coupled Picard loop.
/// MATLAB `th`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalHydraulicState {
    /// Core thermal power at 100 % \[W\]. MATLAB `th.maxpow`.
    pub max_power: f64,
    /// Current relative core power \[-\]. MATLAB `th.powratio`.
    pub power_ratio: f64,
    /// Fuel pins per node \[-\] (a real number, since cases scale by symmetry).
    /// MATLAB `th.nfuelpin`.
    pub n_fuel_pins: f64,
    /// Fraction of fission energy deposited directly in the coolant \[-\].
    /// MATLAB `th.coolheatfrac`; 0.019 in the NEACRP cases.
    pub coolant_heat_fraction: f64,
    /// Coolant mass flux \[g/(s·cm²)\], one per spatial node. MATLAB
    /// `th.flowrate`, which may be a scalar; expand it with
    /// [`uniform_flow_rate`](Self::uniform_flow_rate).
    pub flow_rate: Vec<f64>,
    /// Flow direction. MATLAB `th.flowdir`.
    pub flow_direction: FlowDirection,
    /// Wall heat flux at the rod surface \[W/cm²\], one per spatial node.
    /// MATLAB `th.heatflux`.
    pub heat_flux: Vec<f64>,
    /// Radial solution nodes per rod, MATLAB `maxid`. See
    /// [`radial_solution_nodes`].
    pub radial_nodes: usize,
    /// Rod radial temperature profiles \[K\], `nodes * radial_nodes` entries in
    /// row-major order (node-major, radial index fastest). MATLAB `th.fueltemp`,
    /// an `es x maxid` matrix.
    pub fuel_temperature: Vec<f64>,
    /// Node-average fuel temperature \[K\]. MATLAB `th.fueltempavg`. Note the
    /// MATLAB sets it equal to the Doppler temperature rather than computing a
    /// volume average — the volume-average line is commented out in
    /// `th_solverxyz.m:189`.
    pub fuel_temperature_average: Vec<f64>,
    /// Doppler-weighted fuel temperature \[K\] used by the cross-section
    /// feedback. MATLAB `th.fueltempdoppler`.
    pub fuel_temperature_doppler: Vec<f64>,
    /// Linear power density \[W/cm\] per node. MATLAB `th.linpwrdens`.
    pub linear_power_density: Vec<f64>,
    /// Coolant fields.
    pub coolant: CoolantState,
    /// Warm-start state vector of the six-equation staggered solver,
    /// `6*nz x n_channels` in column-major order. MATLAB `th.stag6_Ustag`.
    /// Unused while the single-channel kernel is missing.
    pub stag6_u_stag: Vec<f64>,
    /// Wall heat flux the warm start was taken at \[W/cm²\], `nz x n_channels`
    /// column-major. MATLAB `th.stag6_qref`.
    pub stag6_q_ref: Vec<f64>,
    /// Per-channel relative residual of the last six-equation solve \[-\],
    /// `NaN` where never solved. MATLAB `th.stag6_relerr`.
    pub stag6_rel_err: Vec<f64>,
}

impl ThermalHydraulicState {
    /// The rod radial temperature profile \[K\] of spatial node `node`.
    ///
    /// Index 0 is the pellet centreline; the last index is the cladding outer
    /// surface. MATLAB `th.fueltemp(idx,:)`.
    #[must_use]
    pub fn fuel_temperature_row(&self, node: usize) -> &[f64] {
        let start = node * self.radial_nodes;
        &self.fuel_temperature[start..start + self.radial_nodes]
    }

    /// Mutable view of [`fuel_temperature_row`](Self::fuel_temperature_row).
    pub fn fuel_temperature_row_mut(&mut self, node: usize) -> &mut [f64] {
        let start = node * self.radial_nodes;
        &mut self.fuel_temperature[start..start + self.radial_nodes]
    }

    /// Set a spatially uniform coolant mass flux \[g/(s·cm²)\].
    ///
    /// MATLAB writes `th.flowrate` as a scalar and every consumer starts with
    /// `if isscalar(flowrate); flowrate = flowrate*ones(es,1); end`.
    pub fn uniform_flow_rate(&mut self, mass_flux: f64, nodes: usize) {
        self.flow_rate = vec![mass_flux; nodes];
    }
}

/// Solver-level knobs the thermal hydraulics reads out of MATLAB `params`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalHydraulicParams {
    /// The node grid and group count. MATLAB `params.maxix/maxiy/maxiz/G`.
    pub grid: Grid,
    /// Fuel-pin radial node counts. MATLAB `params.fuel`.
    pub fuel: FuelRodParams,
    /// Ceiling of the fuel-temperature clamp \[K\]. MATLAB `params.tmaxfuel`,
    /// default 3100 K (the UO₂ melting point).
    pub max_fuel_temperature: f64,
    /// Fallback coolant temperature \[K\] substituted when the rod solve
    /// returns NaN and the local coolant temperature is not finite either.
    /// MATLAB `params.cooltempavg`.
    pub coolant_average_temperature: f64,
    /// Which channel model the static solver uses. MATLAB `params.th_model`.
    pub channel_model: ChannelModel,
    /// Zuber-Findlay distribution parameter `C0` \[-\] of the void-quality
    /// closure. MATLAB `params.evap_C0`, default 1.2.
    pub evaporation_c0: f64,
    /// Force the homogeneous limit (`C0 = 1`, `Vgj = 0`).
    /// MATLAB `params.evap_homog == 1`.
    pub homogeneous_evaporation: bool,
}

impl ThermalHydraulicParams {
    /// Parameters with the MATLAB defaults: 3100 K fuel clamp, `C0 = 1.2`,
    /// slip (non-homogeneous) void closure, two-fluid channel model.
    #[must_use]
    pub fn new(grid: Grid, fuel: FuelRodParams, coolant_average_temperature: f64) -> Self {
        Self {
            grid,
            fuel,
            max_fuel_temperature: 3100.0,
            coolant_average_temperature,
            channel_model: ChannelModel::TwoFluid,
            evaporation_c0: 1.2,
            homogeneous_evaporation: false,
        }
    }
}

/// MATLAB's `real(x^p)` for real `x` and `p`.
///
/// MATLAB raises a **negative** real base to a fractional power in the complex
/// plane and `real()` then takes the real part, so `real((-x)^p)` is
/// `|x|^p * cos(p*pi)` — a finite, generally non-zero number. Rust's
/// `f64::powf` returns `NaN` for the same inputs. The difference is not
/// cosmetic: `th_solverxyz.m:149` wraps both `pran^0.4` and `reynolds^0.8` in
/// `real()` precisely because those arguments can go negative when a property
/// flash misbehaves, and a `NaN` there would propagate into the fuel
/// temperature and trip the `pauseonnan` guard.
///
/// # Arguments
///
/// - `base` — any real number, including negatives, infinities and `NaN`.
/// - `exponent` — the real exponent.
///
/// # Returns
///
/// `base.powf(exponent)` when `base >= 0` or `base` is `NaN`; otherwise
/// `(-base).powf(exponent) * cos(exponent*pi)`.
#[must_use]
pub fn matlab_real_powf(base: f64, exponent: f64) -> f64 {
    if base < 0.0 {
        (-base).powf(exponent) * (exponent * std::f64::consts::PI).cos()
    } else {
        base.powf(exponent)
    }
}

/// MATLAB `eps` — the double-precision machine epsilon, 2.220446049250313e-16.
///
/// Used verbatim wherever the MATLAB writes `max(x, eps)` or `Tsat - 2*eps`.
///
/// # A translation hazard worth knowing about
///
/// `Tsat - 2*eps` is a **no-op** at reactor temperatures: at `Tsat ≈ 618 K` one
/// unit in the last place is about `1.1e-13`, three orders of magnitude larger
/// than `2*eps`. The MATLAB's intent ("nudge just below saturation so the
/// liquid branch is selected") is therefore not achieved in the original
/// either. The translation reproduces the arithmetic exactly rather than
/// repairing it; see [`steam`] for how the region dispatch copes.
pub const MATLAB_EPS: f64 = f64::EPSILON;

/// Return `Err(ThError::NotANumber)` if `values` contains any NaN.
///
/// Translation of `pauseonnan.m`, which prints the offending array and calls
/// `error('NaN occured')`. The MATLAB also errors on complex input; that arm
/// has no counterpart here.
///
/// # Errors
///
/// [`ThError::NotANumber`] naming `field` and the first offending index.
pub fn pause_on_nan(field: &'static str, values: &[f64]) -> ThResult<()> {
    if let Some(index) = values.iter().position(|v| v.is_nan()) {
        return Err(ThError::NotANumber { field, index });
    }
    Ok(())
}

/// Replace every non-finite entry (`+/-Inf`, `NaN`) with zero.
///
/// Translation of `fixinfnan.m` in its default mode. The MATLAB's optional
/// second mode — substituting `min(abs(vector))` — is not used by any file in
/// this module's scope and is not translated.
#[must_use]
pub fn fix_inf_nan(values: &[f64]) -> Vec<f64> {
    values
        .iter()
        .map(|v| if v.is_finite() { *v } else { 0.0 })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_powf_matches_matlab_on_a_negative_base() {
        // MATLAB: real((-8)^(1/3)) = 2*cos(pi/3) = 1. Rust's powf gives NaN.
        let got = matlab_real_powf(-8.0, 1.0 / 3.0);
        assert!((got - 1.0).abs() < 1e-12, "got {got}");
        assert!((-8.0f64).powf(1.0 / 3.0).is_nan(), "premise of the test");
    }

    #[test]
    fn real_powf_is_ordinary_powf_for_non_negative_bases() {
        assert!((matlab_real_powf(16.0, 0.5) - 4.0).abs() < 1e-12);
        assert_eq!(matlab_real_powf(0.0, 0.8), 0.0);
        assert!(matlab_real_powf(f64::NAN, 0.4).is_nan());
    }

    #[test]
    fn radial_solution_nodes_matches_the_neacrp_layout() {
        // 20 fuel rings, 1 gap ring, 1 clad ring -> maxir = 22, two
        // material/gap transitions -> maxid = 24.
        let mut which_k = vec![RodMaterial::Fuel; 20];
        which_k.push(RodMaterial::Gap);
        which_k.push(RodMaterial::Clad);
        assert_eq!(radial_solution_nodes(&which_k), 24);
    }

    #[test]
    fn radial_solution_nodes_counts_no_surface_without_a_gap() {
        let which_k = vec![RodMaterial::Fuel; 5];
        assert_eq!(radial_solution_nodes(&which_k), 5);
    }

    #[test]
    fn uo2_conductivity_matches_a_hand_evaluated_point() {
        // k(900 K) = (1.05 + 2150/(900 - 73.15))/100
        let expected = (1.05 + 2150.0 / (900.0 - 73.15)) / 100.0;
        let got = ThermalConductivityModel::Uo2Neacrp.evaluate(900.0);
        assert!((got - expected).abs() < 1e-15);
        // Sanity: UO2 near 900 K is around 0.036 W/(cm K) = 3.6 W/(m K).
        assert!((0.030..0.040).contains(&got), "got {got}");
    }

    #[test]
    fn zircaloy_conductivity_is_in_the_right_ballpark() {
        // Zircaloy near 600 K is about 15 W/(m K) = 0.15 W/(cm K).
        let got = ThermalConductivityModel::ZircaloyNeacrp.evaluate(600.0);
        assert!((0.12..0.20).contains(&got), "got {got}");
    }

    #[test]
    fn uo2_volumetric_heat_capacity_is_in_the_right_ballpark() {
        // rho*cp for UO2 near 900 K is roughly 3 J/(cm^3 K).
        let got = VolumetricHeatCapacityModel::Uo2Neacrp.evaluate(900.0);
        assert!((2.5..4.0).contains(&got), "got {got}");
    }

    #[test]
    fn fix_inf_nan_zeroes_non_finite_entries() {
        let got = fix_inf_nan(&[1.0, f64::NAN, f64::INFINITY, -2.0, f64::NEG_INFINITY]);
        assert_eq!(got, vec![1.0, 0.0, 0.0, -2.0, 0.0]);
    }

    #[test]
    fn pause_on_nan_reports_the_first_offender() {
        assert!(pause_on_nan("temps", &[1.0, 2.0]).is_ok());
        let err = pause_on_nan("temps", &[1.0, f64::NAN, f64::NAN]).unwrap_err();
        match err {
            ThError::NotANumber { field, index } => {
                assert_eq!(field, "temps");
                assert_eq!(index, 1);
            }
            other => panic!("wrong error: {other}"),
        }
    }
}
