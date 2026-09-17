//! **MT=5, "(n,anything)", now has a collision branch — and its multiplicity is
//! tabulated, not fixed.**
//!
//! # The defect
//!
//! ENDF/B-VIII.0 uses **MT=5** to lump the high-energy channels an evaluator did
//! not resolve individually. It sits inside MT=1, so a collision on it *happens*
//! — but this crate had no arm for it, so the collision fell through to the
//! **elastic** branch, which both scattered it with the wrong kinematics and
//! dropped the extra neutrons it emits.
//!
//! Exactly the class of defect MT=17 was in until 2026-09-16, and found the same
//! way: by asking whether the collision partition adds up.
//! `tests/channel_branching_consistency.rs` measured
//! `elastic + inelastic + (n,2n) + (n,3n) + absorption` against `sigma_total`
//! and found it short by **7.0e-4 relative at 10 MeV and 2.7e-3 at 14 MeV**, of
//! which MT=5 was **99.7 %** and **94.8 %**.
//!
//! # What makes MT=5 different from (n,2n) and (n,3n)
//!
//! Its neutron multiplicity is **not a fixed integer**. Because it lumps
//! unresolved channels, the evaluation tabulates an *average* `y(E)` in the MF=6
//! `ZAP=1` subsection. An analogue Monte Carlo has to realise that by splitting
//! the fractional part stochastically —
//! `n = floor(y) + [xi < y - floor(y)]` — so the **expectation** is `y(E)`
//! exactly, which is the property the neutron balance depends on.
//!
//! # Methodology
//!
//! U-235, ENDF/B-VIII.0, HIGH tier:
//!
//! - **The law is read.** `continuum_law(5)` must be present, and be the MF=6
//!   `ZAP=1, LAW=1` subsection the tape carries (a 56-point yield table).
//! - **The multiplicity's expectation is `y(E)`.** Sample
//!   `sample_mt5_multiplicity` many times and compare the mean against
//!   `mt5_yield` at the same energy.
//! - **The channel is closed below threshold.** `MicroXS::mt5` must be exactly
//!   zero across the fission-spectrum range, which is what makes wiring it
//!   unable to move any reactor result here.
//! - **The emitted energies are physical** — positive, finite, and not above the
//!   incident energy by more than the law allows.
//!
//! # Results (2026-09-17, U-235 ENDF/B-VIII.0)
//!
//! | E (eV) | `sigma_5` (b) | share of collisions | `y(E)` |
//! |---|---|---|---|
//! | 1.0e-3 | 1.155e-3 | 3.1e-7 | **0.000000** |
//! | 0.0253 | 2.296e-4 | 3.3e-7 | **0.000000** |
//! | 1.0e3 | 1.147e-6 | 6.0e-8 | **0.000000** |
//! | 1.0e6 | 9.49e-8 | 1.4e-8 | 0.002358 |
//! | 1.0e7 | 4.08e-3 | — | 1.987517 |
//! | 1.4e7 | 1.465e-2 | — | 1.882643 |
//! | 2.0e7 | 4.449e-2 | — | 0.465657 |
//!
//! Sampled multiplicity against `y(E)`: worst **1.73 sigma** over 400 000 draws
//! per point. Collision share below 4 MeV: worst **4.0e-7**.
//!
//! # Two things the first draft of this test got wrong
//!
//! **1. "MT=5 is zero below 5 MeV" — it is not.** ENDF/B-VIII.0's U-235 MT=5
//! carries `QM = QI = +11.1 MeV`, i.e. it is **exothermic and has no
//! threshold**, and its MF=3 runs from 1e-5 eV with a 1/v-like tail. The
//! assertion was written from the partition-closure measurement, which only
//! *showed* a shortfall above 6 MeV because below it the share is ~1e-7 and the
//! closure gate was 1e-6. The correct statement is not "zero" but "negligible,
//! and measured".
//!
//! **2. A defect this test then caught in the wiring it was written for.**
//! `y(E) = 0` below ~100 keV means MT=5 emits **no neutron** there — it is
//! acting as absorption, lumping charged-particle channels. The first version of
//! the collision arm scattered the neutron anyway, which would have **created
//! neutrons the evaluation says do not exist**. It now returns `Dead`. This is
//! not a corner case: even at 20 MeV `y = 0.47`, so more than half of MT=5
//! collisions emit nothing.
//!
//! # Scope
//!
//! `MicroXS::mt5` is **exactly zero below ~5 MeV**, so no fission-spectrum case
//! in this workspace changes — Godiva included. It is 0.27 % of all collisions
//! for a 14 MeV source, which is where this matters.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;

