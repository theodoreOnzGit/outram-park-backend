//! Nuclide catalog + fuzzy finder (op-ixe).
//!
//! The catalog is built once, at startup, from the crate's embedded
//! windowed-multipole CORE set ([`njoy_outram_park_fork::wmp::WmpLibrary::core`])
//! — the same 125-nuclide reactor-grade + LFTR set documented in
//! `njoy-outram-park-fork/docs/wmp-nuclide-manifest.md`. Every entry has a
//! GNDS-style name with no separator (e.g. `"U238"`, `"H1"`, `"C0"` for natural
//! carbon), so a nuclide's element symbol and mass number are recovered by
//! splitting the name at its first ASCII digit — no regex, no external
//! fuzzy-match crate (workspace dependency policy).
//!
//! The fuzzy matcher is hand-rolled, matching the workspace rule against
//! adding a new fuzzy-match dependency: a query like `"u235"`, `"92235"`, or
//! `"U-235"` is scored against several textual representations of each
//! nuclide (its bare name, `Z A` digits, and a dashed display form), and the
//! best-scoring representation wins. See [`fuzzy_score`].

use njoy_outram_park_fork::wmp::WmpLibrary;

use crate::elements::{symbol_for_z, z_for_symbol};

/// One nuclide's identity, as listed in the embedded WMP CORE set.
///
/// `z`/`a` are recovered from the WMP name at catalog-build time
/// ([`NuclideCatalog::build`]) purely for display and search — the actual
/// cross-section data is looked up later by `name` (see [`crate::xsdata`]).
#[derive(Debug, Clone)]
pub struct NuclideId {
    /// Atomic number (protons). `0` only if the name's element prefix was not
    /// a recognised symbol (defensive fallback; should not occur for the
    /// embedded CORE set).
    pub z: u32,
    /// Mass number (protons + neutrons). `0` denotes a natural-abundance
    /// mixture (e.g. carbon `"C0"`), not a real isotope.
    pub a: u32,
    /// The exact name [`WmpLibrary::get`] expects, e.g. `"U238"`.
    pub name: String,
    /// Human-readable display label, e.g. `"U-235"` (or `"C-nat"` for `a=0`).
    pub label: String,
}

impl NuclideId {
    /// Parse one WMP CORE entry name (e.g. `"U238"`) into a [`NuclideId`].
    ///
    /// Splits at the first ASCII digit: everything before is the element
    /// symbol, everything from there on is the mass number. This is exact for
    /// the embedded CORE set (checked in `catalog_parses_every_core_entry`)
    /// because CORE names carry no metastable-state suffix (`m1`, `m2`, ...) —
    /// those only appear in the EXTENDED (non-embedded) sibling manifest.
    fn parse(name: &str) -> Self {
        let split_at = name
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or(name.len());
        let (sym, digits) = name.split_at(split_at);
        let z = z_for_symbol(sym).unwrap_or(0);
        let a: u32 = digits.parse().unwrap_or(0);
        let label = match (symbol_for_z(z), a) {
            (Some(s), 0) => format!("{s}-nat"),
            (Some(s), a) => format!("{s}-{a}"),
            (None, _) => name.to_string(),
        };
        NuclideId {
            z,
            a,
            name: name.to_string(),
            label,
        }
    }

    /// The searchable representations this entry is fuzzy-matched against:
    /// the bare WMP name, the display label, and the `ZZAAA`-style digit
    /// string (e.g. `"92235"`), lowercased.
    fn search_keys(&self) -> [String; 3] {
        [
            self.name.to_ascii_lowercase(),
            self.label.to_ascii_lowercase(),
            format!("{}{:03}", self.z, self.a),
        ]
    }
}

/// The full set of nuclides the TUI can browse, built once at startup.
#[derive(Debug, Clone)]
pub struct NuclideCatalog {
    pub entries: Vec<NuclideId>,
}

impl NuclideCatalog {
    /// Build the catalog from the embedded WMP CORE set. Cheap (125 short
    /// string parses); intended to be called once at startup and shared
    /// (e.g. via `Arc`) rather than rebuilt per frame.
    pub fn build() -> Self {
        let lib = WmpLibrary::core();
        let mut entries: Vec<NuclideId> = lib.names().into_iter().map(NuclideId::parse).collect();
        entries.sort_by(|a, b| (a.z, a.a).cmp(&(b.z, b.a)));
        NuclideCatalog { entries }
    }

    /// Filter + rank entries against a fuzzy query (see [`fuzzy_score`]).
    ///
    /// An empty query matches everything, in catalog (Z, A) order — so the
    /// finder shows the full list before the user types anything. A
    /// non-empty query returns only entries that score, best match first.
    pub fn search(&self, query: &str) -> Vec<&NuclideId> {
        if query.trim().is_empty() {
            return self.entries.iter().collect();
        }
        let q = query.trim().to_ascii_lowercase();
        let mut scored: Vec<(i32, &NuclideId)> = self
            .entries
            .iter()
            .filter_map(|e| {
                e.search_keys()
                    .iter()
                    .filter_map(|k| fuzzy_score(&q, k))
                    .max()
                    .map(|s| (s, e))
            })
            .collect();
        // Higher score first; break ties by (Z, A) so the list stays stable.
        scored.sort_by(|a, b| b.0.cmp(&a.0).then((a.1.z, a.1.a).cmp(&(b.1.z, b.1.a))));
        scored.into_iter().map(|(_, e)| e).collect()
    }
}

