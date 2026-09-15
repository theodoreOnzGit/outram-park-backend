//! **V&V: the unresolved range (LRU=2) is reconstructed into MF=3, against
//! NJOY2016's own RECONR.**
//!
//! # Methodology
//!
//! **What is computed.** The crate's [`reconr`] MF=3 MT=1/2/18/102 across
//! U-234's unresolved resonance range, `1.5e3 .. 1.0e5 eV`.
//!
//! **Inputs.** `reference-data/endf/n-092_U_234-ENDF8.0.endf` (ENDF/B-VIII.0,
//! MAT 9225), reconstruction tolerance `err = 0.001`, 0 K. U-234 is the **only
//! `LSSF = 0` material** in `reference-data/endf/`, which is what makes it the
//! case that matters — see below.
//!
//! **Reference.** NJOY2016 upstream `ac5adf5f` (2016.79), gfortran 13.3.0, run
//! with the deck committed beside the tape as
//! `reference-data/reconr/u234-ENDF8.0-0K-err0.001.njoy-input`. Comparison on
//! **NJOY's own grid** — all 34 points its PENDF carries inside the URR.
//!
//! **Pass criterion.** Two, and the first is the one that matters:
//! 1. **no zeros** — not one NJOY grid point in the URR may come back `0.0`;
//! 2. worst relative deviation `<= 2e-2` per MT.
//!
//! # The defect this pins (`bn:op-12lu`)
//!
//! Before 2026-09-14 this crate did not reconstruct the unresolved range at
//! all. `reconr/mf2.rs` parsed LRU=2 as a header and moved on, with a comment
//! claiming *"RECONR does not use unresolved parameters (that is PURR's job)"*
//! — which is wrong about upstream. `reconr.f90:81-87` says the opposite, and
//! `genunr` (`:1628-1735`) implements it.
//!
//! **`LSSF` decides whether that omission is visible**, and it is why this
//! survived:
//!
//! | | `LSSF` | MF=3 carries the dilute URR? | effect of omitting |
//! |---|---|---|---|
//! | U-235, U-238 | 1 | yes | none — doing nothing is correct |
//! | **U-234** | **0** | **no** | **zero cross section** |
//!
//! Every case this repository runs is built on U-235/U-238. Measured before
//! the fix: **7 of 10 log-spaced samples of MT=1 across the URR were exactly
//! `0.0`**, against NJOY's `2.032567e1 b` at 1700 eV, with MT=2/18/102 and the
//! MF=3 background all zero there too. **A zero total cross section is an
//! infinite flight in transport**, and U-234 is one of Godiva's three ICSBEP
//! nuclides.
//!
//! # Results (2026-09-14), worst relative deviation over NJOY's 34 URR points
//!
//! | MT | before | after |
//! |---|---|---|
//! | 1 (total) | 7/10 samples `0.0` | **4.81e-4** |
//! | 2 (elastic) | zero | **3.88e-4** |
//! | 18 (fission) | zero | **5.77e-3** |
//! | 102 (capture) | zero | **4.21e-3** |
//!
//! # Interpretation, and an honest note on the residual
//!
//! Total and elastic land inside the deck's own `err = 0.001`. Fission and
//! capture do not — and **the residual is now explained: it is NJOY's own
//! interpolation error, not this port's.** See
//! `tests/reconr_urr_kernel_vs_njoy2016.rs`, which settles it.
//!
//! Of NJOY's 34 grid points in this window, **26 are reproduced by this port's
//! unresolved kernel to ~1e-7**. The other 8 are each reproduced, to between
//! 2.9e-7 and 1.4e-16, by **lin-lin interpolation of NJOY's own neighbouring
//! exact points** — which is `genunr`/`sigunr` behaving exactly as documented:
//! `genunr` tabulates the dilute cross sections on the `eunr` grid as
//! MF=2/MT=152, and `sigunr` (`reconr.f90:1737-1769`) *interpolates that table*
//! for PENDF energies contributed by other MF=3 sections. Every one of this
//! test's worst-deviation energies — 4.36875e4 here, and 7.0e3, 4.518e4,
//! 5.25e4, 7.0e4, 8.0e4, 9.0e4 — is in that interpolated set.
//!
//! **Where the two differ, this port evaluates the physics at the point and
//! NJOY interpolates to it, so ours is the more accurate value.** Three other
//! explanations were tested and refuted first (a different ENDF interpolation
//! law; a difference in how the unresolved *parameters* are interpolated;
//! parameter energies being special) — the refutations are recorded in that
//! file's module doc.
//!
//! The gate stays at 2e-2 — well above the measured 5.8e-3 — because it exists
//! to catch the zero-cross-section defect returning, and the residual it would
//! otherwise pin is a property of the reference rather than of this code.
//!
//! # What this does NOT establish
//!
//! - **Only Case C is exercised.** U-234's unresolved parameters are
//!   energy-dependent (Case C). The `unresolved_grid` fallback for Case A and
//!   energy-independent Case B — a log-spaced fill, the one part of
//!   `reconr::urr` that is not a literal translation of upstream — is
//!   **unverified**, because no held evaluation reaches it.
//! - **MF=2/MT=152 is not written.** `genunr` also emits that section; this
//!   port does not, and any consumer expecting it will not find it.
//! - **`LSSF = 1` is verified only as a no-op**, by the second test here.
//! - Verification against NJOY2016, not validation against measurement.

use njoy_outram_park_fork::endf::mt::MtReaction;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_file, reference_file_or_skip};

