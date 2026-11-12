//! **The GPU DEM timestep against the CPU reference** (gh:#1250).
//!
//! `gpu::dem::GpuDem` is a WGSL translation of `GranularSystem`'s step, in
//! `f32` with hi + lo positions, gathering each particle's contacts. It is
//! not bit-identical to the `f64` CPU path, so each case states its
//! tolerance and why, written before the first run (gh:#1250, 2026-11-05):
//!
//! 1. **Head-on Hertz pair**, 60 µm/step closing at 1 m/s, no gravity: the
//!    rebound velocity and the peak-free final state within **1e-5
//!    relative** (`f32` arithmetic over ~100 contact steps).
//! 2. **Oblique pair with friction and spin**: velocities and spins after
//!    the contact within **1e-4 relative** to the largest component (the
//!    spring's slip branch amplifies rounding).
//! 3. **A ball settling on the valve wall and the cylinder**: rest position
//!    within **1e-7 m** (the static Hertz overlap is ~10 µm).
//! 4. **An HTR-10 pour of 1 500 pebbles**, the conus mesh included: after
//!    2 000 steps (free fall, few contacts) the median pebble within
//!    **1e-6 m** of the CPU's; settled, whole-core φ and surface within 1 %
//!    and 1 pebble diameter of the CPU pour's (a different realisation once
//!    chaos acts); no table overflow. **CHANGED 2026-11-12 (maintainer,
//!    gh:#1503):** the settled bounds now apply to 8-seed MEANS, with a
//!    3-sem agreement gate beside them; see the test's own doc comment.
//! 5. **Balls against the HTR-10 discharge mesh** (added 2026-11-12,
//!    gh:#1503): a crease, a cone face and a quad diagonal, velocity within
//!    1e-5 and spin within 1e-4 relative.
//!
//! **First run (2026-11-05, lavapipe): all four failed, for two reasons.**
//! - *Precision, a real finding.* Head-on 1.76e-5 and oblique spin 1.32e-4
//!   against 1e-5 and 1e-4. The cause was the overlap `δ = (r_i + r_j) −
//!   |x_i − x_j|` formed in plain `f32`: a cancellation leaving ~4e-9 m
//!   (and the radius rounded to `f32`). The kernel now forms every overlap
//!   in double-f32; **the tolerances above were not changed.**
//! - *Two cases were built wrong, not the kernel.* Case 3 pushed the ball
//!   sideways, and without rolling resistance a rolling ball never stops: z
//!   agreed to 2.5e-9 m, x drifted 5e-4 m as it rolled. It is now two cases:
//!   **3a** a vertical drop onto the valve wall (rest within 1e-7 m) and
//!   **3b** a bounce off the cylinder without gravity (velocity within 1e-5
//!   relative, as case 1). Case 4's 1 500 pebbles all fit below the floor
//!   (the conus and tube hold ~2 450), so no pebble reached the core and
//!   neither pour could count as settled (φ NaN on both). It now pours
//!   **4 000**. Its early-time result passed as predicted: median 6.85e-7 m.
//!
//! **Second run: 4 of 5 passed once the compiler was stopped from
//! reassociating** (`keep` in `dem.wgsl`; lavapipe had dropped every lo
//! part): head-on 9.5e-7, oblique 1.1e-6 / 7.7e-7, resting ball 8.8e-10 m,
//! the 4 000 pour settled to ~~φ 0.4465 on both (surface 15.30 / 15.27 cm)~~
//! (old value superseded per maintainer, 2026-11-12).
//! The cylinder bounce (3b) measured 6.0e-5, then 4.7e-5 after more
//! partials were kept, against 1e-5. A single-contact force check located
//! it in the cylinder's double-f32 overlap (the flat wall's force agrees to
//! 1.4e-8); one partial sum there is still regrouped by the compiler.
//! The maintainer then set the acceptance criterion (2026-11-05: "we don't
//! need bit identity"; "the key thing ... is the packing fraction and
//! rdf"): the gate that decides is packing fraction and g(r) (case 4 here
//! and `examples/gpu_pour_compare.rs`). ~~3b's bound moved to 1e-4~~
//! **REVERTED the same day:** with every ordered partial sum kept (not only
//! the identities), the third run met every ORIGINAL tolerance: head-on
//! 8.8e-9, oblique 1.8e-7 / 3.2e-7, resting ball 4.8e-13 m, cylinder
//! 1.65e-6 / 9.7e-6; the 4 000 pour settled to ~~φ 0.4459 (CPU 0.4465),
//! surface 15.24 cm (CPU 15.30)~~ (old value superseded per maintainer,
//! 2026-11-12). Timing on lavapipe (software Vulkan, 4
//! vCPU): 4.0 ms/step against the CPU's 7.0 on 4 threads, early phase.
//!
//! **gh:#1503 (2026-11-12): the GPU's mesh wall caught up with the CPU's.**
//! On 2026-11-11 the CPU mesh wall became upstream's per-facet contact
//! (c5f524d03, gh:#1416); the kernel kept one contact with the nearest
//! facet, and the single-pour gate failed (CPU φ 0.4394, GPU 0.4459). The
//! kernel now carries the per-facet path (`gpu::dem` docs). Case 5 was
//! written for it, and its first runs found two things, both fixed without
//! moving a tolerance:
//! - *A case built wrong:* the cone ball started 1 mm inside the cone
//!   (ejected at 2.3 m/s on both engines). It was moved clear. The
//!   head-on diagonal ball had no spin, so its relative spin had no scale
//!   (CPU 1e-16); it was given one.
//! - *Two precision defects in the kernel.* With the face distance in f32
//!   (facets 0.75 m long against ~1e-4 m overlaps), the cone ball missed at
//!   1.93e-5; with the face distance in double-f32 it measures 2.8e-6. The
//!   barycentric region code in f32 put a pebble on a quad's diagonal on
//!   the inactive shared edge of both coplanar facets, so it lost the wall
//!   (rebound 0.342 against the CPU's 0.248 m/s, 38 %). With the code in
//!   double-f32 it measures 1.1e-7.
//!
//! Measured on case 5 (llvmpipe): crease 4.8e-8 / 6.2e-7, cone 2.8e-6 /
//! 1.1e-5, diagonal 1.1e-7 / 1.1e-6 (velocity / spin).
//!
//! A per-particle check of elastic forces found no missing or extra mesh
//! contact. It used one mid-drain state (seed 7, step 6 000) with the
//! velocities zeroed, so the force is elastic only. 6 of 4 000 pebbles
//! differ by more than 1e-3 relative, the worst by 4.3e-3, all small
//! forces at f32 resolution. Pebbles holding face and edge contacts at
//! once agree to <= 1e-3.
//!
//! The nearest-facet rule is not what moved the CPU's φ: restored on the
//! CPU, it gives 0.4393 at seed 7, against 0.4394.
//!
//! Skips (passes, printing why) when no GPU adapter exists; on a headless
//! Linux host Mesa's lavapipe (`mesa-vulkan-drivers`) provides one.

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::gpu;
use outram_park_fork_liggghts::gpu::dem::GpuDem;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::thermodynamic_temperature::kelvin;

