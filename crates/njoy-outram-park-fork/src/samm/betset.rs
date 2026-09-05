//! Energy-independent, per-resonance channel-amplitude setup — ported from
//! `betset`'s non-derivative core (`samm.f90:1909-2074`).
//!
//! This is the first real consumer of both [`super::penetrability::pgh`]
//! (uncharged channels) and [`super::coulomb::pghcou`] (charged channels):
//! it converts each resonance's ENDF reduced widths `Gamma_c` into R-matrix
//! reduced-width *amplitudes* `beta_c = sqrt(|Gamma_c|/(2*P_c))` (sign-
//! preserved), their pairwise products `beta_c*beta_c'` (the level-matrix
//! building block Phase 4's R-matrix inversion needs), and the eliminated
//! capture channel's own amplitude `gbetpr` from `Gamma_gamma`.
//!
//! **Scope of this pass:** only `betset`'s non-derivative core is ported
//! here. Upstream's `Want_Partial_Derivs`/`Want_Partial_U`-gated "u
//! parameter" conversion (`samm.f90:2009-2041`) and the "rexternal
//! parameters" section (`samm.f90:2048-2068`) feed the not-yet-ported
//! derivative propagation (`derres`/`derext`, Phase 5) — porting them now
//! would itself be code with no consumer yet. They'll be added alongside
//! `derres`/`derext` rather than staged here disconnected from what reads
//! them.

use crate::samm::coulomb::pghcou;
use crate::samm::context::ChannelKinematics;
use crate::samm::mf2::{ParticlePair, SpinGroup};
use crate::samm::penetrability::pgh;
use crate::NjoyError;

/// One resonance's channel-amplitude data — ported from `betset`'s
/// `betapr`/`beta`/`gbetpr` arrays (`samm.f90:1926-2006`), restricted to
/// one resonance and one spin group (matching [`super::mf2::SpinGroup`]'s
/// own per-group nesting).
#[derive(Debug, Clone)]
pub struct ResonanceAmplitudes {
    /// Reduced-width amplitude `beta_c = sign(Gamma_c) * sqrt(|Gamma_c| / (2*P_c))`
    /// per explicit channel, same order as [`SpinGroup::channels`].
    pub beta_pr: Vec<f64>,
    /// Triangular products `beta_c * beta_c'` for `c' <= c`, Fortran order
    /// `(1,1),(2,1),(2,2),(3,1),(3,2),(3,3),...` — i.e. `beta[idx(c,c')]`
    /// where `idx` packs `c=0..n, c'=0..=c` in that nesting order.
    pub beta: Vec<f64>,
    /// Eliminated (capture) channel amplitude: `[sqrt(|Gamma_gamma|/2)`
    /// (sign-preserved), `|Gamma_gamma|/2`, `(|Gamma_gamma|/2)^2]`
    /// (`samm.f90:2001-2006`).
    pub gbetpr: [f64; 3],
}

/// Pack a triangular `(c, c')` pair (`c'<=c`, both 0-indexed) into the flat
/// index used by [`ResonanceAmplitudes::beta`], matching `betset`'s own
/// `kl=kl+1` running counter for `do k=1,mmaxc: do l=1,k`.
pub fn beta_triangular_index(c: usize, c_prime: usize) -> usize {
    debug_assert!(c_prime <= c);
    c * (c + 1) / 2 + c_prime
}

