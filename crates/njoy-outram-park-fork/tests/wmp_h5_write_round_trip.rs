// SPDX-License-Identifier: GPL-3.0

//! **WMP library write → read round trip** — gh:#270 acceptance bullet 1,
//! *"write a file from this workspace, read it back here, and get bit-identical
//! data"*.
//!
//! # Methodology
//!
//! The source is the embedded CORE library (`src/data/wmp_core.wmpl`, MIT CRPG
//! ENDF/B-VII.1), so this needs no external data and no network.
//! `WindowedMultipole::write_h5` emits a `WMP_Library`-format `.h5`;
//! `WindowedMultipole::load_h5` reads it back; the two records are compared
//! **field by field with `f64::to_bits`** — exact, no tolerance. A tolerance
//! here would be meaningless: nothing in the path does arithmetic, so any
//! difference at all is a codec defect.
//!
//! Both a **fissionable** nuclide (U-238: 4 `data` columns, 3 curve-fit
//! channels) and a **non-fissionable** one (Fe-56: 3 and 2) are covered,
//! because the column count is the one thing the writer branches on and a test
//! on one shape would not see the other break.
//!
//! The evaluated cross sections are compared too, at temperatures and energies
//! spanning the multipole range — the round trip preserving *fields* is what is
//! asserted, but a reader is entitled to know the evaluation agrees as well.
//!
//! # Results (2026-09-24, embedded CORE WMPL, ENDF/B-VII.1)
//!
//! Printed by the tests. Every field bit-identical on both nuclides; every
//! evaluated `(scatter, absorption, fission)` bit-identical at 5 energies x
//! 3 temperatures.
//!
//! # What this does NOT establish
//!
//! That **OpenMC** can read the file. This crate's reader is deliberately
//! permissive about two things OpenMC is not (compound field naming, and scalar
//! vs length-1 datasets), so a round trip through this crate alone would pass
//! on a file OpenMC cannot use. That is `tests/wmp_h5_vs_openmc.rs`.

use njoy_outram_park_fork::wmp::{WindowedMultipole, WmpLibrary};

