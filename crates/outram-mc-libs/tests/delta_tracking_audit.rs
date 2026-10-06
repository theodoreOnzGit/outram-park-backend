//! **Delta-tracking audit, 2026-10-06** (gh:#599): one test per suspected
//! defect, each written so that it can fail.
//!
//! Every test compares delta tracking with the unbiased reference it must
//! reproduce: surface tracking on the identical geometry (OpenMC's method), or
//! an exact invariance. The model is small and uses LOW-tier
//! (`Nuclide::from_core`, windowed multipole) data so the whole file runs in
//! seconds; LOW-tier data is also the tier whose cross sections depend on the
//! material temperature, which two of the tests need.
//!
//! Methodology, predictions and results are in each test's doc comment.

use std::time::Instant;

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::keff_delta::{run_keff_delta_seq_in, DeltaDomain};
use outram_mc_libs::physics::keff::{ComputeType, KeffResult, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, run_keff_csg_hybrid, SourceBox};
use outram_mc_libs::tally::filter::{CellFilter, EnergyFilter, FilterKind};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const T_COLD: f64 = 293.6;
const T_HOT: f64 = 1200.0;

fn nuclides() -> Vec<Nuclide> {
    ["U235", "U238", "C0"]
        .iter()
        .map(|n| Nuclide::from_core(n).expect("embedded evaluation"))
        .collect()
}

fn comp(nuclide_idx: usize, atom_density: f64) -> NuclideComponent {
    NuclideComponent {
        nuclide_idx,
        atom_density,
    }
}

/// Fuel (LEU in graphite), graphite, dilute graphite "gas".
fn materials(t: f64) -> Vec<Material> {
    vec![
        Material {
            id: 1,
            name: "fuel".into(),
            temperature: t,
            components: vec![comp(0, 4.0e-4), comp(1, 4.0e-3), comp(2, 5.0e-2)],
        },
        Material {
            id: 2,
            name: "graphite".into(),
            temperature: t,
            components: vec![comp(2, 8.0e-2)],
        },
        Material {
            id: 3,
            name: "dilute gas".into(),
            temperature: t,
            components: vec![comp(2, 1.0e-5)],
        },
    ]
}

fn log_grid(n: usize) -> Vec<f64> {
    let (lo, hi) = (1.0e-5_f64, 2.0e7_f64);
    (0..n)
        .map(|i| (lo.ln() + (hi.ln() - lo.ln()) * i as f64 / (n - 1) as f64).exp())
        .collect()
}

/// What fills the middle shell of [`shells`].
#[derive(Clone, Copy)]
enum Middle {
    Graphite,
    Void,
}

/// A reflective sphere of radius `r_out` holding three concentric zones:
/// fuel (r < 2), the `middle` shell (2 < r < 3), dilute gas (3 < r < r_out).
/// `delta` decides whether the root cell declares delta tracking against
/// majorant 0; the geometry is otherwise identical.
fn shells(r_out: f64, middle: Middle, delta: bool) -> Geometry {
    let sphere = |r, bc| {
        SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r,
            bc,
        })
    };
    let hs = |surface_idx, sense| RegionToken::HalfSpace { surface_idx, sense };
    let shell = vec![
        hs(1, HalfSpaceSense::Outside),
        hs(2, HalfSpaceSense::Inside),
        RegionToken::Intersection,
    ];
    let root = Cell::fill(
        1,
        vec![hs(0, HalfSpaceSense::Inside)],
        CellFill::Universe(1),
        Position::ZERO,
    );
    Geometry {
        surfaces: vec![
            sphere(r_out, BoundaryType::Reflective),
            sphere(2.0, BoundaryType::Transmissive),
            sphere(3.0, BoundaryType::Transmissive),
        ],
        cells: vec![
            Cell::material(10, vec![hs(1, HalfSpaceSense::Inside)], 0, T_COLD),
            match middle {
                Middle::Graphite => Cell::material(11, shell, 1, T_COLD),
                Middle::Void => Cell::fill(11, shell, CellFill::Void, Position::ZERO),
            },
            Cell::material(12, vec![hs(2, HalfSpaceSense::Outside)], 2, T_COLD),
            if delta { root.delta_tracked(0) } else { root },
        ],
        universes: vec![
            Universe {
                id: 0,
                cell_indices: vec![3],
            },
            Universe {
                id: 1,
                cell_indices: vec![0, 1, 2],
            },
        ],
        lattices: vec![],
        root_universe: 0,
    }
}

