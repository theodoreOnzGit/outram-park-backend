// SPDX-License-Identifier: GPL-3.0

//! **OpenMC reads a WMP library this workspace wrote** — gh:#270 acceptance
//! bullet 2, *"OpenMC reads a file this workspace wrote and produces a `k`
//! consistent with the run that produced it. This is the check that makes the
//! format claim real rather than self-consistent."*
//!
//! For WMP the comparable quantity is the **evaluated cross section**, not `k`:
//! a windowed-multipole library is not a transport input on its own, and
//! comparing σ(E, T) directly is a far tighter statement than comparing an
//! eigenvalue through a Monte Carlo run with its own statistics.
//!
//! # Methodology
//!
//! 1. Take a nuclide from the embedded CORE library (`src/data/wmp_core.wmpl`,
//!    MIT CRPG ENDF/B-VII.1) — no external data, no network.
//! 2. Write it with [`WindowedMultipole::write_h5`].
//! 3. Run `verification_and_validation/wmp_h5_write/openmc_inputs/read_wmp.py`,
//!    which reads the file with **`openmc.data.WindowedMultipole.from_hdf5`**
//!    and evaluates it. This needs no cross-section library — `from_hdf5` plus
//!    scipy's Faddeeva is the whole dependency — which is why this cross-code
//!    check is runnable here at all.
//! 4. Compare structure *and* numbers against our own evaluation of the same
//!    record.
//!
//! Floats cross the boundary as Python `repr`, which is shortest-round-trip and
//! parses back to the identical `f64` in Rust, so the comparison is not limited
//! by the transport format.
//!
//! **Structure is asserted, not just values.** Two mistakes produce a file that
//! round-trips perfectly through *this* crate and is useless to OpenMC, so both
//! are checked on the Python side and pinned here:
//!
//! * `data` must come back as **`complex128`**. h5py recognises a two-field f64
//!   compound as complex only when the field names match its `complex_names`
//!   default, `('r', 'i')`. `hdf5-pure`'s `with_complex64_data` helper spells
//!   them `real` / `imag`, which yields a *structured* array instead.
//! * the scalars must be **0-dimensional**. OpenMC reads `group['spacing'][()]`;
//!   from a shape-`[1]` dataset that returns `array([x])`, and `spacing` then
//!   silently becomes an array that broadcasts through the evaluation.
//!
//! # Tolerance, chosen before measuring and deliberately loose
//!
//! `1e-9` relative. The two sides are independent Faddeeva implementations —
//! ours is Weideman, OpenMC's is `scipy.special.wofz` — so bitwise agreement is
//! not expected and a bits gate would be wrong. `1e-9` is generous for
//! double-precision evaluations of the same closed form, on purpose: this test
//! exists to catch a **broken file**, not to certify the two Faddeeva routines
//! against each other to their last bit. The measured agreement is far tighter
//! and is recorded below rather than used to tighten the gate.
//!
//! # Results (2026-09-24, OpenMC `0.1.dev1+gafa7a14ac`, embedded CORE WMPL)
//!
//! Printed by the test. 45 values per nuclide (5 energies x 3 temperatures x 3
//! channels), and the structural assertions (`complex128`, 0-d scalars,
//! pole/window counts, `fit_order`, and every scalar bit-exact) all hold.
//!
//! | nuclide | values | worst relative difference | where |
//! |---|---|---|---|
//! | U-238 | 45 | **1.197e-12** | scatter, 100 eV, 293.6 K |
//! | Fe-56 | 45 | **7.390e-14** | absorption, 1 keV, 1200 K |
//!
//! Three to four decades inside the `1e-9` gate, and the right *kind* of
//! residual: nonzero, at the level two different Faddeeva implementations at
//! double precision should differ by. The prediction recorded before the run was
//! "near 1e-10 relative, not bits"; the measurement is a little better than that
//! and the same order.
//!
//! **A correction worth keeping.** This paragraph first said the values were
//! **bit-identical**, written from a hand check that printed 10 significant
//! figures — at which precision they *are* equal. They are not: at full
//! precision the residual above is real. Printing fewer digits than a claim
//! needs is how a "bit-identical" that is merely "agrees to 1e-10" gets into a
//! record.
//!
//! # Skips rather than fails
//!
//! When no OpenMC-capable interpreter is present the test prints `SKIP` and
//! passes — the reference side is an external tool, and the workspace rule is
//! that absent reference data skips. The round-trip half of the claim is
//! covered unconditionally by `tests/wmp_h5_write_round_trip.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use njoy_outram_park_fork::wmp::WmpLibrary;

