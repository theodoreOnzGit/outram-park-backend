// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # HTR-10 slow pebble recirculation — parameter sweep driver
//!
//! Runs the slow-defuelling case of `tests/htr10_recirculation.rs` at one
//! choice of contact parameters and writes its per-batch history and its final
//! bed, so a sweep over friction settings can be assembled and compared.
//!
//! The *test* is the regression gate at the default setting; this is the
//! instrument for the open physics question, which needs several settings.
//!
//! ## Usage
//!
//! ```text
//! cargo run --release -p outram-park-fork-liggghts --example htr10_recirculation_sweep -- \
//!     --label mu040 --mu 0.4 --mu-r 0.1 --batches 40 --threads 12
//! ```
//!
//! Outputs, into `reference-data/liggghts/`:
//!
//! | file | contents |
//! |---|---|
//! | `htr10_recirc_<label>.csv` | per-batch `φ`, bed top, KE ratio |
//! | `htr10_recirc_<label>_bed.csv` | final pebble centres, `id,x,y,z,vx,vy,vz` |
//!
//! ## The two defects this driver fixes
//!
//! Both were found by the first smoke run of the case and recorded in GitHub
//! issue #216 as blockers.
//!
//! **1. Re-insertion launched pebbles.** Inserting at `top + 0.02` with 0.06 m
//! pebbles starts a new pebble **overlapped by 0.04 m** — two-thirds of a
//! diameter. Hertz at `E = 5e8` gives `F ≈ 3.4e5 N` on a 0.196 kg pebble, i.e.
//! `Δv ≈ 61 m/s` in one 35 µs step. The bed top climbed 1.99 → 3.14 m and `φ`
//! collapsed 0.5764 → 0.3670.
//!
//! This driver **places** pebbles instead of dropping them, on the local
//! surface computed from the bed itself — see [`place_on_surface`]. The
//! placement is exactly tangent to the highest pebble beneath it, so the
//! initial overlap is zero by construction rather than by luck. Note the naive
//! alternative of simply raising the clearance does **not** work: a 0.08 m fall
//! takes 3657 steps to land, longer than the settling window, so batches would
//! overlap in time.
//!
//! **2. The pre-settle was too short.** After 12 000 steps the bed was still
//! mid-slump into the conus (`KE/E_drop = 8.99e-3`), so a baseline `φ` measured
//! there is meaningless and the first batches would be measuring the tail of
//! the initial slump rather than recirculation. This driver settles
//! **adaptively** — in chunks until the kinetic-energy ratio decays below a
//! threshold — so the baseline is a settled bed by measurement rather than by a
//! step count someone guessed.
//!
//! ## What is deliberately NOT asserted
//!
//! Whether slow recirculation densifies the bed is the **question**, so no
//! direction for `φ` is built in anywhere. Nor is any parameter tuned to reach
//! the published 0.61: the sweep varies friction over a *literature* range and
//! reports where each lands, which is an ablation, not a calibration.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::compute::{ComputeType, ThreadCount};
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial, RollingModel};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::mesh_wall::{MeshWall, MovingBoundary, WallGeometry};
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

const R_P: f64 = 0.03;
const R_CORE: f64 = 0.90;
const RHO: f64 = 1730.0;
const H_CONE: f64 = 0.36946;
const TUBE_LEN: f64 = 0.25;
const Z_VALVE: f64 = -(H_CONE + TUBE_LEN);
const YOUNGS_MODULUS: f64 = 5.0e8;
const DT: f64 = 3.5e-5;

/// Clearance above the tangent point when placing a pebble `[m]`.
///
/// Small enough that the pebble effectively starts in contact (a 0.1 mm fall
/// lands in ~130 steps) and large enough that no rounding can put it *inside*
/// its neighbour.
const PLACE_CLEARANCE: f64 = 1.0e-4;

fn data_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name)
}

fn load_positions(name: &str) -> Option<Vec<Vec3>> {
    let text = std::fs::read_to_string(data_path(name)).ok()?;
    let mut rows: Vec<(usize, Vec3)> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l.split(',').map(|v| v.parse().expect("numeric")).collect();
            (f[0] as usize, Vec3::new(f[1], f[2], f[3]))
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    Some(rows.into_iter().map(|r| r.1).collect())
}

