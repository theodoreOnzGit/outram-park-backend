// SPDX-License-Identifier: GPL-3.0

//! **Simplified LEU-COMP-THERM-008 through ACE, against the straight-from-ENDF
//! path** — a parity check with a timing breakdown.
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
//!
//! # This is NOT a reproduction of LEU-COMP-THERM-008
//!
//! The benchmark's whole value is **lumped** fuel: a 1.030 cm pellet that is
//! 176 mean free paths across at the 6.674 eV resonance, so resonance escape is
//! ≈ 0.75 and strongly self-shielded (see `lct008_keff.rs`). This example
//! **homogenises** fuel and moderator into one sphere, which destroys exactly
//! that self-shielding. The `k` it prints is therefore **not comparable to the
//! benchmark's 1.0000** and must not be quoted as an LCT-008 result.
//!
//! What survives the simplification, and is what the parity check needs, is the
//! *thermal* spectrum and the U-238 resonance absorber — so the comparison
//! still exercises the resonance region where an ACE round trip would be most
//! likely to lose something.
//!
//! Also simplified away: the aluminium cladding. Its composition includes
//! **Fe-57**, which OOMs RECONR in this workspace at every tolerance tried
//! (a known open defect), so including it would make this example unrunnable
//! for a reason that has nothing to do with ACE.

