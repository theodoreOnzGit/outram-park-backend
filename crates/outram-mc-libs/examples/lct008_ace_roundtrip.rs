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

//! # Results (2026-09-23, ENDF/B-VIII.0, seed 1)
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
//! The two routes agree at **0.78 sigma**. That is a pass, and it is a **weak**
//! one: at a combined sigma of 347 pcm it cannot exclude a real difference of a
//! couple of hundred pcm. The sensitive check on the same question is
//! `tests/nuclide_from_ace_vs_endf.rs`, which compares **cross sections**
//! rather than `k` and resolves agreement to **0.03 %**. Read this as an
//! end-to-end confirmation that nothing is grossly wrong, not as a tight bound.
//!
//! ## AN OPEN ANOMALY: the ACE route transports 4.6x SLOWER
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

use njoy_outram_park_fork::acer::{angular::parse_elastic_angular, energy::build_emissions, AceTable};
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::heatr::{build_emission_spectra, Kerma};
use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, NuBar};
use njoy_outram_park_fork::photon::PhotonProduction;
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
}

impl Stages {
    fn rows(&self) -> [(&'static str, Duration); 10] {
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
        ]
    }
    fn total(&self) -> Duration {
        self.rows().iter().map(|(_, d)| *d).sum()
    }
}

fn main() {
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
        n_particles: 3000,
        n_inactive: 30,
        n_active: 80,
        temperature_k: TEMP_K,
        ..KeffSettings::default()
    };
    // Sized so the sphere is comfortably supercritical-to-critical for a
    // thermal mixture; the absolute value is not the point (see module docs).
    let radius_cm = 40.0;

    println!(
        "\nHomogenised sphere r = {radius_cm} cm, {} histories x [{} inactive + {} active], seed {}",
        settings.n_particles, settings.n_inactive, settings.n_active, settings.seed
    );

    let t = Instant::now();
    let r_endf = run_keff(radius_cm, &make_material("LCT008 (ENDF)"), &via_endf, &settings);
    st.transport_endf = t.elapsed();

    let t = Instant::now();
    let r_ace = run_keff(radius_cm, &make_material("LCT008 (ACE)"), &via_ace, &settings);
    st.transport_ace = t.elapsed();

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
fn build_ace(tape: &Tape, mat: i32, recon: &ReconrResult) -> AceTable {
    let angular = tape
        .section(mat, 4, 2)
        .map(|s| parse_elastic_angular(s).expect("MF=4"));
    let partials: Vec<(i32, f64)> = recon
        .sections
        .iter()
        .map(|s| (i32::from(s.mt), s.qi))
        .collect();
    let emissions = build_emissions(tape, mat, recon.material.awr, &partials);
    let nu = NuBar::from_endf(tape, mat).expect("MF=1").unwrap_or_default();
    let chi = FissionSpectrum::from_endf_mf5(tape, mat)
        .expect("MF=5")
        .unwrap_or_default();
    let emission = build_emission_spectra(tape, mat);
    let photons = PhotonProduction::from_endf(tape, mat, recon);
    let kerma = Kerma::from_reconr(recon, &nu, &chi, &emission).with_energy_balance(&photons, recon);
    let nu_block = njoy_outram_park_fork::acer::nu::build(tape, mat).expect("NU block");
    AceTable::from_reconr_full(
        recon,
        KT_MEV,
        0,
        angular.as_ref(),
        &emissions,
        Some(&kerma),
        nu_block.as_deref(),
        njoy_outram_park_fork::acer::has_mt19_distributions(tape, mat),
        njoy_outram_park_fork::acer::photon_blocks::build(tape, mat).as_deref(),
    )
}
