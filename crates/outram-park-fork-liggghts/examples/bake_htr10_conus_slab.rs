// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust example: reads a pebble-position file and writes a Rust
// table. It reads and reuses NO GPL-2.0 LIGGGHTS/LAMMPS source.

//! # Bake a cut-away slab of the settled HTR-10 conus bed into widget artwork
//!
//! Reads the **settled HTR-10 conus bed** (27 554 real-size pebbles, published
//! geometry) and writes a Rust module with a `const` table of the pebbles in a
//! thin slab behind a vertical cut through the axis, sorted farthest first. The
//! digital twin's simplified HTR-10 vessel (`Htr10ReactorSchematic`) draws its
//! bed, conus and the top of its discharge tube from that table.
//!
//! ```text
//! cargo run --release -p outram-park-fork-liggghts --example bake_htr10_conus_slab \
//!     > crates/outram-park-digital-twin-engine/src/components/htr10_conus_packing.rs
//! ```
//!
//! The module goes to **stdout**, diagnostics to **stderr**. Optional
//! `--input <csv>` and `--depth <m>` override the defaults below.
//!
//! ## Why reuse this bed rather than bake a new one
//!
//! It is exactly the geometry the schematic draws (core radius 0.90 m, conus
//! 0.36946 m tall, discharge tube radius 0.25 m, 6 cm pebbles), and it is the
//! bed on which this port and LIGGGHTS agree (see
//! `docs/htr10-bed-geometry-and-positions.md` and
//! `docs/verification-and-validation.md`). Baking a second packing of the same
//! vessel would add nothing and would not have that cross-check.
//!
//! **Its discharge tube is only 0.25 m long**, deliberately shortened from the
//! real tube in that run. So the table covers the bed, the conus and the top
//! 0.25 m of the tube; the rest of the drawn tube is not DEM.
//!
//! ## The cut
//!
//! Coordinates are SI with the axis along `z`, origin at the conus inlet
//! (`z = 0` at zero core height), as in the geometry doc. The cut is the plane
//! `y = 0`. The viewer is on the `+y` side, so everything with `y > 0` has been
//! sawn away, and a pebble is kept when its centre lies in
//! `-DEPTH <= y <= 0`: one pebble diameter by default, the nearest layer of
//! whole spheres behind the cut face. Each kept pebble is written as
//! `[x, z, y]` in metres, sorted by `y` ascending (farthest first), so a
//! renderer can paint straight through the table.
//!
//! ## Honest scope
//!
//! Artwork for an offline demonstration, not a validated packing and not for
//! any facility, licensing or safety purpose. The DEM itself is documented
//! with its methodology and results in this crate's V&V write-up.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::compute::{ComputeType, ThreadCount};
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial, RollingModel};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use std::io::Write;
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

/// Default input: the LIGGGHTS run's final slumped conus bed.
const DEFAULT_INPUT: &str = "reference-data/liggghts/htr10_conus_settled_liggghts.csv";

/// Default slab depth behind the cut, metres: one pebble diameter.
const DEFAULT_DEPTH_M: f64 = 0.06;

/// Pebble radius in that run, metres (`docs/htr10-bed-geometry-and-positions.md`).
const PEBBLE_RADIUS_M: f64 = 0.03;

