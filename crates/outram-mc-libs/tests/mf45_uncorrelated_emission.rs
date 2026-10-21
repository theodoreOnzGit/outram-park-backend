//! **MF=4 + MF=5 emission laws reach the sampler — the pre-ENDF-6 form of a
//! continuum reaction, measured against the evaluation's own tables.**
//!
//! # What this closes
//!
//! An evaluation written before ENDF-6 stores a continuum or multiplying
//! reaction as an outgoing-energy law in **MF=5** and an emission cosine in
//! **MF=4**, uncorrelated, rather than as a correlated MF=6 law. A coverage
//! survey on 2026-09-16 (`njoy-outram-park-fork`'s
//! `tests/continuum_law_coverage_survey.rs`) found **11 such sections** in
//! `reference-data/endf/` — Li-7 MT=16, C-12 MT=91, Sr-88 MT=16/17/91 and the
//! JENDL-3.3 U-238 and Pu-239 MT=16/17/91 — where the evaluation supplies a
//! complete law and transport substituted a **Weisskopf evaporation shape**
//! anyway.
//!
//! The survey was itself the correction: those 11 were first counted as benign,
//! "the evaluation carries no MF=6 so the stand-in is all there is". Checking
//! rather than assuming showed every one of them carries MF=4 and MF=5.
//!
//! # What was actually missing
//!
//! Only the reading. `sample_chi` already samples every ported MF=5 `LF`
//! including the analytic ones, and `sample_mf4_mu_cm` already implements
//! OpenMC's statistical-neighbour convention on MF=4's own incident grid. So
//! this wires the two together (`Nuclide::sample_inelastic_emission`) rather
//! than converting the law into `ChiTabular` — converting would have replaced
//! two exact samplers with one tabulated approximation and forced MF=4 onto
//! MF=5's unrelated energy grid.
//!
//! # Methodology
//!
//! JENDL-3.3 U-238 MT=91 exercises both halves non-trivially: MF=5 `LF=1`
//! (tabulated) and MF=4 `LTT=1` (Legendre), `LCT=1` (laboratory). Sample many
//! collisions at an incident energy taken **exactly from the MF=5 grid**, so the
//! sampler's incident interpolation factor is zero and the outgoing distribution
//! is exactly that tabulated row, then compare:
//!
//! - `<E'>` against the row's own first moment, integrated in **closed form on
//!   each linear segment** (trapezoid is exact for `int f` but not for
//!   `int E' f`, which is quadratic);
//! - `<mu>` against MF=4's `<mu>` at that energy. The statistical neighbour pick
//!   has the linear interpolation of the two bracketing tables' means as its
//!   *expectation*, so the oracle is `(1-r)*mubar_i + r*mubar_(i+1)` — reading
//!   the nearest table instead is the trap this crate's V&V record hit three
//!   times.
//!
//! # Results (2026-09-16, JENDL-3.3 U-238 MT=91, 200 000 collisions)
//!
//! Printed by the test.
//!
//! # What this does NOT claim
//!
//! Nothing here validates JENDL-3.3, and none of the 11 affected sections
//! appears in this workspace's criticality cases — Godiva and the thermal cases
//! run on ENDF/B-VIII.0 evaluations that all carry MF=6. **No reported `k`
//! changes.** What this closes is a silent substitution over data that was
//! present.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, UncorrelatedEmission};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use outram_mc_libs::material::nuclide::sample_uncorrelated_emission;

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn sem(v: &[f64]) -> f64 {
    let m = mean(v);
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() as f64 - 1.0);
    (var / v.len() as f64).sqrt()
}

/// `int f dx` over a piecewise-linear table (trapezoid — exact for `f`).
fn integral(x: &[f64], f: &[f64]) -> f64 {
    (1..x.len().min(f.len()))
        .map(|i| 0.5 * (f[i] + f[i - 1]) * (x[i] - x[i - 1]))
        .sum()
}

/// `int x f(x) dx` in closed form — `x f(x)` is quadratic on each segment, so
/// the trapezoid rule is NOT exact for it.
fn first_moment(x: &[f64], f: &[f64]) -> f64 {
    (1..x.len().min(f.len()))
        .map(|i| {
            let (a, b) = (x[i - 1], x[i]);
            let (fa, fb) = (f[i - 1], f[i]);
            (b - a) * (a * (2.0 * fa + fb) + b * (fa + 2.0 * fb)) / 6.0
        })
        .sum()
}

