//! Geometry, boundary conditions and per-face bookkeeping for the SANM path.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! This file has no single `.m` counterpart. It gives Rust types to the pieces
//! of Yan Ren's `geometry` and `params` structs that every ported nodal file
//! reads, and to the `handle3dcoords.m` coordinate lookup:
//!
//! - `geometry.Lx/Ly/Lz/Vi` and the `geometry.{x,y,z}{min,max}` boundary-
//!   condition strings ⇒ [`NodalGeometry`] and [`BoundaryCondition`].
//! - `geometry.{x,y,z}lows` / `{x,y,z}his`, the per-column active index range
//!   used to skip out-of-core nodes ⇒ [`ActiveRange`].
//! - `geometry.adf`, the `philen`×6 assembly-discontinuity-factor table, and
//!   the `philen`×6 `gradterms` / `nodalterms` tables ⇒ [`FaceTerms`].
//! - `geometry.nodalcoeffs` ⇒ [`NodalCoefficients`] (built in
//!   [`super::nodal_coefficients`]).
//! - `params.G` / `params.maxi{x,y,z}` / `params.Nc` ⇒ [`NodalParams`] plus
//!   [`Grid`](crate::reference::grid::Grid).
//!
//! # Units
//!
//! All lengths are centimetres and all volumes cubic centimetres, the units the
//! benchmark specifications and the MATLAB both use. `uom` types are
//! deliberately not used inside the reference translation, so the arithmetic
//! stays line-for-line comparable with the original.

use crate::reference::grid::Grid;

/// Which face of a node a per-face quantity belongs to.
///
/// The MATLAB stores these as columns 1–6 of the `philen`×6 arrays `gradterms`,
/// `nodalterms` and `geometry.adf`. [`Face::column`] is the 0-based column, so
/// `Face::XMinus.column() == 0` is MATLAB column 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// Low-x face. MATLAB column 1.
    XMinus,
    /// High-x face. MATLAB column 2.
    XPlus,
    /// Low-y face. MATLAB column 3.
    YMinus,
    /// High-y face. MATLAB column 4.
    YPlus,
    /// Low-z face. MATLAB column 5.
    ZMinus,
    /// High-z face. MATLAB column 6.
    ZPlus,
}

impl Face {
    /// The 0-based column this face occupies in a [`FaceTerms`] table.
    #[must_use]
    pub const fn column(self) -> usize {
        match self {
            Self::XMinus => 0,
            Self::XPlus => 1,
            Self::YMinus => 2,
            Self::YPlus => 3,
            Self::ZMinus => 4,
            Self::ZPlus => 5,
        }
    }

    /// The low-side face of `axis`.
    #[must_use]
    pub const fn minus(axis: Axis) -> Self {
        match axis {
            Axis::X => Self::XMinus,
            Axis::Y => Self::YMinus,
            Axis::Z => Self::ZMinus,
        }
    }

    /// The high-side face of `axis`.
    #[must_use]
    pub const fn plus(axis: Axis) -> Self {
        match axis {
            Axis::X => Self::XPlus,
            Axis::Y => Self::YPlus,
            Axis::Z => Self::ZPlus,
        }
    }
}

/// Which coordinate direction a quantity is taken along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// x.
    X,
    /// y.
    Y,
    /// z.
    Z,
}

impl Axis {
    /// Number of nodes along this axis.
    #[must_use]
    pub const fn node_count(self, grid: Grid) -> usize {
        match self {
            Self::X => grid.nx,
            Self::Y => grid.ny,
            Self::Z => grid.nz,
        }
    }

    /// Extents of the two indices that identify a line parallel to this axis.
    ///
    /// The MATLAB sweeps each direction with a doubly-nested loop over the
    /// *other* two indices: `(ix, iy)` for z, `(ix, iz)` for y, `(iy, iz)` for
    /// x. Those pairs are also the keys of the matching [`ActiveRange`].
    #[must_use]
    pub const fn line_counts(self, grid: Grid) -> (usize, usize) {
        match self {
            Self::X => (grid.ny, grid.nz),
            Self::Y => (grid.nx, grid.nz),
            Self::Z => (grid.nx, grid.ny),
        }
    }

