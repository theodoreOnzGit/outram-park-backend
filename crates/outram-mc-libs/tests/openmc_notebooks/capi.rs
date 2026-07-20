//! `capi` notebook -> outram-mc verification (PARTIAL-LIVE, op-6tz.20).
//!
//! Notebook: `capi.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Drives a simulation through OpenMC's in-memory
//! C-API (`openmc.lib`): `init` -> `simulation_init` -> step batches
//! (`next_batch`) -> **read and edit** tallies / cells / materials *live*
//! (in memory, without re-reading XML) -> `finalize`. The pedagogical point of
//! the notebook is that the whole model — geometry, materials, tallies, results —
//! is a live in-memory object graph you can introspect and mutate between runs.
//!
//! **What "capi" means for THIS crate.** `outram-mc-libs` is *already* a pure
//! in-memory Rust library — there is no XML layer, no ctypes boundary, no
//! `openmc.lib` shim to mirror. The faithful Rust-equivalent of the notebook's
//! C-API workflow is therefore: **build the geometry + materials as in-memory
//! Rust values, run a k-eigenvalue, read the results back
//! (k ± σ and a scored tally — the introspection analogue), then MUTATE a
//! material in memory and RE-RUN**, confirming the eigenvalue moves in the
//! physically correct direction. That mirrors the notebook's "edit the model
//! live and observe the effect", which is the core value of the C-API demo,
//! using this crate's native API
//! ([`outram_mc_libs::physics::transport_csg::run_keff_csg`]).
//!
//! **What stays a GAP (op-6tz.20).** True **batch-stepping** — the notebook's
//! `next_batch()` loop that hands control back to the user *between* batches so
//! they can introspect / edit mid-run — has no analogue here: `run_keff_csg`
//! runs the whole inactive+active power-iteration loop internally and returns
//! only the final `KeffResult`. So this is an **honest partial-live**: it
//! exercises the in-memory *build -> run -> introspect -> edit -> re-run* surface,
//! **not** mid-run batch stepping. That remaining slice (a resumable stepper
//! exposing per-batch state) is bead op-6tz.20 and is documented as still open.
//!
//! # V&V — methodology and results
//!
//! **Methodology (API-surface + monotonic-response, no reference k).** A finite,
//! **leakage-dominated** homogeneous HEU cylinder — Godiva atom densities
//! (ICSBEP HEU-MET-FAST-001), radius `R_CYL` = 6.0 cm, **vacuum** radial boundary
//! ([`BoundaryType::Vacuum`]), infinite in z — is built entirely in memory.
//! Analog transport, LOW-tier embedded data (`Nuclide::from_core`: WMP +
//! Watt-collapsed fast MGXS). Two runs share one deterministic seed:
//!  1. **Run A (nominal density).** Run `run_keff_csg` with a single-cell
//!     [`CellFilter`] flux + ν-fission tally. Read back the eigenvalue
//!     `k_A ± σ_A` **and** the tally (the "read tallies/materials live"
//!     analogue): assert the fuel cell accumulated strictly positive flux and
//!     positive fission production.
//!  2. **Edit the material in memory + Run B.** Scale *every* nuclide atom
//!     density in the HEU material up by `DENSITY_FACTOR` = 1.5× (a denser core),
//!     then re-run. Read back `k_B ± σ_B`.
//!
//! **Pass criterion (physics of the edit).** For a *fixed-size* leaky core,
//! increasing the atom density shortens every mean free path, which shrinks the
//! migration area `L² ∝ 1/Σ²` while the geometric buckling `B²` (set by the fixed
//! radius) is unchanged; `k ≈ k_inf / (1 + L² B²)` therefore **rises** toward
//! `k_inf`. So a denser core must give **`k_B > k_A`** by more than the combined
//! statistical uncertainty. (Note: this leakage mechanism is why the geometry is
//! deliberately finite/vacuum-bounded — in an *infinite* reflective medium
//! `k_inf = ν Σ_f / Σ_a` is intensive and would *not* respond to a uniform
//! density scale, so that geometry could not test the edit.) No benchmark k is
//! asserted; the reference is the sign/monotonicity of the response and the
//! read-back of a valid tally.
//!
//! **Results (measured 2026-07-17, this harness; deterministic seed, 500
//! particles, 20 inactive + 40 active).**
//! - Run A (nominal HEU, R = 6 cm): **k_A = 0.98609 ± 0.00910**; fuel-cell
//!   track-length flux = 1.303e5 (> 0) and ν-fission tally = 1.951e4 (> 0) read
//!   back live.
//! - Run B (1.5× denser HEU, same R): **k_B = 1.31288 ± 0.00984**.
//! - Response: **Δk = k_B − k_A = +0.3268** (≈ +24× the combined σ ≈ 0.01340) —
//!   a denser fixed-size core is markedly more reactive, the physically correct
//!   direction. The values are printed at run time (`--nocapture`) and recorded
//!   in `verification_and_validation/openmc_notebook_comparisons/capi.csv`.
//!   Interpretation: the in-memory build/run/introspect/edit-and-re-run surface
//!   behaves correctly; only mid-run batch-stepping (op-6tz.20) remains a gap.

use outram_mc_libs::geometry::cell::Cell;
use outram_mc_libs::geometry::cell::{HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, ZCylinder};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::tally::filter::CellFilter;
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

/// Radius of the finite HEU cylinder \[cm\] — small enough to leak substantially
/// (nominal-density k below 1) so the density edit has room to raise k.
const R_CYL: f64 = 6.0;