fn settings(n: usize, inactive: usize, active: usize, run_t: f64, seed: u64) -> KeffSettings {
    KeffSettings {
        n_particles: n,
        n_inactive: inactive,
        n_active: active,
        temperature_k: run_t,
        seed,
        compute: ComputeType::CpuSingleThread,
        ..KeffSettings::default()
    }
}

fn cube(h: f64) -> SourceBox {
    SourceBox {
        lower: Position::new(-h, -h, -h),
        upper: Position::new(h, h, h),
    }
}

fn z(a: &KeffResult, b: &KeffResult) -> (f64, f64) {
    let dk = (a.k_mean - b.k_mean) * 1.0e5;
    let s = (a.k_std.powi(2) + b.k_std.powi(2)).sqrt() * 1.0e5;
    (dk, dk.abs() / s.max(1.0e-12))
}

/// **D1: `keff_delta` must not read the run temperature.**
///
/// Since GitHub #313 (2026-09-30) the CSG drivers take every temperature from
/// the MATERIAL: the cross sections of a multipole nuclide are broadened at
/// it, and its free-gas target moves at it (`transport_csg.rs`, "THE
/// COLLISION MATERIAL'S OWN TEMPERATURE"). `KeffSettings::temperature_k` is
/// read by nothing (`tests/temperature_precedence.rs`).
///
/// Methodology: an infinite medium of the fuel material at 1200 K
/// (reflective sphere, r = 10 cm, LOW-tier U-235/U-238/C), delta-tracked by
/// `run_keff_delta_seq_in`, 1000 × [5 + 20], seed 7. Two arms differ ONLY in
/// `KeffSettings::temperature_k` (1200 K and 293.6 K). Pass: bit-identical
/// `k`. The flight already uses the material's temperature
/// (`macro_xs_total_urr`), so before the fix the flight and the collision
/// partition disagreed whenever the two temperatures differed.
///
/// Prediction (before running): FAILS on the pre-audit kernel, which passed
/// the run temperature to `xs_at_energy` and `free_gas_kt` at the collision;
/// the 293.6 K arm should read a higher `k` (less Doppler capture in the
/// partition) by O(1000) pcm.
///
/// Results (2026-10-06, release, i9-13900K; gh:#720): before the fix the
/// arms differed in the bits (1200 K run 0.917517 ± 0.009973, 293.6 K run
/// 0.915830 ± 0.011633); the worth, measured by
/// [`keff_delta_run_temperature_worth`] at 8 seeds per arm, was
/// **+1053 ± 131 pcm (8.0 σ)**, the cold run higher, as predicted. After the
/// fix: bit-identical, k = 0.917517 ± 0.009973 in both.
#[test]
fn keff_delta_ignores_the_run_temperature() {
    let nucs = nuclides();
    let mats = materials(T_HOT);
    let maj = Majorant::from_materials(&mats[..1], &nucs, &log_grid(1024), 0.1);
    let domain = DeltaDomain::Sphere { radius: 10.0 };
    let fuel = |_p: Position| Some(0_usize);
    let hot = run_keff_delta_seq_in(
        domain,
        &mats,
        &nucs,
        &maj,
        fuel,
        &settings(1000, 5, 20, T_HOT, 7),
    );
    let cold = run_keff_delta_seq_in(
        domain,
        &mats,
        &nucs,
        &maj,
        fuel,
        &settings(1000, 5, 20, T_COLD, 7),
    );
    let (dk, zz) = z(&cold, &hot);
    println!(
        "keff_delta, materials at {T_HOT} K: run T {T_HOT} K k = {:.6} ± {:.6}; run T {T_COLD} K k = {:.6} ± {:.6}; {dk:+.0} pcm ({zz:.1} σ)",
        hot.k_mean, hot.k_std, cold.k_mean, cold.k_std
    );
    assert_eq!(
        hot.k_mean.to_bits(),
        cold.k_mean.to_bits(),
        "the run temperature moved keff_delta's k by {dk:+.0} pcm: the collision \
         must use the material's temperature, as the CSG drivers do (GitHub #313)"
    );
}

