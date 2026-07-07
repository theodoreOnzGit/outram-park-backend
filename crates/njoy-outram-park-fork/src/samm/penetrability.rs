//! Hard-sphere (neutral-particle) penetrability, shift factor, and phase
//! shift — ported from `samm.f90`'s `pf`, `genpsf`, and `pgh`.
//!
//! These implement the classic Reich-Moore/R-matrix hard-sphere formulas for
//! the penetrability `P_l(rho)`, shift factor `S_l(rho)`, and hard-sphere
//! phase shift `phi_l(rho)` as closed-form polynomials in the dimensionless
//! channel radius `rho = k*a_c` for `l=0..4`, with a recursion (`genpsf`) for
//! `l>4`. No Coulomb (charged-particle) physics is needed here — that is
//! `pghcou`/`coulfg` and friends, not yet ported (see `src/samm/README.md`).
//!
//! `pgh` additionally forms `G+iH`, the real/imaginary parts of
//! `1/(S-B+iP)` (`B` the ENDF boundary condition), which is the quantity
//! `betset` actually needs per channel.

/// Penetrability `P_l(rho)` only (no derivative, no shift factor) — ported
/// from `pf` (`samm.f90:3770-3821`). Used by `betset` before its own call
/// into [`pgh`], whose result supersedes this one on every code path (`pgh`
/// always overwrites `p` regardless of the value passed in) — ported as-is
/// even though the call appears to have no effect on `betset`'s output.
pub fn penetrability_factor(rho: f64, l: i32) -> f64 {
    match l {
        0 => rho,
        1 => {
            let a2 = rho * rho;
            rho * a2 / (1.0 + a2)
        }
        2 => {
            let a2 = rho * rho;
            let a4 = a2 * a2;
            rho * a4 / (9.0 + a2 * (3.0 + a2))
        }
        3 => {
            let a2 = rho * rho;
            let a4 = a2 * a2;
            let a6 = a2 * a4;
            let d = 225.0 + a2 * (45.0 + a2 * (6.0 + a2));
            rho * a6 / d
        }
        4 => {
            let a2 = rho * rho;
            let a4 = a2 * a2;
            let a8 = a4 * a4;
            let d = 11025.0 + a2 * (1575.0 + a2 * (135.0 + a2 * (10.0 + a2)));
            rho * a8 / d
        }
        _ => genpsf(rho, l).p,
    }
}

/// `P_l`, `dP_l/d(rho)`, `S_l`, `dS_l/d(rho)` for `l>4`, via the upward
/// recursion from the closed-form `l=4` seed — ported from `genpsf`
/// (`samm.f90:3689-3768`).
pub struct Genpsf {
    pub p: f64,
    pub dp: f64,
    pub s: f64,
    pub ds: f64,
}

/// # Known unverified step — `dss` used before being set at the `l=4` seed
///
/// `samm.f90:3718-3719` reads:
/// ```text
/// sss=-dss
/// dss=-60*a*(157.5+a2*(18+a2))/d+dss*a*(3150+a2*(540+a2*(60+a2*8)))/d
/// ```
/// Both the read on the first line and the self-reference on the right-hand
/// side of the second line use `dss` **before** any value has been assigned
/// to it in this subroutine (Fortran does not zero-initialize local
/// variables by language rule, though some compilers do so in practice).
/// Comparing to `pgh`'s `l=4` branch (`samm.f90:3634-3637`), which computes
/// the analogous quantity as a *self-contained* two-step assignment
/// (`ds=(44100+...)/d` then `gg=-ds-bound` then `ds=(next formula)`), it
/// looks like `genpsf` is missing the equivalent first assignment
/// (`dss=(44100+a2*(4725+a2*(270+10*a2)))/d`) before the two lines shown
/// above — i.e. a likely latent transcription bug in `samm.f90` itself,
/// probably benign in practice because `l>4` (channel orbital angular
/// momentum above g-wave) essentially never occurs in real resonance-region
/// evaluations, so this path is rarely if ever exercised.
///
/// Ported literally with `dss` seeded to `0.0` (matching the most common
/// real-world Fortran compiler default for an uninitialized local, and the
/// only value that keeps this translatable into safe Rust at all) rather
/// than "fixed" using the `pgh` formula guessed above — per this crate's
/// translate-faithfully-and-flag convention, that guess belongs to
/// verification, not translation. Flagged for Opus: if an evaluation with
/// `l>4` channels is ever exercised, check this against `pgh`'s `l=4`
/// pattern first.
pub fn genpsf(rho: f64, l_want: i32) -> Genpsf {
    let a = rho;
    let a2 = a * a;
    let a4 = a2 * a2;
    let a8 = a4 * a4;

    let mut l = 4;
    let mut yyy = 105.0 - 45.0 * a2 + a4;
    let mut qqq = 105.0 - 10.0 * a2;
    let mut dqq = (qqq - 20.0 * a2) * yyy + a2 * qqq * (90.0 - 4.0 * a2);
    qqq *= a;

    let d4 = 11025.0 + a2 * (1575.0 + a2 * (135.0 + a2 * (10.0 + a2)));
    let mut ppp = a * a8 / d4;
    let mut dpp = (a8 / d4) * (9.0 - (1575.0 + a2 * (270.0 + a2 * (30.0 + a2 * 4.0))) * 2.0 * (a2 / d4));

    // See this function's doc comment: `dss` is used before being set in
    // the upstream Fortran too. Seeded to 0.0 here.
    let mut dss = 0.0_f64;
    let mut sss = -dss;
    dss = -60.0 * a * (157.5 + a2 * (18.0 + a2)) / d4
        + dss * a * (3150.0 + a2 * (540.0 + a2 * (60.0 + a2 * 8.0))) / d4;

    loop {
        l += 1;
        let el = l as f64;
        let els = el - sss;

        let x = ppp / els;
        let dx = (dpp + x * dss) / els;
        let yyx = yyy - qqq * x;
        let qqx = qqq + x * yyy;
        let d = qqx * (qqq * dx * yyy + dqq * x);
        let dqx = (dqq + dx * yyy * yyy) * yyx + d;

        let d2 = els * els + ppp * ppp;
        let ppx = a2 * ppp / d2;
        let c = 1.0 + a * (els * dss - ppp * dpp) / d2;
        let dpx = a * (2.0 * ppp * c + a * dpp) / d2;

        let ssx = a2 * els / d2 - el;
        let dsx = a * (2.0 * els * c - a * dss) / d2;

        if l != l_want {
            ppp = ppx;
            dpp = dpx;
            sss = ssx;
            dss = dsx;
            qqq = qqx;
            dqq = dqx;
            yyy = yyx;
            continue;
        }

        return Genpsf { p: ppx, dp: dpx, s: ssx, ds: dsx };
    }
}

