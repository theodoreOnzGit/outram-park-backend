// SPDX-License-Identifier: GPL-3.0

//! **Godiva through a round trip of this workspace's own ACE writer and
//! reader**, with a timing breakdown of every stage.
//!
//! # What this measures
//!
//! `ENDF tape -> RECONR -> BROADR -> ACER (write) -> read -> Nuclide -> k_eff`
//!
//! Every stage is this workspace's own code. That makes it a **round trip**,
//! not a cross-code check: it says the ACE writer and the ACE reader agree with
//! each other and that the result still transports, which is a different and
//! weaker claim than `nuclide_from_ace_vs_endf.rs`, where the tables come from
//! NJOY2016 and nothing on the write side is ours.
//!
//! Both matter. A round trip catches a writer that drops a block or a reader
//! that mis-locates one; only the cross-code test catches a *format* that is
//! self-consistently wrong.
//!
//! # Why the timing breakdown is the interesting part
//!
//! ACE exists because resonance reconstruction is expensive and you would
//! rather do it once. This prints where the time actually goes, so the claim
//! "ACE is the fast path" is measured rather than assumed.
//!
//! # The benchmark
//!
//! HEU-MET-FAST-001 (Godiva): a bare sphere of 93.7 % enriched uranium,
//! `r = 8.7407 cm`, all three ICSBEP nuclides at their specified atom
//! densities. Benchmark value `1.0000 ± 0.0010`.
//!
//! **Single seed, deliberately.** One draw of this problem has a seed-to-seed
//! spread of roughly `sd = 173 pcm` at these settings (measured over 256 seeds,
//! see `godiva_keff_ensemble.rs`), so a single `k` here is one sample from that
//! distribution and must NOT be read as the code's answer. It is reported with
//! that spread attached.

//! # Results (2026-09-23, ENDF/B-VIII.0, seed 1)
//!
//! ```text
//! k_eff = 1.00206 +/- 0.00163   (+206 pcm vs ICSBEP 1.0000 +/- 0.0010)
//!                                +1.19 sd of the 173 pcm seed-to-seed spread
//!
//! stage                     time [s]     % of accounted
//! ENDF parse                   0.32       0.08 %
//! RECONR (0 K)               122.96      29.39 %
//! BROADR (-> 293.6 K)         41.86      10.00 %
//! ACER build                 109.88      26.26 %
//! ACE write                   11.40       2.73 %
//! ACE read                     3.12       0.75 %
//! Nuclide::from_ace            0.26       0.06 %
//! transport (k_eff)          128.64      30.74 %
//!                            418.44     100.00 %
//!
//! U-234  42 357 grid points,  47.8 MB      data prep  69.3 %
//! U-235 143 789 grid points, 267.6 MB      transport  30.7 %
//! U-238 284 415 grid points, 326.1 MB      641.5 MB total
//! ```
//!
//! **Building the library costs 286.42 s; loading it back costs 3.37 s** -- an
//! 85x asymmetry, which is the entire reason the ACE format exists. RECONR
//! (29 %) and the ACER build (26 %) dominate preparation; the ASCII write is
//! 2.7 % and the read 0.8 %, so the format's verbosity is not the cost.
//!
//! The `k` is ONE SEED. At +1.19 sd of this configuration's own spread it is an
//! unremarkable draw, and it is neither evidence for nor against the port's
//! accuracy -- `godiva_keff_ensemble.rs` and its 256-seed `+16 +/- 11 pcm` is
//! where that question is answered.
//!
use std::time::{Duration, Instant};

use njoy_outram_park_fork::acer::{angular::parse_elastic_angular, energy::build_emissions, AceTable};
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::heatr::{build_emission_spectra, Kerma};
use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, NuBar};
use njoy_outram_park_fork::photon::PhotonProduction;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};

const TEMP_K: f64 = 293.6;
/// kT in MeV at `TEMP_K`: Boltzmann 8.617333262e-5 eV/K.
const KT_MEV: f64 = 8.617_333_262e-5 * TEMP_K * 1.0e-6;
/// Seed-to-seed spread of this configuration, measured over 256 seeds.
const ENSEMBLE_SD_PCM: f64 = 173.0;