/// **D2: a void cell inside a delta-tracked region must be crossed, not lost.**
///
/// Methodology: the three-shell reflective sphere (r = 5 cm) with the middle
/// shell (2 < r < 3) a VOID cell (`CellFill::Void`). Surface tracking (the
/// reference) against the hybrid driver with the whole sphere delta-tracked,
/// 1000 × [5 + 20], seed 11. Pass: `|Δk| < 4 σ` combined, and the hybrid
/// arm's `k` must not be catastrophically low.
///
/// Prediction (before running): FAILS on the pre-audit kernel. Its
/// `material_at` closure maps a void cell to `None`, which
/// `bounded_delta_flight_visiting` reports as `Exhausted`, and the driver
/// scores that as a leak: every neutron with a tentative site in the shell is
/// silently killed in a reflective (leak-free) model.
///
/// Results (2026-10-06, gh:#719): before the fix surface 0.956113 ± 0.007873,
/// hybrid **0.018987 ± 0.000990** (−93 713 pcm, 118 σ) — the prediction held.
/// After: hybrid 0.979272 ± 0.009189 (+2316 pcm, 1.9 σ), passes.
#[test]
fn a_void_cell_inside_a_delta_region_is_crossed() {
    let nucs = nuclides();
    let mats = materials(T_COLD);
    let maj = Majorant::over_indices(&mats, &[0, 2], &nucs, &log_grid(1024), 0.1);
    let s = settings(1000, 5, 20, T_COLD, 11);
    let surf = run_keff_csg(
        &shells(5.0, Middle::Void, false),
        &mats,
        &nucs,
        cube(5.0),
        &s,
        None,
    );
    let hyb = run_keff_csg_hybrid(
        &shells(5.0, Middle::Void, true),
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        cube(5.0),
        &s,
        None,
    );
    let (dk, zz) = z(&hyb, &surf);
    println!(
        "void shell: surface k = {:.6} ± {:.6}, hybrid k = {:.6} ± {:.6}, {dk:+.0} pcm ({zz:.1} σ); hybrid virtual {}",
        surf.k_mean, surf.k_std, hyb.k_mean, hyb.k_std, hyb.virtual_collisions
    );
    assert!(
        hyb.virtual_collisions > 0,
        "the delta region was never entered"
    );
    assert!(
        zz < 4.0,
        "a void cell inside a delta region changed k by {dk:+.0} pcm ({zz:.1} σ)"
    );
}

/// **D3: a majorant built at one temperature is not a bound at another.**
///
/// Methodology: LOW-tier (multipole) fuel, whose `Σ_t` is Doppler-broadened
/// at the material temperature. `Majorant::bounding` (the construction that
/// is a bound by construction on pointwise data, gh:#585) over the fuel at
/// 293.6 K, 4096 bins × 16 subsamples, margin 0.05. The SAME majorant is then
/// audited (`Majorant::audit`, 200 001 log energies + breakpoints) against
/// the fuel at 293.6 K and at 1200 K.
///
/// Prediction (before running): at 293.6 K the worst ratio is ≤ 1; at 1200 K
/// Doppler broadening raises the resonance wings above the cold majorant, so
/// the worst ratio exceeds 1. That is a silent bias for any caller that
/// changes a material temperature after building the majorant (nee_soon's
/// `DirectCoupling::with_majorants` does exactly that each outer iteration),
/// which is why the transport must COUNT such sites rather than clamp them.
///
/// Results (2026-10-06, gh:#721): cold fuel worst 0.9893 at 1.598e4 eV;
/// fuel at 1200 K worst **1.3425 at 6.47 eV** (the U-238 6.67 eV wing). The
/// prediction held.
#[test]
fn a_cold_majorant_does_not_bound_hot_multipole_data() {
    let nucs = nuclides();
    let cold = materials(T_COLD);
    let hot = materials(T_HOT);
    let maj = Majorant::bounding(&cold[..1], &nucs, 1.0e-5, 2.0e7, 4096, 16, 0.05);
    let a_cold = maj.audit(&cold[..1], &nucs, 1.0e-5, 2.0e7, 200_000);
    let a_hot = maj.audit(&hot[..1], &nucs, 1.0e-5, 2.0e7, 200_000);
    println!(
        "cold majorant vs cold fuel: worst Σ_t/Σ_maj {:.4} at {:.4e} eV; vs fuel at {T_HOT} K: worst {:.4} at {:.4e} eV",
        a_cold.worst_ratio, a_cold.energy_ev, a_hot.worst_ratio, a_hot.energy_ev
    );
    assert!(
        a_cold.worst_ratio <= 1.0,
        "the cold majorant must bound the cold fuel"
    );
    assert!(
        a_hot.worst_ratio > 1.0,
        "expected the cold majorant to under-bound the hot fuel somewhere; if it \
         bounds it everywhere, the temperature hazard this test documents is not \
         real for this data"
    );
}