    /// The `(ix, iy, iz)` of the node at `pos` along this axis on line
    /// `(k1, k2)`, with `(k1, k2)` as described by [`Axis::line_counts`].
    #[must_use]
    pub const fn coords(self, k1: usize, k2: usize, pos: usize) -> (usize, usize, usize) {
        match self {
            Self::X => (pos, k1, k2),
            Self::Y => (k1, pos, k2),
            Self::Z => (k1, k2, pos),
        }
    }

    /// The state-vector stride to the next node along this axis: `1` in z,
    /// `nz` in y, `ny*nz` in x.
    #[must_use]
    pub const fn stride(self, grid: Grid) -> usize {
        match self {
            Self::X => grid.ny * grid.nz,
            Self::Y => grid.nz,
            Self::Z => 1,
        }
    }
}

/// The outer-boundary condition on one face of the core.
///
/// Translates the MATLAB string fields `geometry.xmin`, `geometry.xmax`,
/// `geometry.ymin`, `geometry.ymax`, `geometry.zmin`, `geometry.zmax`, whose
/// only recognised values are `'vacuum'`, `'reflective'` and `'zeroflux'`.
///
/// # Unfinished in the reference
///
/// The MATLAB `switch` statements have no `otherwise` branch. An unrecognised
/// string therefore silently leaves the coefficient at whatever it was
/// initialised to — usually zero — rather than raising an error. The enum makes
/// that class of typo unrepresentable; the behaviour for the three real values
/// is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryCondition {
    /// Zero incoming partial current (`'vacuum'`).
    Vacuum,
    /// Zero net current (`'reflective'`).
    Reflective,
    /// Zero flux at the surface (`'zeroflux'`).
    ZeroFlux,
}

/// The six outer-boundary conditions of a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryConditions {
    /// `geometry.xmin`.
    pub x_min: BoundaryCondition,
    /// `geometry.xmax`.
    pub x_max: BoundaryCondition,
    /// `geometry.ymin`.
    pub y_min: BoundaryCondition,
    /// `geometry.ymax`.
    pub y_max: BoundaryCondition,
    /// `geometry.zmin`.
    pub z_min: BoundaryCondition,
    /// `geometry.zmax`.
    pub z_max: BoundaryCondition,
}

impl BoundaryConditions {
    /// All six faces set to the same condition.
    #[must_use]
    pub const fn uniform(bc: BoundaryCondition) -> Self {
        Self {
            x_min: bc,
            x_max: bc,
            y_min: bc,
            y_max: bc,
            z_min: bc,
            z_max: bc,
        }
    }

    /// The low-side condition along `axis`.
    #[must_use]
    pub const fn low(&self, axis: Axis) -> BoundaryCondition {
        match axis {
            Axis::X => self.x_min,
            Axis::Y => self.y_min,
            Axis::Z => self.z_min,
        }
    }

    /// The high-side condition along `axis`.
    #[must_use]
    pub const fn high(&self, axis: Axis) -> BoundaryCondition {
        match axis {
            Axis::X => self.x_max,
            Axis::Y => self.y_max,
            Axis::Z => self.z_max,
        }
    }
}

/// The first and last in-core node index along one axis, for every line of
/// nodes parallel to that axis.
///
/// Translates the MATLAB pairs `geometry.zlows`/`geometry.zhis` (indexed by
/// `ix,iy`), `geometry.ylows`/`geometry.yhis` (indexed by `ix,iz`) and
/// `geometry.xlows`/`geometry.xhis` (indexed by `iy,iz`). Where the MATLAB
/// falls back to `ones(...)` / `maxi*(ones(...))` when the field is absent,
/// [`ActiveRange::full`] does the same.
///
/// Indices stored here are **0-based**; the MATLAB values are 1-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRange {
    /// Extent of the first index of the (first, second) key pair.
    first_len: usize,
    /// Extent of the second key index.
    second_len: usize,
    lows: Vec<usize>,
    highs: Vec<usize>,
}