#[derive(Default)]
struct Stages {
    endf_parse: Duration,
    reconr: Duration,
    broadr: Duration,
    ace_build: Duration,
    ace_write: Duration,
    ace_read: Duration,
    from_ace: Duration,
    transport: Duration,
}

impl Stages {
    fn rows(&self) -> [(&'static str, Duration); 8] {
        [
            ("ENDF parse", self.endf_parse),
            ("RECONR (0 K)", self.reconr),
            ("BROADR (-> 293.6 K)", self.broadr),
            ("ACER build", self.ace_build),
            ("ACE write", self.ace_write),
            ("ACE read", self.ace_read),
            ("Nuclide::from_ace", self.from_ace),
            ("transport (k_eff)", self.transport),
        ]
    }
    fn total(&self) -> Duration {
        self.rows().iter().map(|(_, d)| *d).sum()
    }
}

fn main() {
    let mut st = Stages::default();
    let wall = Instant::now();
    // Per-process, so two concurrent runs cannot race on the same paths.
    let scratch = std::env::temp_dir().join(format!("godiva_ace_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch dir");

    println!("Godiva via ENDF -> ACE -> Nuclide, all through this workspace's own code\n");

    let mut nuclides: Vec<Nuclide> = Vec::new();
    let mut ace_bytes_total = 0u64;

    for (name, file, mat) in [
        ("U234", "n-092_U_234-ENDF8.0.endf", 9225_i32),
        ("U235", "n-092_U_235-ENDF8.0.endf", 9228),
        ("U238", "n-092_U_238.endf", 9237),
    ] {
        let Some(path) = reference_endf(file) else {
            println!("SKIP: reference tape {file} is not present");
            return;
        };

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
        .expect("RECONR");
        st.reconr += t.elapsed();

        let t = Instant::now();
        let recon = broaden_result(&recon0, TEMP_K);
        st.broadr += t.elapsed();

        // ── ACER: assemble every block the CE table carries ────────────────
        let t = Instant::now();
        let angular = tape
            .section(mat, 4, 2)
            .map(|s| parse_elastic_angular(s).expect("MF=4"));
        let partials: Vec<(i32, f64)> = recon
            .sections
            .iter()
            .map(|s| (i32::from(s.mt), s.qi))
            .collect();
        let emissions = build_emissions(&tape, mat, recon.material.awr, &partials);
        let nu = NuBar::from_endf(&tape, mat).expect("MF=1").unwrap_or_default();
        let chi = FissionSpectrum::from_endf_mf5(&tape, mat)
            .expect("MF=5")
            .unwrap_or_default();
        let emission = build_emission_spectra(&tape, mat);
        let photons = PhotonProduction::from_endf(&tape, mat, &recon);
        let kerma = Kerma::from_reconr(&recon, &nu, &chi, &emission)
            .with_energy_balance(&photons, &recon);
        let nu_block = njoy_outram_park_fork::acer::nu::build(&tape, mat).expect("NU block");
        let ace = AceTable::from_reconr_full(
            &recon,
            KT_MEV,
            0,
            angular.as_ref(),
            &emissions,
            Some(&kerma),
            nu_block.as_deref(),
            njoy_outram_park_fork::acer::has_mt19_distributions(&tape, mat),
            njoy_outram_park_fork::acer::photon_blocks::build(&tape, mat).as_deref(),
        );
        st.ace_build += t.elapsed();

        // Write to disk and read back, so the round trip goes through the
        // actual Type-1 text format rather than staying in memory -- the
        // formatting layer is where a precision loss would hide.
        let out = scratch.join(format!("{name}.ace"));
        let t = Instant::now();
        ace.write_type1(&out).expect("write ACE");
        st.ace_write += t.elapsed();
        ace_bytes_total += std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);

        let t = Instant::now();
        let raw = njoy_outram_park_fork::acer::read::read(&out).expect("read ACE");
        st.ace_read += t.elapsed();

        let t = Instant::now();
        let n = Nuclide::from_ace(&raw, name).expect("from_ace");
        st.from_ace += t.elapsed();

        println!(
            "  {name}: {} grid points, ACE {:.1} MB (read back, removed)",
            raw.nxs[njoy_outram_park_fork::acer::nxs::NES],
            std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0) as f64 / 1.0e6
        );
        // Delete as soon as it has been read. A full Godiva library is 641 MB
        // and this container's writable space is a fixed allowance, not a
        // disk: the first version of this example left all three files behind.
        let _ = std::fs::remove_file(&out);
        nuclides.push(n);
    }

