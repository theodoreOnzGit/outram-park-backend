//! V&V: regenerate graphite's **coherent-elastic** MF=7/MT=2 section from the
//! 12 KB LEAPR card deck and compare it point by point against the official
//! ENDF/B-VIII.0 tape.
//!
//! # Why this test exists
//!
//! `leapr_graphite_deck_parity.rs` validated the *inelastic* MF=7/MT=4 law to
//! the tape's printed precision, and said so explicitly about its own scope:
//! "this validates MT=4 only ... neither is the MT=2 elastic output". That
//! left the dominant channel unchecked. MT=4 is 99.6 % of the tape's **bytes**,
//! but MT=2 is roughly 90 % of graphite's **thermal cross section** — 4.55 b
//! coherent-elastic against 0.49 b inelastic at 0.0253 eV. Making deck
//! regeneration the default source of graphite `S(alpha, beta)` therefore could
//! not be justified on the MT=4 result alone. This file closes that gap.
//!
//! Two things had never been checked before this file:
//!
//! 1. **Bragg edge positions.** [`coher`] carries hand-transcribed graphite
//!    lattice constants with an explicit `HUMAN RE-VERIFY` marker
//!    (`src/leapr/coher.rs:81`). That the port generates a plausible *number*
//!    of edges said nothing about where they sit.
//! 2. **The absolute Debye-Waller scale.** The MT=4 file could only measure a
//!    *difference* `W'(T_a) - W'(T_b)` (0.48 % agreement), because its method
//!    cancelled the structure factors. It stated plainly that "an error common
//!    to every temperature would survive it". MT=2 tabulates absolute `S(E, T)`
//!    values, so it can pin the constant — and does, below.
//!
//! ## Data / provenance
//!
//! - **Deck:** `tsl-crystalline-graphite.leapr` (12,444 bytes), ENDF/B-VIII.0
//!   thermal-scattering sublibrary (2018; open-source per `DATA_POLICY.md`),
//!   evaluated by A.I. Hawari, Y. Zhu and J.L. Wormald (Low Energy Interaction
//!   Physics group, North Carolina State University), `EVAL-SEP17`.
//! - **Reference tape:** `tsl-crystalline-graphite.endf` (8,730,804 bytes),
//!   MAT 30, MF=7/MT=2, `LTHR = 1`, `LT = 9` (ten tabulated temperatures),
//!   lin-lin temperature interpolation (`LI = 2`).
//! - **Kernel:** port of NJOY2016 `leapr.f90` (commit `ac5adf5f`) — `coher`
//!   (2489-2814) and `endout`'s coherent-elastic writer (3192-3289).
//! - **NJOY constant vintages** are read from NJOY2016's own git history
//!   (`src/phys.f90`), not guessed — see [`SEP17 constants`](Sep17Econ) below.
//! - Neither data file is checked in. Tests read them from `GRAPHITE_TSL_DIR`
//!   (env override) or the default directory, and **skip** (print a note, pass)
//!   when absent.
//!
//! # Methodology
//!
//! 1. Parse the deck; confirm `iel = 1` (graphite) and `npr = 1`.
//! 2. Run [`coher`] at `emax = 5 eV` — the value `leapr.f90:415` hard-codes at
//!    the call site — obtaining 345 raw Bragg edges.
//! 3. Compute the Debye-Waller integral `W'(T) = lambda(T) / (awr * T * k_B)`
//!    for each of the deck's ten temperatures from
//!    [`FrequencyModel::start`]'s `f0`, which is exactly LEAPR's
//!    `dwpix(itemp) = f0` (`leapr.f90:715`) before its `/(awr*T*bk)` conversion
//!    (`leapr.f90:3035`). Graphite has `nd = 0` and `twt = 0`, so neither the
//!    discrete-oscillator nor the translational correction to `dwpix` applies.
//! 4. Emit MF=7/MT=2 through [`endout`], which applies NJOY's `1/E` tail
//!    thinning and writes the base-temperature TAB1 plus nine extra-temperature
//!    LISTs.
//! 5. Parse **both** our tape and the official one with the same THERMR reader
//!    ([`parse_mf7`]) and compare the Bragg energy grid and every `S(E_i, T_j)`
//!    entry, at all ten temperatures.
//!
//! **Pass criterion.** Identical retained-edge count; max relative deviation
//! `< 2e-6` and RMS `< 1e-6` on both the Bragg energies and `S(E, T)` at every
//! temperature. The bar is set by how the reference file stores its numbers:
//! `endout` rounds each with `sigfig(x, 7, 0)`, so a *single* value carries up
//! to `5e-7 / m` relative round-off for a mantissa `m` in `[1, 10)`, and two
//! independently-rounded values whose underlying values differ slightly can
//! land up to about `1.5e-6` apart at `m -> 1`.
//!
//! ## Why a minimal MT=4 filler is sound
//!
//! [`endout`] emits MT=2 and MT=4 together, so a [`LeaprOutput`] must carry an
//! `ssm` array. These tests supply a 3 x 3 constant one rather than paying ~18 s
//! to regenerate the real 150 x 400 phonon expansion at ten temperatures.
//! That is not a shortcut past anything: `build_coherent_elastic` reads only
//! `za`, `awr`, `temperatures_k`, `dwpix` and the edge list — it never touches
//! `alpha`, `beta`, `ssm` or `tempf`. The inelastic law is separately validated
//! in `leapr_graphite_deck_parity.rs`.
//!
//! # Measured results (2026-08-13, ENDF/B-VIII.0, release mode)
//!
//! Every number below was produced by running these tests against the actual
//! files on 2026-08-13.
//!
//! **Edge retention.** `coher` generates **345** edges below 5 eV; NJOY's `1/E`
//! thinning (`tol = 0.9e-7`) retains **221**, and the official tape carries
//! **exactly 221**. The thinning is therefore active and correctly reproduced,
//! and the earlier 345-vs-221 discrepancy is the expected behaviour, not a
//! defect.
//!
//! **Bragg edge energies** (`i` indexes the retained grid):
//!
//! | quantity | value |
//! |---|---|
//! | max relative deviation | **9.937e-7** at `i = 197`, `E = 1.006380 eV` |
//! | RMS relative deviation | **5.512e-7** over 221 edges |
//!
//! **Cumulative structure factor `S(E, T)`**, per tabulated temperature:
//!
//! | T \[K\] | max rel. dev. | at `E` \[eV\] | RMS rel. dev. |
//! |---|---|---|---|
//! | 296 | 9.047e-7 | 0.020837 | 2.854e-7 |
//! | 400 | 9.308e-7 | 0.020917 | 2.741e-7 |
//! | 500 | 9.771e-7 | 0.020837 | 3.102e-7 |
//! | 600 | 9.779e-7 | 0.022164 | 2.621e-7 |
//! | 700 | 8.678e-7 | 0.031611 | 2.324e-7 |
//! | 800 | 8.738e-7 | 0.033673 | 2.528e-7 |
//! | 1000 | 9.986e-7 | 0.031611 | 2.512e-7 |
//! | 1200 | 9.432e-7 | 0.042705 | 1.684e-7 |
//! | 1600 | 8.044e-7 | 0.001822 | 1.557e-7 |
//! | 2000 | 8.238e-7 | 0.001822 | 1.576e-7 |
//!
//! Over all ten temperatures together: max **9.986e-7** (at 1000 K,
//! `E = 0.031611 eV`), RMS **2.408e-7** over 2,200 tabulated values. Nothing
//! anywhere in MT=2 deviates by as much as one part in a million.
//!
//! ## The residual is fully explained, and it is not the lattice constants
//!
//! The deviation in the Bragg energies is not scatter — it is a **uniform
//! multiplicative offset**. Fitted over the 220 real edges, the tape's energies
//! are `(1 + 5.115e-7)` times this port's, with a residual scatter of only
//! 2.088e-7 about that single factor — the 7-figure storage granularity (see
//! [`bragg_edge_residual_is_the_econ_constant_vintage`]).
//!
//! That offset has a single, *sourced* cause. Edge energies are
//! `E = tau^2 / econ` with `econ = ev * 8 * (amassn * amu / hbar) / hbar`
//! (`leapr.f90:2543`). `tau^2` depends only on the lattice constants, so any
//! transcription slip in `a` or `c` would move different `(hkl)` families by
//! *different* amounts — a `c`-axis error would shift `(00l)` and leave
//! `(hk0)` alone. A strictly uniform shift can only come from `econ`, i.e. from
//! the physical constants.
//!
//! Reading NJOY2016's own `src/phys.f90` out of its git history at the commit
//! preceding `007828d` (2017-10-23, "Incorporating Skip's changes") gives the
//! constants in force when the SEP17 evaluation was produced:
//!
//! ```text
//!   bk     = 8.617385e-5      amassn = 1.008664904
//!   amu    = 1.6605402e-24    hbar   = 1.05457266e-27
//!   ev     = 1.60217733e-12   clight = 2.99792458e10
//! ```
//!
//! The `bk` there is exactly the value the MT=4 parity independently needed,
//! which corroborates the vintage. Those literals give an `econ` **4.9257e-7**
//! smaller than the crate's CODATA2018-derived one, matching the fitted
//! 5.115e-7 offset to within the fit's own quantisation.
//!
//! Substituting it is decisive. With the evaluation's own `econ`, the
//! regenerated section stops merely *agreeing* with the tape and becomes
//! **identical to it**: the Bragg energies go from max 9.937e-7 / RMS 5.525e-7
//! to **max 1.001e-13 / RMS 9.986e-14**, and `S(E, T)` across all ten
//! temperatures from max 9.986e-7 to **max 1.001e-13**. That residual is the
//! float round-trip noise of parsing a 7-digit ENDF field — i.e. all 220 edge
//! energies and all 2,200 structure-factor values match the official tape **to
//! the last printed digit**.
//!
//! **Verdict on the `HUMAN RE-VERIFY` marker: the hand-transcribed graphite
//! constants are correct.** Independently, they were diffed against
//! `leapr.f90:2508-2511` character by character (`gr1 = 2.4573e-8`,
//! `gr2 = 6.700e-8`, `gr3 = 12.011`, `gr4 = 5.50`) and match, as do the
//! constants for the other five lattices and the `formf` branches
//! (`leapr.f90:2924-2970`). The marker records a real risk; the risk did not
//! materialise. Flipping it is a human's call, not this test's.
//!
//! ## What is now pinned that was not before
//!
//! Measured against the exact (evaluation-constants) baseline, at a bar of
//! 1e-6 — one unit in the tape's 7th significant figure, the smallest change
//! the file can record at all:
//!
//! - **The absolute Debye-Waller scale.** Scaling every `W'(T)` by `1 + delta`,
//!   the tape tolerates only `|delta| <= 4.91e-7` before a stored digit moves.
//!   The overall constant the MT=4 test explicitly could not see is therefore
//!   pinned to **5 parts in ten million**, against 0.48 % for the temperature
//!   *trend* alone — roughly a four-orders-of-magnitude tightening, and the
//!   direct closure of the gap that file named. `W'(296 K) = 2.860298 /eV`,
//!   `W'(2000 K) = 16.533426 /eV`.
//! - **The absolute structure-factor normalisation.** `S` is linear in the edge
//!   structure factors, so a relative error in `scon`/`scoh` (and with it the
//!   5.50 b graphite coherent cross section and the `(4 pi)^2 / (2 a^2 c
//!   sqrt(3) econ)` prefactor) appears one-for-one in `S` — confirmed, a
//!   1e-4 perturbation produces a 1.0065e-4 response. Measured tolerance:
//!   `|epsilon| <= 5.12e-7`. Nothing in this crate had previously checked
//!   `scoh`, `scon` or the `formf` normalisation against any reference value.
//!
//! Both figures are sensitivities, not discrepancies: the measured values sit
//! at the centre of those intervals, because the unperturbed comparison is
//! exact.
//!
//! # Interpretation
//!
//! Coherent-elastic regeneration reproduces the official ENDF/B-VIII.0 graphite
//! MF=7/MT=2 section to the tape's own printed precision, at all ten tabulated
//! temperatures, with the same retained-edge count and no point deviating by
//! one part in a million — and *exactly*, to the last printed digit, when run
//! with the constants the evaluation itself was produced with. Combined with
//! the MT=4 result, **both** channels of the graphite evaluation are now
//! validated against the tape, so the objection that the dominant scattering
//! channel rested on an untested generator no longer stands.
//!
//! ## Follow-up this test raises (not fixed here)
//!
//! [`PhysicalConstants`] currently models the evaluation vintage through `k_B`
//! alone, and [`coher`] reads `common::phys` directly, with no way to ask it
//! for a different constant set. The SEP17 vintage also carried different `ev`,
//! `amu`, `hbar` and `amassn`, and those move every Bragg edge by 4.93e-7
//! through `econ` — which is, as measured above, the *entire* residual in the
//! primary comparison. Threading the constant set into `coher` would make
//! regeneration bit-exact rather than 1e-6-exact. It is deliberately **not**
//! done here: the effect is three orders of magnitude below physical
//! significance for a Bragg edge, and the change belongs to whoever owns the
//! vintage machinery in `src/leapr/vintage.rs`.
//!
//! Scope limits, stated plainly. This validates one moderator (graphite,
//! `iel = 1`, hexagonal) at `emax = 5 eV`. The other five lattices in [`coher`]
//! — Be, BeO, Al, Pb, Fe — and the fcc/bcc index-box branch are **not** touched
//! by it; neither is incoherent elastic (`LTHR = 2`). And it validates
//! *regeneration at the deck's own tabulated temperatures*: generating MT=2 at
//! an untabulated temperature is the same code path, but the physics caveat on
//! [`LeaprDeck::input_at_temperature`] (a temperature-independent `rho(E)`)
//! still applies to the `W'(T)` it would use.
//!
//! [`coher`]: njoy_outram_park_fork::leapr::coher::coher
//! [`PhysicalConstants`]: njoy_outram_park_fork::leapr::vintage::PhysicalConstants
//! [`endout`]: njoy_outram_park_fork::leapr::endout::endout
//! [`parse_mf7`]: njoy_outram_park_fork::thermr::mf7::parse_mf7
//! [`FrequencyModel::start`]: njoy_outram_park_fork::leapr::frequency::FrequencyModel::start
//! [`LeaprDeck::input_at_temperature`]: njoy_outram_park_fork::leapr::deck::LeaprDeck::input_at_temperature

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::coher::{coher, BraggEdges, CoherentLattice};
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::endout::{endout, ElasticOutput, LeaprOutput};
use njoy_outram_park_fork::leapr::frequency::FrequencyModel;
use njoy_outram_park_fork::leapr::input::ElasticOption;
use njoy_outram_park_fork::leapr::vintage::PhysicalConstants;
use njoy_outram_park_fork::leapr::SabMatrix;
use njoy_outram_park_fork::thermr::mf7::{parse_mf7, CoherentElastic};

