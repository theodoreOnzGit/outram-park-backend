//! Well models and advanced source/sink + boundary conditions (bead op-v6s.15.12).
//!
//! This module supplies the parts of a subsurface-flow model that inject or
//! withdraw fluid at points/columns inside the domain (**wells** and
//! **source/sink** terms) and the pressure/flux conditions imposed on its
//! exterior (**advanced boundary conditions**). All quantities are SI and the
//! sign convention throughout is **positive = fluid entering the cell**
//! (injection), **negative = fluid leaving** (production/outflow).
//!
//! ## Peaceman vertical-well model
//!
//! A well whose radius `r_w` (typically centimetres) is far smaller than a grid
//! cell (metres) cannot be resolved by the pressure the cell carries — the
//! numerical cell pressure is a *volume average*, not the pressure at the well
//! sandface. Peaceman (1978, 1983) showed that for a vertical well centred in a
//! Cartesian cell the steady radial-flow solution recovers the correct rate if
//! the cell pressure is interpreted as the pressure at an **equivalent radius**
//! `r_eq`, and the well is coupled to the cell through a **well index** `WI`:
//!
//! ```text
//!     WI = 2*pi * k * h / ( ln(r_eq / r_w) + skin )
//! ```
//!
//! - `k`  — cell permeability, m^2 (isotropic form here; see review flags).
//! - `h`  — perforated thickness of the cell, m (the cell's `dz` for a vertical
//!          well fully perforating the layer).
//! - `r_w`— wellbore radius, m.
//! - `skin` — dimensionless near-well damage/stimulation factor (`+` damage
//!          reduces `WI`, `-` stimulation increases it).
//!
//! The **isotropic** equivalent radius (used by [`PeacemanWell::well_index`]) is
//!
//! ```text
//!     r_eq = 0.14 * sqrt(dx^2 + dy^2)
//! ```
//!
//! which is the isotropic special case (`kx = ky`) of Peaceman's anisotropic
//! formula
//!
//! ```text
//!     r_eq = 0.28 * sqrt( sqrt(ky/kx)*dx^2 + sqrt(kx/ky)*dy^2 )
//!                  / ( (ky/kx)^0.25 + (kx/ky)^0.25 ).
//! ```
//!
//! The volumetric flow the well drives into a perforated cell is
//!
//! ```text
//!     q_well = WI * lambda * (p_bh - p_cell)          [m^3/s]
//! ```
//!
//! where `lambda = k_r / mu` is the fluid **mobility** (relative permeability
//! over dynamic viscosity, units 1/(Pa*s)) and `p_bh` is the wellbore
//! **bottom-hole pressure** (Pa). With this sign, `p_bh > p_cell` gives `q > 0`
//! (injection) and `p_bh < p_cell` gives `q < 0` (production). A
//! **rate-controlled** well instead distributes a prescribed total volumetric
//! rate across its perforated cells in proportion to each cell's `WI` (the
//! larger the connection, the larger its share).
//!
//! ## Advanced boundary conditions
//!
//! [`AdvancedBc`] collects three state/space/time-dependent conditions common to
//! PFLOTRAN `FLOW_CONDITION` cards:
//!
//! - **Hydrostatic** — a Dirichlet pressure varying with elevation about a
//!   datum, `p(z) = p_datum + rho*g*(z_datum - z)`; pressure increases downward.
//! - **Seepage face** — a *state-dependent switch*: the face conducts (outflow
//!   permitted) only where the cell pressure exceeds atmospheric; otherwise it is
//!   a no-flow boundary. See [`AdvancedBc::value`] for the exact encoding.
//! - **Time-varying** — a tabulated `value(t)` with linear interpolation and
//!   flat clamping outside the table range (a piecewise-linear forcing).
//!
//! ## Units summary
//!
//! | Symbol      | Meaning                          | Unit        |
//! |-------------|----------------------------------|-------------|
//! | `k`         | permeability                     | m^2         |
//! | `dx,dy,dz`  | cell dimensions                  | m           |
//! | `h`         | perforated thickness             | m           |
//! | `r_w`       | wellbore radius                  | m           |
//! | `r_eq`      | Peaceman equivalent radius       | m           |
//! | `skin`      | near-well skin factor            | dimensionless |
//! | `WI`        | well index                       | m^3 (i.e. m^2 * m) |
//! | `lambda`    | fluid mobility `k_r/mu`          | 1/(Pa*s)    |
//! | `p_bh`      | bottom-hole pressure             | Pa          |
//! | `p_cell`    | cell (grid-block) pressure       | Pa          |
//! | `q`         | volumetric source/sink rate      | m^3/s       |
//! | `rho`       | fluid density                    | kg/m^3      |
//! | `g`         | gravitational acceleration       | m/s^2       |
//!
//! ## Design (workspace mandate)
//!
//! Enum dispatch, no trait objects, no `Box`, no lifetime parameters, pure
//! Rust with no external dependencies (Android-clean). Cell geometry is read
//! from [`crate::grid::CartesianGrid`] through its public accessors only.
//!
//! ## Provenance
//!
//! - Peaceman, D. W. (1978), "Interpretation of Well-Block Pressures in
//!   Numerical Reservoir Simulation", *SPE Journal* **18**(3), 183–194.
//! - Peaceman, D. W. (1983), "Interpretation of Well-Block Pressures in
//!   Numerical Reservoir Simulation With Nonsquare Grid Blocks and Anisotropic
//!   Permeability", *SPE Journal* **23**(3), 531–543.
//! - Hydrostatic and seepage-face flow conditions follow the standard PFLOTRAN
//!   `FLOW_CONDITION` formulation (PFLOTRAN theory/user guide,
//!   documentation.pflotran.org).
//!
//! ## Human-review flags (untrusted AI draft, no human V&V)
//!
//! - **Single-phase.** Mobility `lambda` is a single scalar; there is no
//!   multiphase relative-permeability upwinding.
//! - **Vertical well only.** The `WI` uses `h = dz` and the vertical-well
//!   equivalent radius; horizontal/deviated wells are not modelled.
//! - **Isotropic-k well index.** [`PeacemanWell::well_index`] uses the
//!   `r_eq = 0.14*sqrt(dx^2+dy^2)` isotropic form; anisotropic `kx != ky`
//!   correction is documented above but not implemented.

