//! **Name → [`Component`] lookup** over the crate's reference-compound presets.
//!
//! # What this is for
//!
//! Every thermodynamic routine in this crate — the cubic EOS, the flash family,
//! the column energy balance — takes a `&[Component]` slate: critical
//! temperature `Tc` \[K\], critical pressure `Pc` \[Pa\], acentric factor `ω`
//! \[-\], molar mass `M` \[kg/mol\], ideal-gas Cp coefficients. A flowsheet, by
//! contrast, identifies a compound only by a **name string**
//! ([`crate::flowsheet::streams::StreamCompound::name`]). Until this module
//! there was no way to get from one to the other: the only source of
//! `Component` values was the hand-written presets in
//! [`crate::thermo::component::reference`], reached by calling a Rust function
//! by name at compile time.
//!
//! This module closes that gap for the compounds that **already have data in
//! this repository**, and turns every other compound into a *named, explicit
//! error* rather than a silent gap discovered three layers down.
//!
//! # ⚠️ Coverage: seven compounds, and that is the whole registry
//!
//! [`ReferenceCompound`] has exactly **seven** variants — water, methane,
//! ethane, nitrogen, carbon dioxide, benzene, toluene — because those are the
//! only seven presets in [`crate::thermo::component::reference`]. Asking for
//! anything else (propane, n-butane, ammonia, oxygen, …) returns
//! [`ComponentLookupError::UnknownCompound`]. **A working mechanism is not
//! working coverage.** Do not read "the lookup resolves" as "this crate can
//! model your mixture".
//!
//! Expanding the registry is deliberately *not* done here. Compound
//! constant-property data is a `DATA_POLICY.md` provenance question (the
//! obvious upstream source, ChemSep's database as bundled with DWSIM, has
//! licence terms that have not been put to the maintainer) **before** it is a
//! porting question. Inventing plausible-looking `Tc`/`Pc`/`ω` values would be
//! silently wrong and unfalsifiable. When that question is settled, the single
//! place to add data is [`crate::thermo::component::reference`] plus one
//! variant here — the compiler then forces every `match` in this module to
//! account for it, which is exactly why the registry is an enum and not a map.
//!
//! # Matching rule — normalised, case-insensitive, formula-aware
//!
//! See [`ReferenceCompound::from_name`] for the rule, the normalisation it
//! applies, and why it departs from upstream DWSIM's exact-match dictionary.
//!
//! # Units
//!
//! This module moves whole [`Component`] records around and computes no
//! physical quantity of its own. The one unit it must be careful about is molar
//! mass, because the two sides disagree on the prefix:
//!
//! | Source | Field | Unit |
//! |---|---|---|
//! | [`Component::molar_mass`] | `M` | **kg/mol** |
//! | [`crate::flowsheet::streams::StreamCompound::molar_mass`] | `M` | **kg/kmol** (= g/mol), DWSIM's internal convention |
//!
//! The factor of 1000 between them is the reason
//! [`crate::flowsheet::component_basis`] exists as a separate, explicitly
//! converting bridge rather than a naive field copy.
//!
//! # Attribution
//!
//! Structural reference (not data): **DWSIM**
//! (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright 2008-2024 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, not
//! the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Upstream shape consulted: `DWSIM.FlowsheetBase/FlowsheetBase.vb:3098`
//! (`AvailableCompounds As New Dictionary(Of String, ICompoundConstantProperties)`),
//! `:3954` (`GetCompound(name) → AvailableCompounds(name)`) and `:4319`
//! (`AddCompound`). That is a plain `Dictionary` with the default ordinal
//! comparer, keyed by the compound's `Name`, which raises
//! `KeyNotFoundException` on a miss. **No property data was copied from
//! upstream**; only the observation that upstream resolves a compound by an
//! exact name key, which this module deliberately relaxes (see
//! [`ReferenceCompound::from_name`]).

use crate::thermo::component::{reference, Component};

