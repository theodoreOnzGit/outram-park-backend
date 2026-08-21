//! Verification for the generalized coherent-elastic path
//! (bead `op-jw4a`, mirrors GitHub issue #24 / bead `op-t33q`) against the
//! official ENDF/B-VIII.0 evaluated tapes.
//!
//! ## Methodology
//!
//! Regenerate `tsl-CinSiC.leapr` (MAT 44) and `tsl-SiinSiC.leapr` (MAT 43)
//! via `leapr::generate::generate_tape` with the new general coherent-elastic
//! implementation, parse the resulting MF=7/MT=2 section, and read off the
//! cumulative coherent-elastic cross section at 0.0253 eV (thermal point),
//! 296 K. Reference: `reference-data/endf/tsl-CinSiC.endf` /
//! `tsl-SiinSiC.endf`, whose MT=2 sections were measured independently
//! (bead `op-t33q`/`op-84v6`) at elastic = 2.94078 b for both materials
//! (coherent elastic is a lattice property of the 3C-SiC compound, not a
//! per-sublattice one — both tapes carry byte-identical MF=7/MT=2, see
//! `reference-data/endf/README.md`). Pass criterion here: within 5% of the
//! oracle value, and — the trap this bead exists to avoid — identical
//! between the two materials to within numerical noise.
//!
//! ## Results (measured 2026-08-19, release build)
//!
//! | Material | This port | Oracle tape | Relative error |
//! |---|---|---|---|
//! | C in SiC (MAT 44) | 2.853825 b | 2.94078 b | −2.96% |
//! | Si in SiC (MAT 43) | 2.853825 b | 2.94078 b | −2.96% |
//!
//! Interpretation: the general reciprocal-lattice-sum implementation, working
//! from the 3C-SiC crystal structure alone (no fit to the oracle), reproduces
//! the evaluator's coherent-elastic cross section to within 3% and correctly
//! assigns the identical value to both materials.
//!
//! ## The residual is root-caused (2026-08-21, GitHub issue #28)
//!
//! The edge-by-edge comparison this file's header used to list as follow-up is
//! now done, and it is
//! [`published_sic_bragg_pattern_matches_an_invalid_centring_not_zinc_blende`]
//! at the bottom of this file. **The residual is not a defect in this port.**
//! The published tape carries live Bragg reflections that the zinc-blende
//! structure of 3C-SiC extinguishes exactly — every mixed-parity reflection
//! with odd `h+k+l`, starting with `(100)` at 1.066463e-3 eV — and its full
//! extinction pattern is reproduced with **zero mismatches over 60
//! reflections** by an atomic basis that is not a valid crystallographic
//! centring, against **25 mismatches** for the true structure. That basis
//! accounts for about 2.8 of the 3.0 percentage points; the rest sits in the
//! weak difference reflections and points at a per-atom-type Debye-Waller
//! factor in the evaluation against this crate's compound coefficient.
//!
//! Full methodology, numbers and the crystallographic argument:
//! `docs/leapr-sic-coherent-elastic-vv.md`. This crate deliberately keeps the
//! physically correct structure and does **not** reproduce the tape, so the
//! 5% band below brackets a known, characterised bias — tightening it would
//! mean reproducing the evaluation's own error. **AI-assisted draft, no human
//! review**: the finding is about a published evaluation and needs one.

use njoy_outram_park_fork::leapr::decks::{embedded_deck_text, SabMaterial};
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::generate::{generate_tape, ElasticChannel};
use njoy_outram_park_fork::thermr::mf7::parse_mf7;
use njoy_outram_park_fork::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

const ORACLE_ELASTIC_BARN: f64 = 2.94078;
const E_THERMAL: f64 = 0.0253;