fn ball(pos: Vec3, vel: Vec3, spin: Vec3) -> Particle {
    Particle::new(
        pos,
        vel,
        spin,
        Mass::new::<kilogram>(0.2),
        Length::new::<meter>(0.03),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("particle")
}

fn system(ps: Vec<Particle>, walls: Vec<Boundary>, g: f64) -> GranularSystem {
    let m = GranularMaterial::new(5e8, 0.2, 0.5, 0.1).expect("material");
    GranularSystem::new(
        ps,
        walls,
        GranularContactModel::hertz_history(m),
        Vec3::new(0.0, 0.0, g),
        3.5e-5,
    )
    .expect("system")
}

fn rel(a: Vec3, b: Vec3, scale: f64) -> f64 {
    a.sub(b).norm() / scale.max(1e-30)
}

/// Run `steps` on both backends; `None` without an adapter.
fn both(
    mut cpu: GranularSystem,
    steps: usize,
) -> Option<(GranularSystem, Vec<gpu::dem::GpuState>)> {
    let ctx = gpu::probe()?;
    eprintln!("adapter: {} ({:?})", ctx.info.name, ctx.info.backend);
    let mut g = GpuDem::new(ctx, &cpu).expect("supported");
    g.run(steps as u64);
    cpu.run(steps);
    let e = g.errors();
    assert_eq!((e.cell_overflow, e.slot_overflow), (0, 0), "table overflow");
    Some((cpu, g.states()))
}

#[test]
fn a_head_on_pair_rebounds_as_on_the_cpu() {
    let sys = system(
        vec![
            ball(
                Vec3::new(-0.0305, 0.0, 0.0),
                Vec3::new(0.5, 0.0, 0.0),
                Vec3::zero(),
            ),
            ball(
                Vec3::new(0.0305, 0.0, 0.0),
                Vec3::new(-0.5, 0.0, 0.0),
                Vec3::zero(),
            ),
        ],
        vec![],
        0.0,
    );
    let Some((cpu, g)) = both(sys, 400) else {
        return eprintln!("skipping: no GPU adapter");
    };
    for (c, s) in cpu.particles().iter().zip(&g) {
        let dv = rel(c.velocity, s.velocity, c.velocity.norm());
        eprintln!(
            "head-on: CPU v {:?}, GPU v {:?}, rel {dv:.2e}",
            c.velocity, s.velocity
        );
        assert!(c.velocity.x.abs() > 0.1, "it rebounded");
        assert!(dv < 1e-5, "{dv}");
        // The final state: the position within 1e-5 of the distance moved.
        let moved = c
            .position
            .sub(Vec3::new(
                if c.velocity.x > 0.0 { 0.0305 } else { -0.0305 },
                0.0,
                0.0,
            ))
            .norm();
        let dx = c.position.sub(s.position).norm();
        eprintln!("head-on position: |dx| {dx:.2e} m of {moved:.3e} m moved");
        assert!(dx < 1e-5 * moved, "{dx}");
    }
}

#[test]
fn an_oblique_pair_with_friction_and_spin_matches_the_cpu() {
    let sys = system(
        vec![
            ball(
                Vec3::new(-0.0305, 0.012, 0.0),
                Vec3::new(0.8, -0.1, 0.05),
                Vec3::new(0.0, 0.0, 30.0),
            ),
            ball(
                Vec3::new(0.0305, -0.012, 0.0),
                Vec3::new(-0.8, 0.1, 0.0),
                Vec3::new(5.0, 0.0, -10.0),
            ),
        ],
        vec![],
        0.0,
    );
    let Some((cpu, g)) = both(sys, 600) else {
        return eprintln!("skipping: no GPU adapter");
    };
    for (c, s) in cpu.particles().iter().zip(&g) {
        let dv = rel(c.velocity, s.velocity, c.velocity.norm());
        let dw = rel(
            c.angular_velocity,
            s.angular_velocity,
            c.angular_velocity.norm(),
        );
        eprintln!(
            "oblique: dv {dv:.2e}, dw {dw:.2e}; CPU w {:?}, GPU w {:?}",
            c.angular_velocity, s.angular_velocity
        );
        assert!(dv < 1e-5 && dw < 1e-4, "{dv} {dw}");
    }
}

#[test]
fn a_ball_dropped_on_the_valve_wall_rests_as_on_the_cpu() {
    let sys = system(
        vec![ball(Vec3::new(0.4, 0.1, -0.5), Vec3::zero(), Vec3::zero())],
        vec![
            Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.9).expect("cyl"),
            Boundary::wall(Vec3::new(0.0, 0.0, -0.6), Vec3::new(0.0, 0.0, 1.0)).expect("wall"),
        ],
        -9.81,
    );
    let Some((cpu, g)) = both(sys, 60_000) else {
        return eprintln!("skipping: no GPU adapter");
    };
    let (c, s) = (cpu.particles()[0], g[0]);
    let d = c.position.sub(s.position).norm();
    eprintln!(
        "resting ball: CPU {:?}, GPU {:?}, |dx| {d:.2e} m",
        c.position, s.position
    );
    assert!(
        c.position.z < -0.5699 && c.position.z > -0.5701,
        "rests on the floor: {}",
        c.position.z
    );
    assert!(d < 1e-7, "{d}");
}