/// Default location of the ENDF/B-VIII.0 thermal-scattering sublibrary.
const DEFAULT_DIR: &str = "/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt";
/// The LEAPR job that generated the graphite evaluation.
const DECK_FILE: &str = "tsl-crystalline-graphite.leapr";
/// The ENDF tape it generated.
const TAPE_FILE: &str = "tsl-crystalline-graphite.endf";
/// MAT number carried by both.
const MAT: i32 = 30;
/// Maximum incident energy `coher` tabulates, hard-coded at LEAPR's call site
/// (`leapr.f90:415`, `emax = 5`).
const EMAX_EV: f64 = 5.0;

/// Pass bar on the maximum relative deviation, set by the tape's 7-significant-
/// figure storage (see the module docs).
const MAX_TOL: f64 = 2.0e-6;
/// Pass bar on the RMS relative deviation.
const RMS_TOL: f64 = 1.0e-6;

/// `econ_crate / econ_SEP17 - 1` — the `econ` ratio between this crate's
/// physical constants and the ones NJOY2016 carried when the SEP17 graphite
/// evaluation was produced.
///
/// `econ = ev * 8 * (amassn * amu / hbar) / hbar` (`leapr.f90:2543`) converts a
/// squared reciprocal-lattice vector to an energy, so every Bragg edge scales
/// as `1 / econ`: the returned value is the relative amount by which an edge
/// energy produced with the crate's constants falls *below* one produced with
/// NJOY's SEP17-era constants.
///
/// The SEP17-era literals are read from NJOY2016's own `src/phys.f90` at the
/// commit preceding `007828d` (2017-10-23, "Incorporating Skip's changes"):
///
/// ```text
///   ev = 1.60217733e-12   amassn = 1.008664904
///   amu = 1.6605402e-24   hbar = 1.05457266e-27
/// ```
///
/// The `bk = 8.617385e-5` alongside them in that same file is exactly the value
/// the MT=4 parity independently required, which corroborates the vintage.
/// Both sets are recomputed here rather than a bare ratio being trusted.
fn sep17_econ_ratio() -> f64 {
    // Crate / current-NJOY constants (CODATA2018), `common::phys`.
    let c = 2.997_924_58e10_f64;
    let ev_new = 1.602_176_634e-12_f64;
    let amassn_new = 1.008_664_915_95_f64;
    let amu_new = 931.494_102_42e6 * ev_new / (c * c);
    let hbar_new = 6.582_119_569e-16 * ev_new;
    // NJOY2016 `src/phys.f90` before commit 007828d (2017-10-23).
    let ev_old = 1.602_177_33e-12_f64;
    let amassn_old = 1.008_664_904_f64;
    let amu_old = 1.660_540_2e-24_f64;
    let hbar_old = 1.054_572_66e-27_f64;

    let econ = |ev: f64, amassn: f64, amu: f64, hbar: f64| 8.0 * ev * amassn * amu / (hbar * hbar);
    econ(ev_new, amassn_new, amu_new, hbar_new) / econ(ev_old, amassn_old, amu_old, hbar_old) - 1.0
}

