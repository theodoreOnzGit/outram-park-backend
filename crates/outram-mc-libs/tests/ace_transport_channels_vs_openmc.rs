// SPDX-License-Identifier: GPL-3.0

//! **The ACE route transports the channels OpenMC transports — GitHub #366.**
//!
//! # What was wrong
//!
//! 1. `Nuclide::from_ace` built its transport channels from
//!    `CeNeutronAce::channel_mts`, the rule for reconstructing the *total*,
//!    which keeps a lump and drops its components. U-235's ACE table lists
//!    MT=4 (`LQR = 0`, no law of its own) beside MT=51..91, so the 40 discrete
//!    levels and the MT=91 continuum were dropped and every inelastic
//!    collision was sampled from MT=4 with **Q = 0** — no energy loss. The
//!    five-route ICSBEP campaign measured it as **+1853 pcm on Godiva** against
//!    OpenMC on the same tables.
//! 2. U-234's ACE table carries fission only as the partials MT=19/20/21/38
//!    (no MT=18). The pointwise tier reads fission from MT=18, so U-234 could
//!    not fission on the ACE route (0 b at 1 MeV; 1.09 b on the ENDF route).
//!    And the fission spectrum was the *first* partial's law (first-chance)
//!    rather than the cross-section-weighted mixture OpenMC samples.
//!
//! # Methodology
//!
//! Reference: OpenMC's own reader of the **same** `.ace` files, via
//! `verification_and_validation/ace_route_physics/openmc_inputs/transport_channels_reference.py`
//! (OpenMC 0.16.1.dev25, `d7d3284a1`) into
//! `verification_and_validation/ace_route_physics/data/transport_channels_openmc.csv`.
//! Libraries: the NJOY2016 tables in the `reference-data/ace` submodule
//! (`reference`), and the five-route study's NJOY2016 (`njoy`) and Rust-NJOY
//! (`rust`) tables under `target/five_route_keff/` (regenerable; skipped when
//! absent).
//!
//! 1. **Channel list.** `transport_channel_mts()` must equal OpenMC's
//!    non-redundant reaction list (`openmc/data/neutron.py:634-640`) for
//!    U-234/235/238, except for one documented difference: for the absorption
//!    families this crate keeps the lump (MT=103 over 600..649, MT=107 over
//!    800..849) where OpenMC keeps the levels, because nothing distinguishes
//!    them in transport. The comparison maps OpenMC's levels to the lump, and
//!    nothing else.
//! 2. **Levels reach transport.** `Nuclide::inelastic_levels_table()` on U-235
//!    lists MT=51..89 and 91 with each level's ACE `Q`, and no MT=4.
//! 3. **Fission cross section.** `xs_at_energy(E).fission` must equal OpenMC's
//!    transported fission cross section (MT=18, or the sum of the partials) at
//!    six energies from thermal to 14 MeV, to `1e-10` relative — both are
//!    lin-lin interpolation of the same grid values, so anything larger is a
//!    decode or wiring error, not round-off.
//! 4. **U-234 fission spectrum.** The mean of `N = 400 000` sampled prompt
//!    fission-neutron energies at 1, 7, 14 and 19 MeV must match the exact mean
//!    of OpenMC's sampling scheme (partial picked by `sigma_k / sigma_f`, then
//!    its law) within **5 sigma** of the sample mean. The same reference file
//!    carries the MT=19-only mean; at 7, 14 and 19 MeV it differs from the
//!    mixture by far more than 5 sigma, so this check would fail the
//!    first-law-wins behaviour it replaces (asserted, not assumed).
//!
//! # Results (2026-09-29) — recorded in
//! `verification_and_validation/ace_route_physics/ace_transport_channels_2026-09-29.md`
//!
//! Channel lists equal on all nine (library, nuclide) pairs; fission cross
//! sections equal to `<1e-12`; U-235 carries 40 levels + MT=91 and no MT=4.
//! U-234 chi mean vs OpenMC's mixture: z = -0.40 / -0.46 / +0.91 / -0.07 at
//! 1 / 7 / 14 / 19 MeV, while the old first-law (MT=19-only) mean sits at
//! z = -95.8 / -72.2 / -44.9 from the samples at 7 / 14 / 19 MeV. Reverting the
//! source change fails tests 2-4 (run and reverted). k_eff after the fix:
//! Godiva route 3 -54 +/- 40 pcm vs OpenMC (was +1853), Jemima +124 +/- 38 (was
//! -923; equal to the ENDF route's residual).

