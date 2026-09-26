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
//! taskset -c 0 ./target/release/examples/htr10_endf8_height_sweep n20
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
//! The nuclide set is built in this file rather than shared with
//! `htr10_rmc_keff::nuclides`, because that one branches on the ablation knobs
//! this example exists to be free of. The two must not drift: if a tape name
//! or a thermal law changes there, change it here. The physics they build is
//! intended to be identical when `htr10_rmc_keff` is run with no knobs set.
//!
//! ## What every case holds fixed
//!
//! | quantity | value | why |
//! |---|---|---|
//! | library | ENDF/B-VIII.0 | see the thermal laws below |
//! | graphite S(a,b) | crystalline, MAT 30 | a graphite-moderated thermal system |
//! | SiC S(a,b) | C-in-SiC MAT 44, Si-in-SiC MAT 43 | SiC is a crystal; free gas is wrong |
//! | UO2 S(a,b) | U-in-UO2 MAT 48, O-in-UO2 MAT 75 | generated in-process from LEAPR decks |
//! | silicon | natural Si-28/29/30 | splits a correct total, does not change it |
//! | cavity | fixed core cavity | the only treatment there is — see below |
//! | rings | 14 | radial tiling of the bed |
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
//! Atom densities from **IAEA-TECDOC-1382** Table 4-38 via
//! [`nee_soon::htr10_rmc::materials`]; geometry from Terry et al. (2005) and
//! the same TECDOC. Evaluated data is ENDF/B-VIII.0 from `reference-data/endf/`.
//! The UO2 laws ship as no tape anywhere in this repository and are generated
//! from the LEAPR decks committed in `njoy-outram-park-fork`.
//!
//! ## Verification & validation
//!
//! **Methodology.** Each case computes `k_eff` for the HTR-10 first-criticality
//! core at one fuel-loading height and compares it against the RMC result of
//! Li, Yu & Wei (2014), **interpolated to the height actually modelled**
//! (`rmc_at_height`). Comparing against RMC's single 123.576 cm headline while
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
//! (`verification_and_validation/htr10_rmc/README.md`). A single draw is not a
//! mean and the within-run sigma does not contain that scatter.

use std::time::Instant;

use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, mat, HTR10_CORE_CAVITY_CM};
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use nee_soon::htr10_rmc::reflector::zone_composition;
use njoy_outram_park_fork::leapr::decks::SabMaterial;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::htr10::{BoronReading, Htr10Nuclides};
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::{run_keff_csg_hybrid, SourceBox};
use outram_mc_libs::run_diagnostics::{DataSource, RunDiagnostics};
use outram_mc_libs::tally::mesh::RegularMesh;

const TEMP_K: f64 = 300.15;

/// RMC's value at its 123.576 cm headline loading. Kept only so the printed
/// output can show how far the headline is from the height actually modelled.
const RMC_HEADLINE_KEFF: f64 = 1.004288;

const NUC: Htr10Nuclides = Htr10Nuclides {
    u235: 0,
    u238: 1,
    o16: 2,
    c_free: 3,
    c_graphite: 4,
    si28: 5,
    b10: 6,
    c_sic: 7,
    si29: 8,
    si30: 9,
    b11: 10,
};

