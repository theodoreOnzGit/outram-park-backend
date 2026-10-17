//! **What to collect beside k_eff**: [`Diagnostics`], a list of
//! [`MCDiagnostic`]s (gh #940, epic #925).
//!
//! ```no_run
//! use outram_mc_libs::model::prelude::*;
//! # fn f(model: Model) -> Result<(), Box<dyn std::error::Error>> {
//! let diagnostics = Diagnostics::default()
//!     .with_spectrum_per_component(&model, EnergyGrid::default_spectrum());
//! let run = model.keff(KeffSettings::new(MCSettings::Auto, Some(diagnostics)))?;
//! let factors = run.six_factors()?;              // Err if not requested
//! run.diagnostics_report().write_zip("out/godiva.zip")?;
//! # Ok(()) }
//! ```
//!
//! # Two presets, and constructors for everything else
//!
//! Defaults live **only** in the two presets (maintainer decisions on #925,
//! 2026-10-17); every constructor takes every argument:
//!
//! | preset | diagnostics |
//! |---|---|
//! | [`Diagnostics::default`] | RunHealth, NeutronBalance (by material), Ealf, Leakage (spectrum grid), SixFactors (0.625 eV, 0.1 MeV), Spectrum (whole model, 1000 equal-lethargy bins, 1e-3 eV to 20 MeV) |
//! | [`Diagnostics::reactor_physics`] | the default set plus Kinetics (IFP, 10 generations), FissionMap and FluxMap on a 15 × 15 × 15 mesh over the model's bounding box, flux as total + three groups |
//!
//! Maps on a mesh **you** choose (spatial and energy) are an `advanced`
//! choice ([`super::advanced`]). Physics is never a diagnostic setting:
//! nothing here turns URR, DBRC or a thermal law on or off.
//!
//! # How it runs
//!
//! [`Diagnostics`] orchestrates the existing tallies (`tally::Tally`,
//! filters × scores) and leakage accounting of the CSG transport: each
//! diagnostic asks for the tallies it reads, identical requests are shared,
//! and all are scored on the same histories through
//! [`crate::physics::transport_csg::CsgPowerIteration::step_scored`].
//! Scoring draws no random number, so **a diagnostic gives the same numbers
//! bit for bit whether it is requested alone or beside others**, and `k` is
//! the k of a run with no diagnostics at all.
//!
//! Not here: MGXS (the medium-level API, #845 / #848), per-nuclide reaction
//! rates (no nuclide filter exists in `tally` yet).

use uom::si::energy::electronvolt;
use uom::si::f64::{Energy, Length};
use uom::si::length::centimeter;

use super::model::Model;
use super::shape::{Component, Shape};

/// The thermal / resonance group edge of the preset six-factor split and
/// flux map \[eV\].
///
/// **Source.** The value is the existing `ReactorPhysicsConfig` default,
/// kept by maintainer decision (#925, 2026-10-17). It is a conventional
/// group edge: NJOY2016's own GROUPR example deck uses `0.625` among its
/// group bounds (LA-UR-17-20093, the NJOY2016 manual, standard open corpus
/// `lanl/2022laur1720093.pdf`, ERRORJ/GROUPR test input). The six factors
/// themselves follow DOE-HDBK-1019/2-93 (*Nuclear Physics and Reactor
/// Theory*, vol. 2, "Six Factor Formula"; standard open corpus
/// `us-doe/doe-hdbk-1019-2-93-nuclear-physics-reactor-theory.pdf`), which
/// defines "thermal" without an energy. **No corpus source defines 0.625 eV
/// as the cadmium cut-off**; that sentence is not made here (open on #940).
pub const SIX_FACTOR_THERMAL_EDGE_EV: f64 = 0.625;

/// The resonance / fast group edge of the preset six-factor split \[eV\]
/// (0.1 MeV). The existing `ReactorPhysicsConfig` default, kept by
/// maintainer decision (#925). The same NJOY2016 GROUPR example group
/// structure carries `1.e5` as a bound (LA-UR-17-20093); no corpus source
/// defines it as *the* fast edge (open on #940).
pub const SIX_FACTOR_FAST_EDGE_EV: f64 = 1.0e5;

