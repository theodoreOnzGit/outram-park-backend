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

//! # HTR-10 fuel-loading sweep on ENDF/B-VIII.0, with the bound thermal laws
//!
//! Twelve fuel-loading heights, one **function per case**, every setting
//! written in Rust. Run one case per process so each gets a core to itself:
//!
//! ```bash
//! cargo build --release -p nee_soon --example htr10_endf8_height_sweep
//! taskset -c 0 ./target/release/examples/htr10_endf8_height_sweep l12
//! ./target/release/examples/htr10_endf8_height_sweep --list
//! ```
//!
//! ## Why this exists alongside `htr10_rmc_keff`
//!
//! [`htr10_rmc_keff`](../htr10_rmc_keff/index.html) is the **ablation driver**:
//! it takes its configuration from `OUTRAM_HTR10_*` environment variables so an
//! arm can be turned on or off without a rebuild. That is the right shape for
//! hunting a residual and the wrong shape for a published curve, because the
//! settings that produced a number live in whatever shell history ran it. The
//! `OUTRAM_HTR10_FIXED_CAVITY` trap is the standing example — a loading sweep
//! is incoherent without it, nothing enforced it, and the failure is silent
//! (gh:#292).
//!
//! **Here every setting is a literal in the case function**, so the committed
//! source IS the specification of the run, and a reader can diff two cases to
//! see exactly what differs between them. Nothing is read from the
//! environment.
//!
//! ~~The nuclide set is built in this file rather than shared with
//! `htr10_rmc_keff::nuclides`, because that one branches on the ablation knobs
//! this example exists to be free of. The two must not drift: if a tape name
//! or a thermal law changes there, change it here.~~ **CHANGED 2026-10-01:**
//! both now call `nee_soon::htr10_rmc::data` (`Htr10NuclideLayout::plan` +
//! `load_htr10_nuclides`). This example passes
//! `Htr10DataConfig::default()` as a literal and reads nothing from the
//! environment, so the two cannot drift: with no knobs set, `htr10_rmc_keff`
//! builds the identical set.
//!
//! ~~**Blocked on gh:#339 as of 2026-10-01.** The default rod metal is the FULL
//! case (real Ni and Fe-57), and Fe-57 cannot yet be reconstructed, so every
//! case here prints `REFUSED` until #339 is fixed. (Before 2026-10-01 it
//! printed `SKIP`: the Ni tapes were not in the checkout.) Use
//! `htr10_rod_metal_simplified` for a runnable case meanwhile.~~
//! **CORRECTED 2026-10-02:** no longer blocked. `data::FE57_RECONSTRUCTION_FIXED`
//! was flipped to `true` on 2026-10-01 (the LRF=7 `xdot` operand fix), and the
//! FULL rod metal ran in all 22 runs of
//! `verification_and_validation/htr10_seker_2026_10_01_10k/` (each log prints
//! `rod metal: FULL`).
//!
//! **For a whole-sweep record use `htr10_endf8_kvsh_quick` / `_heavy`
//! (gh:#501)**: they run every height in one process (the nuclear data are
//! processed once), and write the figure script, the results table and the
//! parameters block. This example and those share the run machinery in
//! [`nee_soon::htr10_rmc::keff_vs_height`] (majorant, source box, entropy mesh,
//! reference interpolation; since 2026-10-02).
//!
//! ## What every case holds fixed
//!
//! | quantity | value | why |
//! |---|---|---|
//! | library | ENDF/B-VIII.0 | see the thermal laws below |
//! | graphite S(a,b) | ~~crystalline, MAT 30~~ **30 %-porosity reactor graphite, MAT 32** (`GraphiteLaw::default()`; CORRECTED 2026-10-01, gh:#428) | a graphite-moderated thermal system |
//! | SiC S(a,b) | C-in-SiC MAT 44, Si-in-SiC MAT 43 | SiC is a crystal; free gas is wrong |
//! | UO2 S(a,b) | U-in-UO2 MAT 48, O-in-UO2 MAT 75 | generated in-process from LEAPR decks |
//! | silicon | natural Si-28/29/30 | splits a correct total, does not change it |
//! | carbon | natural, C-12 / C-13 98.93 / 1.07 at.% (since 2026-10-01, gh:#425) | ENDF/B-VIII.0 ships the isotopes separately |
//! | coolant | natural helium, 300.15 K, 101.33 kPa (pressure assumed; since 2026-10-01, gh:#426) | was exact vacuum until 2026-10-01 |
//! | rod metal | FULL: real Ni-58..64, Fe-54..58 (since 2026-10-01, gh:#329) | blocked by gh:#339 |
//! | cavity | fixed core cavity | the only treatment there is — see below |
//! | rings | 14 | radial tiling of the bed |
//! | bed | Şeker & Çolak (2003) 13-ball cell, N = 9 … 20 layers | every ball whole (gh:#472) |
//! | particles | 10 000 per generation | |
//! | generations | 5 inactive + 135 active | |
//! | seed | 20260917, single draw | |
//! | threads | 1 | one core per case, cases run in parallel |
//!
//! ### The cavity is what makes a sweep a sweep
//!
//! The HTR-10 core cavity is fixed hardware ([`HTR10_CORE_CAVITY_CM`], 221.818
//! cm); it is the **void** above the bed that shrinks as fuel is added, and
//! [`cavity_above_bed`](nee_soon::htr10_rmc::core_model::cavity_above_bed)
//! computes it as `cavity - bed`. No case needs to ask for this and none can
//! opt out of it: holding the void constant instead was removed from the
//! library on 2026-09-24, because it is exact only at the benchmark loading
//! and drifts in a known direction along exactly the axis this example varies
//! (gh:#292).
//!
//! ## Data provenance
//!
//! Atom densities ~~from **IAEA-TECDOC-1382** Table 4-38~~ via
//! [`nee_soon::htr10_rmc::materials`]: **CORRECTED 2026-10-01 (gh:#428)** —
//! Table 4-38 is MIT's pebble-bed composition table, which the model does not
//! use. The pebble is Li, Yu & Wei (2014) Table 2, the reflector zones are
//! IAEA-TECDOC-1382 Table 4-3 with its p. 242 corrections, and the rods are
//! TECDOC § 4.1.1.5. Geometry from Terry et al. (2005), the same TECDOC and
//! Şeker & Çolak (2003). Evaluated data is ENDF/B-VIII.0 from `reference-data/endf/`.
//! The UO2 laws ship as no tape anywhere in this repository and are generated
//! from the LEAPR decks committed in `njoy-outram-park-fork`.
//!
//! ## Verification & validation
//!
//! **Methodology.** Each case computes `k_eff` for the HTR-10 first-criticality
//! core at one fuel-loading height and compares it against the RMC result of
//! Li, Yu & Wei (2014), ~~**interpolated to the height actually modelled**~~
//! **read at the height where Şeker's model holds as many balls as the built
//! bed** (`rmc_at_height` at `seker_height_for_balls`; CORRECTED 2026-10-01,
//! gh:#428, the comparison changed with gh:#472). Comparing against RMC's single 123.576 cm headline while
//! modelling a different bed imports ~270 pcm per cm of mismatch, which is
//! larger than several of the physics terms being argued about. The pass
//! criterion for the workspace gate is 500-1000 pcm.
//!
//! **Results.** Not yet recorded in this doc comment — the sweep that fills it
//! is the reason this file exists. Each case prints its own `k_eff`, its
//! within-run sigma, the height-matched residual and its full Shannon-entropy
//! trace; the trace is what says whether 5 inactive generations converged the
//! fission source at that loading, and **no residual here should be read as
//! physics until it has been looked at**. Record the table here once measured.
//!
//! **Single seed.** Seed-to-seed scatter on this problem is `sd ~ 179-211 pcm`
//! (~~`verification_and_validation/htr10_rmc/README.md`~~ **CORRECTED
//! 2026-10-02:** no such file exists; the figure is recorded in
//! `docs/software_engineering/htr10-run-log.md`). A single draw is not a
//! mean and the within-run sigma does not contain that scatter.