/// Interpreters to try, in order. The first that imports `openmc.data` wins.
const PYTHONS: [&str; 3] = ["/opt/ompy/bin/python", "python3", "python"];
/// OpenMC's source tree, needed on `PYTHONPATH` for an in-place checkout.
const OPENMC_SRC: &str = "/opt/src/openmc";

/// Gate: relative, and loose on purpose — see the module note.
const REL_TOL: f64 = 1e-9;

const TEMPS: [f64; 3] = [293.6, 600.0, 1200.0];
const ENERGIES: [f64; 5] = [1.0, 6.67, 20.9, 100.0, 1000.0];

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/wmp_h5_write/openmc_inputs/read_wmp.py")
}

/// An interpreter that can `import openmc.data`, or `None`.
fn find_python() -> Option<String> {
    for p in PYTHONS {
        let ok = Command::new(p)
            .env("PYTHONPATH", OPENMC_SRC)
            .args(["-c", "import openmc.data, h5py"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return Some(p.to_string());
        }
    }
    None
}

fn scratch_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("wmp_h5_openmc_{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// One `meta|` or `xs|` record from the reference script.
enum Record {
    Meta(String, Vec<String>),
    Xs { t: f64, e: f64, xs: [f64; 3] },
}

fn parse(stdout: &str) -> Vec<Record> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let f: Vec<&str> = line.split('|').collect();
        match f.first().copied() {
            Some("meta") => out.push(Record::Meta(
                f[1].to_string(),
                f[2..].iter().map(|s| s.to_string()).collect(),
            )),
            Some("xs") => {
                let n = |i: usize| -> f64 {
                    f[i].parse()
                        .unwrap_or_else(|e| panic!("field {i} of {line:?}: {e}"))
                };
                out.push(Record::Xs {
                    t: n(1),
                    e: n(2),
                    xs: [n(3), n(4), n(5)],
                });
            }
            _ => {}
        }
    }
    out
}

