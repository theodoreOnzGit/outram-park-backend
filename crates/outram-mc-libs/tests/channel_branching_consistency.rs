//! **Collision-channel branching: does the partition the transport samples from
//! add up to the total it decides against? — the second of `op-os8x`'s two
//! remaining leads.**
//!
//! # Why this is the lead
//!
//! `op-os8x` is a spectral residual against OpenMC on Godiva: **+0.42 % too much
//! flux at 1.9–3.0 MeV** and 1.2–1.9 % too little near 100 keV, on identical
//! data, with `k` agreeing. That is a **deficit of down-scatter out of the MeV
//! window**, and the only channel that moves a 2 MeV neutron to ~100 keV in one
//! collision is inelastic.
//!
//! The cross-code study excluded the cross sections themselves (≤0.06 %
//! flux-weighted), both angular laws, the MT=91 transfer table, the within-row
//! CDF inversion and the inter-row unit-base rule. Two leads were left; the
//! CM→lab transform was measured and excluded on 2026-09-16
//! (`tests/cm_to_lab_vs_kinematics.rs`, machine precision against Galilean
//! velocity addition). This measures the other: **which MT a collision is
//! assigned to, as distinct from the cross sections.**
//!
//! # The specific failure mode this looks for
//!
//! The transport kernel makes the choice in **two separate places**:
//!
//! 1. `MicroXS` decides *whether* a collision is elastic, inelastic, (n,2n),
//!    (n,3n), or absorption, by comparing a uniform against those totals.
//! 2. `Nuclide::sample_inelastic` then decides *which* inelastic level, by
//!    comparing a second uniform against the sum of the per-MT reconstructed
//!    cross sections.
//!
//! Nothing structurally forces those two to describe the same partition. If
//! `MicroXS::inelastic` disagrees with the sum `sample_inelastic` normalises by,
//! the kernel enters the inelastic arm with one probability and distributes
//! within it using another — and the net effect is a wrong amount of
//! down-scatter at exactly the energies where the levels open. That is the
//! `op-os8x` signature.
//!
//! This is not hypothetical for this crate: `MicroXS::n3n`'s own documentation
//! records that MT=17 had **no branch at all** before 2026-09-16, so those
//! collisions "fell through to the *elastic* arm and the two extra neutrons were
//! silently lost". The same class, found the same way.
//!
//! # Methodology
//!
//! U-235 and U-238 from `reference-data/endf/`, HIGH tier, across 1.9–3.0 MeV
//! (the band where the residual sits) plus a wider sweep. At each energy:
//!
//! - **Partition closure.** `elastic + inelastic + n2n + n3n + absorption` must
//!   equal `total`. A shortfall means some channel has no arm and is being
//!   silently absorbed by whichever branch is last.
//! - **Branching consistency.** `MicroXS::inelastic` must equal the sum over the
//!   levels `sample_inelastic` partitions.
//! - **Empirical branching.** Drawing from `sample_inelastic` many times must
//!   reproduce the per-MT fractions implied by those same cross sections, which
//!   checks the cumulative walk itself rather than the totals feeding it.
//!
//! # Results (2026-09-16, ENDF/B-VIII.0 U-235 and U-238, HIGH tier)
//!
//! **Branching is exact and this lead is excluded.** `MicroXS::inelastic` equals
//! the sum `sample_inelastic` normalises by to **`0.000e0`** — bit-identical, at
//! every energy on both nuclides — and the sampled continuum share matches the
//! cross sections within **1.23 sigma** over 200 000 draws per point:
//!
//! | E (MeV) | continuum share sampled | from sigma | |
//! |---|---|---|---|
//! | 1.9 | 0.75635 ± 0.00096 | 0.75673 | 0.40 sigma |
//! | 2.2 | 0.80916 ± 0.00088 | 0.80974 | 0.66 sigma |
//! | 2.5 | 0.83759 ± 0.00082 | 0.83844 | 1.04 sigma |
//! | 3.0 | 0.86213 ± 0.00077 | 0.86308 | 1.23 sigma |
//!
//! **So both of `op-os8x`'s named leads are now measured and excluded**, the
//! CM→lab transform on 2026-09-16 at machine precision and channel branching
//! here. The residual is unexplained by anything currently on the list; the next
//! hypothesis has to come from somewhere not yet considered.
//!
//! # A separate defect this found: MT=5 has no branch
//!
//! The partition closes to **1.4e-8** relative through 5 MeV — and then does
//! not:
//!
//! | E | shortfall | of which MT=5 | residual |
//! |---|---|---|---|
//! | 10 MeV | 4.086e-3 b (7.0e-4 rel) | 4.075e-3 b (99.7 %) | 1.0e-5 b |
//! | 14 MeV | 1.545e-2 b (2.7e-3 rel) | 1.465e-2 b (94.8 %) | 8.0e-4 b |
//!
//! `MT=1` exceeds the sum of its own partials by exactly the shortfall, while
//! `MT=4` equals `sum(51..91)` to 1e-8 — so the inelastic decomposition is
//! sound and the missing piece is a channel outside it. It is **MT=5,
//! "(n,anything)"**, which ENDF/B-VIII.0 uses to lump high-energy channels the
//! evaluator did not resolve. The kernel has no arm for it, so those collisions
//! fall through to whichever branch is last.
//!
//! **Flagged and pinned, not fixed.** Sampling MT=5 needs its own emission law
//! (MF=6 for MT=5), which is a separate port rather than a branch. It is zero
//! below ~5 MeV, so it cannot affect any fission-spectrum case here — including
//! `op-os8x`, whose band is 1.9–3.0 MeV where closure is 1.4e-8 — but it does
//! matter to a 14 MeV source, where it is 0.27 % of all collisions. The gate
//! below pins it so it cannot grow unnoticed, and so closing it shows up as a
//! test that must be updated.
//!
//! # What a pass means for `op-os8x`
//!
//! Both named leads would then be measured and excluded, and the residual would
//! be unexplained by anything on the current list — which is a more useful state
//! than two open suspicions, and says the next hypothesis has to come from
//! somewhere not yet considered.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::{Inelastic, Nuclide};

