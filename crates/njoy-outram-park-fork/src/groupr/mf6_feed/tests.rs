// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Unit tests for [`super::Mf6Feed`]. Split out of `mf6_feed.rs` to respect
//! this crate's 1000-line file cap (see its `CLAUDE.md`).

use super::*;
use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;

const MAT: i32 = 125;
const MT: i32 = 16;

/// Build a one-section MF=6 tape from raw six-field rows.
fn tape(rows: Vec<[f64; 6]>) -> Tape {
    Tape::from_sections(
        String::new(),
        vec![Section {
            key: EndfKey {
                mat: MAT,
                mf: 6,
                mt: MT,
            },
            rows,
        }],
    )
}

/// A constant-yield TAB1 over `[1e-5, 2e6]` eV: `ZAP, AWP, LIP, LAW` head,
/// one lin-lin region, two points.
fn yield_rows(zap: f64, awp: f64, law: f64, y: f64) -> Vec<[f64; 6]> {
    vec![
        [zap, awp, 0.0, law, 1.0, 2.0],
        [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],
        [1.0e-5, y, 2.0e6, y, 0.0, 0.0],
    ]
}

/// One LAW=1 LIST record: `NA = 0`, `ND = 0`, two `(E', f0)` points.
///
/// `ep_lo` is a parameter so both cases are exercised: a leading `E' = 0`
/// point (the shape `cm2lab`/`ll2lab` always produce) and a grid starting just
/// above zero. Both now give the same answer — see
/// `leading_zero_energy_point_contributes_its_panel`.
fn list_rows(e_in: f64, ep_lo: f64, ep_top: f64, f0: f64) -> Vec<[f64; 6]> {
    vec![
        [0.0, e_in, 0.0, 0.0, 4.0, 2.0],
        [ep_lo, f0, ep_top, f0, 0.0, 0.0],
    ]
}

