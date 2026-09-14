//! **Stream slate → thermodynamic component slate** — the bridge from a
//! flowsheet's `&[StreamCompound]` (names) to a thermo kernel's
//! `Vec<Component>` (critical constants).
//!
//! # What this represents physically
//!
//! A material stream identifies its species by **name and molar mass only**
//! ([`StreamCompound`]) — that is all DWSIM's composition algebra needs, since
//! it only ever forms ratios `x_i / M_i`. Every *thermodynamic* routine, by
//! contrast, needs the pure-compound constants: critical temperature `Tc` \[K\],
//! critical pressure `Pc` \[Pa\], acentric factor `ω` \[-\], ideal-gas Cp
//! coefficients. This module walks the first list and produces the second,
//! entry by entry, through
//! [`crate::thermo::registry`].
//!
//! # Ordering is load-bearing — it is preserved exactly
//!
//! Everything downstream of here indexes **positionally**: `k_values`,
//! `flash_pt`, `liquid_molar_enthalpy` and the mixer's mass accumulator all
//! assume `components[i]` and `z[i]` describe the same species, enforced only by
//! a length check. [`resolve_components`] therefore emits components in exactly
//! the order the compounds arrived, one per input, with no filtering, no
//! deduplication and no reordering; the returned `Vec` has the same length as
//! the input slice or the call fails outright. A test in this module asserts
//! that against a deliberately non-alphabetical slate.
//!
//! # ⚠️ Seven compounds
//!
//! The registry behind this bridge holds constant-property data for seven
//! compounds (water, methane, ethane, nitrogen, carbon dioxide, benzene,
//! toluene). Any other species fails with
//! [`ComponentLookupError::UnknownStreamCompound`] naming the compound and its
//! position. That is the intended behaviour, not a gap to route around: see
//! [`crate::thermo::registry`] for why the data is not simply added.
//!
//! # Units — the kilo trap
//!
//! | Side | Field | Unit |
//! |---|---|---|
//! | Stream | [`StreamCompound::molar_mass`] | **kg/kmol** (= g/mol), DWSIM's internal convention |
//! | Thermo | [`Component::molar_mass`] | **kg/mol** |
//!
//! A factor of 1000 separates them. This module never copies one into the other
//! — it *resolves* a component from the registry and, on the opt-in checked
//! path, compares the two after converting, reporting both numbers in kg/kmol.
//!
//! # Attribution
//!
//! Structural reference only: **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0;
//! upstream copyright 2008-2024 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. Upstream's equivalent is `FlowsheetBase.vb:4319`
//! (`AddCompound`) looking each name up in `AvailableCompounds`. This port is
//! GPL-3.0-only, an independent OUTRAM PARK fork, not the official DWSIM
//! software. **No compound property data was taken from upstream.**

use crate::flowsheet::streams::StreamCompound;
use crate::thermo::component::Component;
use crate::thermo::registry::{ComponentLookupError, ReferenceCompound};

/// kg/kmol per kg/mol — the prefix factor between [`StreamCompound::molar_mass`]
/// and [`Component::molar_mass`].
const KG_PER_KMOL_PER_KG_PER_MOL: f64 = 1000.0;

/// Resolve a stream's compound slate into the thermodynamic component slate the
/// [`crate::thermo`] kernel takes, **preserving order and length exactly**.
///
/// Element `i` of the result is the [`Component`] for `compounds[i]`, so the
/// result can be handed straight to any routine that also takes the stream's
/// composition vector (`z`, `x`, `y`) — the positional correspondence the whole
/// crate relies on is maintained by construction.
///
/// Matching of each name follows [`ReferenceCompound::from_name`]:
/// ASCII-case-insensitive, insensitive to spaces/hyphens/underscores, accepting
/// either the canonical name or the molecular formula. Nothing is fuzzy-matched.
///
/// An empty slice resolves to an empty `Vec` (not an error) — a stream with no
/// compounds is a data-model state, not a lookup failure.
///
/// Duplicate names are **not** rejected: if a caller lists water twice, two
/// identical components come back, because positional correspondence with the
/// caller's composition vector matters more here than species uniqueness. The
/// mixer's existing compound-list guard
/// (`flowsheet_solver::evaluator`) is where species-identity agreement between
/// streams is checked.
///
/// This performs **no molar-mass cross-check** — see
/// [`resolve_components_checked`] for that, and for why it is opt-in.
///
/// # Errors
///
/// [`ComponentLookupError::UnknownStreamCompound`] for the **first** compound
/// with no registry data, naming it and giving its zero-based position and the
/// slate length. Resolution stops there; later unknown compounds are not
/// reported in the same call. Only seven compounds resolve at all — see the
/// module header.
///
/// # Examples
///
/// ```
/// use outram_park_fork_dwsim_libs::prelude::*;
///
/// let slate = vec![
///     StreamCompound::new("Benzene", 78.114),
///     StreamCompound::new("toluene", 92.141),
/// ];
/// let components = resolve_components(&slate).expect("both are presets");
/// assert_eq!(components.len(), 2);
/// assert_eq!(components[0].name, "Benzene");
/// assert_eq!(components[1].name, "Toluene");
///
/// let missing = vec![StreamCompound::new("n-Heptane", 100.204)];
/// let err = resolve_components(&missing).unwrap_err();
/// assert!(err.to_string().contains("no component data for `n-Heptane`"));
/// ```
pub fn resolve_components(
    compounds: &[StreamCompound],
) -> Result<Vec<Component>, ComponentLookupError> {
    let total = compounds.len();
    let mut components = Vec::with_capacity(total);
    for (position, compound) in compounds.iter().enumerate() {
        let entry = ReferenceCompound::from_name(&compound.name)
            .ok_or_else(|| ComponentLookupError::unknown_at(position, total, &compound.name))?;
        components.push(entry.component());
    }
    Ok(components)
}