use std::time::Instant;

use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;
use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, HTR10_CORE_CAVITY_CM};
use nee_soon::htr10_rmc::data::{
    load_htr10_nuclides, Htr10DataConfig, Htr10DataError, Htr10NuclideLayout,
};
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use nee_soon::htr10_rmc::keff_vs_height::{bed_majorant, fissile_entropy_mesh, fissile_source_box};
use nee_soon::htr10_rmc::reflector::zone_composition;
use outram_mc_libs::pebble_beds::htr10::BoronReading;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::run_keff_csg_hybrid;
use outram_mc_libs::run_diagnostics::RunDiagnostics;

const TEMP_K: f64 = 300.15;

/// RMC's value at its 123.576 cm headline loading. Kept only so the printed
/// output can show how far the headline is from the height actually modelled.
const RMC_HEADLINE_KEFF: f64 = 1.004288;

/// Everything one case is. No field has a default and nothing is read from the
/// environment: this struct IS the run.
#[derive(Clone, Copy, Debug)]
struct CaseSpec {
    /// Case name, as typed on the command line.
    name: &'static str,
    /// Şeker layers N (since 2026-10-01, gh:#472). The bed height follows
    /// from it: `9.798 N + 6` cm.
    n_axial: usize,
    /// Radial ring count.
    n_rings: usize,
    /// Nominal loading height \[cm\], for the log only — the geometry decides
    /// the real one, and the run asserts the two agree.
    nominal_height_cm: f64,
    particles: usize,
    inactive: usize,
    active: usize,
    seed: u64,
    threads: usize,
    /// Reflector zone and its carbon scale (TECDOC-1382 Table 4-3).
    reflector_zone: usize,
    reflector_carbon_scale: f64,
    boron: BoronReading,
}

