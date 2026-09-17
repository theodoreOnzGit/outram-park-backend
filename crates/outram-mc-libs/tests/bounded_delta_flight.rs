//! **The boundary handoff for hybrid tracking** — `bn:op-867c.3`, gh #214.
//!
//! NEW WORK, no OpenMC counterpart.
//!
//! # What has to be true
//!
//! `bounded_delta_flight` truncates a delta flight at its region's boundary and
//! reports [`DeltaStep::Exit`] so the caller can continue under the enclosing
//! tracking method. The load-bearing claim is that **truncating is exactly
//! unbiased**: the flight length is exponential with rate `Σ_maj`, which is
//! memoryless, so `P(s > a + b | s > a) = P(s > b)` and cutting the flight at
//! `a` then resuming loses nothing.
//!
//! That claim is what lets a model be split into regions at all, so it is
//! tested directly rather than assumed — `truncation_is_unbiased` below runs
//! the same physical problem as one region and as two and requires the
//! collision-depth distributions to agree.
//!
//! A defect here would be **silent**: the answer would shift by an amount that
//! looks like statistics until someone runs enough histories.
//!
//! # Results (2026-09-17) — truncation is unbiased
//!
//! | | one region | two regions + handoff |
//! |---|---|---|
//! | collided fraction | 0.78875 | 0.78475 (**1.38 sigma**) |
//! | mean collision depth | 7.5739 cm | 7.5029 cm |
//!
//! 40,000 histories per arm, identical seed stream, B-10 at 2e-5 /b/cm,
//! thermal, L = 20 cm split at 10 cm. Splitting the region and handing off at
//! the seam changes neither how often a collision happens nor where — which is
//! the memorylessness argument holding in practice, not just on paper.
//!
//! Also measured: 1960 of 2000 flights through a thin slab exited, and **every
//! one landed on the boundary to within 1e-12 cm** rather than past it.

use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::{
    bounded_delta_flight, DeltaStep, Majorant,
};

const E: f64 = 0.0253; // thermal, where boron is strongest

fn boron() -> Option<Vec<Nuclide>> {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/endf/n-005_B_010-ENDF8.0.endf");
    p.exists().then_some(())?;
    Nuclide::from_endf_file(&p, "B10", 293.6, 1.0e-3)
        .ok()
        .map(|n| vec![n])
}

fn slab_material(atom_density: f64) -> Material {
    Material {
        id: 1,
        name: "absorber".into(),
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density }],
        temperature: 293.6,
    }
}

fn grid() -> Vec<f64> {
    (0..24).map(|i| 1.0e-3 * 10.0_f64.powf(i as f64 * 0.5)).collect()
}

/// A flight that reaches the boundary must land **exactly on it**, not past it.
///
/// This is the specific defect that makes the existing `delta_flight`
/// (`keff_delta.rs:453`) unusable for a handoff: it advances the full sampled
/// distance and only then tests, so on exit it reports a point already outside
/// the region.
#[test]
fn an_exiting_flight_lands_exactly_on_the_boundary() {
    let Some(nucs) = boron() else {
        eprintln!("SKIP: B-10 tape not in this checkout");
        return;
    };
    let mats = vec![slab_material(1.0e-6)]; // very thin: almost everything exits
    let maj = Majorant::from_materials(&mats, &nucs, &grid(), 0.3);
    let u = Direction::new(1.0, 0.0, 0.0);
    const EXIT_X: f64 = 5.0;

    let mut seed = 42_u64;
    let mut exits = 0;
    for _ in 0..2000 {
        let step = bounded_delta_flight(
            Position::ZERO,
            u,
            E,
            &maj,
            &mats,
            &nucs,
            10_000,
            |p: Position, _d: Direction| EXIT_X - p.x,
            |_p: Position| Some(0),
            &mut seed,
        );
        if let DeltaStep::Exit { position, .. } = step {
            exits += 1;
            assert!(
                (position.x - EXIT_X).abs() < 1.0e-12,
                "exit must be ON the boundary x = {EXIT_X}, got x = {}",
                position.x
            );
        }
    }
    println!("{exits} of 2000 flights exited; all landed on the boundary");
    assert!(exits > 1500, "a thin slab should mostly transmit, got {exits}/2000");
}