impl ActiveRange {
    /// Every line spans the whole axis: low `0`, high `n-1`.
    ///
    /// `first_len` × `second_len` is the number of lines; `n` is the number of
    /// nodes along the axis being ranged over.
    #[must_use]
    pub fn full(first_len: usize, second_len: usize, n: usize) -> Self {
        Self {
            first_len,
            second_len,
            lows: vec![0; first_len * second_len],
            highs: vec![n - 1; first_len * second_len],
        }
    }

    /// Explicit per-line bounds, both **0-based** and inclusive.
    ///
    /// `lows` and `highs` are indexed `first * second_len + second`.
    ///
    /// # Panics
    ///
    /// If either vector length differs from `first_len * second_len`.
    #[must_use]
    pub fn new(first_len: usize, second_len: usize, lows: Vec<usize>, highs: Vec<usize>) -> Self {
        assert_eq!(lows.len(), first_len * second_len, "lows length");
        assert_eq!(highs.len(), first_len * second_len, "highs length");
        Self {
            first_len,
            second_len,
            lows,
            highs,
        }
    }

    /// First in-core index on the line keyed by `(first, second)`.
    ///
    /// # Panics
    ///
    /// If either key is out of range.
    #[must_use]
    pub fn low(&self, first: usize, second: usize) -> usize {
        assert!(
            first < self.first_len && second < self.second_len,
            "key OOB"
        );
        self.lows[first * self.second_len + second]
    }

    /// Last in-core index on the line keyed by `(first, second)`.
    ///
    /// # Panics
    ///
    /// If either key is out of range.
    #[must_use]
    pub fn high(&self, first: usize, second: usize) -> usize {
        assert!(
            first < self.first_len && second < self.second_len,
            "key OOB"
        );
        self.highs[first * self.second_len + second]
    }
}

/// A `philen`×6 table of per-face values, one row per state-vector index.
///
/// Three MATLAB arrays share this shape and column order and so share this
/// type: `gradterms` (finite-difference coupling coefficients from
/// `makegradDxyz.m`, dimensionless once divided by a node width),
/// `nodalterms` (the nodal correction coefficients from `calc_sanodalxyz.m`,
/// same units), and `geometry.adf` (assembly discontinuity factors,
/// dimensionless, default 1).
#[derive(Debug, Clone, PartialEq)]
pub struct FaceTerms {
    rows: usize,
    data: Vec<f64>,
}

impl FaceTerms {
    /// A table of `rows` rows, every entry zero — MATLAB `zeros(philen,6)`.
    #[must_use]
    pub fn zeros(rows: usize) -> Self {
        Self {
            rows,
            data: vec![0.0; rows * 6],
        }
    }

    /// A table of `rows` rows, every entry one — MATLAB `ones(philen,6)`, the
    /// default assembly-discontinuity-factor table.
    #[must_use]
    pub fn ones(rows: usize) -> Self {
        Self {
            rows,
            data: vec![1.0; rows * 6],
        }
    }

    /// Number of rows (state-vector length).
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Value at state index `idx` on `face`.
    ///
    /// # Panics
    ///
    /// If `idx >= rows()`.
    #[must_use]
    pub fn get(&self, idx: usize, face: Face) -> f64 {
        assert!(idx < self.rows, "row {idx} >= {}", self.rows);
        self.data[idx * 6 + face.column()]
    }

    /// Overwrites the value at state index `idx` on `face`.
    ///
    /// # Panics
    ///
    /// If `idx >= rows()`.
    pub fn set(&mut self, idx: usize, face: Face, value: f64) {
        assert!(idx < self.rows, "row {idx} >= {}", self.rows);
        self.data[idx * 6 + face.column()] = value;
    }

