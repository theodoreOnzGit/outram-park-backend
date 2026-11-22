// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! **Collision kinematics against OpenMC `8f79d9d0`** (gh:#1444, #1386 prio-1
//! row 4): `rotate_angle`, the centre-of-mass to laboratory block of
//! `inelastic_scatter`, a discrete-level inelastic collision end to end, and
//! `elastic_scatter` (target at rest, free gas by constant cross section, and the
//! resonance-scattering window by RVS and DBRC).
//!
//! # Methodology
//!
//! `verification_and_validation/upstream_openmc_samplers/run.sh` extracts these
//! routines VERBATIM from `math_functions.cpp`, `physics.cpp` and `nuclide.cpp`
//! (commit and SHA-256 checked) and compiles them with `g++ -O2
//! -ffp-contract=off` against stubs of `Particle`, `Nuclide` and `Reaction`
//! (`stubs/kin_stubs.h`); the real `LevelInelastic`, `UncorrelatedAngleEnergy`,
//! `random_lcg.cpp` and `position.cpp` are linked. Each `K ...` line of
//! `reference.txt` records inputs and outputs as hex bits and the final RNG
//! state. This test feeds the same inputs to the port and compares bits.
//!
//! Two sets of port functions are compared:
//!
//! - the **OpenMC-faithful** ones (`rotate_angle_openmc`,
//!   `inelastic_cm_to_lab_openmc`, `level_inelastic_scatter_openmc`,
//!   `elastic_scatter_openmc`), translated line by line from upstream: **pass
//!   criterion bit identity** of every output and of the end state;
//! - the **older kernels** the transport loops still call
//!   (`rotate_direction`, `cm_to_lab`, `two_body_scatter_with_mu`,
//!   `free_gas_elastic_scatter_dbrc`): the same physics in another operation
//!   order and, for free gas, another draw order. These tests REPORT the counts
//!   and assert only what is true of them (worst relative difference of `E'`
//!   below 1e-14 for the at-rest and level paths; end state unchanged where the
//!   draw count is the same).
//!
//! # Results
//!
//! See `verification_and_validation/upstream_openmc_samplers/README.md`.

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::physics::scatter::{
    cm_to_lab, elastic_scatter_openmc, free_gas_elastic_scatter_dbrc, inelastic_cm_to_lab_openmc,
    level_inelastic_scatter_openmc, renormalise_if_drifted, rotate_angle_openmc,
    rotate_angle_openmc_unguarded, rotate_direction, rotate_direction_unguarded,
    two_body_scatter_with_mu, DbrcTable, ResonanceScatteringMethod,
};
use outram_mc_libs::rng::lcg::prn;

const REFERENCE: &str =
    include_str!("../verification_and_validation/upstream_openmc_samplers/reference.txt");

fn kl(kind: &str) -> Vec<&'static str> {
    REFERENCE
        .lines()
        .filter(|l| {
            let mut t = l.split_whitespace();
            t.next() == Some("K") && t.next() == Some(kind)
        })
        .collect()
}
fn field<'a>(line: &'a str, key: &str) -> &'a str {
    let pat = format!(" {key}=");
    let i = line.find(&pat).unwrap_or_else(|| panic!("no {key} in {line}")) + pat.len();
    let rest = &line[i..];
    &rest[..rest.find(' ').unwrap_or(rest.len())]
}
fn bits(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}
fn num(line: &str, key: &str) -> f64 {
    f64::from_bits(bits(field(line, key)))
}
fn vec3(line: &str, key: &str) -> [f64; 3] {
    let v: Vec<f64> = field(line, key).split(',').map(|h| f64::from_bits(bits(h))).collect();
    [v[0], v[1], v[2]]
}
fn dir(line: &str, key: &str) -> Direction {
    let v = vec3(line, key);
    Direction::new(v[0], v[1], v[2])
}
fn same3(a: Direction, b: [f64; 3]) -> bool {
    a.u.to_bits() == b[0].to_bits() && a.v.to_bits() == b[1].to_bits() && a.w.to_bits() == b[2].to_bits()
}
fn seed_of(line: &str) -> u64 {
    field(line, "seed").parse::<u64>().unwrap() * 7919 + 11
}
/// `uniform_distribution(-1, 1, seed)` (`random_dist.h`).
fn uniform_pm1(seed: &mut u64) -> f64 {
    -1.0 + (1.0 - (-1.0)) * prn(seed)
}

