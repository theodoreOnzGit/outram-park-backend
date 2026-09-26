//! Reich-Moore (LRF=3) cross-section evaluation.
//!
//! Implements the zero-temperature R-matrix cross sections from `csrmat` in
//! NJOY2016 `reconr.f90` (lines 3199–3501).
//!
//! ## Physical model
//!
//! For each l-state, for each J value, the resonances with that J are used to
//! build a 3×3 complex R-function matrix (neutron, fission-A, fission-B channels):
//!
//! ```text
//! R(i,j) =  Σ_res  (Γ_γ/4)/den × aᵢ × aⱼ
//! S(i,j) = -Σ_res  (E_r-E)/(2·den) × aᵢ × aⱼ
//! den = (E_r − E)² + (Γ_γ/2)²
//! a₁ = √(Γ_n × P_l(E) / P_l(E_r))
//! a₂ = √|Γ_fA| × sign(Γ_fA)
//! a₃ = √|Γ_fB| × sign(Γ_fB)
//! ```
//!
//! The S-matrix is obtained by inverting the complex matrix `(I + R + iS)`
//! (Frobenius-Schur method), and cross sections follow from `|S|²` terms.
//! For nuclides with no fission widths the 3×3 inversion reduces to a scalar.

use crate::{
    common::phys::PI,
    reconr::{
        mf2::RmLState,
        slbw::{channel_radius, phase_shift, shift_and_penetrability, WAVE_K},
    },
};

// ── Return type ────────────────────────────────────────────────────────────────

/// Cross-section contributions from one Reich-Moore evaluation \[barns\].
#[derive(Debug, Clone, Copy, Default)]
pub struct RmSigmas {
    /// Total \[b\].
    pub total: f64,
    /// Elastic scattering \[b\].
    pub elastic: f64,
    /// Fission (summed over both fission channels) \[b\].
    pub fission: f64,
    /// Radiative capture \[b\].
    pub capture: f64,
}

// ── Matrix helpers (ported from frobns / thrinv / abcmat in reconr.f90) ───────

/// In-place inversion of a symmetric 3×3 real matrix using the modified
/// Gauss-Jordan algorithm (`thrinv` in reconr.f90, lines 3539–3587).
///
/// Returns `false` if the matrix is singular.
fn thrinv(d: &mut [[f64; 3]; 3]) -> bool {
    for j in 0..3 {
        for i in 0..=j {
            d[i][j] = -d[i][j];
            d[j][i] = d[i][j];
        }
        d[j][j] = 1.0 + d[j][j];
    }
    for lr in 0..3 {
        let fooey = 1.0 - d[lr][lr];
        if fooey == 0.0 {
            return false;
        }
        d[lr][lr] = 1.0 / fooey;
        let mut s = [0.0f64; 3];
        for j in 0..3 {
            s[j] = d[lr][j];
            if j != lr {
                d[j][lr] *= d[lr][lr];
                d[lr][j] = d[j][lr];
            }
        }
        for j in 0..3 {
            if j != lr {
                for i in 0..=j {
                    if i != lr {
                        d[i][j] += d[i][lr] * s[j];
                        d[j][i] = d[i][j];
                    }
                }
            }
        }
    }
    true
}