/// The directory to look for the data files in: `GRAPHITE_TSL_DIR` if set, else
/// [`DEFAULT_DIR`].
fn data_dir() -> String {
    std::env::var("GRAPHITE_TSL_DIR").unwrap_or_else(|_| DEFAULT_DIR.to_string())
}

/// Resolve `dir/file`, or `None` plus a skip note when it is not there.
///
/// Split out from [`data_path`] with the directory as a parameter so
/// [`missing_data_skips_cleanly`] can exercise the absent-data branch without
/// mutating the process environment, which would race the other tests in this
/// binary.
fn data_path_in(dir: &str, file: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(dir).join(file);
    if p.exists() {
        Some(p)
    } else {
        eprintln!(
            "SKIP leapr_graphite_coherent_elastic_parity: {file} not found under {dir} \
             (set GRAPHITE_TSL_DIR)"
        );
        None
    }
}

/// Resolve a data-file path (env override -> default), or `None` + a skip note.
fn data_path(file: &str) -> Option<std::path::PathBuf> {
    data_path_in(&data_dir(), file)
}

/// The Bragg edges [`coher`] would produce if it ran with the physical
/// constants NJOY carried at `EVAL-SEP17`, rather than the crate's CODATA2018
/// ones.
///
/// [`coher`] reads `common::phys` directly, so there is no way to ask it for a
/// different constant set. Both the edge energy and the structure factor carry
/// exactly one factor of `1 / econ` — `E = tau^2 * recon` (`leapr.f90:2772`)
/// and `scon = scoh * (4 pi)^2 / (2 a^2 c sqrt(3) econ)` (`leapr.f90:2588`) —
/// and nothing else in `coher` touches a physical constant, `wint` being 0. So
/// scaling both outputs by the [`sep17_econ_ratio`] is exactly equivalent to
/// having run the reciprocal-lattice sum with the older constants, and it stays
/// inside `tests/`.
///
/// This is a *diagnostic*, not a correction applied to the primary comparison:
/// [`coherent_elastic_parity_against_endf_tape`] deliberately measures the
/// as-shipped code with no adjustment.
fn sep17_edges(edges: &BraggEdges) -> BraggEdges {
    let k = 1.0 + sep17_econ_ratio();
    BraggEdges {
        edges: edges.edges.iter().map(|&(e, f)| (e * k, f * k)).collect(),
    }
}

