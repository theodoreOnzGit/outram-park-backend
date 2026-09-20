//! **V&V — the cold H₂/D₂ Young-Koppel path (`coldh`) against NJOY2016's own
//! reference tape.**
//!
//! # Why this test exists
//!
//! `leapr::coldh::add_cold_hydrogen` carried this note:
//!
//! > **Untrusted AI draft, self-consistency tested only.** [...] It has **not**
//! > been validated against a reference LEAPR cold-H2/D2 MF=7 tape — only
//! > checked for finiteness, correct array population, and a plausible
//! > normalization.
//!
//! and `LeaprDeck::unsupported_features` refused every `ncold != 0` deck on
//! exactly that ground. The validation was missing because no cold-moderator
//! reference was to hand: neither `reference-data/endf/` nor the crate's own
//! embedded decks contain an `ncold != 0` case.
//!
//! NJOY2016's **test 22** is one — para-hydrogen at 20 K, `ncold = 2` — and it
//! ships with `referenceTape20`, upstream's own assertion of what its code
//! produces. Both files are vendored verbatim into `reference-data/leapr/`
//! (see that directory's README for licence and provenance).
//!
//! # Result (2026-09-14)
//!
//! Over all **9175** tabulated `S(α, β)` values on the full signed β grid:
//!
//! ```text
//!   worst relative deviation   1.0e-13   at alpha = 4.80e2, beta = -1.60e1
//! ```
//!
//! i.e. machine precision. The "untrusted draft" was correct.
//!
//! # What had to be right at once
//!
//! The signed grid is the whole point. `coldh` splits its output by the sign of
//! β — negative into `ssm`, positive into `ssp` (`leapr.f90:2132-2133`) — and
//! `endout` only reads `ssp` when `isym` is odd. So this passes only if
//!
//! - `isym` is built as `(ncold != 0) + 2*(isabt == 1)` (`leapr.f90:423-425`),
//!   not as `isabt` alone;
//! - `generate_tape` actually calls `coldh`, in its upstream slot between
//!   `discre` and `skold` (`leapr.f90:383-390`);
//! - the resulting `ssp` reaches `LeaprOutput`;
//! - and `coldh`'s own Young-Koppel arithmetic is right.
//!
//! All four were separately broken or missing before 2026-09-14: the first was
//! a real `isym` bug, the middle two were never wired, and the last was the
//! unvalidated draft. The β grid length is asserted first (209 = 2×105 − 1)
//! because a mismatch there means the signed grid itself was not built.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::generate::{generate_tape, ElasticChannel};
use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;
use njoy_outram_park_fork::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

/// Worst relative deviation recorded 2026-09-14: 1.0e-13. The gate sits at
/// 1e-9 — far tighter than any physics tolerance, because this is upstream's
/// own output and the only thing between us and it is float arithmetic.
const S_TOL: f64 = 1.0e-9;

#[test]
fn cold_para_hydrogen_reproduces_njoy2016_reference_tape() {
    let Some(deck_path) = reference_file_or_skip(
        "leapr",
        "tsl-para-H2-20K-njoy2016-leapr.njoy-input",
        "coldh-oracle",
    ) else {
        return;
    };
    let Some(ref_path) = reference_file_or_skip(
        "leapr",
        "tsl-para-H2-20K-njoy2016-leapr.endf",
        "coldh-oracle",
    ) else {
        return;
    };

    let text = std::fs::read_to_string(&deck_path).expect("deck is readable");
    let deck = LeaprDeck::parse(&text).expect("NJOY test-22 deck parses");
    assert_eq!(
        deck.unsupported_features(),
        Vec::<String>::new(),
        "a cold-hydrogen deck must no longer be refused"
    );

    let ours = generate_tape(
        &deck,
        Temperature::new::<kelvin>(20.0),
        ElasticChannel::Omit,
    )
    .expect("cold-hydrogen generation");
    let theirs = Tape::read_file(&ref_path).expect("NJOY reference tape");

    let o = parse_mf7_at_temperature(&ours, deck.mat, Some(20.0))
        .expect("our MF=7")
        .incoherent_inelastic
        .expect("our MT=4");
    let r = parse_mf7_at_temperature(&theirs, deck.mat, Some(20.0))
        .expect("NJOY MF=7")
        .incoherent_inelastic
        .expect("NJOY MT=4");

    // 209 = 2*105 - 1: the signed grid `coldh` produces, not the 105-point
    // one-sided grid every other LEAPR path writes.
    assert_eq!(
        (o.beta.len(), r.beta.len()),
        (209, 209),
        "the signed beta grid was not built"
    );
    assert_eq!(o.s_tables.len(), r.s_tables.len(), "S table count");

    let (mut worst, mut wa, mut wb) = (0.0f64, 0.0f64, 0.0f64);
    let mut compared = 0usize;
    for (ib, (ot, rt)) in o.s_tables.iter().zip(&r.s_tables).enumerate() {
        assert_eq!(ot.alpha.len(), rt.alpha.len(), "alpha grid at beta {ib}");
        for (ia, (&a, &b)) in ot.s.iter().zip(&rt.s).enumerate() {
            if b.abs() < 1.0e-30 {
                continue;
            }
            compared += 1;
            let rel = (a - b).abs() / b.abs();
            if rel > worst {
                worst = rel;
                wa = ot.alpha[ia];
                wb = o.beta[ib];
            }
        }
    }
    println!(
        "  coldh vs NJOY2016 test-22 reference: {compared} values, worst {worst:.3e} \
         at alpha = {wa:.3e}, beta = {wb:.3e}"
    );
    assert!(
        compared > 9000,
        "only {compared} values compared — the law came back mostly empty"
    );
    assert!(
        worst < S_TOL,
        "S(alpha, beta) is {worst:.3e} from NJOY2016's own reference tape at \
         alpha = {wa:.3e}, beta = {wb:.3e} (recorded 1.0e-13 on 2026-09-14)"
    );
}
