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
//!    chaos acts); no table overflow.
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
//! the 4 000 pour settled to φ 0.4465 on both (surface 15.30 / 15.27 cm).
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
//! 1.65e-6 / 9.7e-6; the 4 000 pour settled to φ 0.4459 (CPU 0.4465),
//! surface 15.24 cm (CPU 15.30). Timing on lavapipe (software Vulkan, 4
//! vCPU): 4.0 ms/step against the CPU's 7.0 on 4 threads, early phase.
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
    let v0 = [Vec3::new(0.5, 0.0, 0.0), Vec3::new(-0.4, -0.3, -0.2)];
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

#[test]
fn an_htr10_pour_matches_the_cpu_early_and_settled() {
    use outram_park_fork_liggghts::compute::ThreadCount;
    use outram_park_fork_liggghts::htr10_fill::{Htr10Fill, Htr10FillSettings, Htr10GpuFill};
    let settings = Htr10FillSettings {
        n_pebbles: 4_000,
        threads: ThreadCount::Fixed(4),
        seed: 7,
        ..Htr10FillSettings::default()
    };
    let Ok(mut g) = Htr10GpuFill::new(settings) else {
        return eprintln!("skipping: no GPU adapter");
    };
    let mut c = Htr10Fill::new(settings).expect("cpu");
    eprintln!("adapter: {}", g.adapter());
    let (t0, pc) = (std::time::Instant::now(), c.advance(2_000));
    let tc = t0.elapsed().as_secs_f64();
    let (t1, pg) = (std::time::Instant::now(), g.advance(2_000));
    let tg = t1.elapsed().as_secs_f64();
    let mut d: Vec<f64> = c
        .centres()
        .iter()
        .zip(g.centres())
        .map(|(a, b)| a.sub(b).norm())
        .collect();
    d.sort_by(f64::total_cmp);
    eprintln!(
        "2 000 steps: median |dx| {:.2e} m, max {:.2e} m; CPU {:.1} ms/step (4 threads), GPU {:.1} ms/step",
        d[d.len() / 2],
        d[d.len() - 1],
        1e3 * tc / 2000.0,
        1e3 * tg / 2000.0
    );
    assert!(d[d.len() / 2] < 1e-6, "{}", d[d.len() / 2]);
    assert_eq!(pc.steps, pg.steps);
    // Settle both (chunks of 2 000 steps, up to 60 000).
    let (mut pc, mut pg) = (pc, pg);
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
        "settled: CPU {} steps, φ {:.4}, surface {:.4} m; GPU {} steps, φ {:.4}, surface {:.4} m",
        pc.steps, pc.phi_whole_core, hc, pg.steps, pg.phi_whole_core, hg
    );
    assert!(pc.settled && pg.settled);
    assert!((pc.phi_whole_core - pg.phi_whole_core).abs() / pc.phi_whole_core < 0.01);
    assert!((hc - hg).abs() < 0.06);
    let e = g.errors();
    assert_eq!((e.cell_overflow, e.slot_overflow), (0, 0));
}