#[test]
fn a_ball_bounces_off_the_cylinder_as_on_the_cpu() {
    let sys = system(
        vec![ball(
            Vec3::new(0.86, 0.0, 0.0),
            Vec3::new(0.7, 0.3, 0.0),
            Vec3::new(0.0, 0.0, 4.0),
        )],
        vec![Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.9).expect("cyl")],
        0.0,
    );
    let Some((cpu, g)) = both(sys, 600) else {
        return eprintln!("skipping: no GPU adapter");
    };
    let (c, s) = (cpu.particles()[0], g[0]);
    let dv = rel(c.velocity, s.velocity, c.velocity.norm());
    let dw = rel(
        c.angular_velocity,
        s.angular_velocity,
        c.angular_velocity.norm(),
    );
    eprintln!(
        "cylinder bounce: CPU v {:?}, GPU v {:?}, rel {dv:.2e}, spin rel {dw:.2e}",
        c.velocity, s.velocity
    );
    assert!(c.velocity.x < 0.0, "it bounced");
    assert!(dv < 1e-5 && dw < 1e-4, "{dv} {dw}");
}

/// **Case 5 (gh:#1503): balls against the HTR-10 discharge mesh**, no
/// gravity. One runs radially, spinning, into a crease of the tube (two
/// facets 3° apart: two face contacts under upstream's per-facet rule, one
/// under the nearest-facet rule the kernel had); one runs obliquely into the
/// conus near the cone-tube junction. Tolerances written before the first
/// run, as cases 1 and 2: velocity within 1e-5 relative, spin within 1e-4.
#[test]
fn balls_bounce_off_the_discharge_mesh_as_on_the_cpu() {
    use outram_park_fork_liggghts::htr10_fill::{discharge_mesh, CONE_HEIGHT_M};
    use outram_park_fork_liggghts::mesh_wall::{MovingBoundary, WallGeometry};
    let z_tube = -CONE_HEIGHT_M - 0.12;
    let sys = system(
        vec![
            ball(
                Vec3::new(0.21, 0.0, z_tube),
                Vec3::new(0.5, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 3.0),
            ),
            ball(
                Vec3::new(-0.1838, -0.1838, -CONE_HEIGHT_M + 0.06),
                Vec3::new(-0.4, -0.3, -0.2),
                Vec3::new(1.0, 0.0, 0.0),
            ),
            // At the middle of a tube quad's diagonal (facets 10 of each
            // band: angles 30° to 33°, z from the cone foot to the valve),
            // the edge two coplanar facets share and both mark inactive.
            ball(
                Vec3::new(
                    0.21 * (31.5f64).to_radians().cos(),
                    0.21 * (31.5f64).to_radians().sin(),
                    -CONE_HEIGHT_M - 0.125,
                ),
                Vec3::new(
                    0.5 * (31.5f64).to_radians().cos(),
                    0.5 * (31.5f64).to_radians().sin(),
                    0.0,
                ),
                Vec3::new(0.0, 0.0, 3.0),
            ),
        ],
        vec![],
        0.0,
    )
    .with_moving_walls(vec![MovingBoundary::new(
        WallGeometry::Mesh(discharge_mesh(120).expect("mesh")),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    )]);
    let v0 = [
        Vec3::new(0.5, 0.0, 0.0),
        Vec3::new(-0.4, -0.3, -0.2),
        Vec3::new(
            0.5 * (31.5f64).to_radians().cos(),
            0.5 * (31.5f64).to_radians().sin(),
            0.0,
        ),
    ];
    let Some((cpu, g)) = both(sys, 2_000) else {
        return eprintln!("skipping: no GPU adapter");
    };
    for (k, (c, s)) in cpu.particles().iter().zip(&g).enumerate() {
        let dv = rel(c.velocity, s.velocity, c.velocity.norm());
        let dw = rel(
            c.angular_velocity,
            s.angular_velocity,
            c.angular_velocity.norm(),
        );
        eprintln!(
            "mesh bounce {k}: CPU v {:?}, GPU v {:?}, rel {dv:.2e}; spin rel {dw:.2e}",
            c.velocity, s.velocity
        );
        assert!(c.velocity.sub(v0[k]).norm() > 0.1, "ball {k} hit the mesh");
        assert!(dv < 1e-5 && dw < 1e-4, "{dv} {dw}");
    }
}

