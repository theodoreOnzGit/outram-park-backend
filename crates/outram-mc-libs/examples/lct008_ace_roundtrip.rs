// SPDX-License-Identifier: GPL-3.0

//! **LEU-COMP-THERM-008 case 1 through ACE, against the straight-from-ENDF
//! path** — a parity check with a timing breakdown.
//!
//! The model is the case-1 **lattice** that `lct008_keff.rs` runs, shared
//! through `common/lct008_model.rs`, on that example's 11-nuclide
//! `--cheap-nuclides` tier (2026-09-29, maintainer: "Lct ace roundtrip should
//! follow the lct008 keff").
//!
//! **2026-09-29: the homogenised-sphere model this example used before, and
//! every result recorded on it, were deleted at the maintainer's direction
//! because it was the wrong model; the lattice replaces it.**
//!
//! S(α,β) H in H₂O: built once from `tsl-HinH2O.endf` and attached to H-1 on
//! **every** arm, so the round trip tests the continuous-energy tables only.
//! The thermal ACE round trip has its own verification
//! (`njoy-outram-park-fork/verification_and_validation/thermal_from_ace/`).
//!
//! **The ACE arm was affected by GitHub #366 until `a15958912c`** (2026-09-29):
//! before it, `Nuclide::from_ace` kept the MT=4 lump with Q = 0 in place of the
//! discrete inelastic levels on U-235/U-238 and lost U-234 fission, so an
//! ACE-arm result from earlier builds measures that defect as well as the
//! format. `--endf-only` runs arm A alone.
//!
//! # Results (2026-09-30, code `f78b5180d5`, `--purr --seeds 8 --threads 3`)
//!
//! The lattice at 10000 × [250 + 400], 8 seeds per arm:
//!
//! | arm | k_eff |
//! |---|---|
//! | ENDF route | 1.00275 ± 0.00028 |
//! | ACE route (with PURR) | 1.00225 ± 0.00015 |
//! | ENDF with DBRC ablated, the same physics as ACE | 1.00254 ± 0.00032 |
//!
//! - **ACE − ENDF:** −49.5 ± 31.9 pcm (1.55σ).
//! - **ACE − ablated ENDF (same physics):** −28.4 ± 35.3 pcm (0.80σ).
//! - **DBRC worth here:** −21.1 ± 42.4 pcm.
//!
//! The routes agree within statistics. Resolving a ~50 pcm gap at 3σ would
//! need about 4× the seeds. Timing:
//! - building the ACE library: 183 s and 312 MB;
//! - reading it back: 0.7 s;
//! - the ENDF route: 57 s.
//!
//! Before this run the lattice had been measured at seed 1 only (ENDF 1.00294,
//! ACE 1.00178), on an older build.
//!
//! # This is an EXAMPLE, not a test, on purpose
//!
//! It generates multi-hundred-megabyte ACE files on disk and reads them back.
//! As a `#[test]` that is two problems: the files are a shared resource, so
//! parallel test binaries writing the same paths would race, and U-235 alone is
//! **~256 MB** of Type-1 ASCII, which is not something a routine suite should
//! spend. Run it deliberately:
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example lct008_ace_roundtrip
//! ```
//!
//! Files are written under a per-process directory and **deleted after they are
//! read back**, so a run leaves nothing behind.
//!
//! # What is being checked: PARITY, not the benchmark
//!
//! Two routes to the same nuclides, same geometry, same seed, same settings:
//!
//! ```text
//! A.  ENDF tape -> RECONR -> BROADR ------------------> Nuclide
//! B.  ENDF tape -> RECONR -> BROADR -> ACER -> .ace ->  Nuclide::from_ace
//! ```
//!
//! Route B differs from A only by a trip through this workspace's ACE writer
//! and reader. **The two `k` values should agree to within a few pcm**, and any
//! difference is attributable to the Type-1 ASCII format's finite precision
//! rather than to physics — both routes share the same `ReconrResult`, so the
//! energy grid is identical by construction.

use std::time::{Duration, Instant};

use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig, ReconrResult};
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use std::collections::BTreeMap;

#[path = "common/lct008_model.rs"]
mod lct008_model;

const TEMP_K: f64 = 293.6;
const KT_MEV: f64 = 8.617_333_262e-5 * TEMP_K * 1.0e-6;