use njoy_outram_park_fork::acer::ce_decode::decode_ce;
use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::reference_data::ace_reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;
use std::collections::BTreeSet;
use std::path::PathBuf;

const TEMP_K: f64 = 293.6;

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `.ace` for `(lib, nuclide)`, or `None` (printed) when it is absent.
fn table_path(lib: &str, nuclide: &str) -> Option<PathBuf> {
    match lib {
        "reference" => ace_reference_file_or_skip(
            &format!("reference-njoy/endf-b-viii.0/293.6K/{nuclide}.ace.gz"),
            &format!("transport channels/{nuclide}"),
        ),
        _ => {
            let p = workspace().join(format!("target/five_route_keff/{lib}/293.6K/{nuclide}.ace"));
            if p.is_file() {
                Some(p)
            } else {
                println!("{} absent: skipping", p.display());
                None
            }
        }
    }
}

fn reference_rows() -> Vec<Vec<String>> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/ace_route_physics/data/transport_channels_openmc.csv");
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split(',').map(str::to_string).collect())
        .collect()
}

#[test]
fn transport_channels_match_openmc_redundancy_rule() {
    let mut compared = 0;
    for row in reference_rows().iter().filter(|r| r[0] == "channels") {
        let (lib, nuc) = (row[1].as_str(), row[2].as_str());
        let Some(path) = table_path(lib, nuc) else {
            continue;
        };
        let raw = read::read(&path).expect("read ACE");
        let ace = decode_ce(&raw).expect("decode");
        let has = |mt: i32| ace.reactions.iter().any(|r| r.mt == mt);
        let openmc: BTreeSet<i32> = row[3]
            .split_whitespace()
            .map(|m| m.parse::<i32>().unwrap())
            // The one documented difference: the absorption lump, not its levels.
            .map(|m| match m {
                600..=649 if has(103) => 103,
                800..=849 if has(107) => 107,
                _ => m,
            })
            .collect();
        let ours: BTreeSet<i32> = ace.transport_channel_mts().into_iter().collect();
        assert_eq!(
            ours,
            openmc,
            "{lib}/{nuc}: ours-only {:?}, OpenMC-only {:?}",
            ours.difference(&openmc).collect::<Vec<_>>(),
            openmc.difference(&ours).collect::<Vec<_>>()
        );
        // The defect itself, stated directly: never MT=4 beside its levels.
        assert!(!(ours.contains(&4) && ours.iter().any(|m| (51..=91).contains(m))));
        println!(
            "{lib}/{nuc}: {} transport channels, equal to OpenMC's",
            ours.len()
        );
        compared += 1;
    }
    assert!(
        compared >= 3,
        "the reference-data submodule tables must be compared"
    );
}