//! # Results (2026-09-23, ENDF/B-VIII.0, seed 1) — the ORIGINAL single-seed run
//!
//! Kept for the timing breakdown and because its `k` values are what the
//! superseded parity claim below was drawn from. The eight-seed numbers are the
//! ones to quote.
//!
//! ```text
//! ENDF route : k_eff = 0.84980 +/- 0.00232
//! ACE  route : k_eff = 0.85250 +/- 0.00258
//! difference : +269.3 pcm  (combined sigma 346.6 pcm, 0.78 sigma)  -> AGREE
//!
//! ENDF parse                 0.45 s   0.04 %    U235  267.6 MB
//! RECONR (0 K)             114.15 s   9.22 %    U238  326.1 MB
//! BROADR (-> 293.6 K)       27.52 s   2.22 %    O16    10.2 MB
//! ACER build               198.55 s  16.04 %    H1      0.2 MB
//! ACE write                 10.85 s   0.88 %    B10     2.9 MB
//! ACE read                   2.90 s   0.23 %    606.9 MB total
//! Nuclide::from_ace          0.23 s   0.02 %
//! Nuclide::from_endf_file  147.92 s  11.95 %
//! transport (ENDF route)   131.34 s  10.61 %
//! transport (ACE route)    603.56 s  48.77 %
//!                         1237.47 s 100.00 %
//! ```
//!
//! ## The parity result, and how much it is worth
//!
//! ~~The two routes agree at **0.78 sigma**~~ **SUPERSEDED 2026-09-25 — see the
//! eight-seed result below.** The old text called 0.78 sigma "a pass, and a weak
//! one", which was right, and then left `+269.3 pcm` standing as the measured
//! difference, which was not: over eight seeds the difference is **+23.9 pcm**
//! and the per-seed spread is **~250 pcm**, so `+269.3` was one seed's
//! fluctuation of about one standard deviation. A single-seed difference was
//! never a measurement of the difference; it is kept struck through because it
//! was quoted as one.
//!
//! The sensitive check on the same question remains
//! `tests/nuclide_from_ace_vs_endf.rs`, which compares **cross sections** rather
//! than `k` and resolves agreement to **0.03 %**.
//!
//! # Results (2026-09-25, ENDF/B-VIII.0, EIGHT seeds) — GitHub #307 item 5
//!
//! `--seeds 8`, 3000 histories x [30 inactive + 80 active] per seed. Each arm's
//! uncertainty is the standard error of the mean **over seeds**, from the
//! seed-to-seed scatter — not an average of the per-run internal estimates,
//! which understate it.
//!
//! ```text
//! ENDF route   : k_eff = 0.84956 +/- 0.00090   (URR + DBRC on, per-seed sd 255 pcm)
//! ACE  route   : k_eff = 0.84980 +/- 0.00087   (neither,        per-seed sd 245 pcm)
//! ENDF ablated : k_eff = 0.84994 +/- 0.00098   (both off,       per-seed sd 279 pcm)
//!
//! ACE - ENDF      = +23.9 +/- 125.0 pcm (0.19 sigma)   <- the parity number
//! ablated - ENDF  = +38.4 +/- 133.4 pcm (0.29 sigma)   <- the worth of URR+DBRC here
//! ACE - ablated   = -14.5 +/- 131.2 pcm (0.11 sigma)   <- the routes, SAME physics
//! change in gap   =  -9.4 +/- 181.2 pcm (0.05 sigma)
//!
//! per-seed (ACE - ENDF), pcm: +269.3, +93.4, -214.4, +204.3, -59.4, -666.0,
//!                             -45.5, +609.5
//! ```
//!
//! ## What the three arms settle, and what they do not
//!
//! **The asymmetry was real and its effect here is not.** Route A applies URR
//! self-shielding and DBRC; route B carries neither — the reader decodes the UNR
//! block since 2026-09-25, but this workspace's ACE **writer emits no UNR block**
//! (GitHub #325), and a table broadened to 293.6 K holds no 0 K elastic for DBRC.
//! Imposing the same omissions on the ENDF arm moves the comparison by
//! `-9.4 +/- 181.2 pcm`, i.e. by nothing measurable.
//!
//! **All three differences are consistent with zero**, so this is a set of
//! bounds, not a set of detections:
//!
//! - the two data routes agree to within **+/- 250 pcm at 2 sigma**;
//! - the worth of URR+DBRC on *this homogenised geometry* is below the same
//!   bound, consistent with the sharper twelve-seed measurement in
//!   `verification_and_validation/ace_route_physics/urr_dbrc_worth_2026_09_25.md`
//!   (`+63.5 +/- 77 pcm`, whose own conclusion is the bound `< 154 pcm at
//!   2 sigma`).
//!
//! **Why the worth is small here, stated rather than left to look like a null
//! result about URR in general:** this example *homogenises* the fuel, which
//! destroys the resonance self-shielding that makes the unresolved range matter.
//! The lumped `lct008_keff.rs` geometry is where URR should be priced, and that
//! measurement is not this one.
//!
//! **Resolving the remaining +23.9 pcm would take ~246x these statistics**
//! (3 sigma needs sem <= 8 pcm), which is about 2000 seeds of this example at
//! ~15 min each. That is the honest cost of turning this bound into a
//! measurement, and it is why the cross-section comparison at 0.03 % is the
//! right instrument for route parity and `k` is not.
//!
//! ## The ACE route transports 4.6x SLOWER — **7.6x as of 2026-09-25**
//!
//! Re-measured over the eight-seed run: **694 s per seed on the ACE route
//! against 91 s on the ENDF route**, where 2026-09-23 recorded 603.56 s against
//! 131.34 s. The ratio widened because the **ENDF arm got 1.4x faster** (91 s
//! against 131 s), not because the ACE arm got slower (694 s against 604 s, 1.15x
//! and within run-to-run variation on a shared machine). The ENDF speedup is
//! consistent with `develop`'s `total_at_energy` change (`1a83fad7c`), which
//! stopped the kernel building a full `XsSet` where only the total was needed;
//! that is an attribution, not a measurement — nothing here isolates it.
//!
//! The explanation below still stands, and the wider ratio is what it predicts:
//! the ACE route's cost is in cross-section lookup over one union grid, so a
//! change that makes *lookup* cheaper helps the route that does less of it.
//!
//!
//! 603.56 s against 131.34 s, same geometry, same settings, same seed, and
//! `k` agreeing. **This runs opposite to expectation.** `Nuclide::from_ace`
//! sets `urr: None` and `dbrc: None` (documented omissions -- the UNR block is
//! not decoded and an already-broadened table carries no 0 K elastic), so the
//! ACE route should be doing strictly LESS work per collision than the ENDF
//! route, which applies both.
//!
//! Candidate causes, none of them measured:
//!
//! - **Grid size.** `from_ace` puts MT=1 and MT=2 on the full ESZ grid, which
//!   for U-238 is 284 415 points. If RECONR's own per-MT grids are shorter,
//!   every lookup pays more -- though binary search makes that a logarithmic
//!   penalty, not a 4.6x one.
//! - **Inelastic level count.** `xs_at_energy` sums `eval_mt` over every level
//!   in `inel`, one search each. If the ACE route retains the 40 discrete
//!   levels where the ENDF route lumps them under MT=4 (or the reverse), the
//!   per-collision cost differs by that factor. `ce_decode::channel_mts` drops
//!   levels only when the lump is present, and whether our ACER emits MT=4
//!   has not been checked here.
//! - **Fission spectrum representation.** The ACE route always produces
//!   `FissionSpectrum::ContinuousTabular`; the ENDF route may produce a Watt
//!   form, which is far cheaper to sample.
//!
//! **RESOLVED 2026-09-23** by `examples/ace_vs_endf_route_cost.rs`, which
//! measured U-238 both ways: `xs_at_energy` costs **1.48 us/call on the ENDF
//! route and 5.86 us/call on the ACE route -- 3.96x**, which accounts for
//! essentially the whole 4.6x transport gap. So the cause is cross-section
//! lookup, not secondary sampling.
//!
//! Of the three candidates above, **two are refuted**: the inelastic level
//! count is **40 on both** routes, and the fission spectrum cannot be
//! implicated by a measurement that does no sampling. The premise held (the
//! ENDF route does carry URR 20-149 keV and DBRC; the ACE route carries
//! neither), which sharpens rather than resolves it -- the ACE route does
//! *less* physics per lookup and is still 4x slower.
//!
//! The surviving explanation is **grid size, and it is a design consequence
//! rather than a defect**: ACE stores every reaction on one union grid
//! (284 415 points for U-238), so each of ~49 sections spans its threshold to
//! the top of that grid, where RECONR thins each MT independently.
//! `xs_at_energy` sums over all 40 levels, so the ACE route walks far more
//! memory per call. MCNP pays the same cost. Thinning the per-MT grids after
//! decode would be a legitimate optimisation if it ever matters.
//!
use std::time::{Duration, Instant};