/// Flux and total by the three leaf cells × 2 groups.
fn cell_tally() -> Tally {
    Tally {
        id: 7,
        name: "cells".into(),
        filters: vec![
            FilterKind::Cell(CellFilter {
                cell_indices: vec![0, 1, 2],
            }),
            FilterKind::Energy(EnergyFilter {
                bins: vec![1.0e-5, 0.625, 2.0e7],
            }),
        ],
        scores: vec![ScoreType::Flux, ScoreType::Total],
        bins: vec![TallyBin::default(); 3 * 2 * 2],
    }
}

/// **D7: a `Cell` filter inside a delta region bins each tentative site in
/// the cell that holds it** — or, equivalently, every scored site lies in the
/// flight's starting cell.
///
/// gh:#598 left "the tentative estimator bins a `Cell` filter by the flight's
/// starting cell" as not done. Whether that is a defect depends on whether a
/// scored site can lie outside that cell. The hybrid driver scores only sites
/// before `d_bound`, the nearest surface over EVERY coordinate level, so no
/// scored site can leave the starting leaf cell. This test makes that claim
/// falsifiable.
///
/// Methodology: the three-shell reflective sphere (r = 5 cm, graphite middle
/// shell), 2000 × [5 + 20], seed 3. Surface tracking with track-length
/// tallies (the OpenMC estimator) against the hybrid driver with the
/// tentative-collision estimator, per cell and group: flux per source
/// neutron and `Σ_t = total/flux`. Pass: every flux within 4 σ and every
/// `Σ_t` within 4 σ (σ of the ratio taken as the quadrature sum, ignoring
/// the positive flux/rate correlation, so conservative).
///
/// Prediction (before running): passes. A defect would show as flux moved
/// from the graphite and gas shells into the fuel (the starting cell of a
/// fission-born flight) and as each cell's `Σ_t` drifting toward its
/// neighbours'.
///
/// Results (2026-10-06): passes; all six cell × group fluxes within 1.7 σ
/// and every `Σ_t` within 0.3 σ of surface track length (k surface
/// 1.153553 ± 0.006071, hybrid 1.150624 ± 0.005819). Not a defect today; it
/// would become one if gh:#722 lets flights run past the nearest surface.
#[test]
fn cell_filter_in_a_delta_region_matches_surface_track_length() {
    let nucs = nuclides();
    let mats = materials(T_COLD);
    let maj = Majorant::over_indices(&mats, &[0, 1, 2], &nucs, &log_grid(1024), 0.1);
    let s = settings(2000, 5, 20, T_COLD, 3);
    let n = 20_u64;
    let mut t_surf = cell_tally();
    let surf = run_keff_csg(
        &shells(5.0, Middle::Graphite, false),
        &mats,
        &nucs,
        cube(5.0),
        &s,
        Some(&mut t_surf),
    );
    let mut t_hyb = cell_tally();
    let hyb = run_keff_csg_hybrid(
        &shells(5.0, Middle::Graphite, true),
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        cube(5.0),
        &s,
        Some(&mut t_hyb),
    );
    println!(
        "k: surface {:.6} ± {:.6}, hybrid {:.6} ± {:.6}",
        surf.k_mean, surf.k_std, hyb.k_mean, hyb.k_std
    );
    for c in 0..3 {
        for g in 0..2 {
            let i = (c * 2 + g) * 2;
            let (fs, fh) = (&t_surf.bins[i], &t_hyb.bins[i]);
            let (xs, xh) = (&t_surf.bins[i + 1], &t_hyb.bins[i + 1]);
            let sf = (fs.mean(n), fs.mean(n) * fs.rel_std_dev(n));
            let hf = (fh.mean(n), fh.mean(n) * fh.rel_std_dev(n));
            let zf = (hf.0 - sf.0).abs() / (sf.1.hypot(hf.1)).max(1.0e-300);
            let sig = |f: &TallyBin, x: &TallyBin| {
                let v = x.mean(n) / f.mean(n);
                (v, v * f.rel_std_dev(n).hypot(x.rel_std_dev(n)))
            };
            let (ss, hs) = (sig(fs, xs), sig(fh, xh));
            let zs = (hs.0 - ss.0).abs() / (ss.1.hypot(hs.1)).max(1.0e-300);
            println!(
                "cell {c} group {g}: flux surface {:.5e} hybrid {:.5e} ({zf:.2} σ) | Σ_t surface {:.5} hybrid {:.5} ({zs:.2} σ)",
                sf.0, hf.0, ss.0, hs.0
            );
            assert!(zf < 4.0, "cell {c} group {g}: flux {zf:.2} σ apart");
            assert!(zs < 4.0, "cell {c} group {g}: Σ_t {zs:.2} σ apart");
        }
    }
}