    /// Multiplies every entry by `factor` — MATLAB `gradterms=2*gradterms`.
    pub fn scale(&mut self, factor: f64) {
        for v in &mut self.data {
            *v *= factor;
        }
    }
}

/// A quantity carried once per coordinate direction, each a full state vector.
///
/// The MATLAB writes these as structs with `.x`, `.y` and `.z` fields —
/// `Leakage`, `Ssource`, `A2`, `A4`, `diffvec`, `bdummy` and friends. The
/// physical quantity depends on which one: transverse leakages are in
/// neutrons cm⁻³ s⁻¹, the expansion coefficients `A2`/`A4` are in the same
/// units as the flux, and `diffvec` is in cm⁻¹.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectionVectors {
    /// x-direction values, one per state index.
    pub x: Vec<f64>,
    /// y-direction values, one per state index.
    pub y: Vec<f64>,
    /// z-direction values, one per state index.
    pub z: Vec<f64>,
}

impl DirectionVectors {
    /// Three zero vectors of length `n`.
    #[must_use]
    pub fn zeros(n: usize) -> Self {
        Self {
            x: vec![0.0; n],
            y: vec![0.0; n],
            z: vec![0.0; n],
        }
    }

    /// The component along `axis`.
    #[must_use]
    pub fn axis(&self, axis: Axis) -> &[f64] {
        match axis {
            Axis::X => &self.x,
            Axis::Y => &self.y,
            Axis::Z => &self.z,
        }
    }

    /// The component along `axis`, mutably.
    pub fn axis_mut(&mut self, axis: Axis) -> &mut Vec<f64> {
        match axis {
            Axis::X => &mut self.x,
            Axis::Y => &mut self.y,
            Axis::Z => &mut self.z,
        }
    }
}

/// Solver-shape parameters — the fields of Yan Ren's `params` struct that the
/// nodal path reads.
///
/// The node counts and group count live in [`Grid`]; this carries what is left.
#[derive(Debug, Clone, PartialEq)]
pub struct NodalParams {
    /// The node grid and energy-group count. MATLAB `params.maxi{x,y,z}`,
    /// `params.G`.
    pub grid: Grid,
    /// Number of delayed-neutron precursor groups appended to the state vector.
    /// MATLAB `params.Nc`, defaulting to 0 when the field is absent.
    ///
    /// # Unfinished in the reference
    ///
    /// `Nc > 0` does not work in the MATLAB. `makegradDxyz.m` and
    /// `makesigmadfxyz.m` build `(G+Nc)*nodes`-square operators, but
    /// `calc_sanodalxyz.m` returns a `G*nodes`-square one and
    /// `calc_transleakagexyz.m` multiplies a `G*nodes`-wide operator by the
    /// `(G+Nc)*nodes`-long flux — both of which raise a dimension error in
    /// MATLAB. Translated as-is: the corresponding Rust operations panic on the
    /// same mismatch. Only `Nc == 0` is reachable.
    pub n_precursor_groups: usize,
    /// Source-iteration convergence tolerance on both the fission-source
    /// residual and the `k_eff` residual. MATLAB `diffusion.tol`, overridden by
    /// `params.innertol` when that is set and positive.
    pub inner_tolerance: f64,
    /// Source iterations between rebuilds of the nodal correction matrix.
    /// MATLAB `nodalupd`, default `ceil((maxix+maxiy+maxiz)/10)`, overridden by
    /// a nonzero `params.nodalupd`.
    ///
    /// # An interval of 1 destabilises the iteration
    ///
    /// The MATLAB comment reads: "Smaller values reduce the lag between the
    /// flux shape and the nodal correction matrix, improving stability at the
    /// cost of extra factorisations." **In this port the opposite is observed
    /// at an interval of exactly 1**, where the correction is rebuilt from the
    /// flux that was just computed from it. On a homogeneous leaking cube
    /// (20 cm nodes, one group, `k_inf = 1`) the source iteration then fails to
    /// settle and hits the 5000-iteration ceiling, while an interval of 2 or
    /// more converges to within 1×10⁻³ of the finite-difference answer at every
    /// mesh size tried (3³ to 11³). See the tests in
    /// [`super::sanm_solver`].
    ///
    /// This matters because the default `ceil((nx+ny+nz)/10)` **is** 1 for any
    /// mesh with `nx+ny+nz <= 10`. Every benchmark in the snapshot is far
    /// larger (IAEA-3D gives 6), so the reference never hits it in anger.
    ///
    /// Recorded, not repaired, per the translation rules. It has **not** been
    /// confirmed that the MATLAB behaves the same way — the reference has not
    /// been run (`docs/bedok-port-scoping.md` §4), so this is a property of the
    /// port, and whether it is also a property of the original is open.
    pub nodal_update_interval: usize,
    /// Source iterations between fission-source extrapolations. MATLAB `fsexp`,
    /// default 5, overridden by a nonzero `params.fsexp`.
    pub fission_extrapolation_interval: usize,
}

