// Ported from NJOY2016 `src/samm.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine betset`, l.2009-2041 — the u-parameter conversion factors
//     `uuuu`/`duuu`/`iduu` (`Want_Partial_U = .false.`).
//   - `subroutine babb`, l.2817-2921 — the energy-independent part of
//     `dR/du`, `br`/`bi`, and the parameter vector `par`.
//   - `subroutine allo`, l.1613-1626 — the derivative array shapes.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Partial derivatives of the R-matrix-limited cross sections with respect
//! to the resonance parameters — the `Want_Partial_Derivs` half of
//! `samm.f90`, whose only caller is ERRORR (`errorr.f90:392`, the `LRF=7`
//! MF=32 path `rpxsamm`).
//!
//! The parameter vector is ordered exactly as upstream numbers `ipar`:
//! spin groups in file order, resonances in file order within a group,
//! and for each resonance `(E_λ, Γ_γ, Γ_1, …, Γ_nchan)` — `nchan + 2`
//! entries — so that a `LCOMP=2` MF=32 record (`ER, GAM_1..GAM_NCH` with
//! the eliminated channel first) lines up with it one-to-one.
//!
//! Upstream differentiates with respect to *u-parameters* (`sqrt(E)`,
//! `sqrt(Γ_γ)`, the reduced-width amplitudes `γ_c`) and converts back to
//! `(E, Γ)` in `crosss` through `uuuu`/`duuu`/`iduu`
//! (`samm.f90:3193-3222`); [`DerivSetup`] carries those factors.
//!
//! ```text
//! deriv_setup  (once per section)   babb + betset's u-parameters       (this file)
//! abpart_derivs (per energy)         upr/upi -> pr/pi = dR/du           (energy.rs)
//! setqri / settri / derres           dX/dR, dσ/dR, dσ/du per spin group (energy.rs)
//! cross_sections_with_derivs         the crosss driver + normalisation  (xsformula/crosss.rs)
//! cssammy_with_derivs                the ERRORR-facing (MT slot) wrapper (xsformula/cssammy.rs)
//! ```
//!
//! **Not ported:** `derext` (derivatives with respect to background
//! R-matrix parameters, `nrext > 0`) — [`crate::samm::mf2`] does not parse
//! `KBK` background terms, so a section carrying them is refused one level
//! up; the `Want_Angular_Dist` blocks (`D_Coef_Leg`, `tx`, `gcphase`).

pub mod energy;

use crate::samm::betset::ResonanceAmplitudes;
use crate::samm::mf2::RmlSection;

/// The energy-independent derivative setup for one section — `babb`'s
/// `br`/`bi`/`par` and `betset`'s `uuuu`/`duuu`/`iduu` (`Want_Partial_U =
/// .false.`).
#[derive(Debug, Clone)]
pub struct DerivSetup {
    /// `npar` — number of resonance parameters.
    pub npar: usize,
    /// `mchan` — the largest channel count over the spin groups
    /// (`s2sammy`'s `mchan`; bounds the `mplus` scan in `crosss`).
    pub mchan: usize,
    /// `kstart` per spin group: 0-based offset of the group's first
    /// parameter.
    pub kstart: Vec<usize>,
    /// `npr` per spin group: `(nchan + 2) × nres`.
    pub npr: Vec<usize>,
    /// `par(ipar)` — the parameter values in `ipar` order.
    pub par: Vec<f64>,
    /// `uuuu(ipar)`, `duuu(ipar)`, `iduu(ipar)` (`samm.f90:2009-2041`).
    pub uuuu: Vec<f64>,
    pub duuu: Vec<f64>,
    pub iduu: Vec<i32>,
    /// `br(kj, ipar)` / `bi(kj, ipar)` — per parameter, over the packed
    /// triangle (`k = 1..nchan, j = 1..k`) of *its own* spin group (the
    /// only entries upstream ever sets non-zero).
    pub br: Vec<Vec<f64>>,
    pub bi: Vec<Vec<f64>>,
}

impl DerivSetup {
    /// Spin group (0-based) parameter `ipar` belongs to.
    pub fn group_of(&self, ipar: usize) -> usize {
        let mut g = 0;
        while g + 1 < self.kstart.len() && ipar >= self.kstart[g + 1] {
            g += 1;
        }
        g
    }
}