/// Compute [`ResonanceAmplitudes`] for every resonance in `group` — ported
/// from `betset`'s per-group loop body (`samm.f90:1932-2006`), excluding
/// the derivative-only `dum`/u-parameter bookkeeping (see this module's
/// doc comment).
///
/// `kinematics` must be [`super::context::compute_channel_kinematics`]'s
/// output for this same `group` (one entry per [`SpinGroup::channels`]
/// entry, same order).
///
/// # Known unverified step — stale `drho` when a resonance sits exactly on
/// a channel threshold
///
/// If a resonance's energy exactly equals a channel's threshold energy
/// (`ex=|E_res-E_channel|==0`), upstream skips the `rho`/`pgh`/`pghcou`
/// call entirely for that channel (`samm.f90:1950`, the `if (ex.ne.zero)`
/// guard) — but leaves its `dp`/`drho` **locals at whatever the previous
/// loop iteration set them to** (`samm.f90` declares `dp`,`drho` once per
/// subroutine call, not reset per iteration), which only matters for the
/// derivative-only `dum` term this pass doesn't compute. Flagged here
/// (rather than silently "fixed") for whoever ports the derivative
/// bookkeeping later: the non-derivative amplitude computed here (`p`
/// stays at its initial value of `1.0` when `ex==0`, matching upstream)
/// is unaffected by this staleness, since `p` genuinely is always
/// (re-)initialized to `1.0` before the `ex`-guarded block on every
/// iteration.
pub fn compute_resonance_amplitudes(
    group: &SpinGroup,
    kinematics: &[ChannelKinematics],
    pairs: &[ParticlePair],
) -> Result<Vec<ResonanceAmplitudes>, NjoyError> {
    let mmaxc = group.channels.len();

    let mut out = Vec::with_capacity(group.resonances.len());
    for resonance in &group.resonances {
        let mut beta_pr = vec![0.0_f64; mmaxc];

        for (j, ch) in group.channels.iter().enumerate() {
            let gamma_j = resonance.channel_widths[j];
            if gamma_j == 0.0 {
                continue;
            }

            let pair = pairs.get(ch.particle_pair - 1).ok_or_else(|| {
                NjoyError::EndfParse(format!(
                    "samm betset: channel {} references undeclared particle pair {}",
                    j + 1,
                    ch.particle_pair
                ))
            })?;

            // samm.f90:1942-1980 -- p stays 1.0 (its initial value) unless
            // this channel is penetrability-enabled AND the resonance is
            // off-threshold; pf(rho,lsp) is called upstream just before
            // pgh but its result is unconditionally overwritten by pgh's
            // own `p` output on every path, so it is not called here
            // either (see `penetrability::penetrability_factor`'s doc
            // comment).
            let mut p = 1.0_f64;
            if pair.penetrability_flag > 0 {
                let ex = (resonance.energy - kinematics[j].echan).abs();
                if ex != 0.0 {
                    let ex_sqrt = ex.sqrt();
                    let rho = kinematics[j].zkte * ex_sqrt;
                    if kinematics[j].zeta == 0.0 {
                        p = pgh(rho, ch.l, ch.boundary, pair.shift_flag).p;
                    } else {
                        let eta = kinematics[j].zeta / ex_sqrt;
                        match pghcou(rho, ch.l, ch.boundary, pair.shift_flag, eta, false) {
                            Some(out) if out.p > 0.0 => p = out.p,
                            Some(out) => {
                                log::warn!(
                                    "samm betset: Coulomb penetrability P={} <= 0 at E_res={}, forcing P=1 (samm.f90:1965-1977)",
                                    out.p,
                                    resonance.energy
                                );
                                p = 1.0;
                            }
                            None => {
                                log::warn!(
                                    "samm betset: Coulomb penetrability evaluation failed at E_res={}, forcing P=1",
                                    resonance.energy
                                );
                                p = 1.0;
                            }
                        }
                    }
                }
            }

            let mut beta_c = (0.5 * gamma_j.abs() / p).sqrt();
            if gamma_j < 0.0 {
                beta_c = -beta_c;
            }
            beta_pr[j] = beta_c;
        }

        // samm.f90:1991-1998 -- triangular products.
        let mut beta = Vec::with_capacity(mmaxc * (mmaxc + 1) / 2);
        for k in 0..mmaxc {
            for l in 0..=k {
                beta.push(beta_pr[l] * beta_pr[k]);
            }
        }

        // samm.f90:2001-2006 -- eliminated (capture) channel amplitude.
        let g2 = resonance.gamma_gamma.abs() / 2.0;
        let mut g1 = g2.sqrt();
        if resonance.gamma_gamma < 0.0 {
            g1 = -g1;
        }
        let g3 = g2 * g2;

        out.push(ResonanceAmplitudes {
            beta_pr,
            beta,
            gbetpr: [g1, g2, g3],
        });
    }

    Ok(out)
}
