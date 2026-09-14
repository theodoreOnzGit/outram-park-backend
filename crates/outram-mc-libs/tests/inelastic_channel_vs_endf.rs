//! **The inelastic channel: its cross section against the ENDF tape, and its
//! secondary energy against two-body energy balance.**
//!
//! # Why this exists
//!
//! Pricing mechanisms against the +4000 pcm FHR ring-RPT residual turned up one
//! that is the size of the residual. Dropping the inelastic channel on that
//! pebble is worth **−4190 ± 335 pcm**, and dropping it for **F-19 alone** is
//! worth **−4033 ± 333 pcm** — after which the pebble sits 34 pcm from its
//! OpenMC reference and all four six-factors land on it. Nothing else priced so
//! far is within an order of magnitude.
//!
//! That is a localisation, not an explanation, and it makes the inelastic
//! channel worth reading closely rather than worth believing about. This file is
//! what came of reading it: one real defect (fixed, GitHub #192), and two checks
//! that say the rest of the channel is faithful to its data.
//!
//! # What is checked, and against what
//!
//! 1. **The partials sum to the tape's own MT=4.** ENDF makes MT=4 a *redundant*
//!    section — the evaluator's own sum of MT=51…91 — so `Σ eval_mt(MT=51…91)`
//!    must equal `eval_mt(MT=4)` exactly. This is the failure mode this crate
//!    has already hit once in `absorption_mt27`: the RECONR path linearises every
//!    MF=3 section as it appears and does not mark the redundant aggregates the
//!    way an ACE reader does, so a partial summed on top of a sum double counts.
//!
//! 2. **The reconstruction reproduces the raw tape.** Claim 1 is internal: both
//!    sides come from one `ReconrResult`, so it cannot catch a reconstruction
//!    that is wrong in the same way twice. The golden values below were read by
//!    **parsing the MF=3 TAB1 records out of the ENDF file directly** — no
//!    RECONR, no linearisation, just the evaluator's own tabulated pairs with
//!    lin-lin interpolation between the two bracketing points.
//!
//! 3. **The continuum channel conserves energy.** A pure physics invariant with
//!    no data in it: the lab outgoing energy of a two-body-bounded collision
//!    cannot exceed `(√E'_cm + √(E/(A+1)²))²` with
//!    `E'_cm = E·(A/(A+1))² + Q·A/(A+1)`. Before GitHub #192 the continuum
//!    sampler capped at the *elastic* CM energy — the `Q = 0` bound — and
//!    violated this in **22 % of draws** for U-238 and U-235 at 1 MeV, just
//!    above the MT=91 threshold.
//!
//! Data-gated: skips (passes) when `reference-data/endf/` is absent.

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::{Inelastic, Nuclide};
use outram_mc_libs::physics::scatter::continuum_inelastic_scatter;

const TEMP_K: f64 = 600.0;

fn nuclide_or_skip(file: &str, name: &str) -> Option<Nuclide> {
    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf(file) else {
        println!("[{name}] SKIP: {file} not in reference-data/endf/");
        return None;
    };
    match Nuclide::from_endf_file(&path, name, TEMP_K, 1.0e-3) {
        Ok(n) => Some(n),
        Err(e) => {
            println!("[{name}] SKIP: {e:?}");
            None
        }
    }
}

/// Nuclides whose inelastic channel the FHR pebble actually uses, with F-19
/// first because it is the one that carries the pebble's inelastic moderation.
const CASES: [(&str, &str); 4] = [
    ("n-009_F_019-ENDF8.0.endf", "F19"),
    ("n-092_U_238.endf", "U238"),
    ("n-092_U_235-ENDF8.0.endf", "U235"),
    ("n-003_Li_007-ENDF8.0.endf", "Li7"),
];