/// Everything one case is. No field has a default and nothing is read from the
/// environment: this struct IS the run.
#[derive(Clone, Copy, Debug)]
struct CaseSpec {
    /// Case name, as typed on the command line.
    name: &'static str,
    /// Axial tile count. The bed height follows from it.
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

fn case_n20() -> CaseSpec {
    case!("n20", 20, 97.980)
}
fn case_n21() -> CaseSpec {
    case!("n21", 21, 102.879)
}
fn case_n23() -> CaseSpec {
    case!("n23", 23, 112.677)
}
fn case_n25() -> CaseSpec {
    case!("n25", 25, 122.475)
}
fn case_n27() -> CaseSpec {
    case!("n27", 27, 132.273)
}
fn case_n29() -> CaseSpec {
    case!("n29", 29, 142.071)
}
fn case_n31() -> CaseSpec {
    case!("n31", 31, 151.869)
}
fn case_n33() -> CaseSpec {
    case!("n33", 33, 161.667)
}
fn case_n35() -> CaseSpec {
    case!("n35", 35, 171.465)
}
fn case_n37() -> CaseSpec {
    case!("n37", 37, 181.263)
}
fn case_n39() -> CaseSpec {
    case!("n39", 39, 191.061)
}
fn case_n41() -> CaseSpec {
    case!("n41", 41, 200.859)
}

/// Every case, in loading order.
fn all_cases() -> Vec<fn() -> CaseSpec> {
    vec![
        case_n20, case_n21, case_n23, case_n25, case_n27, case_n29, case_n31, case_n33, case_n35,
        case_n37, case_n39, case_n41,
    ]
}

/// RMC's `k_eff` interpolated to the height actually modelled.
///
/// Returns `None` outside the tabulated range rather than extrapolating: past
/// the ends the curve flattens and a linear extension would invent reactivity.
fn rmc_at_height(h_cm: f64) -> Option<f64> {
    let c = nee_soon::htr10_rmc::RMC_KEFF_VS_HEIGHT;
    if h_cm < c[0].0 || h_cm > c[c.len() - 1].0 {
        return None;
    }
    for w in c.windows(2) {
        let ((h0, k0), (h1, k1)) = (w[0], w[1]);
        if (h0..=h1).contains(&h_cm) {
            return Some(k0 + (h_cm - h0) / (h1 - h0) * (k1 - k0));
        }
    }
    None
}

/// The ENDF/B-VIII.0 nuclide set, with every bound thermal law this model has.
///
/// All four law families are applied unconditionally, per the workspace rule
/// that correct physics is the default and not an opt-in. There is no ablation
/// path here on purpose — `htr10_rmc_keff` is where ablations live.
fn nuclides_endf8(diag: &mut RunDiagnostics) -> Option<Vec<Nuclide>> {
    let base =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");

    macro_rules! load {
        ($diag:expr, $n:expr, $f:expr) => {{
            let p = base.join($f);
            eprint!("  {:<6} ", $n);
            let t = Instant::now();
            let r = $diag.time_data(
                format!("{} cross sections", $n),
                DataSource::File(p.clone()),
                format!("{:.2} K, tol 1.0e-3", TEMP_K),
                || {
                    p.exists().then_some(())?;
                    Nuclide::from_endf_file(&p, $n, TEMP_K, 1.0e-3).ok()
                },
            );
            eprintln!("{:.1?}", t.elapsed());
            r
        }};
    }

    // Crystalline graphite, MAT 30 in ENDF/B-VIII.0. (VII.0 ships it as MAT 31;
    // passing the wrong one returns Err and silently drops the whole set.)
    let sab = diag.time_data(
        "graphite S(a,b)",
        DataSource::File(base.join("tsl-crystalline-graphite.endf")),
        format!("MAT 30, {TEMP_K:.2} K"),
        || {
            let p = base.join("tsl-crystalline-graphite.endf");
            if !p.exists() {
                eprintln!("  graphite S(a,b): tape not in this checkout");
                return None;
            }
            ThermalScattering::from_endf_file(p.to_str()?, 30, TEMP_K, "graphite")
                .map_err(|e| eprintln!("  graphite S(a,b) load FAILED: {e}"))
                .ok()
        },
    )?;

    let sic_sab = |diag: &mut RunDiagnostics, mat_no: i32, file: &str, name: &'static str| {
        let p = base.join(file);
        diag.time_data(
            format!("{name} S(a,b)"),
            DataSource::File(p.clone()),
            format!("MAT {mat_no}, {TEMP_K:.2} K"),
            || {
                if !p.exists() {
                    eprintln!("  {name}: {file} not in this checkout -- falling back to free gas");
                    return None;
                }
                ThermalScattering::from_endf_file(p.to_str()?, mat_no, TEMP_K, name)
                    .map_err(|e| eprintln!("  {name} S(a,b) load FAILED: {e}"))
                    .ok()
            },
        )
    };
    let c_in_sic = sic_sab(diag, 44, "tsl-CinSiC.endf", "c_SiC");
    let si_in_sic = sic_sab(diag, 43, "tsl-SiinSiC.endf", "Si_SiC");