use crate::error::PflotranError;
use crate::grid::CartesianGrid;

/// How a [`PeacemanWell`] is driven each timestep.
///
/// Both variants use the module sign convention: positive rate / positive
/// pressure difference means fluid **entering** the reservoir (injection).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WellControl {
    /// Fixed bottom-hole (sandface) pressure `p_bh`, Pa. Each perforated cell's
    /// rate is then `WI * lambda * (p_bh - p_cell)`.
    BottomHolePressure(f64),
    /// Fixed **total** volumetric rate across all perforations, m^3/s. Positive
    /// injects, negative produces. The total is split among perforated cells in
    /// proportion to their well index `WI`.
    VolumetricRate(f64),
}

/// A Peaceman vertical well perforating one or more Cartesian cells.
///
/// The well couples a single wellbore to every cell in [`PeacemanWell::cells`]
/// through that cell's well index (see the module docs). Physical assumptions:
/// the well is **vertical**, **single-phase**, and each perforated cell uses the
/// **isotropic-k** equivalent radius `r_eq = 0.14*sqrt(dx^2 + dy^2)` with
/// perforated thickness `h = dz`.
///
/// Prefer [`PeacemanWell::new`] to construct one — it validates the geometry —
/// though the fields are public for inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct PeacemanWell {
    /// Linear indices (into the grid) of the perforated cells. Must be non-empty
    /// and each index valid for the grid it is evaluated against. Indices should
    /// be distinct — a repeated cell would be counted twice in rate weighting.
    pub cells: Vec<usize>,
    /// Wellbore radius `r_w`, metres. Must be strictly positive and smaller than
    /// the cell equivalent radius `r_eq` (otherwise `ln(r_eq/r_w) + skin <= 0`
    /// and the well index is unphysical).
    pub radius: f64,
    /// Dimensionless near-well skin factor. Positive = formation damage (lower
    /// `WI`); negative = stimulation (higher `WI`). Must be finite.
    pub skin: f64,
    /// The well's operating control (fixed BHP or fixed total rate).
    pub control: WellControl,
}