use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig, ReconrResult};
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};

const TEMP_K: f64 = 293.6;
const KT_MEV: f64 = 8.617_333_262e-5 * TEMP_K * 1.0e-6;

/// Fuel volume fraction in the homogenised mixture.
///
/// **Chosen, not taken from the benchmark.** A typical LWR lattice is roughly
/// 30 % fuel by volume; the cladding is dropped (see the module docs), and its
/// volume is given to the moderator. Since this example measures the agreement
/// between two data routes on an identical geometry, the exact fraction does
/// not affect what is being tested -- but it does mean the `k` below is not a
/// benchmark value, which is why it is stated here rather than buried.
const FUEL_VF: f64 = 0.30;
const WATER_VF: f64 = 1.0 - FUEL_VF;

/// `(name, ENDF file, MAT, fuel a/o, water a/o)` — LEU-COMP-THERM-008 case 1
/// compositions, from `verification_and_validation/icsbep/leu-comp-therm-008/
/// materials.xml`. U-234 and B-11 are dropped: both are minor, and each costs a
/// full RECONR plus a few hundred MB of ACE.
const NUCLIDES: [(&str, &str, i32, f64, f64); 5] = [
    ("U235", "n-092_U_235-ENDF8.0.endf", 9228, 0.00056868, 0.0),
    ("U238", "n-092_U_238.endf", 9237, 0.022268, 0.0),
    ("O16", "n-008_O_016-ENDF8.0.endf", 825, 0.045683, 0.033369),
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf", 125, 0.0, 0.066737),
    ("B10", "n-005_B_010-ENDF8.0.endf", 525, 2.6055e-07, 1.6769e-05),
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
    let n_particles = arg_usize(&args, "--particles").unwrap_or(3000);
    let n_inactive = arg_usize(&args, "--inactive").unwrap_or(30);
    let n_active = arg_usize(&args, "--active").unwrap_or(80);
    let seed_override = arg_usize(&args, "--seed").map(|v| v as u64);
    // `--seeds N` repeats ONLY the transport, over N consecutive seeds, reusing
    // the nuclides. The ACE build is ~300 s and the library 607 MB, so paying it
    // once and looping the cheap part is what makes a multi-seed statement
    // affordable at all. N = 1 reproduces the single-seed behaviour exactly.
    let n_seeds = arg_usize(&args, "--seeds").unwrap_or(1).max(1);

    let mut st = Stages::default();
    let wall = Instant::now();
    let scratch = std::env::temp_dir().join(format!("lct008_ace_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch dir");

    println!("Simplified LEU-COMP-THERM-008: ENDF route vs ENDF->ACE->read route");
    println!("  homogenised {:.0} % fuel / {:.0} % borated water, NOT the lumped benchmark\n",
        100.0 * FUEL_VF, 100.0 * WATER_VF);

    let mut via_endf: Vec<Nuclide> = Vec::new();
    let mut via_ace: Vec<Nuclide> = Vec::new();
    let mut ace_bytes = 0u64;

    for (name, file, mat, _, _) in NUCLIDES {
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

        // ── Route B: the same tape, out through ACER and back ──────────────
        let t = Instant::now();
        let tape = Tape::read_file(&path).expect("parse ENDF");
        st.endf_parse += t.elapsed();

        let t = Instant::now();
        let recon0 = reconr(
            &tape,
            &ReconrConfig { mat, tolerance: 1.0e-3, temperature: 0.0 },
        )
        .unwrap_or_else(|e| panic!("RECONR({name}): {e}"));
        st.reconr += t.elapsed();

        let t = Instant::now();
        let recon: ReconrResult = broaden_result(&recon0, TEMP_K);
        st.broadr += t.elapsed();

        let t = Instant::now();
        let ace = build_ace(&tape, mat, &recon);
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
        let n_ace = Nuclide::from_ace(&raw, name).unwrap_or_else(|e| panic!("from_ace({name}): {e}"));
        st.from_ace += t.elapsed();
        via_ace.push(n_ace);

        // Delete immediately: these are hundreds of MB and the container's
        // writable disk is a fixed allowance, not a disk.
        let _ = std::fs::remove_file(&out);
        println!("  {name}: ACE {:.1} MB (written, read back, removed)", bytes as f64 / 1.0e6);
    }

    // Homogenised composition: each nuclide's density is its fuel a/o times the
    // fuel volume fraction plus its water a/o times the water fraction.
    let components: Vec<NuclideComponent> = NUCLIDES
        .iter()
        .enumerate()
        .map(|(i, (_, _, _, fuel_ao, water_ao))| NuclideComponent {
            nuclide_idx: i,
            atom_density: fuel_ao * FUEL_VF + water_ao * WATER_VF,
        })
        .collect();
    let make_material = |name: &str| Material {
        id: 1,
        name: name.into(),
        temperature: TEMP_K,
        components: components.clone(),
    };

    let settings = KeffSettings {
        n_particles,
        n_inactive,
        n_active,
        temperature_k: TEMP_K,
        seed: seed_override.unwrap_or(KeffSettings::default().seed),
        ..KeffSettings::default()
    };
    // Sized so the sphere is comfortably supercritical-to-critical for a
    // thermal mixture; the absolute value is not the point (see module docs).
    let radius_cm = 40.0;

    println!(
        "\nHomogenised sphere r = {radius_cm} cm, {} histories x [{} inactive + {} active], seed {}",
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
            return Arm { k, sem: single_std, sd: 0.0, per_seed: per_seed.to_vec() };
        }
        let var = per_seed.iter().map(|v| (v - k) * (v - k)).sum::<f64>() / (n - 1.0);
        Arm { k, sem: (var / n).sqrt(), sd: var.sqrt(), per_seed: per_seed.to_vec() }
    }

    let mut k_endf: Vec<f64> = Vec::new();
    let mut k_ace: Vec<f64> = Vec::new();
    let mut std_endf = 0.0;
    let mut std_ace = 0.0;
    for s in 0..n_seeds {
        let mut set = settings.clone();
        set.seed = settings.seed + s as u64;

        let t = Instant::now();
        let r = run_keff(radius_cm, &make_material("LCT008 (ENDF)"), &via_endf, &set);
        st.transport_endf += t.elapsed();
        std_endf = r.k_std;
        k_endf.push(r.k_mean);

        let t = Instant::now();
        let r = run_keff(radius_cm, &make_material("LCT008 (ACE)"), &via_ace, &set);
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
    let a_ace = summarise(&k_ace, std_ace);
    let r_endf = KRes { k_mean: a_endf.k, k_std: a_endf.sem };
    let r_ace = KRes { k_mean: a_ace.k, k_std: a_ace.sem };

    // ── Arm A': the ENDF route with the ACE route's OMISSIONS imposed ──────
    //
    // GitHub #307 item 5. The parity number below was measured while the two
    // arms carried DIFFERENT PHYSICS: the ENDF route applies URR self-shielding
    // and DBRC by default, and route B carries neither -- the reader decodes the
    // UNR block since 2026-09-25, but THIS WORKSPACE'S ACE WRITER DOES NOT EMIT
    // ONE (no UNR block in `acer::build`), and a table broadened to 293.6 K
    // carries no 0 K elastic for DBRC. So route B still cannot have them here,
    // and the way to price the asymmetry is to take them OFF the ENDF arm.
    //
    // `via_endf` is CONSUMED rather than cloned: arm A's transport is already
    // done, and a third copy of U-238's 284 415-point grid is hundreds of MB.
    let via_endf_ablated: Vec<Nuclide> = via_endf
        .into_iter()
        .map(|n| n.without_urr_probability_tables().without_dbrc())
        .collect();
    let n_urr = via_endf_ablated
        .iter()
        .filter(|n| n.has_urr_probability_tables())
        .count();
    let n_dbrc = via_endf_ablated.iter().filter(|n| n.has_dbrc()).count();
    assert_eq!(
        (n_urr, n_dbrc),
        (0, 0),
        "the ablated arm must carry neither term, or it is not the control it claims to be"
    );
    let mut k_abl: Vec<f64> = Vec::new();
    let mut std_abl = 0.0;
    for s in 0..n_seeds {
        let mut set = settings.clone();
        set.seed = settings.seed + s as u64;
        let t = Instant::now();
        let r = run_keff(
            radius_cm,
            &make_material("LCT008 (ENDF, URR+DBRC ablated)"),
            &via_endf_ablated,
            &set,
        );
        st.transport_endf_ablated += t.elapsed();
        std_abl = r.k_std;
        k_abl.push(r.k_mean);
    }
    let a_abl = summarise(&k_abl, std_abl);
    let r_abl = KRes { k_mean: a_abl.k, k_std: a_abl.sem };

    // ── Parity ─────────────────────────────────────────────────────────────
    let d_pcm = 1.0e5 * (r_ace.k_mean - r_endf.k_mean);
    let combined = 1.0e5 * (r_endf.k_std.powi(2) + r_ace.k_std.powi(2)).sqrt();
    println!("\n  PARITY (the point of this example)");
    println!("    ENDF route : k_eff = {:.5} +/- {:.5}", r_endf.k_mean, r_endf.k_std);
    println!("    ACE  route : k_eff = {:.5} +/- {:.5}", r_ace.k_mean, r_ace.k_std);
    println!(
        "    difference : {d_pcm:+.1} pcm  (combined sigma {combined:.1} pcm, {:.2} sigma)",
        if combined > 0.0 { d_pcm.abs() / combined } else { 0.0 }
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
        "\n    NOT a benchmark value: this homogenises the lumped fuel that gives\n    \
         LEU-COMP-THERM-008 its resonance self-shielding, so k is not comparable\n    \
         to the benchmark's 1.0000."
    );

    // ── The asymmetry, priced (GitHub #307 item 5) ─────────────────────────
    let sig = |a: f64, b: f64| 1.0e5 * (a * a + b * b).sqrt();
    let d_abl_vs_endf = 1.0e5 * (r_abl.k_mean - r_endf.k_mean);
    let s_abl_vs_endf = sig(r_abl.k_std, r_endf.k_std);
    let d_ace_vs_abl = 1.0e5 * (r_ace.k_mean - r_abl.k_mean);
    let s_ace_vs_abl = sig(r_ace.k_std, r_abl.k_std);
    println!("\n  THE ASYMMETRY, PRICED (GitHub #307 item 5)");
    println!(
        "    ENDF ablated : k_eff = {:.5} +/- {:.5}   (URR off, DBRC off)",
        r_abl.k_mean, r_abl.k_std
    );
    println!(
        "    ablated - ENDF : {d_abl_vs_endf:+.1} +/- {s_abl_vs_endf:.1} pcm ({:.2} sigma)          -- the worth of URR+DBRC here",
        d_abl_vs_endf.abs() / s_abl_vs_endf.max(1e-12)
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

    let ace_prep = (st.endf_parse + st.reconr + st.broadr + st.ace_build + st.ace_write).as_secs_f64();
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
fn build_ace(tape: &Tape, mat: i32, recon: &ReconrResult) -> AceTable {
    njoy_outram_park_fork::acer::build_full(tape, mat, recon, KT_MEV, 0)
        .expect("assemble ACE")
}