/// **`Σ MT=51…91` equals the tape's own redundant `MT=4`, so no partial is being
/// summed on top of a sum.**
///
/// # Measured (2026-09-12), ENDF/B-VIII.0
///
/// Worst relative deviation over 4 nuclides × 8 energies is **1.19e-5**, at F-19
/// / 14 MeV; every other point is at or under 2.3e-7. The outlier is the grid
/// mismatch between MT=4's linearisation and the partials', not a data
/// disagreement.
///
/// This test is how GitHub #193 surfaced. Before that fix it failed on F-19 at
/// 200 keV — `Σ = 0.847319 b` against `MT=4 = 0.843051 b`, **+0.51 %** — and the
/// excess was a *constant* 0.004268 b at every energy below 207.46 keV, which is
/// MT=52's first tabulated cross section propagated downward by an endpoint
/// clamp. What looked like a redundant-section double count was a threshold
/// reaction with a cross section below its threshold.
#[test]
fn the_inelastic_partials_sum_to_the_tapes_own_mt4() {
    /// Ignore points where the channel is shut in both quantities.
    const FLOOR_BARN: f64 = 1.0e-12;
    /// Allowed relative deviation above the floor. MT=4 and the MT=51…91
    /// partials are linearised on **different grids** by RECONR, so a lin-lin
    /// evaluation of the two at one energy differs by the grid mismatch even when
    /// the underlying data is identical. Measured worst 1.19e-5; this is ~4x it.
    const REL_TOL: f64 = 5.0e-5;

    let energies = [1.0e5, 2.0e5, 3.0e5, 5.0e5, 1.0e6, 2.0e6, 5.0e6, 1.4e7];
    let mut worst = 0.0_f64;
    let mut worst_at = String::new();
    for (file, name) in CASES {
        let Some(n) = nuclide_or_skip(file, name) else {
            continue;
        };
        for e in energies {
            let summed = n.xs_at_energy(e, TEMP_K).inelastic;
            let (mt4, _levels) = n.inelastic_mt4_and_levels(e);
            if mt4 < FLOOR_BARN {
                println!("[{name}] E={e:>9.3e} below floor: sum {summed:.5} b, MT4 {mt4:.5} b");
                continue;
            }
            let d = ((summed - mt4) / mt4).abs();
            println!(
                "[{name}] E={e:>9.3e}  sum {summed:>9.5} b   MT4 {mt4:>9.5} b   rel {:+.3e}",
                (summed - mt4) / mt4
            );
            if d > worst {
                worst = d;
                worst_at = format!("{name} at {e:.3e} eV ({summed:.6} vs {mt4:.6})");
            }
        }
    }
    assert!(
        worst < REL_TOL,
        "the inelastic partials do not sum to MT=4: worst relative deviation \
         {worst:.3e} at {worst_at} -- a redundant MF=3 section is being summed \
         on top of the aggregate that already contains it"
    );
}

/// **The reconstruction reproduces the raw ENDF tape's MF=3 inelastic, so F-19's
/// large low-energy inelastic cross section is the evaluation's and not this
/// code's.**
///
/// # Why F-19 specifically
///
/// Because it looked wrong. F-19's total inelastic is **2.82 b at 300 keV**,
/// a third of its 8.53 b total, when the only channels open there are levels at
/// 109.9 and 197 keV. That is the single number the ring-RPT residual is most
/// sensitive to, so "is it real?" had to be answered against something other
/// than the code that produced it.
///
/// # Methodology
///
/// The golden values were obtained on 2026-09-12 by parsing the MF=3 TAB1
/// records out of `reference-data/endf/n-009_F_019-ENDF8.0.endf` directly —
/// locating the section by the MAT/MF/MT columns, skipping the HEAD, CONT and
/// interpolation records, and interpolating lin-lin between the two bracketing
/// tabulated pairs. No RECONR, no linearisation, no broadening: the evaluator's
/// own numbers.
///
/// The comparison is not exact by construction — `eval_mt` returns the
/// Doppler-broadened, RECONR-linearised value on a different grid — so the gate
/// is 0.5 %, and the measured worst is far inside it.
///
/// # Measured (2026-09-12)
///
/// ```text
///   E [eV]     MT=4 tape    MT=4 recon   MT=51 tape   MT=1 tape   MT=2 tape
///   2.000e5     0.843051      0.84305      0.843051     4.14486     3.30173
///   3.000e5     2.820252      2.82025      2.529543     8.53036     5.70992
///   5.000e5     1.253938      1.25394      0.488221     6.33367     5.07926
///   1.000e6     0.356546      0.35655      0.097671     2.83528     2.47926
///   2.000e6     0.974700      0.97470      0.162000     3.47587     2.50083
/// ```
///
/// **The evaluation really does put 2.82 b of inelastic into F-19 at 300 keV**,
/// and the tape's own `MT=1 − MT=2` reproduces `MT=4` to five figures at every
/// row, so it is not an artefact of how the partials were assembled either.
#[test]
fn f19_inelastic_reproduces_the_raw_endf_tape() {
    /// `(E [eV], MT=4 [barn])` read straight out of the ENDF/B-VIII.0 F-19 tape.
    const F19_MT4_TAPE: [(f64, f64); 5] = [
        (2.0e5, 0.843_051_05),
        (3.0e5, 2.820_252_49),
        (5.0e5, 1.253_938_09),
        (1.0e6, 0.356_546_01),
        (2.0e6, 0.974_700_00),
    ];
    /// The reconstruction is broadened and re-gridded, so it is not expected to
    /// match the tape's lin-lin interpolant exactly.
    const REL_TOL: f64 = 5.0e-3;

    let Some(n) = nuclide_or_skip("n-009_F_019-ENDF8.0.endf", "F19") else {
        return;
    };
    let mut worst = 0.0_f64;
    for (e, tape) in F19_MT4_TAPE {
        let (recon, _) = n.inelastic_mt4_and_levels(e);
        let d = (recon - tape) / tape;
        println!("[F19] E={e:>9.3e}  tape {tape:>9.6} b   recon {recon:>9.6} b   rel {d:+.3e}");
        worst = worst.max(d.abs());
    }
    assert!(
        worst < REL_TOL,
        "F-19's reconstructed MT=4 does not reproduce the raw tape: worst \
         relative deviation {worst:.3e}, tolerance {REL_TOL}"
    );
}

