//! **Bit-identity gate for moving outram-mc's depletion MATH into PETIR**
//! (GitHub issue #950, 2026-10-17).
//!
//! # Methodology
//!
//! The move relocates pure arithmetic: the dense burnup matrix
//! (`DepletionMatrix`), the IPF CRAM solver (`cram16`, `cram48`) and the time
//! integrators. The chain, the burnup-matrix *entries* and the operators stay
//! here. The CRAM and matrix move must not change a single bit of any result.
//!
//! Each case below runs a fixed calculation through the ordinary public API
//! and folds every output `f64` (`to_bits`) into a 64-bit FNV-1a fingerprint,
//! which is pinned. The pins were printed by THIS FILE on the commit before
//! the move (`DEPL_FP_PRINT=1 cargo test --release -p outram-mc-libs --test
//! depletion_move_fingerprints -- --nocapture`), then the move was made and
//! the file re-run unchanged. A fingerprint is a hash, so a mismatch says only
//! that something moved; rerun with `DEPL_FP_PRINT=1` to see which case.
//!
//! None of these cases calls a transcendental from the platform libm on the
//! CRAM path (CRAM is rational arithmetic); the cross-section collapse and
//! the chain constructor do, so a different libm may need a re-pin there.
//!
//! # Results
//!
//! Pre-move pins printed 2026-10-17 on `6b8607eac2` (x86_64 Linux, glibc),
//! committed as `357033acbe` before any code moved.
//!
//! | case | pre-move | post-move |
//! |---|---|---|
//! | `cram` (CRAM16 + CRAM48, 9x9 notebook matrix at six `dt`, stiff 2x2, 4-chain; `mat_vec`) | `0xfa7d4eb002310aa0` | identical |
//! | `combine` | `0x8356f9616a1ff738` | identical |
//! | `predictor_notebook` (`deplete_predictor`, the `depletion` notebook case) | `0xdae6ceb68ed967b9` | identical |
//! | `with_predictor`, `with_cecm` (`deplete_with`, 16 x 2.5 d at 1 MW) | `0xbea5e79785e15e2a`, `0xe779fefcb24c2d1a` | identical |
//! | `independent_predictor`, `independent_cecm` (`deplete_independent`, 8 x 5 d) | `0x4704be5bbd25a7a9`, `0xf309d7a9bbe2fbcf` | identical |
//! | `coupled_no_spectrum` (`deplete_coupled`) | `0x4704be5bbd25a7a9` | identical |
//! | `with_cf4` | ~~`0x81bdd6ab5450c252`~~ | **`0x0832d959f308025b`** (changed on purpose) |
//! | `independent_cf4` | ~~`0xa186a9d4be6e5777`~~ | **`0xe99d021de20c7dc3`** (changed on purpose) |
//! | `with_cf4_premove_replay` (the pre-move CF4 algorithm on the moved pieces) | — | `0x81bdd6ab5450c252`, the pre-move `with_cf4` pin |
//!
//! **Result (2026-10-17): every case that was required to be bit-identical
//! is.** The last row is the control for the two that changed: replaying the
//! removed CF4 code verbatim on the moved CRAM16 and matrix reproduces the
//! pre-move CF4 fingerprint exactly, so the CF4 change is the algorithm and
//! nothing in the move of the math.
//!
//! The CF4 cases are the deliberate exception: the pre-move CF4 applied its
//! two final exponentials in the order upstream's DOCSTRING writes them,
//! while upstream's CODE (`openmc/deplete/integrators.py:180-183`) applies
//! `cf4_f3` first and `cf4_f4` second; it also skipped upstream's clip of the
//! first stage (`abc.py:765-766`). The PETIR port follows the code. The two
//! pre-move CF4 pins are kept in the table, struck, beside the new values;
//! `verification_and_validation/depletion/petir_integrators_2026_10_17.md`
//! records the measured consequence.

use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::cram::{cram16, cram48};
use outram_mc_libs::depletion::integrators::{combine, deplete_independent, Integrator, MicroXs};
use outram_mc_libs::depletion::matrix::DepletionMatrix;
use outram_mc_libs::depletion::operator::{
    deplete_coupled, deplete_predictor, deplete_with, flux_for_power, one_group_cross_sections,
    reaction_rates, BurnupResult, BurnupSettings,
};

