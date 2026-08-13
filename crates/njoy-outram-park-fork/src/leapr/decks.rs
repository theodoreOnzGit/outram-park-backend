//! LEAPR **card-deck registry** — which decks this crate knows how to
//! regenerate `S(alpha, beta)` from, and where their text comes from.
//!
//! A LEAPR deck is the ~12 KB job that produced a multi-MB ENDF
//! thermal-scattering tape. Keeping the deck instead of the tape is a ~700x
//! storage saving *and* buys the ability to generate at temperatures the tape
//! never tabulated — see [`crate::leapr::generate`] for the measured figures.
//! This module answers only two questions: **which decks** and **from where**.
//!
//! # Where the deck text comes from
//!
//! [`locate_deck`] tries, in order:
//!
//! 1. **Embedded in the crate** — [`embedded_deck_text`]. **Currently empty for
//!    every material**; see the licence note on that function. When populated,
//!    this makes regeneration work with no external data at all.
//! 2. **A local ENDF thermal-scattering directory**, from the
//!    [`TSL_DIR_ENV`] environment variable (or the legacy
//!    [`TSL_DIR_ENV_LEGACY`]).
//! 3. **The crate's artifact cache** (`<cache>/<library>/<base>.leapr`), where
//!    [`crate::acquire`] would place a fetched deck.
//!
//! Nothing here downloads. A deck is small enough that a user can drop it in by
//! hand, and adding a network path would pull the `net-fetch` dependency tree
//! into the default build for 12 KB.
//!
//! # Provenance
//!
//! Full provenance for every registered deck — evaluator, publication, access
//! terms, date accessed, and the licence finding that currently keeps
//! [`embedded_deck_text`] empty — is recorded in
//! `docs/leapr-deck-provenance.md`, per the workspace data-provenance rule.

use std::path::PathBuf;

use crate::acquire::{well_known_tsl, EndfCache, EndfLibrary};
use crate::leapr::coher::CoherentLattice;
use crate::NjoyError;

/// Environment variable naming a directory of ENDF thermal-scattering files
/// (`tsl-*.leapr`, `tsl-*.endf`) — e.g. the `thermal_scatt/` directory of an
/// unpacked ENDF/B-VIII.0 distribution.
///
/// This is the supported way to point the generator at decks while
/// [`embedded_deck_text`] is empty.
pub const TSL_DIR_ENV: &str = "OUTRAM_PARK_TSL_DIR";

/// Legacy variable name honoured for compatibility with
/// `tests/leapr_graphite_deck_parity.rs`, which predates [`TSL_DIR_ENV`].
/// Consulted only when [`TSL_DIR_ENV`] is unset.
pub const TSL_DIR_ENV_LEGACY: &str = "GRAPHITE_TSL_DIR";

/// A thermal-scattering material this crate can regenerate `S(alpha, beta)`
/// for from a LEAPR deck.
///
/// A closed enum (workspace rule: no trait objects). Today it holds the three
/// ENDF/B-VIII.0 graphite evaluations, which are what the HTR-10 pebble-bed
/// work needs: the perfect crystal and the two porous reactor grades.
///
/// The ENDF identity of each (download basename and thermal-sublibrary MAT)
/// is **not** duplicated here — it is read from [`well_known_tsl`], the crate's
/// one tsl registry, so the two cannot drift. A unit test holds that property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SabMaterial {
    /// `tsl-crystalline-graphite`, MAT 30 — perfect single-crystal graphite,
    /// 0 % porosity. The evaluation the LEAPR port is validated against.
    CrystallineGraphite,
    /// `tsl-reactor-graphite-10P`, MAT 31 — reactor graphite at 10 % porosity.
    ReactorGraphite10P,
    /// `tsl-reactor-graphite-30P`, MAT 32 — reactor graphite at 30 % porosity.
    ReactorGraphite30P,
}

impl SabMaterial {
    /// Every registered material, for iterating over the supported set.
    pub const fn all() -> [SabMaterial; 3] {
        [
            SabMaterial::CrystallineGraphite,
            SabMaterial::ReactorGraphite10P,
            SabMaterial::ReactorGraphite30P,
        ]
    }

