//! Verification for the generalized coherent-elastic path
//! (bead `op-jw4a`, mirrors GitHub issue #24 / bead `op-t33q`) against the
//! official ENDF/B-VIII.0 evaluated tapes.
//!
//! ## Methodology
//!
//! Regenerate `tsl-CinSiC.leapr` (MAT 44) and `tsl-SiinSiC.leapr` (MAT 43)
//! via `leapr::generate::generate_tape` with the new general coherent-elastic
//! implementation, parse the resulting MF=7/MT=2 section, and read off the
//! cumulative coherent-elastic cross section at 0.0253 eV (thermal point),
//! 296 K. Reference: `reference-data/endf/tsl-CinSiC.endf` /
//! `tsl-SiinSiC.endf`, whose MT=2 sections were measured independently
//! (bead `op-t33q`/`op-84v6`) at elastic = 2.94078 b for both materials
//! (coherent elastic is a lattice property of the 3C-SiC compound, not a
//! per-sublattice one — both tapes carry byte-identical MF=7/MT=2, see
//! `reference-data/endf/README.md`). Pass criterion here: within 5% of the
//! oracle value, and — the trap this bead exists to avoid — identical
//! between the two materials to within numerical noise.
//!
//! ## Results (measured 2026-08-19, release build)
//!
//! | Material | This port | Oracle tape | Relative error |
//! |---|---|---|---|
//! | C in SiC (MAT 44) | 2.853825 b | 2.94078 b | −2.96% |
//! | Si in SiC (MAT 43) | 2.853825 b | 2.94078 b | −2.96% |
//!
//! Interpretation: the general reciprocal-lattice-sum implementation, working
//! from the 3C-SiC crystal structure alone (no fit to the oracle), reproduces
//! the evaluator's coherent-elastic cross section to within 3% and correctly
//! assigns the identical value to both materials. This is not yet validated
//! to a tight tolerance or against the full Bragg-edge energy spectrum (only
//! the thermal-point cumulative value is checked) — see the bead for
//! follow-up: a full edge-by-edge comparison, and root-causing the residual
//! ~3% gap (candidates: Debye-Waller treatment, form-factor approximation,
//! or a missing higher-order reflection).

use njoy_outram_park_fork::leapr::decks::{embedded_deck_text, SabMaterial};
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::generate::{generate_tape, ElasticChannel};
use njoy_outram_park_fork::thermr::mf7::parse_mf7;
use njoy_outram_park_fork::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

const ORACLE_ELASTIC_BARN: f64 = 2.94078;
const E_THERMAL: f64 = 0.0253;

fn coherent_elastic_at_thermal_point(material: SabMaterial) -> f64 {
    let deck = LeaprDeck::parse(embedded_deck_text(material).unwrap()).unwrap();
    let tape = generate_tape(&deck, Temperature::new::<kelvin>(296.0), ElasticChannel::Generate).unwrap();
    let mf7 = parse_mf7(&tape, deck.mat).unwrap();
    let ce = mf7
        .coherent_elastic
        .expect("generalized coherent-elastic path should now populate MF=7/MT=2 for SiC");
    let mut s = 0.0;
    for (i, &e) in ce.bragg_energies_ev.iter().enumerate() {
        if e <= E_THERMAL {
            s = ce.s_tables[0][i];
        }
    }
    s / E_THERMAL
}

#[test]
fn c_in_sic_coherent_elastic_matches_oracle_within_five_percent() {
    let sigma = coherent_elastic_at_thermal_point(SabMaterial::CInSiC);
    let rel_err = (sigma - ORACLE_ELASTIC_BARN).abs() / ORACLE_ELASTIC_BARN;
    assert!(
        rel_err < 0.05,
        "C-in-SiC coherent elastic {sigma:.6} b vs oracle {ORACLE_ELASTIC_BARN} b, {:.2}% off (want <5%)",
        100.0 * rel_err
    );
}

#[test]
fn si_in_sic_coherent_elastic_matches_oracle_within_five_percent() {
    let sigma = coherent_elastic_at_thermal_point(SabMaterial::SiInSiC);
    let rel_err = (sigma - ORACLE_ELASTIC_BARN).abs() / ORACLE_ELASTIC_BARN;
    assert!(
        rel_err < 0.05,
        "Si-in-SiC coherent elastic {sigma:.6} b vs oracle {ORACLE_ELASTIC_BARN} b, {:.2}% off (want <5%)",
        100.0 * rel_err
    );
}

/// Pins the trap this bead exists to avoid: coherent elastic must come out
/// identical for both materials (one lattice, not two independent ones).
#[test]
fn coherent_elastic_is_identical_across_both_sic_materials() {
    let c = coherent_elastic_at_thermal_point(SabMaterial::CInSiC);
    let si = coherent_elastic_at_thermal_point(SabMaterial::SiInSiC);
    assert!(
        (c - si).abs() < 1e-9,
        "coherent elastic is a lattice property of the 3C-SiC compound and must be identical for \
         both materials, got C-in-SiC {c:.9} b vs Si-in-SiC {si:.9} b -- if a caller sums both \
         materials' elastic channels for one SiC region this double-counts Bragg scattering"
    );
}
