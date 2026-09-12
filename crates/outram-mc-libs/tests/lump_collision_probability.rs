//! **The oracle for resonance absorption in an optically thick lump, and the
//! closed forms it is measured against** (gh:#178, beads `op-qho6`, `op-t9cr`).
//!
//! # Why these gates exist
//!
//! `examples/slowing_down_oracle.rs` settled the *energy* half of self-shielding
//! against an exact infinite-medium solution. The *spatial* half had an oracle
//! in the **transparent limit only**: `examples/lump_self_shielding_scan.rs`
//! reproduces the homogeneous answer to −0.51 % at 0.10 mean free paths and then
//! reports a monotone rise with lump size with nothing to check it against. The
//! FHR ring-RPT fuel annulus is ≈ 6.6 mean free paths thick at the 6.674 eV
//! U-238 peak, which is where the +3327 pcm of lumping reactivity is made and
//! where there was no reference at all.
//!
//! [`outram_mc_libs::physics::collision_probability`] supplies the missing
//! reference. These tests are its verification: the geometry half against closed
//! forms and exact identities, and then the coupled slowing-down solve against
//! the one configuration whose answer is known independently.
//!
//! Nothing here is a Monte Carlo comparison — that is
//! `examples/lump_self_shielding_scan.rs`'s job, and it can only be read once
//! the reference itself is verified.
use outram_mc_libs::physics::collision_probability::{
    first_flight, gauss_legendre, sphere_escape_probability,
};
use outram_mc_libs::vv::{assert_absolute, assert_relative};

/// **The track quadrature reproduces the closed-form escape probability of a
/// bare sphere over six decades of optical radius.**
///
/// # Methodology
///
/// A single-shell cell of radius 1 cm and `Σ_t = x` has, for a uniform isotropic
/// source, the Case–de Hoffmann–Placzek escape probability
///
/// ```text
/// P_esc(x) = (3/4x) · [1 − 1/(2x²) + (1/x + 1/(2x²)) e^{−2x}]
/// ```
///
/// which is `1 − (3/4)x + (2/5)x² − (1/6)x³ + …` as `x → 0` and `3/(4x) =
/// 1/(Σ_t l̄)` as `x → ∞`, with `l̄ = 4V/S`. `first_flight` computes the same
/// quantity by an entirely different route — a Gauss–Legendre quadrature over
/// the impact parameter of the analytic six-fold collision integral — and knows
/// nothing about the closed form.
///
/// `x` is swept over `10^{−3} … 10^{2}` at 10 points per decade, at 8, 16 and 48
/// Gauss nodes per impact-parameter interval, so **convergence is demonstrated
/// rather than assumed**.
///
/// # This gate has already caught a defect — in the oracle
///
/// The first draft of `sphere_escape_probability`'s small-`x` series carried
/// `−(1/5)x³` where the expansion gives `−(1/6)x³`. The quadrature disagreed by
/// 2.1e-6 at `x = 0.0398` and 3.3e-8 at `x = 0.01` — exactly `(1/5 − 1/6)x³`.
/// The numerical method was right and the hand-derived closed form was wrong.
/// That is the value of keeping two independent routes to the same number.
///
/// # Results (2026-09-12)
///
/// Worst relative departure over the whole sweep `x ∈ [10⁻³, 10²]`:
///
/// ```text
///   Gauss nodes per interval     worst |rel|
///                      8           1.09e-04
///                     16           6.62e-06
///                     48           1.54e-13
/// ```
///
/// Converged to round-off by 48 nodes, which is what the lumped solve uses for
/// its reference runs. The gate asserts 1e-12 at 48 nodes and only that the
/// 8-node run is within 1e-3, so the convergence claim is itself tested.
#[test]
fn first_flight_reproduces_the_closed_form_sphere_escape_probability() {
    let mut worst = [0.0_f64; 3];
    let nodes = [8usize, 16, 48];
    for (slot, &n) in nodes.iter().enumerate() {
        for decade in -30..=20 {
            let x = 10f64.powf(decade as f64 / 10.0);
            let cp = first_flight(&[1.0], &[x], n);
            let want = sphere_escape_probability(x);
            let rel = (cp.p_escape[0] / want - 1.0).abs();
            worst[slot] = worst[slot].max(rel);
        }
    }
    for (slot, &n) in nodes.iter().enumerate() {
        println!(
            "  {n:3} Gauss nodes: worst |rel| vs the closed form = {:.2e}",
            worst[slot]
        );
    }
    assert!(
        worst[0] < 1.0e-3,
        "the 8-node quadrature is off by {:.2e}; it is meant to be crude but usable, \
         and if it is this far out the segment construction is wrong rather than the \
         quadrature being coarse",
        worst[0]
    );
    assert!(
        worst[2] < worst[0],
        "refining 8 -> 48 Gauss nodes did not improve the answer ({:.2e} -> {:.2e}); \
         the error is not quadrature error, so something else is wrong",
        worst[0],
        worst[2]
    );
    assert_absolute(
        "48-node track quadrature vs the closed-form sphere escape probability",
        worst[2],
        0.0,
        1.0e-12,
    );
}