/// The compounds this crate has constant-property data for — the complete
/// contents of [`crate::thermo::component::reference`], as a closed enum.
///
/// An enum rather than a map because the set is closed and known at compile
/// time (workspace `CLAUDE.md`, "No trait objects — use enums for dispatch"):
/// adding a preset forces every `match` here to be updated, so the alias table,
/// the canonical-name table and [`ReferenceCompound::ALL`] cannot silently fall
/// out of step with the data.
///
/// Each variant carries no payload — it *names* a compound; call
/// [`ReferenceCompound::component`] to get the [`Component`] record with its
/// critical constants (`Tc` \[K\], `Pc` \[Pa\], `Vc` \[m³/mol\]), acentric
/// factor `ω` \[-\], molar mass `M` \[kg/mol\], normal boiling point `Tb` \[K\]
/// and ideal-gas Cp coefficients.
///
/// **Only benzene and toluene carry real ideal-gas Cp coefficients**; the other
/// five have `0.0` placeholders (documented in
/// [`crate::thermo::component::reference`]). A resolved component is therefore
/// usable for EOS `a(T)`/`b` and K-values, but its ideal-gas enthalpy is zero
/// unless it is one of those two — that is a property of the underlying data,
/// not of this lookup, and this module does not paper over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReferenceCompound {
    /// Water (H₂O), `Tc` = 647.14 K, `Pc` = 22.064 MPa, `ω` = 0.344.
    Water,
    /// Methane (CH₄), `Tc` = 190.56 K, `Pc` = 4.599 MPa, `ω` = 0.011.
    Methane,
    /// Ethane (C₂H₆), `Tc` = 305.32 K, `Pc` = 4.872 MPa, `ω` = 0.099.
    Ethane,
    /// Nitrogen (N₂), `Tc` = 126.20 K, `Pc` = 3.398 MPa, `ω` = 0.037.
    Nitrogen,
    /// Carbon dioxide (CO₂), `Tc` = 304.12 K, `Pc` = 7.374 MPa, `ω` = 0.225.
    CarbonDioxide,
    /// Benzene (C₆H₆), `Tc` = 562.05 K, `Pc` = 48.95 bar, `ω` = 0.210. Carries
    /// real ideal-gas Cp coefficients.
    Benzene,
    /// Toluene (C₇H₈), `Tc` = 591.75 K, `Pc` = 41.08 bar, `ω` = 0.264. Carries
    /// real ideal-gas Cp coefficients.
    Toluene,
}

impl ReferenceCompound {
    /// Every compound the registry can resolve, in a stable order (the
    /// declaration order of [`crate::thermo::component::reference`]).
    ///
    /// **Length 7.** This array is the registry's entire coverage; see the
    /// module docs before reading that as sufficiency for a real mixture.
    pub const ALL: [ReferenceCompound; 7] = [
        ReferenceCompound::Water,
        ReferenceCompound::Methane,
        ReferenceCompound::Ethane,
        ReferenceCompound::Nitrogen,
        ReferenceCompound::CarbonDioxide,
        ReferenceCompound::Benzene,
        ReferenceCompound::Toluene,
    ];

