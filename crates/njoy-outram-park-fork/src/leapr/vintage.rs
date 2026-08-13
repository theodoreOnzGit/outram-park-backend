//! Evaluation **vintage** and the physical-constant set that goes with it.
//!
//! # Why this module exists
//!
//! Reproducing a published ENDF evaluation is not only a question of running the
//! right physics on the right input deck — it also requires the *physical
//! constants the generator carried at the time the evaluation was produced*.
//! `S(alpha, beta)` depends on the Boltzmann constant through `tev = k_B T`,
//! which sets both the phonon grid spacing `deltab = delta_E / tev` and the
//! `LAT = 1` scale factor `0.0253 / tev`. NJOY2016 has changed its `bk` twice, so
//! a modern build of a faithful port does **not** reproduce a 2017 evaluation
//! unless it is told to use the 2017 constant.
//!
//! This was measured, not assumed. Regenerating ENDF/B-VIII.0
//! `tsl-crystalline-graphite` (EVAL-SEP17) and comparing MF=7/MT=4 against the
//! official tape gave, on 2026-08-13:
//!
//! | `bk` used to generate | max rel. dev. | RMS rel. dev. |
//! |---|---|---|
//! | `8.617385e-5` ([`PhysicalConstants::Njoy2016Legacy`]) | **4.917e-6** | **6.390e-7** |
//! | `8.617342e-5` ([`PhysicalConstants::Codata2014`]) | 3.086e-4 | 5.687e-5 |
//! | `8.617333262e-5` ([`PhysicalConstants::Codata2018`], crate default) | 3.711e-4 | 6.842e-5 |
//!
//! The 4.917e-6 figure is the tape's own 6-to-7-significant-figure storage
//! precision; the other two are a real physics mismatch. Full methodology,
//! including the storage-band split that demonstrates the residual is round-off,
//! is in `tests/leapr_graphite_deck_parity.rs`.
//!
//! # It is not only `k_B` — the elastic channel needs four more
//!
//! The coherent-elastic channel (MF=7/MT=2) does not depend on `bk` at all. Its
//! Bragg edges sit at `E = tau^2 / econ` with
//! `econ = ev * 8 * (amassn * amu / hbar) / hbar` (`leapr.f90:2543`), so it
//! depends on `ev`, `amu`, `hbar` and `amassn` instead. Measured against the
//! same tape, over 221 Bragg edges and 2,200 `S(E, T)` values at ten
//! temperatures (2026-08-13):
//!
//! | Constants used | Bragg energies | `S(E, T)` |
//! |---|---|---|
//! | modern (`bk` alone corrected) | 9.937e-7 | 9.986e-7 |
//! | the full [`PhysicalConstants::Njoy2016Legacy`] set | **1.001e-13** | **1.001e-13** |
//!
//! The 1e-6 residual is a **uniform multiplicative offset** — fitted
//! `+5.115e-7` with only `2.088e-7` scatter about that single factor — which is
//! what a wrong `econ` looks like and is not what a mistranscribed lattice
//! constant would look like (a slip in `a` or `c` moves `(00l)` and `(hk0)`
//! families by different amounts). Substituting the whole set takes it to float
//! round-trip noise on a 7-significant-figure ENDF field.
//!
//! The legacy values are read out of NJOY2016's own `src/phys.f90` at the commit
//! preceding `007828d` (2017-10-23) — the constants in force for an
//! `EVAL-SEP17` evaluation — not fitted to the residual. That `bk = 8.617385e-5`
//! from that same file is independently what the *inelastic* channel needed
//! corroborates the vintage inference rather than merely fitting it.
//!
//! # What this module does **not** do
//!
//! It does **not** change [`crate::common::phys::BK_EV_PER_K`]. The modern
//! CODATA2018 value is the correct constant for a *new* calculation; the
//! historical value is correct only for *reproducing an old evaluation*.
//! Conflating the two would trade a visible 100x parity gap for an invisible
//! physics error in every other calculation in the crate. So the constant set is
//! an explicit, documented **input**, carried on
//! [`LeaprInput`](crate::leapr::input::LeaprInput) and
//! [`LeaprOutput`](crate::leapr::endout::LeaprOutput), and defaulting to the
//! crate constant.
//!
//! # Where the vintage comes from
//!
//! From the evaluation's own `EVAL-<MON><YY>` field, not from a hardcoded
//! per-material rule. ENDF evaluations carry it in MF=1/MT=451; the LEAPR decks
//! that generated them carry the same string in their comment cards (card 20).
//! [`EvaluationDate::parse_eval_field`] reads it and
//! [`PhysicalConstants::for_evaluation_date`] maps it through NJOY's own
//! constant history. Nothing here is keyed on a material name.