/// **Surface reciprocity and neutron conservation hold on a four-shell cell
/// whose cross sections span four decades.**
///
/// # Methodology
///
/// Two identities that nothing in the quadrature enforces:
///
/// 1. **Reciprocity** `4 V_i Σ_i P_iS = A P_Si`. `P_iS` comes from a uniform
///    *volume* source in shell `i` escaping the cell; `P_Si` comes from an
///    isotropic *surface* source entering it. They are accumulated in separate
///    passes over the same tracks with different normalisations, so agreement is
///    a real check on both.
/// 2. **Conservation** `Σ_j P_ij = 1` after the white-boundary closure
///    `P_ij = P_ij⁰ + P_iS · P_Sj/(1 − P_SS)`. A Wigner–Seitz cell loses no
///    neutrons; if a row does not sum to one, the closure is wrong and the
///    lumped slowing-down solve built on it would silently gain or lose weight
///    at every lethargy step.
///
/// Radii 0.3 / 0.6 / 0.9 / 1.5 cm; three cross-section sets, including one that
/// puts a near-void (0.01 cm⁻¹) next to a near-black shell (200 cm⁻¹), which is
/// the configuration a resonance peak actually creates.
///
/// # Results (2026-09-12)
///
/// ```text
///   Σ_t per shell [cm⁻¹]          nodes   conservation   reciprocity
///   0.01 / 100 / 1 / 0.3          16/32/64   6.7e-16       1.8e-16
///   50 / 0.02 / 7 / 200           16/32/64   6.7e-16       1.3e-16
///   1 / 1 / 1 / 1                 16/32/64   8.9e-16       3.7e-16
/// ```
///
/// Both identities hold to double-precision round-off at every node count, so
/// the closure is exact rather than approximately exact.
#[test]
fn surface_reciprocity_and_conservation_hold_on_a_multi_shell_cell() {
    let radii = [0.3_f64, 0.6, 0.9, 1.5];
    let sets: [[f64; 4]; 3] = [
        [0.01, 100.0, 1.0, 0.3],
        [50.0, 0.02, 7.0, 200.0],
        [1.0, 1.0, 1.0, 1.0],
    ];
    let mut worst_cons = 0.0_f64;
    let mut worst_recip = 0.0_f64;
    for sigma in sets {
        for nodes in [16usize, 32, 64] {
            let cp = first_flight(&radii, &sigma, nodes);
            let cons = cp.conservation_defect();
            let recip = cp.reciprocity_defect(&sigma, radii[3]);
            println!("  sigma={sigma:?} nodes={nodes:3}  conservation {cons:.2e}  reciprocity {recip:.2e}");
            worst_cons = worst_cons.max(cons);
            worst_recip = worst_recip.max(recip);
        }
    }
    assert_absolute(
        "white-closure conservation, worst |sum_j P_ij - 1|",
        worst_cons,
        0.0,
        1.0e-13,
    );
    assert_absolute(
        "surface reciprocity 4 V_i Sigma_i P_iS = A P_Si, worst relative",
        worst_recip,
        0.0,
        1.0e-13,
    );
}

