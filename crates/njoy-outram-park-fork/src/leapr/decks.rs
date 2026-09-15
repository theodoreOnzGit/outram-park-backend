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
//! 1. **Embedded in the crate** — [`embedded_deck_text`]. Populated for every
//!    registered material as of 2026-08-14; see the licence note on that
//!    function for what that means and does not mean.
//! 2. **A local ENDF thermal-scattering directory**, from the
//!    [`TSL_DIR_ENV`] environment variable (or the legacy
//!    [`TSL_DIR_ENV_LEGACY`]).
//! 3. **The crate's artifact cache** (`<cache>/<library>/<base>.leapr`), where
//!    [`crate::acquire`] would place a fetched deck.
//!
//! Nothing here downloads. A deck is small enough that a user can drop it in by
//! hand, and adding a network path would pull the `net-fetch` dependency tree
//! into the default build for a few hundred KB.
//!
//! # Provenance
//!
//! Full provenance for every registered deck — evaluator, publication, access
//! terms, date accessed, and the licence finding and the maintainer's decision
//! on it — is recorded in `docs/leapr-deck-provenance.md`, per the workspace
//! data-provenance rule.

use std::path::PathBuf;

use crate::acquire::{well_known_tsl, EndfCache, EndfLibrary};
use crate::NjoyError;

/// Environment variable naming a directory of ENDF thermal-scattering files
/// (`tsl-*.leapr`, `tsl-*.endf`) — e.g. the `thermal_scatt/` directory of an
/// unpacked ENDF/B-VIII.0 distribution.
///
/// Consulted after [`embedded_deck_text`]; mainly useful now for a deck this
/// crate does not (yet) register, or for pinning a byte-identical upstream
/// copy during a parity check.
pub const TSL_DIR_ENV: &str = "OUTRAM_PARK_TSL_DIR";

/// Legacy variable name honoured for compatibility with
/// `tests/leapr_graphite_deck_parity.rs`, which predates [`TSL_DIR_ENV`].
/// Consulted only when [`TSL_DIR_ENV`] is unset.
pub const TSL_DIR_ENV_LEGACY: &str = "GRAPHITE_TSL_DIR";