/// The constant values themselves, per vintage. See [`PhysicalConstants`].
mod values {
    /// NJOY2016's `src/phys.f90` **at the commit preceding `007828d`**
    /// (2017-10-23, "Incorporating Skip's changes") — i.e. the constants in
    /// force when an `EVAL-SEP17` evaluation was produced. Read out of the
    /// upstream git history, not inferred.
    pub mod njoy2016_legacy {
        /// Boltzmann constant \[eV/K\].
        pub const BK_EV_PER_K: f64 = 8.617_385e-5;
        /// 1 eV in ergs \[erg/eV\].
        pub const EV_ERG: f64 = 1.602_177_33e-12;
        /// Atomic mass unit \[g/amu\].
        pub const AMU_G: f64 = 1.660_540_2e-24;
        /// Reduced Planck constant \[erg s\].
        pub const HBAR_ERG_S: f64 = 1.054_572_66e-27;
        /// Neutron mass \[amu\].
        pub const AMASSN_AMU: f64 = 1.008_664_904;
    }

    /// CODATA 2014 Boltzmann constant. The other four are not separately
    /// recorded for this vintage — see the note on
    /// [`super::PhysicalConstants::Codata2014`].
    pub const BK_CODATA2014: f64 = 8.617_342e-5;
}

/// A named set of physical constants, selected by the **vintage of the
/// evaluation being reproduced**.
///
/// A closed enum (workspace rule: no trait objects). It carries the five NJOY
/// `src/phys.f90` constants that LEAPR's output depends on — `bk`, `ev`, `amu`,
/// `hbar` and `amassn` — because both LEAPR channels need them and they need
/// *different combinations* of them:
///
/// | Channel | Depends on | Via |
/// |---|---|---|
/// | MF=7/MT=4 incoherent inelastic | `bk` | `tev = bk * T` |
/// | MF=7/MT=2 coherent elastic | `ev`, `amu`, `hbar`, `amassn` | [`Self::econ`], the Bragg-edge energy scale |
///
/// Getting only `bk` right leaves the elastic channel about 1e-6 off — a
/// uniform multiplicative offset, measured at `+5.115e-7` for ENDF/B-VIII.0
/// graphite. Getting the **whole set** right takes it to 1.001e-13, i.e. float
/// round-trip noise on a 7-digit ENDF field. See the module docs.
///
/// # Choosing a variant
///
/// - **Generating new data** for a calculation of your own: use
///   [`PhysicalConstants::Codata2018`] (the [`Default`]). It is the physically
///   correct constant today.
/// - **Reproducing a published evaluation**: use whatever
///   [`PhysicalConstants::for_evaluation_date`] returns for that evaluation's
///   `EVAL-<MON><YY>` field. Using the modern constant instead is not a
///   rounding-level difference — see the module docs' measured table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PhysicalConstants {
    /// `bk = 8.617385e-5 eV/K` — the value NJOY2016 carried from release until
    /// **23 Oct 2017**, when upstream switched to CODATA2014. Every ENDF
    /// evaluation produced with NJOY before that date was generated with this
    /// number, which is why reproducing one needs it.
    Njoy2016Legacy,
    /// `bk = 8.617342e-5 eV/K` — CODATA 2014, which NJOY2016 adopted on
    /// 23 Oct 2017.
    ///
    /// **Not auto-selected.** The date on which NJOY moved from CODATA2014 to
    /// CODATA2018 has not been established in this workspace, so
    /// [`PhysicalConstants::for_evaluation_date`] never returns this variant —
    /// it must be chosen explicitly when you know an evaluation falls in that
    /// window. See the caveat on that function.
    ///
    /// **Partial set.** Only `bk` is recorded for this vintage; the other four
    /// constants fall back to the crate's modern values. That is enough to
    /// reproduce the CODATA2014 row of the module's inelastic table, and is
    /// **not** enough to reproduce a CODATA2014-era elastic channel bit-exactly
    /// — [`Self::econ`] would be wrong. Read the values out of NJOY's
    /// `src/phys.f90` at the relevant commit before relying on it there.
    Codata2014,
    /// `bk = 8.617333262e-5 eV/K` — CODATA 2018, exact under the 2019 SI
    /// redefinition. The crate default
    /// ([`crate::common::phys::BK_EV_PER_K`]) and the right choice for
    /// generating new data.
    #[default]
    Codata2018,
}