    // HEU-MET-FAST-001 atom densities [atoms/barn·cm], the same numbers every
    // other Godiva example here uses, so the runs differ only in where sigma
    // comes from.
    let material = Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: TEMP_K,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 }, // U-234
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 }, // U-235
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 }, // U-238
        ],
    };
    let settings = KeffSettings {
        n_particles: 5000,
        n_inactive: 40,
        n_active: 120,
        temperature_k: TEMP_K,
        ..KeffSettings::default()
    };
    let radius_cm = 8.7407;

    println!(
        "\nGodiva bare sphere, r = {radius_cm} cm, {} histories x [{} inactive + {} active], seed {}",
        settings.n_particles, settings.n_inactive, settings.n_active, settings.seed
    );

    let t = Instant::now();
    let result = run_keff(radius_cm, &material, &nuclides, &settings);
    st.transport = t.elapsed();

    let pcm = 1.0e5 * (result.k_mean - 1.0);
    println!("\n  k_eff = {:.5} +/- {:.5}   ({:+.0} pcm vs ICSBEP 1.0000 +/- 0.0010)",
        result.k_mean, result.k_std, pcm);
    println!(
        "  SINGLE SEED: seed-to-seed sd is ~{ENSEMBLE_SD_PCM:.0} pcm at these settings, so this\n  \
         is one draw from that spread ({:+.2} sd from the benchmark), NOT the code's answer.",
        pcm / ENSEMBLE_SD_PCM
    );

    // ── Timing breakdown ───────────────────────────────────────────────────
    let total = st.total();
    let tot_s = total.as_secs_f64();
    println!("\n  stage                     time [s]     % of accounted");
    println!("  ------------------------------------------------------");
    for (label, d) in st.rows() {
        println!(
            "  {label:<24} {:>8.2}     {:>6.2} %",
            d.as_secs_f64(),
            100.0 * d.as_secs_f64() / tot_s.max(1e-12)
        );
    }
    println!("  ------------------------------------------------------");
    println!("  accounted                {tot_s:>8.2}     100.00 %");
    println!(
        "  wall clock               {:>8.2}     (unaccounted {:.2} s)",
        wall.elapsed().as_secs_f64(),
        wall.elapsed().as_secs_f64() - tot_s
    );

    let data_s = tot_s - st.transport.as_secs_f64();
    println!(
        "\n  data preparation {:.2} s ({:.1} %) vs transport {:.2} s ({:.1} %)",
        data_s,
        100.0 * data_s / tot_s,
        st.transport.as_secs_f64(),
        100.0 * st.transport.as_secs_f64() / tot_s
    );
    println!(
        "  ACE reuse would skip {:.2} s of that ({:.1} % of the run): parse + RECONR + BROADR\n  \
         + ACER build + write. Reading back costs {:.2} s.",
        (st.endf_parse + st.reconr + st.broadr + st.ace_build + st.ace_write).as_secs_f64(),
        100.0 * (st.endf_parse + st.reconr + st.broadr + st.ace_build + st.ace_write).as_secs_f64()
            / tot_s,
        (st.ace_read + st.from_ace).as_secs_f64()
    );
    println!("  ACE written: {:.1} MB total (all removed after reading)", ace_bytes_total as f64 / 1.0e6);
    let _ = std::fs::remove_dir_all(&scratch);
}