    /// The canonical spelling of this compound's name — byte-identical to the
    /// [`Component::name`] the preset constructs, so a resolved slate's names
    /// round-trip through [`ReferenceCompound::from_name`].
    ///
    /// Note the preset spells carbon dioxide `"CarbonDioxide"` (no space), which
    /// is why [`ReferenceCompound::from_name`] normalises separators rather than
    /// comparing raw strings.
    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            ReferenceCompound::Water => "Water",
            ReferenceCompound::Methane => "Methane",
            ReferenceCompound::Ethane => "Ethane",
            ReferenceCompound::Nitrogen => "Nitrogen",
            ReferenceCompound::CarbonDioxide => "CarbonDioxide",
            ReferenceCompound::Benzene => "Benzene",
            ReferenceCompound::Toluene => "Toluene",
        }
    }

    /// Every spelling that resolves to this compound, **already normalised**
    /// (ASCII-lowercase, with spaces, hyphens and underscores removed) so the
    /// table can be compared against a normalised query directly.
    ///
    /// Two kinds of entry only, and both come from this repository — no external
    /// nomenclature source was consulted:
    ///
    /// 1. the canonical name from [`ReferenceCompound::canonical_name`];
    /// 2. the molecular formula already written in the preset's own doc comment
    ///    in [`crate::thermo::component::reference`] (e.g. *"Water (H₂O)"* →
    ///    `"h2o"`), transcribed to ASCII.
    ///
    /// Synonyms that would need a chemistry reference (trade names, IUPAC
    /// alternatives such as *methylbenzene* for toluene, CAS numbers) are
    /// **deliberately absent**: they are compound-identity data, and adding them
    /// is the same provenance question as adding property data.
    #[must_use]
    pub const fn aliases(self) -> &'static [&'static str] {
        match self {
            ReferenceCompound::Water => &["water", "h2o"],
            ReferenceCompound::Methane => &["methane", "ch4"],
            ReferenceCompound::Ethane => &["ethane", "c2h6"],
            ReferenceCompound::Nitrogen => &["nitrogen", "n2"],
            ReferenceCompound::CarbonDioxide => &["carbondioxide", "co2"],
            ReferenceCompound::Benzene => &["benzene", "c6h6"],
            ReferenceCompound::Toluene => &["toluene", "c7h8"],
        }
    }

    /// The full constant-property record for this compound: a freshly
    /// constructed [`Component`] from [`crate::thermo::component::reference`].
    ///
    /// Returned **by value** (not `&'static`) because the presets are
    /// constructed by function call and [`Component`] owns a `String` name;
    /// the workspace forbids lifetime parameters on public types, and a
    /// `Component` is small enough that cloning it per lookup is irrelevant
    /// beside a single EOS evaluation.
    #[must_use]
    pub fn component(self) -> Component {
        match self {
            ReferenceCompound::Water => reference::water(),
            ReferenceCompound::Methane => reference::methane(),
            ReferenceCompound::Ethane => reference::ethane(),
            ReferenceCompound::Nitrogen => reference::nitrogen(),
            ReferenceCompound::CarbonDioxide => reference::carbon_dioxide(),
            ReferenceCompound::Benzene => reference::benzene(),
            ReferenceCompound::Toluene => reference::toluene(),
        }
    }

    /// Resolve a compound **name** to a registry entry, or `None` if the
    /// registry has no data for it.
    ///
    /// # The matching rule, and why
    ///
    /// The query is *normalised* before comparison, by
    /// [`normalized_compound_name`]:
    ///
    /// 1. **ASCII-lowercased** — `"WATER"`, `"Water"` and `"water"` are one
    ///    compound.
    /// 2. **ASCII spaces, tabs, hyphens and underscores removed** — so
    ///    `"carbon dioxide"`, `"Carbon Dioxide"`, `"carbon-dioxide"`,
    ///    `"  CarbonDioxide  "` and `"carbon_dioxide"` all reach the same entry.
    ///    This subsumes trimming.
    /// 3. The result is compared for equality against each entry's
    ///    [`ReferenceCompound::aliases`] table (canonical name + molecular
    ///    formula).
    ///
    /// Nothing else happens: **no fuzzy matching, no prefix matching, no
    /// edit-distance "did you mean"**. A near miss is a miss, and the error
    /// lists the whole registry instead of guessing — guessing which compound a
    /// caller meant is precisely the failure mode that would put wrong critical
    /// constants into a flash with no diagnostic.
    ///
    /// ## Why not upstream's exact match
    ///
    /// DWSIM resolves compounds through a plain `Dictionary(Of String, …)` with
    /// the default ordinal comparer (`FlowsheetBase.vb:3098`, `:3954`), i.e.
    /// exact and case-sensitive. That is free upstream because the user picks a
    /// compound from a database browser, so the string is never hand-typed. In
    /// this crate a name arrives as a hand-written `String` on a
    /// [`crate::flowsheet::streams::StreamCompound`], with no picker and no
    /// database, and the preset spellings are inconsistent on their own terms
    /// (`"CarbonDioxide"` closed up, everything else a single word). Exact
    /// matching would therefore reject `"carbon dioxide"` — the spelling DWSIM's
    /// own database uses — for a reason invisible to the caller.
    ///
    /// Case- and separator-insensitivity is safe *here* specifically because the
    /// registry is seven entries whose normalised keys are pairwise distinct
    /// (asserted by a test in this module); it cannot silently merge two
    /// different compounds. **If the registry ever grows from a real compound
    /// database, re-examine this rule** — at database scale, collisions between
    /// normalised names are a genuine hazard and exact matching may become the
    /// correct default again.
    ///
    /// # Examples
    ///
    /// ```
    /// use outram_park_fork_dwsim_libs::prelude::*;
    ///
    /// assert_eq!(ReferenceCompound::from_name("water"), Some(ReferenceCompound::Water));
    /// assert_eq!(ReferenceCompound::from_name("  Carbon Dioxide "), Some(ReferenceCompound::CarbonDioxide));
    /// assert_eq!(ReferenceCompound::from_name("CO2"), Some(ReferenceCompound::CarbonDioxide));
    /// assert_eq!(ReferenceCompound::from_name("propane"), None); // no data in this crate
    /// ```
    #[must_use]
    pub fn from_name(name: &str) -> Option<ReferenceCompound> {
        let key = normalized_compound_name(name);
        ReferenceCompound::ALL
            .into_iter()
            .find(|entry| entry.aliases().iter().any(|alias| *alias == key))
    }
}

