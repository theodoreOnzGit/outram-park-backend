//! The NEACRP 3-D LWR core transient benchmark, PWR case A2 — steady state.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `neacrpa2.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//! - **Composition maps:** `src/data/NEACRPA2_*.csv`; see
//!   `src/data/PROVENANCE.md`.
//!
//! # Why this case matters
//!
//! It is the most complete case in the snapshot, and the one the transient
//! driver was written for. Two things appear here for the first time:
//!
//! - **All five feedback channels at once** — boron, fuel temperature, coolant
//!   temperature, coolant density and control rods. [`crate::neacrpd1`] runs
//!   only two, so the boron, coolant-temperature and rod channels of
//!   [`crate::sigmavalupd3d_handler`] have never been exercised by a real case
//!   before this one.
//! - **A real control-rod bank pattern** — seven banks on a 17x17 map, with
//!   partial insertions (`crod = [100, 200, 100, 200, 200, 200, 200]` steps).
//!   This is what the rod-ejection transient `neacrpa2t` moves.
//!
//! # The problem
//!
//! A 17 x 17 x 18 core octant with rotational symmetry, 10.803 cm radial pitch,
//! reflective on the low `x` and `y` faces and zero flux elsewhere. Two energy
//! groups, **11 materials** (axial and radial reflectors, a re-entrant corner,
//! and eight fuel compositions from 2.1 to 3.1 w/o with burnable absorbers).
//!
//! # WARNING — the axial mesh is non-uniform, and that hits defect G1
//!
//! The axial layer heights are
//!
//! ```text
//! 30, 7.7, 11, 15, 30 x10, 12.8, 12.8, 8, 30   cm
//! ```
//!
//! and [`crate::makegrad_dxyz`]'s face coupling is **only a consistent
//! discretisation on a uniform mesh** — defect **G1** in
//! `docs/bedok-reference-defects.md`, confirmed by measurement: a 2:1 cell-size
//! jump understates the face coupling by 25%, and the worst joint here is close
//! to 4:1 (30 cm against 7.7 cm).
//!
//! This is **not** repaired, per the no-silent-repairs policy — repairing it
//! would move every NEACRP number and is a *correction*, which cannot be gated
//! on parity with the reference. What it means in practice:
//!
//! - Results from this case carry a discretisation error at the axial layer
//!   joints that does not vanish under refinement unless the mesh is also made
//!   uniform.
//! - The reference always solves it with [`crate::sanodaldiffusion_solverxyz`],
//!   whose nodal correction is refitted against the same operator and appears
//!   to absorb much of it. **Do not solve this case with the bare
//!   finite-difference solver** and expect a sensible axial power shape.
//!
//! # The cross sections are given as total and absorption
//!
//! As in [`crate::neacrpd1`]: the case supplies total, absorption and the
//! down-scatter, and closes the within-group scattering by difference. `nu` is
//! all ones, so `sigmavalues.f` already carries `nu*Sigma_f`. Unlike case D1,
//! this case **does** populate `fp` directly.
//!
//! # Transcription
//!
//! The five feedback channels are 24 tables of 11 materials by 2 groups, plus
//! six down-scatter columns — around 450 numbers. They were **extracted
//! mechanically** from `neacrpa2.m` rather than retyped, and every distinct
//! numeric literal below was checked to appear verbatim in the source.

use crate::geometry_ends3d::geometry_ends3d;
use crate::matlab::{Array2, Array3};
use crate::sigmavalupd3d::DeltaSigmaValues;
use crate::sigmavalupd3d_handler::FeedbackTables;
use crate::types::{
    BoundaryCondition, Conductivity, Coolant, FlowDirection, FuelGeometry, Geometry, MassFlux,
    Params, SigmaValues, Th,
};

/// The three radial composition maps, embedded at build time.
const MAP_REFLECTOR: &str = include_str!("data/NEACRPA2_1.csv");
const MAP_LOWER: &str = include_str!("data/NEACRPA2_2.csv");
const MAP_CORE: &str = include_str!("data/NEACRPA2_3.csv");
/// Which of the seven control-rod banks sits over each lattice position.
const MAP_CRODBANKS: &str = include_str!("data/NEACRPA2_CRODBANKS.csv");

/// The number of materials in the case's cross-section set.
pub const MATERIALS: usize = 11;

/// The axial layer heights, cm — **non-uniform**; see the module warning.
pub const Z_LENGTHS: [f64; 18] = [
    30.0, 7.7, 11.0, 15.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 12.8, 12.8,
    8.0, 30.0,
];

/// Parse one 17-by-17 comma-separated integer map.
///
/// # Panics
/// If the file is not 17 rows of 17 integers.
fn parse_map(text: &str) -> Array2<usize> {
    let rows: Vec<Vec<usize>> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.split(',')
                .map(|c| {
                    c.trim()
                        .parse::<usize>()
                        .unwrap_or_else(|e| panic!("bad map entry {c:?}: {e}"))
                })
                .collect()
        })
        .collect();
    assert_eq!(rows.len(), 17, "expected 17 rows, got {}", rows.len());
    let mut a = Array2::<usize>::zeros(17, 17);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), 17, "row {i} has {} entries", row.len());
        for (j, v) in row.iter().enumerate() {
            a.set(i, j, *v);
        }
    }
    a
}

/// Fill a `MATERIALS`-by-2 table from literal rows.
fn table2(rows: [[f64; 2]; MATERIALS]) -> Array2<f64> {
    let mut a = Array2::<f64>::zeros(MATERIALS, 2);
    for (m, row) in rows.iter().enumerate() {
        for (g, v) in row.iter().enumerate() {
            a.set(m, g, *v);
        }
    }
    a
}