/// Parse the deck, or `None` if the data is absent.
fn load_deck() -> Option<LeaprDeck> {
    let p = data_path(DECK_FILE)?;
    let text = std::fs::read_to_string(p).expect("deck is readable");
    Some(LeaprDeck::parse(&text).expect("graphite deck parses"))
}

/// The official tape's MF=7/MT=2 coherent-elastic section, or `None` if absent.
fn load_official_mt2() -> Option<CoherentElastic> {
    let p = data_path(TAPE_FILE)?;
    let tape = Tape::read(std::fs::File::open(p).expect("tape is readable")).expect("tape parses");
    Some(
        parse_mf7(&tape, MAT)
            .expect("MF=7 parses")
            .coherent_elastic
            .expect("graphite has coherent elastic"),
    )
}

/// The Debye-Waller integral `W'(T) = lambda(T) / (awr * T * k_B)` \[1/eV\] for
/// every temperature the deck tabulates.
///
/// `lambda` is [`FrequencyModel`]'s `f0`, which is LEAPR's `dwpix(itemp) = f0`
/// (`leapr.f90:715`); the division is `leapr.f90:3035`. `constants` selects the
/// Boltzmann constant, which enters twice — once through `tev = k_B T` inside
/// the frequency model, and once in the division here.
fn debye_waller_table(deck: &LeaprDeck, constants: PhysicalConstants) -> Vec<f64> {
    deck.temperatures_k()
        .iter()
        .map(|&t_k| {
            let mut inp = deck
                .input_at_temperature(0, t_k)
                .expect("temperature block 0 exists");
            inp.constants = constants;
            let f = FrequencyModel::start(
                &inp.continuous.rho,
                inp.continuous.delta_ev,
                inp.tev(),
                inp.continuous.tbeta,
            );
            f.f0 / (deck.awr * t_k * constants.bk_ev_per_k())
        })
        .collect()
}

/// Emit MF=7/MT=2 from the given Bragg edges and `W'(T)` table, then read it
/// back with the same THERMR reader the official tape is read with.
///
/// The MT=4 filler is deliberately minimal — see the module docs for why that
/// cannot affect the coherent-elastic section.
fn regenerate_mt2(deck: &LeaprDeck, edges: &BraggEdges, dwpix: &[f64]) -> CoherentElastic {
    let temps = deck.temperatures_k();
    let alpha = vec![0.1, 0.5, 2.0];
    let beta = vec![0.0, 0.3, 1.0];
    let ssm: Vec<SabMatrix> = temps
        .iter()
        .map(|_| {
            let mut m = SabMatrix::zeros(beta.len(), alpha.len());
            for ib in 0..beta.len() {
                for ia in 0..alpha.len() {
                    m.set(ib, ia, 1.0e-3);
                }
            }
            m
        })
        .collect();

    let out = LeaprOutput {
        mat: MAT,
        za: deck.za,
        awr: deck.awr,
        lat: 1,
        isym: 0,
        ilog: false,
        smin: deck.smin,
        alpha,
        beta,
        temperatures_k: temps.clone(),
        dwpix: dwpix.to_vec(),
        tempf: temps,
        ssm,
        ssp: None,
        npr: deck.npr,
        spr: deck.spr,
        elastic: ElasticOutput::Coherent(edges.clone()),
        secondary: deck.secondary_scatterer(),
        constants: deck.constants(),
    };

    parse_mf7(&endout(&out), MAT)
        .expect("regenerated MF=7 parses")
        .coherent_elastic
        .expect("regenerated MT=2 present")
}

