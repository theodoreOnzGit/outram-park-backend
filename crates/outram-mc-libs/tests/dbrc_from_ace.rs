// SPDX-License-Identifier: GPL-3.0

//! **DBRC on the ACE route: attached when the data permits, and never silently
//! absent** — GitHub #307 item 3.
//!
//! `Nuclide::from_ace` left `dbrc: None` unconditionally, which made two very
//! different situations look identical:
//!
//! - the table is broadened, so 0 K elastic **is not in the file** and DBRC
//!   genuinely cannot be built; and
//! - the reader simply did not look.
//!
//! That ambiguity is what #307 was filed about, and it is resolved by making
//! the reason **queryable** ([`Nuclide::dbrc_unavailable_reason`]) rather than
//! by guessing.
//!
//! # Why a broadened table cannot supply it
//!
//! DBRC samples the target velocity from the **unbroadened** elastic cross
//! section. An ACE table's ESZ elastic column is at the table's own
//! temperature, so a 293.6 K table's elastic is already broadened once —
//! using it would broaden twice. The 0 K data is not merely hard to find in
//! such a file; it is not there.
//!
//! # Results, 2026-09-25
//!
//! Printed by the tests. `reference-data/ace` ships U-235 at both 0 K and
//! 293.6 K, so every branch below runs on real data rather than a construction.

use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::reference_data::ace_reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

fn table(temp_dir: &str, label: &str) -> Option<read::RawAceTable> {
    let rel = format!("reference-njoy/endf-b-viii.0/{temp_dir}/U235.ace.gz");
    let p = ace_reference_file_or_skip(&rel, label)?;
    Some(read::read(&p).unwrap_or_else(|e| panic!("read {rel}: {e}")))
}

/// **A 0 K table carries its own 0 K elastic, so DBRC is ON by default.**
///
/// Default-on is the point: the ENDF route applies DBRC without being asked,
/// and a route that required an opt-in would be the same asymmetry #307 was
/// filed for, one level down.
#[test]
fn a_0k_table_gives_dbrc_by_default() {
    let Some(cold) = table("0K", "dbrc-from-ace/0K") else { return };
    let n = Nuclide::from_ace(&cold, "U235").expect("construct from the 0 K table");

    println!(
        "U235 from the 0 K ACE table: has_dbrc={}, reason={:?}",
        n.has_dbrc(),
        n.dbrc_unavailable_reason()
    );
    assert!(
        n.has_dbrc(),
        "a 0 K ACE table's ESZ elastic column IS the 0 K elastic cross section, \
         so DBRC must attach without being asked. {:?}",
        n.dbrc_unavailable_reason()
    );
    assert!(
        n.dbrc_unavailable_reason().is_none(),
        "DBRC is on, so there is no reason to report"
    );
    assert!(
        n.dbrc_table().is_some(),
        "has_dbrc() is true but the table is not reachable"
    );
}

/// **A broadened table gives no DBRC — and says why.**
#[test]
fn a_broadened_table_reports_why_dbrc_is_off() {
    let Some(hot) = table("293.6K", "dbrc-from-ace/293.6K") else { return };
    let n = Nuclide::from_ace(&hot, "U235").expect("construct from the 293.6 K table");

    let reason = n.dbrc_unavailable_reason();
    println!("U235 from the 293.6 K ACE table: has_dbrc={}", n.has_dbrc());
    println!("  reason: {}", reason.clone().unwrap_or_default());

    assert!(
        !n.has_dbrc(),
        "a table broadened to 293.6 K carries no 0 K elastic, so DBRC cannot be \
         built from it alone -- attaching one would mean broadening twice"
    );
    let r = reason.expect("a nuclide without DBRC must say why");
    assert!(
        r.contains("0 K elastic"),
        "the reason must name the missing data, not merely say 'unavailable': {r}"
    );
    assert!(
        r.contains("with_elastic_0k_from_ace"),
        "and must name the way out, or the caller is left guessing: {r}"
    );
}

/// **Pairing a broadened table with its 0 K companion attaches DBRC.**
///
/// This is the resolution #307 asked for: the data exists, in the 0 K table for
/// the same nuclide, and the two can be combined.
#[test]
fn pairing_a_broadened_table_with_its_0k_companion_attaches_dbrc() {
    let (Some(hot), Some(cold)) = (
        table("293.6K", "dbrc-pair/293.6K"),
        table("0K", "dbrc-pair/0K"),
    ) else {
        return;
    };

    let plain = Nuclide::from_ace(&hot, "U235").expect("construct");
    assert!(!plain.has_dbrc(), "precondition: the hot table alone has no DBRC");

    let paired = Nuclide::from_ace(&hot, "U235")
        .expect("construct")
        .with_elastic_0k_from_ace(&cold)
        .expect("the 0 K companion is the right nuclide at the right temperature");

    println!(
        "paired: has_dbrc={}, reason={:?}",
        paired.has_dbrc(),
        paired.dbrc_unavailable_reason()
    );
    assert!(
        paired.has_dbrc(),
        "pairing with the 0 K companion must attach DBRC: {:?}",
        paired.dbrc_unavailable_reason()
    );

    // And the cross sections must be unchanged -- pairing supplies 0 K elastic
    // for the VELOCITY sample only. If the transport cross sections moved, the
    // companion table has overwritten the broadened data, which would silently
    // run the whole problem at 0 K.
    for e in [1.0e-2, 1.0, 6.674, 1.0e3, 1.0e5] {
        let a = plain.xs_at_energy(e, 293.6);
        let b = paired.xs_at_energy(e, 293.6);
        assert!(
            (a.total - b.total).abs() <= 1.0e-12 * a.total.max(1.0),
            "pairing changed the total cross section at {e:e} eV ({} -> {}); the \
             0 K companion must supply the DBRC velocity sample ONLY, never \
             replace the broadened transport data",
            a.total,
            b.total
        );
    }
    println!("  transport cross sections unchanged at every energy checked");
}

/// **A companion that is not at 0 K is refused**, rather than broadening twice.
#[test]
fn a_broadened_companion_is_refused() {
    let Some(hot) = table("293.6K", "dbrc-refuse/293.6K") else { return };
    let err = Nuclide::from_ace(&hot, "U235")
        .expect("construct")
        .with_elastic_0k_from_ace(&hot)
        .expect_err("a 293.6 K table must not be accepted as a 0 K companion");
    let m = format!("{err}");
    println!("refused: {m}");
    assert!(m.contains("not 0 K"), "the error must say what is wrong: {m}");
    assert!(
        m.contains("broadened") && m.contains("shifts k"),
        "and why it matters, since the result would otherwise look plausible: {m}"
    );
}
