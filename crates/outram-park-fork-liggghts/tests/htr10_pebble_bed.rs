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

//! # HTR-10 pebble bed — full-scale DEM, at the published core geometry
//!
//! The crate's other bulk case settles 354 pebbles in a `D/d = 6` laboratory
//! cylinder. This one runs the **real reactor geometry**: 27 000 pebbles in the
//! HTR-10 core, `D/d = 30`. It is the verification case that matters for the
//! pebble-bed work, because it is the geometry every downstream consumer
//! actually uses.
//!
//! ## Geometry: the published HTR-10 design point
//!
//! | Quantity | Value | Source |
//! |---|---|---|
//! | core diameter | 180 cm (`R = 0.90 m`) | IAEA-TECDOC-1382 Table 4-1 |
//! | pebble diameter | 6.0 cm (`r = 0.03 m`) | " |
//! | graphite density | 1.73 g/cm³ (1730 kg/m³) | " |
//! | fuel elements | 27 000 | " |
//! | filling fraction `f` | **0.61** (porosity 0.39) | " |
//! | mean bed height | 197 cm | " |
//!
//! The workspace's single transcription of that design point is
//! `outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint::iaea_benchmark()`,
//! and it is what `htgr_sim_v1` derives its geometry from. **These constants
//! are a second copy, deliberately.** Taking a dev-dependency on that crate
//! would pull `egui`/`eframe` into this crate's test build, which breaks the
//! workspace Android rule (tests and examples are compiled by a native Termux
//! build and are explicitly not exempt). The values are therefore restated here
//! with their citation; if the design point ever changes, this file must be
//! updated with it, and `htr10_geometry_matches_the_published_design_point`
//! below states each number so a drift is at least visible in one place.
//!
//! ## Standard simplifications, and why each is defensible
//!
//! **Stiffness.** Nuclear graphite has `E ≈ 9 GPa`. This runs at `E = 5e8 Pa`,
//! a reduction of ~18x. That is the usual pebble-bed DEM softening: settling is
//! quasi-static, so packing structure is set by geometry and friction rather
//! than by stiffness, while the stable timestep scales as `sqrt(m/k)` — at the
//! true modulus the Rayleigh criterion would demand `dt ≈ 1.4e-5 s` and the run
//! would cost 10x more for a packing fraction that does not move.
//!
//! The price is larger contact overlaps, and the assumption is only safe while
//! they stay small against the pebble radius. **The test measures the maximum
//! overlap and asserts it against 2 % of the radius**, rather than assuming.
//!
//! That guard has already earned its place — twice. The modulus was set by
//! measurement, not by the hand estimate that opened the work:
//!
//! | `E` | measured max overlap | verdict |
//! |---|---|---|
//! | `1e8 Pa` (90x soft) | **3.80 %** of `r` | fails; hand estimate said 1.07 % |
//! | `3e8 Pa` (30x soft) | **2.13 %** of `r` | fails, marginally |
//! | `5e8 Pa` (18x soft) | see results | — |
//!
//! The first estimate was out by 3.5x because it used a single pebble's weight
//! where the real load is the ~2 m column above it. Note the **cross-code
//! comparison was unaffected throughout** — LIGGGHTS runs the same soft
//! material and agreed to 0.02 % and then to 4 decimals — so what the too-soft
//! modulus threatened was never the verification, but whether the bed is a fair
//! stand-in for a real one.
//!
//! **Contacts.** Hertz–Mindlin normal, tangential **history** spring, and CDT
//! rolling friction — the standard pebble-bed set, and the one verified
//! bit-identical against upstream LIGGGHTS in `liggghts_cross_code.rs`. Both
//! extras are load-bearing: without the history spring a bed cannot carry
//! static shear, and without rolling friction monodisperse spheres over-pack.
//!
//! | Parameter | Value | Note |
//! |---|---|---|
//! | `E` | `5e8 Pa` | reduced from ~9 GPa (above) |
//! | `ν` | 0.2 | nuclear graphite |
//! | `e` | 0.5 | dissipative; speeds settling, packing weakly sensitive |
//! | `µ` | 0.4 | graphite-on-graphite sliding |
//! | `µ_r` | 0.1 | CDT rolling |
//! | `dt` | `3.5e-5 s` | 11.7 % of the Rayleigh time at this `E` |
//!
//! ## Methodology
//!
//! Upstream LIGGGHTS inserts the 27 000 pebbles and settles them
//! (`reference-data/liggghts/in.htr10`). This crate starts from **LIGGGHTS'
//! post-insertion state**, so both codes integrate the identical configuration,
//! and settles independently. Solid fraction is measured over a bulk slab
//! excluding 4 pebble radii at the floor and at the free surface, by exact
//! sphere-cap integration.
//!
//! Three numbers are compared, and they are three different claims:
//!
//! 1. **against LIGGGHTS** — cross-code verification of this port;
//! 2. **against the published `f = 0.61`** — how the DEM bed compares to the
//!    benchmark's core-average filling fraction. This is *not* a validation of
//!    the contact model: `f = 0.61` is a whole-core figure that includes the
//!    near-wall region and the bed's top surface, while the slab measurement
//!    is deliberately bulk-only, so the two are not the same quantity and a
//!    difference of a few per cent is expected before any physics is at fault;
//! 3. **the maximum overlap** — whether the softened stiffness held.
//!
//! ## Results
//!
//! Filled in from the measured run; see `docs/verification-and-validation.md`.
//!
//! ## Runtime
//!
//! Measured, not inherited — see the constant below and the workspace
//! "Any test over 5 minutes" rule.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{
    ContactKinematics, GranularContactModel, GranularMaterial, RollingModel,
};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

