// SPDX-License-Identifier: GPL-3.0

//! **Cross-code: this workspace's depletion-chain reader against OpenMC's own
//! parser, on the same files** — gh:#270.
//!
//! # Methodology
//!
//! `openmc.deplete.Chain.from_xml` is the format's reference implementation. It
//! needs no cross-section library, so — unlike almost every other comparison in
//! this crate — it can be run here directly rather than against a stored
//! number.
//!
//! `verification_and_validation/depletion_chain_xml/openmc_inputs/dump_chain.py`
//! walks OpenMC's parsed `Chain` and writes a flat, `|`-separated record per
//! nuclide, decay branch, reaction channel, fission yield and decay source.
//! **Every float is written as its IEEE-754 bit pattern**, because Python's
//! `repr` and Rust's `Display` both round-trip but spell the same value
//! differently (`6.14271e-05` vs `0.0000614271`); comparing bits takes
//! formatting out of the comparison. This test builds the same dump from
//! [`DepletionChainXml`] and diffs the two, line for line.
//!
//! The reference dumps and the source XML files are **committed** beside the
//! driver script, so this runs on any checkout with no OpenMC installation and a
//! reader can regenerate the reference side. Pass criterion: **every line
//! identical**, in order, with no tolerance — the comparison is of parsed
//! structure, where any difference is a defect on one side or the other.
//!
//! # Results (2026-09-23, OpenMC `0.1.dev1+gafa7a14ac`)
//!
//! | file | nuclides | records | disagreements |
//! |---|---|---|---|
//! | `chain_simple.xml` | 9 | 36 | **0** |
//! | `chain_simple_decay.xml` | 11 | 43 | **0** |
//! | `chain_ni.xml` | 21 | 97 | **0** |
//!
//! 176 records over 41 nuclides, exactly equal. What that covers: half-lives
//! and the stable case, decay energies and their `0.0` default, the `"nothing"`
//! sentinel and genuinely targetless decays, branching ratios and their
//! defaults, reaction Q values, fission's forced-`None` target, multi-product
//! yields, and which particles each nuclide sources.
//!
//! # What this does NOT establish
//!
//! Agreement on **parsing**, not on physics: neither side is checked against the
//! evaluated data the chain summarises. And the three files here are OpenMC's
//! own test fixtures — no full ENDF/B-VIII chain (thousands of nuclides,
//! `parent=` yield borrowing, several yield energies) was available on this
//! machine, so those paths are covered by synthetic unit tests in
//! `hdf5::depletion_chain_xml` rather than cross-checked. See
//! `verification_and_validation/depletion_chain_xml/chain_xml_read_2026_09_23.md`.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;

fn inputs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/depletion_chain_xml/openmc_inputs")
}

/// A float as its IEEE-754 bit pattern in decimal — what `dump_chain.py`'s
/// `bits()` writes, so the two sides cannot disagree merely on spelling.
fn bits(x: f64) -> String {
    x.to_bits().to_string()
}

fn or_dash(t: Option<&String>) -> &str {
    t.map(String::as_str).unwrap_or("-")
}

/// Build the same flat dump `dump_chain.py` writes, from our own parse.
fn dump(chain: &DepletionChainXml) -> Vec<String> {
    let mut out = Vec::new();
    for n in &chain.nuclides {
        let hl = match n.half_life_seconds {
            Some(t) => bits(t),
            None => "-".to_string(),
        };
        // Upstream reads `decay_energy` only when a half-life is present and
        // otherwise leaves the attribute's default 0.0 in place, so a stable
        // nuclide always dumps 0.0 there.
        out.push(format!(
            "nuc|{}|{}|{}",
            n.name,
            hl,
            bits(n.decay_energy_ev.unwrap_or(0.0))
        ));
        for (i, d) in n.decays.iter().enumerate() {
            out.push(format!(
                "dec|{}|{i}|{}|{}|{}",
                n.name,
                d.mode,
                or_dash(d.target.as_ref()),
                bits(d.branching_ratio)
            ));
        }
        for (i, r) in n.reactions.iter().enumerate() {
            out.push(format!(
                "rxn|{}|{i}|{}|{}|{}|{}",
                n.name,
                r.kind,
                or_dash(r.target.as_ref()),
                bits(r.q_ev),
                bits(r.branching_ratio)
            ));
        }
        let mut fy: Vec<String> = n
            .fission_yields
            .iter()
            .flat_map(|set| {
                set.yields.iter().map(move |(product, value)| {
                    format!(
                        "fy|{}|{}|{product}|{}",
                        n.name,
                        bits(set.energy_ev),
                        bits(*value)
                    )
                })
            })
            .collect();
        fy.sort();
        out.extend(fy);
        let mut src: Vec<String> = n
            .sources
            .iter()
            .map(|s| format!("src|{}|{}", n.name, s.particle))
            .collect();
        src.sort();
        src.dedup();
        out.extend(src);
    }
    out
}

fn compare(stem: &str, expect_nuclides: usize, expect_records: usize) {
    let dir = inputs_dir();
    let xml_path = dir.join(format!("{stem}.xml"));
    let ref_path = dir.join(format!("openmc_{stem}.txt"));
    let chain = DepletionChainXml::read_file(&xml_path)
        .unwrap_or_else(|e| panic!("{}: {e}", xml_path.display()));
    let ours = dump(&chain);

    let reference = std::fs::read_to_string(&ref_path)
        .unwrap_or_else(|e| panic!("{}: {e}", ref_path.display()));
    let theirs: Vec<&str> = reference
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .collect();

    assert_eq!(
        chain.nuclides.len(),
        expect_nuclides,
        "{stem}: nuclide count"
    );
    assert_eq!(theirs.len(), expect_records, "{stem}: reference record count");

    // Report every disagreement, not just the first — a single wrong default
    // shows up on many lines and the pattern is the diagnosis.
    let mut diffs = Vec::new();
    for (i, (a, b)) in ours.iter().zip(&theirs).enumerate() {
        if a != *b {
            diffs.push(format!("  line {i}:\n    ours   {a}\n    openmc {b}"));
        }
    }
    if ours.len() != theirs.len() {
        diffs.push(format!(
            "  record count: ours {} vs openmc {}",
            ours.len(),
            theirs.len()
        ));
    }
    assert!(
        diffs.is_empty(),
        "{stem}: {} disagreement(s) with OpenMC's own parse:\n{}",
        diffs.len(),
        diffs.join("\n")
    );
    println!("{stem}: {expect_nuclides} nuclides, {} records, 0 disagreements", ours.len());
}

/// The 9-nuclide notebook chain: `"nothing"` sentinel, fission Q values,
/// six-product thermal yields, a `type=" beta"` with a leading space.
#[test]
fn chain_simple_matches_openmcs_own_parse() {
    compare("chain_simple", 9, 36);
}

/// The same chain with decay radiation sources and two metastable states —
/// covers a nuclide with a half-life but no decay branches, a real
/// `decay_energy`, and `<source>` elements of both `discrete` and `tabular` kind.
#[test]
fn chain_simple_decay_matches_openmcs_own_parse() {
    compare("chain_simple_decay", 11, 43);
}

/// 21 nuclides with `(n,2n)`, `(n,p)`, `(n,a)`, targetless reactions, a
/// targetless `ec/beta+` decay, and three source particles on one nuclide.
#[test]
fn chain_ni_matches_openmcs_own_parse() {
    compare("chain_ni", 21, 97);
}