/// `rotate_angle`, OpenMC-faithful port: 6 directions (generic, both poles, an
/// axis, near-pole) x 5 cosines x 4 seeds. Bit identity of the vector and of the
/// end state.
///
/// - `rotate_angle_openmc_unguarded` (the translation): **120/120** outputs and
///   end states bit-identical to upstream's.
/// - `rotate_angle_openmc` (what transport calls, with the gh:#1497 norm guard):
///   bit-identical to `renormalise_if_drifted(upstream output)` on 120/120, which
///   is upstream's output itself on 108/120. The other 12 are the near-pole
///   direction `unit(1e-6, 2e-6, 1)` (2.2e-6 rad from +z) with `mu` = -0.2, 0
///   and 0.41: upstream's own output there is 6.9e-8 to 8.3e-8 off unit length
///   (`1 - w^2` loses about 11 digits to cancellation before
///   `b = sqrt(1 - w^2)` divides by it), past `DIRECTION_NORM_DRIFT_TOL`
///   (1.49e-8), so the guard returns it divided by its norm. Measured
///   2026-11-11; before the guard `rotate_angle_openmc` was 120/120.
#[test]
fn rotate_angle_openmc_is_bit_identical() {
    let (mut n, mut raw_ok, mut ok, mut ok_norm, mut end_ok) = (0, 0, 0, 0, 0);
    for l in kl("rot") {
        let r = vec3(l, "out");
        let r_norm = renormalise_if_drifted(Direction::new(r[0], r[1], r[2]));
        let mut s_raw = seed_of(l);
        let raw = rotate_angle_openmc_unguarded(dir(l, "u"), num(l, "mu"), None, &mut s_raw);
        let mut seed = seed_of(l);
        let out = rotate_angle_openmc(dir(l, "u"), num(l, "mu"), None, &mut seed);
        n += 1;
        raw_ok += (same3(raw, r) && s_raw == bits(field(l, "end"))) as usize;
        ok += same3(out, r) as usize;
        ok_norm += same3(out, [r_norm.u, r_norm.v, r_norm.w]) as usize;
        end_ok += (seed == bits(field(l, "end"))) as usize;
    }
    println!(
        "rotate_angle_openmc_unguarded: {raw_ok}/{n} bit-identical; rotate_angle_openmc: {ok}/{n} \
         bit-identical to upstream, {ok_norm}/{n} to renormalise_if_drifted(upstream); end state {end_ok}/{n}"
    );
    assert_eq!((raw_ok, ok_norm, end_ok), (n, n, n));
    assert_eq!(ok, 108, "the guard fires on exactly the 12 near-pole cases");
}

/// The older `rotate_direction`: identical off the poles, a different (equally
/// valid) azimuth parametrisation at the poles (`1 - w^2 <= 1e-20`). Reports both.
/// Off the poles it goes through the gh:#1497 norm guard as `rotate_angle_openmc`
/// does (see that test): `rotate_direction_unguarded` must equal upstream's
/// output, and `rotate_direction` must equal `renormalise_if_drifted` of it.
#[test]
fn rotate_direction_differs_only_at_the_poles() {
    let (mut n, mut ok, mut pole_n, mut pole_ok, mut end_ok) = (0, 0, 0, 0, 0);
    for l in kl("rot") {
        let u = dir(l, "u");
        let r = vec3(l, "out");
        let r_norm = renormalise_if_drifted(Direction::new(r[0], r[1], r[2]));
        let mut seed = seed_of(l);
        let out = rotate_direction(u, num(l, "mu"), &mut seed);
        let raw = rotate_direction_unguarded(u, num(l, "mu"), &mut seed_of(l));
        let same = same3(out, r);
        let pole = (1.0 - u.w * u.w).sqrt() <= 1.0e-10;
        n += 1;
        ok += same as usize;
        end_ok += (seed == bits(field(l, "end"))) as usize;
        if pole {
            pole_n += 1;
            pole_ok += same as usize;
        } else {
            assert!(same3(raw, r), "off the poles rotate_direction_unguarded must match: {l}");
            assert!(
                same3(out, [r_norm.u, r_norm.v, r_norm.w]),
                "off the poles rotate_direction must match through the guard: {l}"
            );
        }
    }
    println!(
        "rotate_direction: {ok}/{n} bit-identical ({pole_ok}/{pole_n} at the poles), end state {end_ok}/{n}"
    );
    assert_eq!(end_ok, n, "same number of draws");
}

/// The CM -> lab block of `inelastic_scatter`, OpenMC-faithful port.
#[test]
fn inelastic_cm_to_lab_openmc_is_bit_identical() {
    let (mut n, mut ok) = (0, 0);
    for l in kl("inelcm") {
        let (e, mu) = inelastic_cm_to_lab_openmc(num(l, "Ein"), num(l, "Ecm"), num(l, "mu"), num(l, "A"));
        let mr = num(l, "mulab");
        let mr = if mr.abs() > 1.0 { 1.0_f64.copysign(mr) } else { mr };
        n += 1;
        ok += (e.to_bits() == num(l, "E").to_bits() && mu.to_bits() == mr.to_bits()) as usize;
    }
    println!("inelastic_cm_to_lab_openmc: {ok}/{n} bit-identical");
    assert_eq!(ok, n);
}