// --- Published HTR-10 design point (IAEA-TECDOC-1382); see the module docs for
// --- why these are restated here rather than read from the digital-twin crate.
/// Pebble radius `[m]` — 6.0 cm diameter.
const R_P: f64 = 0.03;
/// Core radius `[m]` — 180 cm diameter, so `D/d = 30`.
const R_CORE: f64 = 0.90;
/// Graphite density `[kg/m³]` — 1.73 g/cm³.
const RHO: f64 = 1730.0;
/// Published volumetric filling fraction of balls in the core `[-]`.
const PUBLISHED_FILLING_FRACTION: f64 = 0.61;
/// Published fuel-element count.
const PUBLISHED_PEBBLE_COUNT: usize = 27_000;
/// Published mean bed height `[m]` — 197 cm.
const PUBLISHED_BED_HEIGHT: f64 = 1.97;

/// Reduced Young's modulus `[Pa]` — see "Standard simplifications".
const YOUNGS_MODULUS: f64 = 5.0e8;
/// Integration step `[s]`.
const DT: f64 = 3.5e-5;
/// Settling steps after the LIGGGHTS post-insertion state.
const SETTLE_STEPS: usize = 50_000;

fn data_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name)
}

fn load_state(name: &str) -> Option<Vec<(Vec3, Vec3)>> {
    let text = std::fs::read_to_string(data_path(name)).ok()?;
    let mut rows: Vec<(usize, Vec3, Vec3)> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l
                .split(',')
                .map(|v| v.parse().expect("numeric csv"))
                .collect();
            (
                f[0] as usize,
                Vec3::new(f[1], f[2], f[3]),
                Vec3::new(f[4], f[5], f[6]),
            )
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    Some(rows.into_iter().map(|r| (r.1, r.2)).collect())
}