/// Read an optional `--name <value>` flag.
fn flag(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let workspace = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let input = flag("--input").unwrap_or_else(|| format!("{workspace}/{DEFAULT_INPUT}"));
    let depth: f64 = flag("--depth")
        .map(|d| d.parse().expect("--depth needs a number in metres"))
        .unwrap_or(DEFAULT_DEPTH_M);

    let text = std::fs::read_to_string(&input)
        .unwrap_or_else(|e| panic!("cannot read {input}: {e} (is reference-data checked out?)"));

    // id,x,y,z,vx,vy,vz with a header row.
    let mut all = 0usize;
    let mut kept: Vec<[f64; 3]> = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<f64> = line
            .split(',')
            .map(|v| v.trim().parse().expect("numeric CSV field"))
            .collect();
        all += 1;
        let (x, y, z) = (f[1], f[2], f[3]);
        if (-depth..=0.0).contains(&y) {
            kept.push([x, z, y]);
        }
    }
    let conus_kept = kept.len();

    // The rest of the tube, from a settled DEM column, below the conus run's
    // valve only: its own loose top is cropped away at the join.
    let steps: usize = flag("--column-steps")
        .map(|s| s.parse().expect("--column-steps needs an integer"))
        .unwrap_or(40_000);
    let threads: usize = flag("--threads")
        .map(|s| s.parse().expect("--threads needs an integer"))
        .unwrap_or(8);
    let (column, column_ke) = settle_column(steps, threads);
    let column_all = column.len();
    for c in &column {
        if (-depth..=0.0).contains(&c.y) && c.z + PEBBLE_RADIUS_M <= CONUS_RUN_VALVE_Z {
            kept.push([c.x, c.z, c.y]);
        }
    }
    let column_kept = kept.len() - conus_kept;
    eprintln!("column: kept {column_kept} of {column_all} in the slab below the valve");

    kept.sort_by(|a, b| a[2].total_cmp(&b[2]));

    let top = kept.iter().map(|p| p[1]).fold(f64::MIN, f64::max) + PEBBLE_RADIUS_M;
    let bottom = kept.iter().map(|p| p[1]).fold(f64::MAX, f64::min) - PEBBLE_RADIUS_M;
    let max_r = kept
        .iter()
        .map(|p| p[0].abs() + PEBBLE_RADIUS_M)
        .fold(0.0, f64::max);
    eprintln!("read {all} pebbles from {input}");
    eprintln!("kept {} in the slab -{depth} m <= y <= 0", kept.len());
    eprintln!("slab spans z {bottom:.4} .. {top:.4} m, |x| + r <= {max_r:.4} m");

    let mut out = std::io::stdout().lock();
    let w = &mut out;
    writeln!(w, "// SPDX-License-Identifier: GPL-3.0-only").unwrap();
    writeln!(w, "// GENERATED by `cargo run --release -p outram-park-fork-liggghts --example bake_htr10_conus_slab`.").unwrap();
    writeln!(w, "// Do not edit by hand; regenerate instead.").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "//! A cut-away slab of the **settled HTR-10 conus bed**, as a `const` table."
    )
    .unwrap();
    writeln!(w, "//!").unwrap();
    writeln!(
        w,
        "//! Source: `reference-data/liggghts/htr10_conus_settled_liggghts.csv`, the"
    )
    .unwrap();
    writeln!(
        w,
        "//! LIGGGHTS run's final slumped state ({all} pebbles, published HTR-10"
    )
    .unwrap();
    writeln!(
        w,
        "//! geometry: core radius 0.90 m, conus 0.36946 m, discharge tube radius"
    )
    .unwrap();
    writeln!(w, "//! 0.25 m, 6 cm pebbles). Geometry and provenance:").unwrap();
    writeln!(
        w,
        "//! `crates/outram-park-fork-liggghts/docs/htr10-bed-geometry-and-positions.md`."
    )
    .unwrap();
    writeln!(w, "//!").unwrap();
    writeln!(
        w,
        "//! Kept: the {} pebbles whose centres lie in `-{depth} m <= y <= 0` behind the",
        kept.len()
    )
    .unwrap();
    writeln!(
        w,
        "//! cut plane `y = 0`, viewer on `+y`. Each entry is `[x, z, y]` in metres,"
    )
    .unwrap();
    writeln!(
        w,
        "//! `z` up from zero core height (the conus inlet), sorted by `y` ascending"
    )
    .unwrap();
    writeln!(
        w,
        "//! (**farthest first**): paint straight through the table."
    )
    .unwrap();
    writeln!(w, "//!").unwrap();
    writeln!(
        w,
        "//! That run's discharge tube is only 0.25 m long (valve at z = -0.61946 m)."
    )
    .unwrap();
    writeln!(
        w,
        "//! **Below the valve the tube is a separate DEM column**, settled by this"
    )
    .unwrap();
    writeln!(
        w,
        "//! generator on `GranularSystem` with the same deck's contact parameters"
    )
    .unwrap();
    writeln!(
        w,
        "//! (`in.htr10_conus`: E 5e8 Pa, nu 0.2, e 0.5, mu 0.4, CDT mu_r 0.1, dt 3.5e-5 s),"
    )
    .unwrap();
    writeln!(w, "//! {column_all} pebbles in the 0.25 m-radius tube on a floor at z = {COLUMN_FLOOR_Z} m (the").unwrap();
    writeln!(
        w,
        "//! drawn tube's cited ~3.3 m), {steps} steps, final KE {column_ke:.2e} J per pebble."
    )
    .unwrap();
    writeln!(
        w,
        "//! Only its pebbles wholly below the valve are kept ({column_kept} in the slab), so"
    )
    .unwrap();
    writeln!(
        w,
        "//! its loose top is cropped away at the join. Of the {} kept in all, {conus_kept}",
        kept.len()
    )
    .unwrap();
    writeln!(
        w,
        "//! come from the conus run and {column_kept} from the column."
    )
    .unwrap();
    writeln!(w, "//!").unwrap();
    writeln!(
        w,
        "//! Measured on this slab: `z` from {bottom:.4} to {top:.4} m (pebble extents),"
    )
    .unwrap();
    writeln!(w, "//! `|x| + r <= {max_r:.4} m`.").unwrap();
    writeln!(w, "//!").unwrap();
    writeln!(
        w,
        "//! Artwork for an offline demonstration, not a validated packing."
    )
    .unwrap();
    writeln!(w).unwrap();
    writeln!(w, "/// Pebble radius in the source run, metres.").unwrap();
    writeln!(
        w,
        "pub(crate) const PEBBLE_RADIUS_M: f32 = {PEBBLE_RADIUS_M:?};"
    )
    .unwrap();
    writeln!(w).unwrap();
    writeln!(w, "/// Depth of the kept slab behind the cut, metres.").unwrap();
    writeln!(w, "pub(crate) const SLAB_DEPTH_M: f32 = {depth:?};").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "/// Kept pebbles, `[x, z, y]` in metres, farthest first."
    )
    .unwrap();
    writeln!(w, "#[rustfmt::skip]").unwrap();
    writeln!(w, "pub(crate) const CONUS_SLAB: &[[f32; 3]] = &[").unwrap();
    for p in &kept {
        writeln!(w, "    [{:.5}, {:.5}, {:.5}],", p[0], p[1], p[2]).unwrap();
    }
    writeln!(w, "];").unwrap();
}