/// Bins of the preset spectrum: 1000, equal in lethargy (maintainer, #925).
pub const SPECTRUM_BINS: usize = 1000;
/// Lower edge of the preset spectrum \[eV\] (the existing
/// `ReactorPhysicsConfig` range; "assumed unless the maintainer says
/// otherwise", #925).
pub const SPECTRUM_E_MIN_EV: f64 = 1.0e-3;
/// Upper edge of the preset spectrum \[eV\] (20 MeV, the top of the
/// ENDF/B-VIII.0 neutron sub-library).
pub const SPECTRUM_E_MAX_EV: f64 = 2.0e7;
/// IFP lineage length of the preset kinetics: OpenMC's default
/// `ifp_n_generation` (`IfpSettings::default`, upstream `settings.cpp`).
pub const IFP_GENERATIONS: usize = 10;
/// Divisions per side of the preset maps' mesh over the model's bounding
/// box: 15, so 3 375 cells for any model (maintainer, #925).
pub const MAP_DIVISIONS: usize = 15;

/// Why a diagnostic could not be built, run or read.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DiagnosticError {
    /// A setting out of range (edges not ascending, zero bins, ...).
    #[error("invalid diagnostic: {0}")]
    Invalid(String),
    /// A component made by another model, or used by no shape.
    #[error("{0}")]
    Component(String),
    /// A result was asked for that the run did not collect.
    #[error("the run did not collect {variant}: add it to the Diagnostics ({how})")]
    NotRequested {
        /// The variant to add.
        variant: &'static str,
        /// How to add it.
        how: &'static str,
    },
    /// The run collected it but it has no value (e.g. IFP with too few
    /// active cycles).
    #[error("{0}")]
    NoValue(String),
}

/// **An energy grid** for a spectrum, a leakage spectrum or a map.
#[derive(Debug, Clone, PartialEq)]
pub enum EnergyGrid {
    /// `bins` bins equally spaced in lethargy (log energy) from `e_min` to
    /// `e_max`.
    EqualLethargy {
        /// Number of bins (> 0).
        bins: usize,
        /// Lower edge (> 0).
        e_min: Energy,
        /// Upper edge (> `e_min`).
        e_max: Energy,
    },
    /// Explicit ascending edges (at least two).
    Edges(Vec<Energy>),
    /// One group: every energy.
    OneGroup,
    /// Three groups split at the two edges (thermal < `thermal_edge` <
    /// resonance < `fast_edge` < fast), plus their total: the maps' grid.
    TotalAndThreeGroup {
        /// Thermal / resonance edge.
        thermal_edge: Energy,
        /// Resonance / fast edge.
        fast_edge: Energy,
    },
}

impl EnergyGrid {
    /// **The preset spectrum grid**, named so a hand-built diagnostic can
    /// reuse it in one word: [`SPECTRUM_BINS`] equal-lethargy bins from
    /// [`SPECTRUM_E_MIN_EV`] to [`SPECTRUM_E_MAX_EV`].
    pub fn default_spectrum() -> Self {
        EnergyGrid::EqualLethargy {
            bins: SPECTRUM_BINS,
            e_min: Energy::new::<electronvolt>(SPECTRUM_E_MIN_EV),
            e_max: Energy::new::<electronvolt>(SPECTRUM_E_MAX_EV),
        }
    }

    /// The preset three-group grid of the maps (the six-factor edges).
    pub fn default_three_group() -> Self {
        EnergyGrid::TotalAndThreeGroup {
            thermal_edge: Energy::new::<electronvolt>(SIX_FACTOR_THERMAL_EDGE_EV),
            fast_edge: Energy::new::<electronvolt>(SIX_FACTOR_FAST_EDGE_EV),
        }
    }