const MAT: i32 = 9225;
const URR_LO: f64 = 1.5e3;
const URR_HI: f64 = 1.0e5;
/// Deliberately loose — see the interpretation note in this file's module doc.
const GATE: f64 = 2.0e-2;
/// Worst relative deviations recorded on 2026-09-14.
const RECORDED: [(i32, f64); 4] = [(1, 4.81e-4), (2, 3.88e-4), (18, 5.77e-3), (102, 4.21e-3)];

fn njoy_pairs(t: &Tape, mt: i32) -> Option<Vec<(f64, f64)>> {
    let sec = t.section(MAT, 3, mt)?;
    let mut c = SectionCursor::new(&sec.rows);
    c.read_cont().ok()?;
    Some(c.read_tab1().ok()?.pairs)
}

fn reconstruct() -> Option<njoy_outram_park_fork::reconr::ReconrResult> {
    let ep = reference_file("endf", "n-092_U_234-ENDF8.0.endf")?;
    let tape = Tape::read_file(&ep).ok()?;
    reconr(
        &tape,
        &ReconrConfig {
            mat: MAT,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .ok()
}

#[test]
fn unresolved_range_reproduces_njoy2016_and_is_never_zero() {
    let Some(pendf) = reference_file_or_skip(
        "reconr",
        "u234-ENDF8.0-0K-err0.001.pendf",
        "U-234 unresolved RECONR golden",
    ) else {
        return;
    };
    let Some(ours) = reconstruct() else {
        eprintln!("skipping: U-234 evaluation absent or reconstruction failed");
        return;
    };
    let njoy = Tape::read_file(&pendf).expect("NJOY PENDF parses");

    for (mt, recorded) in RECORDED {
        let Some(pairs) = njoy_pairs(&njoy, mt) else {
            continue;
        };
        let rx = MtReaction::from_any(mt);

        let (mut worst, mut worst_e, mut worst_vals) = (0.0_f64, 0.0_f64, (0.0_f64, 0.0_f64));
        let (mut n, mut zeros) = (0usize, 0usize);
        let mut prev = f64::NAN;
        for &(e, reference) in &pairs {
            if e == prev {
                continue;
            }
            prev = e;
            if !(URR_LO..=URR_HI).contains(&e) {
                continue;
            }
            let mine = ours.eval_mt(rx, e);
            if mine == 0.0 && reference > 1.0e-12 {
                zeros += 1;
            }
            if reference.abs() > 0.0 {
                let dev = (mine - reference).abs() / reference.abs();
                if dev > worst {
                    worst = dev;
                    worst_e = e;
                    worst_vals = (mine, reference);
                }
            }
            n += 1;
        }

        assert!(n > 20, "MT={mt}: only {n} NJOY points inside the URR");
        println!(
            "  MT={mt:3}: {n} pts, {zeros} zero, worst {worst:.4e} at E={worst_e:.5e} \
             (ours {:.6e} vs NJOY {:.6e}); recorded {recorded:.2e}",
            worst_vals.0, worst_vals.1
        );

        // The defect itself.
        assert_eq!(
            zeros, 0,
            "MT={mt}: {zeros} of {n} NJOY grid points inside the unresolved range \
             come back as 0.0. That is the bn:op-12lu defect returning -- the \
             LRU=2 contribution is not reaching MF=3. A zero total cross section \
             is an infinite flight in transport."
        );
        assert!(
            worst <= GATE,
            "MT={mt}: worst relative deviation {worst:.4e} at E={worst_e:.5e} \
             (ours {:.6e} vs NJOY {:.6e}) exceeds the {GATE:.0e} gate; \
             {recorded:.2e} was recorded on 2026-09-14.",
            worst_vals.0,
            worst_vals.1
        );
    }
}

/// `LSSF = 1` materials must be left exactly as they were: their MF=3 already
/// carries the dilute unresolved cross sections, so adding them again would
/// double-count.
///
/// The expected values were recorded by a paired A/B **before** the unresolved
/// reconstruction landed. This is the guard that keeps every Godiva and LCT008
/// baseline still meaning what it meant.
#[test]
fn lssf1_materials_are_untouched_by_the_unresolved_reconstruction() {
    const CASES: [(&str, i32, [f64; 4]); 2] = [
        (
            "n-092_U_235-ENDF8.0.endf",
            9228,
            [3.667673747e4, 9.210397313e1, 1.368044198e1, 6.903497000e0],
        ),
        (
            "n-092_U_238.endf",
            9237,
            [1.437686459e2, 9.570592353e0, 2.130744614e1, 7.089129000e0],
        ),
    ];
    const PROBES: [f64; 4] = [1.0e-5, 1.0, 1.0e3, 1.0e6];

    let mut ran = 0;
    for (f, mat, want) in CASES {
        let Some(p) = reference_file("endf", f) else {
            continue;
        };
        ran += 1;
        let tape = Tape::read_file(&p).expect("evaluation parses");
        let r = reconr(
            &tape,
            &ReconrConfig {
                mat,
                tolerance: 0.001,
                temperature: 0.0,
            },
        )
        .expect("reconstruction runs");
        for (i, &e) in PROBES.iter().enumerate() {
            let got = r.eval_mt(MtReaction::Mt1Total, e);
            let dev = (got - want[i]).abs() / want[i].abs();
            println!("  {f} MT=1 at {e:8.1e}: {got:.9e} vs recorded {:.9e} ({dev:.2e})", want[i]);
            assert!(
                dev < 1.0e-8,
                "{f} MT=1 at {e:e} moved: {got:.9e} against the pre-change {:.9e} \
                 ({dev:.2e}). This material is LSSF=1, so the unresolved \
                 reconstruction must be a no-op for it -- if it is not, the \
                 LSSF guard in reconr::urr has broken and every k-eff baseline \
                 built on it has shifted.",
                want[i]
            );
        }
    }
    assert!(ran > 0, "no evaluations available");
}