/// `G+iH` (the real/imaginary parts of `1/(S-B+iP)`), the penetrability
/// `P`, and their derivatives — ported from `pgh` (`samm.f90:3566-3687`).
///
/// If `S-B+iP == 0` (the `iffy` condition upstream), `g=h=p=dp=ds=0` is
/// returned along with `iffy=true` rather than dividing by zero.
pub struct Pgh {
    pub g: f64,
    pub h: f64,
    pub p: f64,
    pub dp: f64,
    pub ds: f64,
    pub iffy: bool,
}

/// `ishift`: nonzero enables the level-shift term (`ISHIFT`/`shift_flag` on
/// [`super::mf2::ParticlePair`]).
pub fn pgh(rho: f64, l: i32, bound: f64, ishift: i32) -> Pgh {
    let r = rho;
    let mut gg = 0.0_f64;
    let dp: f64;
    let mut ds = 0.0_f64;
    let hh: f64;

    match l {
        0 => {
            hh = r;
            dp = 1.0;
            if ishift > 0 {
                gg = -bound;
            }
        }
        1 => {
            let r2 = r * r;
            let d = 1.0 + r2;
            hh = r * r2 / d;
            dp = (r2 / d) * (3.0 + r2) / d;
            if ishift > 0 {
                gg = -1.0 / d - bound;
                ds = 2.0 * (r / d) / d;
            }
        }
        2 => {
            let r2 = r * r;
            let r4 = r2 * r2;
            let d = 9.0 + r2 * (3.0 + r2);
            hh = r * r4 / d;
            dp = r2 * (r2 / d) * (45.0 + r2 * (9.0 + r2)) / d;
            if ishift > 0 {
                ds = (18.0 + 3.0 * r2) / d;
                gg = -ds - bound;
                ds = -6.0 * r / d + ds * 4.0 * r * (1.5 + r2) / d;
            }
        }
        3 => {
            let r2 = r * r;
            let r4 = r2 * r2;
            let r6 = r4 * r2;
            let d = 225.0 + r2 * (45.0 + r2 * (6.0 + r2));
            hh = r * r6 / d;
            dp = (r6 / d) * (1575.0 + (225.0 + r2 * (18.0 + r2)) * r2) / d;
            if ishift > 0 {
                ds = (675.0 + r2 * (90.0 + r2 * 6.0)) / d;
                gg = -ds - bound;
                ds = -24.0 * (7.5 + r2) * r / d + ds * 6.0 * r * (15.0 + r2 * (4.0 + r2)) / d;
            }
        }
        4 => {
            let r2 = r * r;
            let r4 = r2 * r2;
            let r8 = r4 * r4;
            let d = 11025.0 + r2 * (1575.0 + r2 * (135.0 + r2 * (10.0 + r2)));
            hh = r * r8 / d;
            dp = (r8 / d) * (9.0 - (1575.0 + r2 * (270.0 + r2 * (30.0 + r2 * 4.0))) * 2.0 * (r2 / d));
            if ishift > 0 {
                ds = (44100.0 + r2 * (4725.0 + r2 * (270.0 + 10.0 * r2))) / d;
                gg = -ds - bound;
                ds = -60.0 * r * (157.5 + r2 * (18.0 + r2)) / d
                    + ds * r * (3150.0 + r2 * (540.0 + r2 * (60.0 + r2 * 8.0))) / d;
            }
        }
        _ => {
            let out = genpsf(r, l);
            hh = out.p;
            dp = out.dp;
            if ishift > 0 {
                gg = out.s - bound;
                ds = out.ds;
            }
        }
    }

    let hh = if hh <= 1.0e-35 { 0.0 } else { hh };

    if gg == 0.0 && hh == 0.0 {
        Pgh { g: 0.0, h: 0.0, p: 0.0, dp, ds: 0.0, iffy: true }
    } else if gg == 0.0 {
        Pgh { g: 0.0, h: -1.0 / hh, p: hh, dp, ds, iffy: false }
    } else if hh == 0.0 {
        Pgh { g: 1.0 / gg, h: 0.0, p: 0.0, dp, ds, iffy: false }
    } else if hh + gg != hh {
        if hh + gg != gg {
            let d = hh * hh + gg * gg;
            Pgh { g: gg / d, h: -hh / d, p: hh, dp, ds, iffy: false }
        } else {
            Pgh { g: 1.0 / gg, h: -(hh / gg) / gg, p: hh, dp, ds, iffy: false }
        }
    } else {
        Pgh { g: (gg / hh) / hh, h: -1.0 / hh, p: hh, dp, ds, iffy: false }
    }
}