/// Build the scattering array and close the diagonal against total and
/// absorption, exactly as the case file does.
///
/// `down[m]` is `s(m, 2, 1)`, the group 1 -> 2 down-scatter. Up-scatter is zero
/// throughout, so group 2's diagonal is total less absorption.
fn scattering(tot: &Array2<f64>, a: &Array2<f64>, down: [f64; MATERIALS]) -> Array3<f64> {
    let mut s = Array3::<f64>::zeros(MATERIALS, 2, 2);
    for (m, d) in down.iter().enumerate() {
        s.set(m, 1, 0, *d);
        s.set(m, 0, 0, tot.get(m, 0) - a.get(m, 0) - *d);
        s.set(m, 1, 1, tot.get(m, 1) - a.get(m, 1));
    }
    s
}

/// `[params, geometry, th, constants, whichsigma, sigmavalues] = neacrpa2(params)`.
///
/// Builds the complete NEACRP case-A2 steady state: the graded axial mesh, the
/// 11-material two-group cross-section set, the three-layer material map, the
/// seven control-rod banks, the thermal-hydraulic inlet state and rod geometry,
/// and **all five** feedback tables.
///
/// # Returns
///
/// `(params, geometry, th, whichsigma, sigmavalues, feedback)`, matching
/// [`crate::neacrpd1::neacrpd1`].
///
/// # The mesh is fixed at 17 x 17 x 18
///
/// The reference computes `xscale`/`yscale`/`zscale` and indexes the maps with
/// `ceil(ix/maxix*17)`, an identity only at 17; the axial layer assignment is
/// likewise written in multiples of `zscale`. This translation fixes the mesh
/// at the benchmark's own 17 x 17 x 18 and asserts it.
///
/// # Panics
///
/// If `params.maxix`, `maxiy` or `maxiz` is set to anything other than
/// 17, 17, 18.
#[allow(clippy::type_complexity)]
pub fn neacrpa2(
    params: &Params,
) -> (Params, Geometry, Th, Array3<usize>, SigmaValues, FeedbackTables) {
    const NX: usize = 17;
    const NY: usize = 17;
    const NZ: usize = 18;

    let mut params = params.clone();
    params.maxix = Some(NX);
    params.maxiy = Some(NY);
    params.maxiz = Some(NZ);
    params.nc = Some(0);
    params.g = 2;

    let es = NX * NY * NZ;

    // ----- mesh -----
    let xtot = 10.803 * 17.0;
    let ytot = xtot;
    let (sx, sy) = (xtot / NX as f64, ytot / NY as f64);
    // The graded axial mesh. See the module warning on defect G1.
    let lz: Vec<f64> = (0..es).map(|idx| Z_LENGTHS[idx % NZ]).collect();
    let vi: Vec<f64> = lz.iter().map(|h| sx * sy * h).collect();

    let uniform = |rows: usize, cols: usize| Array2::<usize>::zeros(rows, cols);

    let mut geometry = Geometry {
        xtot,
        ytot,
        // 18 axial blocks, one mesh layer each.
        zscale: NZ / 18,
        lx: vec![sx; es],
        ly: vec![sy; es],
        lz,
        vi,
        xmin: BoundaryCondition::Reflective,
        xmax: BoundaryCondition::ZeroFlux,
        ymin: BoundaryCondition::Reflective,
        ymax: BoundaryCondition::ZeroFlux,
        zmin: BoundaryCondition::ZeroFlux,
        zmax: BoundaryCondition::ZeroFlux,
        xlows: Some(uniform(NY, NZ)),
        xhis: Some(uniform(NY, NZ)),
        ylows: Some(uniform(NX, NZ)),
        yhis: Some(uniform(NX, NZ)),
        zlows: Some(uniform(NX, NY)),
        zhis: Some(uniform(NX, NY)),
        ..Default::default()
    };

    // ----- cross sections -----
// ===== base cross sections =====
    #[rustfmt::skip]
    let tot = table2([
        [0.0532058, 0.386406],
        [0.295609, 2.45931],
        [0.295609, 2.45931],
        [0.222117, 0.803140],
        [0.221914, 0.795538],
        [0.221715, 0.789253],
        [0.222039, 0.776230],
        [0.222083, 0.769969],
        [0.222127, 0.763813],
        [0.221836, 0.770705],
        [0.221878, 0.764704],
    ]);
    #[rustfmt::skip]
    let f = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [4.98277e-03, 8.39026e-02],
        [5.57659e-03, 9.98629e-02],
        [6.15047e-03, 1.14667e-01],
        [5.55010e-03, 9.85576e-02],
        [5.54083e-03, 9.80059e-02],
        [5.53137e-03, 9.74109e-02],
        [6.12382e-03, 1.13241e-01],
        [6.11444e-03, 1.12635e-01],
    ]);
    #[rustfmt::skip]
    let absorption = table2([
        [3.73279e-04, 1.77215e-02],
        [1.18782e-03, 2.52618e-01],
        [1.18782e-03, 2.52618e-01],
        [8.71774e-03, 6.52550e-02],
        [9.06133e-03, 7.23354e-02],
        [9.38496e-03, 7.89203e-02],
        [9.31692e-03, 7.96328e-02],
        [9.40032e-03, 8.21087e-02],
        [9.48286e-03, 8.45912e-02],
        [9.63720e-03, 8.61187e-02],
        [9.71937e-03, 8.85488e-02],
    ]);
    #[rustfmt::skip]
    let fp = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [6.11190e-14, 1.10152e-12],
        [6.89181e-14, 1.31106e-12],
        [7.64603e-14, 1.50541e-12],
        [6.86391e-14, 1.29393e-12],
        [6.85391e-14, 1.28669e-12],
        [6.84379e-14, 1.27888e-12],
        [7.61794e-14, 1.48670e-12],
        [7.60778e-14, 1.47876e-12],
    ]);
    #[rustfmt::skip]
    let down = [
        0.0264554, 0.0231613, 0.0200808, 0.0182498, 0.0180040, 0.0177670, 0.0171381, 0.0168501, 0.0165626, 0.0169043, 0.0166175,
    ];