    /// The bin edges \[eV\], checked. `OneGroup` and `TotalAndThreeGroup`
    /// run from 0 to `f64::MAX`, so no neutron falls outside them.
    ///
    /// # Errors
    /// Zero bins, non-positive or non-finite energies, edges not strictly
    /// ascending.
    pub fn edges_ev(&self) -> Result<Vec<f64>, DiagnosticError> {
        let bad = |m: String| Err(DiagnosticError::Invalid(m));
        match self {
            EnergyGrid::EqualLethargy { bins, e_min, e_max } => {
                let (lo, hi) = (e_min.get::<electronvolt>(), e_max.get::<electronvolt>());
                if *bins == 0 || !(lo > 0.0 && hi > lo && hi.is_finite()) {
                    return bad(format!(
                        "an equal-lethargy grid needs bins > 0 and 0 < e_min < e_max, not {bins} bins from {lo} to {hi} eV"
                    ));
                }
                Ok(crate::physics::reactor_physics::log_energy_grid(
                    lo, hi, *bins,
                ))
            }
            EnergyGrid::Edges(e) => {
                let v: Vec<f64> = e.iter().map(|x| x.get::<electronvolt>()).collect();
                if v.len() < 2
                    || v.iter().any(|x| !(x.is_finite() && *x >= 0.0))
                    || v.windows(2).any(|w| w[1] <= w[0])
                {
                    return bad(format!("energy edges must be at least two, finite, non-negative and strictly ascending: {v:?} eV"));
                }
                Ok(v)
            }
            EnergyGrid::OneGroup => Ok(vec![0.0, f64::MAX]),
            EnergyGrid::TotalAndThreeGroup {
                thermal_edge,
                fast_edge,
            } => {
                let (t, f) = (
                    thermal_edge.get::<electronvolt>(),
                    fast_edge.get::<electronvolt>(),
                );
                if !(t > 0.0 && f > t && f.is_finite()) {
                    return bad(format!(
                        "group edges must satisfy 0 < thermal < fast, not {t} and {f} eV"
                    ));
                }
                Ok(vec![0.0, t, f, f64::MAX])
            }
        }
    }
}

/// **Where a spectrum is taken.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// Every cell of the model, voids included (the preset).
    WholeModel,
    /// Every shape in one [`Component`] (the core, the reflector, all the
    /// control rods together).
    Component(Component),
}

/// How [`MCDiagnostic::NeutronBalance`] splits the absorption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceBy {
    /// The whole model only.
    Total,
    /// Per material (the preset), plus the total.
    Material,
    /// Per component, plus the total (shapes with no component are a row of
    /// their own).
    Component,
}

/// **A spatial mesh for a map.** The presets make the model-relative 15³
/// one; any other mesh is an advanced choice
/// ([`super::advanced::mesh_divisions`], [`super::advanced::mesh_cells`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub(crate) kind: MeshSpec,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MeshSpec {
    /// `n` divisions per axis over `bounds`.
    Divisions { n: [usize; 3], bounds: MeshBounds },
    /// Cells of the given size \[cm\] tiling `bounds` (the last cell on an
    /// axis may be cut by the bound: the count is rounded up and the box
    /// grown to fit, so cells keep the size asked for).
    Cells {
        cell_cm: [f64; 3],
        bounds: MeshBounds,
    },
}

/// **What a mesh covers.**
#[derive(Debug, Clone, PartialEq)]
pub enum MeshBounds {
    /// The model's own bounding box.
    FitModel,
    /// A box given in lengths.
    Explicit {
        /// Lower corner.
        lower: [Length; 3],
        /// Upper corner.
        upper: [Length; 3],
    },
}

impl Mesh {
    /// The preset: [`MAP_DIVISIONS`] per side over the model.
    pub(crate) fn model_relative_default() -> Self {
        Mesh {
            kind: MeshSpec::Divisions {
                n: [MAP_DIVISIONS; 3],
                bounds: MeshBounds::FitModel,
            },
        }
    }

    /// The regular mesh this resolves to on a model with bounding box
    /// `model_bounds` \[cm\].
    pub(crate) fn resolve(
        &self,
        model_bounds: ([f64; 3], [f64; 3]),
    ) -> Result<crate::tally::mesh::RegularMesh, DiagnosticError> {
        let bad = |m: String| Err(DiagnosticError::Invalid(m));
        let bounds = |b: &MeshBounds| -> Result<([f64; 3], [f64; 3]), DiagnosticError> {
            match b {
                MeshBounds::FitModel => Ok(model_bounds),
                MeshBounds::Explicit { lower, upper } => {
                    let l = lower.map(|x| x.get::<centimeter>());
                    let u = upper.map(|x| x.get::<centimeter>());
                    if (0..3).any(|i| !(u[i] > l[i] && l[i].is_finite() && u[i].is_finite())) {
                        return Err(DiagnosticError::Invalid(format!(
                            "a mesh box needs lower < upper on every axis, not {l:?} to {u:?} cm"
                        )));
                    }
                    Ok((l, u))
                }
            }
        };
        match &self.kind {
            MeshSpec::Divisions { n, bounds: b } => {
                if n.iter().any(|&k| k == 0) {
                    return bad(format!("mesh divisions must be > 0 per axis, not {n:?}"));
                }
                let (l, u) = bounds(b)?;
                Ok(crate::tally::mesh::RegularMesh {
                    lower_left: l,
                    upper_right: u,
                    dimension: *n,
                })
            }
            MeshSpec::Cells { cell_cm, bounds: b } => {
                if cell_cm.iter().any(|&c| !(c > 0.0 && c.is_finite())) {
                    return bad(format!("mesh cells must be positive, not {cell_cm:?} cm"));
                }
                let (l, u) = bounds(b)?;
                let n: [usize; 3] =
                    std::array::from_fn(|i| ((u[i] - l[i]) / cell_cm[i]).ceil().max(1.0) as usize);
                let upper: [f64; 3] = std::array::from_fn(|i| l[i] + n[i] as f64 * cell_cm[i]);
                Ok(crate::tally::mesh::RegularMesh {
                    lower_left: l,
                    upper_right: upper,
                    dimension: n,
                })
            }
        }
    }
}

