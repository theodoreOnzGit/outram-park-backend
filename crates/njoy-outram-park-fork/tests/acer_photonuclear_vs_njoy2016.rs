//! **V&V gate** — the photo-nuclear ACE class (`acer iopt = 5`) against
//! NJOY2016.
//!
//! ## Where the tape comes from
//!
//! `reference-data/endf/` holds no photo-nuclear sublibrary tape, and the IAEA
//! NDS host this crate's `acquire` module downloads from (`www-nds.iaea.org`)
//! is blocked by the execution environment's network policy — a `403` on
//! CONNECT — so one cannot be fetched. Verification therefore uses
//! `reference-data/endf/photonuc-synthetic-Z6.endf`, a synthetic NSUB=0 tape
//! built by the committed generator beside it and fed to **both** codes.
//!
//! That is the same device `photoat-synthetic-Z6.endf` and
//! `synthetic-caseb-lfw1.endf` already use in this repository. It verifies
//! that the two codes agree on the same input, which is exactly what a
//! code-to-code comparison needs, and it claims **nothing** about physics:
//! every cross section and spectrum in it is an analytic shape, and it must
//! never be used for anything but this.
//!
//! NJOY input that produced the reference:
//!
//! ```text
//! acer / 20 21 0 24 25 / 5 1 1 .00 / 'z6 photonuclear' / 600 0.
//! ```
//!
//! with the synthetic tape on both units 20 and 21 — a photo-nuclear
//! evaluation has no resonances, so its own linear MF=3 serves as the PENDF
//! `acephn` takes the ACE energy grid from (`acepn.f90:207-227`).
//!
//! ## Results (2026-09-22)
//!
//! | check | result |
//! |---|---|
//! | read NJOY's 27 453-word table | class `u`, ZA 6012, structure recovered |
//! | read-then-write, 556 781 bytes | **byte-identical** |
//! | layout walk | 4 emitted particles, ends exactly at `NXS(1)` |
//!
//! The byte-identical round trip is the load-bearing one: a photo-nuclear
//! table's integer/real split is not stored anywhere, so reproducing it
//! requires walking the *whole* per-particle structure — `PXS`, `PHN`,
//! `MTRP`, `TYRP`, `LSIGP`/`SIGP`, `LANDP`/`ANDP` and `LDLWP`/`DLWP` with its
//! law bodies — exactly as `phnout` does. Getting one count or one locator
//! wrong anywhere in those 27 453 words changes a byte.

use njoy_outram_park_fork::acer::photonuclear::layout::{self, jxs, nxs};
use njoy_outram_park_fork::acer::read::{self, AceClass};
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

const ACE: &str = "z6_photonuclear_njoy2016.ace";

