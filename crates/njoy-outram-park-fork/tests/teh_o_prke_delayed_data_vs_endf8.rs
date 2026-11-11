//! `teh-o-prke`'s ENDF/B-VIII.0 six-group delayed-neutron constants, re-read
//! from the evaluations (gh:#1319).
//!
//! **Why this exists.** `teh-o-prke` is data-free: it carries the evaluated
//! delayed-neutron group data as cited constants
//! (`teh_o_prke::zero_power_prke::six_group_precursor_prke::six_group_constants::ENDF_B_VIII0_U235`
//! and `ENDF_B_VIII0_U238`). This test re-reads the ENDF/B-VIII.0 tapes
//! through this crate's own parsers and asserts every constant equals the
//! tape **bit for bit**, so the two cannot drift. `teh-o-prke` is a
//! dev-dependency here only (neither crate depends on the other in normal
//! builds, so the edge forms no cycle).
//!
//! # Methodology
//!
//! For each nuclide, read from the tape:
//!
//! - MF=1/MT=455 ([`DelayedNuBar`]): the `NNF` precursor decay constants
//!   `lambda_i` (`LDG=0`, energy-independent) and the delayed yield
//!   `nu_d(E)`;
//! - MF=5/MT=455 ([`DelayedChi`]): each group's abundance `p_i(E)` (the
//!   group's share of the delayed neutrons);
//! - MF=1/MT=452 ([`NuBar`]): the total yield `nu(E)`.
//!
//! At the stated incident energy `E` (U-235: 0.0253 eV, thermal; U-238:
//! 2 MeV, about the mean of the fission spectrum, since U-238 has no thermal
//! fission) the group delayed fractions are `beta_i = p_i nu_d(E) / nu(E)`.
//! Pass criterion: `lambda_i`, `p_i`, `nu_d(E)`, `nu(E)` and `E` equal the
//! teh-o-prke constants exactly (`f64::to_bits`), the group count is six on
//! the tape, `p_i` is energy-independent on the tape (so one value
//! represents it), and teh-o-prke's `beta_i` and `lambda_i` arrays equal the
//! ones computed here from the tape values.
//!
//! Tapes: `reference-data/endf/n-092_U_235-ENDF8.0.endf` (MAT 9228) and
//! `reference-data/endf/n-092_U_238.endf` (MAT 9237, header "ENDF/B-VIII.0
//! MATERIAL 9237"). Skipped with a note when a tape is absent
//! ([`reference_endf_or_skip`]).
//!
//! # Results (2026-11-10)
//!
//! Both nuclides: six groups on the tape, `p_i` flat over 1e-5 eV to 30 MeV,
//! every constant bit-identical.
//!
//! | | U-235 (E = 0.0253 eV) | U-238 (E = 2 MeV) |
//! |---|---|---|
//! | `lambda_i` [1/s] | 0.013336, 0.032739, 0.12078, 0.30278, 0.84949, 2.853 | 0.01363, 0.031334, 0.12334, 0.32373, 0.90597, 3.0487 |
//! | `p_i` | 0.03500755, 0.1806982, 0.1725102, 0.3867821, 0.1585752, 0.0664266 | 0.01393795, 0.1127969, 0.1310273, 0.3851429, 0.2539916, 0.1031033 |
//! | `nu_d(E)` | 0.01585000042993 | 0.044 |
//! | `nu(E)` | 2.42985 | 2.61936 |
//! | `beta` | 0.0065230 | 0.016798 |
//!
//! Run with
//! `cargo test --release -p njoy-outram-park-fork --test teh_o_prke_delayed_data_vs_endf8`.

use std::fs::File;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::nuclear_data::secondary::NuBar;
use njoy_outram_park_fork::nuclear_data::{DelayedChi, DelayedNuBar};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use teh_o_prke::zero_power_prke::six_group_precursor_prke::six_group_constants::{
    SixGroupDelayedNeutronData, ENDF_B_VIII0_U235, ENDF_B_VIII0_U238,
};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;