/// One position where a stream's own molar mass disagrees with the molar mass of
/// the [`Component`] its name resolved to.
///
/// A disagreement means the stream and the thermo model **do not agree about
/// what the compound is** — the stream says its species weighs one thing and the
/// registry entry that the name matched weighs another. Typical causes: a name
/// collision (a user's "C2" meaning something other than ethane), a unit slip
/// (kg/mol written into a kg/kmol field, a factor of 1000), or a pseudo-component
/// borrowing a real compound's name.
///
/// Both molar masses are reported in **kg/kmol** (the stream's unit), the
/// component's having been converted from kg/mol, so the two numbers are
/// directly comparable.
#[derive(Debug, Clone, PartialEq)]
pub struct MolarMassDiscrepancy {
    /// Zero-based position in the caller's slate.
    pub position: usize,
    /// The stream compound's name, verbatim.
    pub stream_name: String,
    /// The canonical name of the registry entry it matched (may be spelled
    /// differently — itself a clue).
    pub component_name: String,
    /// The stream's stored molar mass `M` \[kg/kmol\].
    pub stream_molar_mass_kg_per_kmol: f64,
    /// The resolved component's molar mass `M` \[kg/kmol\], i.e.
    /// [`Component::molar_mass`] × 1000.
    pub component_molar_mass_kg_per_kmol: f64,
    /// `|M_stream - M_component| / M_component` \[-\]. Always finite and
    /// non-negative: `Component::new` guarantees a finite, strictly positive
    /// molar mass, and `StreamCompound::new` the same.
    pub relative_difference: f64,
}

impl MolarMassDiscrepancy {
    /// Turn this advisory report into the corresponding hard error, tagged with
    /// the tolerance that was exceeded.
    ///
    /// Used by [`resolve_components_checked`]; exposed so a caller that ran the
    /// advisory pass itself can escalate a specific discrepancy on its own
    /// terms.
    #[must_use]
    pub fn into_error(self, tolerance: f64) -> ComponentLookupError {
        ComponentLookupError::MolarMassMismatch {
            position: self.position,
            name: self.stream_name,
            component_name: self.component_name,
            stream_molar_mass_kg_per_kmol: self.stream_molar_mass_kg_per_kmol,
            component_molar_mass_kg_per_kmol: self.component_molar_mass_kg_per_kmol,
            relative_difference: self.relative_difference,
            tolerance,
        }
    }
}