/// **The unbiasedness claim, tested directly.**
///
/// Run the identical physical problem two ways: as ONE region of length `L`,
/// and as TWO regions of length `L/2` with a handoff in the middle. The
/// distribution of where the first real collision happens must be the same.
///
/// If truncation lost or duplicated any probability, the two-region arm would
/// collide systematically earlier or later. Memorylessness says it cannot.
#[test]
fn truncation_is_unbiased() {
    let Some(nucs) = boron() else {
        eprintln!("SKIP: B-10 tape not in this checkout");
        return;
    };
    // Dense enough that most flights collide before the far boundary, so the
    // comparison is about collision depth rather than about transmission.
    let mats = vec![slab_material(2.0e-5)];
    let maj = Majorant::from_materials(&mats, &nucs, &grid(), 0.3);
    let u = Direction::new(1.0, 0.0, 0.0);
    const L: f64 = 20.0;
    const N: usize = 40_000;

    // Arm A: one region spanning [0, L].
    let mut seed_a = 7_u64;
    let (mut n_coll_a, mut sum_a) = (0usize, 0.0_f64);
    for _ in 0..N {
        if let DeltaStep::Collision { position, .. } = bounded_delta_flight(
            Position::ZERO, u, E, &maj, &mats, &nucs, 100_000,
            |p: Position, _d: Direction| L - p.x,
            |_p: Position| Some(0),
            &mut seed_a,
        ) {
            n_coll_a += 1;
            sum_a += position.x;
        }
    }

    // Arm B: two regions, [0, L/2] then [L/2, L], with a handoff at the seam.
    // The SAME seed stream, so the only difference is the split.
    let mut seed_b = 7_u64;
    let (mut n_coll_b, mut sum_b) = (0usize, 0.0_f64);
    for _ in 0..N {
        let first = bounded_delta_flight(
            Position::ZERO, u, E, &maj, &mats, &nucs, 100_000,
            |p: Position, _d: Direction| L * 0.5 - p.x,
            |_p: Position| Some(0),
            &mut seed_b,
        );
        match first {
            DeltaStep::Collision { position, .. } => {
                n_coll_b += 1;
                sum_b += position.x;
            }
            DeltaStep::Exit { position, direction, .. } => {
                // Handoff: resume in the second region from the boundary.
                if let DeltaStep::Collision { position, .. } = bounded_delta_flight(
                    position, direction, E, &maj, &mats, &nucs, 100_000,
                    |p: Position, _d: Direction| L - p.x,
                    |_p: Position| Some(0),
                    &mut seed_b,
                ) {
                    n_coll_b += 1;
                    sum_b += position.x;
                }
            }
            DeltaStep::Exhausted { .. } => {}
        }
    }

    let frac_a = n_coll_a as f64 / N as f64;
    let frac_b = n_coll_b as f64 / N as f64;
    let mean_a = sum_a / n_coll_a.max(1) as f64;
    let mean_b = sum_b / n_coll_b.max(1) as f64;

    // Binomial sigma on the collided fraction, combined over the two arms.
    let sd = |f: f64| (f * (1.0 - f) / N as f64).sqrt();
    let sigma_frac = (sd(frac_a).powi(2) + sd(frac_b).powi(2)).sqrt();
    let z_frac = (frac_a - frac_b).abs() / sigma_frac.max(1.0e-12);

    println!("            one region     two regions");
    println!("collided    {frac_a:.5}        {frac_b:.5}   ({z_frac:.2} sigma)");
    println!("mean depth  {mean_a:.4} cm     {mean_b:.4} cm");

    assert!(
        z_frac < 4.0,
        "splitting the region changed the collided fraction by {z_frac:.2} sigma \
         ({frac_a:.5} vs {frac_b:.5}). Truncation at a region boundary must be \
         UNBIASED -- the flight length is memoryless, so cutting and resuming \
         cannot move probability. A real difference here means the handoff is \
         losing or double-counting path length."
    );
    assert!(
        (mean_a - mean_b).abs() < 0.05 * L,
        "mean collision depth moved from {mean_a:.4} to {mean_b:.4} cm when the \
         region was split; the distribution must be unchanged, not merely the total"
    );
}

/// Exhaustion is reported, not silent. `keff_delta.rs`'s `delta_flight` returns
/// a bare `None` here, indistinguishable from a legitimate exit, so a lost
/// history shows up only as an unexplained leak (`bn:op-867c.5`).
#[test]
fn an_exhausted_budget_is_reported_with_its_count() {
    let Some(nucs) = boron() else {
        eprintln!("SKIP: B-10 tape not in this checkout");
        return;
    };
    // TWO materials: the majorant is bounded by the STRONG one, while
    // `material_at` always returns the weak one. So p_real = sigma_weak/maj is
    // tiny and virtual collisions dominate — which is exactly the situation a
    // global majorant creates, and the reason this variant has to exist.
    //
    // A single material cannot exercise this: the majorant is then only 1.3x
    // sigma_t, so 77 % of first attempts are REAL collisions and the budget is
    // never reached. (That is how the first version of this test failed.)
    let mats = vec![slab_material(1.0e-6), slab_material(1.0)];
    let maj = Majorant::from_materials(&mats, &nucs, &grid(), 0.3);
    let mut seed = 3_u64;
    // A budget of 1 against a far boundary: the flight cannot finish.
    let step = bounded_delta_flight(
        Position::ZERO,
        Direction::new(1.0, 0.0, 0.0),
        E,
        &maj,
        &mats,
        &nucs,
        1,
        |_p: Position, _d: Direction| f64::INFINITY,
        |_p: Position| Some(0),
        &mut seed,
    );
    match step {
        DeltaStep::Exhausted { virtual_collisions } => {
            println!("exhausted after {virtual_collisions} virtual collisions");
        }
        other => panic!("expected Exhausted with an infinite boundary and budget 1, got {other:?}"),
    }
}

/// A region whose majorant is zero — a void — is crossed ballistically and
/// reported as an exit, not as a lost history.
#[test]
fn a_void_region_is_crossed_not_lost() {
    let nucs: Vec<Nuclide> = Vec::new();
    let mats = vec![Material {
        id: 1,
        name: "void".into(),
        components: vec![],
        temperature: 293.6,
    }];
    let maj = Majorant::from_materials(&mats, &nucs, &grid(), 0.3);
    let mut seed = 11_u64;
    let step = bounded_delta_flight(
        Position::ZERO,
        Direction::new(1.0, 0.0, 0.0),
        E,
        &maj,
        &mats,
        &nucs,
        1000,
        |p: Position, _d: Direction| 4.0 - p.x,
        |_p: Position| Some(0),
        &mut seed,
    );
    match step {
        DeltaStep::Exit { position, .. } => {
            assert!((position.x - 4.0).abs() < 1.0e-12, "void crossed to the boundary");
        }
        other => panic!("a void region must be crossed, not lost; got {other:?}"),
    }
}