impl PeacemanWell {
    /// Construct and validate a Peaceman well.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if `cells` is empty, `radius` is
    /// not strictly positive and finite, `skin` is not finite, or the control's
    /// pressure/rate value is not finite.
    pub fn new(
        cells: Vec<usize>,
        radius: f64,
        skin: f64,
        control: WellControl,
    ) -> Result<Self, PflotranError> {
        if cells.is_empty() {
            return Err(PflotranError::InvalidInput(
                "PeacemanWell has no perforated cells".to_string(),
            ));
        }
        if !radius.is_finite() || radius <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "well radius r_w = {radius} m must be a positive finite length"
            )));
        }
        if !skin.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "well skin factor = {skin} must be finite"
            )));
        }
        let ctrl_value = match control {
            WellControl::BottomHolePressure(p) => p,
            WellControl::VolumetricRate(q) => q,
        };
        if !ctrl_value.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "well control value = {ctrl_value} must be finite"
            )));
        }
        Ok(Self {
            cells,
            radius,
            skin,
            control,
        })
    }

    /// Isotropic-k Peaceman well index for a single cell, m^3.
    ///
    /// Computes `WI = 2*pi * k * h / ( ln(r_eq/r_w) + skin )` with
    /// `r_eq = 0.14*sqrt(dx^2 + dy^2)` and perforated thickness `h = dz`, for a
    /// vertical well through a Cartesian cell of size `(dx, dy, dz)` (m) and
    /// isotropic permeability `permeability` (m^2). `r_w` is the wellbore radius
    /// (m) and `skin` the dimensionless skin factor.
    ///
    /// The returned value has units m^3 (permeability m^2 times thickness m over
    /// the dimensionless log term); multiplying by a mobility `lambda`
    /// (1/(Pa*s)) and a pressure difference (Pa) yields a volumetric rate m^3/s.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if any of `dx, dy, dz,
    /// permeability, r_w` is not strictly positive and finite, if `skin` is not
    /// finite, or if `ln(r_eq/r_w) + skin <= 0` (which happens when the wellbore
    /// radius is not comfortably smaller than the equivalent radius, giving a
    /// non-physical negative or infinite well index).
    pub fn well_index(
        dx: f64,
        dy: f64,
        dz: f64,
        permeability: f64,
        r_w: f64,
        skin: f64,
    ) -> Result<f64, PflotranError> {
        for (name, v) in [
            ("dx", dx),
            ("dy", dy),
            ("dz", dz),
            ("permeability", permeability),
            ("r_w", r_w),
        ] {
            if !v.is_finite() || v <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "well_index: {name} = {v} must be a positive finite value"
                )));
            }
        }
        if !skin.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "well_index: skin = {skin} must be finite"
            )));
        }

        let r_eq = 0.14 * (dx * dx + dy * dy).sqrt();
        let denom = (r_eq / r_w).ln() + skin;
        if denom <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "well_index: ln(r_eq/r_w)+skin = {denom} <= 0 (r_eq = {r_eq} m, \
                 r_w = {r_w} m); wellbore radius must be smaller than the \
                 equivalent radius for a physical well index"
            )));
        }

        let h = dz;
        Ok(2.0 * core::f64::consts::PI * permeability * h / denom)
    }

    /// Per-cell volumetric source/sink rates (m^3/s) this well imposes on the
    /// grid, given the current cell pressures and a uniform single-phase
    /// mobility.
    ///
    /// The returned vector has length `grid.n_cells()` and is zero everywhere
    /// except at this well's perforated cells. Sign convention: positive =
    /// injection into the cell, negative = production. Behaviour by control:
    ///
    /// - [`WellControl::BottomHolePressure`]: each perforated cell gets
    ///   `WI * mobility * (p_bh - p_cell)`.
    /// - [`WellControl::VolumetricRate`]: the prescribed **total** rate is split
    ///   across perforated cells in proportion to their well index `WI`, so the
    ///   returned rates sum to the target total (independent of `mobility`).
    ///
    /// Each cell's `WI` is the isotropic-k Peaceman index (see
    /// [`PeacemanWell::well_index`]); the cell dimensions `(dx, dy, dz)` are read
    /// from `grid`, and `permeability` (m^2) and `mobility` (1/(Pa*s)) are taken
    /// uniform over the well.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if `cell_pressure.len() !=
    /// grid.n_cells()`, if `permeability` is not positive-finite, if `mobility`
    /// is negative or non-finite, if any perforated cell index is out of range,
    /// or if any per-cell well index is invalid (propagated from
    /// [`PeacemanWell::well_index`]).
    pub fn cell_rates(
        &self,
        grid: &CartesianGrid,
        permeability: f64,
        mobility: f64,
        cell_pressure: &[f64],
    ) -> Result<Vec<f64>, PflotranError> {
        let n = grid.n_cells();
        if cell_pressure.len() != n {
            return Err(PflotranError::InvalidInput(format!(
                "cell_rates: cell_pressure length {} != grid.n_cells() {}",
                cell_pressure.len(),
                n
            )));
        }
        if !permeability.is_finite() || permeability <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "cell_rates: permeability = {permeability} must be positive finite"
            )));
        }
        if !mobility.is_finite() || mobility < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "cell_rates: mobility = {mobility} must be non-negative finite"
            )));
        }
        if self.cells.is_empty() {
            return Err(PflotranError::InvalidInput(
                "cell_rates: well has no perforated cells".to_string(),
            ));
        }

        // Reconstruct per-axis cell widths from the grid's cell centres. The
        // grid places the domain origin at edge 0 and centres cell m at the
        // running edge plus half its width, so the widths invert exactly by a
        // forward walk (see CartesianGrid::cell_center / the `centers` helper).
        let (nx, ny, nz) = grid.dims();
        let dx = reconstruct_widths(nx, |i| grid.cell_center(grid.cell_index(i, 0, 0))[0]);
        let dy = reconstruct_widths(ny, |j| grid.cell_center(grid.cell_index(0, j, 0))[1]);
        let dz = reconstruct_widths(nz, |k| grid.cell_center(grid.cell_index(0, 0, k))[2]);

        // Well index at each perforated cell.
        let mut wi = Vec::with_capacity(self.cells.len());
        for &cell in &self.cells {
            if cell >= n {
                return Err(PflotranError::InvalidInput(format!(
                    "cell_rates: perforated cell {cell} out of range (n_cells = {n})"
                )));
            }
            let (i, j, k) = grid.cell_ijk(cell);
            let w = Self::well_index(dx[i], dy[j], dz[k], permeability, self.radius, self.skin)?;
            wi.push(w);
        }

        let mut rates = vec![0.0_f64; n];
        match self.control {
            WellControl::BottomHolePressure(p_bh) => {
                for (idx, &cell) in self.cells.iter().enumerate() {
                    rates[cell] += wi[idx] * mobility * (p_bh - cell_pressure[cell]);
                }
            }
            WellControl::VolumetricRate(q_total) => {
                let wi_sum: f64 = wi.iter().sum();
                // wi_sum > 0 always holds here: every wi came from well_index,
                // which returns Err unless it is strictly positive.
                for (idx, &cell) in self.cells.iter().enumerate() {
                    rates[cell] += q_total * wi[idx] / wi_sum;
                }
            }
        }
        Ok(rates)
    }
}