const TEMP_K: f64 = 293.6;

fn load(file: &str, name: &str) -> Option<Nuclide> {
    let tape = reference_file_or_skip("endf", file, "channel branching")?;
    Nuclide::from_endf_file(&tape, name, TEMP_K, 1.0e-3).ok()
}

#[test]
fn the_collision_partition_closes_and_matches_what_is_sampled() {
    let cases = [
        ("n-092_U_238-ENDF8.0.endf", "U238"),
        ("n-092_U_235-ENDF8.0.endf", "U235"),
    ];
    // The op-os8x band, plus a sweep either side of it.
    let energies = [
        1.0e5_f64, 5.0e5, 1.0e6, 1.9e6, 2.2e6, 2.5e6, 3.0e6, 5.0e6, 1.0e7, 1.4e7,
    ];

    let mut ran = false;
    let mut worst_closure = 0.0f64;
    let mut worst_closure_high = 0.0f64;
    let mut worst_branch = 0.0f64;
    let mut worst_partition_mismatch = 0.0f64;

    for (file, name) in cases {
        let Some(nuc) = load(file, name) else {
            continue;
        };
        ran = true;
        println!("\n{name}: collision partition against sigma_total");
        println!(
            "{:>10} {:>12} {:>12} {:>12} {:>10}",
            "E (eV)", "sum parts", "total", "shortfall", "rel"
        );
        for &e in &energies {
            let x = nuc.xs_at_energy(e, TEMP_K);
            let parts = x.elastic + x.inelastic + x.n2n + x.n3n + x.absorption;
            let shortfall = x.total - parts;
            let rel = shortfall.abs() / x.total.max(1.0e-30);
            // Below 6 MeV the kernel's partition is claimed complete, and that
            // is what is gated. Above it, MT=5 opens -- see the module docs --
            // and the shortfall is pinned separately rather than gated to zero.
            if e <= 6.0e6 {
                worst_closure = worst_closure.max(rel);
            } else {
                worst_closure_high = worst_closure_high.max(rel);
            }
            println!(
                "{e:10.3e} {parts:12.6} {:12.6} {shortfall:12.3e} {rel:10.2e}",
                x.total
            );
            // Name the shortfall where there is one, rather than reporting that
            // some barns are missing: every neutron-producing channel that is
            // inside MT=1 but is not elastic, MT=51..91, MT=16/17, or a
            // disappearance partial.
            if rel > 1.0e-6 {
                let mut named = 0.0f64;
                let mut parts_list = Vec::new();
                for mt in [22i32, 28, 32, 33, 37, 41, 44, 45] {
                    let v = nuc.reaction_xs(mt, e).unwrap_or(0.0);
                    if v > 0.0 {
                        named += v;
                        parts_list.push(format!("MT={mt}:{v:.4e}"));
                    }
                }
                // Nothing in the named list? Then test the evaluation's own
                // redundancy directly: ENDF requires MT=1 to equal the sum of
                // its partials, so compare the reconstructed MT=1 against the
                // reconstructed MT=2 + MT=4 + MT=16 + MT=17 + MT=18 + MT=102.
                let mt5 = nuc.reaction_xs(5, e).unwrap_or(0.0);
                let redundancy: f64 = [2i32, 4, 16, 17, 18, 102]
                    .iter()
                    .map(|&mt| nuc.reaction_xs(mt, e).unwrap_or(0.0))
                    .sum();
                let mt1 = nuc.reaction_xs(1, e).unwrap_or(0.0);
                let mt4 = nuc.reaction_xs(4, e).unwrap_or(0.0);
                println!(
                    "            unaccounted {shortfall:.4e} b; named neutron-producing \
                     channels sum to {named:.4e} b [{}] -> residual {:.3e} b\n\
                     \x20           MT=1 {mt1:.6} vs sum(2,4,16,17,18,102) {redundancy:.6} \
                     (diff {:.4e});  MT=4 {mt4:.6} vs sum(51..91) {:.6} (diff {:.4e});  \
                     MT=5 = {mt5:.6e} b  -> residual after MT=5: {:.4e} b",
                    parts_list.join(" "),
                    shortfall - named,
                    mt1 - redundancy,
                    x.inelastic,
                    mt4 - x.inelastic,
                    shortfall - mt5
                );
            }
        }

        // `MicroXS::inelastic` against the sum the level sampler normalises by,
        // measured empirically: draw many channels and count how often the
        // sampler says "continuum" vs "a discrete level". The fractions must
        // match the cross sections that MicroXS reports.
        println!("{name}: inelastic branching, sampled vs cross sections");
        for &e in &[1.9e6_f64, 2.2e6, 2.5e6, 3.0e6] {
            let x = nuc.xs_at_energy(e, TEMP_K);
            if x.inelastic <= 0.0 {
                continue;
            }
            const N: usize = 200_000;
            let mut seed = 0xBEEF_F00Du64;
            let mut n_cont = 0usize;
            for _ in 0..N {
                if matches!(
                    nuc.sample_inelastic(e, &mut seed),
                    Inelastic::Continuum { .. }
                ) {
                    n_cont += 1;
                }
            }
            let f_cont = n_cont as f64 / N as f64;
            // The same quantity from the data: MT=91 over total inelastic.
            let sigma_91 = nuc.inelastic_channel_xs(91, e).unwrap_or(0.0);
            // Normalise by the sum the SAMPLER uses, so this measures the
            // cumulative walk. The sampler's total against MicroXS::inelastic is
            // the separate assertion below -- conflating the two would let one
            // error hide the other.
            let sampler_total = nuc.inelastic_channel_total(e).unwrap_or(0.0);
            let expect = if sampler_total > 0.0 {
                sigma_91 / sampler_total
            } else {
                0.0
            };
            let micro_vs_sampler = (x.inelastic - sampler_total).abs() / x.inelastic.max(1e-30);
            worst_partition_mismatch = worst_partition_mismatch.max(micro_vs_sampler);
            let sem = (f_cont * (1.0 - f_cont) / N as f64).sqrt().max(1.0e-12);
            let z = (f_cont - expect).abs() / sem;
            worst_branch = worst_branch.max(z);
            println!(
                "   E = {e:9.3e} eV: continuum share sampled {f_cont:.5} +- {sem:.5}, from \
                 sigma {expect:.5}  ({z:.2} sigma);  MicroXS::inelastic vs sampler total: \
                 {micro_vs_sampler:.3e} relative"
            );
        }
    }

    if !ran {
        eprintln!("SKIP: no evaluations available");
        return;
    }

    println!("\nworst partition shortfall below 6 MeV = {worst_closure:.3e} relative");
    println!("worst partition shortfall above 6 MeV = {worst_closure_high:.3e} relative (MT=5)");
    println!("worst MicroXS::inelastic vs sampler total = {worst_partition_mismatch:.3e} relative");
    println!("worst branching deviation = {worst_branch:.2} sigma");

    assert!(
        worst_closure < 1.0e-6,
        "the collision partition does not close BELOW 6 MeV: elastic + inelastic + (n,2n) + \
         (n,3n) + absorption differs from sigma_total by {worst_closure:.3e} relative. Whatever \
         is missing is silently taken by whichever branch the kernel falls through to -- which \
         is exactly how MT=17 was lost into the elastic arm before 2026-09-16. This band \
         contains op-os8x's missing down-scatter, so a failure here would be a direct \
         candidate for it."
    );
    // MT=5 is PINNED, not gated to zero: it is a real gap, measured, and above
    // the band that matters for a fission spectrum. Pinning stops it growing
    // unnoticed and makes closing it show up as a test to update.
    assert!(
        worst_closure_high > 1.0e-5 && worst_closure_high < 5.0e-3,
        "the above-6 MeV shortfall is {worst_closure_high:.3e} relative, outside the measured \
         2.7e-3 band. If it fell to zero, MT=5 has been given a branch -- say so and tighten \
         this. If it grew, another channel has been lost as well."
    );
    assert!(
        worst_partition_mismatch < 1.0e-9,
        "MicroXS::inelastic and the sum sample_inelastic normalises by differ by \
         {worst_partition_mismatch:.3e} relative. The kernel would then enter the inelastic arm \
         with one probability and distribute within it using another -- the exact shape of \
         op-os8x's missing down-scatter at 1.9-3.0 MeV."
    );
    assert!(
        worst_branch < 4.0,
        "the sampled inelastic branching disagrees with the cross sections it should follow by \
         {worst_branch:.2} sigma. The kernel would then enter the inelastic arm with one \
         probability and distribute within it using another, putting the wrong amount of \
         down-scatter at the energies where the levels open."
    );
}
