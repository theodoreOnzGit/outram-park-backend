//! Regression: BROADR must not run SIGMA1 across the resolved/unresolved seam
//! (`op-sdbk`, ENDF/B-VIII.0 U-238, MAT 9237).
//!
//! Fixture: `reference-data/endf/n-092_U_238.endf` (skips when absent).
//!
//! ## What upstream does (read before touching this test)
//!
//! - `reconr.f90` `rdfil2` ("shade nodes to prevent discontinuities") and
//!   `lunion` ("check ahead for discontinuity") never write two PENDF points
//!   at one energy: a step at `E` becomes `sigfig(E,7,-1)` / `sigfig(E,7,+1)`.
//!   U-238's MF=3/MT=102 tabulates `(2e4, 0.0)` then `(2e4, 0.52987)` — the
//!   evaluator's infinitely-dilute unresolved value starts at 20 keV — so the
//!   PENDF carries `1.999999e4` (resolved side) and `2.000001e4` (unresolved).
//! - `broadr.f90` bounds broadening at `thnmax`, by default the upper limit
//!   RECONR wrote on the MF=2 range record (`eresr` = top of the resolved
//!   range = 2e4 eV here; `broadr.f90:354-441`), and copies every point above
//!   it through unchanged (label 190, `broadr.f90:1493-1503`).
//!
//! ## Measured before the fix (develop e09239ee, `examples/seam_stage_probe`)
//!
//! RECONR emitted a single point at 2.0e4 (0.56018 b), BROADR at 600 K gave
//! 0.30629 b there, and the error was interpolated to 24 keV: MT=102 was
//! 46.7 % low at the seam and 41 % low at 20.5 keV. After the fix the
//! unresolved side is bit-identical to the tape from 2.000001e4 eV upward.
//!
//! ## NJOY2016 oracle (same tape, `reconr 0.001` + `broadr 600 K`, 2026-09-10)
//!
//! Upstream's own PENDF (built from commit `ac5adf5` with gfortran 13.3.0):
//! `1.999999e4 → 0.2809805 b`, `2.000001e4 → 0.5298699 b`, and lin-lin at
//! 20.5 / 21 / 22 / 23 keV `0.52262 / 0.51536 / 0.50086 / 0.48635 b`; its
//! listing prints "final maximum energy for broadening/thinning =
//! 2.00000E+04 eV". This port after the fix: `0.2809594` (7e-5 relative)
//! and `0.529870` at the two shaded points, and the same four interpolants
//! to every printed digit. Those oracle values are asserted below.

use njoy_outram_park_fork::broadr::{broaden_result, broadening_limit};
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::mixr::mix::sigfig;
use njoy_outram_park_fork::reconr::{eval_lin_lin, reconr, ReconrConfig, ReconrSection};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