fn compare(name: &str, python: &str) {
    let src = WmpLibrary::core()
        .get(name)
        .unwrap_or_else(|e| panic!("{name} in the embedded CORE library: {e}"));
    let path = scratch_dir().join(format!("{name}.h5"));
    src.write_h5(&path)
        .unwrap_or_else(|e| panic!("{name}: write_h5: {e}"));

    let temps: Vec<String> = TEMPS.iter().map(|t| t.to_string()).collect();
    let energies: Vec<String> = ENERGIES.iter().map(|e| e.to_string()).collect();
    let out = Command::new(python)
        .env("PYTHONPATH", OPENMC_SRC)
        .arg(script())
        .arg(&path)
        .arg(name)
        .arg(temps.join(","))
        .arg(energies.join(","))
        .output()
        .expect("running the reference script");
    assert!(
        out.status.success(),
        "{name}: OpenMC FAILED TO READ the file this workspace wrote.\n\
         stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let records = parse(&stdout);
    assert!(!records.is_empty(), "{name}: no records parsed from\n{stdout}");

    // --- Structure: the two mistakes that pass a same-crate round trip ---
    let meta = |key: &str| -> String {
        records
            .iter()
            .find_map(|r| match r {
                Record::Meta(k, v) if k == key => Some(v.join("|")),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{name}: no meta|{key} in\n{stdout}"))
    };
    assert_eq!(
        meta("dtype"),
        "complex128",
        "{name}: h5py did not see `data` as complex128, so OpenMC cannot evaluate it. \
         The compound field names must be `r`/`i`, not `real`/`imag`."
    );
    assert_eq!(
        meta("scalar_ndim"),
        "0",
        "{name}: `spacing` is not a 0-d dataset, so OpenMC's `[()]` yields an array \
         rather than a float and it broadcasts silently through the evaluation."
    );
    assert_eq!(meta("name"), name, "{name}: group name");
    let n_cols = if src.fissionable { 4 } else { 3 };
    assert_eq!(
        meta("shape"),
        format!("{}|{}", src.poles.len(), n_cols),
        "{name}: `data` shape"
    );
    assert_eq!(
        meta("windows"),
        src.windows.len().to_string(),
        "{name}: window count"
    );
    assert_eq!(
        meta("fit_order"),
        src.fit_order.to_string(),
        "{name}: fit_order as OpenMC infers it from the curvefit shape"
    );
    for (key, ours) in [
        ("spacing", 1.0 / src.inv_spacing),
        ("sqrtAWR", src.awr.sqrt()),
        ("E_min", src.e_min),
        ("E_max", src.e_max),
    ] {
        let theirs: f64 = meta(key).parse().expect("scalar");
        assert_eq!(
            theirs.to_bits(),
            ours.to_bits(),
            "{name}: {key} came back {theirs} against {ours} — a scalar must survive exactly"
        );
    }

    // --- Values ---
    let mut worst = 0.0f64;
    let mut worst_at = String::new();
    let mut n = 0usize;
    for r in &records {
        let Record::Xs { t, e, xs } = r else { continue };
        let ours = src.evaluate(*e, *t);
        for (label, a, b) in [
            ("scatter", ours.scatter, xs[0]),
            ("absorption", ours.absorption, xs[1]),
            ("fission", ours.fission, xs[2]),
        ] {
            let scale = a.abs().max(b.abs());
            let rel = if scale == 0.0 {
                0.0
            } else {
                (a - b).abs() / scale
            };
            if rel > worst {
                worst = rel;
                worst_at = format!("{label} at E={e:.4e} eV, T={t} K ({a} vs {b})");
            }
            assert!(
                rel <= REL_TOL,
                "{name}: {label} at E={e:.6e} eV, T={t} K differs by {rel:.3e} relative \
                 (ours {a}, OpenMC {b}), over the {REL_TOL:.0e} gate"
            );
            n += 1;
        }
    }
    assert!(n >= 3 * TEMPS.len() * ENERGIES.len(), "{name}: only {n} values compared");
    let _ = std::fs::remove_file(&path);

    if worst == 0.0 {
        println!(
            "{name}: OpenMC read our file and all {n} values are BIT-IDENTICAL to ours \
             (complex128, 0-d scalars, {} poles, {} windows)",
            src.poles.len(),
            src.windows.len()
        );
    } else {
        println!(
            "{name}: OpenMC read our file; {n} values agree, worst {worst:.3e} relative \
             at {worst_at}"
        );
    }
}

/// **Fissionable** — 4 `data` columns, 3 curve-fit channels.
#[test]
fn openmc_reads_our_u238_library_and_agrees() {
    let Some(py) = find_python() else {
        println!("SKIP: no interpreter that can `import openmc.data, h5py`");
        return;
    };
    compare("U238", &py);
}

/// **Non-fissionable** — 3 `data` columns, 2 curve-fit channels. The shape the
/// writer branches on, so this is not a duplicate of the case above.
#[test]
fn openmc_reads_our_fe56_library_and_agrees() {
    let Some(py) = find_python() else {
        println!("SKIP: no interpreter that can `import openmc.data, h5py`");
        return;
    };
    compare("Fe56", &py);
}