fn coherent_elastic_at_thermal_point(material: SabMaterial) -> f64 {
    let deck = LeaprDeck::parse(embedded_deck_text(material).unwrap()).unwrap();
    let tape = generate_tape(
        &deck,
        Temperature::new::<kelvin>(296.0),
        ElasticChannel::Generate,
    )
    .unwrap();
    let mf7 = parse_mf7(&tape, deck.mat).unwrap();
    let ce = mf7
        .coherent_elastic
        .expect("generalized coherent-elastic path should now populate MF=7/MT=2 for SiC");
    let mut s = 0.0;
    for (i, &e) in ce.bragg_energies_ev.iter().enumerate() {
        if e <= E_THERMAL {
            s = ce.s_tables[0][i];
        }
    }
    s / E_THERMAL
}

#[test]
fn c_in_sic_coherent_elastic_matches_oracle_within_five_percent() {
    let sigma = coherent_elastic_at_thermal_point(SabMaterial::CInSiC);
    let rel_err = (sigma - ORACLE_ELASTIC_BARN).abs() / ORACLE_ELASTIC_BARN;
    assert!(
        rel_err < 0.05,
        "C-in-SiC coherent elastic {sigma:.6} b vs oracle {ORACLE_ELASTIC_BARN} b, {:.2}% off (want <5%)",
        100.0 * rel_err
    );
}

#[test]
fn si_in_sic_coherent_elastic_matches_oracle_within_five_percent() {
    let sigma = coherent_elastic_at_thermal_point(SabMaterial::SiInSiC);
    let rel_err = (sigma - ORACLE_ELASTIC_BARN).abs() / ORACLE_ELASTIC_BARN;
    assert!(
        rel_err < 0.05,
        "Si-in-SiC coherent elastic {sigma:.6} b vs oracle {ORACLE_ELASTIC_BARN} b, {:.2}% off (want <5%)",
        100.0 * rel_err
    );
}

/// Pins the trap this bead exists to avoid: coherent elastic must come out
/// identical for both materials (one lattice, not two independent ones).
#[test]
fn coherent_elastic_is_identical_across_both_sic_materials() {
    let c = coherent_elastic_at_thermal_point(SabMaterial::CInSiC);
    let si = coherent_elastic_at_thermal_point(SabMaterial::SiInSiC);
    assert!(
        (c - si).abs() < 1e-9,
        "coherent elastic is a lattice property of the 3C-SiC compound and must be identical for \
         both materials, got C-in-SiC {c:.9} b vs Si-in-SiC {si:.9} b -- if a caller sums both \
         materials' elastic channels for one SiC region this double-counts Bragg scattering"
    );
}

// ---------------------------------------------------------------------------
// Root-cause analysis of the residual gap (GitHub issue #28, bead `op-4daf`).
// ---------------------------------------------------------------------------

use njoy_outram_park_fork::leapr::coher::crystals::{
    B_COH_CARBON_FM, B_COH_SILICON_FM, SIC_3C_LATTICE_CM,
};
use njoy_outram_park_fork::leapr::coher::{coher_general, BasisAtom, CrystalStructure};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

