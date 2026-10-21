// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py
//                    (`nuclide_import`, `nuclide_import_accident`,
//                     `inventory_processing`)
//                    trisoatops/utility_functions/run_functions.py
//                    (`nuclide_sort`, `convert_time`)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// Ported under GPL-3.0; see LICENSE.triso-atops and NOTICE.triso-atops.

//! Run set-up: which nuclides a run uses, how they are classified, and how a
//! bulk inventory is distributed over the axial nodes.
//!
//! This is the layer between a user's nuclide list and the physics: it
//! normalises names, looks them up in the database, decides short-lived vs
//! long-lived against the run's own timescale, and wires parent → daughter
//! coupling. Upstream calls it `nuclide_import` / `nuclide_import_accident` /
//! `inventory_processing`.
//!
//! # Upstream defects reproduced or corrected here — read before trusting
//!
//! Porting this module meant deciding, for five upstream bugs, whether to
//! reproduce or correct. The rule applied throughout: **reproduce upstream's
//! observable behaviour where it is a physics choice, correct it where the
//! upstream code plainly states an intent its own syntax defeats** — and in
//! every case make the divergence explicit and selectable, never silent. Each
//! is documented on the item it affects. Summary:
//!
//! | Upstream | Here |
//! |---|---|
//! | `parent_decay` assigned with `==` (dead short-lived test) | [`ParentDecayPolicy`] — intended logic is the default, bug-compatible mode is explicit |
//! | Rh-105 defaulted `parent_decay = False` despite having a parent | Reproduced under [`ParentDecayPolicy::UpstreamTableDefault`] only |
//! | `nuclide_import` mutates the shared module table | Impossible here: values are owned |
//! | `nuclide_import_accident` unguarded table lookup (`KeyError`) | Returns [`SelectionError::UnknownNuclide`] |
//! | `nuclide_sort` compares a `list` to `str` (dead reordering) | [`sort_parents_before_daughters`] does what upstream intended |

use super::nuclide_model::{nuclide_database::find_nuclide, TrisoAtopsNuclide};
use uom::si::f64::Time;
use uom::si::time::second;

/// Why a nuclide could not be taken into a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionError {
    /// The name did not parse as `Element[-]MassNumber[metastable]`, e.g. `"Cs137"`,
    /// `"cs-137"` and `"Cs-137m"` all parse; `"plutonium"` and `"137"` do not.
    Unparseable {
        /// The name as supplied by the caller.
        supplied: String,
    },
    /// The name parsed but is absent from the TRISO-ATOPS nuclide table.
    ///
    /// Upstream's `nuclide_import` logs a warning and skips; its
    /// `nuclide_import_accident` omits that guard and raises `KeyError`. This
    /// port returns the same error from both and lets the caller decide (see
    /// [`select_nuclides`], which skips, versus
    /// [`select_nuclides_accident`], which also skips — deliberately unlike
    /// upstream).
    UnknownNuclide {
        /// The normalised name that was looked up, e.g. `"Cs-137"`.
        normalised: String,
    },
}

/// How to decide whether a daughter's parent-decay coupling is switched on.
///
/// # Why this is an enum and not a `bool`
///
/// Upstream's `nuclide_import` contains this (lines 275 and 277):
///
/// ```python
/// if nuclide_out[parent].sl == True:
///     nuclide_out[nuclide].parent_decay == True      # `==`, not `=`
/// else:
///     nuclide_out[nuclide].parent_decay == False     # `==`, not `=`
/// ```
///
/// Both statements are comparisons whose results are discarded, so in the
/// branch that is supposed to *decide* the flag, nothing is written and the
/// value hard-coded in the nuclide table survives. The consequence is not that
/// parent decay is disabled — it is that **the short-lived-parent test the
/// code was written to perform never runs**.
///
/// The two behaviours are genuinely different physics, so the port exposes
/// both rather than picking silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParentDecayPolicy {
    /// Apply the test upstream's source plainly intends: couple a daughter to
    /// its parent only when the parent is present in the run **and** is
    /// classified short-lived.
    ///
    /// This is the default, because the workspace requires correct physics to
    /// be the default rather than an opt-in, and because it is what upstream's
    /// own control flow says it wants. It is **not** what a stock TRISO-ATOPS
    /// run does.
    #[default]
    ShortLivedParentOnly,
    /// Reproduce the stock TRISO-ATOPS behaviour bug-for-bug: take the flag
    /// from the nuclide table and ignore the parent's half-life entirely,
    /// except that a parent absent from the run still forces it off (upstream
    /// lines 280 and 283 use `=` correctly).
    ///
    /// Use this for code-to-code comparison against upstream, or to reproduce
    /// a published TRISO-ATOPS result. Note it also carries upstream's Rh-105
    /// inconsistency — see [`upstream_table_parent_decay`].
    UpstreamTableDefault,
}