#[test]
fn u235_levels_reach_transport_with_their_q() {
    for lib in ["reference", "njoy", "rust"] {
        let Some(path) = table_path(lib, "U235") else {
            continue;
        };
        let raw = read::read(&path).expect("read");
        let ace = decode_ce(&raw).expect("decode");
        let nuc = Nuclide::from_ace(&raw, "U235").expect("from_ace");
        let levels = nuc.inelastic_levels_table();
        let mts: Vec<i32> = levels.iter().map(|l| l.0 as i32).collect();
        assert!(
            !mts.contains(&4),
            "{lib}: MT=4 (Q = 0) must not be a transport channel"
        );
        let want: Vec<i32> = ace
            .reactions
            .iter()
            .map(|r| r.mt)
            .filter(|m| (51..=91).contains(m))
            .collect();
        assert_eq!(mts, want, "{lib}: every discrete level and MT=91");
        assert_eq!(
            want.len(),
            40,
            "{lib}: U-235 ENDF/B-VIII.0 has 39 levels + MT=91"
        );
        for (mt, q, _) in &levels {
            let rx = ace.reactions.iter().find(|r| r.mt == *mt as i32).unwrap();
            assert_eq!(*q, rx.q_value, "{lib}: MT={mt} carries its own Q");
        }
        let (mt51, q51, _) = levels[0];
        println!(
            "{lib}: {} inelastic channels, MT={mt51} Q = {q51:.1} eV, no MT=4",
            levels.len()
        );
    }
}

#[test]
fn fission_cross_section_matches_openmc_including_partial_only_u234() {
    let mut n = 0;
    for row in reference_rows().iter().filter(|r| r[0] == "fission") {
        let (lib, nuc) = (row[1].as_str(), row[2].as_str());
        let e: f64 = row[3].parse().unwrap();
        let want: f64 = row[4].parse().unwrap();
        let Some(path) = table_path(lib, nuc) else {
            continue;
        };
        let raw = read::read(&path).expect("read");
        let nucl = Nuclide::from_ace(&raw, nuc).expect("from_ace");
        let got = nucl.xs_at_energy(e, TEMP_K).fission;
        let rel = (got - want).abs() / want.abs().max(1e-300);
        if e == 1.0e6 {
            println!("{lib}/{nuc} sigma_f(1 MeV) = {got:.7} b (OpenMC {want:.7}), rel {rel:.1e}");
        }
        assert!(
            rel < 1e-10,
            "{lib}/{nuc} E = {e}: sigma_f {got} vs OpenMC {want} (rel {rel:.2e})"
        );
        n += 1;
    }
    assert!(n >= 18);
}

#[test]
fn u234_fission_spectrum_is_the_partial_mixture() {
    const N: usize = 400_000;
    for row in reference_rows().iter().filter(|r| r[0] == "chimean") {
        let lib = row[1].as_str();
        let e: f64 = row[3].parse().unwrap();
        let mix: f64 = row[4].parse().unwrap();
        let first_only: f64 = row[5].parse().unwrap();
        let Some(path) = table_path(lib, "U234") else {
            continue;
        };
        let raw = read::read(&path).expect("read");
        let nuc = Nuclide::from_ace(&raw, "U234").expect("from_ace");
        let mut seed = 0x234_0000_u64 + e as u64;
        let xs: Vec<f64> = (0..N)
            .map(|_| nuc.sample_fission_energy(e, &mut seed))
            .collect();
        let mean = xs.iter().sum::<f64>() / N as f64;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (N - 1) as f64;
        let sigma = (var / N as f64).sqrt();
        let z = (mean - mix) / sigma;
        println!(
            "{lib} U234 E = {:5.1} MeV: <E'> = {:.5} MeV, OpenMC mixture {:.5}, MT=19-only {:.5}, \
             sigma {:.1e} MeV, z = {z:+.2}, first-law z = {:+.1}",
            e / 1e6,
            mean / 1e6,
            mix / 1e6,
            first_only / 1e6,
            sigma / 1e6,
            (mean - first_only) / sigma
        );
        assert!(
            z.abs() < 5.0,
            "{lib} E = {e}: mean {mean} vs OpenMC {mix}, z = {z}"
        );
        // Power: wherever the second chance is open, first-law-wins is far off.
        if e >= 7.0e6 {
            assert!(
                ((first_only - mix) / sigma).abs() > 20.0,
                "{lib} E = {e}: the check could not tell the mixture from MT=19 alone"
            );
        }
    }
}