/// The settings every case in this sweep shares. Spelled once, but **copied
/// into each case by value** so a case function remains a complete statement
/// of its own run rather than a diff against a default.
macro_rules! case {
    ($name:literal, $n_axial:expr, $height:expr) => {
        CaseSpec {
            name: $name,
            n_axial: $n_axial,
            n_rings: 14,
            nominal_height_cm: $height,
            particles: 10_000,
            inactive: 5,
            active: 135,
            seed: 20260917,
            threads: 1,
            reflector_zone: 22,
            reflector_carbon_scale: 1.0,
            boron: BoronReading::Natural,
        }
    };
}

// ---------------------------------------------------------------------------
// One function per case. The twelve heights are RMC's own tabulated loadings,
// so each case has a reference point that needs no extrapolation.
// ---------------------------------------------------------------------------

// ~~n20 … n41: half-layer counts 2N+1, built volume-equivalent~~ **CHANGED
// 2026-10-01 (gh:#472):** one case per Şeker layer count N = 9..20. Şeker's
// bed is built on the paper's own height axis (9.798 N + 6 cm, every ball
// whole), so each case IS a tabulated row and needs no conversion or
// interpolation (closes the gh:#427 driver defects for this bed).
fn case_l09() -> CaseSpec {
    case!("l09", 9, 94.182)
}
fn case_l10() -> CaseSpec {
    case!("l10", 10, 103.980)
}
fn case_l11() -> CaseSpec {
    case!("l11", 11, 113.778)
}
fn case_l12() -> CaseSpec {
    case!("l12", 12, 123.576)
}
fn case_l13() -> CaseSpec {
    case!("l13", 13, 133.374)
}
fn case_l14() -> CaseSpec {
    case!("l14", 14, 143.172)
}
fn case_l15() -> CaseSpec {
    case!("l15", 15, 152.970)
}
fn case_l16() -> CaseSpec {
    case!("l16", 16, 162.768)
}
fn case_l17() -> CaseSpec {
    case!("l17", 17, 172.566)
}
fn case_l18() -> CaseSpec {
    case!("l18", 18, 182.364)
}
fn case_l19() -> CaseSpec {
    case!("l19", 19, 192.162)
}
fn case_l20() -> CaseSpec {
    case!("l20", 20, 201.960)
}

/// Every case, in loading order.
fn all_cases() -> Vec<fn() -> CaseSpec> {
    vec![
        case_l09, case_l10, case_l11, case_l12, case_l13, case_l14, case_l15, case_l16, case_l17,
        case_l18, case_l19, case_l20,
    ]
}

/// RMC's `k_eff` interpolated to ~~the height actually modelled~~ a given
/// height. Since 2026-10-01 (gh:#472) it is called at the equal-ball-count
/// height, not at the built height (CORRECTED 2026-10-01, gh:#428).
///
/// Returns `None` outside the tabulated range rather than extrapolating: past
/// the ends the curve flattens and a linear extension would invent reactivity.
///
/// Since 2026-10-02 (gh:#501) a call to the shared
/// [`keff_curve_at_height`](nee_soon::htr10_rmc::keff_curve_at_height).
fn rmc_at_height(h_cm: f64) -> Option<f64> {
    nee_soon::htr10_rmc::keff_curve_at_height(nee_soon::htr10_rmc::RMC_KEFF_VS_HEIGHT, h_cm)
}

/// The ENDF/B-VIII.0 data configuration: the correct-physics default, every
/// field stated so the source is the specification of the run. No ablation
/// path here on purpose -- `htr10_rmc_keff` is where ablations live.
fn data_config_endf8() -> Htr10DataConfig {
    Htr10DataConfig {
        temperature: ThermodynamicTemperature::new::<kelvin>(TEMP_K),
        ..Htr10DataConfig::default()
    }
}