impl PhysicalConstants {
    /// The Boltzmann constant for this set, in **eV per kelvin**.
    ///
    /// Spelled out for human readers even though callers multiply it by a
    /// temperature in kelvin to get an energy in eV: `tev = bk * T`.
    ///
    /// This is the constant that governs the **inelastic** law
    /// ([`crate::leapr::continuous::phonon_expansion`]) through
    /// `tev = bk * T`.
    pub const fn bk_ev_per_k(self) -> f64 {
        match self {
            PhysicalConstants::Njoy2016Legacy => values::njoy2016_legacy::BK_EV_PER_K,
            PhysicalConstants::Codata2014 => values::BK_CODATA2014,
            PhysicalConstants::Codata2018 => crate::common::phys::BK_EV_PER_K,
        }
    }

    /// One electronvolt in **ergs** (`erg/eV`) — NJOY's `ev`.
    pub const fn ev_erg(self) -> f64 {
        match self {
            PhysicalConstants::Njoy2016Legacy => values::njoy2016_legacy::EV_ERG,
            _ => crate::common::phys::EV_ERG,
        }
    }

    /// The atomic mass unit in **grams** (`g/amu`) — NJOY's `amu`.
    pub const fn amu_g(self) -> f64 {
        match self {
            PhysicalConstants::Njoy2016Legacy => values::njoy2016_legacy::AMU_G,
            _ => crate::common::phys::AMU_G,
        }
    }

    /// The reduced Planck constant in **erg-seconds** — NJOY's `hbar`.
    pub const fn hbar_erg_s(self) -> f64 {
        match self {
            PhysicalConstants::Njoy2016Legacy => values::njoy2016_legacy::HBAR_ERG_S,
            _ => crate::common::phys::HBAR_ERG_S,
        }
    }

    /// The neutron mass in **amu** — NJOY's `amassn`.
    pub const fn amassn_amu(self) -> f64 {
        match self {
            PhysicalConstants::Njoy2016Legacy => values::njoy2016_legacy::AMASSN_AMU,
            _ => crate::common::phys::AMASSN_AMU,
        }
    }

    /// LEAPR's `econ` — the Bragg-edge energy conversion
    /// `ev * 8 * (amassn * amu / hbar) / hbar` (`leapr.f90:2543`), in
    /// **reciprocal-lattice units per eV**.
    ///
    /// An edge at squared reciprocal-lattice vector `tau^2` sits at
    /// `E = tau^2 / econ`, so `econ` scales **every** coherent-elastic edge
    /// energy by the same factor. That is why a wrong constant set shows up as
    /// a *uniform multiplicative offset* in the Bragg grid rather than as
    /// scatter — a mistranscribed lattice constant would move `(00l)` and
    /// `(hk0)` families differently, and this does not.
    ///
    /// This is the quantity that governs the **elastic** channel, and it is a
    /// different combination of constants from [`Self::bk_ev_per_k`]. Both must
    /// come from the same vintage.
    pub fn econ(self) -> f64 {
        let amne = self.amassn_amu() * self.amu_g();
        self.ev_erg() * 8.0 * (amne / self.hbar_erg_s()) / self.hbar_erg_s()
    }

    /// A short human-readable label for logs, cache keys and provenance records
    /// (e.g. `"njoy2016-legacy"`). Stable — cache keys are built from it, so
    /// changing a label invalidates cached artifacts.
    pub const fn label(self) -> &'static str {
        match self {
            PhysicalConstants::Njoy2016Legacy => "njoy2016-legacy",
            PhysicalConstants::Codata2014 => "codata2014",
            PhysicalConstants::Codata2018 => "codata2018",
        }
    }

    /// Every variant, for iterating over the supported set.
    pub const fn all() -> [PhysicalConstants; 3] {
        [
            PhysicalConstants::Njoy2016Legacy,
            PhysicalConstants::Codata2014,
            PhysicalConstants::Codata2018,
        ]
    }

    /// The constant set NJOY carried when an evaluation dated `date` was
    /// produced.
    ///
    /// # The one date this is allowed to know
    ///
    /// NJOY2016 replaced `bk = 8.617385e-5` with the CODATA2014 value on
    /// **23 Oct 2017** (confirmed against the upstream git history while
    /// measuring the graphite parity, 2026-08-13). An evaluation dated strictly
    /// before that was generated with [`PhysicalConstants::Njoy2016Legacy`].
    ///
    /// # Caveat — the later boundary is not established
    ///
    /// The date NJOY moved from CODATA2014 to CODATA2018 has **not** been
    /// established here, so this function returns
    /// [`PhysicalConstants::Codata2018`] for anything on or after the 2017
    /// cutoff. For an evaluation produced in the CODATA2014 window that is the
    /// wrong answer, and the error would be of order the `8.617342e-5` row in
    /// the module table. Pass [`PhysicalConstants::Codata2014`] explicitly if
    /// you know an evaluation falls there; do not rely on this function to find
    /// it for you.
    ///
    /// # Caveat — month resolution
    ///
    /// ENDF `EVAL-<MON><YY>` fields carry no day. An evaluation dated the same
    /// month as the cutoff (`OCT17`) is therefore genuinely ambiguous; this
    /// function treats a month as its first day, so `OCT17` resolves to
    /// [`PhysicalConstants::Njoy2016Legacy`]. Check such a case by measurement
    /// rather than trusting the default.
    pub const fn for_evaluation_date(date: EvaluationDate) -> Self {
        // 2017-10-23, taken as (year, month) with the month's first day.
        if date.year < 2017 || (date.year == 2017 && date.month <= 10) {
            PhysicalConstants::Njoy2016Legacy
        } else {
            PhysicalConstants::Codata2018
        }
    }
}