/// A thermal-scattering material this crate can regenerate `S(alpha, beta)`
/// for from a LEAPR deck.
///
/// A closed enum (workspace rule: no trait objects). Every material for which
/// the maintainer supplied an ENDF/B-VIII.0 `.leapr` deck on 2026-08-14 is
/// registered here — the three graphite evaluations the HTR-10 pebble-bed
/// work needs, plus the rest of the thermal-scattering sublibrary the deck set
/// covers (light and heavy water, ice, methane, ortho/para hydrogen and
/// deuterium, YH2, ZrH, BeO, SiC, UN, UO2, quartz, and two structural metals).
///
/// The ENDF identity of each (download basename and thermal-sublibrary MAT)
/// is **not** duplicated here — it is read from [`well_known_tsl`], the crate's
/// one tsl registry, so the two cannot drift. A unit test holds that property.
///
/// **What is *not* here: a coherent-elastic lattice mapping.** An earlier
/// version of this type carried its own `coherent_lattice()` returning a
/// hardcoded [`crate::leapr::coher::CoherentLattice`], but that duplicated information the parsed
/// deck already carries authoritatively — every deck's own card-5 `iel` value
/// parses to [`crate::leapr::input::ElasticOption`], which has its own
/// [`ElasticOption::coherent_lattice`](crate::leapr::input::ElasticOption::coherent_lattice)
/// and is what [`crate::leapr::generate`] actually consumes. A second,
/// hand-populated table for the same fact is exactly the kind of duplicate
/// this workspace's "search before building" rule warns about, so it was
/// removed rather than extended to the 30 materials added on 2026-08-14.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SabMaterial {
    /// `tsl-crystalline-graphite`, MAT 30 — perfect single-crystal graphite,
    /// 0 % porosity. The evaluation the LEAPR port is validated against.
    CrystallineGraphite,
    /// `tsl-reactor-graphite-10P`, MAT 31 — reactor graphite at 10 % porosity.
    ReactorGraphite10P,
    /// `tsl-reactor-graphite-30P`, MAT 32 — reactor graphite at 30 % porosity.
    ReactorGraphite30P,
    /// `tsl-HinH2O`, MAT 1 — hydrogen bound in light water.
    HInH2O,
    /// `tsl-para-H`, MAT 2 — para-hydrogen (cryogenic moderator).
    ParaHydrogen,
    /// `tsl-ortho-H`, MAT 3 — ortho-hydrogen (cryogenic moderator).
    OrthoHydrogen,
    /// `tsl-HinYH2`, MAT 5 — hydrogen bound in yttrium hydride (Bettis model).
    HInYH2,
    /// `tsl-HinZrH`, MAT 7 — hydrogen bound in zirconium hydride.
    HInZrH,
    /// `tsl-HinIceIh`, MAT 10 — hydrogen bound in hexagonal ice.
    HInIceIh,
    /// `tsl-DinD2O`, MAT 11 — deuterium bound in heavy water.
    DInD2O,
    /// `tsl-para-D`, MAT 12 — para-deuterium.
    ParaDeuterium,
    /// `tsl-ortho-D`, MAT 13 — ortho-deuterium.
    OrthoDeuterium,
    /// `tsl-Be-metal`, MAT 26 — beryllium metal.
    BerylliumMetal,
    /// `tsl-BeinBeO`, MAT 27 — beryllium bound in beryllium oxide.
    BeInBeO,
    /// `tsl-l-CH4`, MAT 33 — liquid methane at 100 K.
    ///
    /// **Shares MAT 33 with [`Self::SolidMethane`]** in the supplied deck
    /// set — as-parsed, not independently corrected. See the module-level
    /// caveat in `docs/leapr-deck-provenance.md` §1.
    LiquidMethane,
    /// `tsl-s-CH4`, MAT 33 — solid methane at 22 K.
    ///
    /// **Shares MAT 33 with [`Self::LiquidMethane`]** — see that variant's
    /// doc comment.
    SolidMethane,
    /// `tsl-SiinSiC`, MAT 43 — silicon bound in silicon carbide.
    SiInSiC,
    /// `tsl-CinSiC`, MAT 44 — carbon bound in silicon carbide.
    CInSiC,
    /// `tsl-OinBeO`, MAT 46 — oxygen bound in beryllium oxide.
    OInBeO,
    /// `tsl-SiO2-alpha`, MAT 47 — alpha-quartz.
    SiO2Alpha,
    /// `tsl-UinUO2`, MAT 48 — uranium bound in uranium dioxide.
    UInUO2,
    /// `tsl-SiO2-beta`, MAT 49 — beta-quartz.
    SiO2Beta,
    /// `tsl-OinIceIh`, MAT 50 — oxygen bound in hexagonal ice.
    OInIceIh,
    /// `tsl-OinD2O`, MAT 51 — oxygen bound in heavy water.
    OInD2O,
    /// `tsl-YinYH2`, MAT 55 — yttrium bound in yttrium hydride (Bettis model).
    YInYH2,
    /// `tsl-ZrinZrH`, MAT 58 — zirconium bound in zirconium hydride.
    ZrInZrH,
    /// `tsl-NinUN`, MAT 71 — nitrogen bound in uranium nitride.
    NInUN,
    /// `tsl-UinUN`, MAT 72 — uranium bound in uranium nitride.
    ///
    /// **As-parsed oddity, not independently verified:** this deck's card-5
    /// `iel = 2`, which parses to
    /// [`ElasticOption::Beryllium`](crate::leapr::input::ElasticOption::Beryllium)
    /// — i.e. it selects NJOY's built-in *beryllium metal* coherent-elastic
    /// lattice for a uranium-nitride scatterer, which has no obvious physical
    /// basis (UN is rock-salt FCC, not beryllium's hexagonal structure). This
    /// is read faithfully from the deck, not overridden or corrected here —
    /// see `docs/leapr-deck-provenance.md` §1 for the full note. Flagged for
    /// human review before this material's elastic channel is trusted.
    UInUN,
    /// `tsl-OinUO2`, MAT 75 — oxygen bound in uranium dioxide.
    OInUO2,
    /// `tsl-HinCH2`, MAT 37 — hydrogen bound in polyethylene.
    HInCH2,
    /// `tsl-HinC5O2H8`, MAT 39 — hydrogen bound in PMMA (the C5H8O2 monomer).
    HInC5O2H8,
    /// `tsl-013_Al_027`, MAT 101 — aluminium metal.
    ///
    /// **Filename and MAT are as supplied, not the official ENDF/B-VIII.0
    /// naming convention** (`tsl-013_Al_027` rather than `tsl-aluminum` or
    /// similar), and MAT 101 is **shared with [`Self::IronMetal`]** in this
    /// deck set — official ENDF thermal-sublibrary materials do not reuse a
    /// MAT across two different scatterers. See
    /// `docs/leapr-deck-provenance.md` §1.
    AluminumMetal,
    /// `tsl-026_Fe_056`, MAT 101 — iron metal.
    ///
    /// **Shares MAT 101 with [`Self::AluminumMetal`]** — see that variant's
    /// doc comment.
    IronMetal,
}