/// A nuclide admitted to a run, with the run-dependent classification attached.
///
/// Upstream stores `sl` and `parent_decay` by mutating the shared module-level
/// `nuclides` dictionary, so classification from one run leaks into the next
/// within a process. Owning the values here makes that class of bug
/// impossible.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedNuclide {
    /// The database record (name, `Z`, `A`, half-life, parents).
    pub nuclide: TrisoAtopsNuclide,
    /// `true` when the half-life is short compared to the run's own timescale.
    ///
    /// Upstream: `hl / irad_time < short_lived_ratio`, default ratio `0.2`.
    /// Short-lived species reach equilibrium within the irradiation and take
    /// the undivided birth rate in [`release_rate`](super::activities::release_rate);
    /// long-lived ones are divided by `1 - exp(-lambda t)`.
    pub short_lived: bool,
    /// `true` when this nuclide's activity is fed by its parent's decay.
    ///
    /// Governed by [`ParentDecayPolicy`]; see that type for why it is not a
    /// straightforward read of upstream.
    pub parent_decay: bool,
}

/// The nuclides upstream's table defaults to `parent_decay = True`.
///
/// Thirteen of the fourteen table rows that carry a parent are defaulted
/// `True`. The fourteenth, **Rh-105**, is defaulted `False` despite carrying
/// `['Ru-105']` and the same `# og with parent decay` comment as the other
/// thirteen — which, because the runtime assignment is defeated by the `==`
/// bug, means a stock TRISO-ATOPS run never applies parent decay to Rh-105.
///
/// That looks like an oversight rather than a decision, but it is upstream's,
/// so [`ParentDecayPolicy::UpstreamTableDefault`] reproduces it exactly rather
/// than quietly repairing it.
///
/// # Returns
/// `true` if the stock table would default this nuclide's `parent_decay` flag
/// on; `false` otherwise (including for every nuclide with no parent).
#[must_use]
pub fn upstream_table_parent_decay(name: &str) -> bool {
    // The 13 rows defaulted True in calculation_functions.py lines 85-172.
    // Rh-105 is deliberately absent — see this function's doc comment.
    const TABLE_TRUE: [&str; 13] = [
        "Sr-89", "Sr-90", "Y-91", "Nb-95", "Tc-99m", "Te-127", "I-131", "I-132", "Xe-133",
        "Xe-135", "Cs-138", "La-140", "Pr-143",
    ];
    TABLE_TRUE.contains(&name)
}

/// Normalise a nuclide name to the database's canonical spelling.
///
/// Ports the regex `([a-z]{1,2})(?:[-]?)([0-9]+)([a-z]?)` plus the
/// `f"{element.capitalize()}-{main_number}{suffix}"` reassembly that upstream
/// applies in both `nuclide_import` and `nuclide_import_accident`. Hand-rolled
/// rather than pulling in `regex`, which the crate does not otherwise need.
///
/// Accepts one or two element letters, an optional hyphen, the mass number,
/// and an optional single metastable letter. Input case is irrelevant.
///
/// # Arguments
/// - `supplied` — the caller's spelling, e.g. `"cs137"`, `"CS-137"`, `"Cs-137m"`.
///
/// # Returns
/// The canonical name (`"Cs-137"`, `"Cs-137m"`), or
/// [`SelectionError::Unparseable`] if the pattern does not match.
///
/// # Note on strictness
/// Like upstream's regex this is anchored only at the start, so trailing junk
/// after the optional metastable letter is rejected here but silently ignored
/// by Python's `re.match`. That is a deliberate tightening: a name upstream
/// would have silently truncated is far more likely a typo than an intent.
pub fn normalise_nuclide_name(supplied: &str) -> Result<String, SelectionError> {
    let unparseable = || SelectionError::Unparseable {
        supplied: supplied.to_string(),
    };
    let lower = supplied.trim().to_ascii_lowercase();
    let bytes = lower.as_bytes();

    // 1-2 leading ASCII letters.
    let mut i = 0;
    while i < bytes.len() && i < 2 && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == 0 {
        return Err(unparseable());
    }
    let element = &lower[..i];

    // Optional single hyphen.
    if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
    }

    // One or more digits.
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digits_start {
        return Err(unparseable());
    }
    let mass = &lower[digits_start..i];

    // Optional single trailing letter (metastable state).
    let suffix = if i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        let s = &lower[i..i + 1];
        i += 1;
        s
    } else {
        ""
    };
    if i != bytes.len() {
        return Err(unparseable());
    }

    // Python's str.capitalize(): first character upper, rest lower.
    let mut canonical = String::with_capacity(element.len() + 1 + mass.len() + suffix.len());
    let mut chars = element.chars();
    if let Some(first) = chars.next() {
        canonical.extend(first.to_uppercase());
        canonical.push_str(chars.as_str());
    }
    canonical.push('-');
    canonical.push_str(mass);
    canonical.push_str(suffix);
    Ok(canonical)
}