/// Reconstruct the per-cell widths along one axis from its cell-centre
/// coordinates, inverting [`CartesianGrid`]'s centring (origin at edge 0, cell
/// `m` centred at the running edge plus half its width).
fn reconstruct_widths<F: Fn(usize) -> f64>(n: usize, center: F) -> Vec<f64> {
    let mut widths = Vec::with_capacity(n);
    let mut edge = 0.0_f64;
    for m in 0..n {
        let w = 2.0 * (center(m) - edge);
        widths.push(w);
        edge += w;
    }
    widths
}

/// A single per-cell source/sink term: a fixed volumetric or mass rate applied
/// to one grid cell.
///
/// Distinct from [`PeacemanWell`], which derives its rate from a pressure
/// difference and a well index; a `SourceSink` is a directly prescribed rate
/// (e.g. a recharge flux or a tracer injection). Sign convention: positive =
/// mass/fluid **entering** the cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SourceSink {
    /// A prescribed **volumetric** rate at a cell, m^3/s (positive = injection).
    Volumetric {
        /// Linear cell index the rate is applied to.
        cell: usize,
        /// Volumetric rate, m^3/s.
        rate: f64,
    },
    /// A prescribed **mass** rate at a cell, kg/s (positive = injection).
    Mass {
        /// Linear cell index the rate is applied to.
        cell: usize,
        /// Mass rate, kg/s.
        rate: f64,
    },
}

impl SourceSink {
    /// The cell this term is applied to.
    pub fn cell(&self) -> usize {
        match self {
            SourceSink::Volumetric { cell, .. } => *cell,
            SourceSink::Mass { cell, .. } => *cell,
        }
    }