/// **Advisory** cross-check: report every position where a stream compound's
/// molar mass differs from its resolved component's by more than
/// `relative_tolerance` \[-\].
///
/// Returns an empty `Vec` when the two slates agree. Never fails and never
/// panics — it is a diagnostic, and it is the caller's decision what a
/// disagreement means.
///
/// # Why advisory, and not a hard failure
///
/// Three reasons, and they are the reason [`resolve_components`] does not do
/// this by default:
///
/// 1. **A mismatch is not always an error.** DWSIM's molar masses come from
///    whichever database a case was built against; this crate's come from
///    Poling, Prausnitz & O'Connell (2001). Agreement to the digit is not
///    guaranteed even when both are right about the species, and the composition
///    algebra only uses *ratios* of molar masses, so a small difference changes
///    nothing it touches.
/// 2. **Failing hard would break existing callers** that resolve components for
///    a stream whose molar masses were rounded, or written in a different
///    number of significant figures, at import.
/// 3. **The interesting case is gross, not marginal.** A factor of 1000 (a
///    kg/mol value in a kg/kmol field) or a wrong species shows up as a relative
///    difference of order 1, which any sane tolerance catches. Marginal
///    disagreements are noise; making them fatal would train callers to widen
///    the tolerance until the check stops meaning anything.
///
/// So: run it, log it, decide. [`resolve_components_checked`] is the
/// ready-made "treat it as fatal" wrapper for callers who want that.
///
/// # Arguments
///
/// - `compounds` — the stream slate.
/// - `components` — the resolved components, positionally aligned with
///   `compounds` (i.e. what [`resolve_components`] returned for that slate).
///   If the two slices differ in length, only the first `min(len)` positions are
///   compared; that cannot happen with a slate straight from
///   [`resolve_components`], which preserves length.
/// - `relative_tolerance` — the fractional difference tolerated \[-\], e.g.
///   `1e-3` for "agree to about 0.1 %". A non-positive tolerance reports every
///   position whose molar masses are not bit-equal.
///
/// # Examples
///
/// ```
/// use outram_park_fork_dwsim_libs::prelude::*;
///
/// // 18.015 kg/kmol matches the water preset's 0.018015 kg/mol exactly.
/// let ok = vec![StreamCompound::new("Water", 18.015)];
/// let components = resolve_components(&ok).expect("water is a preset");
/// assert!(molar_mass_discrepancies(&ok, &components, 1e-6).is_empty());
///
/// // A kg/mol value left in a kg/kmol field: out by a factor of 1000.
/// let slipped = vec![StreamCompound::new("Water", 0.018015)];
/// let components = resolve_components(&slipped).expect("water is a preset");
/// let found = molar_mass_discrepancies(&slipped, &components, 1e-6);
/// assert_eq!(found.len(), 1);
/// assert!((found[0].relative_difference - 0.999).abs() < 1e-3);
/// ```
#[must_use]
pub fn molar_mass_discrepancies(
    compounds: &[StreamCompound],
    components: &[Component],
    relative_tolerance: f64,
) -> Vec<MolarMassDiscrepancy> {
    compounds
        .iter()
        .zip(components.iter())
        .enumerate()
        .filter_map(|(position, (compound, component))| {
            let component_kg_per_kmol = component.molar_mass * KG_PER_KMOL_PER_KG_PER_MOL;
            let relative_difference =
                (compound.molar_mass - component_kg_per_kmol).abs() / component_kg_per_kmol;
            if relative_difference > relative_tolerance {
                Some(MolarMassDiscrepancy {
                    position,
                    stream_name: compound.name.clone(),
                    component_name: component.name.clone(),
                    stream_molar_mass_kg_per_kmol: compound.molar_mass,
                    component_molar_mass_kg_per_kmol: component_kg_per_kmol,
                    relative_difference,
                })
            } else {
                None
            }
        })
        .collect()
}