/// Scratch path unique to this process **and to the calling test**.
///
/// The `tag` is not decoration. `round_trip("U238")` is called by two different
/// tests in this binary, which cargo runs as parallel threads of one process; a
/// path keyed only on the process id gave both the same file, so one test
/// deleted the file the other was still reading. That showed up as a failure
/// only when this binary ran alongside another (which changed the timing) and
/// passed in isolation — the shape of a defect that gets written off as a flake.
/// It was neither: it was this helper.
fn scratch(tag: &str, name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("wmp_h5_rt_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir.join(format!("{name}.h5"))
}

/// Compare two records exactly, naming the first field that differs.
fn assert_identical(a: &WindowedMultipole, b: &WindowedMultipole) {
    assert_eq!(a.name, b.name, "name");
    assert_eq!(a.fissionable, b.fissionable, "fissionable");
    assert_eq!(a.fit_order, b.fit_order, "fit_order");
    for (label, x, y) in [
        ("awr", a.awr, b.awr),
        ("e_min", a.e_min, b.e_min),
        ("e_max", a.e_max, b.e_max),
        ("inv_spacing", a.inv_spacing, b.inv_spacing),
    ] {
        assert_eq!(x.to_bits(), y.to_bits(), "{label}: {x} vs {y}");
    }

    assert_eq!(a.poles.len(), b.poles.len(), "pole count");
    for (i, (p, q)) in a.poles.iter().zip(&b.poles).enumerate() {
        assert_eq!(p.re.to_bits(), q.re.to_bits(), "pole {i} re");
        assert_eq!(p.im.to_bits(), q.im.to_bits(), "pole {i} im");
    }

    assert_eq!(a.residues.len(), b.residues.len(), "residue rows");
    for (i, (r, s)) in a.residues.iter().zip(&b.residues).enumerate() {
        for c in 0..3 {
            assert_eq!(r[c].re.to_bits(), s[c].re.to_bits(), "residue {i} ch{c} re");
            assert_eq!(r[c].im.to_bits(), s[c].im.to_bits(), "residue {i} ch{c} im");
        }
    }

    assert_eq!(a.curvefit.len(), b.curvefit.len(), "curvefit windows");
    for (w, (rw, sw)) in a.curvefit.iter().zip(&b.curvefit).enumerate() {
        assert_eq!(rw.len(), sw.len(), "curvefit window {w} length");
        for (k, (rc, sc)) in rw.iter().zip(sw).enumerate() {
            for c in 0..3 {
                assert_eq!(
                    rc[c].to_bits(),
                    sc[c].to_bits(),
                    "curvefit w{w} term{k} ch{c}: {} vs {}",
                    rc[c],
                    sc[c]
                );
            }
        }
    }

    assert_eq!(a.windows.len(), b.windows.len(), "window count");
    for (w, (x, y)) in a.windows.iter().zip(&b.windows).enumerate() {
        // An empty window is normalised to (1, 0) on both sides — see the
        // writer's lossiness note. Compare emptiness, then the bounds.
        let (x_empty, y_empty) = (x.end < x.start, y.end < y.start);
        assert_eq!(x_empty, y_empty, "window {w} emptiness");
        if !x_empty {
            assert_eq!((x.start, x.end), (y.start, y.end), "window {w} bounds");
        }
        assert_eq!(x.broaden_poly, y.broaden_poly, "window {w} broaden_poly");
    }
}

fn round_trip(tag: &str, name: &str) -> (WindowedMultipole, WindowedMultipole) {
    let src = WmpLibrary::core()
        .get(name)
        .unwrap_or_else(|e| panic!("{name} must be in the embedded CORE library: {e}"));
    let path = scratch(tag, name);
    src.write_h5(&path)
        .unwrap_or_else(|e| panic!("{name}: write_h5 failed: {e}"));
    let back = WindowedMultipole::load_h5(&path)
        .unwrap_or_else(|e| panic!("{name}: load_h5 of our own file failed: {e}"));
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    println!(
        "{name}: wrote {bytes} bytes — {} poles, {} windows, fit_order {}, fissionable {}",
        src.poles.len(),
        src.windows.len(),
        src.fit_order,
        src.fissionable
    );
    let _ = std::fs::remove_file(&path);
    (src, back)
}

/// **A fissionable nuclide** — 4 `data` columns and 3 curve-fit channels.
#[test]
fn u238_survives_the_round_trip_bit_for_bit() {
    let (src, back) = round_trip("u238_fields", "U238");
    assert!(src.fissionable, "U-238 must be fissionable in this library");
    assert_identical(&src, &back);
    println!("U238: every field bit-identical through write_h5 -> load_h5");
}

/// **A non-fissionable nuclide** — 3 `data` columns and 2 curve-fit channels.
/// The column count is the only thing the writer branches on, so this is not a
/// redundant case.
#[test]
fn fe56_survives_the_round_trip_bit_for_bit() {
    let (src, back) = round_trip("fe56_fields", "Fe56");
    assert!(
        !src.fissionable,
        "Fe-56 must be non-fissionable, or this test is not covering the 3-column shape"
    );
    assert_identical(&src, &back);
    println!("Fe56: every field bit-identical through write_h5 -> load_h5");
}

/// The evaluation agrees too — not merely the stored fields.
#[test]
fn the_reloaded_record_evaluates_identically() {
    for name in ["U238", "Fe56"] {
        let (src, back) = round_trip("evaluate", name);
        let mut checked = 0usize;
        for &t in &[0.0_f64, 293.6, 1200.0] {
            // Spread across the multipole range in sqrt(E), where the windows live.
            for k in 0..5 {
                let f = (k as f64 + 0.5) / 5.0;
                let e = (src.e_min.sqrt() + f * (src.e_max.sqrt() - src.e_min.sqrt())).powi(2);
                let a = src.evaluate(e, t);
                let b = back.evaluate(e, t);
                for (label, x, y) in [
                    ("scatter", a.scatter, b.scatter),
                    ("absorption", a.absorption, b.absorption),
                    ("fission", a.fission, b.fission),
                ] {
                    assert_eq!(
                        x.to_bits(),
                        y.to_bits(),
                        "{name} {label} at E={e:.6e} eV, T={t} K: {x} vs {y}"
                    );
                }
                checked += 1;
            }
        }
        println!("{name}: {checked} (E, T) points evaluate bit-identically after the round trip");
    }
}

/// A malformed record is refused **here**, where the cause can be named, rather
/// than producing a file whose reader has to guess.
#[test]
fn an_inconsistent_record_is_refused_before_it_reaches_disk() {
    let good = WmpLibrary::core().get("U238").expect("U238");

    let mut no_name = good.clone();
    no_name.name.clear();
    assert!(no_name.write_h5(scratch("refuse", "bad_noname")).is_err());

    let mut short_residues = good.clone();
    short_residues.residues.pop();
    let e = short_residues
        .write_h5(scratch("refuse", "bad_residues"))
        .unwrap_err();
    assert!(format!("{e}").contains("residue rows"), "{e}");

    let mut short_curvefit = good.clone();
    short_curvefit.curvefit.pop();
    let e = short_curvefit
        .write_h5(scratch("refuse", "bad_curvefit"))
        .unwrap_err();
    assert!(format!("{e}").contains("curve-fit rows"), "{e}");

    // OpenMC refuses a curve fit with fewer than 3 terms; so do we, rather than
    // writing a file it will reject.
    let mut low_order = good.clone();
    low_order.fit_order = 1;
    let e = low_order.write_h5(scratch("refuse", "bad_order")).unwrap_err();
    assert!(format!("{e}").contains("at least 3"), "{e}");

    println!("four malformed records refused at write time, each naming its own cause");
}