/// Select and classify the nuclides for a **normal-operation** run.
///
/// Ports `nuclide_import`. For each supplied name: normalise it, look it up,
/// classify short-lived against the irradiation time, then wire parent-decay
/// coupling per `policy`.
///
/// # Arguments
/// - `supplied_names` — the run's nuclide list, in any spelling
///   [`normalise_nuclide_name`] accepts.
/// - `irradiation_time` — the reactor irradiation time the short-lived test is
///   measured against (SI seconds; must be `> 0`).
/// - `short_lived_ratio` — the threshold on `t½ / t_irrad` below which a
///   nuclide counts as short-lived. Upstream's default is `0.2`; pass
///   `None` for it.
/// - `policy` — see [`ParentDecayPolicy`].
///
/// # Returns
/// `(selected, skipped)` — the admitted nuclides in input order, and one
/// [`SelectionError`] per name that could not be taken. Upstream logs those as
/// warnings and continues; this returns them so a caller can decide, which is
/// the only behavioural difference.
///
/// # Panics
/// Panics if `irradiation_time` is not strictly positive — upstream would
/// divide by zero and classify everything as not-short-lived.
#[must_use]
pub fn select_nuclides(
    supplied_names: &[&str],
    irradiation_time: Time,
    short_lived_ratio: Option<f64>,
    policy: ParentDecayPolicy,
) -> (Vec<SelectedNuclide>, Vec<SelectionError>) {
    let t_irrad = irradiation_time.get::<second>();
    assert!(
        t_irrad > 0.0,
        "irradiation time must be positive; got {t_irrad} s"
    );
    let ratio = short_lived_ratio.unwrap_or(0.2);

    let mut selected: Vec<SelectedNuclide> = Vec::new();
    let mut skipped: Vec<SelectionError> = Vec::new();

    for name in supplied_names {
        match normalise_nuclide_name(name) {
            Err(e) => skipped.push(e),
            Ok(canonical) => match find_nuclide(&canonical) {
                None => skipped.push(SelectionError::UnknownNuclide {
                    normalised: canonical,
                }),
                Some(nuclide) => {
                    let short_lived = nuclide.half_life.get::<second>() / t_irrad < ratio;
                    selected.push(SelectedNuclide {
                        nuclide,
                        short_lived,
                        // Provisional; resolved below once every nuclide's
                        // short_lived flag is known, exactly as upstream does
                        // it in a second pass over `used_nuclides`.
                        parent_decay: false,
                    });
                }
            },
        }
    }

    // Second pass: parent → daughter wiring. Upstream needs this separated
    // because a daughter may be listed before its parent.
    for idx in 0..selected.len() {
        let Some(parent_name) = selected[idx].nuclide.parents.first().copied() else {
            // No parent in the model: upstream sets False (line 283, correct `=`).
            selected[idx].parent_decay = false;
            continue;
        };
        let parent = selected
            .iter()
            .find(|s| s.nuclide.name == parent_name)
            .map(|s| s.short_lived);
        selected[idx].parent_decay = match (parent, policy) {
            // Parent absent from the run: upstream line 280, correct `=`, False.
            (None, _) => false,
            (Some(parent_short_lived), ParentDecayPolicy::ShortLivedParentOnly) => {
                parent_short_lived
            }
            (Some(_), ParentDecayPolicy::UpstreamTableDefault) => {
                upstream_table_parent_decay(selected[idx].nuclide.name)
            }
        };
    }

    (selected, skipped)
}