/// Normalise a compound name for registry matching: ASCII-lowercase it and drop
/// ASCII spaces, tabs, hyphens and underscores.
///
/// Exposed because the rule is part of the registry's contract — a caller
/// building its own name index against
/// [`ReferenceCompound::aliases`] needs the same function, and a second,
/// slightly-different copy of it is how a lookup starts disagreeing with
/// itself.
///
/// Non-ASCII characters are passed through unchanged (`char::to_ascii_lowercase`
/// is a no-op on them), so a name containing e.g. a subscript digit will simply
/// fail to match rather than matching something unintended.
///
/// # Examples
///
/// ```
/// use outram_park_fork_dwsim_libs::thermo::registry::normalized_compound_name;
///
/// assert_eq!(normalized_compound_name("  Carbon Dioxide "), "carbondioxide");
/// assert_eq!(normalized_compound_name("n-Butane"), "nbutane");
/// ```
#[must_use]
pub fn normalized_compound_name(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '-' | '_'))
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The canonical names of every compound the registry can resolve, in
/// [`ReferenceCompound::ALL`] order — seven of them.
///
/// Useful for an error message, a CLI `--list-compounds`, or a test that asserts
/// coverage has not silently changed.
#[must_use]
pub fn known_component_names() -> Vec<&'static str> {
    ReferenceCompound::ALL
        .into_iter()
        .map(ReferenceCompound::canonical_name)
        .collect()
}