    /// The **volumetric** rate (m^3/s) of this term, converting a mass rate with
    /// the supplied fluid density `density` (kg/m^3).
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if the rate is non-finite, or —
    /// for a [`SourceSink::Mass`] term — if `density` is not strictly positive
    /// and finite.
    pub fn volumetric_rate(&self, density: f64) -> Result<f64, PflotranError> {
        match self {
            SourceSink::Volumetric { rate, .. } => {
                if !rate.is_finite() {
                    return Err(PflotranError::InvalidInput(format!(
                        "SourceSink volumetric rate = {rate} must be finite"
                    )));
                }
                Ok(*rate)
            }
            SourceSink::Mass { rate, .. } => {
                if !rate.is_finite() {
                    return Err(PflotranError::InvalidInput(format!(
                        "SourceSink mass rate = {rate} must be finite"
                    )));
                }
                if !density.is_finite() || density <= 0.0 {
                    return Err(PflotranError::InvalidInput(format!(
                        "SourceSink: density = {density} kg/m^3 must be positive finite \
                         to convert a mass rate to volumetric"
                    )));
                }
                Ok(rate / density)
            }
        }
    }
}

/// Advanced (state/space/time-dependent) boundary-condition value types.
///
/// Each variant is evaluated through [`AdvancedBc::value`], which returns the
/// boundary pressure (Pa) or forcing value to impose at a given elevation,
/// current cell pressure, and time. See the per-variant docs for the physics
/// and the seepage-face encoding.
#[derive(Debug, Clone, PartialEq)]
pub enum AdvancedBc {
    /// Hydrostatic Dirichlet pressure about a datum:
    /// `p(z) = pressure_datum + density*gravity*(z_datum - z)`.
    ///
    /// Pressure increases downward (decreasing `z`). `pressure_datum` is the
    /// pressure (Pa) at elevation `z_datum` (m); `density` is the fluid density
    /// (kg/m^3) and `gravity` the gravitational acceleration (m/s^2, a positive
    /// magnitude).
    Hydrostatic {
        /// Pressure at the datum elevation, Pa.
        pressure_datum: f64,
        /// Datum elevation `z_datum`, m.
        z_datum: f64,
        /// Fluid density `rho`, kg/m^3.
        density: f64,
        /// Gravitational acceleration `g`, m/s^2 (positive magnitude).
        gravity: f64,
    },
    /// Seepage-face condition: outflow is permitted only where the cell pressure
    /// exceeds `atmospheric_pressure`; elsewhere the face is a no-flow boundary.
    /// A state-dependent switch — see [`AdvancedBc::value`] for the encoding.
    SeepageFace {
        /// Atmospheric (reference) pressure `p_atm`, Pa.
        atmospheric_pressure: f64,
    },
    /// Time-varying forcing given as a table of `(time_s, value)` pairs, with
    /// linear interpolation between successive points and flat clamping outside
    /// the tabulated time range. The table must be sorted by ascending time; an
    /// empty table evaluates to `0.0`.
    TimeVarying {
        /// `(time_s, value)` samples, sorted by ascending `time_s`.
        table: Vec<(f64, f64)>,
    },
}

impl AdvancedBc {
    /// Construct and validate a [`AdvancedBc::TimeVarying`] table.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if the table is empty, contains a
    /// non-finite time or value, or is not sorted by strictly ascending time.
    pub fn time_varying(table: Vec<(f64, f64)>) -> Result<Self, PflotranError> {
        if table.is_empty() {
            return Err(PflotranError::InvalidInput(
                "TimeVarying BC table is empty".to_string(),
            ));
        }
        for &(t, v) in &table {
            if !t.is_finite() || !v.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "TimeVarying BC table has a non-finite entry (t = {t}, value = {v})"
                )));
            }
        }
        for pair in table.windows(2) {
            if pair[1].0 <= pair[0].0 {
                return Err(PflotranError::InvalidInput(format!(
                    "TimeVarying BC table times must strictly increase (found {} then {})",
                    pair[0].0, pair[1].0
                )));
            }
        }
        Ok(AdvancedBc::TimeVarying { table })
    }

    /// Boundary pressure / forcing value at elevation `z` (m), current
    /// `cell_pressure` (Pa), and time `time` (s).
    ///
    /// Which arguments matter depends on the variant:
    ///
    /// - **Hydrostatic** uses only `z`: returns
    ///   `pressure_datum + density*gravity*(z_datum - z)` (Pa).
    /// - **SeepageFace** uses only `cell_pressure`. **Encoding:** when
    ///   `cell_pressure > atmospheric_pressure` the face is *active* (open,
    ///   outflow permitted) and this returns `cell_pressure` — a finite Dirichlet
    ///   value equal to the cell pressure, so no spurious gradient is imposed.
    ///   When `cell_pressure <= atmospheric_pressure` the face is *closed*
    ///   (no-flow) and this returns [`f64::NAN`] as an explicit no-flow sentinel:
    ///   a caller must branch on `value.is_nan()` (no-flow) versus finite
    ///   (active Dirichlet), never treat the result as an ordinary pressure.
    /// - **TimeVarying** uses only `time`: linear interpolation of the table,
    ///   clamped flat outside the tabulated range; an empty table gives `0.0`.
    pub fn value(&self, z: f64, cell_pressure: f64, time: f64) -> f64 {
        match self {
            AdvancedBc::Hydrostatic {
                pressure_datum,
                z_datum,
                density,
                gravity,
            } => pressure_datum + density * gravity * (z_datum - z),
            AdvancedBc::SeepageFace {
                atmospheric_pressure,
            } => {
                if cell_pressure > *atmospheric_pressure {
                    // Active seepage face: open, held at the cell pressure.
                    cell_pressure
                } else {
                    // Closed face: no-flow. NaN is the explicit sentinel.
                    f64::NAN
                }
            }
            AdvancedBc::TimeVarying { table } => interpolate_table(table, time),
        }
    }
}