/// **Splitting a homogeneous sphere into sub-shells does not change its escape
/// probability.**
///
/// # Methodology and why it is not trivial
///
/// One sphere of `Σ_t R = 5` is re-described as 1, 2, 5 and 20 concentric shells
/// of the same material, and the volume-weighted aggregate escape probability
/// `Σ_i V_i P_iS / Σ_i V_i` is compared against the single-region answer.
///
/// The two calculations are structurally different: the 20-shell run builds
/// tracks with up to 39 segments, accumulates 39² ordered segment pairs, and
/// divides by twenty different `V_i Σ_i`. If the segment ordering, the
/// attenuation between segments, or the turning-point segment at the closest
/// approach were wrong, the split answers would drift from the whole. This is
/// also the invariance the multi-region slowing-down solve leans on when it
/// refines the flat-flux discretisation.
///
/// # Results (2026-09-12)
///
/// ```text
///   sub-shells      aggregate P_esc        relative vs 1 shell
///        1        0.147001498197682             —
///        2        0.147001498197682          2.2e-16
///        5        0.147001498197682          2.2e-16
///       20        0.147001498197682          4.4e-16
/// ```
#[test]
fn splitting_a_homogeneous_sphere_does_not_change_its_escape_probability() {
    let (sigma, r) = (5.0_f64, 1.0_f64);
    let base = first_flight(&[r], &[sigma], 48).p_escape[0];
    println!("  1 shell: aggregate P_esc = {base:.15}");
    for k in [2usize, 5, 20] {
        let radii: Vec<f64> = (1..=k).map(|i| r * i as f64 / k as f64).collect();
        let cp = first_flight(&radii, &vec![sigma; k], 48);
        let v_tot: f64 = cp.volume.iter().sum();
        let agg: f64 = (0..k).map(|i| cp.volume[i] * cp.p_escape[i]).sum::<f64>() / v_tot;
        println!(
            "  {k:2} shells: aggregate P_esc = {agg:.15}  rel {:.2e}",
            (agg / base - 1.0).abs()
        );
        assert_relative(
            "aggregate escape probability is invariant under sub-shelling",
            agg,
            base,
            1.0e-13,
        );
    }
}

/// **The Gauss–Legendre rule the oracle rests on integrates polynomials
/// exactly.**
///
/// # Methodology
///
/// An `n`-point Gauss–Legendre rule is exact for degree `2n − 1`. The nodes and
/// weights are generated here by Newton iteration on the Legendre recurrence
/// rather than pulled from a dependency, so they are checked against
/// `∫_{−1}^{1} x^{2m} dx = 2/(2m+1)` for every even moment up to `2n − 2`, at
/// `n = 2, 4, 8, 16, 32, 48, 64`.
///
/// # Result (2026-09-12)
///
/// Worst relative error over all `(n, m)`: **1.68e-14**. Nodes are symmetric to
/// **0** (bit-for-bit, by construction) and the weights sum to 2 to **4.4e-16**.
#[test]
fn gauss_legendre_integrates_polynomials_exactly() {
    let mut worst = 0.0_f64;
    let mut worst_sym = 0.0_f64;
    let mut worst_sum = 0.0_f64;
    for n in [2usize, 4, 8, 16, 32, 48, 64] {
        let (x, w) = gauss_legendre(n);
        worst_sum = worst_sum.max((w.iter().sum::<f64>() - 2.0).abs());
        for i in 0..n {
            worst_sym = worst_sym.max((x[i] + x[n - 1 - i]).abs());
        }
        for m in 0..n {
            let got: f64 = x
                .iter()
                .zip(&w)
                .map(|(a, b)| b * a.powi(2 * m as i32))
                .sum();
            let want = 2.0 / (2.0 * m as f64 + 1.0);
            worst = worst.max((got / want - 1.0).abs());
        }
    }
    println!(
        "  worst moment {worst:.2e}, node symmetry {worst_sym:.2e}, weight sum {worst_sum:.2e}"
    );
    assert_absolute("Gauss-Legendre even moments", worst, 0.0, 1.0e-12);
    assert_absolute("Gauss-Legendre node symmetry", worst_sym, 0.0, 1.0e-14);
    assert_absolute("Gauss-Legendre weights sum to 2", worst_sum, 0.0, 1.0e-13);
}