/// **One thing to collect beside k_eff.** Each variant carries its own
/// settings; build them with the constructors (no hidden defaults), or take
/// a preset ([`Diagnostics::default`], [`Diagnostics::reactor_physics`]).
#[derive(Debug, Clone, PartialEq)]
pub enum MCDiagnostic {
    /// k and the source entropy per cycle, the convergence verdict, lost and
    /// stuck histories, collisions, wall time and hardware.
    RunHealth,
    /// Where neutrons end: fission, capture and leakage per source neutron,
    /// by track length and by analog count, and whether they balance.
    NeutronBalance {
        /// The split.
        by: BalanceBy,
    },
    /// The energy of the average lethargy causing fission (EALF), a
    /// spectral index used in criticality-safety validation (NUREG-1520
    /// Rev. 2, 2015, §5; standard open corpus
    /// `nrc/nureg-1520-rev2-2015.pdf`).
    Ealf,
    /// The leakage per source neutron and its energy spectrum.
    Leakage {
        /// The spectrum's grid.
        grid: EnergyGrid,
    },
    /// The six-factor formula (η, f, p, ε, P_FNL, P_TNL) on a three-group
    /// split (`physics::reactor_physics::assemble_six_factors`).
    SixFactors {
        /// Thermal / resonance edge.
        thermal_edge: Energy,
        /// Resonance / fast edge.
        fast_edge: Energy,
    },
    /// The lethargy-normalised flux spectrum in a region.
    Spectrum {
        /// Where.
        region: Region,
        /// On which grid.
        grid: EnergyGrid,
    },
    /// Adjoint-weighted β_eff and Λ by iterated fission probability (IFP,
    /// `physics::ifp`, OpenMC `src/ifp.cpp`).
    Kinetics {
        /// Generations of lineage (> 0).
        ifp_generations: usize,
    },
    /// The fission rate per mesh cell.
    FissionMap {
        /// The mesh.
        mesh: Mesh,
    },
    /// The flux per mesh cell and energy group.
    FluxMap {
        /// The mesh.
        mesh: Mesh,
        /// The groups.
        grid: EnergyGrid,
    },
}

impl MCDiagnostic {
    /// [`MCDiagnostic::RunHealth`].
    pub fn run_health() -> Self {
        MCDiagnostic::RunHealth
    }

    /// [`MCDiagnostic::NeutronBalance`], split `by`.
    pub fn neutron_balance(by: BalanceBy) -> Self {
        MCDiagnostic::NeutronBalance { by }
    }

    /// [`MCDiagnostic::Ealf`].
    pub fn ealf() -> Self {
        MCDiagnostic::Ealf
    }

    /// [`MCDiagnostic::Leakage`] with its spectrum on `grid`.
    pub fn leakage(grid: EnergyGrid) -> Self {
        MCDiagnostic::Leakage { grid }
    }

    /// [`MCDiagnostic::SixFactors`] split at the two edges.
    ///
    /// # Errors
    /// Unless `0 < thermal_edge < fast_edge` (finite).
    pub fn six_factors(thermal_edge: Energy, fast_edge: Energy) -> Result<Self, DiagnosticError> {
        EnergyGrid::TotalAndThreeGroup {
            thermal_edge,
            fast_edge,
        }
        .edges_ev()?;
        Ok(MCDiagnostic::SixFactors {
            thermal_edge,
            fast_edge,
        })
    }

    /// [`MCDiagnostic::Spectrum`] in `region` on `grid`.
    pub fn spectrum(region: Region, grid: EnergyGrid) -> Self {
        MCDiagnostic::Spectrum { region, grid }
    }