#[test]
fn jendl_u238_mt91_samples_its_mf45_law_not_the_stand_in() {
    let Some(p) = reference_endf_or_skip("n-092_U_238-JENDL3.3.endf", "U-238 JENDL-3.3 (MF=4/5)")
    else {
        return;
    };
    let tape = Tape::read_file(&p).expect("JENDL-3.3 U-238 tape parses");
    let mat = tape.materials()[0];

    let law = UncorrelatedEmission::from_endf(&tape, mat, 91)
        .expect("MF=4/5 read does not fail")
        .expect(
            "JENDL-3.3 U-238 MT=91 produced no uncorrelated law. It carries MF=5 LF=1 and \
             MF=4 LTT=1; a None here means the MF=4/5 path has regressed and the Weisskopf \
             stand-in is back.",
        );
    assert_eq!(
        law.lct, 1,
        "this section is laboratory-frame (LCT=1); a CM law here would need a frame transform \
         that `from_endf` deliberately refuses to guess at"
    );
    assert!(
        law.is_anisotropic(),
        "U-238 MT=91's MF=4 is LTT=1 (Legendre) and should carry real angular structure"
    );

    // The MF=5 side: pick an incident energy exactly on its grid, so the
    // sampler's interpolation factor is zero and the outgoing distribution is
    // exactly that tabulated row.
    let FissionSpectrum::ContinuousTabular(chi) = &law.energy else {
        panic!(
            "expected MF=5 LF=1 (ContinuousTabular), got {:?}",
            law.energy
        );
    };
    let idx = chi.incident.len() * 3 / 5;
    let e_in = chi.incident[idx];
    let row = &chi.tables[idx];
    let expect_e = first_moment(&row.e_out, &row.pdf) / integral(&row.e_out, &row.pdf);

    // The MF=4 side: the statistical neighbour pick's EXPECTATION is the linear
    // interpolation of the bracketing tables' means.
    // Computed here independently of the library, in closed form. Until
    // 2026-09-16 `EnergyAngular::mean_cosine` used the trapezoid rule on
    // `mu*f(mu)` -- which is quadratic, so trapezoid is not exact -- and this
    // comparison is what found it: the sample sat 3.20 sigma from the library's
    // value and 0.72 sigma from this one. The library is fixed; recomputing here
    // rather than calling it keeps this an independent check rather than a
    // restatement.
    let mubar_exact = |ea: &njoy_outram_park_fork::acer::angular::EnergyAngular| -> f64 {
        first_moment(&ea.cosines, &ea.pdf)
    };
    let d = &law.angular.energies;
    let e_mev = e_in * 1.0e-6;
    let expect_mu = if d.is_empty() {
        0.0
    } else if e_mev <= d[0].e_mev {
        mubar_exact(&d[0])
    } else if e_mev >= d[d.len() - 1].e_mev {
        mubar_exact(&d[d.len() - 1])
    } else {
        let mut i = 0;
        while i + 1 < d.len() && d[i + 1].e_mev <= e_mev {
            i += 1;
        }
        let (e0, e1) = (d[i].e_mev, d[i + 1].e_mev);
        let r = if e1 > e0 {
            (e_mev - e0) / (e1 - e0)
        } else {
            0.0
        };
        (1.0 - r) * mubar_exact(&d[i]) + r * mubar_exact(&d[i + 1])
    };

    const N: usize = 200_000;
    let mut s = 0xC0FFEEu64;
    let mut es = Vec::with_capacity(N);
    let mut mus = Vec::with_capacity(N);
    for _ in 0..N {
        let (e_out, mu) = sample_uncorrelated_emission(&law, e_in, &mut s);
        es.push(e_out);
        mus.push(mu);
    }
    let (got_e, got_e_sem) = (mean(&es), sem(&es));
    let (got_mu, got_mu_sem) = (mean(&mus), sem(&mus));

    println!(
        "JENDL-3.3 U-238 MT=91 (MF=5 LF=1 + MF=4 LTT=1) at E_in = {:.6} MeV, {N} collisions:",
        e_in / 1.0e6
    );
    println!(
        "   <E'>: sampled {:.6e} eV +- {:.3e}, evaluation {:.6e} eV  ({:.2} sigma)",
        got_e,
        got_e_sem,
        expect_e,
        (got_e - expect_e).abs() / got_e_sem
    );
    // What the library says now, via `EnergyAngular::mean_cosine`. Before the
    // 2026-09-16 fix this printed +0.342991 against the exact +0.339994; it
    // should now agree with the independent closed form above to round-off.
    let expect_mu_lib = {
        let mut i = 0;
        while i + 1 < d.len() && d[i + 1].e_mev <= e_mev {
            i += 1;
        }
        let (e0, e1) = (d[i].e_mev, d[i + 1].e_mev);
        let r = if e1 > e0 {
            (e_mev - e0) / (e1 - e0)
        } else {
            0.0
        };
        (1.0 - r) * d[i].mean_cosine() + r * d[i + 1].mean_cosine()
    };
    println!(
        "   <mu> via the library's mean_cosine = {expect_mu_lib:+.6} ({:.2} sigma from the \
         sample)",
        (got_mu - expect_mu_lib).abs() / got_mu_sem
    );
    assert!(
        (expect_mu_lib - expect_mu).abs() < 1.0e-12,
        "EnergyAngular::mean_cosine gives {expect_mu_lib:+.9}, the independent closed form \
         {expect_mu:+.9}. They integrate the same piecewise-linear density and must agree to \
         round-off; a gap means mean_cosine has gone back to the trapezoid rule, which is not \
         exact for the quadratic integrand mu*f(mu)."
    );
    println!(
        "   <mu>: sampled {:+.6} +- {:.6}, evaluation {:+.6}  ({:.2} sigma)",
        got_mu,
        got_mu_sem,
        expect_mu,
        (got_mu - expect_mu).abs() / got_mu_sem
    );

    assert!(
        (got_e - expect_e).abs() < 4.0 * got_e_sem,
        "sampled <E'> = {got_e:.6e} +- {got_e_sem:.3e} eV is not the evaluation's \
         {expect_e:.6e} eV. The MF=5 law is not being drawn from."
    );
    assert!(
        (got_mu - expect_mu).abs() < 4.0 * got_mu_sem,
        "sampled <mu> = {got_mu:+.6} +- {got_mu_sem:.6} is not MF=4's {expect_mu:+.6}. Either \
         the angular law is not reaching the sampler, or the incident-energy lookup reads the \
         nearest table instead of picking a neighbour statistically."
    );
    assert!(
        expect_mu.abs() > 1.0e-3,
        "MF=4's <mu> at this energy is {expect_mu:+.6}, effectively isotropic, so the cosine \
         comparison above proves nothing. Pick an incident energy where the law has structure."
    );
}