// ─────────────────────────────────────────────────────────────────────────────
// Data-gated: the coupled solve, and the measurement it was built for
// ─────────────────────────────────────────────────────────────────────────────

/// Room temperature, matching `examples/lump_self_shielding_scan.rs`.
const TEMP_K: f64 = 293.6;
/// C-12 potential scattering \[b\], for expressing the dilution as a background
/// cross section per U-238 atom.
const SIGMA_P_C12: f64 = 4.7392;
/// 10 keV source — inside U-238's resolved resonance range and below its 44.9 keV
/// first inelastic level — scored down to 1 eV.
const BAND: SlowingDownBand = SlowingDownBand {
    e_top: 10.0e3,
    e_bot: 1.0,
};
/// Background cross section per U-238 atom, cell-averaged.
const SIGMA_B: f64 = 300.0;
/// Fuel volume fraction of the Wigner-Seitz cell.
const VFRAC: f64 = 0.30;
/// Cell-averaged U-238 atom density \[atoms·barn⁻¹·cm⁻¹\].
const N_U8: f64 = 1.0e-3;

use outram_mc_libs::geometry::surface::BoundaryType;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::pebble_beds::fhr_pebble::fhr_pebble_geometry;
use outram_mc_libs::physics::slowing_down::{
    solve_deterministic, solve_deterministic_multiregion, CellBoundary, LumpCellMc, MixComponent,
    ScatterKernel, ShellCell, SlowingDownBand,
};

/// `Some(nuclide)` when the tape is present, else `None` after a skip note — the
/// repo-only `reference-data/endf/` is not packaged, and a data-gated test
/// passes rather than fails when it is absent.
fn nuclide_or_skip(name: &str, file: &str) -> Option<outram_mc_libs::prelude::Nuclide> {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::prelude::Nuclide;
    let Some(path) = reference_endf(file) else {
        println!("[{name}] SKIP: {file} not in reference-data/endf/ (set OUTRAM_PARK_ENDF_DIR)");
        return None;
    };
    match Nuclide::from_endf_file(&path, name, TEMP_K, 1.0e-3) {
        Ok(n) => Some(n),
        Err(e) => {
            println!("[{name}] SKIP: reconstruction failed: {e}");
            None
        }
    }
}

/// C-12 + U-238 and their union energy grid, reconstructed once and shared.
fn medium() -> Option<&'static (Vec<outram_mc_libs::prelude::Nuclide>, Vec<f64>)> {
    use std::sync::OnceLock;
    static MEDIUM: OnceLock<Option<(Vec<outram_mc_libs::prelude::Nuclide>, Vec<f64>)>> =
        OnceLock::new();
    MEDIUM
        .get_or_init(|| {
            let c12 = nuclide_or_skip("C12", "n-006_C_012-ENDF8.0.endf")?;
            let u238 = nuclide_or_skip("U238", "n-092_U_238.endf")?;
            let mut grid = u238.native_energy_grid(BAND.e_bot * 0.9, BAND.e_top * 1.1);
            grid.extend_from_slice(&c12.native_energy_grid(BAND.e_bot * 0.9, BAND.e_top * 1.1));
            grid.sort_by(|a, b| a.partial_cmp(b).expect("finite grid"));
            grid.dedup_by(|a, b| (*a - *b).abs() <= 1.0e-12 * b.abs());
            Some((vec![c12, u238], grid))
        })
        .as_ref()
}

