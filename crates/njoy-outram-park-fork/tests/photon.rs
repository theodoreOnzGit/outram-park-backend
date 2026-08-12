//! Photon-production (energy-balance) integration tests against real ENDF data.
//!
//! Run with:
//! ```bash
//! cargo test -p njoy-outram-park-fork --test photon --release
//! ```

use std::fs::File;

use njoy_outram_park_fork::{
    endf::tape::Tape,
    photon::PhotonProduction,
    reconr::{reconr, ReconrConfig},
};

fn fixture(name: &str) -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources");
    p.push(name);
    p
}

/// **H6 fission-photon V&V.**
///
/// **Methodology.** U-235 (MAT=9228, ENDF/B-VIII.0) carries its prompt fission
/// photons as MF=12/MT=18 (LO=1 multiplicity `y(E)` ≈ 8.6 photons) with the
/// continuous photon spectrum in MF=15/MT=18. The photon energy production is
/// `E_γ,prod(E) = σ_f(E)·y(E)·Ē_γ(E)`, where `Ē_γ` is the first moment of the
/// MF=15 spectrum. Assert (a) it is non-zero (the parser found the data), and
/// (b) the per-fission photon energy `E_γ,prod/σ_f` is ~5–12 MeV — U-235 emits
/// ~6.6 prompt photons averaging ~0.9 MeV ≈ 6–8 MeV/fission, and the ~8.6-photon
/// multiplicity here (including lower-energy lines) lands in that band.
///
/// **Results (2026-07-04, ENDF/B-VIII.0).** Non-zero; per-fission photon energy
/// ≈ 7 MeV (within [5, 12] MeV) at both thermal and fast incident energies.
#[test]
fn u235_fission_photon_energy_is_physical() {
    let tape = Tape::read(File::open(fixture("n-092_U_235-ENDF8.0.endf")).unwrap()).unwrap();
    let cfg = ReconrConfig {
        mat: 9228,
        tolerance: 0.001,
        temperature: 0.0,
    };
    let recon = reconr(&tape, &cfg).unwrap();
    let pp = PhotonProduction::from_endf(&tape, 9228, &recon);

    assert!(!pp.is_empty(), "U-235 has MF=12/MT=18 fission photons");

    // Fission cross section for normalising to per-fission photon energy.
    let sigma_f = |e: f64| {
        recon
            .sections
            .iter()
            .find(|s| i32::from(s.mt) == 18)
            .map(|s| njoy_outram_park_fork::reconr::eval_lin_lin(&s.pairs, e))
            .unwrap_or(0.0)
    };

    for &e in &[0.0253, 1.0e6, 2.0e6] {
        let e_prod = pp.eval(e, &recon); // eV·barn
        let sf = sigma_f(e);
        assert!(sf > 0.0, "fission xs positive at {e} eV");
        let per_fission = e_prod / sf; // eV of photon energy per fission
        assert!(
            (5.0e6..12.0e6).contains(&per_fission),
            "per-fission photon energy {per_fission} eV at {e} eV should be ~7 MeV"
        );
    }
}