impl NodalParams {
    /// Parameters with the MATLAB defaults for a given grid.
    ///
    /// Reproduces the defaults set at the top of `sanodaldiffusion_solverxyz.m`:
    /// tolerance `1e-6`, `nodalupd = ceil((maxix+maxiy+maxiz)/10)`, `fsexp = 5`,
    /// `Nc = 0`.
    #[must_use]
    pub fn with_matlab_defaults(grid: Grid) -> Self {
        let span = grid.nx + grid.ny + grid.nz;
        Self {
            grid,
            n_precursor_groups: 0,
            inner_tolerance: 1e-6,
            // ceil(span/10) in integer arithmetic.
            nodal_update_interval: span.div_ceil(10),
            fission_extrapolation_interval: 5,
        }
    }

    /// Length of a neutronics state vector, `G*nodes`. MATLAB `philen`.
    #[must_use]
    pub const fn philen(&self) -> usize {
        self.grid.state_len()
    }

    /// Length of the full state vector including precursors,
    /// `(G+Nc)*nodes`. MATLAB `philenf`.
    #[must_use]
    pub const fn philenf(&self) -> usize {
        (self.grid.ngroups + self.n_precursor_groups) * self.grid.nodes()
    }
}

/// Node dimensions, boundary conditions and the in-core index ranges the SANM
/// path needs — Yan Ren's `geometry` struct, minus the fields only the
/// thermal-hydraulic side reads.
///
/// `lx`, `ly`, `lz` and `volume` are indexed by **spatial** node index
/// (`ix*ny*nz + iy*nz + iz`), matching the MATLAB's `geometry.Lx` etc. before
/// the `repmat(...,G,1)` lift to full state length.
#[derive(Debug, Clone, PartialEq)]
pub struct NodalGeometry {
    /// Node width in x \[cm\], one per spatial node. MATLAB `geometry.Lx`.
    pub lx: Vec<f64>,
    /// Node width in y \[cm\], one per spatial node. MATLAB `geometry.Ly`.
    pub ly: Vec<f64>,
    /// Node height in z \[cm\], one per spatial node. MATLAB `geometry.Lz`.
    pub lz: Vec<f64>,
    /// Node volume \[cm³\], one per spatial node. MATLAB `geometry.Vi`.
    pub volume: Vec<f64>,
    /// Material index per spatial node, **1-based**, `0` meaning "no material
    /// here". MATLAB `geometry.whichsigma` / the `whichsigma` argument.
    pub which_sigma: Vec<usize>,
    /// Outer-boundary conditions.
    pub boundaries: BoundaryConditions,
    /// In-core `ix` range for each `(iy, iz)` line. MATLAB
    /// `geometry.xlows`/`xhis`.
    pub x_range: ActiveRange,
    /// In-core `iy` range for each `(ix, iz)` line. MATLAB
    /// `geometry.ylows`/`yhis`.
    pub y_range: ActiveRange,
    /// In-core `iz` range for each `(ix, iy)` line. MATLAB
    /// `geometry.zlows`/`zhis`.
    pub z_range: ActiveRange,
    /// Assembly discontinuity factors, `philen`×6, dimensionless. MATLAB
    /// `geometry.adf`; unity everywhere when the field is absent.
    pub adf: FaceTerms,
    /// Semi-analytic expansion coefficients, filled by
    /// [`super::nodal_coefficients::assemble`]. MATLAB `geometry.nodalcoeffs`.
    pub nodal_coefficients: NodalCoefficients,
}