/// `(name, ENDF file, MAT)` — `lct008_keff.rs`'s `TAPES_CHEAP` tier, the
/// nuclides the lattice keeps (the 24 others the model names are dropped, not
/// renormalised, exactly as that example's `--cheap-nuclides` does). MATs read
/// out of the tapes themselves.
const NUCLIDES: [(&str, &str, i32); 11] = [
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf", 125),
    ("B10", "n-005_B_010-ENDF8.0.endf", 525),
    ("O16", "n-008_O_016-ENDF8.0.endf", 825),
    ("U234", "n-092_U_234-ENDF8.0.endf", 9225),
    ("U235", "n-092_U_235-ENDF8.0.endf", 9228),
    ("U238", "n-092_U_238.endf", 9237),
    ("Al27", "n-013_Al_027-ENDF8.0.endf", 1325),
    ("Si28", "n-014_Si_028-ENDF8.0.endf", 1425),
    ("Si29", "n-014_Si_029-ENDF8.0.endf", 1428),
    ("Si30", "n-014_Si_030-ENDF8.0.endf", 1431),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf", 2525),
];

#[derive(Default, Clone, Copy)]
struct Stages {
    endf_parse: Duration,
    reconr: Duration,
    broadr: Duration,
    ace_build: Duration,
    ace_write: Duration,
    ace_read: Duration,
    from_ace: Duration,
    endf_direct: Duration,
    transport_endf: Duration,
    transport_ace: Duration,
    transport_endf_ablated: Duration,
}

impl Stages {
    fn rows(&self) -> [(&'static str, Duration); 11] {
        [
            ("ENDF parse", self.endf_parse),
            ("RECONR (0 K)", self.reconr),
            ("BROADR (-> 293.6 K)", self.broadr),
            ("ACER build", self.ace_build),
            ("ACE write", self.ace_write),
            ("ACE read", self.ace_read),
            ("Nuclide::from_ace", self.from_ace),
            ("Nuclide::from_endf_file", self.endf_direct),
            ("transport (ENDF route)", self.transport_endf),
            ("transport (ACE route)", self.transport_ace),
            ("transport (ENDF ablated)", self.transport_endf_ablated),
        ]
    }
    fn total(&self) -> Duration {
        self.rows().iter().map(|(_, d)| *d).sum()
    }
}

/// One arm's answer, as the reporting code wants it: a `k` and the uncertainty
/// **on that `k`**.
///
/// With one seed those are `run_keff`'s own mean and its internal standard
/// error; with several they are the mean over seeds and the standard error of
/// *that mean*, from the seed-to-seed scatter. One type for both is what lets
/// the comparison below be written once — and the two uncertainties are **not**
/// interchangeable, which is why the multi-seed path reports the scatter rather
/// than averaging the internal estimates: independent runs of a power iteration
/// disagree by more than each run thinks it knows.
struct KRes {
    k_mean: f64,
    k_std: f64,
}

/// `--flag <usize>` from the command line, if present.
fn arg_usize(args: &[String], flag: &str) -> Option<usize> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1)?.parse().ok()
}