/// The older `cm_to_lab`: the same transform in another operation order.
#[test]
fn cm_to_lab_differs_from_openmc_block_by_roundoff_only() {
    let (mut n, mut e_ok, mut mu_ok) = (0, 0, 0);
    let mut worst = 0.0_f64;
    for l in kl("inelcm") {
        let (e_out, mu_lab) = cm_to_lab(num(l, "Ein"), num(l, "Ecm"), num(l, "mu"), num(l, "A"));
        let (er, mr) = (num(l, "E"), num(l, "mulab"));
        let mr = if mr.abs() > 1.0 { 1.0_f64.copysign(mr) } else { mr };
        n += 1;
        e_ok += (e_out.to_bits() == er.to_bits()) as usize;
        mu_ok += (mu_lab.to_bits() == mr.to_bits()) as usize;
        worst = worst.max(((e_out - er) / er).abs()).max((mu_lab - mr).abs());
    }
    println!("cm_to_lab vs inelastic_scatter block: E' {e_ok}/{n}, mu {mu_ok}/{n}, worst {worst:.3e}");
    assert!(worst < 1.0e-13, "round-off level only: {worst:e}");
}

fn level_u0() -> Direction {
    dir(kl("rot")[0], "u")
}

/// Discrete-level inelastic scatter end to end (`LevelInelastic::sample`, the CM
/// -> lab block, `rotate_angle`), OpenMC-faithful port: bit identity of E', the
/// direction and the end state.
#[test]
fn discrete_level_scatter_openmc_is_bit_identical() {
    let u0 = level_u0();
    let (mut n, mut ok) = (0, 0);
    for l in kl("level") {
        let mut seed = seed_of(l);
        let mu = uniform_pm1(&mut seed);
        let (e, u) = level_inelastic_scatter_openmc(num(l, "Ein"), u0, num(l, "A"), num(l, "Q"), mu, &mut seed);
        n += 1;
        ok += (e.to_bits() == num(l, "E").to_bits()
            && same3(u, vec3(l, "out"))
            && seed == bits(field(l, "end"))) as usize;
    }
    println!("level_inelastic_scatter_openmc: {ok}/{n} bit-identical (E', direction, end state)");
    assert_eq!(ok, n);
}

/// The older `two_body_scatter_with_mu` for a level: same draws, round-off-level `E'`.
#[test]
fn two_body_level_differs_from_openmc_by_roundoff_only() {
    let u0 = level_u0();
    let (mut n, mut e_ok, mut u_ok, mut end_ok) = (0, 0, 0, 0);
    let mut worst = 0.0_f64;
    for l in kl("level") {
        let mut seed = seed_of(l);
        let mu = uniform_pm1(&mut seed);
        let (e_out, u_out) =
            two_body_scatter_with_mu(num(l, "Ein"), u0, num(l, "A"), num(l, "Q"), mu, &mut seed);
        let er = num(l, "E");
        n += 1;
        e_ok += (e_out.to_bits() == er.to_bits()) as usize;
        u_ok += same3(u_out, vec3(l, "out")) as usize;
        end_ok += (seed == bits(field(l, "end"))) as usize;
        worst = worst.max(((e_out - er) / er).abs());
    }
    println!("two_body level: E' {e_ok}/{n}, direction {u_ok}/{n}, end {end_ok}/{n}, worst rel(E') {worst:.3e}");
    assert_eq!(end_ok, n);
    assert!(worst < 1.0e-14, "round-off level only: {worst:e}");
}

fn table0k() -> Vec<(f64, f64)> {
    let l = REFERENCE.lines().find(|l| l.starts_with("kin table0k")).unwrap();
    let list = |key: &str| -> Vec<f64> { field(l, key).split(',').map(|h| f64::from_bits(bits(h))).collect() };
    list("egrid").into_iter().zip(list("xs")).collect()
}

/// What a reference `K elastic` line sets up: mass ratio, the 0 K table of a
/// resonant nuclide (with its method), and whether the target is at rest.
fn elastic_setup(l: &str, pairs: &[(f64, f64)]) -> (f64, Option<DbrcTable>, &'static str) {
    let kt = 0.0517;
    let e = num(l, "E");
    let nuc = field(l, "nuc");
    let (awr, table) = match nuc {
        "U" => {
            let t = DbrcTable::from_pairs(pairs, 1000.0).unwrap();
            let m = if field(l, "meth") == "rvs" {
                ResonanceScatteringMethod::Rvs
            } else {
                ResonanceScatteringMethod::Dbrc
            };
            (235.0, Some(t.with_method(m)))
        }
        "H" => (0.99917, None),
        _ => (11.898, None),
    };
    let at_rest = match nuc {
        "U" => e > 1000.0,
        "H" => false,
        _ => e >= 400.0 * kt,
    };
    let category = if at_rest {
        "target at rest"
    } else if nuc == "U" && e >= 0.01 {
        "resonance window"
    } else {
        "free gas (CXS)"
    };
    (awr, table, category)
}