/// Why a name could not be turned into a [`Component`].
///
/// Follows the crate's error idiom: a `thiserror`-derived enum whose message
/// **names the offending compound** (and, where the caller supplied a slate, its
/// position in that slate), so a missing compound surfaces at the lookup rather
/// than as a length mismatch or a nonsense flash result further down.
///
/// `PartialEq` but not `Eq`: the molar-mass variant carries `f64` fields.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ComponentLookupError {
    /// No preset matched the requested name under the
    /// [`ReferenceCompound::from_name`] rule.
    ///
    /// This is the expected outcome for nearly every real compound: the registry
    /// holds seven. It is **not** a bug to be worked around by fabricating
    /// constants — see the module docs on the provenance question.
    #[error(
        "no component data for `{name}`; this crate has constant properties for only {known_count} reference compounds: {known}"
    )]
    UnknownCompound {
        /// The name that was requested, verbatim as the caller supplied it.
        name: String,
        /// How many compounds the registry holds (currently 7).
        known_count: usize,
        /// Comma-separated canonical names of every compound that *would* have
        /// resolved.
        known: String,
    },

    /// Same as [`ComponentLookupError::UnknownCompound`], but raised while
    /// resolving a whole slate, so it can say **which position** failed.
    ///
    /// Position matters because every consumer in this crate indexes components
    /// and mole fractions positionally (`components[i]` describes `z[i]`), so
    /// "the third compound" is the actionable part of the report.
    #[error(
        "no component data for `{name}` (stream compound {position} of {total}); this crate has constant properties for only {known_count} reference compounds: {known}"
    )]
    UnknownStreamCompound {
        /// Zero-based index of the offending compound in the caller's slice.
        position: usize,
        /// How many compounds were in the slice.
        total: usize,
        /// The name that was requested, verbatim.
        name: String,
        /// How many compounds the registry holds (currently 7).
        known_count: usize,
        /// Comma-separated canonical names of every compound that *would* have
        /// resolved.
        known: String,
    },

    /// The stream's own molar mass disagrees with the resolved component's, by
    /// more than the caller's tolerance.
    ///
    /// **Only ever produced by the opt-in checked path**
    /// ([`crate::flowsheet::component_basis::resolve_components_checked`]); the
    /// default resolution never raises it. See that function for why the check
    /// is advisory rather than mandatory.
    ///
    /// Units are reported in **kg/kmol on both sides** (the stream's own unit),
    /// having converted the component's kg/mol by ×1000, so the two numbers in
    /// the message are directly comparable.
    #[error(
        "stream compound {position} (`{name}`): molar mass {stream_molar_mass_kg_per_kmol} kg/kmol disagrees with registry component `{component_name}` at {component_molar_mass_kg_per_kmol} kg/kmol (relative difference {relative_difference:.3e} exceeds tolerance {tolerance:.3e})"
    )]
    MolarMassMismatch {
        /// Zero-based index of the compound in the caller's slice.
        position: usize,
        /// The stream's compound name, verbatim.
        name: String,
        /// The registry entry's canonical name (may differ in spelling from
        /// `name` — that is allowed, and is itself a useful clue).
        component_name: String,
        /// The stream's molar mass \[kg/kmol\], as stored.
        stream_molar_mass_kg_per_kmol: f64,
        /// The registry component's molar mass \[kg/kmol\], i.e.
        /// [`Component::molar_mass`] (kg/mol) × 1000.
        component_molar_mass_kg_per_kmol: f64,
        /// `|M_stream - M_component| / M_component` \[-\].
        relative_difference: f64,
        /// The tolerance the caller passed \[-\].
        tolerance: f64,
    },
}

impl ComponentLookupError {
    /// Build an [`ComponentLookupError::UnknownCompound`] for `name`, filling in
    /// the registry inventory for the message.
    pub(crate) fn unknown(name: &str) -> ComponentLookupError {
        ComponentLookupError::UnknownCompound {
            name: name.to_owned(),
            known_count: ReferenceCompound::ALL.len(),
            known: known_component_names().join(", "),
        }
    }

    /// Build an [`ComponentLookupError::UnknownStreamCompound`] for the compound
    /// at `position` of `total`.
    pub(crate) fn unknown_at(position: usize, total: usize, name: &str) -> ComponentLookupError {
        ComponentLookupError::UnknownStreamCompound {
            position,
            total,
            name: name.to_owned(),
            known_count: ReferenceCompound::ALL.len(),
            known: known_component_names().join(", "),
        }
    }
}