/// **A continuum-inelastic collision never leaves the neutron with more energy
/// than two-body balance allows for the channel's own Q — GitHub #192.**
///
/// # The invariant
///
/// For a channel of Q-value `Q` on a target of mass ratio `A`, the outgoing
/// neutron's CM energy is at most `E'_cm = E·(A/(A+1))² + Q·A/(A+1)` (it is
/// exactly that for a discrete level), so the lab energy is at most
/// `(√E'_cm + √(E/(A+1)²))²`. There is no data in this: it is energy and
/// momentum conservation for a two-body exit channel.
///
/// # What it caught
///
/// `continuum_inelastic_scatter` capped the sampled CM energy at the **elastic**
/// CM energy — the `Q = 0` bound — because `Inelastic::Continuum` was a unit
/// variant and `sample_inelastic` discarded MT=91's `QI` before the sampler saw
/// it. Measured before the fix, 50 000 collisions per point:
///
/// ```text
///   nuclide   MT=91 QI       1 MeV   2 MeV    5 MeV    14 MeV
///   U-238     -0.4338 MeV     22 %    2.5 %    0.04 %    0 %
///   U-235     -0.4337 MeV     22 %    2.4 %    0.04 %    0 %
///   F-19      -5.640  MeV      --      --       --      21 %
/// ```
///
/// Worst exactly where the channel opens, which is where it matters most: at
/// 1 MeV the actinide cap is `E'/E = 0.566` and the sampler was drawing to 0.992.
///
/// # Measured after the fix (2026-09-12)
///
/// **Zero violations** at every point, and the sampled mean moves toward *more*
/// moderation (U-238 at 2 MeV, `⟨E'/E⟩` 0.2945 → 0.2787) — so this is a
/// correctness fix and not a reactivity one.
#[test]
fn continuum_inelastic_respects_two_body_energy_balance() {
    /// Samples per (nuclide, energy) point.
    const N: usize = 50_000;
    /// Slack on the bound, for the floating-point comparison only.
    const REL_SLACK: f64 = 1.0e-9;

    let z = Direction {
        u: 0.0,
        v: 0.0,
        w: 1.0,
    };
    let energies = [1.0e6, 2.0e6, 5.0e6, 1.4e7];
    for (file, name) in CASES {
        let Some(n) = nuclide_or_skip(file, name) else {
            continue;
        };
        for e in energies {
            // Only sample the channel where it is actually open; the dispatch in
            // transport does the same, in proportion to `MicroXS::inelastic`.
            if n.xs_at_energy(e, TEMP_K).inelastic <= 0.0 {
                continue;
            }
            let mut seed = 0x1_1E1_u64.wrapping_add(e as u64);
            let (mut drawn, mut over) = (0usize, 0usize);
            for _ in 0..N {
                let Inelastic::Continuum { q } = n.sample_inelastic(e, &mut seed) else {
                    continue;
                };
                let (ep, _) = continuum_inelastic_scatter(e, z, n.awr, q, &mut seed);
                drawn += 1;
                let ap1 = n.awr + 1.0;
                let e_cm = (e * (n.awr / ap1).powi(2) + q * n.awr / ap1).max(0.0);
                let e_trans = e / (ap1 * ap1);
                let bound = (e_cm.sqrt() + e_trans.sqrt()).powi(2);
                if ep > bound * (1.0 + REL_SLACK) {
                    over += 1;
                }
            }
            if drawn == 0 {
                continue;
            }
            println!(
                "[{name}] E={e:>9.3e}  continuum draws {drawn:>6}  over-bound {over:>6} \
                 ({:.3} %)",
                100.0 * over as f64 / drawn as f64
            );
            assert!(
                over == 0,
                "[{name}] {over}/{drawn} continuum-inelastic draws at {e:.3e} eV exceed \
                 the two-body energy bound -- the channel is emitting energy the \
                 reaction cannot have left the neutron (GitHub #192)"
            );
        }
    }
}

