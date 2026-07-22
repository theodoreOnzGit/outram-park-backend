//! Plain, self-contained specification structs produced by the input-deck
//! parser ([`super::parse_deck`]).
//!
//! These types are made of primitives only (`usize` / `f64` / `String` / `Vec`)
//! and deliberately do **not** reference the [`crate::grid`],
//! [`crate::properties`], or [`crate::solver`] modules. The RICHARDS driver is
//! responsible for mapping a parsed [`InputDeck`] onto those modules; keeping
//! the deck decoupled lets the parser and the physics evolve independently.
//!
//! Units follow the deck convention throughout: **lengths in metres, pressures
//! in pascals (Pa), times in seconds (s), permeability in m^2, van-Genuchten /
//! Brooks-Corey `alpha` in 1/Pa, and Darcy fluxes in m/s.**

/// A parsed v1 input deck.
///
/// All quantities use SI: lengths in metres, pressures in pascals (Pa), times
/// in seconds (s). Produced by [`super::parse_deck`]; consumed by the RICHARDS
/// driver, which maps it onto the grid / properties / solver modules.
#[derive(Debug, Clone, PartialEq)]
pub struct InputDeck {
    /// Structured Cartesian grid dimensions and cell sizes (metres).
    pub grid: GridSpec,
    /// Bulk material properties: porosity (-) and permeability (m^2).
    pub material: MaterialSpec,
    /// Saturation / relative-permeability characteristic-curve parameters.
    pub curves: CurveSpec,
    /// Zero or more boundary-condition cards. May be empty (an all-no-flow
    /// domain), though a well-posed RICHARDS problem normally needs at least
    /// one Dirichlet face.
    pub boundary_conditions: Vec<BoundaryConditionSpec>,
    /// Time-stepping controls (seconds).
    pub time: TimeSpec,
    /// Uniform initial liquid pressure applied to every cell, in pascals (Pa).
    pub initial_pressure: f64,
}

/// `GRID` card: a structured Cartesian mesh with uniform cell spacing.
///
/// `nx`/`ny`/`nz` are cell counts along each axis (all `> 0`); `dx`/`dy`/`dz`
/// are the corresponding uniform cell edge lengths in **metres** (all `> 0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSpec {
    /// Number of cells along x (`> 0`).
    pub nx: usize,
    /// Number of cells along y (`> 0`).
    pub ny: usize,
    /// Number of cells along z (`> 0`).
    pub nz: usize,
    /// Cell edge length along x, in metres (`> 0`).
    pub dx: f64,
    /// Cell edge length along y, in metres (`> 0`).
    pub dy: f64,
    /// Cell edge length along z, in metres (`> 0`).
    pub dz: f64,
}

/// `MATERIAL` card: bulk porous-medium properties.
///
/// A single homogeneous, isotropic material for the whole domain in v1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialSpec {
    /// Porosity, dimensionless, strictly in the open interval `(0, 1)`.
    pub porosity: f64,
    /// Isotropic intrinsic permeability, in m^2 (`> 0`).
    pub permeability: f64,
}

/// `CHARACTERISTIC_CURVES` card: saturation / relative-permeability model.
///
/// The parameter [`Self::n_or_lambda`] carries the van-Genuchten `n` (the pore
/// size distribution exponent, dimensionless) when [`Self::model`] is
/// [`CurveModel::VanGenuchten`], or the Brooks-Corey `lambda` (dimensionless)
/// when it is [`CurveModel::BrooksCorey`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveSpec {
    /// Which retention model the parameters describe.
    pub model: CurveModel,
    /// Air-entry / scaling parameter `alpha`, in 1/Pa (`> 0`).
    pub alpha: f64,
    /// Van-Genuchten `n` or Brooks-Corey `lambda` (dimensionless, `> 0`), per
    /// [`Self::model`].
    pub n_or_lambda: f64,
    /// Residual (irreducible) liquid saturation, dimensionless, in `[0, 1)`.
    pub residual_saturation: f64,
}

/// Retention / relative-permeability model family selected by the
/// `CHARACTERISTIC_CURVES` `MODEL` keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveModel {
    /// Van-Genuchten (1980) capillary-pressure / saturation model.
    VanGenuchten,
    /// Brooks-Corey (1964) capillary-pressure / saturation model.
    BrooksCorey,
}

/// `BOUNDARY_CONDITION` card: one condition applied to one named domain face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryConditionSpec {
    /// Which of the six domain faces the condition applies to.
    pub location: BoundaryLocationSpec,
    /// The condition type and its value.
    pub kind: BoundaryKindSpec,
}

/// The domain face a boundary condition applies to (the outward faces of the
/// structured box).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryLocationSpec {
    /// Face at minimum x (`i = 0`).
    XMin,
    /// Face at maximum x (`i = nx - 1`).
    XMax,
    /// Face at minimum y (`j = 0`).
    YMin,
    /// Face at maximum y (`j = ny - 1`).
    YMax,
    /// Face at minimum z (`k = 0`).
    ZMin,
    /// Face at maximum z (`k = nz - 1`).
    ZMax,
}

/// The type and value of a boundary condition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoundaryKindSpec {
    /// Fixed liquid pressure on the face, in pascals (Pa).
    DirichletPressure(f64),
    /// Fixed Darcy flux normal to the face, in m/s (positive = into the
    /// domain by convention; the driver fixes the sign).
    NeumannFlux(f64),
}

/// `TIME` card: transient-run time controls, all in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeSpec {
    /// Final simulated time, in seconds (`> 0`).
    pub final_time: f64,
    /// Initial timestep size, in seconds (`> 0`).
    pub initial_dt: f64,
    /// Maximum allowed timestep size, in seconds (`> 0`).
    pub max_dt: f64,
}