/// Max and RMS relative deviation of `ours` against `theirs`, plus the index of
/// the worst point.
fn deviation(ours: &[f64], theirs: &[f64]) -> (f64, f64, usize, usize) {
    let (mut max, mut at, mut sumsq, mut n) = (0.0f64, 0usize, 0.0f64, 0usize);
    for (i, (&a, &b)) in ours.iter().zip(theirs).enumerate() {
        if b == 0.0 {
            continue;
        }
        let rel = (a - b).abs() / b.abs();
        if rel > max {
            max = rel;
            at = i;
        }
        sumsq += rel * rel;
        n += 1;
    }
    (max, (sumsq / n.max(1) as f64).sqrt(), at, n)
}

/// The regenerated coherent-elastic section matches the official tape at every
/// Bragg edge and every one of the ten tabulated temperatures.
///
/// **Methodology and pass criterion:** see the module documentation. In short —
/// run `coher` at `emax = 5 eV`, build `W'(T)` from the deck's own frequency
/// spectrum with the evaluation's constant vintage, emit MF=7/MT=2 through
/// `endout`, read both tapes with the same THERMR reader, and require the same
/// retained-edge count plus max relative deviation `< 2e-6` and RMS `< 1e-6` on
/// the Bragg energies and on `S(E, T)` at every temperature.
///
/// **Result (2026-08-13):** 345 edges generated, 221 retained by NJOY's `1/E`
/// thinning, 221 in the tape — an exact match. Bragg energies: max 9.937e-7 at
/// `i = 197`, `E = 1.006380 eV`, RMS 5.512e-7 over 221 edges. `S(E, T)` over all
/// ten temperatures: max 9.986e-7 (at 1000 K, `E = 0.031611 eV`), RMS 2.408e-7
/// over 2,200 values. The per-temperature breakdown is tabulated in the module
/// docs. Nothing in MT=2 deviates by as much as one part in a million.
#[test]
fn coherent_elastic_parity_against_endf_tape() {
    let Some(d) = load_deck() else { return };
    let Some(theirs) = load_official_mt2() else {
        return;
    };

    assert_eq!(d.iel, ElasticOption::Graphite, "deck selects iel = 1");
    assert_eq!(d.npr, 1, "one principal scattering atom");
    assert_eq!(
        d.constants(),
        PhysicalConstants::Njoy2016Legacy,
        "the EVAL-SEP17 deck selects the pre-2018 NJOY constants"
    );

    let br = coher(CoherentLattice::Graphite, d.npr as usize, EMAX_EV);
    let dwpix = debye_waller_table(&d, d.constants());
    let ours = regenerate_mt2(&d, &br, &dwpix);

    eprintln!(
        "coher generated {} edges below {EMAX_EV} eV; endout retained {}, tape has {}",
        br.edges.len(),
        ours.bragg_energies_ev.len(),
        theirs.bragg_energies_ev.len()
    );
    eprintln!(
        "W'(296 K) = {:.6} /eV, W'(2000 K) = {:.6} /eV",
        dwpix[0], dwpix[9]
    );

    assert!(
        br.edges.len() > ours.bragg_energies_ev.len(),
        "NJOY's 1/E thinning must actually fire for graphite ({} generated, {} retained)",
        br.edges.len(),
        ours.bragg_energies_ev.len()
    );
    assert_eq!(
        ours.bragg_energies_ev.len(),
        theirs.bragg_energies_ev.len(),
        "retained Bragg edge count"
    );
    assert_eq!(
        ours.temperatures_k, theirs.temperatures_k,
        "tabulated temperature list"
    );

    // Bragg edge energies.
    let (emax, erms, eat, en) = deviation(&ours.bragg_energies_ev, &theirs.bragg_energies_ev);
    eprintln!(
        "Bragg energies: max {emax:.4e} at i = {eat} (E = {:.6} eV), rms {erms:.4e} over {en} edges",
        theirs.bragg_energies_ev[eat]
    );
    assert!(emax < MAX_TOL, "Bragg energy max deviation {emax:.4e}");
    assert!(erms < RMS_TOL, "Bragg energy rms deviation {erms:.4e}");

    // S(E, T) at every tabulated temperature.
    let (mut gmax, mut grms_sq, mut gn, mut gat) = (0.0f64, 0.0f64, 0usize, (0usize, 0usize));
    for (t, &temp_k) in theirs.temperatures_k.iter().enumerate() {
        let (smax, srms, sat, sn) = deviation(&ours.s_tables[t], &theirs.s_tables[t]);
        eprintln!(
            "S(E, {temp_k:>6.1} K): max {smax:.4e} at i = {sat} (E = {:.6} eV), \
             rms {srms:.4e} over {sn} points",
            theirs.bragg_energies_ev[sat]
        );
        assert!(
            smax < MAX_TOL,
            "S max deviation {smax:.4e} at {temp_k} K, edge {sat}"
        );
        assert!(srms < RMS_TOL, "S rms deviation {srms:.4e} at {temp_k} K");
        if smax > gmax {
            gmax = smax;
            gat = (t, sat);
        }
        grms_sq += srms * srms * sn as f64;
        gn += sn;
    }
    let grms = (grms_sq / gn as f64).sqrt();
    eprintln!(
        "S(E, T) over all {} temperatures: max {gmax:.4e} at {} K / E = {:.6} eV, \
         rms {grms:.4e} over {gn} values",
        theirs.temperatures_k.len(),
        theirs.temperatures_k[gat.0],
        theirs.bragg_energies_ev[gat.1]
    );
    assert!(gmax < MAX_TOL, "global S max deviation {gmax:.4e}");
    assert!(grms < RMS_TOL, "global S rms deviation {grms:.4e}");
}