/// Resolve a compound **name** to its full [`Component`] constant-property
/// record, or fail with an error that names the compound.
///
/// This is the single-compound door into the registry; for a whole stream slate
/// use [`crate::flowsheet::component_basis::resolve_components`], which
/// preserves ordering and reports the failing position.
///
/// Matching follows [`ReferenceCompound::from_name`] (normalised,
/// case-insensitive, canonical name or molecular formula).
///
/// # Errors
///
/// [`ComponentLookupError::UnknownCompound`] if no preset matches — which is the
/// case for every compound outside the seven listed in
/// [`ReferenceCompound::ALL`]. The message names the requested compound and
/// lists what *is* available.
///
/// # Examples
///
/// ```
/// use outram_park_fork_dwsim_libs::prelude::*;
///
/// let benzene = component_by_name("benzene").expect("benzene is a preset");
/// assert_eq!(benzene.name, "Benzene");
/// assert!((benzene.critical_temperature - 562.05).abs() < 1e-12); // K
///
/// let err = component_by_name("propane").unwrap_err();
/// assert!(err.to_string().contains("no component data for `propane`"));
/// ```
pub fn component_by_name(name: &str) -> Result<Component, ComponentLookupError> {
    ReferenceCompound::from_name(name)
        .map(ReferenceCompound::component)
        .ok_or_else(|| ComponentLookupError::unknown(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Exact, NaN-aware field-by-field equality for two [`Component`] records.
    ///
    /// `Component` derives `PartialEq`, but five of the seven presets set
    /// `ig_entropy_formation_25c` to `f64::NAN` ("unknown"), and `NaN != NaN` —
    /// so `assert_eq!` on two copies of the *same* preset fails. Comparing the
    /// `Debug` representations is exact (`{:?}` on `f64` round-trips) and treats
    /// `NaN` as equal to `NaN`, which is the intended meaning here: "the same
    /// record, unknown fields included".
    fn same_component(a: &Component, b: &Component) -> bool {
        format!("{a:?}") == format!("{b:?}")
    }

    /// **Methodology.** Resolve each of the seven presets by its canonical name
    /// and assert the returned [`Component`] is field-for-field identical to the
    /// preset function's own output, so the lookup cannot drift from the data.
    /// Comparison is exact and NaN-aware (see `same_component`) — no tolerance
    /// is involved.
    ///
    /// **Results (2026-09-11):** all 7 canonical names resolve; every resolved
    /// `Component` is identical to its `reference::*()` counterpart, including
    /// the `NaN` `ig_entropy_formation_25c` that five of them carry.
    #[test]
    fn every_preset_resolves_by_canonical_name_to_the_preset_itself() {
        let expected = [
            ("Water", reference::water()),
            ("Methane", reference::methane()),
            ("Ethane", reference::ethane()),
            ("Nitrogen", reference::nitrogen()),
            ("CarbonDioxide", reference::carbon_dioxide()),
            ("Benzene", reference::benzene()),
            ("Toluene", reference::toluene()),
        ];
        assert_eq!(expected.len(), ReferenceCompound::ALL.len());
        for (name, want) in expected {
            let got = component_by_name(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                same_component(&got, &want),
                "resolved `{name}` differs from its preset:\n  got  {got:?}\n  want {want:?}"
            );
        }
    }

    /// **Methodology.** Check the documented normalisation: case, surrounding
    /// and internal whitespace, hyphens and underscores must all be irrelevant,
    /// and the molecular formula from each preset's doc comment must resolve.
    ///
    /// **Results (2026-09-11):** `"WATER"`, `"  water  "`, `"H2O"` → `Water`;
    /// `"carbon dioxide"`, `"Carbon-Dioxide"`, `"carbon_dioxide"`,
    /// `"CarbonDioxide"`, `"co2"` → `CarbonDioxide`; `"c7h8"` → `Toluene`. All
    /// 7 formula aliases resolve to their own compound.
    #[test]
    fn matching_is_case_separator_and_formula_insensitive() {
        for spelling in ["WATER", "  water  ", "Water", "H2O", "h2o"] {
            assert_eq!(
                ReferenceCompound::from_name(spelling),
                Some(ReferenceCompound::Water),
                "`{spelling}` should resolve to Water"
            );
        }
        for spelling in [
            "carbon dioxide",
            "Carbon-Dioxide",
            "carbon_dioxide",
            "CarbonDioxide",
            "co2",
            "CO2",
        ] {
            assert_eq!(
                ReferenceCompound::from_name(spelling),
                Some(ReferenceCompound::CarbonDioxide),
                "`{spelling}` should resolve to CarbonDioxide"
            );
        }
        assert_eq!(
            ReferenceCompound::from_name("c7h8"),
            Some(ReferenceCompound::Toluene)
        );
        // Every alias resolves to the entry that claims it.
        for entry in ReferenceCompound::ALL {
            for alias in entry.aliases() {
                assert_eq!(
                    ReferenceCompound::from_name(alias),
                    Some(entry),
                    "alias `{alias}` should resolve to {entry:?}"
                );
            }
        }
    }

    /// **Methodology.** The permissive matching rule is only safe if no two
    /// registry entries share a normalised key. Collect every alias of every
    /// entry into a set and compare counts.
    ///
    /// **Results (2026-09-11):** 14 aliases across 7 entries (2 each: canonical
    /// name + formula), 14 distinct normalised keys — zero collisions. Each
    /// alias is also already in normal form (`normalized_compound_name(alias)
    /// == alias`), so the table can be compared against a normalised query
    /// directly.
    #[test]
    fn alias_keys_are_pairwise_distinct_and_already_normalised() {
        let mut seen: BTreeSet<&'static str> = BTreeSet::new();
        let mut count = 0usize;
        for entry in ReferenceCompound::ALL {
            for alias in entry.aliases() {
                assert_eq!(
                    normalized_compound_name(alias),
                    *alias,
                    "alias `{alias}` of {entry:?} is not in normal form"
                );
                assert!(seen.insert(alias), "alias `{alias}` is claimed twice");
                count += 1;
            }
        }
        assert_eq!(count, 14, "expected 2 aliases for each of 7 compounds");
        assert_eq!(seen.len(), count);
    }

    /// **Methodology.** Ask for a compound the crate has no data for and check
    /// the error names it and enumerates what is available, rather than
    /// returning a default or a nearest match.
    ///
    /// **Results (2026-09-11):** `component_by_name("propane")` returns
    /// `UnknownCompound { name: "propane", known_count: 7, .. }`; its `Display`
    /// is "no component data for `propane`; this crate has constant properties
    /// for only 7 reference compounds: Water, Methane, Ethane, Nitrogen,
    /// CarbonDioxide, Benzene, Toluene". `"n-butane"`, `"ammonia"`, `"oxygen"`
    /// and `""` likewise fail rather than matching anything.
    #[test]
    fn unknown_compound_error_names_the_compound_and_lists_coverage() {
        let err = component_by_name("propane").unwrap_err();
        match &err {
            ComponentLookupError::UnknownCompound {
                name, known_count, ..
            } => {
                assert_eq!(name, "propane");
                assert_eq!(*known_count, 7);
            }
            other => panic!("expected UnknownCompound, got {other:?}"),
        }
        let message = err.to_string();
        assert!(
            message.contains("no component data for `propane`"),
            "{message}"
        );
        assert!(
            message.contains("Water, Methane, Ethane, Nitrogen, CarbonDioxide, Benzene, Toluene"),
            "{message}"
        );

        for missing in ["n-butane", "ammonia", "oxygen", "", "wate", "waterr"] {
            assert!(
                component_by_name(missing).is_err(),
                "`{missing}` must not resolve"
            );
        }
    }

    /// **Methodology.** Guard the registry's coverage claim: assert the variant
    /// count, the `ALL` length, and that `canonical_name` agrees byte-for-byte
    /// with the `Component::name` the preset builds (otherwise a resolved
    /// slate's names would not round-trip).
    ///
    /// **Results (2026-09-11):** `ALL.len() == 7`;
    /// `known_component_names() == ["Water", "Methane", "Ethane", "Nitrogen",
    /// "CarbonDioxide", "Benzene", "Toluene"]`; for all 7,
    /// `canonical_name() == component().name`.
    #[test]
    fn coverage_is_seven_and_canonical_names_round_trip() {
        assert_eq!(ReferenceCompound::ALL.len(), 7);
        assert_eq!(
            known_component_names(),
            vec![
                "Water",
                "Methane",
                "Ethane",
                "Nitrogen",
                "CarbonDioxide",
                "Benzene",
                "Toluene"
            ]
        );
        for entry in ReferenceCompound::ALL {
            assert_eq!(entry.canonical_name(), entry.component().name);
            assert_eq!(
                ReferenceCompound::from_name(entry.canonical_name()),
                Some(entry)
            );
        }
    }
}