    /// [`MCDiagnostic::Kinetics`] with `ifp_generations` of lineage. The run
    /// needs more active cycles than that; every preset has them.
    ///
    /// # Errors
    /// `ifp_generations == 0`.
    pub fn kinetics(ifp_generations: usize) -> Result<Self, DiagnosticError> {
        if ifp_generations == 0 {
            return Err(DiagnosticError::Invalid(
                "IFP needs at least one generation of lineage".into(),
            ));
        }
        Ok(MCDiagnostic::Kinetics { ifp_generations })
    }

    /// The variant's name, as the report and errors print it.
    pub fn name(&self) -> &'static str {
        match self {
            MCDiagnostic::RunHealth => "RunHealth",
            MCDiagnostic::NeutronBalance { .. } => "NeutronBalance",
            MCDiagnostic::Ealf => "Ealf",
            MCDiagnostic::Leakage { .. } => "Leakage",
            MCDiagnostic::SixFactors { .. } => "SixFactors",
            MCDiagnostic::Spectrum { .. } => "Spectrum",
            MCDiagnostic::Kinetics { .. } => "Kinetics",
            MCDiagnostic::FissionMap { .. } => "FissionMap",
            MCDiagnostic::FluxMap { .. } => "FluxMap",
        }
    }
}

/// **The high-level outputs to collect**, passed as
/// `KeffSettings::new(mc, Some(diagnostics))`. `None` there means k_eff
/// only. Two of one kind are allowed (spectra in several regions, six
/// factors at several edges); their results are told apart by their
/// settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostics {
    items: Vec<MCDiagnostic>,
}

impl Default for Diagnostics {
    /// **"Wanted by everyone"** (maintainer, #925): RunHealth,
    /// NeutronBalance by material, Ealf, Leakage on the spectrum grid,
    /// SixFactors at [`SIX_FACTOR_THERMAL_EDGE_EV`] and
    /// [`SIX_FACTOR_FAST_EDGE_EV`], and the whole-model Spectrum on
    /// [`EnergyGrid::default_spectrum`].
    fn default() -> Self {
        let edges = MCDiagnostic::SixFactors {
            thermal_edge: Energy::new::<electronvolt>(SIX_FACTOR_THERMAL_EDGE_EV),
            fast_edge: Energy::new::<electronvolt>(SIX_FACTOR_FAST_EDGE_EV),
        };
        Diagnostics {
            items: vec![
                MCDiagnostic::RunHealth,
                MCDiagnostic::NeutronBalance {
                    by: BalanceBy::Material,
                },
                MCDiagnostic::Ealf,
                MCDiagnostic::Leakage {
                    grid: EnergyGrid::default_spectrum(),
                },
                edges,
                MCDiagnostic::Spectrum {
                    region: Region::WholeModel,
                    grid: EnergyGrid::default_spectrum(),
                },
            ],
        }
    }
}

impl Diagnostics {
    /// Empty: k_eff only, as `None` would be. Add diagnostics with
    /// [`Self::with`].
    pub fn new() -> Self {
        Diagnostics { items: Vec::new() }
    }

    /// **The reactor-physics set**: [`Diagnostics::default`] plus Kinetics
    /// (IFP, [`IFP_GENERATIONS`] generations), a FissionMap and a FluxMap on
    /// [`MAP_DIVISIONS`]³ cells over the model's bounding box, the flux as
    /// total + three groups ([`EnergyGrid::default_three_group`]).
    pub fn reactor_physics() -> Self {
        let mut d = Self::default();
        d.items.push(MCDiagnostic::Kinetics {
            ifp_generations: IFP_GENERATIONS,
        });
        d.items.push(MCDiagnostic::FissionMap {
            mesh: Mesh::model_relative_default(),
        });
        d.items.push(MCDiagnostic::FluxMap {
            mesh: Mesh::model_relative_default(),
            grid: EnergyGrid::default_three_group(),
        });
        d
    }

    /// Add one diagnostic.
    pub fn with(mut self, d: MCDiagnostic) -> Self {
        self.items.push(d);
        self
    }