    /// The key this material is registered under in [`well_known_tsl`].
    pub const fn registry_key(self) -> &'static str {
        match self {
            SabMaterial::CrystallineGraphite => "crystalline-graphite",
            SabMaterial::ReactorGraphite10P => "reactor-graphite-10P",
            SabMaterial::ReactorGraphite30P => "reactor-graphite-30P",
        }
    }

    /// The ENDF thermal-sublibrary basename, e.g.
    /// `"tsl-crystalline-graphite"`. Delegates to [`well_known_tsl`].
    ///
    /// # Panics
    /// If the material is missing from [`well_known_tsl`] — a programming error
    /// in this crate, held down by `every_material_resolves_in_the_tsl_registry`.
    pub fn base(self) -> &'static str {
        well_known_tsl(self.registry_key())
            .expect("registered SabMaterial must exist in well_known_tsl")
            .base
    }

    /// The ENDF thermal-sublibrary material number (MF=7 MAT). Delegates to
    /// [`well_known_tsl`]; see [`Self::base`] for the panic condition.
    pub fn mat(self) -> i32 {
        well_known_tsl(self.registry_key())
            .expect("registered SabMaterial must exist in well_known_tsl")
            .mat
    }

    /// The LEAPR deck's file name as ENDF/B-VIII.0 distributes it, e.g.
    /// `"tsl-crystalline-graphite.leapr"`.
    pub fn deck_file_name(self) -> String {
        format!("{}.leapr", self.base())
    }

    /// The coherent-elastic lattice this material scatters from, for
    /// [`crate::leapr::coher::coher`].
    ///
    /// All three graphite evaluations use the graphite lattice; the porous
    /// grades differ in their phonon spectrum and free-atom cross section (deck
    /// contents), not in their crystal structure.
    pub const fn coherent_lattice(self) -> CoherentLattice {
        match self {
            SabMaterial::CrystallineGraphite
            | SabMaterial::ReactorGraphite10P
            | SabMaterial::ReactorGraphite30P => CoherentLattice::Graphite,
        }
    }

    /// A short human-readable label for logs and cache keys, e.g.
    /// `"crystalline-graphite"`. Stable — cache keys are built from it.
    pub const fn label(self) -> &'static str {
        self.registry_key()
    }
}

/// The LEAPR deck text embedded in this crate for `material`, if any.
///
/// # Currently `None` for every material — a deliberate licence hold
///
/// Embedding the three ENDF/B-VIII.0 `tsl-*graphite*.leapr` decks (~40 KB
/// total) would make regeneration work with no external data whatsoever, and
/// that is the intended end state. It has **not** been done because the
/// redistribution terms of those files could not be established:
///
/// - Neither the sublibrary `README.txt`, nor the per-material `.readme`, nor
///   the `CHANGELOG.txt` shipped with ENDF/B-VIII.0 carries any copyright,
///   licence, or terms-of-use statement (checked 2026-08-13).
/// - The NNDC ENDF pages carry no licence or redistribution statement either
///   (checked 2026-08-13). Public hosting is not a grant of redistribution
///   rights.
///
/// The workspace `DATA_POLICY.md` rule is directional: unclear terms mean *do
/// not ship it*. Shipping it wrongly is a licence violation in a public GPL-3.0
/// repository; not shipping it is a one-line change to undo. So the arms below
/// return `None`, and [`locate_deck`] falls through to a local copy the user
/// already has.
///
/// **To populate, once terms are established:** drop the deck files into
/// `src/leapr/decks/`, replace the relevant arm with
/// `Some(include_str!("decks/tsl-crystalline-graphite.leapr"))`, and record the
/// established terms in `docs/leapr-deck-provenance.md`. Nothing else changes —
/// [`locate_deck`] already prefers the embedded copy.
pub const fn embedded_deck_text(material: SabMaterial) -> Option<&'static str> {
    match material {
        SabMaterial::CrystallineGraphite => None,
        SabMaterial::ReactorGraphite10P => None,
        SabMaterial::ReactorGraphite30P => None,
    }
}

/// Where a located deck's text came from — recorded so a generated artifact's
/// provenance can be reported rather than guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeckSource {
    /// Compiled into this crate by [`embedded_deck_text`].
    Embedded,
    /// Read from a file on disk.
    File(PathBuf),
}

impl std::fmt::Display for DeckSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeckSource::Embedded => write!(f, "embedded"),
            DeckSource::File(p) => write!(f, "file:{}", p.display()),
        }
    }
}

/// A LEAPR deck's text plus where it was found.
#[derive(Debug, Clone)]
pub struct LocatedDeck {
    /// The raw deck text, ready for
    /// [`crate::leapr::deck::LeaprDeck::parse`].
    pub text: String,
    /// Where the text came from.
    pub source: DeckSource,
}

/// Find the LEAPR deck for `material`, following the resolution order in the
/// [module docs](self).
///
/// # Errors
///
/// [`NjoyError::Download`] when no copy can be found, with a message naming
/// every path tried and the environment variable to set. That message is the
/// user-facing consequence of the licence hold on [`embedded_deck_text`], so it
/// is written to be actionable rather than terse.
pub fn locate_deck(material: SabMaterial) -> Result<LocatedDeck, NjoyError> {
    if let Some(text) = embedded_deck_text(material) {
        return Ok(LocatedDeck {
            text: text.to_string(),
            source: DeckSource::Embedded,
        });
    }

    let mut tried: Vec<String> = Vec::new();
    for path in candidate_deck_paths(material) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                return Ok(LocatedDeck {
                    text,
                    source: DeckSource::File(path),
                })
            }
            Err(_) => tried.push(path.display().to_string()),
        }
    }

    Err(NjoyError::Download(format!(
        "no LEAPR deck found for {}: this crate does not embed the ENDF/B-VIII.0 \
         `.leapr` decks (their redistribution terms are unestablished — see \
         `leapr::decks::embedded_deck_text`), so it needs a local copy of \
         `{}`. Set {TSL_DIR_ENV} to the `thermal_scatt/` directory of an unpacked \
         ENDF/B-VIII.0 distribution, or place the file in the crate's artifact \
         cache. Tried: {}",
        material.label(),
        material.deck_file_name(),
        tried.join(", ")
    )))
}