/// **D4: does the hybrid driver's delta tracking skip internal surfaces?**
///
/// The point of delta tracking is to stream through internal surfaces
/// without stopping at them (`TrackingMethod`, `distance_out_of_level`). The
/// hybrid driver bounds every flight by `d_bound`, the nearest surface over
/// EVERY level, so it stops at each internal surface just as surface tracking
/// does and pays the virtual collisions on top.
///
/// Methodology: the three-shell reflective sphere, 2000 × [5 + 20], seed 5,
/// one thread; wall time of surface tracking and of the hybrid driver with
/// the whole sphere delta-tracked; the number of real collisions in each.
/// Reported, not gated on time (a timing gate is host-dependent); the gate is
/// only that `k` agrees within 4 σ.
///
/// Prediction (before running): the hybrid arm is SLOWER than surface
/// tracking, not faster, on a model whose internal surfaces it should skip.
///
/// Results (2026-10-06, gh:#722; Intel i9-13900K, 16 logical cores, 62.5 GiB,
/// Linux, CPU only, 1 thread, machine loaded by a parallel test suite):
/// surface 8.97 s, hybrid 22.29 s (2.48x); a second run 7.00 s / 16.14 s
/// (2.31x). Equal real collisions (3 668 614 vs 3 667 458), 39.4 M virtual;
/// k 1.155194 ± 0.007045 vs 1.150512 ± 0.005564 (0.5 σ).
#[test]
fn hybrid_cost_against_surface_tracking() {
    let nucs = nuclides();
    let mats = materials(T_COLD);
    let maj = Majorant::over_indices(&mats, &[0, 1, 2], &nucs, &log_grid(1024), 0.1);
    let s = settings(2000, 5, 20, T_COLD, 5);
    let t0 = Instant::now();
    let surf = run_keff_csg(
        &shells(5.0, Middle::Graphite, false),
        &mats,
        &nucs,
        cube(5.0),
        &s,
        None,
    );
    let t_surf = t0.elapsed().as_secs_f64();
    let t0 = Instant::now();
    let hyb = run_keff_csg_hybrid(
        &shells(5.0, Middle::Graphite, true),
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        cube(5.0),
        &s,
        None,
    );
    let t_hyb = t0.elapsed().as_secs_f64();
    let (dk, zz) = z(&hyb, &surf);
    println!(
        "hardware: {}",
        outram_mc_libs::perf_report::HardwareInfo::detect().headline()
    );
    println!(
        "surface: {t_surf:.2} s, k {:.6} ± {:.6}, {} collisions | hybrid: {t_hyb:.2} s, k {:.6} ± {:.6}, {} collisions, {} virtual | {dk:+.0} pcm ({zz:.1} σ) | time ratio hybrid/surface {:.2}",
        surf.k_mean, surf.k_std, surf.collisions, hyb.k_mean, hyb.k_std, hyb.collisions, hyb.virtual_collisions, t_hyb / t_surf
    );
    assert!(
        zz < 4.0,
        "hybrid and surface disagree: {dk:+.0} pcm ({zz:.1} σ)"
    );
}