fn pebble(x: Vec3) -> Particle {
    let m = RHO * 4.0 / 3.0 * std::f64::consts::PI * R_P.powi(3);
    Particle::new(
        x,
        Vec3::zero(),
        Vec3::zero(),
        Mass::new::<kilogram>(m),
        Length::new::<meter>(R_P),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    )
    .expect("valid pebble")
}

/// Bulk solid fraction over a slab excluding `4 r` at the bottom and the free
/// surface, by exact sphere-cap integration, over the cylindrical core only
/// (`z > 0`, i.e. above the conus inlet).
///
/// The same measure `tests/htr10_pebble_bed.rs` uses, so the numbers are
/// directly comparable to the settled-bed result.
fn bulk_solid_fraction(centres: &[Vec3]) -> (f64, f64) {
    let mut zs: Vec<f64> = centres.iter().map(|c| c.z).filter(|z| *z > 0.0).collect();
    if zs.len() < 100 {
        return (f64::NAN, f64::NAN);
    }
    zs.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let (z_lo, z_hi) = (zs[0] + 4.0 * R_P, zs[zs.len() - 1] - 4.0 * R_P);
    let cap = |zc: f64| {
        let lo = z_lo.max(zc - R_P);
        let hi = z_hi.min(zc + R_P);
        if hi <= lo {
            return 0.0;
        }
        let f = |z: f64| std::f64::consts::PI * (R_P * R_P * (z - zc) - (z - zc).powi(3) / 3.0);
        f(hi) - f(lo)
    };
    let solid: f64 = centres.iter().map(|c| cap(c.z)).sum();
    let slab = std::f64::consts::PI * R_CORE * R_CORE * (z_hi - z_lo);
    (solid / slab, zs[zs.len() - 1])
}

/// A **robust** bed surface height `[m]`: the 99th percentile of pebble centre
/// height over the cylindrical core, plus one radius.
///
/// ~~The maximum centre height~~ **CORRECTED** — `max z` is a single-pebble
/// statistic and is the wrong instrument here. Re-inserting a pebble onto the
/// local surface necessarily places it tangent to whatever is below, so if that
/// happens to be the currently-highest pebble the maximum jumps by a full
/// diameter — 60 mm — with no change in the bed at all. The first
/// recirculation run showed `top` moving +36 mm in one batch and it was read as
/// pebbles stacking into towers, which it was not.
///
/// The 99th percentile has 275 pebbles above it at HTR-10 scale, so no single
/// placement can move it, while it still tracks a genuine change in bed height.
fn bed_surface_height(centres: &[Vec3]) -> f64 {
    let mut zs: Vec<f64> = centres.iter().map(|c| c.z).filter(|z| *z > 0.0).collect();
    if zs.is_empty() {
        return f64::NAN;
    }
    zs.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let k = ((zs.len() as f64 * 0.99) as usize).min(zs.len() - 1);
    zs[k] + R_P
}

/// Whole-core closure `φ = N V_pebble / (π R² h)` — the **like-for-like**
/// comparison with the published 0.61, which is itself a whole-core figure
/// (27 000 pebbles in a 1.97 m core gives exactly 0.609).
///
/// `h` is the robust surface height above, not the maximum.
fn whole_core_fraction(centres: &[Vec3]) -> f64 {
    let top = bed_surface_height(centres);
    let n_above = centres.iter().filter(|c| c.z > 0.0).count();
    let v_pebble = 4.0 / 3.0 * std::f64::consts::PI * R_P.powi(3);
    n_above as f64 * v_pebble / (std::f64::consts::PI * R_CORE * R_CORE * top)
}

/// Van der Corput radical inverse in base 2 — the bit-reversal of `n` read as
/// a binary fraction.
///
/// Gives a well-spread value in `[0, 1)` for **any** run of consecutive
/// integers, which is the property needed below: a batch of 50 takes 50
/// consecutive indices and must still cover the whole disc.
fn radical_inverse_base2(n: usize) -> f64 {
    let mut x = n as u64;
    let (mut result, mut place) = (0.0_f64, 0.5_f64);
    while x > 0 {
        result += ((x & 1) as f64) * place;
        x >>= 1;
        place *= 0.5;
    }
    result
}