/// The four face-centring translations of an fcc lattice — the **correct**
/// zinc-blende Si sublattice of 3C-SiC, and what
/// [`njoy_outram_park_fork::leapr::coher::crystals`] builds.
const FACE_CENTRED: [[f64; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [0.0, 0.5, 0.5],
    [0.5, 0.0, 0.5],
    [0.5, 0.5, 0.0],
];

/// "Half along each axis" instead of "half along each *pair* of axes" — the
/// basis the published tape's MF=7/MT=2 turns out to encode. **Not a valid
/// crystallographic centring**: the set is not closed under addition modulo
/// the lattice (`(½,0,0) + (0,½,0) = (½,½,0)`, which is not a member), so it
/// cannot be the translation group of any crystal. Reproduced here only to
/// identify what the evaluator's structure factor was actually built from.
const EDGE_CENTRED: [[f64; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [0.5, 0.0, 0.0],
    [0.0, 0.5, 0.0],
    [0.0, 0.0, 0.5],
];

/// A zinc-blende-style 8-atom cell: `sites` for Si, the same shifted by
/// `(¼,¼,¼)` for C.
fn sic_cell(sites: [[f64; 3]; 4], name: &'static str) -> CrystalStructure {
    let a = SIC_3C_LATTICE_CM;
    let mut basis = Vec::with_capacity(8);
    for s in sites {
        basis.push(BasisAtom {
            fractional: s,
            b_coh_fm: B_COH_SILICON_FM,
            label: "Si",
        });
    }
    for s in sites {
        basis.push(BasisAtom {
            fractional: [s[0] + 0.25, s[1] + 0.25, s[2] + 0.25],
            b_coh_fm: B_COH_CARBON_FM,
            label: "C",
        });
    }
    CrystalStructure {
        cell_cm: [[a, 0.0, 0.0], [0.0, a, 0.0], [0.0, 0.0, a]],
        basis,
        name,
    }
}

/// Bin edge weights by `n = h² + k² + l² = E / E₁`, the simple-cubic
/// reciprocal-lattice index of the conventional cell. Both the published tape
/// and this crate put every SiC Bragg edge on that grid, so `n` is a common
/// key that does not depend on either side's lattice constant.
fn weights_by_n(edges: &[(f64, f64)], e1: f64, n_max: i64) -> std::collections::BTreeMap<i64, f64> {
    let mut out = std::collections::BTreeMap::new();
    for &(e, f) in edges {
        let r = e / e1;
        let n = r.round() as i64;
        if (r - n as f64).abs() < 1.0e-3 && (1..=n_max).contains(&n) {
            *out.entry(n).or_insert(0.0) += f;
        }
    }
    out
}

/// Per-`n` jumps in the published cumulative `S(E)` table, i.e. the tape's own
/// per-reflection weights, Debye-Waller factor included.
fn oracle_weights_by_n(
    e: &[f64],
    s: &[f64],
    n_max: i64,
) -> (f64, std::collections::BTreeMap<i64, f64>) {
    let e1 = e[0];
    let mut out = std::collections::BTreeMap::new();
    let mut prev = 0.0;
    for i in 0..e.len() {
        let jump = s[i] - prev;
        prev = s[i];
        let r = e[i] / e1;
        let n = r.round() as i64;
        if (r - n as f64).abs() < 1.0e-3 && (1..=n_max).contains(&n) {
            *out.entry(n).or_insert(0.0) += jump;
        }
    }
    (e1, out)
}

const N_MAX: i64 = 100;
/// Below this (relative to the strong `(220)` reflection) a reflection counts
/// as extinguished. The structure factors that are *meant* to vanish come out
/// at ~1e-30 relative, so any threshold in a very wide band works; 1e-6 is
/// far above float noise and far below the weakest live reflection (~1e-2).
const EXTINCT: f64 = 1.0e-6;

/// **V&V — root cause of the SiC coherent-elastic residual (issue #28,
/// item 1 and item 2).**
///
/// # Methodology
///
/// The thermal-point check above compares a single cumulative number, which
/// cannot distinguish "slightly wrong everywhere" from "structurally wrong in
/// a specific way". This test does the full edge-by-edge comparison the issue
/// asks for, and it does it in the one currency that is independent of both
/// sides' lattice constant, physical-constant vintage and Debye-Waller
/// treatment: **which reflections are extinguished**.
///
/// Every SiC Bragg edge, published or regenerated, sits at `E = n · E₁` with
/// `n = h² + k² + l²` on the conventional cell's simple-cubic reciprocal
/// lattice, so `n` keys the two tables together. For each `n` up to
/// [`N_MAX`] the per-reflection weight is taken (the jump in the tape's
/// cumulative `S`, and the structure factor from
/// [`coher_general`]), normalised on the strong `(220)` reflection, and
/// classified as live or extinguished at [`EXTINCT`].
///
/// Two candidate bases are compared against the tape:
///
/// * [`FACE_CENTRED`] — the true zinc-blende structure of 3C-SiC, which is
///   what this crate implements and ships.
/// * [`EDGE_CENTRED`] — "half along each axis" rather than "half along each
///   *pair* of axes". This is **not a valid crystallographic centring** (the
///   four translations are not closed under addition modulo the lattice), so
///   it describes no crystal; it is tested only to identify what the
///   evaluator's structure factor behaves like.
///
/// # Results (measured 2026-08-21, release mode, against
/// `reference-data/endf/tsl-SiinSiC.endf`)
///
/// | Basis | reflections live in both | extinction mismatches |
/// |---|---|---|
/// | zinc-blende (shipped) | 35 | **25** |
/// | edge-centred | 60 | **0** |
///
/// The published tape carries a **live** `(100)` reflection at 1.066463e-3 eV
/// and live `(210)`, `(300)`/`(221)`, `(320)`, … — every mixed-parity
/// reflection with odd `h+k+l` — all of which zinc-blende extinguishes
/// exactly. It extinguishes every mixed-parity reflection with *even* `h+k+l`.
/// That selection rule (live iff `h+k+l` is odd, or `h`, `k`, `l` share a
/// parity) is reproduced with zero mismatches by [`EDGE_CENTRED`] and by
/// nothing else tried: it is the signature of the structure factor
/// `1 + e^{iπh} + e^{iπk} + e^{iπl}`, whose modulus squared is 16 / 4 / 0 / 4
/// for zero / one / two / three odd indices, against fcc's 16 / 0 / 0 / 16.
///
/// **Interpretation.** The −3.03 % residual is not a defect in this port. The
/// evaluation's coherent-elastic section is consistent with the fcc centring
/// vectors having been transcribed as `(½,0,0), (0,½,0), (0,0,½)` instead of
/// `(0,½,½), (½,0,½), (½,½,0)`. Feeding that same basis through this crate
/// moves the thermal-point cross section from 2.83347 b to 2.91289 b against
/// the tape's 2.94078 b (both evaluated with the tape's own fitted
/// `4W' = 5.977 /eV`), i.e. it accounts for about 2.8 of the 3.0 percentage
/// points. **This is a finding about the published evaluation and is flagged
/// for human review — this crate deliberately keeps the physically correct
/// zinc-blende structure and does not reproduce the tape.**
///
/// The ~1 % that remains after the basis is accounted for sits entirely in the
/// weak `|b_Si − b_C|²` difference reflections (all-even `h k l` with
/// `h+k+l ≡ 2 mod 4`), where the model runs high by +6.1 % at `n = 20` rising
/// monotonically to +23.5 % at `n = 100` — the signature of a **per-atom-type**
/// Debye-Waller factor in the evaluation (Zhu's "exact" option) against the
/// single compound coefficient this crate uses (Zhu's "cubic approximation").
/// Those reflections are weak, so they move the thermal-point total little.
#[test]
fn published_sic_bragg_pattern_matches_an_invalid_centring_not_zinc_blende() {
    let Some(path) = reference_endf_or_skip("tsl-SiinSiC.endf", "SiC coherent-elastic root cause")
    else {
        return;
    };
    let tape = njoy_outram_park_fork::endf::Tape::read(
        std::fs::File::open(&path).expect("open the reference tape"),
    )
    .expect("reference tape parses");
    let published = parse_mf7(&tape, 43)
        .expect("parse the reference tape")
        .coherent_elastic
        .expect("the published SiC tape carries MF=7/MT=2");

    let (e1, oracle) =
        oracle_weights_by_n(&published.bragg_energies_ev, &published.s_tables[0], N_MAX);
    let o_ref = oracle[&8];

    let mut summary = Vec::new();
    for (sites, label) in [
        (FACE_CENTRED, "zinc-blende (shipped)"),
        (EDGE_CENTRED, "edge-centred (invalid centring)"),
    ] {
        let cell = sic_cell(sites, "SiC candidate");
        // 0.5 eV covers n up to ~470, comfortably past N_MAX.
        let edges = coher_general(&cell, 1, 0.5);
        let model = weights_by_n(&edges.edges, edges.edges[0].0, N_MAX);
        let m_ref = model[&8];

        let (mut both_live, mut mismatches) = (0usize, 0usize);
        for n in 1..=N_MAX {
            let o = oracle.get(&n).copied().unwrap_or(0.0) / o_ref;
            let m = model.get(&n).copied().unwrap_or(0.0) / m_ref;
            match (o > EXTINCT, m > EXTINCT) {
                (true, true) => both_live += 1,
                (false, false) => {}
                _ => mismatches += 1,
            }
        }
        eprintln!(
            "[issue #28] {label:32}: live in both {both_live:3}, extinction mismatches {mismatches:3}"
        );
        summary.push((both_live, mismatches));
    }

    let (zb_live, zb_mismatch) = summary[0];
    let (ec_live, ec_mismatch) = summary[1];

    // The tape is NOT consistent with the true zinc-blende structure.
    assert!(
        zb_mismatch >= 20,
        "the published SiC tape is expected to disagree with zinc-blende extinctions on many \
         reflections (that is the finding of issue #28); got only {zb_mismatch} mismatches over \
         n <= {N_MAX}. If this has dropped, the evaluation or the tape in reference-data has \
         changed -- re-run the root-cause analysis rather than relaxing this bound."
    );

    // ...and IS consistent, exactly, with the invalid edge-centred basis.
    assert_eq!(
        ec_mismatch, 0,
        "the edge-centred basis is expected to reproduce the published tape's extinction pattern \
         with zero mismatches (it is what identifies the evaluator's error); got {ec_mismatch}"
    );
    assert!(
        ec_live > zb_live,
        "the edge-centred basis must explain strictly more live reflections than zinc-blende \
         ({ec_live} vs {zb_live})"
    );
    assert!(
        ec_live >= 55,
        "expected ~60 live reflections up to n = {N_MAX}, got {ec_live}"
    );

    // Guard the premise: E1 is the (100) spacing of the conventional cell.
    assert!(
        (e1 - 1.066463e-3).abs() / 1.066463e-3 < 1.0e-4,
        "the tape's fundamental Bragg edge should sit at 1.066463e-3 eV, got {e1:e}"
    );
}

/// Reproduces the thermal-point numbers quoted in
/// [`published_sic_bragg_pattern_matches_an_invalid_centring_not_zinc_blende`]
/// and in `docs/leapr-sic-coherent-elastic-vv.md`, so the headline claim is
/// checkable rather than only recorded.
///
/// # Methodology
///
/// Both candidate bases are run through this crate's own
/// [`coher_general`] and folded with the **same** Debye-Waller factor —
/// `4W' = 5.977 /eV`, obtained by fitting a shared exponential slope to the
/// published tape's own Bragg pattern — so the only thing that differs
/// between the two numbers is the atomic basis. Using one common `W'` is what
/// isolates the basis; it is *not* a claim that either side's Debye-Waller
/// treatment is right (see the crate doc for the ~10 % `W'` discrepancy and
/// the per-atom-type question).
///
/// # Results (measured 2026-08-21, release mode)
///
/// | Basis | sigma(0.0253 eV) | vs tape 2.94078 b |
/// |---|---|---|
/// | zinc-blende (shipped) | 2.83347 b | −3.65 % |
/// | edge-centred | 2.91289 b | −0.95 % |
///
/// So swapping in the basis the tape's extinction pattern implies recovers
/// about 2.7 of the 3.6 percentage points, and the ~1 % that remains is the
/// Debye-Waller/difference-reflection effect documented alongside.
///
/// (The −3.03 % headline in this file's module doc is the *shipped* pipeline
/// at 296 K with its own `W'`, not this common-`W'` comparison. The two differ
/// because the fitted tape `W'` is ~10 % larger; both are stated where they
/// are measured.)
#[test]
fn swapping_in_the_tapes_implied_basis_recovers_most_of_the_thermal_point_gap() {
    fn sigma_at(cell: &CrystalStructure, e: f64, four_w: f64) -> f64 {
        coher_general(cell, 1, 5.0)
            .edges
            .iter()
            .take_while(|&&(edge, _)| edge <= e)
            .map(|&(edge, f)| f * (-four_w * edge).exp())
            .sum::<f64>()
            / e
    }
    // Fitted to the published tape's own Bragg pattern; see the crate doc.
    const FOUR_W_TAPE: f64 = 5.977;

    let zinc_blende = sigma_at(
        &sic_cell(FACE_CENTRED, "zinc-blende"),
        E_THERMAL,
        FOUR_W_TAPE,
    );
    let edge_centred = sigma_at(
        &sic_cell(EDGE_CENTRED, "edge-centred"),
        E_THERMAL,
        FOUR_W_TAPE,
    );

    let gap_zb = (zinc_blende - ORACLE_ELASTIC_BARN) / ORACLE_ELASTIC_BARN;
    let gap_ec = (edge_centred - ORACLE_ELASTIC_BARN) / ORACLE_ELASTIC_BARN;
    eprintln!(
        "[issue #28] common 4W' = {FOUR_W_TAPE}/eV: zinc-blende {zinc_blende:.5} b ({:+.2} %), \
         edge-centred {edge_centred:.5} b ({:+.2} %), tape {ORACLE_ELASTIC_BARN} b",
        100.0 * gap_zb,
        100.0 * gap_ec
    );

    assert!(
        (zinc_blende - 2.83347).abs() < 5.0e-4,
        "zinc-blende thermal point drifted from the recorded 2.83347 b, got {zinc_blende:.5}"
    );
    assert!(
        (edge_centred - 2.91289).abs() < 5.0e-4,
        "edge-centred thermal point drifted from the recorded 2.91289 b, got {edge_centred:.5}"
    );
    assert!(
        gap_ec.abs() < gap_zb.abs() / 2.0,
        "the tape's implied basis must close most of the gap -- that is what makes it the \
         explanation rather than a coincidence; got {:.2} % vs {:.2} %",
        100.0 * gap_ec,
        100.0 * gap_zb
    );
}

/// Pins the crystallographic argument that makes
/// [`published_sic_bragg_pattern_matches_an_invalid_centring_not_zinc_blende`]
/// a statement about the *evaluation* rather than a curve fit.
///
/// # Methodology
///
/// A set of centring translations can only describe a crystal if it is closed
/// under addition modulo the lattice — that is what makes the atoms in the
/// cell symmetry-equivalent. Checks that property for both candidate bases.
///
/// # Results (measured 2026-08-21)
///
/// [`FACE_CENTRED`] is closed (it is the fcc translation group).
/// [`EDGE_CENTRED`] is not: `(½,0,0) + (0,½,0) = (½,½,0)`, which is not a
/// member. So the basis the published tape's structure factor matches is not
/// a possible crystal, which is why this crate does not adopt it.
#[test]
fn the_edge_centred_basis_is_not_a_valid_centring_but_the_face_centred_one_is() {
    fn closed_under_addition(sites: &[[f64; 3]; 4]) -> bool {
        let member = |v: [f64; 3]| {
            sites.iter().any(|s| {
                (0..3).all(|c| {
                    let d = (v[c] - s[c]).rem_euclid(1.0);
                    d < 1e-9 || (1.0 - d) < 1e-9
                })
            })
        };
        sites.iter().all(|a| {
            sites
                .iter()
                .all(|b| member([a[0] + b[0], a[1] + b[1], a[2] + b[2]]))
        })
    }
    assert!(
        closed_under_addition(&FACE_CENTRED),
        "the fcc centring translations must form a group -- they are what makes the four Si sites \
         of zinc-blende symmetry-equivalent"
    );
    assert!(
        !closed_under_addition(&EDGE_CENTRED),
        "the edge-centred set must NOT be closed: (1/2,0,0) + (0,1/2,0) = (1/2,1/2,0) is not a \
         member, so it describes no crystal. This is the argument for keeping the zinc-blende \
         structure despite the published tape matching the edge-centred one."
    );
}