/// The table NJOY wrote, and the structure this port recovers from it.
#[test]
fn photonuclear_layout_matches_njoy2016() {
    let Some(path) = reference_file_or_skip("acer", ACE, "acer photonuclear") else {
        return;
    };
    let t = read::read_type1(&path).expect("read NJOY's photonuclear ACE");
    assert_eq!(t.header.class, AceClass::Photonuclear, "class from the ZAID");
    assert_eq!(t.header.zaid, "6012.00u");
    assert_eq!(t.nxs[nxs::LXS] as usize, t.xss.len(), "NXS(1) = XSS length");
    assert_eq!(t.nxs[nxs::ZA], 6012, "NXS(2) = ZA");
    assert_eq!(t.nxs[nxs::NES], 38, "NXS(3) = NES");
    assert_eq!(t.nxs[nxs::NTR], 1, "NXS(4) = NTR — the LANL MT=5-only form");
    assert_eq!(t.nxs[nxs::NTYPE], 4, "NXS(5) = NTYPE — n, p, alpha, photon");
    assert_eq!(t.nxs[nxs::NEIXS], 12, "NXS(7) = NEIXS — 12 locators per particle");
    assert_eq!(t.nxs[nxs::IZ], 6, "NXS(10) = Z");
    assert_eq!(t.nxs[nxs::IA], 12, "NXS(11) = A");
    // NON aliases TOT when the evaluation has no elastic (`acepn.f90:236-247`).
    assert_eq!(t.jxs[jxs::ELS], 0, "no MF=3/MT=2 on this tape");
    assert_eq!(
        t.jxs[jxs::NON], t.jxs[jxs::TOT],
        "with no elastic, NON must alias TOT rather than get its own block"
    );

    let w = layout::walk(&t.nxs, &t.jxs, &t.xss).expect("walk the layout");
    assert_eq!(w.particles.len(), 4, "one IXSA row per emitted particle");
    assert_eq!(
        w.reached,
        t.xss.len(),
        "the walk must end exactly at the last word: it reached {} of {}",
        w.reached,
        t.xss.len()
    );
    // Every particle's blocks must lie inside the table and ascend, which is
    // what `advance_to_locator` requires of a well-formed table.
    for (i, p) in w.particles.iter().enumerate() {
        assert!(p.ntrp >= 1, "particle {i} produces no reactions");
        let locs = [
            p.pxs, p.phn, p.mtrp, p.tyrp, p.lsigp, p.sigp, p.landp, p.andp, p.ldlwp, p.dlwp,
        ];
        assert!(
            locs.iter().all(|&l| l >= 1 && l <= t.xss.len()),
            "particle {i} has a locator outside the table: {locs:?}"
        );
        assert!(
            locs.windows(2).all(|w| w[0] <= w[1]),
            "particle {i}'s blocks must be in writing order: {locs:?}"
        );
    }
    let ipts: Vec<i32> = w.particles.iter().map(|p| p.ipt).collect();
    eprintln!(
        "[photonuclear] {} words, {} particles (IPT {ipts:?}), walk ends at {}",
        t.xss.len(),
        w.particles.len(),
        w.reached
    );
}

/// The integer/real split is not stored in the file, so reproducing NJOY's
/// bytes means the whole per-particle walk is right.
#[test]
fn photonuclear_read_then_write_is_byte_exact() {
    let Some(path) = reference_file_or_skip("acer", ACE, "acer photonuclear") else {
        return;
    };
    let t = read::read_type1(&path).expect("read NJOY's photonuclear ACE");
    assert!(
        t.derive_xss_is_int().is_some(),
        "the photonuclear class must derive its integer mask, not fall back \
         to the reader's value heuristic"
    );
    let want = std::fs::read_to_string(&path).expect("read as text");
    let got = t.to_type1_string();
    if got != want {
        let n = got
            .bytes()
            .zip(want.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        panic!(
            "photonuclear round trip differs at byte {n} of {} (NJOY {} bytes)\n  \
             ours: {:?}\n  njoy: {:?}",
            got.len(),
            want.len(),
            &got[n.saturating_sub(40)..(n + 40).min(got.len())],
            &want[n.saturating_sub(40)..(n + 40).min(want.len())],
        );
    }
    eprintln!("[photonuclear] reproduced NJOY's {} bytes exactly", want.len());
}

/// A malformed table must be reported, not walked off the end of. The walk is
/// the only thing standing between a corrupt count and a plausible-looking
/// answer, so it is checked that it actually refuses.
#[test]
fn a_corrupt_count_is_refused_rather_than_walked_past() {
    let Some(path) = reference_file_or_skip("acer", ACE, "acer photonuclear") else {
        return;
    };
    let t = read::read_type1(&path).expect("read NJOY's photonuclear ACE");
    let mut bad = t.clone();
    // Claim a far longer energy grid than the table holds.
    bad.nxs[nxs::NES] = 1_000_000;
    let err = layout::walk(&bad.nxs, &bad.jxs, &bad.xss)
        .expect_err("an impossible NES must not walk");
    assert!(
        format!("{err}").contains("past the end"),
        "the refusal should say what went wrong: {err}"
    );
}