/// [`resolve_components`] plus an **opt-in** hard molar-mass cross-check.
///
/// Resolves the slate exactly as [`resolve_components`] does (same order, same
/// length, same unknown-compound error), then runs
/// [`molar_mass_discrepancies`] and escalates the **first** discrepancy to
/// [`ComponentLookupError::MolarMassMismatch`].
///
/// Use this when the stream slate comes from outside the crate — an imported
/// case, a user-typed flowsheet — and a silent species mix-up would be worse
/// than a refused run. Use plain [`resolve_components`] otherwise; see
/// [`molar_mass_discrepancies`] for why the check is not the default.
///
/// # Arguments
///
/// - `compounds` — the stream slate.
/// - `relative_tolerance` — fractional molar-mass agreement required \[-\]. A
///   sensible starting point is `1e-2` (1 %), which passes rounded database
///   values and catches wrong species and unit slips alike.
///
/// # Errors
///
/// - [`ComponentLookupError::UnknownStreamCompound`] — a compound has no
///   registry data (checked first, for the whole slate, before any molar mass).
/// - [`ComponentLookupError::MolarMassMismatch`] — the first position exceeding
///   `relative_tolerance`, naming the compound, both molar masses in kg/kmol,
///   the relative difference and the tolerance.
///
/// # Examples
///
/// ```
/// use outram_park_fork_dwsim_libs::prelude::*;
///
/// let slate = vec![StreamCompound::new("Water", 18.015)];
/// assert!(resolve_components_checked(&slate, 1e-2).is_ok());
///
/// let wrong = vec![StreamCompound::new("Water", 44.01)];
/// let err = resolve_components_checked(&wrong, 1e-2).unwrap_err();
/// assert!(err.to_string().contains("molar mass"));
/// ```
pub fn resolve_components_checked(
    compounds: &[StreamCompound],
    relative_tolerance: f64,
) -> Result<Vec<Component>, ComponentLookupError> {
    let components = resolve_components(compounds)?;
    match molar_mass_discrepancies(compounds, &components, relative_tolerance)
        .into_iter()
        .next()
    {
        Some(discrepancy) => Err(discrepancy.into_error(relative_tolerance)),
        None => Ok(components),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact, NaN-aware field-by-field equality for two [`Component`] records.
    ///
    /// `Component` derives `PartialEq`, but the presets set unknown fields to
    /// `f64::NAN` (water's `ig_entropy_formation_25c`, for one), and
    /// `NaN != NaN` — so `assert_eq!` on two copies of the *same* preset fails.
    /// Comparing `Debug` representations is exact (`{:?}` on `f64`
    /// round-trips) and treats `NaN` as equal to `NaN`, which is the intended
    /// meaning: "the same record, unknown fields included".
    fn same_component(a: &Component, b: &Component) -> bool {
        format!("{a:?}") == format!("{b:?}")
    }

    fn slate(entries: &[(&str, f64)]) -> Vec<StreamCompound> {
        entries
            .iter()
            .map(|(name, molar_mass)| StreamCompound::new(*name, *molar_mass))
            .collect()
    }

    /// **Methodology.** Resolve a mixed-spelling three-compound slate (canonical
    /// name, lowercase, molecular formula) and assert every returned
    /// [`Component`] carries the canonical name and the cited critical
    /// constants.
    ///
    /// **Results (2026-09-11):** `["Benzene", "toluene", "n2"]` resolves to 3
    /// components named `Benzene`, `Toluene`, `Nitrogen` with `Tc` = 562.05,
    /// 591.75, 126.20 K respectively — exact matches (difference < 1e-12 K) to
    /// the Poling et al. (2001) Appendix-A values the presets cite.
    #[test]
    fn whole_slate_resolves_through_mixed_spellings() {
        let compounds = slate(&[("Benzene", 78.114), ("toluene", 92.141), ("n2", 28.014)]);
        let components = resolve_components(&compounds).expect("all three are presets");
        assert_eq!(components.len(), 3);
        assert_eq!(
            components
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Benzene", "Toluene", "Nitrogen"]
        );
        assert!((components[0].critical_temperature - 562.05).abs() < 1e-12);
        assert!((components[1].critical_temperature - 591.75).abs() < 1e-12);
        assert!((components[2].critical_temperature - 126.20).abs() < 1e-12);
    }

    /// **Methodology.** Because every downstream consumer indexes `components[i]`
    /// against `z[i]`, resolution must not sort, filter or deduplicate. Resolve a
    /// deliberately reverse-alphabetical slate containing a duplicate and assert
    /// the output order and length match the input exactly.
    ///
    /// **Results (2026-09-11):** input `["Water", "Toluene", "Methane",
    /// "Benzene", "Water"]` (5 entries, reverse-alphabetical at the head, water
    /// repeated) resolves to exactly 5 components in that same order, with
    /// positions 0 and 4 field-for-field identical to each other and to
    /// `reference::water()`. No reordering, no deduplication. (The identity
    /// check is NaN-aware — the water preset's `ig_entropy_formation_25c` is
    /// `NaN`, so derived `PartialEq` would report two copies of it as unequal.)
    #[test]
    fn ordering_and_length_are_preserved_including_duplicates() {
        let names = ["Water", "Toluene", "Methane", "Benzene", "Water"];
        let compounds = slate(&[
            ("Water", 18.015),
            ("Toluene", 92.141),
            ("Methane", 16.043),
            ("Benzene", 78.114),
            ("Water", 18.015),
        ]);
        let components = resolve_components(&compounds).expect("all are presets");
        assert_eq!(components.len(), names.len());
        assert_eq!(
            components
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            names.to_vec()
        );
        assert!(same_component(&components[0], &components[4]));
        assert!(
            same_component(
                &components[0],
                &crate::thermo::component::reference::water()
            ),
            "position 0 must be the water preset itself, got {:?}",
            components[0]
        );
    }

    /// **Methodology.** Put an unavailable compound in the middle of an
    /// otherwise-resolvable slate and assert the error names it and reports its
    /// position, rather than silently shortening the result or failing later.
    ///
    /// **Results (2026-09-11):** `["Water", "n-Heptane", "Benzene"]` returns
    /// `UnknownStreamCompound { position: 1, total: 3, name: "n-Heptane",
    /// known_count: 7, .. }`. Its `Display` contains "no component data for
    /// `n-Heptane` (stream compound 1 of 3)" and lists all seven available
    /// compounds. An empty slate returns `Ok` with 0 components.
    #[test]
    fn unknown_compound_error_names_it_and_its_position() {
        let compounds = slate(&[
            ("Water", 18.015),
            ("n-Heptane", 100.204),
            ("Benzene", 78.114),
        ]);
        let err = resolve_components(&compounds).unwrap_err();
        match &err {
            ComponentLookupError::UnknownStreamCompound {
                position,
                total,
                name,
                known_count,
                ..
            } => {
                assert_eq!(*position, 1);
                assert_eq!(*total, 3);
                assert_eq!(name, "n-Heptane");
                assert_eq!(*known_count, 7);
            }
            other => panic!("expected UnknownStreamCompound, got {other:?}"),
        }
        let message = err.to_string();
        assert!(
            message.contains("no component data for `n-Heptane` (stream compound 1 of 3)"),
            "{message}"
        );
        assert!(message.contains("Toluene"), "{message}");

        assert_eq!(
            resolve_components(&[]).expect("empty slate is fine").len(),
            0
        );
    }

    /// **Methodology.** Exercise the opt-in molar-mass cross-check on three
    /// slates: consistent, unit-slipped by a factor of 1000, and wrong species
    /// (carbon dioxide's molar mass on a compound named water). Check the
    /// advisory pass and the hard-failing wrapper agree.
    ///
    /// **Results (2026-09-11):**
    /// - `("Water", 18.015 kg/kmol)` vs the preset's 0.018015 kg/mol → relative
    ///   difference 0 exactly; `molar_mass_discrepancies` empty and
    ///   `resolve_components_checked(.., 1e-2)` returns `Ok`.
    /// - `("Water", 0.018015 kg/kmol)` → relative difference 0.999 (1 − 1/1000);
    ///   one discrepancy reported, and the checked path errors with
    ///   `MolarMassMismatch { position: 0, .. }`.
    /// - `("Water", 44.01 kg/kmol)` → relative difference 1.4429 (measured
    ///   `|44.01 − 18.015| / 18.015`); flagged at a 1 % tolerance, and its error
    ///   message reports both molar masses in kg/kmol.
    #[test]
    fn molar_mass_cross_check_is_advisory_then_escalatable() {
        let consistent = slate(&[("Water", 18.015)]);
        let resolved = resolve_components(&consistent).expect("water is a preset");
        let none = molar_mass_discrepancies(&consistent, &resolved, 1e-6);
        assert!(none.is_empty(), "{none:?}");
        assert_eq!(
            resolved[0].molar_mass * KG_PER_KMOL_PER_KG_PER_MOL,
            18.015,
            "the preset's 0.018015 kg/mol is 18.015 kg/kmol"
        );
        assert!(resolve_components_checked(&consistent, 1e-2).is_ok());

        let slipped = slate(&[("Water", 0.018015)]);
        let resolved = resolve_components(&slipped).expect("water is a preset");
        let found = molar_mass_discrepancies(&slipped, &resolved, 1e-6);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].position, 0);
        assert!(
            (found[0].relative_difference - 0.999).abs() < 1e-6,
            "expected 1 - 1/1000, got {}",
            found[0].relative_difference
        );
        match resolve_components_checked(&slipped, 1e-2).unwrap_err() {
            ComponentLookupError::MolarMassMismatch { position, name, .. } => {
                assert_eq!(position, 0);
                assert_eq!(name, "Water");
            }
            other => panic!("expected MolarMassMismatch, got {other:?}"),
        }

        let wrong_species = slate(&[("Water", 44.01)]);
        let resolved = resolve_components(&wrong_species).expect("water is a preset");
        let found = molar_mass_discrepancies(&wrong_species, &resolved, 1e-2);
        assert_eq!(found.len(), 1);
        let expected = (44.01f64 - 18.015).abs() / 18.015;
        assert!((found[0].relative_difference - expected).abs() < 1e-12);
        let message = found[0].clone().into_error(1e-2).to_string();
        assert!(message.contains("44.01 kg/kmol"), "{message}");
        assert!(message.contains("18.015 kg/kmol"), "{message}");

        // Plain resolution never applies the check.
        assert!(resolve_components(&wrong_species).is_ok());
    }
}
