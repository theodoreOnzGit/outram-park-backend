// SPDX-License-Identifier: GPL-3.0

//! **Writing the ACE UNR block** — the unresolved-range probability tables at
//! `JXS(23)` — GitHub #325.
//!
//! The reader has existed since GitHub #307
//! ([`crate::purr::UrrProbabilityTables::from_ace`]); the writer did not, so a
//! table this crate wrote carried **no URR self-shielding** even though both
//! ends of the round trip could now handle it. Every `ENDF -> ACER -> read`
//! parity study in the workspace was therefore comparing different physics —
//! `outram-mc-libs`' `lct008_ace_roundtrip` had to ablate URR off its ENDF arm to
//! get a symmetric comparison, because it could not add URR to the ACE one.
//!
//! # Upstream is the specification
//!
//! Ported from `acefc.f90:5958-5990` (NJOY2016), the "store unresolved-range
//! probability tables after energy distributions" block of `acelod`, with the
//! Type-1 integer typing from `:13447-13458`:
//!
//! | words | written as | value |
//! |---|---|---|
//! | 0 | integer | `N`, number of energies |
//! | 1 | integer | `M`, number of bands |
//! | 2 | integer | interpolation — ACER always writes `2` |
//! | 3 | integer | inelastic competition flag (`ILF`) |
//! | 4 | integer | other-absorption flag (`IOA`) |
//! | 5 | integer | `IFF` = `LSSF` |
//! | 6..6+N | real | energies \[MeV\], `sigfig(·, 7)` |
//! | then, per energy | real | `M` cumulative probabilities, then `M` each of total, elastic, fission, capture, heating |
//!
//! Three details that are easy to get wrong and that the tests pin:
//!
//! - **The probability column is cumulative**, rounded to 7 figures, and the
//!   **last band is forced to exactly `1`** (`xss(ll+nurb)=1`, `:5988`), after
//!   the rounding. A table whose cumulative sum rounds to `0.9999999` must still
//!   end at `1`, or a sampler drawing `ξ` close to 1 falls off the end.
//! - **Heating is divided by 1e6 only when `LSSF = 0`** (`:5979-5983`). With
//!   `LSSF = 1` it is a dimensionless factor and must not be scaled — the one
//!   column whose unit depends on a flag.
//! - **The block goes immediately after DLW** and before the delayed-neutron
//!   and photon-production blocks, which is where NJOY puts it: measured on
//!   NJOY's own U-238 table, `DLW -> LUNR -> DNU -> … -> GPD -> MTRP`.
//!
//! The value columns are rounded to 7 significant figures here because PURR
//! writes them that way on MT=153 (`purr.f90:500-522`) and ACER copies them
//! verbatim, so a table generated in-process must be rounded where NJOY's
//! arrive already rounded. On a table read from an NJOY file the rounding is a
//! no-op, which is what the round-trip gate in
//! `tests/unr_block_write_vs_njoy2016.rs` checks word for word.

use crate::acer::build::sigfig;
use crate::purr::UrrProbabilityTables;

const EMEV: f64 = 1.0e6;

/// The UNR block as `(value, is_integer)` words, ready to append to `XSS`.
///
/// Empty for an empty table, in which case the caller must leave `JXS(23)` at
/// zero — a zero locator is ACE's "no unresolved range", and a locator pointing
/// at an empty block would be a malformed table.
pub fn unr_words(t: &UrrProbabilityTables) -> Vec<(f64, bool)> {
    let points = t.points();
    let nure = points.len();
    let nurb = t.n_bands();
    if nure == 0 || nurb == 0 {
        return Vec::new();
    }
    let mut w: Vec<(f64, bool)> = Vec::with_capacity(6 + nure * (1 + 6 * nurb));

    // Header: six integers (`acefc.f90:13450-13457`).
    w.push((nure as f64, true));
    w.push((nurb as f64, true));
    w.push((f64::from(t.interpolation), true));
    w.push((f64::from(t.inelastic_competition), true));
    w.push((f64::from(t.absorption_competition), true));
    w.push((f64::from(t.lssf), true));

    // Energies, eV -> MeV, 7 figures (`:5972`).
    for &e in t.energies() {
        w.push((sigfig(e / EMEV, 7), false));
    }

    for p in points {
        // Cumulative probability, 7 figures, last band pinned to 1 AFTER the
        // rounding (`:5987-5988`).
        let mut cum: Vec<f64> = p.cum.iter().map(|&c| sigfig(c, 7)).collect();
        if let Some(last) = cum.last_mut() {
            *last = 1.0;
        }
        for c in cum {
            w.push((c, false));
        }
        // Total, elastic, fission, capture — column-major over bands, as ACE
        // stores `(column, band)` within one energy.
        for col in 0..4 {
            for v in &p.value {
                w.push((sigfig(v[col], 7), false));
            }
        }
        // Heating: MeV for LSSF = 0, a factor for LSSF = 1; not rounded, as
        // upstream does not round it.
        for &h in &p.heating {
            let v = if t.lssf == 0 { h / EMEV } else { h };
            w.push((v, false));
        }
    }
    w
}
