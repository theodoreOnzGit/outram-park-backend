use super::*;
use crate::endf::tape::Tape;
use std::fs::File;

fn u235_fission_chi() -> Law4 {
    let p = crate::reference_data::reference_endf_dir().join("n-092_U_235-ENDF8.0.endf");
    let tape = Tape::read(File::open(p).unwrap()).unwrap();
    let sec = tape.section(9228, 5, 18).expect("U-235 MF=5/MT=18");
    parse_mf5_law4(sec).unwrap()
}

#[test]
fn fission_chi_parses_with_many_incident_energies() {
    let law = u235_fission_chi();
    assert!(law.incident.len() >= 10, "expected the χ E_in grid");
    // Incident energies ascending, in MeV (top ~30 MeV).
    let e: Vec<f64> = law.incident.iter().map(|d| d.e_in_mev).collect();
    assert!(e.windows(2).all(|w| w[1] >= w[0]), "E_in ascending");
    assert!(*e.last().unwrap() < 1000.0, "E_in in MeV not eV");
}

#[test]
fn fission_chi_distributions_are_valid() {
    let law = u235_fission_chi();
    for d in &law.incident {
        assert!(
            d.e_out_mev.windows(2).all(|w| w[1] >= w[0]),
            "E_out ascending"
        );
        assert!(d.pdf.iter().all(|&p| p >= 0.0), "pdf non-negative");
        assert!((d.cdf[0]).abs() < 1e-9, "cdf starts at 0");
        assert!((d.cdf.last().unwrap() - 1.0).abs() < 1e-6, "cdf ends at 1");
        assert!(
            d.cdf.windows(2).all(|w| w[1] >= w[0] - 1e-9),
            "cdf monotone"
        );
    }
}

fn u235_mf6(mt: i32) -> Mf6Neutron {
    let p = crate::reference_data::reference_endf_dir().join("n-092_U_235-ENDF8.0.endf");
    let tape = Tape::read(File::open(p).unwrap()).unwrap();
    let sec = tape.section(9228, 6, mt).expect("U-235 MF=6");
    parse_mf6_law1_neutron(sec).unwrap()
}

#[test]
fn mf6_n2n_yield_and_frame() {
    // MT=16 (n,2n): neutron multiplicity 2, centre-of-mass frame (LCT=2).
    let n2n = u235_mf6(16);
    assert_eq!(n2n.lct, 2, "LCT=2 (CM)");
    assert!(
        n2n.yield_pairs.iter().all(|&(_, y)| (y - 2.0).abs() < 1e-6),
        "(n,2n) yield is 2"
    );
    assert!(n2n.law4.incident.len() > 5, "MT16 has an E_in grid");
}

#[test]
fn mf6_energy_distributions_are_valid() {
    // Both MT16 (n,2n) and MT91 (continuum inelastic) energy spectra.
    for mt in [16, 91] {
        let d = u235_mf6(mt);
        assert!(d.yield_pairs.iter().all(|&(_, y)| y >= 1.0), "yield ≥ 1");
        for dist in &d.law4.incident {
            assert!(
                dist.e_out_mev.windows(2).all(|w| w[1] >= w[0]),
                "E_out ascending"
            );
            assert!(dist.pdf.iter().all(|&p| p >= 0.0), "pdf ≥ 0");
            assert!((dist.cdf[0]).abs() < 1e-9, "cdf starts at 0");
            assert!(
                (dist.cdf.last().unwrap() - 1.0).abs() < 1e-6,
                "cdf ends at 1"
            );
            assert!(
                dist.cdf.windows(2).all(|w| w[1] >= w[0] - 1e-9),
                "cdf monotone"
            );
        }
    }
}