/// A complete MF=6 section: HEAD + one `ZAP = 1` LAW=1 subsection with two
/// incident energies, each a flat normalised spectrum out to `ep_top`.
fn law1_tape(lct: f64, ep_lo: f64, ep_top: f64) -> Tape {
    let f0 = 1.0 / ep_top;
    let mut rows = vec![[1001.0, 10.0, 0.0, lct, 1.0, 0.0]];
    rows.extend(yield_rows(1.0, 1.0, 1.0, 2.0));
    // TAB2: LANG = 1, LEP = 2, NE = 2, one lin-lin region.
    rows.push([0.0, 0.0, 1.0, 2.0, 1.0, 2.0]);
    rows.push([2.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
    rows.extend(list_rows(1.0e6, ep_lo, ep_top, f0));
    rows.extend(list_rows(2.0e6, ep_lo, ep_top, f0));
    tape(rows)
}

fn cfg(nl: usize) -> Mf6FeedConfig {
    Mf6FeedConfig {
        awr: 10.0,
        awrp: 1.0,
        izap: 1,
        q: -1.0e7,
        ismooth: false,
        nl,
    }
}

/// **Methodology.** A lab-frame (`LCT = 1`) LAW=1 subsection whose
/// secondary spectrum is flat and normalised (`f0 = 1/2e6` over
/// `E' = 1e-5 .. 2e6` eV) with a constant yield `y = 2`, over four
/// equal-width secondary groups spanning the same range.
///
/// Hand-derived expectation from `getmf6`'s own trapezoid march
/// (`:8043-8049`): `f6lab` returns `term = 0` once its pointer passes the
/// table's last point (`groupr.f90:9161-9163, 9216`), so the panel that
/// *ends* on the top tabulated `E'` is a half-triangle. The raw deposits
/// are therefore `pe * w * f0 = 0.5` in the first three groups and `0.25`
/// in the last, i.e. `[0.5, 0.5, 0.5, 0.25]`, summing to `1.75`. The
/// label-700 renormalisation (`:8128-8132`) then scales by
/// `yld/test = 2/1.75 = 8/7`, giving `[4/7, 4/7, 4/7, 2/7]`.
///
/// The incident energy is the top table energy so `jgmax` lands on the
/// last group and the upscatter fold-back (`:8115-8120`) is a no-op —
/// that path is exercised separately by
/// `illegal_upscatter_is_folded_into_the_ingroup`.
///
/// **Result (2026-09-13).** `[4/7, 4/7, 4/7, 2/7]` to < 1e-9, column sum
/// `= yld = 2.0` to < 1e-12.
#[test]
fn flat_lab_spectrum_deposits_equally_and_sums_to_the_yield() {
    let t = law1_tape(1.0, 1.0e-5, 2.0e6);
    let mut feed = Mf6Feed::new(&t, MAT, 6, MT, cfg(1)).unwrap();
    assert_eq!(feed.lct(), 1);
    assert_eq!(feed.jzap(), 1);
    assert_eq!(feed.nss(), 1);
    // `enext = elo` after initialization (`:7953`).
    assert!(
        (feed.enext_init() - 1.0e6).abs() < 1e-6,
        "{}",
        feed.enext_init()
    );

    let eg = [1.0e-5, 5.0e5, 1.0e6, 1.5e6, 2.0e6];
    let at = feed.feed(2.0e6, &eg, 1).unwrap();
    assert_eq!(at.ng2, 4);
    assert_eq!(at.iglo, 1);
    assert_eq!(at.nq, 0);
    assert!((at.yld - 2.0).abs() < 1e-12, "{at:?}");
    let sum: f64 = at.ans[0].iter().sum();
    assert!((sum - at.yld).abs() < 1e-12, "sum {sum} vs {}", at.yld);
    let want = [4.0 / 7.0, 4.0 / 7.0, 4.0 / 7.0, 2.0 / 7.0];
    for (j, (v, w)) in at.ans[0].iter().zip(want).enumerate() {
        assert!((v - w).abs() < 1e-9, "group {j}: {v} want {w}");
    }
}

/// **Methodology.** The same lab-frame subsection carries `NA = 0`, i.e.
/// the secondary emission is isotropic in the lab, so every Legendre
/// moment above `P0` must be identically zero — `f6lab` has no coefficient
/// to interpolate for `l >= 2` (`groupr.f90:9251-9260`). Ask for `nl = 3`
/// and assert the `P1` and `P2` columns vanish while `P0` is unchanged.
///
/// **Result (2026-09-13).** `P1` and `P2` are exactly `0.0`; `P0` still
/// sums to the yield.
#[test]
fn isotropic_lab_input_has_vanishing_higher_moments() {
    let t = law1_tape(1.0, 1.0e-5, 2.0e6);
    let mut feed = Mf6Feed::new(&t, MAT, 6, MT, cfg(3)).unwrap();
    let eg = [1.0e-5, 5.0e5, 1.0e6, 1.5e6, 2.0e6];
    let at = feed.feed(2.0e6, &eg, 3).unwrap();
    let sum: f64 = at.ans[0].iter().sum();
    assert!((sum - at.yld).abs() < 1e-12, "{at:?}");
    for l in 1..3 {
        for (j, v) in at.ans[l].iter().enumerate() {
            assert_eq!(*v, 0.0, "moment {l} group {j} = {v}");
        }
    }
}

/// **Methodology.** With `LCT = 2` the same LAW=1 data is centre-of-mass,
/// so `getmf6` routes it through `cm2lab` (`:7812-7820`). For `A = 10`,
/// `A' = 1` the CM-motion factor is `xc = A'/(A+1)^2 = 1/121`, and a CM
/// spectrum capped at `E'_cm = 1e6` eV at `E = 2e6` eV cannot produce a lab
/// energy above `elmax = E (sqrt(E'_cm/E) + sqrt(xc))^2 = 1.274e6` eV
/// (`f6cm:8316`). Prediction: the top group `[1.5e6, 2e6]` eV gets exactly
/// zero, the rest carries the whole yield.
///
/// **Result (2026-09-13).** The top group is `0.0`; the remaining three
/// groups sum to the yield `2.0` to < 1e-12.
#[test]
fn cm_frame_law1_respects_the_kinematic_lab_energy_ceiling() {
    let t = law1_tape(2.0, 1.0e-5, 1.0e6);
    let mut feed = Mf6Feed::new(&t, MAT, 6, MT, cfg(1)).unwrap();
    assert_eq!(feed.lct(), 2);
    let eg = [1.0e-5, 5.0e5, 1.0e6, 1.5e6, 2.0e6];
    let at = feed.feed(2.0e6, &eg, 1).unwrap();
    let elmax = 2.0e6 * ((1.0e6f64 / 2.0e6).sqrt() + (1.0f64 / 121.0).sqrt()).powi(2);
    assert!(elmax < 1.5e6, "elmax {elmax}");
    assert_eq!(at.ans[0][3], 0.0, "{at:?}");
    let sum: f64 = at.ans[0].iter().sum();
    assert!((sum - at.yld).abs() < 1e-12, "{at:?}");
}

/// **Methodology.** `nl` is baked into the stored lab tables by `cm2lab`
/// (`:7813`), so asking `feed` for a different `nl` than `new` was given
/// cannot be honoured. Assert it is refused rather than silently padded.
#[test]
fn nl_mismatch_between_new_and_feed_is_refused() {
    let t = law1_tape(1.0, 1.0e-5, 2.0e6);
    let mut feed = Mf6Feed::new(&t, MAT, 6, MT, cfg(1)).unwrap();
    let eg = [1.0e-5, 1.0e6, 2.0e6];
    assert!(matches!(
        feed.feed(2.0e6, &eg, 3),
        Err(NjoyError::EndfParse(_))
    ));
}

/// **Methodology.** LAW=6 (n-body phase space, `:7823-7875`) is converted
/// upstream by calling `cm2lab` with `LANG = 0`, which needs `f6psp` — a
/// documented gap in [`crate::groupr::kinematics`]. Assert the documented
/// [`NjoyError::NotPorted`] rather than a fabricated spectrum.
#[test]
fn law6_phase_space_reports_not_ported() {
    let mut rows = vec![[1001.0, 10.0, 0.0, 1.0, 1.0, 0.0]];
    rows.extend(yield_rows(1.0, 1.0, 6.0, 1.0));
    rows.push([4.0, 0.0, 0.0, 0.0, 0.0, 5.0]); // APSX, NPSX CONT
    let t = tape(rows);
    match Mf6Feed::new(&t, MAT, 6, MT, cfg(1)) {
        Err(NjoyError::NotPorted(m)) => assert!(m.contains("LAW=6"), "{m}"),
        other => panic!("expected NotPorted, got {other:?}"),
    }
}

/// **Methodology.** LAW=2 (discrete two-body) leaves `getmf6` at `:7936`
/// into `getdis` with File-6 data (`mft = 8`), which
/// [`crate::groupr::two_body`] explicitly does not cover. Assert the
/// documented [`NjoyError::NotPorted`].
#[test]
fn law2_two_body_reports_not_ported() {
    let mut rows = vec![[1001.0, 10.0, 0.0, 1.0, 1.0, 0.0]];
    rows.extend(yield_rows(1.0, 1.0, 2.0, 1.0));
    let t = tape(rows);
    match Mf6Feed::new(&t, MAT, 6, MT, cfg(1)) {
        Err(NjoyError::NotPorted(m)) => assert!(m.contains("LAW=2..5"), "{m}"),
        other => panic!("expected NotPorted, got {other:?}"),
    }
}

/// **Methodology.** `mfd = 18` sets `zad = 0` (`:7586`); when no `ZAP = 0`
/// subsection exists, `:7603` takes the `199` special exit with
/// `enext = -1` instead of erroring. Assert that path.
#[test]
fn mfd18_without_a_zap0_subsection_takes_the_no_gammas_exit() {
    let t = law1_tape(1.0, 1.0e-5, 2.0e6);
    let feed = Mf6Feed::new(&t, MAT, 18, MT, cfg(1)).unwrap();
    assert!(feed.no_gammas());
    assert_eq!(feed.enext_init(), -1.0);
}

/// **Methodology.** Any other `mfd` whose particle is absent is the
/// `:7605-7606` hard error, not the `199` exit. `mfd = 21` asks for a
/// proton (`zad = 1001`), which this tape does not carry.
#[test]
fn missing_particle_is_an_error_for_non_fission_mfd() {
    let t = law1_tape(1.0, 1.0e-5, 2.0e6);
    match Mf6Feed::new(&t, MAT, 21, MT, cfg(1)) {
        Err(NjoyError::EndfParse(m)) => {
            assert!(m.contains("desired particle not found"), "{m}")
        }
        other => panic!("expected EndfParse, got {other:?}"),
    }
}

/// **Methodology.** `:8115-8120` folds every secondary group above the
/// group holding `ed` back into it, but only when the emitted particle is
/// the projectile (`zap == izap`) and `q <= 0`. At `ed = 1e6` eV with
/// boundaries `[1e-5, 5e5, 1e6, 1.5e6, 2e6]` the `jgmax` search
/// (`:8001-8005`) stops at `jgmax = 2`, so groups 3 and 4 must be emptied
/// into group 2.
///
/// Hand-derived expectation: the raw march gives `[0.5, 0.5, 0.5, 0.25]`
/// as in `flat_lab_spectrum_deposits_equally_and_sums_to_the_yield`;
/// `test` accumulates `ans(1,i)` *before* each fold (`:8113-8114`), so it
/// is still `1.75`, and the folded column is `[0.5, 1.25, 0, 0]` scaled by
/// `8/7` → `[4/7, 10/7, 0, 0]`.
///
/// **Result (2026-09-13).** `[4/7, 10/7, 0, 0]` to < 1e-9, sum `= 2.0`.
#[test]
fn illegal_upscatter_is_folded_into_the_ingroup() {
    let t = law1_tape(1.0, 1.0e-5, 2.0e6);
    let mut feed = Mf6Feed::new(&t, MAT, 6, MT, cfg(1)).unwrap();
    let eg = [1.0e-5, 5.0e5, 1.0e6, 1.5e6, 2.0e6];
    let at = feed.feed(1.0e6, &eg, 1).unwrap();
    assert_eq!(at.ans[0][2], 0.0, "{at:?}");
    assert_eq!(at.ans[0][3], 0.0, "{at:?}");
    assert!((at.ans[0][0] - 4.0 / 7.0).abs() < 1e-9, "{at:?}");
    assert!((at.ans[0][1] - 10.0 / 7.0).abs() < 1e-9, "{at:?}");
    let sum: f64 = at.ans[0].iter().sum();
    assert!((sum - at.yld).abs() < 1e-12, "{at:?}");
}

/// A LAW=7 (lab angle-energy) section: two incident energies, each with
/// two `mu` curves (`mu = -1, +1`) carrying the same flat, normalised
/// `f(E')` — i.e. isotropic in the lab.
fn law7_tape() -> Tape {
    let f0 = 1.0 / 2.0e6;
    let mut rows = vec![[1001.0, 10.0, 0.0, 1.0, 1.0, 0.0]];
    rows.extend(yield_rows(1.0, 1.0, 7.0, 2.0));
    // Outer TAB2 over incident energy: NE = 2.
    rows.push([0.0, 0.0, 0.0, 0.0, 1.0, 2.0]);
    rows.push([2.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
    for e_in in [1.0e6, 2.0e6] {
        // TAB2 over mu: NMU = 2.
        rows.push([0.0, e_in, 0.0, 0.0, 1.0, 2.0]);
        rows.push([2.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
        for mu in [-1.0, 1.0] {
            rows.push([0.0, mu, 0.0, 0.0, 1.0, 2.0]);
            rows.push([2.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
            rows.push([1.0e-5, f0, 2.0e6, f0, 0.0, 0.0]);
        }
    }
    tape(rows)
}

/// **Methodology.** LAW=7 (`:7876-7913`) is read as a TAB2 of incident
/// energies, each holding a TAB2 of `mu` with one `f(E')` TAB1 per cosine,
/// and is converted to a lab Legendre table by `ll2lab` before the feed
/// ever runs. With `f(E')` independent of `mu` the emission is isotropic,
/// so every moment above `P0` must vanish, and `P0` must carry the whole
/// yield after the label-700 renormalisation.
///
/// This also pins `langn`/`lepn` being forced to `1`/`2` for LAW=7
/// (`:7992`, `:7997`) — without that forcing `f6lab` would reject the
/// TAB2's `LANG = 0`.
///
/// **Result (2026-09-13).** `P0` sums to `yld = 2.0` to < 1e-12; `P1` is
/// `<= 6e-12` in every group — the residue of `ll2lab`'s 8-point
/// Gauss-Legendre mu-quadrature (`groupr.f90:8949-8956`), i.e. ~1e-11 of
/// the `P0` column value, not a real moment.
#[test]
fn law7_lab_angle_energy_is_converted_and_stays_isotropic() {
    let t = law7_tape();
    let mut feed = Mf6Feed::new(&t, MAT, 6, MT, cfg(2)).unwrap();
    assert_eq!(feed.nss(), 1);
    let eg = [1.0e-5, 5.0e5, 1.0e6, 1.5e6, 2.0e6];
    let at = feed.feed(2.0e6, &eg, 2).unwrap();
    let sum: f64 = at.ans[0].iter().sum();
    assert!((sum - at.yld).abs() < 1e-12, "{at:?}");
    for (j, v) in at.ans[1].iter().enumerate() {
        assert!(v.abs() < 1e-9, "P1 group {j} = {v}");
    }
}

/// **Regression pin for a defect that WAS in
/// [`crate::groupr::kinematics::f6lab`], fixed 2026-09-13.**
///
/// `groupr.f90:9214` guards the low table's coefficient interpolation with
/// `llo /= ilo`, where `ilo` is the first continuum record and `llo` starts
/// one record later whenever that first record sits at `E' = 0`
/// (`:9119-9121`). The guard is therefore *true* from the first call and the
/// leading `[0, E'_1]` panel contributes normally. `f6lab` had been comparing
/// the running pointer against its own already-advanced start (`lo_ptr !=
/// lo_first`), which is *false* on that first panel, so the panel was dropped
/// — and since [`crate::groupr::kinematics::cm2lab`] and
/// [`crate::groupr::kinematics::ll2lab`] both always emit a leading `E' = 0`
/// point, every CM-frame and LAW=7 feed lost its first secondary-energy panel.
///
/// **Methodology.** Two tables whose only two points are `(0, f0)` and
/// `(2e6, f0)`: the whole spectrum lives in that one panel. Before the fix the
/// feed came back identically zero and the label-700 normalisation bailed out
/// at `test == 0` (`:8121`) leaving it zero. After the fix it matches the
/// shifted-grid test's `[4/7, 4/7, 4/7, 2/7]` exactly — the leading zero point
/// is now just an ordinary left end, which is what it is upstream.
#[test]
fn leading_zero_energy_point_contributes_its_panel() {
    let t = law1_tape(1.0, 0.0, 2.0e6);
    let mut feed = Mf6Feed::new(&t, MAT, 6, MT, cfg(1)).unwrap();
    let eg = [1.0e-5, 5.0e5, 1.0e6, 1.5e6, 2.0e6];
    let at = feed.feed(2.0e6, &eg, 1).unwrap();
    assert!((at.yld - 2.0).abs() < 1e-12, "{at:?}");
    let want = [4.0 / 7.0, 4.0 / 7.0, 4.0 / 7.0, 2.0 / 7.0];
    for (g, (&got, &exp)) in at.ans[0].iter().zip(&want).enumerate() {
        assert!(
            (got - exp).abs() < 1.0e-9,
            "group {g}: {got} against {exp} — the leading E' = 0 panel must \
             contribute (groupr.f90:9214 guards on `llo /= ilo`, not on the \
             post-skip start)"
        );
    }
}