/// **D1 worth, measured before the fix** (ignored: a measurement, ~5 min).
///
/// Methodology: as [`keff_delta_ignores_the_run_temperature`] at
/// 4000 × [10 + 40], 8 seeds per arm (seeds 1..=8), materials at 1200 K; arm
/// A run temperature 1200 K, arm B 293.6 K. Reports the pooled mean of each
/// arm and the difference with its standard error (seed-to-seed sd /√8 per
/// arm, combined in quadrature). After the fix the difference is exactly
/// zero by construction (the test above pins bit identity), so this only
/// prices the defect on the pre-fix kernel.
#[test]
#[ignore]
fn keff_delta_run_temperature_worth() {
    let nucs = nuclides();
    let mats = materials(T_HOT);
    let maj = Majorant::from_materials(&mats[..1], &nucs, &log_grid(1024), 0.1);
    let domain = DeltaDomain::Sphere { radius: 10.0 };
    let fuel = |_p: Position| Some(0_usize);
    let arm = |run_t: f64| -> Vec<f64> {
        (1..=8)
            .map(|seed| {
                run_keff_delta_seq_in(
                    domain,
                    &mats,
                    &nucs,
                    &maj,
                    fuel,
                    &settings(4000, 10, 40, run_t, seed),
                )
                .k_mean
            })
            .collect()
    };
    let stats = |v: &[f64]| {
        let n = v.len() as f64;
        let m = v.iter().sum::<f64>() / n;
        let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
        (m, sd / n.sqrt())
    };
    let (a, b) = (arm(T_HOT), arm(T_COLD));
    let ((ma, sa), (mb, sb)) = (stats(&a), stats(&b));
    let d = (mb - ma) * 1.0e5;
    let s = sa.hypot(sb) * 1.0e5;
    println!(
        "run T {T_HOT} K: k = {ma:.6} ± {sa:.6}; run T {T_COLD} K: k = {mb:.6} ± {sb:.6}; difference {d:+.0} ± {s:.0} pcm ({:.1} σ)",
        d.abs() / s
    );
}

/// **D3 fix: the transport COUNTS sites where the majorant fails to bound
/// `Σ_t`** (`KeffResult::majorant_violations`, gh:#721) instead of clamping
/// them silently.
///
/// Methodology: the three-shell reflective sphere with every material at
/// 1200 K (LOW-tier, Doppler-broadened at the material temperature), hybrid
/// driver, 500 × [2 + 5], seed 13. Arm A: majorant built over the 1200 K
/// materials. Arm B: majorant built over the same materials at 293.6 K, then
/// the materials heated (the `DirectCoupling::with_majorants` pattern).
/// Pass: A reports 0 violations and 0 delta-lost histories; B reports > 0
/// violations. Counting draws no random number, so neither arm's history
/// changes because of it.
///
/// Results (2026-10-06): hot majorant 0 violations, 0 delta-lost (k 1.076684
/// ± 0.035910); cold majorant on the hot materials **94 502 violations**,
/// 0 delta-lost (k 1.069428 ± 0.029139; the bias is not resolved at this
/// size, which is why the count, not k, is the instrument).
#[test]
fn majorant_violations_are_counted_not_hidden() {
    let nucs = nuclides();
    let hot = materials(T_HOT);
    let cold = materials(T_COLD);
    let s = settings(500, 2, 5, T_HOT, 13);
    let run = |maj: Majorant| {
        run_keff_csg_hybrid(
            &shells(5.0, Middle::Graphite, true),
            &hot,
            &nucs,
            std::slice::from_ref(&maj),
            None,
            cube(5.0),
            &s,
            None,
        )
    };
    let a = run(Majorant::over_indices(
        &hot,
        &[0, 1, 2],
        &nucs,
        &log_grid(1024),
        0.1,
    ));
    let b = run(Majorant::over_indices(
        &cold,
        &[0, 1, 2],
        &nucs,
        &log_grid(1024),
        0.0,
    ));
    println!(
        "hot majorant: k {:.6} ± {:.6}, {} violations, {} delta-lost, {} virtual | cold majorant on hot materials: k {:.6} ± {:.6}, {} violations, {} delta-lost",
        a.k_mean, a.k_std, a.majorant_violations, a.delta_lost, a.virtual_collisions,
        b.k_mean, b.k_std, b.majorant_violations, b.delta_lost
    );
    assert_eq!(
        a.majorant_violations, 0,
        "a majorant built on the run's materials was violated"
    );
    assert_eq!(a.delta_lost, 0);
    assert!(
        b.majorant_violations > 0,
        "a cold majorant on hot multipole materials should be violated somewhere"
    );
}