fn run_case(spec: &CaseSpec) {
    println!("HTR-10 loading sweep, ENDF/B-VIII.0 -- case {}", spec.name);
    println!("=========================================================");
    println!(
        "  n_axial {}, {} rings, nominal height {:.3} cm",
        spec.n_axial, spec.n_rings, spec.nominal_height_cm
    );
    println!(
        "  {} particles x [{} inactive + {} active], seed {}, {} thread(s)",
        spec.particles, spec.inactive, spec.active, spec.seed, spec.threads
    );
    println!("  cavity: fixed core cavity {HTR10_CORE_CAVITY_CM} cm, void = cavity - bed");
    println!("  bound thermal laws: graphite, C-in-SiC, Si-in-SiC, U-in-UO2, O-in-UO2\n");

    eprintln!("Reconstructing cross sections:");
    let mut diag = RunDiagnostics::new(&format!("htr10-endf8-sweep-{}", spec.name));
    diag.note(format!(
        "{} particles x [{} inactive + {} active], {} rings x {} layers, fixed core cavity",
        spec.particles, spec.inactive, spec.active, spec.n_rings, spec.n_axial
    ));
    let data_cfg = data_config_endf8();
    let layout = Htr10NuclideLayout::plan(&data_cfg).expect("the default data configuration is valid");
    println!("  rod metal: {}", data_cfg.rod_metal.label());
    let nucs = match load_htr10_nuclides(&data_cfg, &layout, &mut diag) {
        Ok(v) => v,
        Err(e @ Htr10DataError::BlockedByGh339) => {
            println!("REFUSED: {e}");
            return;
        }
        Err(e) => {
            println!("SKIP: {e}");
            return;
        }
    };

    let z = zone_composition(spec.reflector_zone).expect("zone is listed");
    println!(
        "  reflector zone: {} (C {:.4e} x{:.3}, natural B {:.4e})",
        spec.reflector_zone, z.carbon, spec.reflector_carbon_scale, z.natural_boron
    );
    let mats = htr10_material_set(
        &layout,
        Htr10MaterialConfig {
            temperature_k: TEMP_K,
            boron: spec.boron,
            reflector_zone: spec.reflector_zone,
            reflector_carbon_scale: spec.reflector_carbon_scale,
        },
    );

    let core = assemble_explicit_triso(spec.n_rings, spec.n_axial, 0);
    println!(
        "  geometry: {} tiles, {} cells, {} universes",
        core.tiles, core.cells, core.universes
    );

    let maj = bed_majorant(&mats, &nucs);

    let settings = KeffSettings {
        n_particles: spec.particles,
        n_inactive: spec.inactive,
        n_active: spec.active,
        temperature_k: TEMP_K,
        seed: spec.seed,
        // Pinned, not Auto: with Auto the thread count follows machine load, so
        // two runs of the same case on a busy box need not use the same count,
        // and thread-count independence is NOT tested for the hybrid CSG path.
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(spec.threads)),
        ..KeffSettings::default()
    };

    // Source box and entropy mesh span the WHOLE fissile region, conus floor
    // included. A mesh blind to part of the core reports convergence of the
    // part it can see, which is the one diagnostic that must not be trusted.
    let src = fissile_source_box(&core);
    let entropy_mesh = fissile_entropy_mesh(&core);

    println!(
        "  nuclear data processed in {:.1} s ({} items)",
        diag.data_seconds(),
        diag.data_item_count()
    );

    let t = Instant::now();
    let res = diag.time_phase("transport (k-eigenvalue)", || {
        run_keff_csg_hybrid(
            &core.geometry,
            &mats,
            &nucs,
            std::slice::from_ref(&maj),
            Some(&entropy_mesh),
            src,
            &settings,
            None,
        )
    });
    let secs = t.elapsed().as_secs_f64();

    let bed_height_cm = core.bed_half_height * 2.0;
    // The reference is read at equal BALL COUNT, not equal height (gh:#472,
    // 2026-10-01): Şeker's model kept wall-crossing balls, ours keeps every
    // ball whole, so at the same height it holds 1.2 % fewer.
    let ref_height_cm = core
        .bed
        .as_ref()
        .and_then(|b| b.core_balls())
        .map_or(bed_height_cm, nee_soon::htr10_rmc::seker_height_for_balls);
    println!("  reference read at {ref_height_cm:.3} cm (equal ball count)");
    // The nominal height in the case function is documentation; the geometry is
    // the truth. If they disagree the case is mislabelled, and a mislabelled
    // height silently compares against the wrong RMC point.
    let drift = (bed_height_cm - spec.nominal_height_cm).abs();
    if drift > 0.01 {
        println!(
            "  WARNING: case says {:.3} cm, geometry built {:.3} cm ({:.3} cm apart)",
            spec.nominal_height_cm, bed_height_cm, drift
        );
    }

    let sigma_pcm = res.k_std * 1.0e5;
    println!("\n  bed height   = {bed_height_cm:.3} cm");
    println!("  k_eff        = {:.6} +/- {:.6}", res.k_mean, res.k_std);
    match rmc_at_height(ref_height_cm) {
        Some(k) => {
            let dk = (res.k_mean - k) * 1.0e5;
            println!(
                "  RMC(interp)  = {k:.6}   (headline {RMC_HEADLINE_KEFF:.6} is at 123.576 cm)"
            );
            println!("  dk           = {dk:+.0} pcm   (sigma {sigma_pcm:.0} pcm)");
            // One machine-readable line per case, so the figure's CSV is built
            // from the runs rather than retyped from them.
            println!(
                "CSV,{},{},{:.4},{:.6},{:.6},{:.1},{:.6},{:.1}",
                spec.name, spec.n_axial, bed_height_cm, res.k_mean, res.k_std, sigma_pcm, k, dk
            );
        }
        None => println!(
            "  RMC(interp)  = NONE -- {ref_height_cm:.3} cm (equal ball count) is outside the tabulated range, \
             so there is no like-for-like reference and no CSV line is emitted."
        ),
    }

    let n_hist = res.histories.max(1) as f64;
    println!(
        "  histories    = {} (planned {})",
        res.histories,
        spec.particles * (spec.inactive + spec.active)
    );
    println!(
        "  collisions   = {} ({:.2} per history)",
        res.collisions,
        res.collisions as f64 / n_hist
    );
    println!(
        "  lost locate  = {} ({:.3} %)",
        res.lost_locate,
        100.0 * res.lost_locate as f64 / n_hist
    );
    println!(
        "  stuck events = {} ({:.3} %)",
        res.stuck_events,
        100.0 * res.stuck_events as f64 / n_hist
    );
    println!(
        "  neg distance = {} (worst {:.4e} cm, level {})",
        res.neg_dist, res.neg_worst, res.neg_level
    );
    println!(
        "  leak vacuum  = {} ({:.3} %)",
        res.leak_vacuum,
        100.0 * res.leak_vacuum as f64 / n_hist
    );
    println!("  wall clock   = {secs:.1} s");

    // The entropy trace is the only thing that says whether 5 inactive
    // generations converged the source at this loading. A still-rising trace
    // means every active generation after it is biased, and the residual above
    // is then not a physics result.
    if !res.entropy.is_empty() {
        let step = (res.entropy.len() / 12).max(1);
        print!("  entropy trace:");
        for (i, h) in res.entropy.iter().enumerate() {
            if i % step == 0 || i + 1 == res.entropy.len() {
                print!(" {h:.3}");
            }
        }
        println!();
        let first = res.entropy[0];
        let last = res.entropy[res.entropy.len() - 1];
        println!(
            "  entropy      = {first:.4} -> {last:.4} bits (drift {:+.4})",
            last - first
        );
        println!(
            "ENTROPY,{},{:.4},{:.4},{:+.4}",
            spec.name,
            first,
            last,
            last - first
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cases = all_cases();

    if args.is_empty() || args[0] == "--list" {
        println!("HTR-10 ENDF/B-VIII.0 loading sweep -- one function per case.\n");
        println!("  {:<6} {:>8} {:>12}", "case", "n_axial", "height [cm]");
        for f in &cases {
            let c = f();
            println!(
                "  {:<6} {:>8} {:>12.3}",
                c.name, c.n_axial, c.nominal_height_cm
            );
        }
        println!("\nRun one case per process, pinned to its own core:");
        println!("  taskset -c 0 ./target/release/examples/htr10_endf8_height_sweep l12");
        return;
    }

    let want = args[0].as_str();
    match cases.iter().map(|f| f()).find(|c| c.name == want) {
        Some(spec) => run_case(&spec),
        None => {
            eprintln!("unknown case {want:?} -- run with --list to see the twelve names");
            std::process::exit(2);
        }
    }
}