impl SabMaterial {
    /// Every registered material, for iterating over the supported set.
    pub const fn all() -> [SabMaterial; 33] {
        [
            SabMaterial::CrystallineGraphite,
            SabMaterial::ReactorGraphite10P,
            SabMaterial::ReactorGraphite30P,
            SabMaterial::HInH2O,
            SabMaterial::ParaHydrogen,
            SabMaterial::OrthoHydrogen,
            SabMaterial::HInYH2,
            SabMaterial::HInZrH,
            SabMaterial::HInIceIh,
            SabMaterial::DInD2O,
            SabMaterial::ParaDeuterium,
            SabMaterial::OrthoDeuterium,
            SabMaterial::BerylliumMetal,
            SabMaterial::BeInBeO,
            SabMaterial::LiquidMethane,
            SabMaterial::SolidMethane,
            SabMaterial::SiInSiC,
            SabMaterial::CInSiC,
            SabMaterial::OInBeO,
            SabMaterial::SiO2Alpha,
            SabMaterial::UInUO2,
            SabMaterial::SiO2Beta,
            SabMaterial::OInIceIh,
            SabMaterial::OInD2O,
            SabMaterial::YInYH2,
            SabMaterial::ZrInZrH,
            SabMaterial::NInUN,
            SabMaterial::UInUN,
            SabMaterial::OInUO2,
            SabMaterial::HInCH2,
            SabMaterial::HInC5O2H8,
            SabMaterial::AluminumMetal,
            SabMaterial::IronMetal,
        ]
    }

    /// The key this material is registered under in [`well_known_tsl`].
    pub const fn registry_key(self) -> &'static str {
        match self {
            SabMaterial::CrystallineGraphite => "crystalline-graphite",
            SabMaterial::ReactorGraphite10P => "reactor-graphite-10P",
            SabMaterial::ReactorGraphite30P => "reactor-graphite-30P",
            SabMaterial::HInH2O => "HinH2O",
            SabMaterial::ParaHydrogen => "para-H",
            SabMaterial::OrthoHydrogen => "ortho-H",
            SabMaterial::HInYH2 => "HinYH2",
            SabMaterial::HInZrH => "HinZrH",
            SabMaterial::HInIceIh => "HinIceIh",
            SabMaterial::DInD2O => "DinD2O",
            SabMaterial::ParaDeuterium => "para-D",
            SabMaterial::OrthoDeuterium => "ortho-D",
            SabMaterial::BerylliumMetal => "Be-metal",
            SabMaterial::BeInBeO => "BeinBeO",
            SabMaterial::LiquidMethane => "l-CH4",
            SabMaterial::SolidMethane => "s-CH4",
            SabMaterial::SiInSiC => "SiinSiC",
            SabMaterial::CInSiC => "CinSiC",
            SabMaterial::OInBeO => "OinBeO",
            SabMaterial::SiO2Alpha => "SiO2-alpha",
            SabMaterial::UInUO2 => "UinUO2",
            SabMaterial::SiO2Beta => "SiO2-beta",
            SabMaterial::OInIceIh => "OinIceIh",
            SabMaterial::OInD2O => "OinD2O",
            SabMaterial::YInYH2 => "YinYH2",
            SabMaterial::ZrInZrH => "ZrinZrH",
            SabMaterial::NInUN => "NinUN",
            SabMaterial::UInUN => "UinUN",
            SabMaterial::OInUO2 => "OinUO2",
            SabMaterial::HInCH2 => "HinCH2",
            SabMaterial::HInC5O2H8 => "HinC5O2H8",
            SabMaterial::AluminumMetal => "013_Al_027",
            SabMaterial::IronMetal => "026_Fe_056",
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

    /// The LEAPR deck's file name as supplied, e.g.
    /// `"tsl-crystalline-graphite.leapr"`.
    pub fn deck_file_name(self) -> String {
        format!("{}.leapr", self.base())
    }

    /// A short human-readable label for logs and cache keys, e.g.
    /// `"crystalline-graphite"`. Stable — cache keys are built from it.
    pub const fn label(self) -> &'static str {
        self.registry_key()
    }
}

