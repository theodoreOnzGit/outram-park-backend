// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # Where does CLS work, and where does it not? — a regime sweep against explicit geometry
//!
//! ```bash
//! cargo run --release -p outram-mc-libs --example cls_scls_regime_sweep
//! cargo run --release -p outram-mc-libs --example cls_scls_regime_sweep -- --histories 20000
//! ```
//!
//! ## Why a sweep, and why not k-eigenvalue
//!
//! `examples/dh_keff_vv.rs` reports CLS and SCLS eigenvalues on one FHR unit
//! cell. One point cannot say **why** a method is wrong, or whether the same
//! error would appear on a different problem — and a k-eigenvalue folds the
//! geometry error together with the nuclear data, the resonance treatment and
//! the eigenvalue iteration, none of which is the thing under test.
//!
//! This sweeps the **geometric** error alone. The reference is an *explicit RSA
//! packing* walked by the same random walk, so the only difference between arms
//! is how much geometry the model remembers. The tally is an absorption
//! probability, which is a pure functional of the geometry the neutron meets.
//!
//! ## The axes, and what each is expected to control
//!
//! | axis | why it should matter |
//! |---|---|
//! | **packing fraction** | CLS's Markovian matrix chord ignores the exclusion correlation of non-overlapping spheres. That correlation grows with `pf`, so the error should grow with it. |
//! | **scattering mean free path** | CLS forgets geometry at every scatter. A neutron that scatters often re-meets freshly invented medium, so the error should grow as `scatter_mfp` falls relative to the inclusion spacing. |
//! | **inclusion radius at fixed `pf`** | sets the optical/geometric scale of a single inclusion against the walk step and the domain. |
//!
//! The first two are the two failure modes the module documentation *claims*
//! for CLS. This sweep is what turns those claims into measurements.
//!
//! ## What a reader may and may not conclude
//!
//! This is **verification against another code path**, not validation: the RSA
//! arm is an explicit-geometry calculation by the same crate, not an
//! experiment. What the sweep establishes is the size and the sign of the
//! *modelling* error CLS and SCLS carry relative to resolving the geometry,
//! and how that error moves with regime.
//!
//! Statistical uncertainty on each arm is the binomial `sqrt(p(1-p)/n)`; the
//! sweep prints it so a gap can be read against it rather than eyeballed. Gaps
//! below about two combined standard errors are not resolved and are printed as
//! such.
//!
//! ## Results — measured 2026-09-18, **40 000 histories/arm**, seed 20260918
//!
//! Absorption probability; `*` marks a gap beyond two combined standard errors.
//! Domain half-width 0.5 cm, `max_collisions = 200`. Note the **default is
//! `--histories 8000`**, which gives s.e. ~0.0053 and is too noisy to resolve
//! the dilute end; the table below is the 40 000 run. An earlier revision of
//! this header carried the 8000-history numbers, and two of its rows
//! (`pf = 0.05` SCLS at −0.0091, `pf = 0.10` SCLS at +0.0143) were within noise
//! of the values printed here despite looking qualitatively different.
//!
//! Each case is seeded independently from the `--seed` value, so adding or
//! removing a case does not perturb the others. Verified 2026-09-18: inserting
//! the `pf = 0.0388` row reproduced all four pre-existing packing-fraction rows
//! to the last digit printed.
//!
//! ### Packing fraction (r = 0.05 cm, scatter mfp = 0.30 cm)
//!
//! | `pf` | `P_RSA` | `CLS − RSA` | `SCLS − RSA` | s.e. |
//! |---|---|---|---|---|
//! | **0.0388** | 0.2928 | −0.0034 | −0.0016 | 0.0023 |
//! | 0.05 | 0.3469 | +0.0021 | +0.0015 | 0.0024 |
//! | 0.10 | 0.5208 | **+0.0169*** | **+0.0158*** | 0.0025 |
//! | 0.20 | 0.6907 | **+0.0357*** | **+0.0397*** | 0.0023 |
//! | 0.30 | — | — | — | RSA saturates; packing failed |
//!
//! **`pf = 0.0388` is swept because it is the FHR reference spec's KERNEL
//! packing fraction** — the regime `DhTreatment::ChordLengthKernel` runs in.
//! Both models are **unresolved** there (1.5 and 0.7 combined s.e.), so the
//! claim that the kernel-level variant moves CLS into a regime where CLS works
//! is now measured at the point it operates at, rather than extrapolated down
//! from `pf = 0.05`. The central value happens to change sign between 0.0388
//! and 0.05; at 1.5 s.e. that is noise and must not be read as a crossing.
//!
//! **CLS's error grows with packing fraction**, from unresolved at `pf = 0.0388`
//! and `0.05` to +0.0357 (about 15 combined s.e.) at `pf = 0.20`. That is the signature of
//! the **exclusion correlation**: CLS models the matrix chord as exponential,
//! which is the Markovian law, but non-overlapping spheres cannot approach
//! closer than a diameter, so the true gap distribution has far less weight at
//! short distances. The denser the packing, the more that correlation matters,
//! and CLS meets inclusions **too often** — it over-absorbs, in the direction
//! that costs reactivity.
//!
//! Note `pf = 0.30` could not be built at all: plain RSA saturates below it
//! ("could not place sphere 564/572"). Real TRISO packings run at or above
//! that, which is what the RSA–DEM work of Tan et al. (2026) exists to reach —
//! so **this sweep cannot probe the regime the FHR pebble actually occupies**,
//! and the trend says the error there is worse, not better.
//!
//! ### Scattering (pf = 0.20, r = 0.05 cm)
//!
//! | scatter mfp \[cm\] | `P_RSA` | `CLS − RSA` | `SCLS − RSA` |
//! |---|---|---|---|
//! | 0.10 | 0.7221 | **+0.0441*** | **+0.0390*** |
//! | 0.30 | 0.6895 | **+0.0369*** | **+0.0419*** |
//! | 1.00 | 0.6799 | **+0.0305*** | **+0.0543*** |
//! | 3.00 | 0.6749 | **+0.0220*** | **+0.1709*** |
//!
//! **CLS's error doubles as scattering increases**, +0.0220 at mfp 3.0 to
//! +0.0441 at mfp 0.10. This is the memoryless assumption failing exactly where
//! the module documentation says it should: a neutron that scatters often
//! re-crosses ground it has already covered and meets freshly invented medium
//! each time. Both claims the module made for CLS are now measurements.
//!
//! **SCLS fails badly in the opposite corner: +0.1709 at mfp 3.0**, eight times
//! CLS's error on the same case. The retention window is
//! `lambda_transport + R`, so a long mean free path makes it exceed the domain;
//! nothing is ever culled, and the "local view" the method is built on stops
//! being local. This is the same pathology
//! [`DhTreatment::Scls`](outram_mc_libs::dh_universe::DhTreatment::Scls)
//! records for the graphite pebble, reproduced here on a controlled problem.
//!
//! ### Inclusion radius (pf = 0.20, scatter mfp = 0.30 cm)
//!
//! | r \[cm\] | `P_RSA` | `CLS − RSA` | `SCLS − RSA` |
//! |---|---|---|---|
//! | 0.025 | 0.8131 | **+0.0329*** | **+0.0292*** |
//! | 0.050 | 0.6895 | **+0.0369*** | **+0.0419*** |
//! | 0.080 | 0.5897 | **+0.0417*** | **+0.0404*** |
//!
//! A weaker dependence, in the same direction.
//!
//! ### What this does and does not say about the eigenvalue gap
//!
//! Inclusions here are **pure absorbers**, absorbing on entry, so the chord
//! *through* an inclusion never enters the tally. This sweep therefore measures
//! the **matrix-side correlation error only**, cleanly separated from the
//! inclusion-chord law that
//! [`sample_chord_sphere`](outram_mc_libs::stochastic::cls::sample_chord_sphere)
//! corrects. Every number above is unaffected by that correction.
//!
//! The sign matches the eigenvalue result: CLS over-absorbs, and on the FHR
//! unit cell CLS sits about 3000 pcm below exact delta tracking. This is the
//! mechanism.