/// 3×3 matrix multiplication `c = a × b` (`abcmat` in reconr.f90).
fn mat_mul_3x3(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut c = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                c[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    c
}

/// Invert the complex 3×3 matrix `A + i·B`, returning `(C, D)` such that
/// `(A + iB)(C + iD) = I` (`frobns` in reconr.f90, Frobenius-Schur method).
///
/// Returns identity/zeros on singular input (defensive, should not occur for
/// physical resonance parameters).
fn frobns(mut a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> ([[f64; 3]; 3], [[f64; 3]; 3]) {
    let a_orig = a;
    if !thrinv(&mut a) {
        return ([[0.0; 3]; 3], [[0.0; 3]; 3]);
    }
    let q = mat_mul_3x3(&a, &b);
    let d_tmp = mat_mul_3x3(&b, &q);
    let mut c_mat = a_orig;
    for i in 0..3 {
        for j in 0..3 {
            c_mat[i][j] += d_tmp[i][j];
        }
    }
    if !thrinv(&mut c_mat) {
        return ([[0.0; 3]; 3], [[0.0; 3]; 3]);
    }
    let mut d_mat = mat_mul_3x3(&q, &c_mat);
    for i in 0..3 {
        for j in 0..3 {
            d_mat[i][j] = -d_mat[i][j];
        }
    }
    (c_mat, d_mat)
}

// ── Channel spin / kkkkkk logic ────────────────────────────────────────────────

/// Action code for one (J, channel-spin) contribution.
///
/// - 0 → skip
/// - 1 → resonance contribution only
/// - 2 → resonance + hard-sphere potential scattering
///
/// Ported from the `kkkkkk` block in `csrmat` (reconr.f90 ~3386–3425).
fn kkkkkk_code(
    kchanl: usize,
    kpstv: usize,
    kngtv: usize,
    jj: usize,
    jjl: usize,
    numj: usize,
) -> u32 {
    let interior = jj > jjl && jj < numj;
    match kchanl {
        1 => {
            if kpstv > 0 {
                if kngtv == 0 {
                    if interior {
                        2
                    } else {
                        1
                    }
                } else {
                    1
                }
            } else if kngtv == 0 {
                if interior {
                    2
                } else {
                    1
                }
            } else {
                0
            }
        }
        2 => {
            if kpstv > 0 {
                if kngtv > 0 {
                    1
                } else {
                    0
                }
            } else if kngtv > 0 {
                if interior {
                    2
                } else {
                    1
                }
            } else {
                0
            }
        }
        _ => 0,
    }
}

// ── Main evaluation ────────────────────────────────────────────────────────────

/// Evaluate Reich-Moore cross sections at energy `e` \[eV\] for one l-state.
///
/// A single-l-state view of [`eval_rm_range`]; kept for callers that want the
/// contribution of one l. Summing these over l is **not** bit-identical to
/// [`eval_rm_range`], which follows upstream's accumulation order.
pub fn eval_rm_lstate(e: f64, ls: &RmLState, ap: f64, spi: f64, naps: i32) -> RmSigmas {
    eval_rm_range(e, std::slice::from_ref(ls), ap, spi, naps)
}

/// Evaluate a whole Reich-Moore range at energy `e` \[eV\], in `csrmat`'s own
/// order (`reconr.f90:3199-3501`):
///
/// - `k`, `pifac` and the formula channel radius come from the **first**
///   l-state's AWRI (`awri=res(inow+12)`);
/// - the raw terms are summed over every l and J, and `pifac` applied **once**
///   at the end;
/// - `gf`, the "some resonance has fission widths" flag, is set once and
///   **never reset** for the rest of the call, so every later J group takes
///   the 3x3 path.
///
/// ~~Per l-state, `pifac` applied per l, `gf` reset per J group.~~ CHANGED
/// 2026-09-26: those orderings differ from upstream's in the last bits, which
/// is enough to flip 7-figure rounding against NJOY2016's own tables.
pub fn eval_rm_range(e: f64, lstates: &[RmLState], ap: f64, spi: f64, naps: i32) -> RmSigmas {
    if e <= 0.0 || lstates.is_empty() {
        return RmSigmas::default();
    }
    let awri = lstates[0].awri;
    let arat = awri / (awri + 1.0);
    let ra = channel_radius(awri, naps, ap);
    let k = WAVE_K * arat * e.abs().sqrt();
    let pifac = PI / (k * k);
    let gjd = 2.0 * (2.0 * spi + 1.0);
    let mut gf = false;
    let mut sig = [0.0f64; 4]; // total, elastic, fission, capture

    for ls in lstates {
        let ll = ls.l;
        let apl = ls.apl;
        let mut rhoc = k * ap;
        let mut rho = k * ra;
        if apl != 0.0 {
            rhoc = k * apl;
        }
        if apl != 0.0 && naps == 1 {
            rho = k * apl;
        }
        let (_, pe) = shift_and_penetrability(ll, rho);
        let phi = phase_shift(ll, rhoc);
        let p1 = (2.0 * phi).cos();
        let p2 = (2.0 * phi).sin();

        // `per` as `rdf2bw` stored it (`reconr.f90:936-1010`): this l-state's
        // own AWRI and radius `ral`.
        let arat_l = ls.awri / (ls.awri + 1.0);
        let ascatl = if apl != 0.0 { apl } else { ap };
        let ral = if naps == 1 { ascatl } else { channel_radius(ls.awri, 0, ap) };

        let fl = ll as f64;
        let ajmin = ((spi - fl).abs() - 0.5).abs();
        let ajmax = spi + fl + 0.5;
        let numj = (ajmax - ajmin + 1.0).round() as usize;
        let jjl = if ll != 0 && fl > spi - 0.5 && fl <= spi { 0usize } else { 1 };

        let mut ajc = ajmin - 1.0;
        for jj in 1..=numj {
            ajc += 1.0;
            let gj = (2.0 * ajc + 1.0) / gjd;
            for kchanl in 1usize..=2 {
                let mut r = [[0.0f64; 3]; 3];
                let mut s = [[0.0f64; 3]; 3];
                let mut kpstv = 0usize;
                let mut kngtv = 0usize;
                for res in &ls.resonances {
                    if (res.aj.abs() - ajc).abs() > 0.25 {
                        continue;
                    }
                    if res.aj < 0.0 {
                        kngtv += 1;
                    }
                    if res.aj > 0.0 {
                        kpstv += 1;
                    }
                    if (kchanl == 1 && res.aj < 0.0) || (kchanl == 2 && res.aj > 0.0) {
                        continue;
                    }
                    let rho_r = WAVE_K * arat_l * res.er.abs().sqrt() * ral;
                    let (_, per) = shift_and_penetrability(ll, rho_r);
                    let a1 = (res.gn * pe / per).sqrt();
                    let mut a2 = 0.0;
                    if res.gfa != 0.0 {
                        a2 = res.gfa.abs().sqrt();
                    }
                    if res.gfa < 0.0 {
                        a2 = -a2;
                    }
                    let mut a3 = 0.0;
                    if res.gfb != 0.0 {
                        a3 = res.gfb.abs().sqrt();
                    }
                    if res.gfb < 0.0 {
                        a3 = -a3;
                    }
                    let diff = res.er - e;
                    let den = diff * diff + 0.25 * res.gg * res.gg;
                    let de2 = 0.5 * diff / den;
                    let gg4 = 0.25 * res.gg / den;
                    r[0][0] += gg4 * a1 * a1;
                    s[0][0] -= de2 * a1 * a1;
                    if res.gfa != 0.0 || res.gfb != 0.0 {
                        r[0][1] += gg4 * a1 * a2;
                        s[0][1] -= de2 * a1 * a2;
                        r[0][2] += gg4 * a1 * a3;
                        s[0][2] -= de2 * a1 * a3;
                        r[1][1] += gg4 * a2 * a2;
                        s[1][1] -= de2 * a2 * a2;
                        r[2][2] += gg4 * a3 * a3;
                        s[2][2] -= de2 * a3 * a3;
                        r[1][2] += gg4 * a2 * a3;
                        s[1][2] -= de2 * a2 * a3;
                        gf = true;
                    }
                }
                let kkk = kkkkkk_code(kchanl, kpstv, kngtv, jj, jjl, numj);
                if kkk == 0 {
                    continue;
                }
                let (mut termt, mut termn, termf);
                if gf {
                    r[0][0] = 1.0 + r[0][0];
                    r[1][1] = 1.0 + r[1][1];
                    r[2][2] = 1.0 + r[2][2];
                    r[1][0] = r[0][1];
                    s[1][0] = s[0][1];
                    r[2][0] = r[0][2];
                    s[2][0] = s[0][2];
                    r[2][1] = r[1][2];
                    s[2][1] = s[1][2];
                    let (ri, si) = frobns(r, s);
                    let (t1, t2, t3, t4) = (ri[0][1], si[0][1], ri[0][2], si[0][2]);
                    termf = 4.0 * gj * (t1 * t1 + t2 * t2 + t3 * t3 + t4 * t4);
                    let u11r = p1 * (2.0 * ri[0][0] - 1.0) + 2.0 * p2 * si[0][0];
                    let u11i = p2 * (1.0 - 2.0 * ri[0][0]) + 2.0 * p1 * si[0][0];
                    termt = 2.0 * gj * (1.0 - u11r);
                    termn = gj * ((1.0 - u11r) * (1.0 - u11r) + u11i * u11i);
                } else {
                    let dd = r[0][0];
                    let rr = 1.0 + dd;
                    let ss = s[0][0];
                    let amag = rr * rr + ss * ss;
                    let rri = rr / amag;
                    let ssi = -ss / amag;
                    let uur = p1 * (2.0 * rri - 1.0) + 2.0 * p2 * ssi;
                    let uui = p2 * (1.0 - 2.0 * rri) + 2.0 * p1 * ssi;
                    const SMALL: f64 = 3.0e-4;
                    if dd.abs() < SMALL && phi.abs() < SMALL {
                        let mut xx = 2.0 * dd;
                        xx += 2.0 * (dd * dd + ss * ss + phi * phi + p2 * ss);
                        xx -= 2.0 * phi * phi * (dd * dd + ss * ss);
                        xx /= amag;
                        termt = 2.0 * gj * xx;
                        termn = gj * (xx * xx + uui * uui);
                    } else {
                        termt = 2.0 * gj * (1.0 - uur);
                        termn = gj * ((1.0 - uur) * (1.0 - uur) + uui * uui);
                    }
                    termf = 0.0;
                }
                if kkk == 2 {
                    termn += 2.0 * gj * (1.0 - p1);
                    termt += 2.0 * gj * (1.0 - p1);
                }
                let termg = termt - termf - termn;
                sig[1] += termn;
                sig[3] += termg;
                sig[2] += termf;
                sig[0] += termt;
            }
        }
    }
    RmSigmas {
        total: pifac * sig[0],
        elastic: pifac * sig[1],
        fission: pifac * sig[2],
        capture: pifac * sig[3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconr::mf2::{RmLState, RmResonance};

    #[test]
    fn rm_non_fissile_scalar_path() {
        let ls = RmLState {
            awri: 36.6,
            apl: 0.0,
            l: 0,
            resonances: vec![RmResonance {
                er: 1000.0,
                aj: 2.0,
                gn: 10.0,
                gg: 1.0,
                gfa: 0.0,
                gfb: 0.0,
            }],
        };
        let s = eval_rm_lstate(1000.0, &ls, 0.338, 1.5, 1);
        assert!(s.elastic > 0.0, "elastic={}", s.elastic);
        assert_eq!(s.fission, 0.0);
        assert!(s.total > 0.0);
    }

    #[test]
    fn rm_fissile_matrix_path() {
        let ls = RmLState {
            awri: 235.0,
            apl: 0.0,
            l: 0,
            resonances: vec![RmResonance {
                er: 0.3,
                aj: 3.0,
                gn: 0.001,
                gg: 0.04,
                gfa: 0.05,
                gfb: 0.0,
            }],
        };
        let s = eval_rm_lstate(0.3, &ls, 0.9, 3.5, 1);
        assert!(s.elastic >= 0.0, "elastic={}", s.elastic);
        assert!(s.fission >= 0.0, "fission={}", s.fission);
    }
}