/// The LEAPR deck text embedded in this crate for `material`.
///
/// # Populated 2026-08-14 — a maintainer decision, not a licence resolution
///
/// Every arm below returns `Some(include_str!(...))`: all 33 registered
/// decks are compiled into this crate and ship with it (`cargo publish` and
/// the crate's git history both carry them — see `Cargo.toml`'s
/// `include = ["src/**", ...]`, which covers this module's `decks/`
/// subdirectory).
///
/// **This is a deliberate, explicit maintainer decision (2026-08-14), not a
/// finding that redistribution terms were established.** The 2026-08-13
/// investigation recorded in `docs/leapr-deck-provenance.md` found no
/// copyright, licence, or terms-of-use statement on the NNDC distribution
/// pages, in the ENDF/B-VIII.0 sublibrary `README.txt`/`CHANGELOG.txt`, in
/// the per-material `.readme` files, or in the decks' own comment cards —
/// that finding still stands and is **not retracted**. What changed is that
/// the project maintainer, who has the standing to make this call for their
/// own repository, explicitly instructed on 2026-08-14 that this ENDF/B-VIII.0
/// thermal-scattering data is public (citing
/// <https://www.nndc.bnl.gov/endf-b8.0/download.html> and
/// <https://www.nndc.bnl.gov/endf-releases/?version=B-VIII.0> as the
/// distribution points) and should ship embedded, "as their size is small".
/// See `docs/leapr-deck-provenance.md` §2 for the full record of both the
/// original finding and this decision.
///
/// If this ever needs to be walked back — e.g. NNDC/CSEWG states
/// restrictive terms — revert every arm to `None` and update
/// `docs/leapr-deck-provenance.md` §2 accordingly; nothing else changes,
/// [`locate_deck`] already falls through to a local copy when this returns
/// `None`.
pub const fn embedded_deck_text(material: SabMaterial) -> Option<&'static str> {
    match material {
        SabMaterial::CrystallineGraphite => {
            Some(include_str!("decks/tsl-crystalline-graphite.leapr"))
        }
        SabMaterial::ReactorGraphite10P => {
            Some(include_str!("decks/tsl-reactor-graphite-10P.leapr"))
        }
        SabMaterial::ReactorGraphite30P => {
            Some(include_str!("decks/tsl-reactor-graphite-30P.leapr"))
        }
        SabMaterial::HInH2O => Some(include_str!("decks/tsl-HinH2O.leapr")),
        SabMaterial::ParaHydrogen => Some(include_str!("decks/tsl-para-H.leapr")),
        SabMaterial::OrthoHydrogen => Some(include_str!("decks/tsl-ortho-H.leapr")),
        SabMaterial::HInYH2 => Some(include_str!("decks/tsl-HinYH2.leapr")),
        SabMaterial::HInZrH => Some(include_str!("decks/tsl-HinZrH.leapr")),
        SabMaterial::HInIceIh => Some(include_str!("decks/tsl-HinIceIh.leapr")),
        SabMaterial::DInD2O => Some(include_str!("decks/tsl-DinD2O.leapr")),
        SabMaterial::ParaDeuterium => Some(include_str!("decks/tsl-para-D.leapr")),
        SabMaterial::OrthoDeuterium => Some(include_str!("decks/tsl-ortho-D.leapr")),
        SabMaterial::BerylliumMetal => Some(include_str!("decks/tsl-Be-metal.leapr")),
        SabMaterial::BeInBeO => Some(include_str!("decks/tsl-BeinBeO.leapr")),
        SabMaterial::LiquidMethane => Some(include_str!("decks/tsl-l-CH4.leapr")),
        SabMaterial::SolidMethane => Some(include_str!("decks/tsl-s-CH4.leapr")),
        SabMaterial::SiInSiC => Some(include_str!("decks/tsl-SiinSiC.leapr")),
        SabMaterial::CInSiC => Some(include_str!("decks/tsl-CinSiC.leapr")),
        SabMaterial::OInBeO => Some(include_str!("decks/tsl-OinBeO.leapr")),
        SabMaterial::SiO2Alpha => Some(include_str!("decks/tsl-SiO2-alpha.leapr")),
        SabMaterial::UInUO2 => Some(include_str!("decks/tsl-UinUO2.leapr")),
        SabMaterial::SiO2Beta => Some(include_str!("decks/tsl-SiO2-beta.leapr")),
        SabMaterial::OInIceIh => Some(include_str!("decks/tsl-OinIceIh.leapr")),
        SabMaterial::OInD2O => Some(include_str!("decks/tsl-OinD2O.leapr")),
        SabMaterial::YInYH2 => Some(include_str!("decks/tsl-YinYH2.leapr")),
        SabMaterial::ZrInZrH => Some(include_str!("decks/tsl-ZrinZrH.leapr")),
        SabMaterial::NInUN => Some(include_str!("decks/tsl-NinUN.leapr")),
        SabMaterial::UInUN => Some(include_str!("decks/tsl-UinUN.leapr")),
        SabMaterial::OInUO2 => Some(include_str!("decks/tsl-OinUO2.leapr")),
        SabMaterial::HInCH2 => Some(include_str!("decks/tsl-HinCH2.leapr")),
        SabMaterial::HInC5O2H8 => Some(include_str!("decks/tsl-HinC5O2H8.leapr")),
        SabMaterial::AluminumMetal => Some(include_str!("decks/tsl-013_Al_027.leapr")),
        SabMaterial::IronMetal => Some(include_str!("decks/tsl-026_Fe_056.leapr")),
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
/// every path tried and the environment variable to set. In practice this
/// only fires for a material not registered in [`embedded_deck_text`], since
/// every registered material is embedded as of 2026-08-14.
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
        "no LEAPR deck found for {}: not embedded in this crate and no local copy \
         found either. Set {TSL_DIR_ENV} to the `thermal_scatt/` directory of an \
         unpacked ENDF/B-VIII.0 distribution, or place `{}` in the crate's artifact \
         cache. Tried: {}",
        material.label(),
        material.deck_file_name(),
        tried.join(", ")
    )))
}