/// One incident-energy table with a flat outgoing pdf `0.5 /MeV` on
/// `[0, 2] MeV` has mean `∫E'·pdf dE' = 1.0 MeV`. `mean_energy` (in eV) must
/// reproduce it — the exact first-moment check for the MF=6 lab-frame `ebar`.
#[test]
fn mf6_mean_energy_exact_flat_pdf() {
    let table = OutgoingEnergy {
        e_in_mev: 1.0,
        intt: 2, // lin-lin
        e_out_mev: vec![0.0, 2.0],
        pdf: vec![0.5, 0.5],
        cdf: vec![0.0, 1.0],
    };
    let m = Mf6Neutron {
        lct: 1,
        yield_pairs: vec![(0.0, 2.0)],
        law4: Law4 {
            e_in_interp: vec![],
            incident: vec![table],
        },
        lang: Mf6AngularLaw::Legendre,
        angular: vec![],
        za_target: 92238,
    };
    // Single table ⇒ same mean at any incident energy.
    assert!(
        (m.mean_energy(5.0e6) - 1.0e6).abs() < 1.0,
        "got {}",
        m.mean_energy(5.0e6)
    );
}

/// Between two incident-energy tables the mean is linearly interpolated:
/// table at 1 MeV has mean 1.0 MeV, table at 3 MeV has mean 2.0 MeV, so at
/// 2 MeV the mean must be 1.5 MeV.
#[test]
fn mf6_mean_energy_interpolates_in_incident() {
    let t = |e_in: f64, e_max: f64| OutgoingEnergy {
        e_in_mev: e_in,
        intt: 2,
        e_out_mev: vec![0.0, e_max],
        pdf: vec![1.0 / e_max, 1.0 / e_max], // flat, ∫=1, mean = e_max/2
        cdf: vec![0.0, 1.0],
    };
    let m = Mf6Neutron {
        lct: 1,
        yield_pairs: vec![(0.0, 2.0)],
        law4: Law4 {
            e_in_interp: vec![],
            incident: vec![t(1.0, 2.0), t(3.0, 4.0)],
        },
        lang: Mf6AngularLaw::Legendre,
        angular: vec![],
        za_target: 92238,
    };
    // mean(1 MeV)=1.0, mean(3 MeV)=2.0 ⇒ mean(2 MeV)=1.5 MeV.
    assert!(
        (m.mean_energy(2.0e6) - 1.5e6).abs() < 1.0,
        "got {}",
        m.mean_energy(2.0e6)
    );
}

/// Real-data sanity: the U-235 (n,2n) MF=6 mean emitted-neutron energy at a
/// high incident energy is a physically reasonable few MeV — positive, and
/// well below the incident energy (the two neutrons share `E + Q`, `Q < 0`).
#[test]
fn mf6_n2n_mean_energy_is_physical() {
    let n2n = u235_mf6(16);
    // MT=16 threshold is ~5–6 MeV; sample at 14 MeV.
    let ebar = n2n.mean_energy(1.4e7);
    assert!(
        ebar > 0.0,
        "mean emitted energy must be positive, got {ebar}"
    );
    assert!(
        ebar < 1.4e7,
        "mean emitted energy {ebar} must be < incident 14 MeV"
    );
    assert!(
        (0.2e6..8.0e6).contains(&ebar),
        "U-235 (n,2n) ⟨E'⟩ {ebar} eV should be ~1–5 MeV"
    );
}

#[test]
fn fission_spectrum_peaks_near_1_mev() {
    // The U-235 prompt fission spectrum peaks around 0.7–1 MeV. Take a
    // thermal incident energy and find the mode of the outgoing pdf.
    let law = u235_fission_chi();
    let d = &law.incident[0]; // lowest incident energy
    let (mut mode_e, mut mode_p) = (0.0, 0.0);
    for (&e, &p) in d.e_out_mev.iter().zip(&d.pdf) {
        if p > mode_p {
            mode_p = p;
            mode_e = e;
        }
    }
    assert!(
        (0.2..=2.0).contains(&mode_e),
        "fission χ should peak near ~1 MeV, got {mode_e} MeV"
    );
}