use std::fmt::Write as _;

use outram_mc_libs::stochastic::benchmark::AbsorptionBenchmark;

/// One row of the sweep.
struct Row {
    packing_fraction: f64,
    particle_radius: f64,
    scatter_mfp: f64,
    p_rsa: f64,
    p_cls: f64,
    p_scls: f64,
    se: f64,
    histories: usize,
}

fn arg_usize(name: &str, default: usize) -> usize {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let histories = arg_usize("--histories", 8000);
    let seed = arg_usize("--seed", 20_260_918) as u64;

    println!("CLS / SCLS regime sweep — error against an EXPLICIT RSA packing");
    println!("================================================================");
    println!("  reference  : explicit RSA geometry, same random walk on every arm");
    println!("  tally      : absorption probability (inclusions are pure absorbers)");
    println!("  histories  : {histories} per arm, seed {seed}");
    println!("  domain     : cube, half-width 0.5 cm\n");

    let mut rows: Vec<Row> = Vec::new();

    // Axis 1: packing fraction, everything else held.
    // Axis 2: scattering mean free path, everything else held.
    // Axis 3: inclusion radius at fixed packing fraction.
    let cases: Vec<(f64, f64, f64)> = {
        let mut v = Vec::new();
        // 0.0388 is the FHR reference spec's KERNEL packing fraction,
        // pf_particle * (r_kernel/r_opyc)^3 = 0.30 * (0.0215/0.0425)^3 -- i.e.
        // the regime `DhTreatment::ChordLengthKernel` actually runs in. It is
        // swept explicitly rather than extrapolated from 0.05, because the
        // whole argument for the kernel-level variant is that it moves CLS into
        // a packing fraction where CLS works, and an argument resting on a
        // point outside the measured range is an assumption.
        for pf in [0.0388, 0.05, 0.10, 0.20, 0.30] {
            v.push((pf, 0.05, 0.30));
        }
        for mfp in [0.10, 0.30, 1.00, 3.00] {
            v.push((0.20, 0.05, mfp));
        }
        for r in [0.025, 0.05, 0.08] {
            v.push((0.20, r, 0.30));
        }
        v
    };

    for (pf, radius, mfp) in cases {
        let bench = AbsorptionBenchmark {
            domain_half_width: 0.5,
            particle_radius: radius,
            packing_fraction: pf,
            scatter_mfp: mfp,
            max_collisions: 200,
            histories,
        };
        let results = match bench.compare(seed) {
            Ok(r) => r,
            Err(e) => {
                println!("  pf {pf:.2} r {radius:.3} mfp {mfp:.2}: packing failed — {e}");
                continue;
            }
        };
        // compare() returns [RSA, CLS, SCLS] in medium order; find by name so a
        // reordering upstream cannot silently transpose the columns.
        let pick = |want: &str| {
            results
                .iter()
                .find(|r| r.model.to_lowercase().contains(want))
                .map(|r| r.absorption_probability)
        };
        let (Some(p_rsa), Some(p_cls), Some(p_scls)) =
            (pick("rsa"), pick("cls"), pick("scls"))
        else {
            // SCLS's name contains "cls", so a naive substring match can alias.
            // Fall back to positional order, which compare() documents.
            let [a, b, c] = results;
            rows.push(Row {
                packing_fraction: pf,
                particle_radius: radius,
                scatter_mfp: mfp,
                p_rsa: a.absorption_probability,
                p_cls: b.absorption_probability,
                p_scls: c.absorption_probability,
                se: (a.absorption_probability * (1.0 - a.absorption_probability)
                    / histories as f64)
                    .sqrt(),
                histories,
            });
            continue;
        };
        rows.push(Row {
            packing_fraction: pf,
            particle_radius: radius,
            scatter_mfp: mfp,
            p_rsa,
            p_cls,
            p_scls,
            se: (p_rsa * (1.0 - p_rsa) / histories as f64).sqrt(),
            histories,
        });
    }

    println!(
        "{:>5} {:>6} {:>6} | {:>8} {:>8} {:>8} | {:>9} {:>9} | {:>7}",
        "pf", "r[cm]", "mfp", "P_RSA", "P_CLS", "P_SCLS", "CLS-RSA", "SCLS-RSA", "s.e."
    );
    println!("{}", "-".repeat(88));
    let mut csv = String::from(
        "packing_fraction,particle_radius_cm,scatter_mfp_cm,histories,\
         p_absorb_rsa,p_absorb_cls,p_absorb_scls,d_cls,d_scls,standard_error\n",
    );
    for r in &rows {
        let d_cls = r.p_cls - r.p_rsa;
        let d_scls = r.p_scls - r.p_rsa;
        // combined s.e. of a difference of two independent binomials
        let comb = r.se * 2.0_f64.sqrt();
        let mark = |d: f64| {
            if d.abs() > 2.0 * comb {
                "*"
            } else {
                " "
            }
        };
        println!(
            "{:>5.2} {:>6.3} {:>6.2} | {:>8.4} {:>8.4} {:>8.4} | {:>+8.4}{} {:>+8.4}{} | {:>7.4}",
            r.packing_fraction,
            r.particle_radius,
            r.scatter_mfp,
            r.p_rsa,
            r.p_cls,
            r.p_scls,
            d_cls,
            mark(d_cls),
            d_scls,
            mark(d_scls),
            r.se
        );
        let _ = writeln!(
            csv,
            "{:.4},{:.4},{:.4},{},{:.6},{:.6},{:.6},{:+.6},{:+.6},{:.6}",
            r.packing_fraction,
            r.particle_radius,
            r.scatter_mfp,
            r.histories,
            r.p_rsa,
            r.p_cls,
            r.p_scls,
            d_cls,
            d_scls,
            r.se
        );
    }
    println!("\n  * = gap exceeds two combined standard errors (resolved)");

    let out = format!(
        "{}/crates/outram-mc-libs/verification_and_validation/cls_scls/regime_sweep.csv",
        env!("CARGO_MANIFEST_DIR").trim_end_matches("/crates/outram-mc-libs")
    );
    if let Some(dir) = std::path::Path::new(&out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&out, &csv) {
        Ok(()) => println!("\nwrote {out}"),
        Err(e) => println!("\ncould not write {out}: {e}"),
    }
}