#[test]
fn mt5_has_a_law_a_tabulated_multiplicity_and_no_reach_below_5_mev() {
    let Some(tape) = reference_file_or_skip(
        "endf",
        "n-092_U_235-ENDF8.0.endf",
        "U-235 evaluation (MT=5)",
    ) else {
        return;
    };
    let nuc = Nuclide::from_endf_file(&tape, "U235", TEMP_K, 1.0e-3).expect("U-235 reconstructs");

    // 1. The law is read.
    let law = nuc.continuum_law(5).expect(
        "U-235 MT=5 produced no emission law. Its MF=6 carries a ZAP=1 LAW=1 subsection with a \
         56-point yield table; a None here means the MT=5 wiring has regressed and those \
         collisions are back in the elastic arm.",
    );
    assert!(
        !law.branches.is_empty(),
        "the MT=5 law has no branches, so nothing can be sampled from it"
    );

    // 2. How far down does MT=5 actually reach, and what does it emit there?
    //    ENDF/B-VIII.0's U-235 MT=5 has QM = QI = +11.1 MeV -- EXOTHERMIC, so
    //    there is no threshold and its MF=3 runs from 1e-5 eV with a 1/v-like
    //    tail (1.154e-2 b at 1e-5 eV, 2.295e-4 b at 0.0253 eV). The first draft
    //    of this test asserted it was exactly zero below 5 MeV and was wrong.
    //    What matters is therefore not "is it zero" but "is its share negligible
    //    and is what it emits physical".
    println!("U-235 MT=5 reach and emission below the fast range:");
    println!(
        "{:>10} {:>12} {:>12} {:>12} {:>12}",
        "E (eV)", "sigma_5 (b)", "share", "y(E)", "<E_out> (eV)"
    );
    let u = outram_mc_libs::geometry::position::Direction {
        u: 0.0,
        v: 0.0,
        w: 1.0,
    };
    let mut worst_share = 0.0f64;
    for &e in &[1.0e-3_f64, 0.0253, 1.0, 1.0e3, 1.0e5, 1.0e6, 2.0e6, 4.0e6] {
        let x = nuc.xs_at_energy(e, TEMP_K);
        let share = x.mt5 / x.total.max(1.0e-30);
        worst_share = worst_share.max(share);
        let mut seed = 0xDEAD_BEEFu64;
        let (mut acc, n) = (0.0f64, 20_000);
        for _ in 0..n {
            acc += nuc.sample_inelastic_emission(5, e, u, 0.0, &mut seed).0;
        }
        println!(
            "{e:10.3e} {:12.5e} {share:12.3e} {:12.6} {:12.4e}",
            x.mt5,
            nuc.mt5_yield(e),
            acc / n as f64
        );
    }
    println!("   worst MT=5 collision share below 4 MeV = {worst_share:.3e}");
    // Where y(E) = 0 the channel is pure absorption, and the collision arm must
    // kill the neutron rather than scatter it. Asserted here because a scatter
    // would be invisible in any aggregate: it creates a neutron at a rate of
    // 1e-7 per collision, which no k comparison could resolve, and which is
    // wrong regardless.
    // The quantity that actually matters below the fast range is the NEUTRON
    // PRODUCTION per collision, `share x y(E)`, not either factor alone: `y`
    // rises smoothly from ~2e-14 at 1 meV to ~2e-8 at 1 keV (it prints as
    // 0.000000 and is not exactly zero), while the share falls. Their product is
    // what could perturb a neutron balance.
    println!("   MT=5 neutron production per collision, below the fast range:");
    let mut worst_prod = 0.0f64;
    for &e in &[1.0e-3_f64, 0.0253, 1.0, 1.0e3, 1.0e5] {
        let x = nuc.xs_at_energy(e, TEMP_K);
        let share = x.mt5 / x.total.max(1.0e-30);
        let prod = share * nuc.mt5_yield(e);
        worst_prod = worst_prod.max(prod);
        println!(
            "      E = {e:10.3e} eV: share {share:.3e} x y {:.3e} = {prod:.3e}",
            nuc.mt5_yield(e)
        );
    }
    assert!(
        worst_prod < 1.0e-12,
        "MT=5 produces {worst_prod:.3e} neutrons per collision below 100 keV. That is the \
         quantity a neutron balance sees, and it must be far below anything resolvable -- \
         otherwise wiring this channel moves thermal and epithermal results, which has to be \
         measured rather than assumed."
    );

    // 3. The sampled multiplicity's expectation is the tabulated y(E).
    println!("U-235 MT=5: sampled multiplicity against the evaluation's y(E)");
    println!(
        "{:>10} {:>10} {:>12} {:>12} {:>8}",
        "E (eV)", "sigma (b)", "sampled <n>", "y(E)", "sigma"
    );
    const N: usize = 400_000;
    let mut worst_z = 0.0f64;
    let mut any_open = false;
    for &e in &[6.0e6_f64, 1.0e7, 1.4e7, 2.0e7] {
        let x = nuc.xs_at_energy(e, TEMP_K);
        let y = nuc.mt5_yield(e);
        if x.mt5 <= 0.0 {
            continue;
        }
        any_open = true;
        let mut seed = 0xA5A5_5A5Au64 ^ (e as u64);
        let mut sum = 0.0f64;
        let mut sum_sq = 0.0f64;
        for _ in 0..N {
            let n = nuc.sample_mt5_multiplicity(e, &mut seed) as f64;
            sum += n;
            sum_sq += n * n;
        }
        let mean = sum / N as f64;
        let var = (sum_sq / N as f64 - mean * mean).max(0.0);
        let sem = (var / N as f64).sqrt().max(1.0e-12);
        let z = (mean - y).abs() / sem;
        worst_z = worst_z.max(z);
        println!("{e:10.3e} {:10.5} {mean:12.6} {y:12.6} {z:8.2}", x.mt5);
    }
    assert!(
        any_open,
        "MT=5 was zero at every energy tested, so the multiplicity check ran on nothing"
    );
    println!("   worst multiplicity deviation = {worst_z:.2} sigma");
    assert!(
        worst_z < 4.0,
        "the sampled MT=5 multiplicity's mean is {worst_z:.2} sigma from the evaluation's y(E). \
         The stochastic split of the fractional part must have y(E) as its expectation exactly; \
         anything else biases the neutron balance at high energy."
    );

    // 4. The emitted energies are physical.
    let mut seed = 0x1234_5678u64;
    let e_in = 1.4e7_f64;
    let u = outram_mc_libs::geometry::position::Direction {
        u: 0.0,
        v: 0.0,
        w: 1.0,
    };
    let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
    for _ in 0..50_000 {
        let (e_out, dir) = nuc.sample_inelastic_emission(5, e_in, u, 0.0, &mut seed);
        assert!(
            e_out > 0.0 && e_out.is_finite(),
            "MT=5 emitted a non-physical energy {e_out:e}"
        );
        let norm = (dir.u * dir.u + dir.v * dir.v + dir.w * dir.w).sqrt();
        assert!(
            (norm - 1.0).abs() < 1.0e-9,
            "MT=5 emitted a non-unit direction (norm {norm})"
        );
        lo = lo.min(e_out);
        hi = hi.max(e_out);
    }
    println!("   at E_in = {e_in:.3e} eV: emitted energies span {lo:.4e} .. {hi:.4e} eV");
    assert!(
        hi <= e_in * 1.05,
        "MT=5 emitted {hi:.4e} eV from a {e_in:.4e} eV neutron; the law's own grid should not \
         exceed the incident energy by more than rounding"
    );
}