/// Bulk solid fraction `[-]` and bed top `[m]`, over a slab excluding `4 r` at
/// the floor and at the free surface, by exact sphere-cap integration.
fn bulk_solid_fraction(centres: &[Vec3]) -> (f64, f64) {
    let mut zs: Vec<f64> = centres.iter().map(|c| c.z).collect();
    zs.sort_by(|a, b| a.partial_cmp(b).expect("finite z"));
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

/// Largest pair overlap `[m]` in the ensemble — the check on the softened
/// stiffness.
fn max_overlap(ps: &[Particle], pairs: &[(usize, usize)]) -> f64 {
    let mut worst = 0.0_f64;
    for &(i, j) in pairs {
        if let Some(k) = ContactKinematics::pair(&ps[i], &ps[j]) {
            worst = worst.max(k.delta_n);
        }
    }
    worst
}


/// Write this port's own settled state to `reference-data/liggghts/` in the
/// same CSV layout as the LIGGGHTS dumps (`id,x,y,z,vx,vy,vz`, `%.17g`), so a
/// reader can diff the two codes' beds without paying for a 40-minute run.
///
/// Best-effort: a failure to write warns and does not fail the test, because
/// the comparison this test asserts does not depend on the file existing.
fn write_our_state(name: &str, ps: &[Particle]) {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(ps.len() * 96);
    out.push_str("id,x,y,z,vx,vy,vz\n");
    for (i, p) in ps.iter().enumerate() {
        let (x, v) = (p.position, p.velocity);
        let _ = writeln!(
            out,
            "{},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e}",
            i + 1,
            x.x,
            x.y,
            x.z,
            v.x,
            v.y,
            v.z
        );
    }
    match std::fs::write(data_path(name), out) {
        Ok(()) => eprintln!("wrote our settled state to reference-data/liggghts/{name}"),
        Err(e) => eprintln!("warning: could not write {name}: {e}"),
    }
}
/// The published design point, restated so a drift against
/// `Htr10DesignPoint::iaea_benchmark()` is visible in one place.
///
/// **Methodology.** Check the geometry constants this file uses are internally
/// consistent with the published set: 27 000 pebbles of 6 cm diameter at
/// `f = 0.61` must fill a 180 cm cylinder to the published 197 cm mean height.
///
/// **Result (2026-09-17).** Total pebble volume `3.0536 m³`; at `f = 0.61` the
/// bed occupies `5.0059 m³`, which over the `2.5447 m²` core cross-section is
/// `1.9671 m` — against the published `1.97 m`, a closure of `-0.15 %`. The
/// constants are mutually consistent, so a later edit that breaks one of them
/// fails here rather than silently changing the DEM case.
#[test]
fn htr10_geometry_matches_the_published_design_point() {
    let pebble_volume = 4.0 / 3.0 * std::f64::consts::PI * R_P.powi(3);
    let total = pebble_volume * PUBLISHED_PEBBLE_COUNT as f64;
    let bed_volume = total / PUBLISHED_FILLING_FRACTION;
    let area = std::f64::consts::PI * R_CORE * R_CORE;
    let height = bed_volume / area;
    let closure = (height - PUBLISHED_BED_HEIGHT) / PUBLISHED_BED_HEIGHT;
    eprintln!(
        "pebble volume {total:.4} m^3; bed {bed_volume:.4} m^3; implied height \
         {height:.4} m vs published {PUBLISHED_BED_HEIGHT} m ({:+.2} %)",
        closure * 100.0
    );
    assert!(
        closure.abs() < 0.01,
        "HTR-10 constants are not mutually consistent: implied bed height \
         {height:.4} m vs published {PUBLISHED_BED_HEIGHT} m"
    );
    assert!(
        (R_CORE * 2.0 / (R_P * 2.0) - 30.0).abs() < 1e-12,
        "D/d must be 30"
    );
}

/// Settle the full 27 000-pebble HTR-10 core and compare against LIGGGHTS and
/// against the published filling fraction.
///
/// See the module docs for geometry, simplifications, methodology and results.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "full-scale HTR-10 DEM: 27 000 pebbles x 50 000 steps; gated behind \
              `long-tests` (default-on) per the workspace 5-minute rule"
)]
fn htr10_full_core_settles_and_matches_liggghts() {
    let (Some(init), Some(settled)) = (
        load_state("htr10_init.csv"),
        load_state("htr10_settled.csv"),
    ) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    eprintln!("N = {} (published {PUBLISHED_PEBBLE_COUNT})", init.len());

    let mass = RHO * 4.0 / 3.0 * std::f64::consts::PI * R_P.powi(3);
    let particles: Vec<Particle> = init
        .iter()
        .map(|(x, v)| {
            Particle::new(
                *x,
                *v,
                Vec3::zero(),
                Mass::new::<kilogram>(mass),
                Length::new::<meter>(R_P),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .expect("valid pebble")
        })
        .collect();

    let material =
        GranularMaterial::new(YOUNGS_MODULUS, 0.2, 0.5, 0.4).expect("valid graphite material");
    let model = GranularContactModel::hertz_history(material)
        .with_rolling(RollingModel::cdt(0.1).expect("valid mu_r"));
    let boundaries = vec![
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), R_CORE)
            .expect("valid core barrel"),
        Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("valid floor"),
    ];

    let started = std::time::Instant::now();
    let mut sys = GranularSystem::new(particles, boundaries, model, Vec3::new(0.0, 0.0, -9.81), DT)
        .expect("valid system");
    sys.run(SETTLE_STEPS);
    let elapsed = started.elapsed().as_secs_f64();

    write_our_state("htr10_settled_ours.csv", sys.particles());
    let ours: Vec<Vec3> = sys.particles().iter().map(|p| p.position).collect();
    let theirs: Vec<Vec3> = settled.iter().map(|(x, _)| *x).collect();
    let (phi_ours, z_ours) = bulk_solid_fraction(&ours);
    let (phi_theirs, z_theirs) = bulk_solid_fraction(&theirs);
    let overlap = max_overlap(sys.particles(), &sys.contact_pairs());

    eprintln!(
        "HTR-10 settled: phi ours {phi_ours:.4} vs LIGGGHTS {phi_theirs:.4} vs published \
         {PUBLISHED_FILLING_FRACTION};  bed top {z_ours:.4} m vs {z_theirs:.4} m (published \
         {PUBLISHED_BED_HEIGHT});  max overlap {overlap:.3e} m = {:.2} % of r;  KE {:.3e} J;  \
         {elapsed:.0} s",
        100.0 * overlap / R_P,
        sys.kinetic_energy()
    );

    // 1. The softened stiffness must not have produced unphysical overlaps.
    assert!(
        overlap < 0.02 * R_P,
        "max overlap {overlap:.3e} m is {:.2} % of the pebble radius — the \
         reduced Young's modulus is too soft for this bed load",
        100.0 * overlap / R_P
    );

    // 2. The bed must be a bed, not a collapse or an explosion.
    assert!(
        (0.55..0.68).contains(&phi_ours),
        "settled solid fraction {phi_ours:.4} is outside the physical range for \
         a random sphere packing"
    );

    // 3. Cross-code: this is the verification claim.
    let rel = (phi_ours - phi_theirs).abs() / phi_theirs;
    assert!(
        rel < 0.02,
        "bulk packing fraction differs from LIGGGHTS by {:.2} % (ours \
         {phi_ours:.4}, theirs {phi_theirs:.4})",
        rel * 100.0
    );

    // 4. Bed height within a pebble diameter of LIGGGHTS'.
    assert!(
        (z_ours - z_theirs).abs() < 2.0 * R_P,
        "bed top {z_ours:.4} m differs from LIGGGHTS {z_theirs:.4} m by more \
         than one pebble diameter"
    );
}