fn main() {
    // Settings are overridable so a sweep can drive this example at a fixed
    // particle count and one seed per run, rather than the single hard-coded
    // configuration the parity check alone needed (2026-09-24).
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Lattice defaults: `lct008_keff.rs`'s high-statistics setting, which its
    // source-convergence checks were sized for.
    let n_particles = arg_usize(&args, "--particles").unwrap_or(10_000);
    let n_inactive = arg_usize(&args, "--inactive").unwrap_or(250);
    let n_active = arg_usize(&args, "--active").unwrap_or(400);
    let threads = arg_usize(&args, "--threads").unwrap_or(4);
    let seed_override = arg_usize(&args, "--seed").map(|v| v as u64);
    // `--seeds N` repeats ONLY the transport, over N consecutive seeds, reusing
    // the nuclides. The ACE build is ~300 s and the library 607 MB, so paying it
    // once and looping the cheap part is what makes a multi-seed statement
    // affordable at all. N = 1 reproduces the single-seed behaviour exactly.
    let n_seeds = arg_usize(&args, "--seeds").unwrap_or(1).max(1);
    // `--purr` builds route B with `build_full_with_purr` (GitHub #325), so the
    // ACE arm carries URR probability tables AT SOURCE, as the ENDF arm does.
    // That changes what the right control is: only DBRC is then asymmetric, so
    // the third arm ablates DBRC alone. Without the flag the example is exactly
    // the configuration the eight-seed record was taken with.
    let purr = args.iter().any(|a| a == "--purr");
    // `--endf-only` runs arm A alone -- no ACE build, no ACE or ablated arm.
    // For measuring the ENDF arm while the ACE arm is known-defective
    // (GitHub #366); it prints arm A's per-seed k and pooled mean and stops.
    let endf_only = args.iter().any(|a| a == "--endf-only");

    let mut st = Stages::default();
    let wall = Instant::now();
    let scratch = std::env::temp_dir().join(format!("lct008_ace_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch dir");

    println!(
        "LEU-COMP-THERM-008 case 1 lattice (11-nuclide tier): ENDF route vs ENDF->ACE->read route"
    );

    let mut via_endf: Vec<Nuclide> = Vec::new();
    let mut via_ace: Vec<Nuclide> = Vec::new();
    let mut ace_bytes = 0u64;

    for (name, file, mat) in NUCLIDES {
        let Some(path) = reference_endf(file) else {
            println!("SKIP: reference tape {file} is not present");
            return;
        };

        // ── Route A: straight from ENDF ────────────────────────────────────
        let t = Instant::now();
        let n_endf = Nuclide::from_endf_file(&path, name, TEMP_K, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({name}): {e}"));
        st.endf_direct += t.elapsed();
        via_endf.push(n_endf);
        if endf_only {
            continue;
        }

        // ── Route B: the same tape, out through ACER and back ──────────────
        let t = Instant::now();
        let tape = Tape::read_file(&path).expect("parse ENDF");
        st.endf_parse += t.elapsed();

        let t = Instant::now();
        let recon0 = reconr(
            &tape,
            &ReconrConfig {
                mat,
                tolerance: 1.0e-3,
                temperature: 0.0,
            },
        )
        .unwrap_or_else(|e| panic!("RECONR({name}): {e}"));
        st.reconr += t.elapsed();

        let t = Instant::now();
        let recon: ReconrResult = broaden_result(&recon0, TEMP_K);
        st.broadr += t.elapsed();

        let t = Instant::now();
        let ace = build_ace(&tape, mat, &recon, purr);
        st.ace_build += t.elapsed();

        let out = scratch.join(format!("{name}.ace"));
        let t = Instant::now();
        ace.write_type1(&out).expect("write ACE");
        st.ace_write += t.elapsed();
        let bytes = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
        ace_bytes += bytes;

        let t = Instant::now();
        let raw = njoy_outram_park_fork::acer::read::read(&out).expect("read ACE");
        st.ace_read += t.elapsed();

        let t = Instant::now();
        let n_ace =
            Nuclide::from_ace(&raw, name).unwrap_or_else(|e| panic!("from_ace({name}): {e}"));
        st.from_ace += t.elapsed();
        via_ace.push(n_ace);

        // Delete immediately: these are hundreds of MB and the container's
        // writable disk is a fixed allowance, not a disk.
        let _ = std::fs::remove_file(&out);
        println!(
            "  {name}: ACE {:.1} MB (written, read back, removed)",
            bytes as f64 / 1.0e6
        );
    }

    // The lattice model, shared with `lct008_keff.rs` (common/lct008_model.rs).
    let _ = lct008_model::ACTIVE_CASE.set(1);
    let spec = lct008_model::parse_materials(lct008_model::materials_xml());
    let slots: BTreeMap<String, usize> = NUCLIDES
        .iter()
        .enumerate()
        .map(|(i, (n, _, _))| (n.to_string(), i))
        .collect();
    let mut omitted: BTreeMap<String, f64> = BTreeMap::new();
    for m in &spec {
        for (n, ao) in &m.nuclides {
            if !slots.contains_key(n) {
                *omitted.entry(n.clone()).or_insert(0.0) += ao;
            }
        }
    }
    let (materials, _) = lct008_model::build_materials(&spec, &slots, &omitted, false);
    lct008_model::report_omissions(&spec, &omitted);
    let geom = lct008_model::build_geometry(&materials, true);
    let _ = lct008_model::check_geometry(&geom, &materials);
    let src = SourceBox {
        lower: Position::new(
            -lct008_model::R_CORE,
            -lct008_model::R_CORE,
            lct008_model::Z_LO,
        ),
        upper: Position::new(
            lct008_model::R_CORE,
            lct008_model::R_CORE,
            lct008_model::Z_HI,
        ),
    };
    let make_material = |_name: &str| -> Vec<Material> { materials.clone() };

    // S(a,b) H in H2O on H-1, the SAME table on every arm (see module docs).
    let sab = ThermalScattering::from_endf_file(
        reference_endf("tsl-HinH2O.endf")
            .expect("H(H2O) tape")
            .to_str()
            .expect("path"),
        1,
        TEMP_K,
        "c_H_in_H2O",
    )
    .expect("H(H2O) S(a,b)");
    let h1 = slots["H1"];
    via_endf[h1] = via_endf[h1].clone().with_thermal_scattering(sab.clone());
    if !endf_only {
        via_ace[h1] = via_ace[h1].clone().with_thermal_scattering(sab);
    }

    let settings = KeffSettings {
        n_particles,
        n_inactive,
        n_active,
        temperature_k: TEMP_K,
        seed: seed_override.unwrap_or(KeffSettings::default().seed),
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
        ..KeffSettings::default()
    };

    println!(
        "\nLattice, {} histories x [{} inactive + {} active], seed {}, {threads} threads",
        settings.n_particles, settings.n_inactive, settings.n_active, settings.seed
    );

    // One arm's `k` over the seeds: the mean, the standard error OF THAT MEAN,
    // and the per-seed spread. With one seed the sem is the run's own reported
    // `k_std`; with several it is the seed-to-seed scatter, which is the honest
    // uncertainty on a mean over independent runs and is usually LARGER than a
    // single run's internal estimate.
    struct Arm {
        k: f64,
        sem: f64,
        sd: f64,
        per_seed: Vec<f64>,
    }
    fn summarise(per_seed: &[f64], single_std: f64) -> Arm {
        let n = per_seed.len() as f64;
        let k = per_seed.iter().sum::<f64>() / n;
        if per_seed.len() < 2 {
            return Arm {
                k,
                sem: single_std,
                sd: 0.0,
                per_seed: per_seed.to_vec(),
            };
        }
        let var = per_seed.iter().map(|v| (v - k) * (v - k)).sum::<f64>() / (n - 1.0);
        Arm {
            k,
            sem: (var / n).sqrt(),
            sd: var.sqrt(),
            per_seed: per_seed.to_vec(),
        }
    }

    let mut k_endf: Vec<f64> = Vec::new();
    let mut k_ace: Vec<f64> = Vec::new();
    let mut std_endf = 0.0;
    let mut std_ace = 0.0;
    for s in 0..n_seeds {
        let mut set = settings.clone();
        set.seed = settings.seed + s as u64;

        let t = Instant::now();
        let r = run_keff_csg(&geom, &make_material("ENDF"), &via_endf, src, &set, None);
        st.transport_endf += t.elapsed();
        std_endf = r.k_std;
        k_endf.push(r.k_mean);
        if endf_only {
            println!(
                "    seed {:>3}: ENDF {:.5} +/- {:.5}",
                set.seed, r.k_mean, r.k_std
            );
            continue;
        }

        let t = Instant::now();
        let r = run_keff_csg(&geom, &make_material("ACE"), &via_ace, src, &set, None);
        st.transport_ace += t.elapsed();
        std_ace = r.k_std;
        k_ace.push(r.k_mean);

        if n_seeds > 1 {
            println!(
                "    seed {:>3}: ENDF {:.5}   ACE {:.5}   ({:+.1} pcm)",
                set.seed,
                k_endf[s],
                k_ace[s],
                1.0e5 * (k_ace[s] - k_endf[s])
            );
        }
    }
    let a_endf = summarise(&k_endf, std_endf);
    if endf_only {
        println!(
            "\n  ARM A ONLY (--endf-only): k_eff = {:.5} +/- {:.5} over {} seeds (seed sd {:.0} pcm), \
             delta vs 1.0000 = {:+.0} +/- {:.0} pcm",
            a_endf.k,
            a_endf.sem,
            k_endf.len(),
            1.0e5 * a_endf.sd,
            1.0e5 * (a_endf.k - 1.0),
            1.0e5 * a_endf.sem
        );
        let _ = std::fs::remove_dir_all(&scratch);
        return;
    }
    let a_ace = summarise(&k_ace, std_ace);
    let r_endf = KRes {
        k_mean: a_endf.k,
        k_std: a_endf.sem,
    };
    let r_ace = KRes {
        k_mean: a_ace.k,
        k_std: a_ace.sem,
    };

    // ── Arm A': the ENDF route with the ACE route's OMISSIONS imposed ──────
    //
    // GitHub #307 item 5. The parity number below was measured while the two
    // arms carried DIFFERENT PHYSICS: the ENDF route applies URR self-shielding
    // and DBRC by default, and route B carries neither -- the reader decodes the
    // UNR block since 2026-09-25, but route B below is built by
    // `build_full` -- the RECONR+BROADR+ACER deck, WITHOUT PURR -- which writes
    // no UNR block, and a table broadened to 293.6 K carries no 0 K elastic for
    // DBRC. So route B does not have them here, and the way to price the
    // asymmetry is to take them OFF the ENDF arm.
    //
    // (Until 2026-09-26 the writer could not emit a UNR block at all; #325 added
    // `build_full_with_purr`, which does. Switching route B to it would remove
    // the URR half of the asymmetry at source, leaving only DBRC -- and would be
    // a different experiment from the eight seeds recorded above.)
    //
    // `via_endf` is CONSUMED rather than cloned: arm A's transport is already
    // done, and a third copy of U-238's 284 415-point grid is hundreds of MB.
    // With `--purr` route B carries URR itself, so only DBRC is asymmetric and
    // only DBRC comes off; without it both do. Either way the arm asserts it
    // carries exactly what route B carries, so it cannot silently stop being
    // the control it is labelled as.
    let via_endf_ablated: Vec<Nuclide> = via_endf
        .into_iter()
        .map(|n| {
            if purr {
                n.without_dbrc()
            } else {
                n.without_urr_probability_tables().without_dbrc()
            }
        })
        .collect();
    let n_urr = via_endf_ablated
        .iter()
        .filter(|n| n.has_urr_probability_tables())
        .count();
    let n_dbrc = via_endf_ablated.iter().filter(|n| n.has_dbrc()).count();
    let n_urr_ace = via_ace
        .iter()
        .filter(|n| n.has_urr_probability_tables())
        .count();
    assert_eq!(
        (n_urr, n_dbrc),
        (n_urr_ace, 0),
        "the ablated arm must carry the ACE arm's URR and no DBRC, or it is not the \
         control it claims to be"
    );
    println!(
        "\n  route B {} PURR: ACE arm carries URR on {n_urr_ace} nuclide(s); the control \
         arm ablates {}",
        if purr { "WITH" } else { "WITHOUT" },
        if purr { "DBRC only" } else { "URR and DBRC" }
    );
    let mut k_abl: Vec<f64> = Vec::new();
    let mut std_abl = 0.0;
    for s in 0..n_seeds {
        let mut set = settings.clone();
        set.seed = settings.seed + s as u64;
        let t = Instant::now();
        let r = run_keff_csg(
            &geom,
            &make_material("ENDF, ablated"),
            &via_endf_ablated,
            src,
            &set,
            None,
        );
        st.transport_endf_ablated += t.elapsed();
        std_abl = r.k_std;
        k_abl.push(r.k_mean);
    }
    let a_abl = summarise(&k_abl, std_abl);
    let r_abl = KRes {
        k_mean: a_abl.k,
        k_std: a_abl.sem,
    };

    // ── Parity ─────────────────────────────────────────────────────────────
    let d_pcm = 1.0e5 * (r_ace.k_mean - r_endf.k_mean);
    let combined = 1.0e5 * (r_endf.k_std.powi(2) + r_ace.k_std.powi(2)).sqrt();
    println!("\n  PARITY (the point of this example)");
    println!(
        "    ENDF route : k_eff = {:.5} +/- {:.5}",
        r_endf.k_mean, r_endf.k_std
    );
    println!(
        "    ACE  route : k_eff = {:.5} +/- {:.5}",
        r_ace.k_mean, r_ace.k_std
    );
    println!(
        "    difference : {d_pcm:+.1} pcm  (combined sigma {combined:.1} pcm, {:.2} sigma)",
        if combined > 0.0 {
            d_pcm.abs() / combined
        } else {
            0.0
        }
    );
    if d_pcm.abs() <= 2.0 * combined {
        println!("    => the two routes AGREE within statistics.");
    } else {
        println!(
            "    => the routes DISAGREE at {:.1} sigma. Both share the same ReconrResult, so\n       \
             the energy grid is identical by construction and this is not a\n       \
             reconstruction difference -- look at the ACE writer or reader.",
            d_pcm.abs() / combined.max(1e-12)
        );
    }
    println!(
        "\n    The lattice on an 11-nuclide tier: 24 clad and boron trace nuclides are\n    \
         dropped, so k is close to, but not exactly, the full model's."
    );

    // ── The asymmetry, priced (GitHub #307 item 5) ─────────────────────────
    let sig = |a: f64, b: f64| 1.0e5 * (a * a + b * b).sqrt();
    let d_abl_vs_endf = 1.0e5 * (r_abl.k_mean - r_endf.k_mean);
    let s_abl_vs_endf = sig(r_abl.k_std, r_endf.k_std);
    let d_ace_vs_abl = 1.0e5 * (r_ace.k_mean - r_abl.k_mean);
    let s_ace_vs_abl = sig(r_ace.k_std, r_abl.k_std);
    println!("\n  THE ASYMMETRY, PRICED (GitHub #307 item 5)");
    println!(
        "    ENDF ablated : k_eff = {:.5} +/- {:.5}   ({})",
        r_abl.k_mean,
        r_abl.k_std,
        if purr {
            "DBRC off; URR on, as in route B"
        } else {
            "URR off, DBRC off"
        }
    );
    println!(
        "    ablated - ENDF : {d_abl_vs_endf:+.1} +/- {s_abl_vs_endf:.1} pcm ({:.2} sigma)          -- the worth of {} here",
        d_abl_vs_endf.abs() / s_abl_vs_endf.max(1e-12),
        if purr { "DBRC" } else { "URR+DBRC" }
    );
    println!(
        "    ACE - ablated  : {d_ace_vs_abl:+.1} +/- {s_ace_vs_abl:.1} pcm ({:.2} sigma)          -- the routes with the SAME physics",
        d_ace_vs_abl.abs() / s_ace_vs_abl.max(1e-12)
    );
    println!(
        "    ACE - ENDF     : {d_pcm:+.1} +/- {combined:.1} pcm ({:.2} sigma)          -- what was quoted before",
        d_pcm.abs() / combined.max(1e-12)
    );
    // THE VERDICT IS GATED ON THE STATISTICS, not on which number is bigger.
    //
    // A first version of this block printed "removing the asymmetry did NOT
    // close the gap" whenever |ACE - ablated| >= |ACE - ENDF|, and on the first
    // run that meant announcing a conclusion from a 37 pcm change between two
    // differences whose own sigmas are ~350 pcm. That is the failure this
    // workspace's process rule is about -- a comparison that cannot fail is not
    // evidence -- so the change in gap is quoted WITH its uncertainty and a
    // direction is claimed only when it exceeds it.
    let d_gap = d_ace_vs_abl.abs() - d_pcm.abs();
    let s_gap = (s_ace_vs_abl * s_ace_vs_abl + combined * combined).sqrt();
    println!(
        "    change in gap  : {d_gap:+.1} +/- {s_gap:.1} pcm ({:.2} sigma)",
        d_gap.abs() / s_gap.max(1e-12)
    );
    if d_gap.abs() < s_gap {
        println!(
            "    => NOT RESOLVED: the gap changes by less than the uncertainty on that\n       \
             change, so this run cannot say whether the asymmetry explained any of the\n       \
             {:+.1} pcm. What it does say is that removing the asymmetry creates no\n       \
             disagreement either. Resolving a {:.0} pcm gap at 3 sigma needs sem <=\n       \
             {:.0} pcm, about {:.0}x this run's histories or seeds.",
            d_pcm,
            d_pcm.abs(),
            d_pcm.abs() / 3.0,
            (3.0 * combined / d_pcm.abs().max(1e-12)).powi(2)
        );
    } else if d_gap < 0.0 {
        println!(
            "    => removing the asymmetry moved the arms CLOSER by {:.1} pcm, more than the\n       \
             uncertainty on that change, so part of the {:+.1} pcm was the missing\n       \
             physics rather than the format.",
            d_gap.abs(),
            d_pcm
        );
    } else {
        println!(
            "    => removing the asymmetry moved the arms FURTHER APART by {:.1} pcm, more\n       \
             than the uncertainty on that change, so the {:+.1} pcm is not explained by\n       \
             URR+DBRC and the routes differ for another reason.",
            d_gap.abs(),
            d_pcm
        );
    }
    if n_seeds > 1 {
        println!(
            "    per-seed spread: ENDF sd {:.0} pcm, ACE sd {:.0} pcm, ablated sd {:.0} pcm \
             over {n_seeds} seeds",
            1.0e5 * a_endf.sd,
            1.0e5 * a_ace.sd,
            1.0e5 * a_abl.sd
        );
    }
    println!(
        "    NOTE ON PAIRING: URR and DBRC change how many variates a history draws, so\n             the ablated arm's random stream DIVERGES from the unablated one. These are\n             independent runs, not a paired difference -- the sigmas above are sqrt(2) x the\n             per-arm sigma and no variance cancels. Measured in\n             verification_and_validation/ace_route_physics/urr_dbrc_worth_2026_09_25.md."
    );

    // ── Timing ─────────────────────────────────────────────────────────────
    let total = st.total();
    let tot_s = total.as_secs_f64();
    println!("\n  stage                       time [s]     % of accounted");
    println!("  --------------------------------------------------------");
    for (label, d) in st.rows() {
        println!(
            "  {label:<26} {:>8.2}     {:>6.2} %",
            d.as_secs_f64(),
            100.0 * d.as_secs_f64() / tot_s.max(1e-12)
        );
    }
    println!("  --------------------------------------------------------");
    println!("  accounted                  {tot_s:>8.2}     100.00 %");
    println!(
        "  wall clock                 {:>8.2}     (unaccounted {:.2} s)",
        wall.elapsed().as_secs_f64(),
        wall.elapsed().as_secs_f64() - tot_s
    );

    let ace_prep =
        (st.endf_parse + st.reconr + st.broadr + st.ace_build + st.ace_write).as_secs_f64();
    let ace_load = (st.ace_read + st.from_ace).as_secs_f64();
    println!(
        "\n  Building the ACE library cost {ace_prep:.2} s and {:.1} MB; loading it back cost\n  \
         {ace_load:.2} s. The straight-from-ENDF route cost {:.2} s. So ACE pays for itself\n  \
         after about {:.1} reuses of this library.",
        ace_bytes as f64 / 1.0e6,
        st.endf_direct.as_secs_f64(),
        if st.endf_direct.as_secs_f64() > ace_load {
            ace_prep / (st.endf_direct.as_secs_f64() - ace_load).max(1e-9)
        } else {
            f64::INFINITY
        }
    );

    let _ = std::fs::remove_dir_all(&scratch);
}

/// Assemble every block a CE ACE table carries, from a broadened `ReconrResult`.
/// Assemble the ACE table. Thin wrapper over
/// [`njoy_outram_park_fork::acer::build_full`], which owns the assembly order
/// so this example, `njoy`'s own `write_ace.rs` and `lct008_keff.rs` cannot
/// drift apart (the three copies this replaced were identical, and that is
/// exactly the state in which one quietly stops being).
fn build_ace(tape: &Tape, mat: i32, recon: &ReconrResult, purr: bool) -> AceTable {
    if purr {
        // The reference library's own PURR settings (`purr / MAT 1 1 20 64 /`)
        // with this crate's verified sample count.
        njoy_outram_park_fork::acer::build_full_with_purr(
            tape, mat, recon, KT_MEV, 0, 20, 64, 10_000,
        )
        .expect("assemble ACE with PURR")
    } else {
        njoy_outram_park_fork::acer::build_full(tape, mat, recon, KT_MEV, 0).expect("assemble ACE")
    }
}