fn components(v: &[(usize, f64)]) -> Vec<NuclideComponent> {
    v.iter()
        .map(|&(nuclide_idx, atom_density)| NuclideComponent {
            nuclide_idx,
            atom_density,
        })
        .collect()
}

/// **A shell cell filled with ONE material returns the infinite-medium solution
/// identically — at every size, and under every subdivision.**
///
/// # Why this is the test that cannot be fudged
///
/// When every shell carries the same material, reciprocity plus conservation
/// give `Σ_j V_j P_{j→i} = V_i`, so `C_i ∝ V_i` solves the coupled system and
/// the whole spatial apparatus must cancel: the answer has to be the infinite
/// homogeneous one, which `solve_deterministic` produces by a completely
/// different route (no geometry, no collision probabilities, no white closure).
///
/// Every ingredient is exercised and none is free to be wrong:
///
/// - the `V_i Σ_i` normalisation of each row — a wrong power of the radius
///   breaks it immediately;
/// - the white-boundary closure `P_ij⁰ + P_iS P_Sj/(1−P_SS)` — without it the
///   cell leaks and the escape probability comes out high;
/// - the per-shell lethargy march and the `G_{i,k}` lookback, now carried
///   independently in each of up to 24 shells;
/// - the impact-parameter quadrature at optical radii spanning **four decades**
///   (R = 0.01 cm is transparent at the resonance peaks, R = 100 cm is black).
///
/// # Methodology and results (2026-09-12, ENDF/B-VIII.0 @ 293.6 K)
///
/// U-238 + C-12 at σ_b = 300 b, 10 keV → 1 eV, U-238's own reconstructed energy
/// grid (230 779 lethargy points after the ε/20 cap). Exact homogeneous
/// solution `p_esc = 0.387629247041`.
///
/// ```text
///   R_cell [cm]   shells   multi-region p_esc     relative
///        0.01        3       0.387629247042       +9.7e-13
///        0.01        9       0.387629247042       +1.1e-12
///        0.01       24       0.387629247042       +9.6e-13
///        1.00        3       0.387629247042       +9.4e-13
///        1.00        9       0.387629247042       +1.1e-12
///        1.00       24       0.387629247042       +9.3e-13
///      100.00        3       0.387629247042       +9.7e-13
///      100.00        9       0.387629247042       +1.1e-12
///      100.00       24       0.387629247042       +9.7e-13
///   ```
///
/// Worst conservation defect `|Σ_j P_ij − 1|` over the whole sweep: **1.6e-14**.
///
/// The test runs the 3- and 9-shell cases at two sizes; the 24-shell rows above
/// cost ~73 s each and are recorded rather than re-run.
#[test]
fn a_homogeneous_shell_cell_reproduces_the_infinite_medium_solution() {
    let Some((nuclides, grid)) = medium() else {
        return;
    };
    let n_c = SIGMA_B / SIGMA_P_C12 * N_U8;
    let mix = vec![
        MixComponent {
            nuclide_idx: 0,
            atom_density: n_c,
        },
        MixComponent {
            nuclide_idx: 1,
            atom_density: N_U8,
        },
    ];
    let exact = solve_deterministic(nuclides, &mix, BAND, TEMP_K, grid);
    println!("  exact infinite-medium p_esc = {:.12}", exact.escaped);
    let mats = vec![Material {
        id: 3,
        name: "homogenised".into(),
        temperature: TEMP_K,
        components: components(&[(0, n_c), (1, N_U8)]),
    }];
    let mut worst_cons = 0.0_f64;
    for r in [0.01_f64, 100.0] {
        for k in [1usize, 3] {
            let cell = ShellCell {
                radii: vec![0.4 * r, 0.7 * r, r],
                material: vec![0, 0, 0],
            }
            .subdivide(k);
            let got =
                solve_deterministic_multiregion(&cell, &mats, nuclides, BAND, TEMP_K, grid, 16, 20);
            println!(
                "  R = {r:8.2} cm, {:2} shells: p_esc = {:.12}  rel {:+.2e}  conservation {:.2e}",
                cell.radii.len(),
                got.escaped,
                got.escaped / exact.escaped - 1.0,
                got.worst_conservation_defect
            );
            worst_cons = worst_cons.max(got.worst_conservation_defect);
            assert_relative(
                "multi-region cell of ONE material vs the infinite-medium solution",
                got.escaped,
                exact.escaped,
                1.0e-10,
            );
        }
    }
    assert_absolute(
        "worst white-closure conservation defect over the sweep",
        worst_cons,
        0.0,
        1.0e-12,
    );
}