/// Select the nuclides relevant to an **accident** window.
///
/// Ports `nuclide_import_accident`. Keeps only nuclides whose half-life is
/// long enough to matter over the accident: `t½ / t_accident >= use_ratio`.
/// Note the inequality runs the opposite way to [`select_nuclides`] — here a
/// *short* half-life is what disqualifies a nuclide, because anything that has
/// already decayed away cannot be released.
///
/// # Arguments
/// - `supplied_names` — the run's nuclide list.
/// - `accident_time` — duration of the accident transient (SI seconds, `> 0`).
/// - `use_ratio` — threshold on `t½ / t_accident`; upstream's default is
///   `0.04`. Pass `None` for it.
///
/// # Returns
/// `(selected, skipped)`. No classification is attached: upstream does not set
/// `sl` or `parent_decay` on this path, and the accident driver does not read
/// them.
///
/// # Divergence from upstream
/// Upstream indexes the table **without** the `in nuclides` guard its sibling
/// has, so a name that satisfies the regex but is absent raises `KeyError` and
/// aborts the run. This port skips it and reports
/// [`SelectionError::UnknownNuclide`], matching `nuclide_import`'s
/// warn-and-continue. Upstream's `else` branch also logs `{match}`, which is
/// `None` whenever that branch is reached.
///
/// # Panics
/// Panics if `accident_time` is not strictly positive.
#[must_use]
pub fn select_nuclides_accident(
    supplied_names: &[&str],
    accident_time: Time,
    use_ratio: Option<f64>,
) -> (Vec<TrisoAtopsNuclide>, Vec<SelectionError>) {
    let t_acc = accident_time.get::<second>();
    assert!(t_acc > 0.0, "accident time must be positive; got {t_acc} s");
    let ratio = use_ratio.unwrap_or(0.04);

    let mut selected = Vec::new();
    let mut skipped = Vec::new();
    for name in supplied_names {
        match normalise_nuclide_name(name) {
            Err(e) => skipped.push(e),
            Ok(canonical) => match find_nuclide(&canonical) {
                None => skipped.push(SelectionError::UnknownNuclide {
                    normalised: canonical,
                }),
                Some(nuclide) => {
                    if nuclide.half_life.get::<second>() / t_acc >= ratio {
                        selected.push(nuclide);
                    }
                }
            },
        }
    }
    (selected, skipped)
}