/// Factor by which every atom density is scaled for the in-memory "material
/// edit" run (a denser core). 1.5× is a large, unambiguous change.
const DENSITY_FACTOR: f64 = 1.5;

/// Godiva HEU atom densities (HEU-MET-FAST-001), atoms/barn·cm, scaled by
/// `factor` (1.0 = nominal). Nuclide indices: `0 U234, 1 U235, 2 U238`.
fn heu_material(factor: f64) -> Material {
    Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 * factor }, // U-234
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 * factor }, // U-235
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 * factor }, // U-238
        ],
    }
}

/// Nuclides referenced by [`heu_material`], all from the embedded CORE WMP
/// library: `0 U234, 1 U235, 2 U238`.
fn heu_nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ]
}

/// A finite homogeneous HEU cylinder (radius [`R_CYL`]) with a **vacuum** radial
/// boundary, infinite in z. One cell (index 0), material index 0. Outside the
/// cylinder is undefined ⇒ the vacuum boundary lets neutrons leak (radially).
fn cylinder_geometry() -> Geometry {
    let surfaces = vec![SurfaceKind::ZCylinder(ZCylinder {
        x0: 0.0,
        y0: 0.0,
        r: R_CYL,
        bc: BoundaryType::Vacuum,
    })];
    let fuel = Cell::material(
        1,
        vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
        0,
        293.6,
    );
    Geometry {
        surfaces,
        cells: vec![fuel],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Run the in-memory model once and return `(k ± σ, fuel-cell flux sum,
/// fuel-cell ν-fission sum)`. The tally read-back is the crate-native analogue of
/// the notebook's "read tallies live".
fn run_and_read(material: Material, nuclides: &[Nuclide], settings: &KeffSettings) -> (f64, f64, f64, f64) {
    let geom = cylinder_geometry();
    let materials = vec![material];

    // Single-cell flux + ν-fission tally (the "introspection" surface).
    let filter = CellFilter { cell_indices: vec![0] };
    let mut tally = Tally {
        id: 1,
        name: "fuel cell".into(),
        filters: vec![Box::new(filter)],
        scores: vec![ScoreType::Flux, ScoreType::NuFission],
        bins: vec![TallyBin::default(); 2], // 1 cell × 2 scores
    };

    let src = SourceBox {
        lower: Position::new(-R_CYL, -R_CYL, -1.0),
        upper: Position::new(R_CYL, R_CYL, 1.0),
    };
    let result = run_keff_csg(&geom, &materials, nuclides, src, settings, Some(&mut tally));
    // bins: [flux, nu_fission]
    (result.k_mean, result.k_std, tally.bins[0].sum, tally.bins[1].sum)
}

/// PARTIAL-LIVE (op-6tz.20): the crate-native analogue of the `capi` notebook's
/// in-memory *build -> run -> read tallies/materials live -> edit material ->
/// re-run* workflow. Asserts the introspection surface (positive flux + fission
/// read back) and that a denser fixed-size core is more reactive (monotonic
/// response). Mid-run batch-stepping (`next_batch`) stays a documented gap. See
/// the module V&V note for methodology and measured results.
#[test]
fn capi_in_memory_build_run_edit_rerun() {
    let nuclides = heu_nuclides();
    let settings = KeffSettings {
        n_particles: 500,
        n_inactive: 20,
        n_active: 40,
        ..KeffSettings::default()
    };

    // ── Run A: nominal density; read back k and the live tally ────────────────
    let mat_a = heu_material(1.0);
    let (k_a, s_a, flux_a, nufis_a) = run_and_read(mat_a, &nuclides, &settings);

    // ── Edit the material IN MEMORY: uniformly denser core ────────────────────
    let mat_b = heu_material(DENSITY_FACTOR);
    let (k_b, s_b, _flux_b, _nufis_b) = run_and_read(mat_b, &nuclides, &settings);

    let dk = k_b - k_a;
    let combined_sigma = (s_a * s_a + s_b * s_b).sqrt();

    eprintln!(
        "[capi] Run A (nominal HEU, R={R_CYL} cm): k = {k_a:.5} ± {s_a:.5}  |  \
         fuel flux = {flux_a:.4e}, ν-fis = {nufis_a:.4e}  ||  \
         Run B ({DENSITY_FACTOR}× denser): k = {k_b:.5} ± {s_b:.5}  ||  \
         Δk = {dk:+.5} (combined σ = {combined_sigma:.5})  \
         [monotonic-response test, NO notebook reference k]"
    );

    // ── Introspection surface: the live-read tally is valid & physical ────────
    assert!(k_a.is_finite() && k_a > 0.0, "Run A k must be finite & positive, got {k_a}");
    assert!(k_b.is_finite() && k_b > 0.0, "Run B k must be finite & positive, got {k_b}");
    assert!(flux_a > 0.0, "fuel cell accumulated no flux (tally read-back failed)");
    assert!(nufis_a > 0.0, "no fission production tallied in the fuel (tally read-back failed)");
    assert!(s_a < 0.02 && s_b < 0.02, "eigenvalue under-converged: σ_A={s_a}, σ_B={s_b}");

    // ── Monotonic response: a denser fixed-size (leaky) core is more reactive ──
    // Require the increase to exceed 3× the combined statistical uncertainty so
    // it is a real physical response, not noise.
    assert!(
        dk > 3.0 * combined_sigma,
        "denser core should raise k (less leakage): k_A={k_a} ± {s_a}, k_B={k_b} ± {s_b}, Δk={dk}"
    );
}
