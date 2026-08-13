//! Notebook: **`nuclear-data`** — verification tests (op-6tz.6).
//!
//! Notebook provenance: openmc-notebooks `nuclear-data.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! The OpenMC notebook loads a continuous-energy nuclide (Gd-157 from ACE),
//! reads cross sections by reaction MT at given energies and temperatures,
//! inspects a reaction threshold, and lists secondary distributions. njoy has no
//! ACE *reader* (it is an ACE *writer*), so these tests reproduce the same **data
//! operations** from the njoy production path instead: RECONR reconstruction of
//! an ENDF evaluation, BROADR/analytic Doppler broadening, and the ν̄ / χ
//! secondary parsers. Each is judged against a physical reference (accepted
//! U-235 thermal cross sections, monotonic Doppler behaviour, physical fission
//! ν̄ and mean outgoing energy) — the same bar as the crate's `reconr_*`/`wmp_*`
//! suites.
//!
//! # Results (2026-07-15, ENDF/B-VIII.0 fixtures)
//!
//! - U-235 reconstructed σ at 0.0253 eV: σ_f ≈ 585 b, σ_γ ≈ 99 b, σ_el ≈ 15 b,
//!   σ_t ≈ 699 b (matches accepted 2200 m/s values to a few percent).
//! - U-235 ν̄(0.0253 eV) ≈ 2.44 (MF=1/452), MF=5/18 χ mean outgoing energy ≈ 2 MeV.
//! - WMP U-238 6.67 eV capture peak falls monotonically with temperature
//!   (0 K → 294 K → 1000 K) — the notebook's temperature-dependent-σ operation.
//!
//! # Gaps (see `docs/openmc-notebooks-data-verification.md`)
//!
//! - No ACE/HDF5 *reader* (`IncidentNeutron.from_ace`).
//! - No atomic-data helpers (`atomic_mass`, `NATURAL_ABUNDANCE`, `atomic_weight`).

use std::fs::File;
use std::path::PathBuf;

use njoy_outram_park_fork::{
    endf::tape::Tape,
    interface::NuclearDataLibrary,
    nuclear_data::secondary::{FissionSpectrum, NuBar},
    wmp::WmpLibrary,
    MtReaction,
};
use uom::si::{area::barn, energy::electronvolt, f64::Energy};

const U235_MAT: i32 = 9228;

fn resource(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources");
    p.push(name);
    p
}

fn u235_path() -> PathBuf {
    resource("n-092_U_235-ENDF8.0.endf")
}

fn ev(e: f64) -> Energy {
    Energy::new::<electronvolt>(e)
}

/// Notebook op: `nuc[MT].xs[T](E)` — read a cross section by reaction MT at an
/// energy. njoy reconstructs U-235 from ENDF and reads the four principal
/// channels at the 2200 m/s point; the accepted thermal values anchor the check.
#[test]
fn read_cross_sections_by_mt() {
    let lib = NuclearDataLibrary::from_file(u235_path(), U235_MAT)
        .expect("load U-235 ENDF")
        .reconstruct(0.001)
        .expect("RECONR U-235");

    let e = ev(0.0253);
    let fis = lib.fission_xs(e).get::<barn>();
    let cap = lib.capture_xs(e).get::<barn>();
    let el = lib.elastic_xs(e).get::<barn>();
    let tot = lib.total_xs(e).get::<barn>();

    println!("U-235 @ 0.0253 eV: σ_f={fis:.1} σ_γ={cap:.1} σ_el={el:.1} σ_t={tot:.1} b");
    assert!(
        (fis - 585.0).abs() < 25.0,
        "σ_f = {fis:.1} b, expected ≈585"
    );
    assert!((cap - 99.0).abs() < 12.0, "σ_γ = {cap:.1} b, expected ≈99");
    assert!((el - 15.0).abs() < 4.0, "σ_el = {el:.1} b, expected ≈15");
    assert!(
        (tot - 699.0).abs() < 30.0,
        "σ_t = {tot:.1} b, expected ≈699"
    );
}