/// The tape values for one nuclide at one incident energy.
struct TapeValues {
    lambda: Vec<f64>,
    abundance: Vec<f64>,
    nu_delayed: f64,
    nu_total: f64,
}

/// Reads MF=1/455, MF=5/455 and MF=1/452 of `file` (material `mat`) at
/// incident energy `e_ev`; `None` when the tape is absent.
fn read_tape(file: &str, mat: i32, e_ev: f64) -> Option<TapeValues> {
    let path = reference_endf_or_skip(file, "teh-o-prke delayed data vs ENDF/B-VIII.0")?;
    let tape = Tape::read(File::open(path).expect("open tape")).expect("parse tape");
    let dnu = DelayedNuBar::from_endf(&tape, mat).unwrap().expect("MF=1/455");
    let chi = DelayedChi::from_endf(&tape, mat).unwrap().expect("MF=5/455");
    let nu = NuBar::from_endf(&tape, mat).unwrap().expect("MF=1/452");
    assert!(!dnu.ldg1_energy_dependent, "{file}: expected LDG=0");
    assert_eq!(dnu.n_groups(), 6, "{file}: NNF");
    assert_eq!(chi.n_groups(), 6, "{file}: NK");
    let mut abundance = Vec::new();
    for (i, g) in chi.groups.iter().enumerate() {
        let p0 = g.fraction[0].1;
        assert!(
            g.fraction.iter().all(|&(_, p)| p.to_bits() == p0.to_bits()),
            "{file}: group {} abundance is energy-dependent",
            i + 1
        );
        abundance.push(p0);
    }
    Some(TapeValues {
        lambda: dnu.lambdas().to_vec(),
        abundance,
        nu_delayed: dnu.nu_delayed_at(e_ev),
        nu_total: nu.at(e_ev),
    })
}

/// Asserts `data` equals the tape bit for bit, and that its `beta_i` and
/// `lambda_i` arrays are the ones the tape values give.
fn assert_matches_tape(data: &SixGroupDelayedNeutronData, file: &str, mat: i32) {
    let Some(t) = read_tape(file, mat, data.incident_energy_ev) else {
        return;
    };
    println!(
        "{file}: lambda {:?}\n  p {:?}\n  nu_d {:?} nu {:?} beta {:?}",
        t.lambda,
        t.abundance,
        t.nu_delayed,
        t.nu_total,
        t.nu_delayed / t.nu_total
    );
    for i in 0..6 {
        assert_eq!(data.decay_constants_per_s[i].to_bits(), t.lambda[i].to_bits(), "{file} lambda {i}");
        assert_eq!(data.abundances[i].to_bits(), t.abundance[i].to_bits(), "{file} p {i}");
    }
    assert_eq!(data.nu_delayed.to_bits(), t.nu_delayed.to_bits(), "{file} nu_d");
    assert_eq!(data.nu_total.to_bits(), t.nu_total.to_bits(), "{file} nu");
    let lambdas = data.decay_constant_array();
    let betas = data.delayed_fraction_array();
    for i in 0..6 {
        assert_eq!(lambdas[i].get::<hertz>().to_bits(), t.lambda[i].to_bits(), "{file} lambda array {i}");
        let beta_tape = t.abundance[i] * (t.nu_delayed / t.nu_total);
        assert_eq!(betas[i].get::<ratio>().to_bits(), beta_tape.to_bits(), "{file} beta {i}");
    }
}

#[test]
fn u235_constants_equal_endf_b_viii0() {
    assert_matches_tape(&ENDF_B_VIII0_U235, "n-092_U_235-ENDF8.0.endf", 9228);
}

#[test]
fn u238_constants_equal_endf_b_viii0() {
    assert_matches_tape(&ENDF_B_VIII0_U238, "n-092_U_238.endf", 9237);
}