/// `elastic_scatter` (+ `sample_target_velocity`, `sample_cxs_target_velocity`),
/// OpenMC-faithful port: 1280 collisions off a resonant U-like nuclide (RVS and
/// DBRC), hydrogen and carbon, from 3 meV to 200 keV, 4 directions (both poles),
/// 8 seeds, by regime.
///
/// **Criterion:** the end RNG state is identical in EVERY collision (same draws
/// in the same order, so the same accept/reject decisions) and the target at
/// rest is bit-identical in `E'`, direction and lab cosine. In the free-gas and
/// resonance regimes `E'` is accepted within 1e-12 relative where it is not
/// bit-identical: those paths call `ln` and `cos` several times per sample, and
/// PETIR's `RealMath` (gh:#985) differs from glibc in the last bit for a few
/// percent of arguments, as measured for the same two functions by
/// `maxwell_energy_differences_are_libm_roundoff` and
/// `nbody_differences_are_libm_roundoff`. The counts are printed.
#[test]
fn elastic_scatter_openmc_matches_openmc() {
    let pairs = table0k();
    let kt = 0.0517;
    // per regime: n, bit-identical, end state equal, worst relative E' difference
    let mut cat: std::collections::BTreeMap<&'static str, (usize, usize, usize, f64)> = Default::default();
    for l in kl("elastic") {
        let (awr, table, category) = elastic_setup(l, &pairs);
        let mut seed = seed_of(l);
        let (eo, uo, mu) = elastic_scatter_openmc(
            num(l, "E"),
            dir(l, "u"),
            awr,
            kt,
            table.as_ref(),
            false,
            uniform_pm1,
            &mut seed,
        );
        let c = cat.entry(category).or_default();
        c.0 += 1;
        c.1 += (eo.to_bits() == num(l, "Eo").to_bits()
            && same3(uo, vec3(l, "out"))
            && mu.to_bits() == num(l, "mu").to_bits()) as usize;
        c.2 += (seed == bits(field(l, "end"))) as usize;
        c.3 = c.3.max(((eo - num(l, "Eo")) / num(l, "Eo")).abs());
    }
    for (k, c) in &cat {
        println!(
            "elastic_scatter_openmc [{k:17}] n={:4}  bit-identical {:4}  end state {:4}  worst rel(E') {:.3e}",
            c.0, c.1, c.2, c.3
        );
        assert_eq!(c.2, c.0, "{k}: end state differs, i.e. a different draw sequence");
        assert!(c.3 < 1.0e-12, "{k}: worst rel(E') {:e}", c.3);
    }
    let rest = cat["target at rest"];
    assert_eq!(rest.1, rest.0, "target at rest must be bit-identical");
}

/// The older free-gas kernel with the CM cosine drawn first (as every call site
/// does): at rest it spends the same draws and differs by round-off in `E'`; in
/// the free-gas and resonance regimes it spends a different number of draws, so
/// the collisions are different samples of the same distribution (reported, and
/// the distributions are compared statistically by `dbrc_single_scatter_vs_openmc.rs`).
#[test]
fn legacy_free_gas_kernel_is_report_only() {
    let pairs = table0k();
    let kt = 0.0517;
    let mut cat: std::collections::BTreeMap<&'static str, [usize; 4]> = Default::default();
    let mut worst_rest = 0.0_f64;
    for l in kl("elastic") {
        let (awr, table, category) = elastic_setup(l, &pairs);
        let e = num(l, "E");
        let mut seed = seed_of(l);
        let mu_cm = uniform_pm1(&mut seed);
        let (eo, uo) =
            free_gas_elastic_scatter_dbrc(e, dir(l, "u"), awr, kt, mu_cm, &mut seed, table.as_ref());
        let c = cat.entry(category).or_default();
        c[0] += 1;
        c[1] += (eo.to_bits() == num(l, "Eo").to_bits()) as usize;
        c[2] += same3(uo, vec3(l, "out")) as usize;
        c[3] += (seed == bits(field(l, "end"))) as usize;
        if category == "target at rest" {
            worst_rest = worst_rest.max(((eo - num(l, "Eo")) / num(l, "Eo")).abs());
        }
    }
    for (k, c) in &cat {
        println!("legacy elastic [{k:17}] n={:4}  E' {:4}  direction {:4}  end state {:4}", c[0], c[1], c[2], c[3]);
    }
    // at rest the draw count is the same and the difference is round-off
    assert_eq!(cat["target at rest"][3], cat["target at rest"][0]);
    assert!(worst_rest < 1.0e-14, "at rest: round-off level only, got {worst_rest:e}");
}