/// **C-12 MT=91 is the `LF=9` (evaporation) case, and is declared isotropic.**
///
/// It is the one section of the eleven that does not use `LF=1`, and its MF=4 is
/// `LTT=0` with `LI=1` — the evaluation stating isotropic emission, which is a
/// fact about the physics rather than a gap in this port. Both facts are worth a
/// gate: the analytic `LF=9` sampler is a different code path from the tabulated
/// one, and an "isotropic" that silently came from a failed parse would look
/// identical to one the evaluator wrote.
#[test]
fn c12_mt91_uses_the_analytic_lf9_law_and_is_evaluated_isotropic() {
    let Some(p) = reference_endf_or_skip("n-006_C_012-ENDF8.0.endf", "C-12 (MF=5 LF=9)") else {
        return;
    };
    let tape = Tape::read_file(&p).expect("C-12 tape parses");
    let mat = tape.materials()[0];

    let law = UncorrelatedEmission::from_endf(&tape, mat, 91)
        .expect("MF=4/5 read does not fail")
        .expect("C-12 MT=91 carries MF=5 LF=9 and must produce an uncorrelated law");

    assert!(
        matches!(law.energy, FissionSpectrum::Evaporation { .. }),
        "C-12 MT=91's MF=5 is LF=9 (evaporation with a tabulated theta(E)); got {:?}",
        law.energy
    );
    assert!(
        !law.is_anisotropic(),
        "C-12 MT=91's MF=4 is LTT=0 / LI=1 -- the EVALUATION declares isotropic emission"
    );
    assert_eq!(law.yield_n, 1, "MT=91 emits one neutron");

    // The analytic sampler must produce energies inside the law's own bound,
    // E' <= E - U. A draw above it would mean theta(E) or U is being read wrong.
    let FissionSpectrum::Evaporation { u, .. } = &law.energy else {
        unreachable!()
    };
    let e_in = 1.4e7_f64;
    let cap = e_in - u;
    let mut s = 0xBEEF_u64;
    let mut worst = 0.0f64;
    let mut es = Vec::with_capacity(20_000);
    for _ in 0..20_000 {
        let (e_out, mu) = sample_uncorrelated_emission(&law, e_in, &mut s);
        assert!(
            (-1.0..=1.0).contains(&mu),
            "cosine {mu} outside [-1, 1] from an isotropic law"
        );
        worst = worst.max(e_out);
        es.push(e_out);
    }
    println!(
        "C-12 MT=91 (MF=5 LF=9 evaporation, U = {:.4e} eV) at E_in = {:.1} MeV: <E'> = {:.4e} eV, \
         max E' = {:.4e} eV, bound E-U = {:.4e} eV",
        u,
        e_in / 1.0e6,
        mean(&es),
        worst,
        cap
    );
    assert!(
        worst <= cap * (1.0 + 1.0e-9),
        "sampled E' = {worst:.6e} eV exceeds the evaporation law's own bound E - U = \
         {cap:.6e} eV, so theta(E) or U is being read wrong"
    );
    assert!(
        mean(&es) > 0.0,
        "the LF=9 sampler returned an all-zero spectrum"
    );
}