/// Build the derivative setup — `betset`'s u-parameter block
/// (`samm.f90:2009-2041`) followed by `babb` (`samm.f90:2817-2921`).
///
/// `amplitudes` must be [`crate::samm::betset::compute_resonance_amplitudes`]'s
/// output for every spin group of `section`, in order (it carries the
/// `dum` term the `duuu` factors need).
#[allow(clippy::needless_range_loop)]
pub fn deriv_setup(section: &RmlSection, amplitudes: &[Vec<ResonanceAmplitudes>]) -> DerivSetup {
    let ngroup = section.spin_groups.len();
    let mchan = section
        .spin_groups
        .iter()
        .map(|g| g.channels.len())
        .max()
        .unwrap_or(0);

    let mut kstart = Vec::with_capacity(ngroup);
    let mut npr = Vec::with_capacity(ngroup);
    let mut par = Vec::new();
    let mut uuuu = Vec::new();
    let mut duuu = Vec::new();
    let mut iduu = Vec::new();
    let mut br: Vec<Vec<f64>> = Vec::new();
    let mut bi: Vec<Vec<f64>> = Vec::new();

    for (ig, group) in section.spin_groups.iter().enumerate() {
        let mmax = group.channels.len();
        let mmax2 = mmax + 2;
        let ntriag = mmax * (mmax + 1) / 2;
        kstart.push(par.len());
        npr.push(mmax2 * group.resonances.len());

        for (ires, res) in group.resonances.iter().enumerate() {
            let amp = &amplitudes[ig][ires];
            for m in 1..=mmax2 {
                // --- betset u-parameters (samm.f90:2011-2039), Want_Partial_U false
                let mut idu = 0;
                let (u, du);
                if m == 1 {
                    u = 2.0 * res.energy.abs().sqrt();
                    du = 0.0;
                    idu = 1;
                } else if m == 2 {
                    u = 4.0 * amp.gbetpr[0].abs();
                    du = 0.0;
                } else {
                    let m2 = m - 3; // 0-based channel
                    let gam = res.channel_widths[m2];
                    if gam != 0.0 {
                        u = 2.0 * gam / amp.beta_pr[m2];
                        du = amp.dum[m2];
                    } else {
                        u = 0.0;
                        du = 0.0;
                    }
                }
                uuuu.push(u);
                duuu.push(du);
                iduu.push(idu);

                // --- babb (samm.f90:2836-2903)
                let mut brk = vec![0.0f64; ntriag];
                let mut bik = vec![0.0f64; ntriag];
                if m == 1 {
                    par.push(res.energy);
                    let d1 = 2.0 * res.energy.abs().sqrt();
                    for kj in 0..ntriag {
                        let d = amp.beta[kj] * d1;
                        brk[kj] = d;
                        bik[kj] = -d - d;
                    }
                } else if m == 2 {
                    par.push(res.gamma_gamma);
                    let d1 = amp.gbetpr[0];
                    for kj in 0..ntriag {
                        let mut d = amp.beta[kj] * d1;
                        d += d;
                        brk[kj] = -d - d;
                        bik[kj] = d;
                    }
                } else {
                    let mmm = m - 2; // 1-based channel
                    par.push(res.channel_widths[mmm - 1]);
                    let mut kj = 0usize;
                    for k in 1..=mmax {
                        for j in 1..=k {
                            kj += 1;
                            if j == mmm {
                                let mut d = amp.beta_pr[k - 1];
                                if k == mmm {
                                    d += d;
                                }
                                brk[kj - 1] = d;
                                bik[kj - 1] = d;
                            }
                        }
                        if k != mmax {
                            for j in (k + 1)..=mmax {
                                if j == mmm {
                                    let jk = (j * (j - 1)) / 2 + k;
                                    let mut d = amp.beta_pr[k - 1];
                                    if k == mmm {
                                        d += d;
                                    }
                                    brk[jk - 1] = d;
                                    bik[jk - 1] = d;
                                }
                            }
                        }
                    }
                }
                // samm.f90:2907-2917 -- double every non-zero entry
                for v in brk.iter_mut() {
                    if *v != 0.0 {
                        *v *= 2.0;
                    }
                }
                for v in bik.iter_mut() {
                    if *v != 0.0 {
                        *v *= 2.0;
                    }
                }
                br.push(brk);
                bi.push(bik);
            }
        }
    }

    DerivSetup {
        npar: par.len(),
        mchan,
        kstart,
        npr,
        par,
        uuuu,
        duuu,
        iduu,
        br,
        bi,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samm::betset::compute_resonance_amplitudes;
    use crate::samm::context;
    use crate::samm::mf2::{ParticlePair, RmlChannel, RmlResonance, SpinGroup};

    fn two_channel_section() -> RmlSection {
        let pairs = vec![
            ParticlePair {
                mass_a: 0.0,
                mass_b: 35.0,
                z_a: 0,
                z_b: 0,
                spin_a: 1.0,
                spin_b: 0.0,
                q_value: 0.0,
                penetrability_flag: 0,
                shift_flag: -1,
                mt: 102,
                parity_a: 0.0,
                parity_b: 0.0,
            },
            ParticlePair {
                mass_a: 1.0,
                mass_b: 34.6685,
                z_a: 0,
                z_b: 0,
                spin_a: 0.5,
                spin_b: 1.5,
                q_value: 0.0,
                penetrability_flag: 1,
                shift_flag: -1,
                mt: 2,
                parity_a: 0.0,
                parity_b: 0.0,
            },
        ];
        let ch = |s: f64| RmlChannel {
            particle_pair: 2,
            l: 0,
            channel_spin: s,
            boundary: 0.0,
            radius_eff: 0.3668,
            radius_true: 0.4822,
        };
        RmlSection {
            particle_pairs: pairs,
            spin_groups: vec![SpinGroup {
                j: 1.0,
                parity: 1.0,
                channels: vec![ch(1.0), ch(2.0)],
                resonances: vec![
                    RmlResonance {
                        energy: 100.0,
                        gamma_gamma: 0.5,
                        channel_widths: vec![2.0, 0.0],
                    },
                    RmlResonance {
                        energy: 300.0,
                        gamma_gamma: 0.6,
                        channel_widths: vec![1.0, 3.0],
                    },
                ],
            }],
        }
    }

    /// The parameter bookkeeping: `(nchan+2)` per resonance, `E` flagged
    /// `iduu = 1`, a zero width giving `uuuu = 0`, and `babb`'s `br`/`bi`
    /// entries in the packed triangle doubled once at the end.
    ///
    /// **Result (2026-09-11):** passes; `br` for the `Γ_1` parameter of the
    /// first resonance is `2*[2β_1, β_2, 0]` — non-zero only where `j = 1`.
    #[test]
    fn parameter_layout_and_babb_pattern() {
        let mut sec = two_channel_section();
        context::apply_particle_pair_defaults(&mut sec.particle_pairs);
        let kin = context::compute_channel_kinematics(
            &mut sec.particle_pairs,
            &sec.spin_groups,
            34.6685,
        )
        .unwrap();
        let amp = vec![compute_resonance_amplitudes(&sec.spin_groups[0], &kin[0], &sec.particle_pairs).unwrap()];
        let ds = deriv_setup(&sec, &amp);
        assert_eq!(ds.npar, 8);
        assert_eq!(ds.kstart, vec![0]);
        assert_eq!(ds.npr, vec![8]);
        assert_eq!(ds.iduu, vec![1, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(ds.par[0], 100.0);
        assert_eq!(ds.par[1], 0.5);
        assert_eq!(ds.par[2], 2.0);
        assert_eq!(ds.uuuu[0], 2.0 * 10.0);
        assert_eq!(ds.uuuu[3], 0.0, "zero width -> uuuu = 0");
        // Γ_1 of resonance 1: entries (1,1) and (2,1) of the packed triangle
        let b1 = amp[0][0].beta_pr[0];
        let b2 = amp[0][0].beta_pr[1];
        assert_eq!(ds.br[2], vec![2.0 * 2.0 * b1, 2.0 * b2, 0.0]);
        assert_eq!(ds.bi[2], ds.br[2]);
        // E of resonance 1: br = 2*beta*2sqrt(E), bi = -2*br
        let d1 = 2.0 * 10.0;
        for kj in 0..3 {
            let d = amp[0][0].beta[kj] * d1;
            assert_eq!(ds.br[0][kj], if d != 0.0 { 2.0 * d } else { 0.0 });
            assert_eq!(ds.bi[0][kj], if d != 0.0 { -4.0 * d } else { 0.0 });
        }
        assert_eq!(ds.group_of(7), 0);
    }
}