/// The month and year of an ENDF `EVAL-<MON><YY>` field.
///
/// No day: the format does not carry one. `month` is 1..=12.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvaluationDate {
    /// Four-digit calendar year, expanded from the field's two digits (see
    /// [`EvaluationDate::parse_eval_field`]).
    pub year: i32,
    /// Calendar month, 1 (January) .. 12 (December).
    pub month: u32,
}

/// The three-letter month abbreviations ENDF `EVAL-` fields use, index 0 = JAN.
const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

impl EvaluationDate {
    /// Parse the **first** `EVAL-<MON><YY>` field found anywhere in `text`.
    ///
    /// `text` is any block that may contain the field — an MF=1/451 comment
    /// line, or a LEAPR deck's card-20 comment block, which carries the same
    /// string (ENDF/B-VIII.0 graphite's reads
    /// `' Graphite  LEIP LAB   EVAL-SEP17 A.I. Hawari, Y. Zhu, J.L. Wormald'`).
    /// Matching is case-sensitive on the uppercase ENDF spelling.
    ///
    /// # Two-digit years
    ///
    /// The field carries two digits. They are expanded with a pivot at 50:
    /// `00`..`49` -> 2000..2049, `50`..`99` -> 1950..1999. Nuclear evaluations
    /// exist on both sides of 2000, and no ENDF evaluation postdates 2049, so
    /// this pivot is unambiguous for the whole corpus.
    ///
    /// Returns `None` when no well-formed field is present.
    pub fn parse_eval_field(text: &str) -> Option<Self> {
        let mut i = 0usize;
        while let Some(off) = text[i..].find("EVAL-") {
            let start = i + off + "EVAL-".len();
            i = start;
            // Need three letters of month plus two ASCII digits of year, and the
            // slice must land on character boundaries (the comment cards are
            // ASCII, but `text` is not required to be).
            if !text.is_char_boundary(start) || !text.is_char_boundary(start + 5) {
                continue;
            }
            let Some(field) = text.get(start..start + 5) else {
                continue;
            };
            let (mon, yy) = field.split_at(3);
            let Some(month0) = MONTHS.iter().position(|&x| x == mon) else {
                continue;
            };
            if !yy.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let y: i32 = yy.parse().expect("two ASCII digits parse");
            let year = if y >= 50 { 1900 + y } else { 2000 + y };
            return Some(EvaluationDate {
                year,
                month: month0 as u32 + 1,
            });
        }
        None
    }
}