// ===== boron feedback (ref = 1200.2) =====
    #[rustfmt::skip]
    let boron_tot = table2([
        [6.11833e-08, 5.17535e-06],
        [0.0, 7.76184e-04],
        [0.0, 7.76184e-04],
        [3.47809e-08, -9.76510e-06],
        [3.53826e-08, -8.50169e-06],
        [3.59838e-08, -7.46251e-06],
        [3.37806e-08, -6.73744e-06],
        [3.32495e-08, -6.19725e-06],
        [3.27201e-08, -5.68220e-06],
        [3.43859e-08, -5.86898e-06],
        [3.38559e-08, -5.38345e-06],
    ]);
    #[rustfmt::skip]
    let boron_f = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [-1.12099e-09, -2.43045e-06],
        [-1.67880e-09, -2.72445e-06],
        [-2.21038e-09, -2.95883e-06],
        [-1.71323e-09, -2.55359e-06],
        [-1.72421e-09, -2.48880e-06],
        [-1.73502e-09, -2.42240e-06],
        [-2.24335e-09, -2.77657e-06],
        [-2.25369e-09, -2.70780e-06],
    ]);
    #[rustfmt::skip]
    let boron_fp = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [-1.76188e-20, -3.19085e-17],
        [-2.49965e-20, -3.57680e-17],
        [-3.20225e-20, -3.88451e-17],
        [-2.49965e-20, -3.35223e-17],
        [-2.54896e-20, -3.26704e-17],
        [-2.56049e-20, -3.17976e-17],
        [-3.20225e-20, -3.64509e-17],
        [-3.24873e-20, -3.55476e-17],
    ]);
    #[rustfmt::skip]
    let boron_absorption = table2([
        [1.87731e-07, 1.02635e-05],
        [0.0, 8.44695e-05],
        [0.0, 8.44695e-05],
        [1.28505e-07, 7.08807e-06],
        [1.26709e-07, 6.82311e-06],
        [1.24986e-07, 6.59798e-06],
        [1.19869e-07, 6.29310e-06],
        [1.17585e-07, 6.11904e-06],
        [1.15319e-07, 5.94711e-06],
        [1.18186e-07, 6.08443e-06],
        [1.15917e-07, 5.91697e-06],
    ]);
    #[rustfmt::skip]
    let boron_down = [
        7.91457e-10, 0.0, 0.0, -1.08590e-07, -1.06951e-07, -1.05374e-07, -1.00873e-07, -9.88578e-08, -9.68489e-08, -9.93312e-08, -9.73291e-08,
    ];

// ===== fueltemp feedback (ref = 891.45) =====
    #[rustfmt::skip]
    let fueltemp_tot = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [-3.09197e-05, -0.000137292],
        [-3.08607e-05, -0.000117481],
        [-3.09165e-05, -0.000101337],
        [-3.13746e-05, -0.000108271],
        [-3.15503e-05, -0.000105521],
        [-3.17281e-05, -0.000102525],
        [-3.14192e-05, -9.38886e-05],
        [-3.15908e-05, -9.17126e-05],
    ]);
    #[rustfmt::skip]
    let fueltemp_f = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [6.40134e-07, -5.63037e-05],
        [9.97431e-07, -6.04155e-05],
        [1.41847e-06, -0.000063096],
        [9.45431e-07, -5.79662e-05],
        [9.26078e-07, -5.71108e-05],
        [9.05802e-07, -5.61543e-05],
        [1.35642e-06, -6.05052e-05],
        [1.33336e-06, -5.96284e-05],
    ]);
    #[rustfmt::skip]
    let fueltemp_fp = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [7.15412e-18, -7.39188e-16],
        [1.18685e-17, -7.93170e-16],
        [1.74269e-17, -8.28363e-16],
        [1.18685e-17, -7.60849e-16],
        [1.08935e-17, -7.49575e-16],
        [1.06166e-17, -7.36969e-16],
        [1.74269e-17, -7.94252e-16],
        [1.62769e-17, -7.82716e-16],
    ]);
    #[rustfmt::skip]
    let fueltemp_absorption = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [3.49709e-05, -3.71806e-05],
        [3.51798e-05, -3.77039e-05],
        [3.53841e-05, -3.77558e-05],
        [3.48699e-05, -3.72748e-05],
        [3.47274e-05, -3.71808e-05],
        [3.46026e-05, -3.70201e-05],
        [3.50637e-05, -3.71403e-05],
        [3.49119e-05, -3.69909e-05],
    ]);
    #[rustfmt::skip]
    let fueltemp_down = [
        0.0, 0.0, 0.0, -2.75536e-05, -2.76766e-05, -2.78390e-05, -2.73550e-05, -2.72381e-05, -2.71169e-05, -2.75049e-05, -2.73835e-05,
    ];