/// Every on-disk path [`locate_deck`] will try, in order.
///
/// Exposed so a caller (or a diagnostic) can report what would be searched
/// without attempting the read.
pub fn candidate_deck_paths(material: SabMaterial) -> Vec<PathBuf> {
    let file = material.deck_file_name();
    let mut out = Vec::new();

    for var in [TSL_DIR_ENV, TSL_DIR_ENV_LEGACY] {
        if let Some(dir) = std::env::var_os(var) {
            out.push(PathBuf::from(dir).join(&file));
        }
    }

    // Where `acquire` lays out ENDF/B-VIII.0 artifacts, if a cache directory is
    // discoverable. `default_dir` is used rather than `EndfCache::new` so that
    // merely asking what would be searched does not create a directory.
    if let Some(dir) = EndfCache::default_dir() {
        out.push(dir.join(EndfLibrary::EndfBVIII0.dir()).join(&file));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every registered material resolves in the crate's one tsl registry, with
    /// the basename and MAT the ENDF/B-VIII.0 distribution publishes.
    ///
    /// **Methodology.** This module deliberately does not carry its own copy of
    /// the basenames and MATs; it reads them from
    /// [`crate::acquire::well_known_tsl`]. That delegation is only safe if every
    /// enum variant is present there, so assert it — and assert the published
    /// values, so a wrong edit to either side fails here rather than 404ing a
    /// download or mislabelling a tape.
    /// **Pass criterion:** all three resolve; basenames and MATs match the
    /// ENDF/B-VIII.0 thermal sublibrary (30/31/32); deck file names are the
    /// distributed `.leapr` names.
    ///
    /// **Result (2026-08-13):** all hold.
    #[test]
    fn every_material_resolves_in_the_tsl_registry() {
        assert_eq!(
            SabMaterial::CrystallineGraphite.base(),
            "tsl-crystalline-graphite"
        );
        assert_eq!(SabMaterial::CrystallineGraphite.mat(), 30);
        assert_eq!(
            SabMaterial::ReactorGraphite10P.base(),
            "tsl-reactor-graphite-10P"
        );
        assert_eq!(SabMaterial::ReactorGraphite10P.mat(), 31);
        assert_eq!(
            SabMaterial::ReactorGraphite30P.base(),
            "tsl-reactor-graphite-30P"
        );
        assert_eq!(SabMaterial::ReactorGraphite30P.mat(), 32);

        assert_eq!(
            SabMaterial::CrystallineGraphite.deck_file_name(),
            "tsl-crystalline-graphite.leapr"
        );

        // Labels feed cache keys, so they must be distinct.
        let mut labels: Vec<&str> = SabMaterial::all().iter().map(|m| m.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), 3, "material labels must be unique");
    }

    /// The licence hold is real and visible in code, not only in prose.
    ///
    /// **Methodology.** Assert that no material currently has embedded deck
    /// text. This test is a tripwire: when the maintainer establishes
    /// redistribution terms and populates [`embedded_deck_text`], it fails, and
    /// whoever does that must consciously update this test *and*
    /// `docs/leapr-deck-provenance.md` in the same change.
    /// **Pass criterion:** every variant yields `None`.
    ///
    /// **Result (2026-08-13):** all three are `None`, matching the documented
    /// finding that the ENDF/B-VIII.0 distribution states no terms.
    #[test]
    fn no_deck_is_embedded_while_the_licence_question_is_open() {
        for m in SabMaterial::all() {
            assert!(
                embedded_deck_text(m).is_none(),
                "{}: a deck was embedded — establish and record its redistribution \
                 terms in docs/leapr-deck-provenance.md, then update this test",
                m.label()
            );
        }
    }

    /// The search path honours both environment variables, in the documented
    /// order, and names the distributed deck file.
    ///
    /// **Methodology.** Set [`TSL_DIR_ENV`] to a temporary directory and check
    /// the first candidate is `<dir>/tsl-crystalline-graphite.leapr`. Env vars
    /// are process-global, so this test sets and clears its own.
    /// **Pass criterion:** the override appears first and carries the right file
    /// name.
    ///
    /// **Result (2026-08-13):** holds.
    #[test]
    fn deck_search_path_honours_the_env_override() {
        let dir = std::env::temp_dir().join("op_leapr_deck_search_test");
        // SAFETY (std >= 1.87 marks this unsafe): single-threaded within this
        // test, and the variable is read only by `candidate_deck_paths` below.
        std::env::set_var(TSL_DIR_ENV, &dir);
        let paths = candidate_deck_paths(SabMaterial::CrystallineGraphite);
        std::env::remove_var(TSL_DIR_ENV);

        assert!(!paths.is_empty(), "at least the env override must be tried");
        assert_eq!(
            paths[0],
            dir.join("tsl-crystalline-graphite.leapr"),
            "the {TSL_DIR_ENV} override must be searched first"
        );
    }
}