/// The whole Bragg-energy residual is a uniform scale factor traceable to
/// NJOY's physical-constant vintage, **not** to the hand-transcribed lattice
/// constants.
///
/// **Why this matters.** `src/leapr/coher.rs:81` carries a `HUMAN RE-VERIFY`
/// marker on the transcribed lattice constants, because a slip in `a` or `c`
/// would silently move Bragg edges. This test distinguishes the two candidate
/// causes of the ~5e-7 residual, which a bare max/RMS figure cannot.
///
/// **Methodology.** Edge energies are `E = tau^2 / econ` where `tau^2` is pure
/// lattice geometry and `econ = ev * 8 * (amassn * amu / hbar) / hbar`
/// (`leapr.f90:2543`) is pure physical constants. The two causes have different
/// signatures:
///
/// - A wrong `a` or `c` moves `(hkl)` families by *different* relative amounts
///   (a `c`-axis error shifts `(00l)` and leaves `(hk0)` untouched), so the
///   residual would depend on the reflection.
/// - A wrong `econ` scales *every* edge by the same factor.
///
/// So: fit a single scale factor `r` by least squares on
/// `ln(E_tape) - ln(E_ours)` over all retained edges, and measure the residual
/// scatter about it. Then compute, from NJOY2016's own `src/phys.f90` as it
/// stood before commit `007828d` (2017-10-23) — the constants in force for an
/// `EVAL-SEP17` evaluation — the predicted `econ` ratio, and check the fitted
/// value against it. Finally re-scale this port's edges by that predicted ratio
/// (energies *and* structure factors, since `scon` carries the same `1/econ`)
/// and confirm the comparison collapses to storage round-off.
///
/// **Pass criterion:** the fitted scale agrees with the constants-derived
/// prediction to better than 10 %; the residual scatter about the uniform fit
/// is below 5e-7 (one 7-figure ulp at mantissa 1); and with the predicted ratio
/// applied, **every** Bragg energy and **every** `S(E, T)` at all ten
/// temperatures agrees with the tape to better than 1e-12 relative.
///
/// **Result (2026-08-13):** fitted scale **+5.115e-7** over 220 edges, predicted
/// from the SEP17-era constants **+4.926e-7** — 3.83 % apart, with the
/// discrepancy itself explained by quantisation (the fit is taken on values the
/// tape stores to 7 figures, and the per-edge scatter is 2.09e-7).
///
/// The decisive check is the substitution, and it is emphatic. With the
/// evaluation's own `econ`, the regenerated section stops merely *agreeing* with
/// the tape and becomes **identical to it**:
///
/// | | as-shipped constants | SEP17 constants |
/// |---|---|---|
/// | Bragg `E`, max rel. dev. | 9.937e-7 | **1.001e-13** |
/// | Bragg `E`, RMS | 5.525e-7 | 9.986e-14 |
/// | `S(E, T)` all ten, max | 9.986e-7 | **1.001e-13** |
///
/// 1e-13 is the float round-trip noise of parsing a 7-digit ENDF field — i.e.
/// every one of the 220 edge energies and 2,200 structure-factor values matches
/// the official tape **to the last printed digit**.
///
/// **Conclusion: the graphite lattice constants survive contact with the tape.**
/// Had `a` or `c` been mistranscribed, no single scale factor could have
/// reconciled 220 reflections at once, let alone to 13 digits. The residual in
/// the primary comparison is entirely an artefact of running a 2017
/// evaluation's deck with 2018 physical constants; it is 5e-7, three orders of
/// magnitude below any physical significance for a Bragg edge. Clearing the
/// `HUMAN RE-VERIFY` marker at `src/leapr/coher.rs:81` remains a human's call —
/// this test supplies the evidence, not the sign-off.
#[test]
fn bragg_edge_residual_is_the_econ_constant_vintage() {
    let Some(d) = load_deck() else { return };
    let Some(theirs) = load_official_mt2() else {
        return;
    };

    let br = coher(CoherentLattice::Graphite, d.npr as usize, EMAX_EV);
    let dwpix = debye_waller_table(&d, d.constants());
    let ours = regenerate_mt2(&d, &br, &dwpix);

    // Least-squares uniform scale on ln E (the mean log ratio), and the scatter
    // about it. Skip the synthetic final edge, which endout pins at emax = 5 eV
    // in both files by construction (`ulim * recon` cancels `econ` exactly) and
    // so carries no information about the constants.
    let n = ours.bragg_energies_ev.len() - 1;
    let ratios: Vec<f64> = (0..n)
        .map(|i| (theirs.bragg_energies_ev[i] / ours.bragg_energies_ev[i]).ln())
        .collect();
    let fitted = ratios.iter().sum::<f64>() / n as f64;
    let scatter = (ratios.iter().map(|r| (r - fitted).powi(2)).sum::<f64>() / n as f64).sqrt();
    let predicted = sep17_econ_ratio();
    let agreement = (fitted - predicted).abs() / predicted.abs();
    eprintln!(
        "uniform Bragg scale: fitted {fitted:+.4e} over {n} edges (scatter {scatter:.4e}), \
         predicted from SEP17-era econ {predicted:+.4e} -> {:.2} % apart",
        100.0 * agreement
    );

    assert!(
        agreement < 0.10,
        "fitted scale {fitted:+.4e} vs constants-derived {predicted:+.4e} ({:.1} % apart)",
        100.0 * agreement
    );
    assert!(
        scatter < 5.0e-7,
        "residual scatter about a uniform scale is {scatter:.4e}; a lattice-constant error \
         would show up here as reflection-dependent structure"
    );

    // The decisive check: run with the evaluation's own econ and compare
    // everything, at every temperature. The synthetic final edge is excluded
    // because scaling it is a diagnostic artefact — the tape and this port both
    // pin it at exactly emax = 5 eV whatever econ is.
    let fixed = regenerate_mt2(&d, &sep17_edges(&br), &dwpix);
    assert_eq!(
        fixed.bragg_energies_ev.len(),
        theirs.bragg_energies_ev.len(),
        "rescaling must not change the retained-edge count"
    );

    let (e0, r0, _, _) = deviation(&ours.bragg_energies_ev[..n], &theirs.bragg_energies_ev[..n]);
    let (e1, r1, _, _) = deviation(
        &fixed.bragg_energies_ev[..n],
        &theirs.bragg_energies_ev[..n],
    );
    let (mut s0, mut s1) = (0.0f64, 0.0f64);
    for t in 0..theirs.temperatures_k.len() {
        s0 = s0.max(deviation(&ours.s_tables[t][..n], &theirs.s_tables[t][..n]).0);
        s1 = s1.max(deviation(&fixed.s_tables[t][..n], &theirs.s_tables[t][..n]).0);
    }
    eprintln!(
        "with SEP17 econ applied — Bragg E: max {e0:.4e} -> {e1:.4e}, rms {r0:.4e} -> {r1:.4e}; \
         S(E, T) over all {} temperatures: max {s0:.4e} -> {s1:.4e}",
        theirs.temperatures_k.len()
    );

    // "Identical to the last printed digit": 1e-12 is far below one unit in the
    // 7th significant figure (1e-6 relative), so passing this bar means the
    // stored decimal strings agree exactly and only float round-trip noise is
    // left.
    const EXACT: f64 = 1.0e-12;
    assert!(
        e1 < EXACT,
        "with the evaluation's econ the Bragg energies should match to the last printed \
         digit, got max {e1:.4e} (was {e0:.4e})"
    );
    assert!(
        s1 < EXACT,
        "with the evaluation's econ S(E, T) should match to the last printed digit at every \
         temperature, got max {s1:.4e} (was {s0:.4e})"
    );
    assert!(r1 < r0, "econ correction must improve the RMS too");
}