// ── The rest of the discharge tube: a DEM column ────────────────────────────
//
// The conus run's tube stops at its valve, 0.25 m below the conus. The drawn
// tube runs the cited ~3.3 m (the schematic's `DISCHARGE_TUBE_LENGTH_FRACTION`),
// so the rest is baked here: a column of the same pebbles, settled under
// gravity on a floor at the drawn tube's end, with the conus deck's own
// contact parameters (`reference-data/liggghts/in.htr10_conus`), on the
// LIGGGHTS-faithful `GranularSystem`.

/// Conus height, metres (published geometry).
const H_CONE: f64 = 0.36946;
/// The conus run's valve, metres: the bottom of the tube it covers.
const CONUS_RUN_VALVE_Z: f64 = -(H_CONE + 0.25);
/// Drawn discharge tube length below the conus, metres: the cited ~3.3 m.
const DRAWN_TUBE_LENGTH: f64 = 3.30;
/// Floor of the column, metres: the drawn tube's end.
const COLUMN_FLOOR_Z: f64 = -(H_CONE + DRAWN_TUBE_LENGTH);
/// Discharge tube radius, metres.
const TUBE_RADIUS: f64 = 0.25;
/// Pebble density, kg/m^3 (`in.htr10_conus`).
const RHO: f64 = 1730.0;
/// Contact parameters, all from `in.htr10_conus`.
const YOUNGS_MODULUS: f64 = 5.0e8;
const POISSON_RATIO: f64 = 0.2;
const RESTITUTION: f64 = 0.5;
const FRICTION: f64 = 0.4;
const ROLLING_FRICTION: f64 = 0.1;
/// Timestep, s (`in.htr10_conus`).
const DT: f64 = 3.5e-5;
/// Settled height the column must reach ABOVE the conus run's valve, metres,
/// so its loose top is cropped away rather than drawn mid-tube.
const COLUMN_OVERFILL: f64 = 0.30;
/// Solid fraction assumed when sizing the seed, for `D/d = 8.3` with its
/// strong wall effect. Only sets how many pebbles are seeded; the settled
/// height is measured and checked, not assumed.
const SEED_SOLID_FRACTION: f64 = 0.55;
/// Fixed seed for the lattice jitter, so the bake is reproducible.
const COLUMN_SEED: u64 = 0x7B0E_D15C_4A26_2026;