    // No UO2 tape ships in reference-data/endf; both laws are GENERATED from
    // the LEAPR decks committed in njoy-outram-park-fork. Reproducible from a
    // deck that can be read, with no new binary tapes. Costs ~10 s and ~15 s.
    let uo2_sab = |diag: &mut RunDiagnostics, material: SabMaterial, name: &'static str| {
        eprint!("  {name:<8} LEAPR ");
        let t = Instant::now();
        let out = diag.time_data(
            format!("{name} S(a,b)"),
            DataSource::GeneratedFromLeaprDeck(material.base().to_string()),
            format!(
                "MAT {}, {TEMP_K:.2} K, generated in-process",
                material.mat()
            ),
            || {
                ThermalScattering::from_leapr(material, TEMP_K, name)
                    .map_err(|e| eprintln!("  {name} LEAPR generation FAILED: {e}"))
                    .ok()
            },
        );
        eprintln!("{:.1?}", t.elapsed());
        out
    };
    let u_in_uo2 = uo2_sab(diag, SabMaterial::UInUO2, "U_UO2");
    let o_in_uo2 = uo2_sab(diag, SabMaterial::OInUO2, "O_UO2");

    let bind = |n: Nuclide, s: &Option<ThermalScattering>| match s {
        Some(t) => n.with_thermal_scattering(t.clone()),
        None => n,
    };

    Some(vec![
        bind(load!(diag, "U235", "n-092_U_235-ENDF8.0.endf")?, &u_in_uo2),
        bind(load!(diag, "U238", "n-092_U_238.endf")?, &u_in_uo2),
        bind(load!(diag, "O16", "n-008_O_016-ENDF8.0.endf")?, &o_in_uo2),
        // 3: free-gas carbon. Unused by this example's materials, but slot 3
        // must stay occupied or every later index repoints.
        load!(diag, "C12", "n-006_C_012-ENDF8.0.endf")?,
        // 4: graphite-bound carbon.
        load!(diag, "C12", "n-006_C_012-ENDF8.0.endf")?.with_thermal_scattering(sab),
        // 5, 8, 9: natural silicon, bound in SiC.
        bind(
            load!(diag, "Si28", "n-014_Si_028-ENDF8.0.endf")?,
            &si_in_sic,
        ),
        load!(diag, "B10", "n-005_B_010-ENDF8.0.endf")?,
        // 7: carbon bound in SiC.
        bind(load!(diag, "C12", "n-006_C_012-ENDF8.0.endf")?, &c_in_sic),
        bind(
            load!(diag, "Si29", "n-014_Si_029-ENDF8.0.endf")?,
            &si_in_sic,
        ),
        bind(
            load!(diag, "Si30", "n-014_Si_030-ENDF8.0.endf")?,
            &si_in_sic,
        ),
        load!(diag, "B11", "n-005_B_011-ENDF8.0.endf")?, // 10: B-11 (gh:#311)
    ])
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
    let Some(nucs) = nuclides_endf8(&mut diag) else {
        println!("SKIP: reference-data/endf/ not in this checkout.");
        return;
    };

    let z = zone_composition(spec.reflector_zone).expect("zone is listed");
    println!(
        "  reflector zone: {} (C {:.4e} x{:.3}, natural B {:.4e})",
        spec.reflector_zone, z.carbon, spec.reflector_carbon_scale, z.natural_boron
    );
    let mats = htr10_material_set(
        NUC,
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

    let grid: Vec<f64> = (0..4096)
        .map(|i| (1.0e-4_f64.ln() + (2.0e7_f64.ln() - 1.0e-4_f64.ln()) * i as f64 / 4095.0).exp())
        .collect();
    let bed_mats: Vec<usize> = (0..=mat::HELIUM).collect();
    let maj = Majorant::over_indices(&mats, &bed_mats, &nucs, &grid, 0.3);

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
    let (zl, zu, rb) = (core.conus_floor, core.bed_half_height, core.bed_radius);
    let src = SourceBox {
        lower: Position::new(-rb, -rb, zl),
        upper: Position::new(rb, rb, zu),
    };
    let entropy_mesh = RegularMesh {
        lower_left: [-rb, -rb, zl],
        upper_right: [rb, rb, zu],
        dimension: [4, 4, 4],
    };

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
    match rmc_at_height(bed_height_cm) {
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
            "  RMC(interp)  = NONE -- {bed_height_cm:.3} cm is outside the tabulated range, \
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
        println!("  taskset -c 0 ./target/release/examples/htr10_endf8_height_sweep n20");
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