impl NodalGeometry {
    /// A uniform-mesh geometry with whole-axis in-core ranges, unity ADFs and
    /// zeroed nodal coefficients.
    ///
    /// This reproduces every `isfield(geometry, ...)` fallback in the ported
    /// MATLAB in one place. `lx`, `ly`, `lz` and `which_sigma` are per spatial
    /// node; `volume` is computed as `lx*ly*lz` \[cm³\].
    ///
    /// # Panics
    ///
    /// If any input vector length differs from `grid.nodes()`.
    #[must_use]
    pub fn new(
        grid: Grid,
        lx: Vec<f64>,
        ly: Vec<f64>,
        lz: Vec<f64>,
        which_sigma: Vec<usize>,
        boundaries: BoundaryConditions,
    ) -> Self {
        let nodes = grid.nodes();
        assert_eq!(lx.len(), nodes, "lx length");
        assert_eq!(ly.len(), nodes, "ly length");
        assert_eq!(lz.len(), nodes, "lz length");
        assert_eq!(which_sigma.len(), nodes, "which_sigma length");
        let volume = (0..nodes).map(|i| lx[i] * ly[i] * lz[i]).collect();
        Self {
            lx,
            ly,
            lz,
            volume,
            which_sigma,
            boundaries,
            x_range: ActiveRange::full(grid.ny, grid.nz, grid.nx),
            y_range: ActiveRange::full(grid.nx, grid.nz, grid.ny),
            z_range: ActiveRange::full(grid.nx, grid.ny, grid.nz),
            adf: FaceTerms::ones(grid.state_len()),
            nodal_coefficients: NodalCoefficients::zeros(grid.state_len()),
        }
    }

    /// The in-core index range along `axis`.
    #[must_use]
    pub const fn range(&self, axis: Axis) -> &ActiveRange {
        match axis {
            Axis::X => &self.x_range,
            Axis::Y => &self.y_range,
            Axis::Z => &self.z_range,
        }
    }

    /// Node width along `axis` at spatial node `node` \[cm\].
    #[must_use]
    pub fn width(&self, axis: Axis, node: usize) -> f64 {
        match axis {
            Axis::X => self.lx[node],
            Axis::Y => self.ly[node],
            Axis::Z => self.lz[node],
        }
    }

    /// Node widths along `axis` lifted to full state length by repeating the
    /// spatial vector once per energy group — MATLAB `repmat(Lx,G,1)`.
    #[must_use]
    pub fn width_state_vector(&self, axis: Axis, grid: Grid) -> Vec<f64> {
        let spatial = match axis {
            Axis::X => &self.lx,
            Axis::Y => &self.ly,
            Axis::Z => &self.lz,
        };
        let mut out = Vec::with_capacity(grid.state_len());
        for _ in 0..grid.ngroups {
            out.extend_from_slice(spatial);
        }
        out
    }
}