fn find(secs: &[ReconrSection], mt: i32) -> &ReconrSection {
    secs.iter()
        .find(|s| i32::from(s.mt) == mt)
        .unwrap_or_else(|| panic!("no MT={mt}"))
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

#[test]
fn u238_urr_seam_survives_reconr_and_bounded_broadr() {
    let Some(path) = reference_endf_or_skip("n-092_U_238.endf", "u238-urr-seam") else {
        return;
    };
    let tape = Tape::read_file(&path).unwrap();
    let mat = tape.materials()[0];
    let tape_mt102 = {
        let sec = tape.section(mat, 3, 102).unwrap();
        let mut cur = SectionCursor::new(&sec.rows);
        cur.read_cont().unwrap();
        cur.read_tab1().unwrap().pairs
    };
    // The evaluation's own seam: (2e4, 0.0) then (2e4, 0.52987).
    let seam = 2.0e4;
    let i = tape_mt102.iter().position(|&(e, _)| e == seam).unwrap();
    assert_eq!(tape_mt102[i], (seam, 0.0));
    assert_eq!(tape_mt102[i + 1].0, seam);
    let urr_value = tape_mt102[i + 1].1; // 0.52987
    assert!(urr_value > 0.5);

    let r = reconr(&tape, &ReconrConfig { mat, tolerance: 1e-3, temperature: 0.0 }).unwrap();

    // thnmax = top of the resolved range (broadr.f90 rule (i)).
    assert_eq!(r.resonance_upper_limit, Some(seam));
    assert_eq!(broadening_limit(&r), seam);

    // RECONR carries the step as two distinct, sigfig-shaded energies.
    let lo = sigfig(seam, 7, -1);
    let hi = sigfig(seam, 7, 1);
    assert!(lo < seam && seam < hi, "shading: {lo} < {seam} < {hi}");
    let cap0 = &find(&r.sections, 102).pairs;
    assert!(
        !cap0.iter().any(|&(e, _)| e == seam),
        "RECONR must not leave a point exactly at the seam"
    );
    let at = |pairs: &[(f64, f64)], e: f64| {
        pairs
            .iter()
            .find(|&&(x, _)| rel(x, e) < 1e-12)
            .map(|&(_, s)| s)
            .unwrap_or_else(|| panic!("no grid point at {e}"))
    };
    let res_side_0k = at(cap0, lo);
    let urr_side_0k = at(cap0, hi);
    assert!(
        res_side_0k < 0.1,
        "resolved side is MF=3 (0.0) + resonance tail only, got {res_side_0k}"
    );
    assert!(
        rel(urr_side_0k, urr_value) < 1e-9,
        "unresolved side at 0 K must be the tape value {urr_value}, got {urr_side_0k}"
    );

    // BROADR at 600 K, bounded at thnmax.
    let b = broaden_result(&r, 600.0);
    for mt in [1, 2, 18, 102] {
        let before = &find(&r.sections, mt).pairs;
        let after = &find(&b.sections, mt).pairs;
        assert_eq!(before.len(), after.len(), "MT={mt}: grid length changed");
        let split = before.partition_point(|&(e, _)| e <= seam);
        assert_eq!(
            &before[split..],
            &after[split..],
            "MT={mt}: every point above thnmax must be copied through bit-for-bit"
        );
        assert!(split > 0 && split < before.len());
    }
    let capb = &find(&b.sections, 102).pairs;
    // Unresolved side of the seam: untouched.
    assert!(rel(at(capb, hi), urr_value) < 1e-9);
    // 20–24 keV: the tape's own lin-lin MF=3 (the pre-fix kernel was 41 %
    // low at 20.5 keV and 25 % low at 22 keV here). The only difference left
    // is that the URR-side node sits at the shaded 2.000001e4 rather than at
    // 2e4, which moves the interpolant by ~3e-7 relative — hence 1e-5, not
    // machine precision.
    for e in [2.05e4, 2.1e4, 2.2e4, 2.3e4] {
        let want = eval_lin_lin(&tape_mt102, e);
        let got = eval_lin_lin(capb, e);
        assert!(rel(got, want) < 1e-5, "E={e}: got {got}, tape {want}");
    }
    // Resolved side of the seam IS broadened, and — as in upstream, whose
    // SIGMA1 integral also spans all loaded pages — it sees the step above
    // it: strictly between the 0 K resolved value and the unresolved value.
    let res_side_b = at(capb, lo);
    assert!(
        res_side_b > res_side_0k && res_side_b < urr_value,
        "resolved side at 600 K: {res_side_b} (0 K {res_side_0k}, URR {urr_value})"
    );
    // NJOY2016 oracle values (see the module docs). The resolved-side point
    // depends on the reconstruction grid below it, hence 1e-3; the seam
    // interpolants are grid-independent and match to print precision.
    assert!(rel(res_side_b, 0.280_980_5) < 1e-3, "NJOY 0.2809805 vs {res_side_b}");
    for (e, njoy) in [(2.05e4, 0.52262), (2.1e4, 0.51536), (2.2e4, 0.50086), (2.3e4, 0.48635)] {
        let got = eval_lin_lin(capb, e);
        assert!(rel(got, njoy) < 2e-5, "E={e}: {got} vs NJOY {njoy}");
    }
}
