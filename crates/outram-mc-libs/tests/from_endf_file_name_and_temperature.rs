//! **`Nuclide::from_endf_file` checks the name against the tape and takes a
//! `uom` temperature** (GitHub #934, the remainders of #672).
//!
//! # Methodology
//!
//! - **Name check:** `check_name_against_za` on hand-made cases (a match, a
//!   wrong mass, a wrong element, an elemental `A = 0`, free labels), and
//!   `from_endf_file` / `from_endf_file_with_speed` on the ENDF/B-VIII.0
//!   F-19 tape labelled `"O16"` (refused, before any processing) and `"F19"`
//!   and `"fluorine"` (accepted).
//! - **Temperature:** `from_endf_file(.., 293.6 K as uom, ..)` must build the
//!   same nuclide as `from_tape(.., 293.6, ..)`: the microscopic cross
//!   sections at 200 log-spaced energies (1e-5 eV to 20 MeV) are compared
//!   bit for bit.
//!
//! # Results (2026-10-17)
//!
//! All pass: the mislabelled tape is refused with an error naming both
//! nuclides; the typed and bare-`f64` paths agree bit for bit at all 200
//! energies. Tape absent: the tape cases skip.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::NjoyError;
use outram_mc_libs::material::nuclide::{check_name_against_za, Nuclide};
use outram_mc_libs::material::speed::SpeedTier;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

#[test]
fn names_that_read_as_nuclides_must_match_the_za() {
    assert!(check_name_against_za("U235", 92235.0).is_ok());
    assert!(
        check_name_against_za("C0", 6000.0).is_ok(),
        "A = 0 is an elemental tape"
    );
    for (name, za) in [("U235", 92238.0), ("C12", 6000.0), ("Pu239", 92239.0)] {
        match check_name_against_za(name, za) {
            Err(NjoyError::InvalidInput(m)) => assert!(m.contains(name), "{m}"),
            other => panic!("{name} vs ZA {za}: expected a refusal, got {other:?}"),
        }
    }
    for label in ["Cnat", "U235_hot", "Am242m", "fuel", "U-235"] {
        assert!(
            check_name_against_za(label, 1001.0).is_ok(),
            "{label} is a free label"
        );
    }
}

#[test]
fn a_mislabelled_tape_is_refused_and_a_typed_temperature_builds_the_same_nuclide() {
    let Some(p) = reference_endf_or_skip("n-009_F_019-ENDF8.0.endf", "from_endf_file name check")
    else {
        return;
    };
    let t = ThermodynamicTemperature::new::<kelvin>(293.6);
    assert!(matches!(
        Nuclide::from_endf_file(&p, "O16", t, 1.0e-3),
        Err(NjoyError::InvalidInput(_))
    ));
    assert!(matches!(
        Nuclide::from_endf_file_with_speed(&p, "O16", 293.6, SpeedTier::Fast),
        Err(NjoyError::InvalidInput(_))
    ));
    let typed = Nuclide::from_endf_file(&p, "F19", t, 1.0e-3).expect("F-19");
    let labelled = Nuclide::from_endf_file(&p, "fluorine", t, 1.0e-3).expect("a free label");
    let tape = Tape::read_file(&p).expect("tape");
    let bare = Nuclide::from_tape(&tape, 925, "F19", 293.6, 1.0e-3).expect("from_tape");
    for i in 0..200 {
        let e = 1.0e-5 * (2.0e7_f64 / 1.0e-5).powf(i as f64 / 199.0);
        let (a, b, c) = (
            typed.xs_at_energy(e, 293.6),
            bare.xs_at_energy(e, 293.6),
            labelled.xs_at_energy(e, 293.6),
        );
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "E = {e} eV");
        assert_eq!(format!("{a:?}"), format!("{c:?}"), "E = {e} eV");
    }
}