/// Reorder a nuclide list so every parent precedes its daughters.
///
/// Ports `run_functions.py::nuclide_sort`, whose stated purpose is to let the
/// driver accumulate parent activities before the daughters that consume them.
///
/// # Divergence: upstream's version never reorders anything
///
/// `nuclide_sort` reads
///
/// ```python
/// par = calc.nuclides[n].parents          # a LIST, e.g. ['Kr-89']
/// if par is not None and par in list(nuke_list[:, 0]):
/// ```
///
/// which tests whether the *list* `['Kr-89']` is an element of a list of
/// *strings*. That is never true, so the reordering branch is dead and the
/// function returns the input order unchanged. This port does what the
/// function says it does; a caller wanting the upstream no-op can simply not
/// call it.
///
/// # Arguments
/// - `names` — nuclide names, already normalised.
///
/// # Returns
/// The same names, with each parent that is present moved ahead of its first
/// daughter. Names absent from the database keep their relative order.
/// Stable: nuclides with no parent relationship are not moved relative to one
/// another.
#[must_use]
pub fn sort_parents_before_daughters(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(names.len());
    for name in names {
        if out.contains(name) {
            continue;
        }
        if let Some(parent) = find_nuclide(name)
            .and_then(|n| n.parents.first().copied())
            .filter(|p| names.iter().any(|n| n == p) && !out.iter().any(|o| o == p))
        {
            out.push(parent.to_string());
        }
        out.push(name.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::time::{day, year};

    #[test]
    fn names_normalise_like_upstream_regex() {
        for (input, want) in [
            ("cs137", "Cs-137"),
            ("CS-137", "Cs-137"),
            ("Cs-137", "Cs-137"),
            ("i131", "I-131"),
            ("kr-83m", "Kr-83m"),
            ("TC99M", "Tc-99m"),
        ] {
            assert_eq!(
                normalise_nuclide_name(input).unwrap(),
                want,
                "input {input}"
            );
        }
        for bad in ["plutonium", "137", "", "-137", "cs-"] {
            assert!(
                normalise_nuclide_name(bad).is_err(),
                "should reject {bad:?}"
            );
        }
    }

    #[test]
    fn short_lived_classification_uses_the_upstream_ratio() {
        // Kr-85m, t½ = 16 128 s, against a 3-year irradiation: deeply short-lived.
        let (sel, skipped) = select_nuclides(
            &["Kr-85m", "Cs-137"],
            Time::new::<year>(3.0),
            None,
            ParentDecayPolicy::default(),
        );
        assert!(skipped.is_empty());
        assert!(sel[0].short_lived, "Kr-85m should be short-lived");
        // Cs-137, t½ ≈ 30 a, ratio ≈ 10 ≫ 0.2.
        assert!(!sel[1].short_lived, "Cs-137 should be long-lived");
    }

    #[test]
    fn unknown_and_unparseable_names_are_reported_not_panicked() {
        let (sel, skipped) = select_nuclides(
            &["Cs-137", "Zz-999", "plutonium"],
            Time::new::<year>(3.0),
            None,
            ParentDecayPolicy::default(),
        );
        assert_eq!(sel.len(), 1);
        assert_eq!(skipped.len(), 2);
        assert!(matches!(skipped[0], SelectionError::UnknownNuclide { .. }));
        assert!(matches!(skipped[1], SelectionError::Unparseable { .. }));
    }

    /// The two parent-decay policies must actually differ, or the enum is
    /// decoration. Xe-135's parent I-135 (t½ = 23 760 s) is short-lived
    /// against a 3-year irradiation, so both policies agree there; the
    /// discriminating case is Rh-105, which upstream's table defaults off.
    #[test]
    fn parent_decay_policies_differ_on_rh105() {
        let names = ["Ru-105", "Rh-105"];
        let t = Time::new::<year>(3.0);
        let (intended, _) =
            select_nuclides(&names, t, None, ParentDecayPolicy::ShortLivedParentOnly);
        let (upstream, _) =
            select_nuclides(&names, t, None, ParentDecayPolicy::UpstreamTableDefault);
        let rh_intended = intended
            .iter()
            .find(|s| s.nuclide.name == "Rh-105")
            .unwrap();
        let rh_upstream = upstream
            .iter()
            .find(|s| s.nuclide.name == "Rh-105")
            .unwrap();
        // Ru-105 (t½ = 15 984 s) is short-lived over 3 a, so the intended
        // logic couples them; upstream's table defaults Rh-105 off.
        assert!(rh_intended.parent_decay, "intended logic couples Rh-105");
        assert!(
            !rh_upstream.parent_decay,
            "upstream table defaults Rh-105 off — the documented inconsistency"
        );
    }

    #[test]
    fn a_parent_absent_from_the_run_always_disables_coupling() {
        for policy in [
            ParentDecayPolicy::ShortLivedParentOnly,
            ParentDecayPolicy::UpstreamTableDefault,
        ] {
            let (sel, _) = select_nuclides(&["Xe-135"], Time::new::<year>(3.0), None, policy);
            assert!(
                !sel[0].parent_decay,
                "I-135 absent ⇒ no coupling, under {policy:?}"
            );
        }
    }

    #[test]
    fn accident_selection_keeps_long_lived_and_drops_the_rest() {
        // Over a 3-day accident: Cs-137 (30 a) stays, Kr-89 (189 s) goes.
        let (sel, skipped) =
            select_nuclides_accident(&["Cs-137", "Kr-89"], Time::new::<day>(3.0), None);
        assert!(skipped.is_empty());
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].name, "Cs-137");
    }

    #[test]
    fn accident_selection_skips_unknown_instead_of_panicking() {
        // Upstream raises KeyError here; this port reports and continues.
        let (sel, skipped) =
            select_nuclides_accident(&["Zz-999", "Cs-137"], Time::new::<day>(3.0), None);
        assert_eq!(sel.len(), 1);
        assert_eq!(skipped.len(), 1);
    }

    #[test]
    fn parents_are_moved_ahead_of_daughters() {
        let names: Vec<String> = ["Xe-135", "I-135", "Cs-137"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let sorted = sort_parents_before_daughters(&names);
        let i135 = sorted.iter().position(|n| n == "I-135").unwrap();
        let xe135 = sorted.iter().position(|n| n == "Xe-135").unwrap();
        assert!(i135 < xe135, "parent I-135 must precede Xe-135: {sorted:?}");
        assert_eq!(sorted.len(), 3, "no duplicates: {sorted:?}");
    }
}