/// **A threshold reaction has no cross section below its threshold — GitHub
/// #193, and the +4000 pcm FHR ring-RPT residual.**
///
/// # The invariant
///
/// An MF=3 section that opens above the evaluation's lower bound is a threshold
/// reaction. Below its first tabulated abscissa the reaction cannot occur, so its
/// cross section is zero. No data is needed to know this.
///
/// # What it caught
///
/// `ReconrResult::eval_mt` evaluated the section with `eval_lin_lin`, which
/// clamps to the endpoint value outside the grid — correct at the top, correct
/// for a section spanning the whole evaluation, and wrong at the bottom of a
/// threshold section. Evaluations do not all open their threshold sections at
/// zero: ENDF/B-VIII.0 F-19 starts MT=51 at `(115 840 eV, 0.018129 b)` and MT=52
/// at `(207 460 eV, 0.0042683 b)`. So F-19 carried a constant **0.0224 b of
/// inelastic scattering at every energy below 115 keV**, into the thermal range —
/// about 0.6 % of its total cross section through the whole resonance region.
///
/// The kinematics turned a 0.6 % cross-section error into a transport
/// catastrophe: `two_body_scatter` clamps a negative outgoing CM energy to zero,
/// so a sub-threshold "inelastic" collision leaves the neutron at `E/(A+1)²` — a
/// factor of 394 for fluorine, **six units of lethargy in one collision**. Very
/// roughly one F-19 collision in 170, at every energy below 115 keV, teleported
/// the neutron past the U-238 resonance region.
///
/// # Measured (2026-09-12) on the FHR ring-RPT pebble
///
/// ```text
///                     k_eff (CSG)         vs OpenMC    eta      f       p        eps
///   before            1.40546 +/- 0.00234  +4067 pcm  +0.11 % -0.17 % +8.54 %  -4.99 %
///   after             1.36140 +/- 0.00229   -339 pcm  -0.00 % -0.01 % +0.04 %  -0.08 %
///   OpenMC reference  1.36479 +/- 0.00067      —      2.0073  0.9216  0.4842   1.5043
/// ```
#[test]
fn a_threshold_reaction_has_no_cross_section_below_its_threshold() {
    /// Fractions of the nuclide's own lowest threshold to test at. Taking them
    /// from the level table rather than hard-coding energies matters: U-235's
    /// first excited state is at **76.5 eV**, four orders of magnitude below
    /// U-238's 44.9 keV, so a fixed epithermal probe energy would be *above*
    /// U-235's threshold and would assert something false.
    const FRACTIONS: [f64; 5] = [1.0e-6, 1.0e-4, 1.0e-2, 0.5, 0.99];

    let mut any = false;
    for (file, name) in CASES {
        let Some(n) = nuclide_or_skip(file, name) else {
            continue;
        };
        any = true;
        // Lowest threshold over the nuclide's own channels: E_th = |Q|·(A+1)/A.
        let e_th = n
            .inelastic_levels_table()
            .iter()
            .map(|&(_, q, _)| q.abs() * (n.awr + 1.0) / n.awr)
            .fold(f64::INFINITY, f64::min);
        assert!(
            e_th.is_finite() && e_th > 0.0,
            "[{name}] has no inelastic channels to take a threshold from"
        );
        println!("[{name}] lowest inelastic threshold {e_th:.4e} eV");
        for e in FRACTIONS.map(|f| f * e_th) {
            let x = n.xs_at_energy(e, TEMP_K);
            let (mt4, _) = n.inelastic_mt4_and_levels(e);
            println!(
                "[{name}] E={e:>9.3e}  inelastic {:.6e} b   MT4 {mt4:.6e} b",
                x.inelastic
            );
            assert!(
                x.inelastic == 0.0,
                "[{name}] has {:.6e} b of inelastic scattering at {e:.3e} eV, below \
                 every one of its thresholds. A threshold reaction cannot occur \
                 below its threshold (GitHub #193)",
                x.inelastic
            );
            assert!(
                mt4 == 0.0,
                "[{name}] MT=4 is {mt4:.6e} b at {e:.3e} eV, below its own threshold"
            );
            assert!(
                x.n2n == 0.0,
                "[{name}] has {:.6e} b of (n,2n) at {e:.3e} eV, below its threshold",
                x.n2n
            );
        }
    }
    if !any {
        println!("SKIP: no reference tapes");
    }
}