/// Seeds of the case-4 ensemble, the same list on both engines.
const POUR_SEEDS: [u64; 8] = [7, 8, 9, 10, 11, 12, 13, 14];

/// Mean, sample sd and standard error of the mean.
fn mean_sd_sem(x: &[f64]) -> (f64, f64, f64) {
    let n = x.len() as f64;
    let m = x.iter().sum::<f64>() / n;
    let sd = (x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (n - 1.0)).sqrt();
    (m, sd, sd / n.sqrt())
}

/// **Case 4, an HTR-10 pour of 4 000 pebbles (the conus mesh included):
/// early per-pebble agreement, and settled φ and surface as N-SEED MEANS.**
///
/// **Why means (maintainer decision, 2026-11-12, gh:#1503).** Once chaos
/// acts, the two engines settle different realisations, so the previous
/// gate, 1 % between ONE CPU pour and ONE GPU pour, compared two random
/// draws. ~~Recorded 2026-11-05: φ 0.4459 (GPU) against 0.4465 (CPU),
/// surface 15.24 / 15.30 cm~~ **(old value superseded per maintainer,
/// 2026-11-12):** that CPU φ is not what the CPU path gives now (seed 7
/// measures 0.4394 with today's contact path and 0.4393 with the
/// nearest-facet mesh rule restored, so the shift is not the mesh rule), and
/// a single pair cannot resolve a 1 % bound against the scatter measured
/// below. The 1 % bound is kept and applied to the means.
///
/// **Gates.**
/// 1. Early (2 000 steps, free fall, few contacts), seed 7: median pebble
///    within **1e-6 m** of the CPU's (unchanged).
/// 2. `|mean_cpu − mean_gpu| / mean_cpu < 0.01` for whole-core φ, and
///    `|Δmean| < 0.06 m` (one pebble diameter) for the surface height.
/// 3. `|Δmean| <= 3 sqrt(sem_cpu² + sem_gpu²)` for both: the means agree
///    within their own uncertainty.
/// 4. Every pour settles, and no GPU table overflows.
///
/// **N, chosen before the full run (2026-11-12)** from the pours measured
/// while finding gh:#1503, all on today's code: CPU φ 0.4394, 0.4410,
/// 0.4437 (seeds 7, 8, 9: sd 0.0022); GPU φ 0.4293, 0.4396, 0.4470 (sd
/// 0.0089). With N = 8 per engine, sem(Δ) ≈ sqrt(0.0022² + 0.0089²)/√8 =
/// 0.0032, 0.7 % of φ: a true zero difference passes gate 2 about 4 times
/// in 5 and gate 3 almost always. Fewer seeds leave the 1 % bound below the
/// resolution of the means. Cost: ~12 min a seed (both engines, run one
/// after the other) on llvmpipe, about 100 min, so it runs under the
/// `long-tests` feature.
///
/// **Result (2026-11-12, llvmpipe: LLVM 20.1.2, 4 shared vCPUs, CPU 4
/// threads; whole `gpu_dem` file 6/6 pass in 11 395 s): PASS.**
///
/// | seed | CPU φ | GPU φ | CPU surface [m] | GPU surface [m] |
/// |---|---|---|---|---|
/// | 7 | 0.4394 | 0.4293 | 0.1530 | 0.1577 |
/// | 8 | 0.4410 | 0.4396 | 0.1533 | 0.1541 |
/// | 9 | 0.4437 | 0.4470 | 0.1528 | 0.1512 |
/// | 10 | 0.4359 | 0.4474 | 0.1550 | 0.1513 |
/// | 11 | 0.4441 | 0.4416 | 0.1534 | 0.1526 |
/// | 12 | 0.4481 | 0.4481 | 0.1518 | 0.1518 |
/// | 13 | 0.4383 | 0.4444 | 0.1539 | 0.1528 |
/// | 14 | 0.4480 | 0.4355 | 0.1517 | 0.1566 |
///
/// - φ: CPU mean 0.4423 (sd 0.0044, sem 0.0016); GPU mean 0.4416 (sd 0.0066,
///   sem 0.0023). Δ −0.0007, i.e. **−0.15 %** and −0.24 sem(Δ).
/// - Surface: CPU 0.1531 m (sem 0.0004); GPU 0.1535 m (sem 0.0009). Δ
///   **+0.4 mm**, +0.43 sem(Δ).
/// - Early phase, seed 7: median |dx| 6.82e-7 m.
///
/// The single-pour gate this replaces compared two realisations whose
/// per-pour sd (0.0044 / 0.0066) is itself ~1-1.5 % of φ. The prediction for
/// N = 8 (sem(Δ) ≈ 0.0032) held: the measured sem(Δ) is 0.0028.
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long test (~100 min on llvmpipe)")]
fn an_htr10_pour_matches_the_cpu_early_and_settled() {
    use outram_park_fork_liggghts::compute::ThreadCount;
    use outram_park_fork_liggghts::htr10_fill::{Htr10Fill, Htr10FillSettings, Htr10GpuFill};
    let settings = |seed: u64| Htr10FillSettings {
        n_pebbles: 4_000,
        threads: ThreadCount::Fixed(4),
        seed,
        ..Htr10FillSettings::default()
    };
    let (mut phi, mut top) = ([Vec::new(), Vec::new()], [Vec::new(), Vec::new()]);
    for (k, &seed) in POUR_SEEDS.iter().enumerate() {
        let Ok(mut g) = Htr10GpuFill::new(settings(seed)) else {
            return eprintln!("skipping: no GPU adapter");
        };
        let mut c = Htr10Fill::new(settings(seed)).expect("cpu");
        let (mut pc, mut pg) = (c.advance(2_000), g.advance(2_000));
        if k == 0 {
            eprintln!("adapter: {}", g.adapter());
            let mut d: Vec<f64> = c
                .centres()
                .iter()
                .zip(g.centres())
                .map(|(a, b)| a.sub(b).norm())
                .collect();
            d.sort_by(f64::total_cmp);
            eprintln!(
                "seed {seed}, 2 000 steps: median |dx| {:.2e} m, max {:.2e} m",
                d[d.len() / 2],
                d[d.len() - 1]
            );
            assert!(d[d.len() / 2] < 1e-6, "{}", d[d.len() / 2]);
            assert_eq!(pc.steps, pg.steps);
        }
        // Settle both (chunks of 2 000 steps, up to 60 000).
        while !(pc.settled || pc.gave_up) && pc.steps < 60_000 {
            pc = c.advance(2_000);
        }
        while !(pg.settled || pg.gave_up) && pg.steps < 60_000 {
            pg = g.advance(2_000);
        }
        let (hc, hg) = (
            pc.surface_height.get::<meter>(),
            pg.surface_height.get::<meter>(),
        );
        eprintln!(
            "seed {seed} settled: CPU {} steps, φ {:.4}, surface {:.4} m; GPU {} steps, φ {:.4}, surface {:.4} m",
            pc.steps, pc.phi_whole_core, hc, pg.steps, pg.phi_whole_core, hg
        );
        assert!(pc.settled && pg.settled, "seed {seed}");
        let e = g.errors();
        assert_eq!((e.cell_overflow, e.slot_overflow), (0, 0), "seed {seed}");
        phi[0].push(pc.phi_whole_core);
        phi[1].push(pg.phi_whole_core);
        top[0].push(hc);
        top[1].push(hg);
    }
    let report = |name: &str, x: &[Vec<f64>; 2]| {
        let (c, g) = (mean_sd_sem(&x[0]), mean_sd_sem(&x[1]));
        let (dm, s) = (g.0 - c.0, (c.2 * c.2 + g.2 * g.2).sqrt());
        eprintln!(
            "{name}: CPU mean {:.4} sd {:.4} sem {:.4}; GPU mean {:.4} sd {:.4} sem {:.4}; Δ {dm:+.4} = {:+.2} sem(Δ), {:+.2} %",
            c.0, c.1, c.2, g.0, g.1, g.2, dm / s, 100.0 * dm / c.0
        );
        (c.0, dm, s)
    };
    let (pc, dphi, sphi) = report("φ", &phi);
    let (_, dtop, stop) = report("surface [m]", &top);
    assert!(dphi.abs() / pc < 0.01, "φ means {dphi}");
    assert!(dphi.abs() <= 3.0 * sphi, "φ means {dphi} vs sem {sphi}");
    assert!(dtop.abs() < 0.06, "surface means {dtop}");
    assert!(dtop.abs() <= 3.0 * stop, "surface means {dtop} vs sem {stop}");
}