/// **The Monte Carlo's resonance absorption in an OPTICALLY THICK lump matches a
/// deterministic solution of the same equation on the same geometry.**
///
/// # The candidate this rules on (gh:#178, beads `op-qho6`, `op-t9cr`)
///
/// The FHR ring-RPT pebble sits +4004 pcm above its OpenMC reference, and the
/// residual survives in the **ring-RPT** model — four concentric spheres of
/// homogeneous materials, surface-tracked — so double heterogeneity and the
/// tracking method are both excluded. What was left was what this code does to
/// the flux inside an optically thick lump: the fuel annulus is 1.4934-1.7531 cm
/// and **6.6 mean free paths thick at the 6.674 eV U-238 peak**, it carries the
/// entire heavy-metal inventory, and that concentration is worth **+3327 pcm**
/// over naive homogenisation on this code's own numbers. Nothing checked whether
/// it was the right +3327 pcm; `examples/lump_self_shielding_scan.rs` anchored
/// only the *transparent* limit.
///
/// # Methodology
///
/// A Wigner-Seitz cell — U-238 lump in a graphite shell, white outer boundary,
/// cell-averaged σ_b = 300 b, fuel volume fraction 0.30 — run two independent
/// ways on the **same** geometry, sharing the reconstructed cross sections and
/// nothing else:
///
/// - `LumpCellMc`, which flies through `Geometry::locate`,
///   `distance_to_boundary` and `cross_surface` exactly as
///   `transport_csg::transport_history` does;
/// - `solve_deterministic_multiregion`, which has no Monte Carlo in it at all:
///   exact first-flight collision probabilities from an impact-parameter track
///   quadrature, recomputed at every lethargy point, closed with a white
///   boundary, coupled into the same Volterra slowing-down march that
///   `examples/slowing_down_oracle.rs` already validated in energy.
///
/// Both use `ScatterKernel::IsotropicCmAtRest`, which is the kernel the
/// deterministic side models exactly.
///
/// # Results (2026-09-12, ENDF/B-VIII.0 @ 293.6 K, seed 0xABCD_0003)
///
/// At 200 000 histories per row
/// (`MODE=thick HIST=50000 cargo run --release --features endf-pebble-cases
/// --example lump_self_shielding_scan`), exact homogeneous `p_esc = 0.38763`:
///
/// ```text
///   R/mfp    MC p_esc    1 sigma     ORACLE    MC - ORACLE      z
///    2.57     0.39811    0.00109    0.40013      -0.50 %     -1.84
///    8.56     0.43320    0.00111    0.43180      +0.32 %     +1.26
///   25.69     0.50082    0.00112    0.50059      +0.05 %     +0.20
/// ```
///
/// and on the **real ring-RPT annulus** (1.4934-1.7531 cm, 6.65 mean free paths
/// at the 6.674 eV peak, 200 000 histories) the lumping effect
/// `L = p_esc(annulus) - p_esc(homogenised)` comes out MC 0.124042 against the
/// reference's 0.125259: **-0.97 % +/- 0.72 %**, i.e. **-32 +/- 24 pcm** of the
/// ring-RPT pebble's +3327 pcm of lumping reactivity, against a residual of
/// +4004 pcm. The Monte Carlo absorbs 0.61 % MORE in the fuel than the
/// reference, which is the opposite sign to the "too much self-shielding"
/// hypothesis this was built to test.
///
/// This test re-runs two of those rows at 100 000 histories each, so its own
/// envelope is ~1.5 % on `p_esc`; the measurement lives in the example, and
/// what is guarded here is that the two methods do not drift apart.
#[test]
fn the_thick_lump_matches_a_deterministic_collision_probability_solution() {
    let Some((nuclides, grid)) = medium() else {
        return;
    };
    let n_c = SIGMA_B / SIGMA_P_C12 * N_U8;
    let mix = vec![
        MixComponent {
            nuclide_idx: 0,
            atom_density: n_c,
        },
        MixComponent {
            nuclide_idx: 1,
            atom_density: N_U8,
        },
    ];
    let hom = solve_deterministic(nuclides, &mix, BAND, TEMP_K, grid).escaped;
    let mats = vec![
        Material {
            id: 1,
            name: "U-238 lump".into(),
            temperature: TEMP_K,
            components: components(&[(1, N_U8 / VFRAC)]),
        },
        Material {
            id: 2,
            name: "graphite".into(),
            temperature: TEMP_K,
            components: components(&[(0, n_c / (1.0 - VFRAC))]),
        },
    ];
    let mut sigma_peak = 0.0_f64;
    let mut e = 6.0_f64;
    while e <= 7.5 {
        sigma_peak = sigma_peak.max(mats[0].macro_xs_total(e, nuclides));
        e += 5.0e-4;
    }
    println!("  exact homogeneous p_esc = {hom:.6}; lump Sigma_t at the 6.674 eV peak = {sigma_peak:.2} /cm");

    for (r_cell, histories) in [(0.5_f64, 100_000usize), (1.5, 100_000)] {
        let r_fuel = r_cell * VFRAC.cbrt();
        let mid = 0.5 * (r_fuel + r_cell);
        let geom = fhr_pebble_geometry(
            0.0,
            r_fuel,
            mid,
            r_cell,
            0,
            1,
            1,
            BoundaryType::Vacuum,
            TEMP_K,
        );
        let mc = LumpCellMc {
            histories,
            seed: 0xABCD_0003,
            kernel: ScatterKernel::IsotropicCmAtRest,
            boundary: CellBoundary::White,
            max_events: 40_000_000,
        }
        .run(&geom, &mats, nuclides, BAND, TEMP_K, r_cell);
        let cell = ShellCell {
            radii: vec![r_fuel, mid, r_cell],
            material: vec![0, 1, 1],
        }
        .subdivide_each(&[8, 1, 1]);
        let oracle =
            solve_deterministic_multiregion(&cell, &mats, nuclides, BAND, TEMP_K, grid, 16, 20);
        let se = mc.stderr_of(mc.escaped);
        println!(
            "  R_cell {r_cell:.2} cm ({:.2} mfp): MC {:.5} +/- {:.5}   oracle {:.5}   \
             {:+.2} %   {:.2} sigma   (lumping L: MC {:.5}, oracle {:.5})",
            r_fuel * sigma_peak,
            mc.escaped,
            se,
            oracle.escaped,
            100.0 * (mc.escaped / oracle.escaped - 1.0),
            (mc.escaped - oracle.escaped) / se,
            mc.escaped - hom,
            oracle.escaped - hom,
        );
        assert_absolute(
            "MC escape probability in a thick lump vs the deterministic \
             collision-probability solution",
            mc.escaped,
            oracle.escaped,
            4.0 * se + 0.002 * oracle.escaped,
        );
        assert_absolute(
            "no history left the Wigner-Seitz cell",
            mc.lost,
            0.0,
            1.0e-12,
        );
        assert!(
            r_fuel * sigma_peak > 6.0,
            "the lump is only {:.2} mean free paths across at the 6.674 eV peak; \
             this test exists to judge the DEEPLY self-shielded regime and would be \
             checking the transparent limit that `vv_gate` already covers",
            r_fuel * sigma_peak
        );
    }
}