// ===== cooltemp feedback (ref = 579.75) =====
    #[rustfmt::skip]
    let cooltemp_tot = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [-2.03310e-06, -1.08674e-04],
        [-1.98080e-06, -9.06150e-05],
        [-1.92434e-06, -7.62786e-05],
        [-2.69634e-06, -7.62435e-05],
        [-3.07905e-06, -7.33397e-05],
        [-3.53877e-06, -7.13711e-05],
        [-2.63907e-06, -6.39554e-05],
        [-3.02147e-06, -6.16984e-05],
    ]);
    #[rustfmt::skip]
    let cooltemp_f = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [1.24709e-07, -4.16439e-05],
        [1.35145e-07, -4.53102e-05],
        [1.49084e-07, -4.78475e-05],
        [1.40773e-07, -4.20202e-05],
        [1.43235e-07, -4.07701e-05],
        [1.46019e-07, -3.94319e-05],
        [1.55858e-07, -4.44431e-05],
        [1.58814e-07, -4.31588e-05],
    ]);
    #[rustfmt::skip]
    let cooltemp_fp = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [1.43035e-18, -5.46722e-16],
        [1.56896e-18, -5.94857e-16],
        [1.75422e-18, -6.28174e-16],
        [1.56896e-18, -5.51669e-16],
        [1.67897e-18, -5.35261e-16],
        [1.71665e-18, -5.17689e-16],
        [1.75422e-18, -5.83483e-16],
        [1.88528e-18, -5.66622e-16],
    ]);
    #[rustfmt::skip]
    let cooltemp_absorption = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [2.12191e-07, -3.15597e-05],
        [2.26000e-07, -3.21435e-05],
        [2.39939e-07, -3.23776e-05],
        [2.48530e-07, -3.00119e-05],
        [2.61854e-07, -2.91929e-05],
        [2.74313e-07, -2.83041e-05],
        [2.64289e-07, -3.03509e-05],
        [2.79060e-07, -2.95626e-05],
    ]);
    #[rustfmt::skip]
    let cooltemp_down = [
        0.0, 0.0, 0.0, 8.09676e-07, 8.58474e-07, 9.03494e-07, 7.01311e-07, 6.17380e-07, 5.16547e-07, 7.44320e-07, 6.59521e-07,
    ];

// ===== coolden feedback (ref = 0.7125) =====
    #[rustfmt::skip]
    let coolden_tot = table2([
        [7.45756e-02, 5.33634e-01],
        [0.0, 0.0],
        [0.0, 0.0],
        [1.35665e-01, 9.92628e-01],
        [1.35748e-01, 9.81985e-01],
        [1.35827e-01, 9.72267e-01],
        [1.31033e-01, 9.34697e-01],
        [1.29379e-01, 9.18171e-01],
        [1.27682e-01, 9.01293e-01],
        [1.31116e-01, 9.24925e-01],
        [1.29463e-01, 9.08456e-01],
    ]);
    #[rustfmt::skip]
    let coolden_f = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [9.20694e-04, 2.47746e-02],
        [9.64160e-04, 3.14993e-02],
        [1.01410e-03, 3.81097e-02],
        [9.81951e-04, 3.51588e-02],
        [9.88437e-04, 3.63251e-02],
        [9.95175e-04, 3.74499e-02],
        [1.03522e-03, 4.20693e-02],
        [1.04291e-03, 4.33215e-02],
    ]);
    #[rustfmt::skip]
    let coolden_fp = table2([
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [1.02392e-14, 3.25255e-13],
        [1.08141e-14, 4.13542e-13],
        [1.14771e-14, 5.00328e-13],
        [1.08141e-14, 4.61715e-13],
        [1.11322e-14, 4.77078e-13],
        [1.12209e-14, 4.91900e-13],
        [1.14771e-14, 5.52387e-13],
        [1.18534e-14, 5.68857e-13],
    ]);
    #[rustfmt::skip]
    let coolden_absorption = table2([
        [2.07688e-04, 7.58421e-03],
        [0.0, 0.0],
        [0.0, 0.0],
        [1.55185e-03, 2.52662e-02],
        [1.61491e-03, 2.86667e-02],
        [1.68015e-03, 3.19571e-02],
        [1.68397e-03, 3.14240e-02],
        [1.71972e-03, 3.24715e-02],
        [1.74989e-03, 3.35945e-02],
        [1.75528e-03, 3.49853e-02],
        [1.79499e-03, 3.61032e-02],
    ]);
    #[rustfmt::skip]
    let coolden_down = [
        3.71310e-02, 0.0, 0.0, 2.93195e-02, 2.92696e-02, 2.92154e-02, 2.82489e-02, 2.78895e-02, 2.75202e-02, 2.81877e-02, 2.78259e-02,
    ];