/// Linear interpolation of a `(time, value)` table at `t`, clamped flat outside
/// the tabulated range. An empty table returns `0.0`; a single-point table
/// returns that point's value. The table is assumed sorted by ascending time.
fn interpolate_table(table: &[(f64, f64)], t: f64) -> f64 {
    if table.is_empty() {
        return 0.0;
    }
    if t <= table[0].0 {
        return table[0].1;
    }
    let last = table.len() - 1;
    if t >= table[last].0 {
        return table[last].1;
    }
    for pair in table.windows(2) {
        let (t0, v0) = pair[0];
        let (t1, v1) = pair[1];
        if t >= t0 && t <= t1 {
            let span = t1 - t0;
            if span <= 0.0 {
                return v0;
            }
            let frac = (t - t0) / span;
            return v0 + frac * (v1 - v0);
        }
    }
    // Unreachable for a sorted table given the clamps above; fall back safely.
    table[last].1
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    /// Build a rectilinear grid from explicit per-cell width arrays (m).
    fn grid(dx: Vec<f64>, dy: Vec<f64>, dz: Vec<f64>) -> CartesianGrid {
        CartesianGrid::rectilinear(dx, dy, dz).expect("valid grid")
    }

    #[test]
    fn well_index_matches_isotropic_closed_form() {
        let dx = 10.0;
        let dy = 10.0;
        let dz = 5.0;
        let k = 1.0e-12;
        let r_w = 0.1;
        let skin = 0.0;

        let wi = PeacemanWell::well_index(dx, dy, dz, k, r_w, skin).unwrap();

        // Hand-evaluated reference: r_eq = 0.14*sqrt(dx^2+dy^2), WI = 2 pi k h / ln(r_eq/r_w).
        let r_eq = 0.14 * (dx * dx + dy * dy).sqrt();
        let expected = 2.0 * core::f64::consts::PI * k * dz / (r_eq / r_w).ln();
        assert!(
            (wi - expected).abs() <= expected.abs() * 1e-12,
            "wi = {wi}, expected = {expected}"
        );
    }

    #[test]
    fn well_index_increases_with_permeability() {
        let lo = PeacemanWell::well_index(10.0, 10.0, 5.0, 1.0e-13, 0.1, 0.0).unwrap();
        let hi = PeacemanWell::well_index(10.0, 10.0, 5.0, 1.0e-12, 0.1, 0.0).unwrap();
        assert!(hi > lo, "higher k should give larger WI: hi = {hi}, lo = {lo}");
        // WI is linear in k: 10x permeability -> 10x WI.
        assert!((hi / lo - 10.0).abs() < 1e-9);
    }

    #[test]
    fn well_index_rejects_bad_inputs() {
        // Non-positive geometry / permeability.
        assert!(PeacemanWell::well_index(0.0, 10.0, 5.0, 1e-12, 0.1, 0.0).is_err());
        assert!(PeacemanWell::well_index(10.0, 10.0, 5.0, -1e-12, 0.1, 0.0).is_err());
        // r_w >= r_eq makes ln(r_eq/r_w) <= 0 -> unphysical.
        let r_eq = 0.14 * (10.0f64 * 10.0 + 10.0 * 10.0).sqrt();
        assert!(PeacemanWell::well_index(10.0, 10.0, 5.0, 1e-12, r_eq * 2.0, 0.0).is_err());
    }

    #[test]
    fn bhp_well_sign_convention() {
        let g = grid(vec![10.0], vec![10.0], vec![5.0]);
        let p_cell = 1.0e6;

        // Injection: p_bh > p_cell -> positive rate.
        let inj = PeacemanWell::new(
            vec![0],
            0.1,
            0.0,
            WellControl::BottomHolePressure(2.0e6),
        )
        .unwrap();
        let r_inj = inj.cell_rates(&g, 1.0e-12, 1.0e-3, &[p_cell]).unwrap();
        assert_eq!(r_inj.len(), 1);
        assert!(r_inj[0] > 0.0, "injection should be positive, got {}", r_inj[0]);

        // Production: p_bh < p_cell -> negative rate.
        let prod = PeacemanWell::new(
            vec![0],
            0.1,
            0.0,
            WellControl::BottomHolePressure(0.5e6),
        )
        .unwrap();
        let r_prod = prod.cell_rates(&g, 1.0e-12, 1.0e-3, &[p_cell]).unwrap();
        assert!(r_prod[0] < 0.0, "production should be negative, got {}", r_prod[0]);

        // Balanced: p_bh == p_cell -> zero rate.
        let bal = PeacemanWell::new(
            vec![0],
            0.1,
            0.0,
            WellControl::BottomHolePressure(p_cell),
        )
        .unwrap();
        let r_bal = bal.cell_rates(&g, 1.0e-12, 1.0e-3, &[p_cell]).unwrap();
        assert!(r_bal[0].abs() < EPS, "balanced rate should be ~0, got {}", r_bal[0]);
    }

    #[test]
    fn rate_controlled_sums_to_target_and_splits_by_wi() {
        // Two stacked cells with dz = 5 and dz = 10 -> WI ratio 1:2 (WI ~ h).
        let g = grid(vec![10.0], vec![10.0], vec![5.0, 10.0]);
        assert_eq!(g.n_cells(), 2);

        let q_total = -3.0e-4; // production
        let well = PeacemanWell::new(
            vec![0, 1],
            0.1,
            0.0,
            WellControl::VolumetricRate(q_total),
        )
        .unwrap();
        let rates = well.cell_rates(&g, 1.0e-12, 1.0e-3, &[1.0e6, 1.0e6]).unwrap();

        // Sum equals the target total.
        assert!((rates[0] + rates[1] - q_total).abs() < 1e-12);
        // WI(cell1)/WI(cell0) = dz1/dz0 = 2, so rate1 = 2 * rate0.
        assert!((rates[1] / rates[0] - 2.0).abs() < 1e-9);
    }

    #[test]
    fn rate_controlled_is_mobility_independent() {
        let g = grid(vec![10.0], vec![10.0], vec![5.0]);
        let well = PeacemanWell::new(
            vec![0],
            0.1,
            0.0,
            WellControl::VolumetricRate(2.5e-4),
        )
        .unwrap();
        let a = well.cell_rates(&g, 1.0e-12, 1.0e-3, &[1.0e6]).unwrap();
        let b = well.cell_rates(&g, 1.0e-12, 9.9e-1, &[1.0e6]).unwrap();
        assert!((a[0] - b[0]).abs() < 1e-15);
        assert!((a[0] - 2.5e-4).abs() < 1e-15);
    }

    #[test]
    fn cell_rates_validates_pressure_length_and_indices() {
        let g = grid(vec![10.0], vec![10.0], vec![5.0]);
        let well = PeacemanWell::new(
            vec![0],
            0.1,
            0.0,
            WellControl::BottomHolePressure(2.0e6),
        )
        .unwrap();
        // Wrong pressure-array length.
        assert!(well.cell_rates(&g, 1.0e-12, 1.0e-3, &[1.0e6, 2.0e6]).is_err());
        // Out-of-range perforated cell.
        let bad = PeacemanWell::new(vec![5], 0.1, 0.0, WellControl::BottomHolePressure(2.0e6)).unwrap();
        assert!(bad.cell_rates(&g, 1.0e-12, 1.0e-3, &[1.0e6]).is_err());
        // Negative mobility.
        assert!(well.cell_rates(&g, 1.0e-12, -1.0, &[1.0e6]).is_err());
    }

    #[test]
    fn constructor_validation() {
        assert!(PeacemanWell::new(vec![], 0.1, 0.0, WellControl::BottomHolePressure(1.0)).is_err());
        assert!(PeacemanWell::new(vec![0], 0.0, 0.0, WellControl::BottomHolePressure(1.0)).is_err());
        assert!(PeacemanWell::new(vec![0], -0.1, 0.0, WellControl::BottomHolePressure(1.0)).is_err());
        assert!(
            PeacemanWell::new(vec![0], 0.1, f64::NAN, WellControl::BottomHolePressure(1.0)).is_err()
        );
        assert!(PeacemanWell::new(vec![0], 0.1, 0.0, WellControl::VolumetricRate(1.0)).is_ok());
    }

    #[test]
    fn hydrostatic_increases_downward() {
        let bc = AdvancedBc::Hydrostatic {
            pressure_datum: 1.0e5,
            z_datum: 0.0,
            density: 1000.0,
            gravity: 9.81,
        };
        // Deeper (smaller z) is higher pressure.
        let p_top = bc.value(0.0, 0.0, 0.0);
        let p_deep = bc.value(-10.0, 0.0, 0.0);
        assert!((p_top - 1.0e5).abs() < EPS);
        // p increases by rho*g*dz over 10 m of depth.
        let expected = 1.0e5 + 1000.0 * 9.81 * 10.0;
        assert!((p_deep - expected).abs() < 1e-6, "p_deep = {p_deep}, expected = {expected}");
        assert!(p_deep > p_top);
    }

    #[test]
    fn seepage_face_switch_encoding() {
        let bc = AdvancedBc::SeepageFace {
            atmospheric_pressure: 1.0e5,
        };
        // Above atmospheric: active, returns the cell pressure (finite).
        let active = bc.value(0.0, 1.2e5, 0.0);
        assert!(active.is_finite());
        assert!((active - 1.2e5).abs() < EPS);
        // At or below atmospheric: no-flow, returns NaN sentinel.
        assert!(bc.value(0.0, 1.0e5, 0.0).is_nan());
        assert!(bc.value(0.0, 0.9e5, 0.0).is_nan());
    }

    #[test]
    fn time_varying_interpolates_and_clamps() {
        let bc = AdvancedBc::time_varying(vec![(0.0, 10.0), (10.0, 20.0)]).unwrap();
        // Midpoint linear interpolation.
        assert!((bc.value(0.0, 0.0, 5.0) - 15.0).abs() < EPS);
        // Quarter point.
        assert!((bc.value(0.0, 0.0, 2.5) - 12.5).abs() < EPS);
        // Clamp below and above the tabulated range.
        assert!((bc.value(0.0, 0.0, -1.0) - 10.0).abs() < EPS);
        assert!((bc.value(0.0, 0.0, 100.0) - 20.0).abs() < EPS);
    }

    #[test]
    fn time_varying_validation() {
        assert!(AdvancedBc::time_varying(vec![]).is_err());
        // Non-increasing times.
        assert!(AdvancedBc::time_varying(vec![(1.0, 0.0), (1.0, 1.0)]).is_err());
        assert!(AdvancedBc::time_varying(vec![(2.0, 0.0), (1.0, 1.0)]).is_err());
        // Non-finite entry.
        assert!(AdvancedBc::time_varying(vec![(0.0, f64::NAN)]).is_err());
        assert!(AdvancedBc::time_varying(vec![(0.0, 1.0), (5.0, 2.0)]).is_ok());
    }

    #[test]
    fn source_sink_mass_to_volumetric() {
        let vol = SourceSink::Volumetric { cell: 3, rate: 2.0e-4 };
        assert_eq!(vol.cell(), 3);
        assert!((vol.volumetric_rate(1000.0).unwrap() - 2.0e-4).abs() < EPS);

        let mass = SourceSink::Mass { cell: 1, rate: 1.0 };
        assert_eq!(mass.cell(), 1);
        // 1 kg/s at 1000 kg/m^3 -> 1e-3 m^3/s.
        assert!((mass.volumetric_rate(1000.0).unwrap() - 1.0e-3).abs() < EPS);
        // Bad density for a mass term.
        assert!(mass.volumetric_rate(0.0).is_err());
    }
}