/// Notebook op: evaluating σ over an *array* of energies. Mirrors the notebook's
/// `gd157[1].xs['294K']([1.0, 2.0, 3.0])`. Every value must be finite and
/// non-negative, and thermal fission must dominate the fast-region point.
#[test]
fn cross_sections_over_energy_array() {
    let lib = NuclearDataLibrary::from_file(u235_path(), U235_MAT)
        .expect("load U-235 ENDF")
        .reconstruct(0.001)
        .expect("RECONR U-235");

    let energies = [0.0253_f64, 1.0, 10.0, 1.0e3];
    let fis: Vec<f64> = energies
        .iter()
        .map(|&e| {
            lib.xs_for_reaction(MtReaction::Mt18Fission, ev(e))
                .get::<barn>()
        })
        .collect();
    println!("U-235 σ_f over {energies:?} eV = {fis:?} b");
    for (e, s) in energies.iter().zip(&fis) {
        assert!(s.is_finite() && *s >= 0.0, "σ_f({e} eV) = {s} invalid");
    }
    // The 1/v thermal fission (~585 b) dwarfs the ~keV smooth region.
    assert!(
        fis[0] > fis[3],
        "thermal fission {} should exceed keV {}",
        fis[0],
        fis[3]
    );
}

/// Notebook op: `nuc[MT].xs['294K']` — temperature-dependent cross section.
/// njoy's analytic-Doppler WMP path evaluates U-238's 6.67 eV capture resonance
/// at three temperatures; Doppler broadening lowers the peak monotonically with
/// temperature. Uses the embedded CORE WMP library (no external file); skips if
/// U-238 is not embedded in this build's CORE set.
#[test]
fn temperature_dependent_xs_doppler() {
    let wmp = match WmpLibrary::core().get("U238") {
        Ok(w) => w,
        Err(_) => {
            eprintln!("SKIP: U238 not in embedded CORE WMP set");
            return;
        }
    };

    let peak = |t: f64| -> f64 {
        let mut best = 0.0_f64;
        let mut e = 6.0;
        while e < 7.4 {
            best = best.max(wmp.evaluate(e, t).capture());
            e += 0.002;
        }
        best
    };
    let (p0, p294, p1000) = (peak(0.0), peak(293.6), peak(1000.0));
    println!("U-238 6.67 eV capture peak: 0K={p0:.0} 294K={p294:.0} 1000K={p1000:.0} b");

    assert!(p0 > 1000.0, "0 K peak {p0:.0} b should be a tall resonance");
    assert!(p294 < p0, "294 K peak {p294:.0} !< 0 K {p0:.0} (Doppler)");
    assert!(
        p1000 < p294,
        "1000 K peak {p1000:.0} !< 294 K {p294:.0} (Doppler)"
    );
}

/// Notebook op: secondary fission neutron data — ν̄(E) and the χ energy
/// distribution. njoy parses ν̄ from ENDF MF=1/452 and χ from MF=5/MT=18. The
/// U-235 thermal ν̄ ≈ 2.44 and the fission-spectrum mean outgoing energy ≈ 2 MeV
/// are the physical anchors.
#[test]
fn secondary_nubar_and_fission_spectrum() {
    let tape = Tape::read(File::open(u235_path()).expect("open U-235")).expect("parse U-235 tape");

    let nu = NuBar::from_endf(&tape, U235_MAT)
        .expect("MF=1/452 parse")
        .expect("U-235 has ν̄");
    let nu_thermal = nu.at(0.0253);
    println!("U-235 ν̄(0.0253 eV) = {nu_thermal:.4}");
    assert!(
        (nu_thermal - 2.44).abs() < 0.15,
        "ν̄ = {nu_thermal:.3}, expected ≈2.44"
    );

    let chi = FissionSpectrum::from_endf_mf5(&tape, U235_MAT)
        .expect("MF=5/18 parse")
        .expect("U-235 has an MF=5/MT=18 fission spectrum");
    let mean = chi.mean_energy(1.0e6);
    println!(
        "U-235 χ mean outgoing energy (E_in=1 MeV) = {:.3} MeV",
        mean / 1.0e6
    );
    assert!(
        (0.5e6..5.0e6).contains(&mean),
        "χ mean outgoing energy {mean:.0} eV outside the physical fission range"
    );
}