/// The `A`, `B`, `E`, `F`, `G`, `H` coefficients of the semi-analytic nodal
/// expansion, one full state vector per coefficient per direction.
///
/// All six are dimensionless functions of the node's optical half-width
/// `alpha = 0.5*L*sqrt(sigma_r/D)`; see [`super::nodal_coefficients`] for the
/// formulas and for what happens as `alpha -> 0`. MATLAB
/// `geometry.nodalcoeffs.{Aa,Bb,Ee,Ff,Gg,Hh}`.
#[derive(Debug, Clone, PartialEq)]
pub struct NodalCoefficients {
    /// `Aa` — multiplies the first leakage moment in the odd expansion.
    pub aa: DirectionVectors,
    /// `Bb` — multiplies the second leakage moment in the even expansion.
    pub bb: DirectionVectors,
    /// `Ee` — the even-mode buckling weight.
    pub ee: DirectionVectors,
    /// `Ff` — the odd-mode current weight.
    pub ff: DirectionVectors,
    /// `Gg` — the fourth-order surface-flux weight.
    pub gg: DirectionVectors,
    /// `Hh` — the third-order surface-current weight.
    pub hh: DirectionVectors,
}

impl NodalCoefficients {
    /// All six coefficients zeroed, for a state vector of length `n`.
    #[must_use]
    pub fn zeros(n: usize) -> Self {
        Self {
            aa: DirectionVectors::zeros(n),
            bb: DirectionVectors::zeros(n),
            ee: DirectionVectors::zeros(n),
            ff: DirectionVectors::zeros(n),
            gg: DirectionVectors::zeros(n),
            hh: DirectionVectors::zeros(n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_columns_match_the_matlab_column_order() {
        // gradterms(:,1..6) = x-, x+, y-, y+, z-, z+
        assert_eq!(Face::XMinus.column(), 0);
        assert_eq!(Face::XPlus.column(), 1);
        assert_eq!(Face::YMinus.column(), 2);
        assert_eq!(Face::YPlus.column(), 3);
        assert_eq!(Face::ZMinus.column(), 4);
        assert_eq!(Face::ZPlus.column(), 5);
    }

    #[test]
    fn face_terms_round_trip() {
        let mut t = FaceTerms::zeros(3);
        t.set(1, Face::YPlus, 2.5);
        assert_eq!(t.get(1, Face::YPlus), 2.5);
        assert_eq!(t.get(1, Face::YMinus), 0.0);
        t.scale(2.0);
        assert_eq!(t.get(1, Face::YPlus), 5.0);
    }

    #[test]
    fn full_active_range_spans_the_axis() {
        let r = ActiveRange::full(3, 4, 7);
        assert_eq!(r.low(2, 3), 0);
        assert_eq!(r.high(2, 3), 6);
    }

    #[test]
    fn matlab_default_nodal_update_interval_is_ceil_of_span_over_ten() {
        // IAEA-3D: 17+17+19 = 53 -> ceil(5.3) = 6.
        let grid = Grid::new(17, 17, 19, 2).expect("valid grid");
        assert_eq!(
            NodalParams::with_matlab_defaults(grid).nodal_update_interval,
            6
        );
        // A 17x17x14 mesh: 48 -> ceil(4.8) = 5, the value the MATLAB comment
        // quotes as the default.
        let grid = Grid::new(17, 17, 14, 2).expect("valid grid");
        assert_eq!(
            NodalParams::with_matlab_defaults(grid).nodal_update_interval,
            5
        );
    }

    #[test]
    fn width_state_vector_repeats_the_spatial_vector_per_group() {
        let grid = Grid::new(2, 1, 1, 2).expect("valid grid");
        let geom = NodalGeometry::new(
            grid,
            vec![1.0, 2.0],
            vec![3.0, 3.0],
            vec![4.0, 4.0],
            vec![1, 1],
            BoundaryConditions::uniform(BoundaryCondition::Vacuum),
        );
        assert_eq!(
            geom.width_state_vector(Axis::X, grid),
            vec![1.0, 2.0, 1.0, 2.0]
        );
        assert_eq!(geom.volume, vec![12.0, 24.0]);
    }
}