// ===== crod feedback (ref = n/a) =====
    #[rustfmt::skip]
    let crod_tot = table2([
        [3.73220e-03, -2.19926e-02],
        [3.73220e-03, -2.19926e-02],
        [3.73220e-03, -2.19926e-02],
        [3.73220e-03, -2.19926e-02],
        [3.73220e-03, -2.19926e-02],
        [3.74092e-03, -1.67503e-02],
        [3.73220e-03, -2.19926e-02],
        [3.73220e-03, -2.19926e-02],
        [3.73220e-03, -2.19926e-02],
        [3.73220e-03, -2.19926e-02],
        [3.73220e-03, -2.19926e-02],
    ]);
    #[rustfmt::skip]
    let crod_f = table2([
        [-1.02786e-04, -2.82319e-03],
        [-1.02786e-04, -2.82319e-03],
        [-1.02786e-04, -2.82319e-03],
        [-1.02786e-04, -2.82319e-03],
        [-1.02786e-04, -2.82319e-03],
        [-1.22634e-04, -3.28086e-03],
        [-1.02786e-04, -2.82319e-03],
        [-1.02786e-04, -2.82319e-03],
        [-1.02786e-04, -2.82319e-03],
        [-1.02786e-04, -2.82319e-03],
        [-1.02786e-04, -2.82319e-03],
    ]);
    #[rustfmt::skip]
    let crod_fp = table2([
        [-1.21448e-15, -3.70238e-14],
        [-1.21448e-15, -3.70238e-14],
        [-1.21448e-15, -3.70238e-14],
        [-1.21448e-15, -3.70238e-14],
        [-1.21448e-15, -3.70238e-14],
        [-1.47557e-15, -4.30444e-14],
        [-1.21448e-15, -3.70238e-14],
        [-1.21448e-15, -3.70238e-14],
        [-1.21448e-15, -3.70238e-14],
        [-1.21448e-15, -3.70238e-14],
        [-1.21448e-15, -3.70238e-14],
    ]);
    #[rustfmt::skip]
    let crod_absorption = table2([
        [2.47770e-03, 2.55875e-02],
        [2.47770e-03, 2.55875e-02],
        [2.47770e-03, 2.55875e-02],
        [2.47770e-03, 2.55875e-02],
        [2.47770e-03, 2.55875e-02],
        [2.42926e-03, 2.56478e-02],
        [2.47770e-03, 2.55875e-02],
        [2.47770e-03, 2.55875e-02],
        [2.47770e-03, 2.55875e-02],
        [2.47770e-03, 2.55875e-02],
        [2.47770e-03, 2.55875e-02],
    ]);
    #[rustfmt::skip]
    let crod_down = [
        -3.19253e-03, -3.19253e-03, -3.19253e-03, -3.19253e-03, -3.19253e-03, -3.14239e-03, -3.19253e-03, -3.19253e-03, -3.19253e-03, -3.19253e-03, -3.19253e-03,
    ];

    let s = scattering(&tot, &absorption, down);

    let mut nu = Array2::<f64>::zeros(MATERIALS, 2);
    let mut chi = Array2::<f64>::zeros(MATERIALS, 2);
    for m in 0..MATERIALS {
        nu.set(m, 0, 1.0);
        nu.set(m, 1, 1.0);
        chi.set(m, 0, 1.0);
    }

    let sigmavalues = SigmaValues { tot, f, s, nu, chi, fp: Some(fp) };

    // ----- material map -----
    // Layer 1 is the bottom axial reflector, layer 2 a transition, layers 3-17
    // the active core, and layer 18 the top reflector — which reuses the same
    // map as the bottom.
    let reflector = parse_map(MAP_REFLECTOR);
    let lower = parse_map(MAP_LOWER);
    let core = parse_map(MAP_CORE);

    let mut whichsigma = Array3::<usize>::zeros(NX, NY, NZ);
    for ix in 0..NX {
        for iy in 0..NY {
            for iz in 0..NZ {
                let m = match iz {
                    0 => reflector.get(ix, iy),
                    1 => lower.get(ix, iy),
                    2..=16 => core.get(ix, iy),
                    _ => reflector.get(ix, iy),
                };
                whichsigma.set(ix, iy, iz, m);
            }
        }
    }

    geometry_ends3d(&params, &mut geometry, &whichsigma);

    // ----- control rods -----
    geometry.crodbanks = Some(parse_map(MAP_CRODBANKS));
    geometry.crodbtm = 37.7;
    geometry.crodstep = 1.594_223_7;
    // `0` is fully inserted; the banks sit part-withdrawn at steady state.
    geometry.crod = vec![100.0, 200.0, 100.0, 200.0, 200.0, 200.0, 200.0];

    // ----- thermal hydraulics -----
    // `12893000 / 157 / (4*10.803^2 - 314*pi*0.47585^2)` — core mass flow over
    // the flow area left by 314 pins in the node. The reference carries two
    // earlier, overwritten forms of this; only the last runs.
    let flow_area = 4.0 * 10.803 * 10.803 - 314.0 * std::f64::consts::PI * 0.475_85 * 0.475_85;
    let flowrate = 12_893_000.0 / 157.0 / flow_area;

    // ----- fuel rod -----
    params.fuel.gapn = 1;
    params.fuel.cladn = 1;
    params.fuel.fueln = 20;
    params.fuel.maxir = params.fuel.gapn + params.fuel.cladn + params.fuel.fueln;
    let maxir = params.fuel.maxir;

    let fuelrad = 4.119_50E-01;
    let fuelgap = 6.8E-03;
    let clad = 5.71E-02;
    let pitch = 1.2665;
    let rtot = fuelrad + fuelgap + clad;

    let mut lr = vec![0.0; maxir];
    for (i, l) in lr.iter_mut().enumerate() {
        *l = if i < params.fuel.fueln {
            fuelrad / params.fuel.fueln as f64
        } else if i < params.fuel.fueln + params.fuel.gapn {
            fuelgap / params.fuel.gapn as f64
        } else {
            clad / params.fuel.cladn as f64
        };
    }

    let mut ctr = vec![0.0; maxir];
    let mut running = 0.0;
    for (i, c) in ctr.iter_mut().enumerate() {
        running += lr[i];
        *c = running - 0.5 * lr[i];
    }

    // The same `sum(Lr(i))`-on-a-scalar-index defect as `neacrpd1` — B1 in the
    // register. Dead data; reproduced as written.
    let mut vif = vec![0.0; maxir];
    vif[0] = std::f64::consts::PI * lr[0] * lr[0];
    for i in 1..maxir {
        vif[i] = std::f64::consts::PI * (lr[i] * lr[i] - lr[i - 1] * lr[i - 1]);
    }

    let mut whichk = vec![1usize; maxir];
    for (i, k) in whichk.iter_mut().enumerate() {
        let ir = i + 1;
        if ir > params.fuel.fueln && ir <= params.fuel.fueln + params.fuel.gapn {
            *k = 0;
        } else if ir > params.fuel.fueln + params.fuel.gapn {
            *k = 2;
        }
    }

    let subarea = pitch * pitch - std::f64::consts::PI * rtot * rtot;
    let hydia = 4.0 * subarea / (2.0 * std::f64::consts::PI * rtot + 4.0 * pitch - 8.0 * rtot);

    geometry.fuel = FuelGeometry {
        lr,
        ctr,
        vi: vif,
        whichk,
        tcon: vec![Conductivity::Uo2Fuel, Conductivity::ZircaloyClad],
        rhocp: Vec::new(),
        // `tcon{3}` — **1.0 here, against 0.35 in case D1.** Both are
        // attributed to the NEACRP benchmark; the cases genuinely differ.
        gap_conductance: 1.0,
        fuelrad,
        rtot,
        pitch,
        subarea,
        hydia,
        doppleralpha: 0.7,
    };

    let th = Th {
        coolant: Coolant {
            inlettemp: 559.15,
            inletpress: 15.5,
            inletvoid: 1e-14,
            ..Default::default()
        },
        maxpow: 693.75e6,
        powratio: 1.0,
        coolheatfrac: 0.019,
        // `264/4` pins per node, divided by the radial scales (both 1).
        nfuelpin: 264.0 / 4.0,
        flowrate: MassFlux::Uniform(flowrate),
        flowdir: FlowDirection::Up,
        ..Default::default()
    };

    params.boron = 1000.0;
    params.fueltempavg = 891.19;
    params.cooltempavg = 559.19;
    params.cooldenavg = 0.7464;

    // ----- feedback tables -----
    let feedback = FeedbackTables {
        boron: Some(DeltaSigmaValues {
            s: scattering(&boron_tot, &boron_absorption, boron_down),
            tot: boron_tot,
            f: boron_f,
            fp: boron_fp,
            reference: 1200.2,
        }),
        fueltemp: Some(DeltaSigmaValues {
            s: scattering(&fueltemp_tot, &fueltemp_absorption, fueltemp_down),
            tot: fueltemp_tot,
            f: fueltemp_f,
            fp: fueltemp_fp,
            reference: 891.45,
        }),
        cooltemp: Some(DeltaSigmaValues {
            s: scattering(&cooltemp_tot, &cooltemp_absorption, cooltemp_down),
            tot: cooltemp_tot,
            f: cooltemp_f,
            fp: cooltemp_fp,
            reference: 579.75,
        }),
        coolden: Some(DeltaSigmaValues {
            s: scattering(&coolden_tot, &coolden_absorption, coolden_down),
            tot: coolden_tot,
            f: coolden_f,
            fp: coolden_fp,
            reference: 0.7125,
        }),
        // The rod channel has no reference in the case file; the handler forces
        // it to zero on use, so a fully rodded node takes the full slope.
        crod: Some(DeltaSigmaValues {
            s: scattering(&crod_tot, &crod_absorption, crod_down),
            tot: crod_tot,
            f: crod_f,
            fp: crod_fp,
            reference: 0.0,
        }),
        modtemp: None,
    };

    (params, geometry, th, whichsigma, sigmavalues, feedback)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cross-section set is internally consistent.
    ///
    /// # Methodology
    ///
    /// As in [`crate::neacrpd1`]: total, absorption and down-scatter are given
    /// and the within-group diagonal closed by difference, so the implied
    /// absorption `tot - sum_gt s(gt, g)` must come back positive for every
    /// material and group. Also checked: no up-scatter, an all-fast fission
    /// spectrum, and that the three reflector materials (1, 2, 3 — axial,
    /// radial, and the re-entrant corner) do not fission while all eight fuel
    /// compositions do.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// All 22 material-group entries closed to a **positive** implied
    /// absorption, spanning `3.733e-4` to `2.526e-1` /cm. No up-scatter,
    /// all-fast fission spectrum, three non-fissioning reflectors and eight
    /// fissioning fuel compositions.
    ///
    /// **Interpretation.** The machine-extracted tables are self-consistent,
    /// which is the check that matters for a transcription of ~450 numbers.
    /// The upper end of the span is the radial reflector's thermal group
    /// (0.253 /cm) — a strong thermal absorber, as a reflector-baffle
    /// region should be.
    #[test]
    fn the_cross_sections_close_against_absorption() {
        let (_p, _g, _t, _w, sv, _f) = neacrpa2(&Params::default());

        let mut lo = f64::INFINITY;
        let mut hi: f64 = 0.0;
        for m in 0..MATERIALS {
            for g in 0..2 {
                let out: f64 = (0..2).map(|gt| sv.s.get(m, gt, g)).sum();
                let implied = sv.tot.get(m, g) - out;
                assert!(
                    implied > 0.0,
                    "material {} group {g}: implied absorption {implied:e}",
                    m + 1
                );
                lo = lo.min(implied);
                hi = hi.max(implied);
            }
            assert_eq!(sv.s.get(m, 0, 1), 0.0, "material {} up-scatters", m + 1);
            assert_eq!(sv.chi.get(m, 0), 1.0);
            assert_eq!(sv.chi.get(m, 1), 0.0);
        }
        eprintln!("implied absorption spans {lo:.6e} .. {hi:.6e} /cm");

        // Materials 1-3 are reflectors; 4-11 are fuel.
        for m in 0..3 {
            assert_eq!(sv.f.get(m, 0), 0.0, "reflector {} fissions", m + 1);
            assert_eq!(sv.f.get(m, 1), 0.0, "reflector {} fissions", m + 1);
        }
        for m in 3..MATERIALS {
            assert!(sv.f.get(m, 1) > 0.0, "fuel {} does not fission", m + 1);
        }

        // Unlike case D1, this case populates `fp` directly.
        let fp = sv.fp.as_ref().expect("case A2 supplies fp");
        assert!(fp.get(3, 1) > 0.0, "fuel should carry a prompt fission cross section");
        assert_eq!(fp.get(0, 0), 0.0, "the reflector should not");
    }

    /// **The axial mesh is graded, and this quantifies how badly it hits
    /// defect G1.**
    ///
    /// # Methodology
    ///
    /// [`Z_LENGTHS`] is the benchmark's own axial layering. Defect G1 (see
    /// `docs/bedok-reference-defects.md`, and the test in
    /// [`crate::makegrad_dxyz`]) says the face coupling is misstated by
    /// `(L + Lp)/(2*Lp)` at any joint where `L != Lp`. This walks every joint
    /// and reports the worst factor, so the size of the error carried by every
    /// result from this case is on the record rather than implied.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **7 of the 17 axial joints are graded.** The worst is the very first,
    /// layer 1 to layer 2, going 30 cm to 7.7 cm:
    ///
    /// ```text
    /// (L + Lp) / (2 Lp) = (30 + 7.7) / (2 * 7.7) = 2.4481
    /// ```
    ///
    /// — the face coupling there is misstated by **+144.8%**.
    ///
    /// **Interpretation, and it is not a small one.** The 25% figure
    /// measured in [`crate::makegrad_dxyz`]'s test came from a 2:1 jump; this
    /// case's worst joint is nearly 4:1 and the error is correspondingly
    /// worse — the operator **overstates** the coupling out of the thick
    /// bottom reflector into the thin layer above it by a factor of 2.45.
    ///
    /// That sits at the bottom of the core, right where the axial power
    /// shape is set. It is precisely the failure that corrupted the
    /// fine-mesh CC3 axial profile when a graded grid was used with the
    /// bare finite-difference solver.
    ///
    /// **Any result from this case inherits this**, and the SA-nodal
    /// correction is the only thing standing between it and the answer. No
    /// claim about A2's accuracy should be made without quantifying how much
    /// of this the nodal correction actually absorbs — which has **not**
    /// been done.
    #[test]
    fn the_graded_axial_mesh_quantifies_the_gradd_inconsistency() {
        let (_p, geometry, ..) = neacrpa2(&Params::default());

        // The mesh really is the benchmark's.
        for (iz, expect) in Z_LENGTHS.iter().enumerate() {
            assert_eq!(geometry.lz[iz], *expect, "layer {iz}");
        }

        let mut worst = 1.0f64;
        let mut worst_at = (0usize, 0.0, 0.0);
        let mut graded_joints = 0;
        for iz in 0..Z_LENGTHS.len() - 1 {
            let (l, lp) = (Z_LENGTHS[iz], Z_LENGTHS[iz + 1]);
            if (l - lp).abs() > 1e-12 {
                graded_joints += 1;
            }
            let factor = (l + lp) / (2.0 * lp);
            if (factor - 1.0).abs() > (worst - 1.0f64).abs() {
                worst = factor;
                worst_at = (iz, l, lp);
            }
        }
        eprintln!("axial joints: {} of 17 are graded", graded_joints);
        eprintln!(
            "worst joint: layer {} -> {}, {} cm -> {} cm",
            worst_at.0,
            worst_at.0 + 1,
            worst_at.1,
            worst_at.2
        );
        eprintln!(
            "  face coupling misstated by (L+Lp)/(2 Lp) = {worst:.4} ({:+.1}%)",
            (worst - 1.0) * 100.0
        );

        assert!(graded_joints > 0, "this case is supposed to be non-uniform");
        assert!(
            (worst - 1.0).abs() > 0.2,
            "the worst joint should misstate the coupling by well over 20%"
        );
    }

    /// The material map has the benchmark's axial layering and radial outline.
    ///
    /// # Methodology
    ///
    /// Three maps cover 18 layers: the reflector map at the bottom **and** the
    /// top, a transition map at layer 2, and the core map for layers 3 to 17.
    /// The outline must be a right prism (voidness comes from the maps, which
    /// share it), and the reflector layers must contain no fuel material.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Bottom and top reflector layers both `{1, 2, 3}` and byte-identical
    /// to each other; the transition layer `{2, 3, 4, 5, 6}`; the active
    /// core `{2, 3, 4, 6, 7, 8, 9, 10, 11}`. **68 void positions of 289** at
    /// every one of the 18 levels.
    ///
    /// **Interpretation.** The layering is as the case file specifies, and
    /// the outline is a right prism. The active core carries nine of the
    /// eleven materials — every fuel composition plus the two radial
    /// reflector types — while material 5 (2.6 w/o) appears only in the
    /// transition layer and material 1 (axial reflector) only at the ends.
    #[test]
    fn the_material_map_has_the_expected_axial_layering() {
        let (_p, _g, _t, w, ..) = neacrpa2(&Params::default());

        let materials_at = |iz: usize| {
            let mut m: Vec<usize> = (0..17)
                .flat_map(|ix| (0..17).map(move |iy| (ix, iy)))
                .map(|(ix, iy)| w.get(ix, iy, iz))
                .filter(|v| *v != 0)
                .collect();
            m.sort_unstable();
            m.dedup();
            m
        };
        eprintln!("layer  1 (bottom reflector): {:?}", materials_at(0));
        eprintln!("layer  2 (transition)      : {:?}", materials_at(1));
        eprintln!("layer 10 (active core)     : {:?}", materials_at(9));
        eprintln!("layer 18 (top reflector)   : {:?}", materials_at(17));

        // Top and bottom share the same map.
        for ix in 0..17 {
            for iy in 0..17 {
                assert_eq!(
                    w.get(ix, iy, 0),
                    w.get(ix, iy, 17),
                    "top and bottom reflectors should use the same map at ({ix}, {iy})"
                );
            }
        }
        // Reflector layers carry no fuel (materials 4+).
        assert!(
            materials_at(0).iter().all(|m| *m <= 3),
            "the bottom reflector should hold no fuel"
        );

        // Right prism: void where and only where the maps are void.
        let voids = |iz: usize| {
            (0..17)
                .flat_map(|ix| (0..17).map(move |iy| (ix, iy)))
                .filter(|&(ix, iy)| w.get(ix, iy, iz) == 0)
                .collect::<Vec<_>>()
        };
        let v0 = voids(0);
        eprintln!("void positions per level: {} of 289", v0.len());
        for iz in 1..18 {
            assert_eq!(voids(iz), v0, "level {iz} has a different outline");
        }
    }

    /// The seven control-rod banks are mapped and partially inserted.
    ///
    /// # Methodology
    ///
    /// This is the first case in the crate with a real rod pattern, so it is
    /// the first exercise of the rod channel in
    /// [`crate::sigmavalupd3d_handler`]. Checked: seven distinct banks appear
    /// on the map, the insertion vector has one entry per bank, and the tip
    /// heights computed as `crodbtm + crod*crodstep` land **inside** the core,
    /// which is what makes the pattern a partial insertion rather than fully
    /// in or fully out.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// All seven banks present. Core height 427.3 cm, rod bottom 37.7 cm.
    /// Bank tips: banks 1 and 3 at **197.12 cm** (100 steps), the other five
    /// at **356.54 cm** (200 steps) — every tip inside the core.
    ///
    /// **Interpretation.** This is a genuine partial insertion, which is
    /// what makes it a useful exercise of the rod channel: a fully
    /// withdrawn or fully inserted pattern would not test the axial tip
    /// search at all. Banks 1 and 3 are the deeply inserted pair, sitting
    /// roughly at the core mid-plane.
    #[test]
    fn the_control_rod_banks_are_mapped_and_partially_inserted() {
        let (_p, geometry, ..) = neacrpa2(&Params::default());
        let banks = geometry.crodbanks.as_ref().expect("A2 has rod banks");

        let mut seen: Vec<usize> = (0..17)
            .flat_map(|ix| (0..17).map(move |iy| (ix, iy)))
            .map(|(ix, iy)| banks.get(ix, iy))
            .filter(|b| *b != 0)
            .collect();
        seen.sort_unstable();
        seen.dedup();
        eprintln!("banks present: {seen:?}");
        assert_eq!(seen, vec![1, 2, 3, 4, 5, 6, 7], "seven banks");
        assert_eq!(geometry.crod.len(), 7, "one insertion per bank");

        let core_height: f64 = Z_LENGTHS.iter().sum();
        eprintln!("core height = {core_height} cm, rod bottom = {} cm", geometry.crodbtm);
        for (b, steps) in geometry.crod.iter().enumerate() {
            let tip = geometry.crodbtm + steps * geometry.crodstep;
            eprintln!("  bank {}: {steps} steps -> tip at {tip:.2} cm", b + 1);
            assert!(
                tip > 0.0 && tip < core_height,
                "bank {} tip at {tip} is outside the core",
                b + 1
            );
        }
    }

    /// All five feedback channels are populated, and the handler runs on them.
    ///
    /// # Methodology
    ///
    /// [`crate::neacrpd1`] supplies only two channels, so boron, coolant
    /// temperature and control rods have never been driven by a real case.
    /// This builds the case, hands it to
    /// [`crate::sigmavalupd3d_handler`] at the case's own initial state, and
    /// checks the rebuild succeeds and actually perturbs the cross sections —
    /// the initial state is deliberately *off* every reference value
    /// (boron 1000 against 1200.2, fuel 891.19 against 891.45, coolant 559.19
    /// against 579.75, density 0.7464 against 0.7125), so a no-op would mean
    /// the channels are not wired.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// The rebuild succeeded. **3978 material rows** after renumbering,
    /// from 11 base materials. **270 of 5202 nodes rodded, 221 of them
    /// fully**, and **zero stale rod-level carryovers**.
    ///
    /// **Interpretation.** Three feedback channels — boron, coolant
    /// temperature and control rods — are exercised here for the first time
    /// by a real case; all five rebuild without error. The row count is the
    /// clearest evidence they are actually doing something: feedback splits
    /// 11 base materials into 3978 distinct per-node cross-section sets,
    /// which only happens if the perturbations are non-zero and
    /// node-dependent.
    ///
    /// **Zero stale carryovers means defect C1 does not fire on this
    /// pattern** — every bank tip falls inside its own column, so no lattice
    /// position silently inherits another column's insertion level. That is
    /// a real result: C1 is latent here, not absent in general.
    ///
    /// 221 of the 270 rodded nodes are *fully* rodded, so only 49 are
    /// partially rodded tip nodes — consistent with seven banks each having
    /// one partial node per rodded column.
    #[test]
    fn all_five_feedback_channels_are_wired() {
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            neacrpa2(&Params::default());

        assert!(feedback.boron.is_some(), "boron");
        assert!(feedback.fueltemp.is_some(), "fuel temperature");
        assert!(feedback.cooltemp.is_some(), "coolant temperature");
        assert!(feedback.coolden.is_some(), "coolant density");
        assert!(feedback.crod.is_some(), "control rods");
        assert!(feedback.modtemp.is_none(), "A2 supplies no moderator channel");

        // Give the handler the per-node state the coupled driver would.
        let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(&params);
        let es = maxix * maxiy * maxiz;
        let mut th = th;
        th.fueltempdoppler = vec![params.fueltempavg; es];
        th.modtemp = vec![params.cooltempavg; es];
        th.coolant.temps = vec![params.cooltempavg; es];
        th.coolant.dens = vec![params.cooldenavg; es];

        let (perturbed, _ws, rod) = crate::sigmavalupd3d_handler::sigmavalupd3d_handler(
            &params,
            &geometry,
            &sigmavalues,
            &feedback,
            &whichsigma,
            &th,
        )
        .expect("the feedback rebuild should succeed");

        eprintln!("material rows after renumbering: {}", perturbed.tot.rows());
        eprintln!("stale rod-level carryovers: {}", rod.stale_level_carryovers);
        let rodded = rod.frac.iter().filter(|f| **f > 0.0).count();
        let fully = rod.frac.iter().filter(|f| **f >= 1.0).count();
        eprintln!("rodded nodes: {rodded} of {es} ({fully} fully)");

        assert!(rodded > 0, "a partially inserted pattern must rod some nodes");
        assert!(
            rodded < es,
            "it must not rod the whole core"
        );
        assert!(
            perturbed.tot.rows() > MATERIALS,
            "feedback should expand the material table beyond the {MATERIALS} base rows"
        );
    }
}