/// Re-insertion site on the core cross-section for the `s`-th pebble inserted
/// **over the whole run**, as a low-discrepancy (Hammersley-style) sequence:
/// radius from the base-2 radical inverse, angle from the golden angle.
///
/// Deliberately **not** a random draw — this crate has no RNG dependency and
/// does not need one here, and a deterministic sequence keeps the case
/// reproducible.
///
/// ## Why the index is global and not per-batch
///
/// ~~Sites were `(k + 0.5) / n` over the `n` pebbles of one batch~~
/// **CORRECTED** — that made every batch reuse the *same* 50 positions, so each
/// batch placed its pebbles directly on top of the previous batch's and the bed
/// grew 50 towers. Measured in the first run: the bed top climbed **+45 mm per
/// batch** where conservation of pebbles allows about 4 mm, and the toppling
/// towers drove `KE/E_drop` to 2.86e-2, breaching the 1e-2 quasi-static bound
/// the case depends on.
///
/// Taking `s` across the whole run fixes it, but only with a radius sequence
/// that is well spread over *consecutive* indices — a plain `(s % M) / M` ramp
/// would put all 50 pebbles of a batch on nearly one ring. Hence the radical
/// inverse.
fn insertion_site(s: usize) -> (f64, f64) {
    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    // +1 so the very first site is not the exact centre.
    let frac = radical_inverse_base2(s + 1);
    // sqrt maps a uniform fraction to uniform AREA density over the disc.
    let r = 0.85 * frac.sqrt();
    let theta = golden * s as f64;
    (r * theta.cos(), r * theta.sin())
}

/// Height at which a pebble dropped at `(x, y)` would come to rest on the
/// existing bed — the local surface.
///
/// For every existing pebble whose horizontal distance `d_h` is under one
/// diameter, the new pebble touches it at `z_j + sqrt(4r² − d_h²)`; the surface
/// is the highest such tangency. Placing there gives **exactly zero overlap**
/// by construction: the separation is `sqrt(d_h² + (4r² − d_h²)) = 2r`.
///
/// This is the fix for the re-insertion defect in GitHub issue #216 — see the
/// module docs.
fn place_on_surface(existing: &[Vec3], x: f64, y: f64, fallback_z: f64) -> Vec3 {
    let d_contact = 2.0 * R_P;
    let d2_contact = d_contact * d_contact;
    let mut z = fallback_z;
    for c in existing {
        let dh2 = (c.x - x).powi(2) + (c.y - y).powi(2);
        if dh2 < d2_contact {
            let zc = c.z + (d2_contact - dh2).sqrt();
            if zc > z {
                z = zc;
            }
        }
    }
    Vec3::new(x, y, z + PLACE_CLEARANCE)
}