impl std::fmt::Display for EvaluationDate {
    /// Renders back in the ENDF spelling, e.g. `EVAL-SEP17`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EVAL-{}{:02}",
            MONTHS[(self.month - 1) as usize],
            self.year % 100
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three constant sets carry the values measured in the graphite parity
    /// study, and the default is the crate constant.
    ///
    /// **Methodology.** Assert each variant's `bk` against the literal recorded
    /// in `tests/leapr_graphite_deck_parity.rs` and in bead `op-4i5m`, and
    /// assert that the default variant is bit-identical to
    /// [`crate::common::phys::BK_EV_PER_K`] — the property that guarantees this
    /// module changed nothing for existing callers.
    /// **Pass criterion:** exact equality.
    ///
    /// **Result (2026-08-13):** all hold.
    #[test]
    fn constant_sets_carry_the_measured_values() {
        assert_eq!(
            PhysicalConstants::Njoy2016Legacy.bk_ev_per_k(),
            8.617_385e-5
        );
        assert_eq!(PhysicalConstants::Codata2014.bk_ev_per_k(), 8.617_342e-5);
        assert_eq!(
            PhysicalConstants::Codata2018.bk_ev_per_k(),
            8.617_333_262e-5
        );
        assert_eq!(
            PhysicalConstants::default().bk_ev_per_k(),
            crate::common::phys::BK_EV_PER_K,
            "the default must not change the crate's physics"
        );
        // Labels feed cache keys, so they must be distinct.
        let mut labels: Vec<&str> = PhysicalConstants::all().iter().map(|c| c.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), 3, "constant-set labels must be unique");
    }

    /// `EVAL-<MON><YY>` parses out of a real LEAPR comment card, and the
    /// two-digit year pivots at 50.
    ///
    /// **Methodology.** Feed the verbatim ENDF/B-VIII.0 crystalline-graphite
    /// card-20 comment line, plus synthetic lines exercising the pivot and the
    /// rejection paths. **Pass criterion:** SEP17 -> 2017-09; `EVAL-JAN99` ->
    /// 1999; malformed fields -> `None`.
    ///
    /// **Result (2026-08-13):** all hold; the graphite deck's own comment line
    /// yields 2017-09, which is what selects the legacy constant.
    #[test]
    fn eval_field_parses_from_a_comment_card() {
        let graphite = "' Graphite  LEIP LAB   EVAL-SEP17 A.I. Hawari, Y. Zhu, J.L. Wormald'/";
        let d = EvaluationDate::parse_eval_field(graphite).expect("graphite EVAL field");
        assert_eq!(
            d,
            EvaluationDate {
                year: 2017,
                month: 9
            }
        );
        assert_eq!(d.to_string(), "EVAL-SEP17");

        assert_eq!(
            EvaluationDate::parse_eval_field("EVAL-JAN99"),
            Some(EvaluationDate {
                year: 1999,
                month: 1
            })
        );
        assert_eq!(
            EvaluationDate::parse_eval_field("EVAL-DEC05"),
            Some(EvaluationDate {
                year: 2005,
                month: 12
            })
        );
        // A stray "EVAL-" that is not followed by a date must not stop the scan.
        assert_eq!(
            EvaluationDate::parse_eval_field("EVAL-xxx and then EVAL-MAR11"),
            Some(EvaluationDate {
                year: 2011,
                month: 3
            })
        );
        assert_eq!(EvaluationDate::parse_eval_field("no field here"), None);
        assert_eq!(EvaluationDate::parse_eval_field("EVAL-SEP1"), None);
        assert_eq!(EvaluationDate::parse_eval_field("EVAL-ZZZ17"), None);
    }

    /// The vintage -> constant-set mapping honours the one date it is allowed to
    /// know, and falls back to the modern constant afterwards.
    ///
    /// **Methodology.** Check both sides of the 23 Oct 2017 NJOY cutoff and the
    /// ambiguous month itself. **Pass criterion:** anything up to and including
    /// OCT17 selects the legacy constant; NOV17 and later select CODATA2018;
    /// [`PhysicalConstants::Codata2014`] is never auto-selected.
    ///
    /// **Result (2026-08-13):** all hold. SEP17 (the graphite evaluation)
    /// selects the legacy constant, which is what closes the parity.
    #[test]
    fn vintage_selects_the_constant_set() {
        let at =
            |year, month| PhysicalConstants::for_evaluation_date(EvaluationDate { year, month });
        assert_eq!(at(2017, 9), PhysicalConstants::Njoy2016Legacy, "SEP17");
        assert_eq!(at(2017, 10), PhysicalConstants::Njoy2016Legacy, "OCT17");
        assert_eq!(at(2017, 11), PhysicalConstants::Codata2018, "NOV17");
        assert_eq!(at(2011, 1), PhysicalConstants::Njoy2016Legacy, "JAN11");
        assert_eq!(at(2020, 6), PhysicalConstants::Codata2018, "JUN20");
        // CODATA2014 is documented as never auto-selected; hold that property so
        // the doc cannot drift from the code.
        for year in 1960..2050 {
            for month in 1..=12 {
                assert_ne!(
                    at(year, month),
                    PhysicalConstants::Codata2014,
                    "{year}-{month:02} must not auto-select CODATA2014"
                );
            }
        }
    }
}