    /// **One spectrum per component** of `model` that some shape is in, on
    /// `grid` (the helper decided on #925: per component, not per cell).
    pub fn with_spectrum_per_component(mut self, model: &Model, grid: EnergyGrid) -> Self {
        let mut seen: Vec<Component> = Vec::new();
        fn walk(s: &Shape, seen: &mut Vec<Component>) {
            for c in &s.children {
                walk(c, seen);
            }
            if let Some(c) = s.component {
                if !seen.contains(&c) {
                    seen.push(c);
                }
            }
        }
        for s in model.shapes() {
            walk(s, &mut seen);
        }
        seen.sort_by_key(|c| c.index);
        for c in seen {
            self.items.push(MCDiagnostic::Spectrum {
                region: Region::Component(c),
                grid: grid.clone(),
            });
        }
        self
    }

    /// The diagnostics, in the order they were added.
    pub fn items(&self) -> &[MCDiagnostic] {
        &self.items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(x: f64) -> Energy {
        Energy::new::<electronvolt>(x)
    }

    /// The presets hold exactly the decided diagnostics and values.
    #[test]
    fn presets_hold_the_decided_diagnostics() {
        let d = Diagnostics::default();
        let names: Vec<&str> = d.items().iter().map(MCDiagnostic::name).collect();
        assert_eq!(
            names,
            [
                "RunHealth",
                "NeutronBalance",
                "Ealf",
                "Leakage",
                "SixFactors",
                "Spectrum"
            ]
        );
        assert!(d
            .items()
            .contains(&MCDiagnostic::six_factors(ev(0.625), ev(1.0e5)).unwrap()));
        let g = EnergyGrid::default_spectrum().edges_ev().unwrap();
        assert_eq!(g.len(), 1001);
        assert!((g[0] - 1.0e-3).abs() < 1e-15 && (g[1000] - 2.0e7).abs() < 1e-6);
        let r = Diagnostics::reactor_physics();
        let names: Vec<&str> = r.items().iter().map(MCDiagnostic::name).collect();
        assert_eq!(&names[6..], ["Kinetics", "FissionMap", "FluxMap"]);
        assert!(r.items().contains(&MCDiagnostic::kinetics(10).unwrap()));
        let m = Mesh::model_relative_default()
            .resolve(([-1.0, -2.0, -3.0], [1.0, 2.0, 3.0]))
            .unwrap();
        assert_eq!(m.dimension, [15, 15, 15]);
        assert_eq!(m.upper_right, [1.0, 2.0, 3.0]);
        assert_eq!(
            Diagnostics::new().with(MCDiagnostic::ealf()).items().len(),
            1
        );
    }

    /// Constructors refuse what does not make sense, with the reason.
    #[test]
    fn constructors_validate() {
        assert!(MCDiagnostic::six_factors(ev(1.0e5), ev(0.625)).is_err());
        assert!(MCDiagnostic::kinetics(0).is_err());
        assert!(EnergyGrid::Edges(vec![ev(2.0), ev(1.0)])
            .edges_ev()
            .is_err());
        assert!(EnergyGrid::EqualLethargy {
            bins: 0,
            e_min: ev(1.0),
            e_max: ev(2.0)
        }
        .edges_ev()
        .is_err());
        assert_eq!(
            EnergyGrid::OneGroup.edges_ev().unwrap(),
            vec![0.0, f64::MAX]
        );
        assert_eq!(
            EnergyGrid::default_three_group().edges_ev().unwrap(),
            vec![0.0, 0.625, 1.0e5, f64::MAX]
        );
        let cm = Length::new::<centimeter>;
        let bad = Mesh {
            kind: MeshSpec::Divisions {
                n: [0, 1, 1],
                bounds: MeshBounds::FitModel,
            },
        };
        assert!(bad.resolve(([0.0; 3], [1.0; 3])).is_err());
        let cells = Mesh {
            kind: MeshSpec::Cells {
                cell_cm: [0.4, 1.0, 1.0],
                bounds: MeshBounds::Explicit {
                    lower: [cm(0.0); 3],
                    upper: [cm(1.0); 3],
                },
            },
        };
        let m = cells.resolve(([9.0; 3], [9.0; 3])).unwrap();
        assert_eq!(m.dimension, [3, 1, 1]);
        assert!((m.upper_right[0] - 1.2).abs() < 1e-12);
        for d in [
            MCDiagnostic::run_health(),
            MCDiagnostic::neutron_balance(BalanceBy::Total),
            MCDiagnostic::leakage(EnergyGrid::OneGroup),
            MCDiagnostic::spectrum(Region::WholeModel, EnergyGrid::OneGroup),
        ] {
            assert!(!d.name().is_empty());
        }
    }
}