/// The tape's absolute `S(E, T)` values pin the **absolute** Debye-Waller scale
/// and the **absolute** structure-factor normalisation — neither of which the
/// MT=4 parity could see.
///
/// **Why this is here.** `leapr_graphite_deck_parity.rs` measured `W'` only as a
/// *difference* between two temperatures, because its method cancelled the
/// unknown structure factors, and it said so: "an error common to every
/// temperature would survive it". MF=7/MT=2 tabulates absolute
/// `S(E, T) = sum_{j <= i} f_j exp(-4 W'(T) E_j)`, so both constants are
/// observable here.
///
/// **Methodology.** Perturb one constant at a time and re-run the full
/// ten-temperature comparison against the tape, bisecting for the largest
/// perturbation the tape still tolerates:
///
/// - **Debye-Waller scale:** multiply every `W'(T)` by `1 + delta`. This is the
///   error mode the MT=4 test is blind to — a constant common to all
///   temperatures.
/// - **Structure-factor scale:** multiply every `f_j` by `1 + epsilon`, which
///   stands in for an error in `scoh` (graphite's 5.50 b coherent cross
///   section) or in the `(4 pi)^2 / (2 a^2 c sqrt(3) econ)` prefactor `scon`.
///   `S` is linear in `f`, so the response should be one-for-one.
///
/// The baseline is the [`sep17_edges`] one, whose unperturbed deviation from
/// the tape is 1e-13 rather than 1e-6. That matters: measuring a sensitivity
/// against a baseline that already spends half the error budget would report
/// the budget, not the sensitivity. The bar is correspondingly tightened to
/// `1e-6` — one unit in the tape's 7th significant figure, the smallest
/// difference the file can express at all.
///
/// **Pass criterion:** both tolerances below 1e-5 — i.e. the tape genuinely
/// constrains the absolute scales rather than merely being consistent with
/// them — and the structure-factor response confirmed linear to 5 %.
///
/// **Result (2026-08-13):** against the exact baseline, the tape tolerates
/// `|delta| <= 4.91e-7` on the Debye-Waller scale and `|epsilon| <= 5.12e-7` on
/// the structure-factor scale before any stored digit changes. The linearity
/// check holds: `epsilon = 1e-4` produces a max `S` deviation of 1.0065e-4.
///
/// So the **absolute** `W'` — `W'(296 K) = 2.860298 /eV`,
/// `W'(2000 K) = 16.533426 /eV` — is pinned to **5 parts in ten million**. The
/// MT=4 file could only reach 0.48 % on the temperature *trend* and explicitly
/// could not constrain the overall constant at all; this is a
/// ~10,000x tightening, and it closes the gap that file named. The absolute
/// coherent normalisation is pinned to about **5 parts in ten million**, which
/// was previously unconstrained by any test in this crate — nothing had checked
/// `scoh`, `scon`, or the `formf` normalisation against a reference value.
///
/// Both are sensitivities, not discrepancies: the measured values sit at the
/// centre of those intervals, since the unperturbed comparison is exact.
#[test]
fn coherent_elastic_pins_the_absolute_debye_waller_and_structure_factor_scales() {
    let Some(d) = load_deck() else { return };
    let Some(theirs) = load_official_mt2() else {
        return;
    };

    // Measure against the exact (evaluation-constants) baseline, so the bisection
    // reports the tape's sensitivity to each scale rather than the leftover
    // constants-vintage offset.
    let br = sep17_edges(&coher(CoherentLattice::Graphite, d.npr as usize, EMAX_EV));
    let dwpix = debye_waller_table(&d, d.constants());
    /// One unit in the tape's 7th significant figure — the smallest change the
    /// reference file is capable of recording.
    const PIN_BAR: f64 = 1.0e-6;

    /// Worst relative `S(E, T)` deviation over every temperature and edge.
    fn worst(ours: &CoherentElastic, theirs: &CoherentElastic) -> f64 {
        (0..theirs.temperatures_k.len())
            .map(|t| deviation(&ours.s_tables[t], &theirs.s_tables[t]).0)
            .fold(0.0f64, f64::max)
    }

    /// Largest `|perturbation|` the tape still tolerates at `bar`, found by
    /// bisection on the magnitude. `apply` rebuilds the MT=2 section with the
    /// perturbation applied; both signs must stay inside the bar.
    ///
    /// Generic over the perturbation rather than taking a trait object, per the
    /// crate's no-`dyn` rule.
    fn tolerated<F: Fn(f64) -> CoherentElastic>(
        apply: F,
        theirs: &CoherentElastic,
        bar: f64,
    ) -> f64 {
        let (mut lo, mut hi) = (0.0f64, 1.0e-3f64);
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            let bad = worst(&apply(mid), theirs) >= bar || worst(&apply(-mid), theirs) >= bar;
            if bad {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        lo
    }

    let dw_tol = tolerated(
        |delta: f64| {
            let perturbed: Vec<f64> = dwpix.iter().map(|w| w * (1.0 + delta)).collect();
            regenerate_mt2(&d, &br, &perturbed)
        },
        &theirs,
        PIN_BAR,
    );
    let sf_tol = tolerated(
        |eps: f64| {
            let perturbed = BraggEdges {
                edges: br
                    .edges
                    .iter()
                    .map(|&(e, f)| (e, f * (1.0 + eps)))
                    .collect(),
            };
            regenerate_mt2(&d, &perturbed, &dwpix)
        },
        &theirs,
        PIN_BAR,
    );

    eprintln!(
        "absolute scales pinned by the tape: Debye-Waller |delta| <= {dw_tol:.2e}, \
         structure factor |epsilon| <= {sf_tol:.2e} (at a {PIN_BAR:.0e} bar, one unit in the \
         tape's 7th significant figure)"
    );
    eprintln!(
        "pinned values: W'(296 K) = {:.6} /eV, W'(2000 K) = {:.6} /eV",
        dwpix[0], dwpix[9]
    );

    assert!(
        dw_tol > 0.0 && dw_tol < 1.0e-5,
        "the tape must genuinely constrain the absolute W' scale, got {dw_tol:.2e}"
    );
    assert!(
        sf_tol > 0.0 && sf_tol < 1.0e-5,
        "the tape must genuinely constrain the absolute structure-factor scale, got {sf_tol:.2e}"
    );

    // S is linear in the structure factors, so a relative perturbation should
    // appear one-for-one in S. Check that at a size well above the noise.
    const EPS: f64 = 1.0e-4;
    let scaled = BraggEdges {
        edges: br
            .edges
            .iter()
            .map(|&(e, f)| (e, f * (1.0 + EPS)))
            .collect(),
    };
    let response = worst(&regenerate_mt2(&d, &scaled, &dwpix), &theirs);
    eprintln!("linearity check: epsilon = {EPS:.0e} produces a max S deviation of {response:.4e}");
    assert!(
        (response - EPS).abs() < 0.05 * EPS,
        "S should respond one-for-one to a structure-factor scale: {response:.4e} vs {EPS:.0e}"
    );
}

/// Both Boltzmann-constant vintages are measured and recorded, so the
/// as-shipped CODATA2018 figure is on the record rather than hidden.
///
/// **Why.** The MT=4 parity only closed with the `bk = 8.617385e-5 eV/K` NJOY
/// carried at `EVAL-SEP17`; CODATA2018 made it ~100x worse. `bk` reaches MT=2
/// by a different route — not through the alpha/beta grid, but through
/// `W'(T) = lambda(T) / (awr * T * k_B)`, where it enters twice (once inside
/// `tev = k_B T`, which sets the frequency model's `deltab` and hence `lambda`,
/// and once in the division). The Bragg *energies* do not depend on `bk` at all
/// — `coher` never sees a temperature — so only `S(E, T)` moves.
///
/// **Methodology.** Rebuild the `W'(T)` table under each constant set, re-emit
/// MT=2 and compare `S(E, T)` at all ten temperatures. **Pass criterion:** the
/// evaluation-era constant is the better match, and both figures are reported.
///
/// **Result (2026-08-13):** era constant (`8.617385e-5`) max **9.986e-7**;
/// CODATA2018 (`8.617333262e-5`) max **6.936e-6** — 6.9x worse, and above this
/// file's 2e-6 bar. The penalty is milder than MT=4's ~100x, because `S`
/// depends on `W'` only through `exp(-4 W' E)` rather than through a grid
/// spacing, but the direction and the conclusion are the same: reproducing a
/// published tape requires the constants it was produced with. Note that even
/// the wrong-`bk` figure stays under 1e-5 — so `bk` is *not* what drives the
/// residual in the primary comparison. That is `econ`; see
/// [`bragg_edge_residual_is_the_econ_constant_vintage`].
#[test]
fn both_boltzmann_vintages_are_measured_for_the_elastic_channel() {
    let Some(d) = load_deck() else { return };
    let Some(theirs) = load_official_mt2() else {
        return;
    };

    let br = coher(CoherentLattice::Graphite, d.npr as usize, EMAX_EV);

    let run = |pc: PhysicalConstants| -> f64 {
        let dwpix = debye_waller_table(&d, pc);
        let ours = regenerate_mt2(&d, &br, &dwpix);
        (0..theirs.temperatures_k.len())
            .map(|t| deviation(&ours.s_tables[t], &theirs.s_tables[t]).0)
            .fold(0.0f64, f64::max)
    };

    let era = run(PhysicalConstants::Njoy2016Legacy);
    let modern = run(PhysicalConstants::Codata2018);
    eprintln!(
        "S(E, T) max deviation — bk = {:e} (EVAL-SEP17 era): {era:.4e}; \
         bk = {:e} (CODATA2018): {modern:.4e} ({:.1}x worse)",
        PhysicalConstants::Njoy2016Legacy.bk_ev_per_k(),
        PhysicalConstants::Codata2018.bk_ev_per_k(),
        modern / era
    );

    assert!(era < MAX_TOL, "era-constant max deviation {era:.4e}");
    assert!(
        modern > era,
        "the era constant must be the better match ({era:.4e} vs {modern:.4e})"
    );
    // The as-shipped constant, recorded at its own honest level.
    assert!(modern < 1.0e-4, "CODATA2018 max deviation {modern:.4e}");
}

/// The data-absent path skips cleanly instead of failing.
///
/// **Methodology.** Resolve both data files against a directory that does not
/// exist and confirm the resolver returns `None`, having printed a skip note,
/// rather than panicking — the behaviour every test in this file relies on to
/// stay green on a machine without the 8.7 MB tape. The directory is passed as
/// an argument rather than through `GRAPHITE_TSL_DIR`, because mutating the
/// process environment would race the other tests in this binary.
/// **Pass criterion:** both resolve to `None`, no panic.
///
/// **Result (2026-08-13):** both return `None` and print the skip note naming
/// the missing file and the directory searched.
#[test]
fn missing_data_skips_cleanly() {
    const ABSENT: &str = "/nonexistent/graphite/tsl/dir";
    assert!(
        data_path_in(ABSENT, DECK_FILE).is_none(),
        "absent deck must skip, not panic"
    );
    assert!(
        data_path_in(ABSENT, TAPE_FILE).is_none(),
        "absent tape must skip, not panic"
    );
}