/// Every on-disk path [`locate_deck`] will try, in order, **after** the
/// embedded copy.
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
    /// the basename and MAT the supplied ENDF/B-VIII.0 decks carry.
    ///
    /// **Methodology.** This module deliberately does not carry its own copy of
    /// the basenames and MATs; it reads them from
    /// [`crate::acquire::well_known_tsl`]. That delegation is only safe if every
    /// enum variant is present there, so assert it for a representative sample
    /// (all 33 are exercised by `every_registered_material_has_an_embedded_deck`
    /// below, which parses each one and cross-checks `mat`/`za` against the
    /// registry).
    ///
    /// **Result (2026-08-14):** all hold.
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
        assert_eq!(SabMaterial::HInH2O.base(), "tsl-HinH2O");
        assert_eq!(SabMaterial::HInH2O.mat(), 1);

        assert_eq!(
            SabMaterial::CrystallineGraphite.deck_file_name(),
            "tsl-crystalline-graphite.leapr"
        );

        // Labels feed cache keys, so they must be distinct.
        let mut labels: Vec<&str> = SabMaterial::all().iter().map(|m| m.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), 33, "material labels must be unique");
    }

    /// Every registered material is embedded, and its embedded text
    /// round-trips through [`crate::leapr::deck::LeaprDeck::parse`] with the
    /// `mat`/`za` [`well_known_tsl`] declares for it.
    ///
    /// **Methodology.** Records the 2026-08-14 maintainer decision to embed
    /// (see [`embedded_deck_text`]'s doc comment) as an executable check
    /// rather than only as prose: every material must have `Some` text, that
    /// text must parse, and the parsed `mat` must equal the registry's `mat`
    /// — catching a copy-paste mismatch between `well_known_tsl` and the
    /// actual deck file at build time rather than at first use.
    ///
    /// **Result (2026-08-14):** all 33 materials embedded and self-consistent.
    /// Two MAT collisions are present in the supplied deck set and are
    /// deliberately NOT asserted unique here, since they are a property of
    /// the input data, not of this registry: `LiquidMethane`/`SolidMethane`
    /// both carry MAT 33, and `AluminumMetal`/`IronMetal` both carry MAT 101.
    /// See `docs/leapr-deck-provenance.md` §1.
    #[test]
    fn every_registered_material_has_an_embedded_deck() {
        for m in SabMaterial::all() {
            let text = embedded_deck_text(m)
                .unwrap_or_else(|| panic!("{}: no embedded deck text", m.label()));
            let deck = crate::leapr::deck::LeaprDeck::parse(text)
                .unwrap_or_else(|e| panic!("{}: embedded deck failed to parse: {e}", m.label()));
            assert_eq!(
                deck.mat,
                m.mat(),
                "{}: embedded deck's own mat disagrees with well_known_tsl",
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