fn arg(name: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn main() {
    let label = arg("--label", "default");
    let mu: f64 = arg("--mu", "0.4").parse().expect("mu");
    let mu_r: f64 = arg("--mu-r", "0.1").parse().expect("mu-r");
    let batches: usize = arg("--batches", "40").parse().expect("batches");
    let batch_size: usize = arg("--batch-size", "50").parse().expect("batch-size");
    let settle_steps: usize = arg("--settle-steps", "2000").parse().expect("settle-steps");
    let threads: usize = arg("--threads", "12").parse().expect("threads");
    let start_bed = arg("--start", "htr10_settled.csv");
    // Record phi every this many steps during settling, for the phi-vs-step
    // trace. The batch CSV samples once per batch, which is far too coarse to
    // show the slump.
    let sample_every: usize = arg("--sample-every", "500").parse().expect("sample-every");
    // Run ONLY the pre-settle, sampling densely, and stop. Used to capture the
    // phi-vs-step trajectory of the conus slump without repeating a 40-batch
    // recirculation run.
    let trace_only = std::env::args().any(|a| a == "--trace-only");

    let Some(init) = load_positions(&start_bed) else {
        eprintln!("missing reference-data/liggghts/{start_bed}");
        std::process::exit(1);
    };
    let stl = std::fs::read_to_string(data_path("htr10_discharge.stl")).expect("discharge stl");
    let mesh = MeshWall::from_ascii_stl(&stl).expect("valid discharge mesh");

    println!(
        "# HTR-10 slow recirculation | label={label} mu={mu} mu_r={mu_r} \
         batches={batches}x{batch_size} settle={settle_steps} threads={threads}"
    );
    println!(
        "# start bed {start_bed}: N = {}, discharge mesh {} facets",
        init.len(),
        mesh.triangles.len()
    );

    let material =
        GranularMaterial::new(YOUNGS_MODULUS, 0.2, 0.5, mu).expect("valid graphite material");
    let mut model = GranularContactModel::hertz_history(material);
    if mu_r > 0.0 {
        model = model.with_rolling(RollingModel::cdt(mu_r).expect("valid mu_r"));
    }
    let boundaries = vec![
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), R_CORE).expect("core barrel"),
        // The defuelling valve at the bottom of the tube — NOT a floor under
        // the core; the conus is what the bed rests on.
        Boundary::wall(Vec3::new(0.0, 0.0, Z_VALVE), Vec3::new(0.0, 0.0, 1.0)).expect("valve"),
    ];

    let particles: Vec<Particle> = init.iter().map(|x| pebble(*x)).collect();
    let n0 = particles.len();
    let mut sys = GranularSystem::new(
        particles,
        boundaries,
        model,
        Vec3::new(0.0, 0.0, -9.81),
        DT,
    )
    .expect("valid system")
    .with_compute(ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)))
    .with_moving_walls(vec![MovingBoundary::new(
        // Static in practice: the discharge geometry does not move. It is a
        // MovingBoundary only because that is the path a mesh wall takes.
        WallGeometry::Mesh(mesh),
        Vec3::zero(),
        Vec3::zero(),
        Vec3::zero(),
    )]);

    let mut skip_presettle = false;
    // phi vs STEP, sampled densely through both phases. Kept separate from the
    // per-batch log because it is a different series at a different resolution:
    // the batch log answers "what did each batch do", this answers "how did the
    // bed evolve", and the slump happens entirely between batch 0 and step 0.
    let mut trace = String::from(
        "phase,step,phi_bulk,phi_whole_core,surface_height,\
         core_ke_per_pebble_over_edrop,all_ke_per_pebble_over_edrop\n",
    );
    let mut total_steps = 0usize;
    let m_pebble = RHO * 4.0 / 3.0 * std::f64::consts::PI * R_P.powi(3);
    // Gravitational energy of a one-radius drop, per pebble — the yardstick for
    // "is this quasi-static?".
    let e_drop = m_pebble * 9.81 * R_P;
    // Mean kinetic energy per pebble IN THE CORE (z > 0), as a fraction of a
    // one-pebble-radius drop.
    //
    // Deliberately excludes the conus and discharge tube. Pebbles there are
    // being extracted — they are *meant* to be moving, and including them
    // measures the discharge stream rather than the bed whose packing is in
    // question, with the stream dominating because it is the only part moving
    // by design. The whole-system value is reported alongside so the choice is
    // visible rather than silent.
    let ke_ratio = |s: &GranularSystem| {
        let (mut ke, mut n) = (0.0, 0usize);
        for p in s.particles() {
            if p.position.z > 0.0 {
                ke += 0.5 * p.mass * p.velocity.norm_squared();
                n += 1;
            }
        }
        if n == 0 {
            return f64::NAN;
        }
        ke / n as f64 / e_drop
    };
    let ke_ratio_all =
        |s: &GranularSystem| s.kinetic_energy() / s.particles().len() as f64 / e_drop;

    // --- adaptive pre-settle (issue #216 defect 2) ---
    //
    // Cached per label: the slump into the conus depends on mu and mu_r, so the
    // cache cannot be shared between settings, but it makes a re-run of one
    // setting start where it left off instead of repeating ~8 minutes of
    // settling.
    let started = std::time::Instant::now();
    let presettle_cache = format!("htr10_conus_presettled_{label}.csv");
    if let Some(cached) = load_positions(&presettle_cache) {
        if cached.len() == n0 {
            println!("# reusing cached pre-settled bed {presettle_cache}");
            let fresh: Vec<Particle> = cached.iter().map(|x| pebble(*x)).collect();
            let all: Vec<usize> = (0..sys.particles().len()).collect();
            sys.remove_particles(&all);
            sys.insert_particles(fresh);
            skip_presettle = true;
        }
    }
    const PRESETTLE_CHUNK: usize = 2_000;
    const PRESETTLE_MAX: usize = 120_000;
    const PRESETTLE_TARGET: f64 = 1.0e-3;
    let mut presettle_steps = 0usize;
    while !skip_presettle {
        // Step in `sample_every` slices so the slump is resolved, not just its
        // endpoint. PRESETTLE_CHUNK is the decision interval; sample_every is
        // the recording interval.
        let mut done = 0usize;
        while done < PRESETTLE_CHUNK {
            let slice = sample_every.min(PRESETTLE_CHUNK - done);
            sys.run(slice);
            done += slice;
            total_steps += slice;
            let centres: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
            let (phi, _) = bulk_solid_fraction(&centres);
            trace.push_str(&format!(
                "presettle,{total_steps},{phi:.6},{:.6},{:.6},{:.6e},{:.6e}\n",
                whole_core_fraction(&centres),
                bed_surface_height(&centres),
                ke_ratio(&sys),
                ke_ratio_all(&sys)
            ));
        }
        presettle_steps += PRESETTLE_CHUNK;
        // Flush the trace every decision chunk, for the same reason the batch
        // log is flushed every batch: a pre-settle is ~10 minutes and losing it
        // to a crash or a Ctrl-C would be gratuitous.
        let _ = std::fs::write(
            data_path(&format!("htr10_phi_vs_step_{label}.csv")),
            &trace,
        );
        let ratio = ke_ratio(&sys);
        let centres: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
        let (phi, top) = bulk_solid_fraction(&centres);
        println!(
            "# presettle {presettle_steps:6}  phi {phi:.4}  top {top:.4}  KE/E_drop {ratio:.2e}  \
             [{:.0} s]",
            started.elapsed().as_secs_f64()
        );
        if ratio < PRESETTLE_TARGET || presettle_steps >= PRESETTLE_MAX {
            if ratio >= PRESETTLE_TARGET {
                println!(
                    "# WARNING: pre-settle hit the {PRESETTLE_MAX}-step cap at KE/E_drop \
                     {ratio:.2e}, still above the {PRESETTLE_TARGET:.0e} target — the bed was \
                     NOT settled when recirculation began."
                );
            }
            let mut cache = String::from("id,x,y,z,vx,vy,vz\n");
            for (i, p) in sys.particles().iter().enumerate() {
                cache.push_str(&format!(
                    "{},{:.17e},{:.17e},{:.17e},0,0,0\n",
                    i + 1,
                    p.position.x,
                    p.position.y,
                    p.position.z
                ));
            }
            let _ = std::fs::write(data_path(&presettle_cache), &cache);
            break;
        }
    }

    let trace_path = format!("htr10_phi_vs_step_{label}.csv");
    if trace.lines().count() > 1 {
        std::fs::write(data_path(&trace_path), &trace).expect("write phi-vs-step trace");
        println!("# wrote {trace_path} ({} samples)", trace.lines().count() - 1);
    }
    if trace_only {
        println!(
            "# --trace-only: pre-settle complete at step {total_steps}; stopping before \
             recirculation"
        );
        return;
    }

    let mut log = String::from(
        // `step` counts DEM steps from the start of this run. When the
        // pre-settle was served from cache it therefore starts at 0 at the
        // first batch; the absolute pre-settle trajectory lives in the
        // companion `htr10_phi_vs_step_<label>.csv`, which always runs from the
        // flat-floor bed.
        "phase,batch,step,extracted,n,phi_bulk,phi_whole_core,surface_height,\
         core_ke_per_pebble_over_edrop,all_ke_per_pebble_over_edrop,elapsed_s\n",
    );
    let mut worst_ratio: f64 = 0.0;
    let mut record = |sys: &GranularSystem, batch: usize, extracted: usize, log: &mut String| {
        let centres: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
        let (phi, _slab_top) = bulk_solid_fraction(&centres);
        let top = bed_surface_height(&centres);
        let phi_wc = whole_core_fraction(&centres);
        let ratio = ke_ratio(sys);
        let ratio_all = ke_ratio_all(sys);
        worst_ratio = worst_ratio.max(ratio);
        let el = started.elapsed().as_secs_f64();
        log.push_str(&format!(
            "recirculation,{batch},{step},{extracted},{},{phi:.6},{phi_wc:.6},{top:.6},\
             {ratio:.6e},{ratio_all:.6e},{el:.1}\n",
            sys.particles().len(),
            step = presettle_steps + batch * settle_steps,
        ));
        println!(
            "batch {batch:3}  extracted {extracted:5}  N {:5}  phi {phi:.4}  phi_wc {phi_wc:.4}  \
             top {top:.4} m  KE/E_drop core {ratio:.2e} (all {ratio_all:.2e})  [{el:.0} s]",
            sys.particles().len()
        );
        phi
    };
    let phi_0 = record(&sys, 0, 0, &mut log);

    let mut extracted = 0usize;
    // Monotonic across the whole run — see `insertion_site`.
    let mut site_counter = 0usize;
    for batch in 1..=batches {
        // --- extract: the lowest pebbles resting on the valve ---
        let mut by_z: Vec<(usize, f64)> = sys
            .particles()
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.position.z))
            .filter(|(_, z)| *z < -H_CONE)
            .collect();
        by_z.sort_by(|a, b| a.1.partial_cmp(&b.1).expect("finite z"));
        let doomed: Vec<usize> = by_z.iter().take(batch_size).map(|(i, _)| *i).collect();
        let removed = sys.remove_particles(&doomed).len();
        extracted += removed;
        if removed == 0 {
            println!("# no pebbles reached the valve at batch {batch} — stopping");
            break;
        }

        // --- re-insert the same number, PLACED on the surface, not dropped ---
        let mut existing: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
        let top = existing.iter().map(|c| c.z).fold(f64::NEG_INFINITY, f64::max);
        let mut fresh = Vec::with_capacity(removed);
        for _ in 0..removed {
            let (x, y) = insertion_site(site_counter);
            site_counter += 1;
            // Placed against the bed as it stands INCLUDING the pebbles already
            // placed in this batch, so a batch cannot self-overlap either.
            let p = place_on_surface(&existing, x, y, top);
            existing.push(p);
            fresh.push(pebble(p));
        }
        sys.insert_particles(fresh);
        assert_eq!(
            sys.particles().len(),
            n0,
            "recirculation must conserve pebble count"
        );

        sys.run(settle_steps);
        record(&sys, batch, extracted, &mut log);
        // Flush after EVERY batch, not once at the end. A 40-batch run is over
        // an hour; writing only on completion means a crash, an OOM or a
        // Ctrl-C throws away every measurement taken so far. The file is small
        // (one line per batch) so rewriting it each time costs nothing.
        let _ = std::fs::write(data_path(&format!("htr10_recirc_{label}.csv")), &log);
    }

    let centres: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
    let (phi_end, top_end) = bulk_solid_fraction(&centres);
    println!(
        "RECIRCULATION DONE [{label}]: phi {phi_0:.4} -> {phi_end:.4} ({:+.4}) over {extracted} \
         pebbles ({:.1} % of the bed);  phi_whole_core {:.4};  bed top {top_end:.4} m;  \
         worst KE/E_drop {worst_ratio:.2e};  {:.0} s",
        phi_end - phi_0,
        100.0 * extracted as f64 / n0 as f64,
        whole_core_fraction(&centres),
        started.elapsed().as_secs_f64()
    );

    std::fs::write(data_path(&format!("htr10_recirc_{label}.csv")), &log).expect("write log");
    let mut bed = String::from("id,x,y,z,vx,vy,vz\n");
    for (i, p) in sys.particles().iter().enumerate() {
        bed.push_str(&format!(
            "{},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e}\n",
            i + 1,
            p.position.x,
            p.position.y,
            p.position.z,
            p.velocity.x,
            p.velocity.y,
            p.velocity.z
        ));
    }
    std::fs::write(data_path(&format!("htr10_recirc_{label}_bed.csv")), &bed).expect("write bed");
    println!("# wrote htr10_recirc_{label}.csv and htr10_recirc_{label}_bed.csv");
}
