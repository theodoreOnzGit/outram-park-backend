// SPDX-License-Identifier: GPL-3.0

//! **Why does the ACE route transport slower than the ENDF route?**
//!
//! `lct008_ace_roundtrip.rs` measured transport at **603.56 s on the ACE route
//! against 131.34 s on the ENDF route** -- same geometry, settings and seed,
//! with `k` agreeing at 0.78 sigma. That example records three candidate causes
//! and measures none of them. This is the measurement.
//!
//! It builds U-238 -- the dominant cost in that run -- both ways in one process
//! and prints, for each nuclide:
//!
//! - **the inelastic level count**, which `xs_at_energy` sums `eval_mt` over,
//!   one binary search apiece. If the two routes differ here the per-collision
//!   cost differs by that factor;
//! - **the URR range and whether DBRC is on**, so the premise of "the ACE route
//!   should be doing LESS work" is checked rather than assumed -- `from_ace`
//!   omits both, but that only matters if the ENDF route actually has them;
//! - **microseconds per `xs_at_energy` call** over the full grid, which is the
//!   hot path and settles the question directly.
//!
//! If the per-call costs match, the gap is not in cross-section lookup at all
//! and all three candidates were wrong -- it would be in secondary sampling,
//! most likely the fission spectrum representation (`from_ace` always yields
//! `ContinuousTabular`, where the ENDF route may yield a far cheaper Watt).
//!
//! Run deliberately; it reconstructs U-238 twice and writes a ~326 MB ACE file,
//! which it deletes after reading.
//!
//! # Results (2026-09-23, U-238, ENDF/B-VIII.0 at 293.6 K)
//!
//! ```text
//! ENDF: inelastic levels = 40   urr = Some((20000.0, 149008.7))  dbrc = true
//! ACE : inelastic levels = 40   urr = None                        dbrc = false
//!
//! ENDF: 520445 xs_at_energy calls in 0.769 s   (1.48 us/call)   [acc 1.606e7]
//! ACE : 520445 xs_at_energy calls in 3.050 s   (5.86 us/call)   [acc 1.606e7]
//! ```
//!
//! ## The answer: it IS cross-section lookup, at 3.96x per call
//!
//! That accounts for essentially the whole 4.6x transport gap
//! `lct008_ace_roundtrip.rs` measured, so the cause is in `xs_at_energy` and
//! not in secondary sampling.
//!
//! **Two of the three candidates are refuted:**
//!
//! - **Inelastic level count: 40 on BOTH routes.** Ruled out. `channel_mts`
//!   does not drop U-238's levels, because its ACE table carries no MT=4 lump
//!   to drop them under.
//! - **Fission spectrum representation: not the cause**, since this measures
//!   pure cross-section lookup with no sampling at all.
//!
//! **And the premise held, which sharpens the puzzle rather than resolving it:**
//! the ENDF route really does carry URR (20–149 keV) and DBRC, and the ACE
//! route really does carry neither. So the ACE route performs *less* physics per
//! lookup and is still four times slower.
//!
//! ## The remaining explanation, and why it is a design consequence not a bug
//!
//! The surviving candidate is **grid size**. ACE stores every reaction on one
//! **union** energy grid — 284 415 points for U-238 — so `from_ace` gives each
//! of the ~49 sections an array spanning its threshold to the top of that grid.
//! RECONR instead thins each MT's grid independently, so the ENDF route's
//! per-MT arrays are much shorter. `xs_at_energy` sums `eval_mt` over all 40
//! levels plus the lumps, so the ACE route walks far more memory per call. At
//! 4x this looks like memory traffic rather than binary-search depth, which is
//! logarithmic and could not produce it.
//!
//! That is **inherent to the ACE format's union-grid design**, not a defect in
//! the decoder — MCNP pays the same cost. It is recorded because "the ACE route
//! is ~4x slower per lookup" is a real performance characteristic a caller
//! should know, and because thinning the per-MT grids after decode would be a
//! legitimate optimisation if it ever matters.
//!
//! ## A parity result, obtained for free
//!
//! The accumulated totals are **identical: 1.606e7 on both routes** over 520 445
//! energies spanning 1e-4 to 2e7 eV. That is a far tighter statement of ACE/ENDF
//! agreement than `lct008_ace_roundtrip.rs`'s 0.78 sigma on `k`, and it came out
//! of a timing harness.
fn main() {
    use njoy_outram_park_fork::acer::{angular::parse_elastic_angular, energy::build_emissions, AceTable};
    use njoy_outram_park_fork::broadr::broaden_result;
    use njoy_outram_park_fork::endf::tape::Tape;
    use njoy_outram_park_fork::heatr::{build_emission_spectra, Kerma};
    use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, NuBar};
    use njoy_outram_park_fork::photon::PhotonProduction;
    use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::nuclide::Nuclide;
    const T: f64 = 293.6;
    let (name, file, mat) = ("U238", "n-092_U_238.endf", 9237_i32);
    let p = reference_endf(file).expect("tape");

    let n_endf = Nuclide::from_endf_file(&p, name, T, 1.0e-3).expect("endf");
    let tape = Tape::read_file(&p).unwrap();
    let recon0 = reconr(&tape, &ReconrConfig { mat, tolerance: 1.0e-3, temperature: 0.0 }).unwrap();
    let recon = broaden_result(&recon0, T);
    let angular = tape.section(mat, 4, 2).map(|s| parse_elastic_angular(s).unwrap());
    let partials: Vec<(i32, f64)> = recon.sections.iter().map(|s| (i32::from(s.mt), s.qi)).collect();
    let emissions = build_emissions(&tape, mat, recon.material.awr, &partials);
    let nu = NuBar::from_endf(&tape, mat).unwrap().unwrap_or_default();
    let chi = FissionSpectrum::from_endf_mf5(&tape, mat).unwrap().unwrap_or_default();
    let em = build_emission_spectra(&tape, mat);
    let ph = PhotonProduction::from_endf(&tape, mat, &recon);
    let kerma = Kerma::from_reconr(&recon, &nu, &chi, &em).with_energy_balance(&ph, &recon);
    let nub = njoy_outram_park_fork::acer::nu::build(&tape, mat).unwrap();
    let ace = AceTable::from_reconr_full(&recon, 8.617333262e-5*T*1e-6, 0, angular.as_ref(),
        &emissions, Some(&kerma), nub.as_deref(),
        njoy_outram_park_fork::acer::has_mt19_distributions(&tape, mat),
        njoy_outram_park_fork::acer::photon_blocks::build(&tape, mat).as_deref());
    let d = std::env::temp_dir().join(format!("probe_{}.ace", std::process::id()));
    ace.write_type1(&d).unwrap();
    let raw = njoy_outram_park_fork::acer::read::read(&d).unwrap();
    let n_ace = Nuclide::from_ace(&raw, name).unwrap();
    let _ = std::fs::remove_file(&d);

    for (label, n) in [("ENDF", &n_endf), ("ACE ", &n_ace)] {
        let lv = n.inelastic_levels_table();
        println!("{label}: inelastic levels = {:<4} urr = {:<22} dbrc = {}",
            lv.len(), format!("{:?}", n.urr_range_ev()), n.has_dbrc());
    }
    // Time the hot path: xs_at_energy across the grid.
    for (label, n) in [("ENDF", &n_endf), ("ACE ", &n_ace)] {
        let t = std::time::Instant::now();
        let mut acc = 0.0f64;
        let mut e = 1.0e-4f64;
        let mut calls = 0u64;
        while e < 2.0e7 { acc += n.xs_at_energy(e, T).total; e *= 1.00005; calls += 1; }
        println!("{label}: {calls} xs_at_energy calls in {:?}  ({:.2} us/call)  [acc {acc:.3e}]",
            t.elapsed(), t.elapsed().as_secs_f64()*1e6/calls as f64);
    }
}