/// FNV-1a over the bit patterns of `xs`.
fn fnv(state: &mut u64, xs: &[f64]) {
    for x in xs {
        for b in x.to_bits().to_le_bytes() {
            *state ^= u64::from(b);
            *state = state.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fold_result(r: &BurnupResult) -> u64 {
    let mut h = FNV_OFFSET;
    for s in &r.steps {
        fnv(&mut h, &[s.time_days, s.flux, s.k_inf]);
        let d: Vec<f64> = s.densities.iter().map(|(_, v)| *v).collect();
        fnv(&mut h, &d);
    }
    h
}

fn initial() -> Vec<(String, f64)> {
    let n_u = 0.02320_f64;
    vec![
        ("U234".to_string(), 0.0),
        ("U235".to_string(), 0.0425 * n_u),
        ("U238".to_string(), 0.9575 * n_u),
    ]
}

/// The simple chain's burnup matrix at the notebook's BOL flux.
fn notebook_matrix() -> (DepletionMatrix, Vec<f64>) {
    let chain = DepletionChain::simple();
    let settings = BurnupSettings::default();
    let names = chain.nuclide_names();
    let xs = one_group_cross_sections(&chain, &settings);
    let mut n = vec![0.0; names.len()];
    for (name, d) in initial() {
        if let Some(i) = chain.index_of(&name) {
            n[i] = d;
        }
    }
    // Seed every nuclide so the matrix acts on a full vector.
    for v in n.iter_mut() {
        if *v == 0.0 {
            *v = 1.0e-9;
        }
    }
    let flux = flux_for_power(&names, &n, &xs, &settings);
    (chain.build_matrix(&reaction_rates(&names, flux, &xs)), n)
}

fn case_cram() -> u64 {
    let mut h = FNV_OFFSET;
    let (a, n) = notebook_matrix();
    for dt in [0.0, 1.0, 3600.0, 86_400.0, 30.0 * 86_400.0, 3.0e8] {
        fnv(&mut h, &cram16(&a, &n, dt));
        fnv(&mut h, &cram48(&a, &n, dt));
    }
    // Stiff 2x2 and a 4-chain, from the module's own tests.
    let mut s = DepletionMatrix::zeros(2);
    s.set(0, 0, -1.0e-2).unwrap();
    s.set(1, 0, 1.0e-2).unwrap();
    s.set(1, 1, -1.0e3).unwrap();
    fnv(&mut h, &cram16(&s, &[1.0, 0.0], 100.0));
    fnv(&mut h, &cram48(&s, &[1.0, 0.0], 100.0));
    let mut c = DepletionMatrix::zeros(4);
    let l = [1.0 / 500.0, 1.0 / 1500.0, 1.0 / 4000.0];
    for i in 0..3 {
        c.set(i, i, -l[i]).unwrap();
        c.set(i + 1, i, l[i]).unwrap();
    }
    fnv(&mut h, &cram16(&c, &[1.0, 0.0, 0.0, 0.0], 2.0e4));
    fnv(&mut h, &cram48(&c, &[1.0, 0.0, 0.0, 0.0], 2.0e4));
    // mat_vec on the notebook matrix. (Since the move `set`, `add` and
    // `mat_vec` return a `Result`; this `.expect` and the `.unwrap()`s above
    // are the only edits made to this file's computations between the
    // pre-move print and the post-move check. They hash the same values.)
    fnv(&mut h, &a.mat_vec(&n).expect("matching length"));
    h
}

fn case_combine() -> u64 {
    let (a, _) = notebook_matrix();
    let mut b = a.clone();
    b.add(0, 0, -1.0e-9).unwrap();
    let m = combine(&[(&a, 0.25), (&b, 1.0 / 6.0), (&a, -1.0 / 12.0)]);
    let mut h = FNV_OFFSET;
    fnv(&mut h, m.as_slice());
    h
}

fn settings_h(step_days: f64, n_steps: usize) -> BurnupSettings {
    BurnupSettings {
        power_watts: 1.0e6,
        fuel_volume_cm3: 1000.0,
        step_days,
        n_steps,
        ..BurnupSettings::default()
    }
}

fn case_predictor_notebook() -> u64 {
    let chain = DepletionChain::simple();
    fold_result(&deplete_predictor(
        &chain,
        &initial(),
        &BurnupSettings::default(),
    ))
}

fn case_with(i: Integrator) -> u64 {
    let chain = DepletionChain::simple();
    fold_result(&deplete_with(i, &chain, &initial(), &settings_h(2.5, 16)))
}

fn case_independent(i: Integrator) -> u64 {
    let chain = DepletionChain::simple();
    let s = settings_h(5.0, 8);
    let micro = MicroXs::from_chain(&chain, &s);
    fold_result(&deplete_independent(&chain, &initial(), &s, &micro, i).expect("independent"))
}

/// The PRE-move CF4 (`Integrator::step`, removed by the move), replayed
/// verbatim on the moved CRAM16 and matrix: the docstring's exponential order,
/// the first stage carried forward unclipped, `combine`d stage matrices.
/// Reproducing the pre-move `with_cf4` pin shows that the CF4 change is the
/// algorithm and nothing else.
fn case_with_cf4_premove_replay() -> u64 {
    use outram_mc_libs::depletion::chain::DepletionChain as C;
    use outram_mc_libs::depletion::operator::{k_inf, BurnupStep};
    let chain = C::simple();
    let settings = settings_h(2.5, 16);
    let names = chain.nuclide_names();
    let xs = one_group_cross_sections(&chain, &settings);
    let mut d = vec![0.0; names.len()];
    for (name, v) in initial() {
        if let Some(i) = chain.index_of(&name) {
            d[i] = v;
        }
    }
    let clamp = |mut v: Vec<f64>| {
        for x in &mut v {
            if *x < 0.0 {
                *x = 0.0;
            }
        }
        v
    };
    let rebuild = |n: &[f64]| {
        let flux = flux_for_power(&names, n, &xs, &settings);
        chain.build_matrix(&reaction_rates(&names, flux, &xs))
    };
    let rec = |step: usize, flux: f64, d: &[f64]| BurnupStep {
        step,
        time_days: step as f64 * settings.step_days,
        flux,
        k_inf: k_inf(d, &xs),
        densities: names
            .iter()
            .zip(d)
            .map(|(n, v)| (n.to_string(), *v))
            .collect(),
    };
    let mut steps = vec![rec(0, 0.0, &d)];
    let dt = settings.step_days * 86_400.0;
    for step in 1..=settings.n_steps {
        let flux0 = flux_for_power(&names, &d, &xs, &settings);
        let a1 = rebuild(&d);
        let n1 = cram16(&a1, &d, 0.5 * dt);
        let a2 = rebuild(&clamp(n1.clone()));
        let n2 = cram16(&a2, &d, 0.5 * dt);
        let a3 = rebuild(&clamp(n2));
        let n3 = cram16(&combine(&[(&a1, -0.5), (&a3, 1.0)]), &n1, dt);
        let a4 = rebuild(&clamp(n3));
        let second = combine(&[
            (&a1, -1.0 / 12.0),
            (&a2, 1.0 / 6.0),
            (&a3, 1.0 / 6.0),
            (&a4, 1.0 / 4.0),
        ]);
        let first = combine(&[
            (&a1, 1.0 / 4.0),
            (&a2, 1.0 / 6.0),
            (&a3, 1.0 / 6.0),
            (&a4, -1.0 / 12.0),
        ]);
        let inner = cram16(&second, &d, dt);
        d = clamp(cram16(&first, &inner, dt));
        steps.push(rec(step, flux0, &d));
    }
    fold_result(&BurnupResult { steps })
}

fn case_coupled_no_spectrum() -> u64 {
    let chain = DepletionChain::simple();
    fold_result(&deplete_coupled(
        &chain,
        &initial(),
        &settings_h(5.0, 8),
        |_| None,
    ))
}

/// `(name, pre-move pin, post-move pin)`. Equal pins mean the case was
/// required to be, and is, bit-identical across the move.
const PINS: &[(&str, u64, u64)] = &[
    ("cram", 0xfa7d4eb002310aa0, 0xfa7d4eb002310aa0),
    ("combine", 0x8356f9616a1ff738, 0x8356f9616a1ff738),
    ("predictor_notebook", 0xdae6ceb68ed967b9, 0xdae6ceb68ed967b9),
    ("with_predictor", 0xbea5e79785e15e2a, 0xbea5e79785e15e2a),
    ("with_cecm", 0xe779fefcb24c2d1a, 0xe779fefcb24c2d1a),
    ("with_cf4", 0x81bdd6ab5450c252, 0x0832d959f308025b),
    (
        "with_cf4_premove_replay",
        0x81bdd6ab5450c252,
        0x81bdd6ab5450c252,
    ),
    (
        "independent_predictor",
        0x4704be5bbd25a7a9,
        0x4704be5bbd25a7a9,
    ),
    ("independent_cecm", 0xf309d7a9bbe2fbcf, 0xf309d7a9bbe2fbcf),
    ("independent_cf4", 0xa186a9d4be6e5777, 0xe99d021de20c7dc3),
    (
        "coupled_no_spectrum",
        0x4704be5bbd25a7a9,
        0x4704be5bbd25a7a9,
    ),
];

fn compute(name: &str) -> u64 {
    match name {
        "cram" => case_cram(),
        "combine" => case_combine(),
        "predictor_notebook" => case_predictor_notebook(),
        "with_predictor" => case_with(Integrator::Predictor),
        "with_cecm" => case_with(Integrator::CeCm),
        "with_cf4" => case_with(Integrator::Cf4),
        "with_cf4_premove_replay" => case_with_cf4_premove_replay(),
        "independent_predictor" => case_independent(Integrator::Predictor),
        "independent_cecm" => case_independent(Integrator::CeCm),
        "independent_cf4" => case_independent(Integrator::Cf4),
        "coupled_no_spectrum" => case_coupled_no_spectrum(),
        other => panic!("unregistered case {other}"),
    }
}

#[test]
fn depletion_math_move_is_bit_identical() {
    let print = std::env::var("DEPL_FP_PRINT").is_ok();
    let mut failures = Vec::new();
    for (name, _pre, post) in PINS {
        let got = compute(name);
        if print {
            println!("(\"{name}\", 0x{got:016x}),");
        }
        if got != *post {
            failures.push(format!("{name}: got 0x{got:016x}, pinned 0x{post:016x}"));
        }
    }
    if !print {
        assert!(
            failures.is_empty(),
            "fingerprints moved:\n{}",
            failures.join("\n")
        );
    }
}