/// Score how well `query` matches `candidate` (both assumed already
/// lowercased), or `None` if it does not match at all.
///
/// Two match kinds, contiguous substring preferred over subsequence:
/// - **Substring**: `candidate` contains `query` verbatim. Scored highest,
///   with a bonus for matching at the very start (prefix match) and a
///   penalty proportional to leftover length (a tighter candidate ranks
///   above a looser one containing the same substring).
/// - **Subsequence**: every character of `query` appears in `candidate`, in
///   order, but not necessarily contiguous (classic fuzzy-find). Scored
///   below any substring match, with a penalty for how spread out the
///   matched characters are (tighter clusters rank higher).
///
/// Returns `None` if `query` is not even a subsequence of `candidate`.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    const SUBSTRING_BASE: i32 = 1_000;
    const SUBSEQUENCE_BASE: i32 = 0;

    if let Some(pos) = candidate.find(query) {
        let prefix_bonus = if pos == 0 { 50 } else { 0 };
        let tightness = 100 - (candidate.len() as i32 - query.len() as i32).min(100);
        return Some(SUBSTRING_BASE + prefix_bonus + tightness);
    }

    // Subsequence scan: greedily match query chars left-to-right through
    // candidate, tracking the span consumed to penalize spread.
    let cand: Vec<char> = candidate.chars().collect();
    let mut ci = 0usize;
    let mut first_match: Option<usize> = None;
    let mut last_match: usize = 0;
    for qc in query.chars() {
        let mut found = false;
        while ci < cand.len() {
            if cand[ci] == qc {
                if first_match.is_none() {
                    first_match = Some(ci);
                }
                last_match = ci;
                ci += 1;
                found = true;
                break;
            }
            ci += 1;
        }
        if !found {
            return None;
        }
    }
    let span = last_match - first_match.unwrap_or(0) + 1;
    let spread_penalty = span as i32 - query.len() as i32;
    Some(SUBSEQUENCE_BASE - spread_penalty)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every embedded CORE entry parses to a recognised element symbol and a
    /// label containing that element's dash form — a regression guard for
    /// the "split at first digit" assumption documented on
    /// [`NuclideId::parse`].
    #[test]
    fn catalog_parses_every_core_entry() {
        let cat = NuclideCatalog::build();
        assert!(
            cat.entries.len() >= 100,
            "expected the ~125-nuclide CORE set"
        );
        for e in &cat.entries {
            assert_ne!(
                e.z, 0,
                "{} did not resolve to a known element symbol",
                e.name
            );
            assert!(
                e.label.contains('-'),
                "{} label {} missing dash",
                e.name,
                e.label
            );
        }
    }

    #[test]
    fn parses_natural_carbon_and_common_isotopes() {
        let c = NuclideId::parse("C0");
        assert_eq!((c.z, c.a), (6, 0));
        assert_eq!(c.label, "C-nat");

        let u235 = NuclideId::parse("U235");
        assert_eq!((u235.z, u235.a), (92, 235));
        assert_eq!(u235.label, "U-235");

        let h1 = NuclideId::parse("H1");
        assert_eq!((h1.z, h1.a), (1, 1));
    }

    #[test]
    fn search_finds_u235_by_gnds_name_zaid_and_dashed_form() {
        let cat = NuclideCatalog::build();
        for query in ["u235", "U235", "92235", "u-235", "U-235"] {
            let hits = cat.search(query);
            assert!(
                hits.iter().any(|n| n.name == "U235"),
                "query {query:?} did not find U235: {:?}",
                hits.iter().map(|n| &n.name).take(5).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn search_by_element_symbol_alone_returns_all_its_isotopes() {
        let cat = NuclideCatalog::build();
        let hits = cat.search("zr");
        assert!(
            hits.len() >= 4,
            "expected several Zr isotopes, got {}",
            hits.len()
        );
        assert!(hits.iter().all(|n| n.z == 40));
    }

    #[test]
    fn empty_query_returns_everything_in_za_order() {
        let cat = NuclideCatalog::build();
        let hits = cat.search("");
        assert_eq!(hits.len(), cat.entries.len());
        for w in hits.windows(2) {
            assert!((w[0].z, w[0].a) <= (w[1].z, w[1].a));
        }
    }

    #[test]
    fn fuzzy_score_prefers_substring_over_subsequence() {
        let sub = fuzzy_score("235", "u235").unwrap();
        let seq = fuzzy_score("u35", "u235").unwrap(); // subsequence only (skips the 2)
        assert!(sub > seq);
    }

    #[test]
    fn fuzzy_score_rejects_out_of_order_or_missing_chars() {
        assert_eq!(fuzzy_score("53u", "u235"), None); // wrong order
        assert_eq!(fuzzy_score("xyz", "u235"), None); // not present at all
    }
}