/// SplitMix64, for a reproducible jitter without an RNG dependency.
fn splitmix(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

fn pebble(x: Vec3) -> Particle {
    let m = RHO * 4.0 / 3.0 * std::f64::consts::PI * PEBBLE_RADIUS_M.powi(3);
    Particle::new(
        x,
        Vec3::zero(),
        Vec3::zero(),
        Mass::new::<kilogram>(m),
        Length::new::<meter>(PEBBLE_RADIUS_M),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid pebble")
}

/// Seed a loose, jittered lattice filling the tube from the floor up: square
/// layers at 1.1 diameters' pitch, each layer shifted by a random offset, each
/// pebble jittered by at most 0.04 diameters so no two seeds overlap.
fn seed_column(n_target: usize) -> Vec<Particle> {
    let d = 2.0 * PEBBLE_RADIUS_M;
    let pitch = 1.1 * d;
    let reach = TUBE_RADIUS - PEBBLE_RADIUS_M - 1.0e-3;
    let mut state = COLUMN_SEED;
    let mut out = Vec::with_capacity(n_target);
    let mut z = COLUMN_FLOOR_Z + PEBBLE_RADIUS_M + 1.0e-3;
    while out.len() < n_target {
        let (ox, oy) = (splitmix(&mut state) * pitch, splitmix(&mut state) * pitch);
        let span = (reach / pitch).ceil() as i64 + 1;
        for i in -span..=span {
            for j in -span..=span {
                let x =
                    i as f64 * pitch + ox - 0.5 * pitch + (splitmix(&mut state) - 0.5) * 0.08 * d;
                let y =
                    j as f64 * pitch + oy - 0.5 * pitch + (splitmix(&mut state) - 0.5) * 0.08 * d;
                if (x * x + y * y).sqrt() <= reach && out.len() < n_target {
                    out.push(pebble(Vec3::new(x, y, z)));
                }
            }
        }
        z += pitch;
    }
    out
}

/// Settle the column and return every pebble centre, printing progress to
/// stderr.
fn settle_column(steps: usize, threads: usize) -> (Vec<Vec3>, f64) {
    let v_pebble = 4.0 / 3.0 * std::f64::consts::PI * PEBBLE_RADIUS_M.powi(3);
    let height = (CONUS_RUN_VALVE_Z - COLUMN_FLOOR_Z) + COLUMN_OVERFILL;
    let n = (std::f64::consts::PI * TUBE_RADIUS * TUBE_RADIUS * height * SEED_SOLID_FRACTION
        / v_pebble)
        .ceil() as usize;
    let material = GranularMaterial::new(YOUNGS_MODULUS, POISSON_RATIO, RESTITUTION, FRICTION)
        .expect("valid graphite material");
    let model = GranularContactModel::hertz_history(material)
        .with_rolling(RollingModel::cdt(ROLLING_FRICTION).expect("valid mu_r"));
    let boundaries = vec![
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), TUBE_RADIUS)
            .expect("discharge tube"),
        Boundary::wall(
            Vec3::new(0.0, 0.0, COLUMN_FLOOR_Z),
            Vec3::new(0.0, 0.0, 1.0),
        )
        .expect("tube floor"),
    ];
    let mut sys = GranularSystem::new(
        seed_column(n),
        boundaries,
        model,
        Vec3::new(0.0, 0.0, -9.81),
        DT,
    )
    .expect("valid system")
    .with_compute(ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)));
    eprintln!("column: {n} pebbles seeded, settling {steps} steps on {threads} threads");
    let started = std::time::Instant::now();
    let chunk = 5_000;
    let mut done = 0;
    while done < steps {
        let s = chunk.min(steps - done);
        sys.run(s);
        done += s;
        eprintln!(
            "column: step {done:>6}  KE per pebble {:.3e} J  ({:.0} s)",
            sys.kinetic_energy() / n as f64,
            started.elapsed().as_secs_f64()
        );
    }
    let centres: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
    let top = centres.iter().map(|c| c.z).fold(f64::MIN, f64::max) + PEBBLE_RADIUS_M;
    eprintln!(
        "column: settled top at z = {top:.3} m; needs to exceed the conus run's valve at {CONUS_RUN_VALVE_Z:.3} m"
    );
    assert!(
        top > CONUS_RUN_VALVE_Z + PEBBLE_RADIUS_M,
        "the settled column does not reach the conus run's valve; raise COLUMN_OVERFILL"
    );
    (centres, sys.kinetic_energy() / n as f64)
}
