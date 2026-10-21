// Ported from NJOY2016 `src/acepa.f90` (subroutine `alax`, lines 504-801).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `alax` — the JFLO block: fluorescence in the **Cashwell–Everett** form, and
//! the photoelectric photons it subtracts back out of the heating.
//!
//! The block is `nflo` rows of four columns — edge energy, cumulative
//! probability `phi`, yield `y`, and mean fluorescent photon energy `f` — laid
//! out **column-major** in XSS (`acepa.f90:906-909` prints
//! `xss(jflo+i-1+nflo*(j-1))`). `nflo` is fixed by `Z` alone
//! (`acepa.f90:154-165`) and never by what the relaxation data contains:
//!
//! | `Z` | `nflo` | shells represented |
//! |---|---|---|
//! | < 12 | 0 | none — no JFLO block at all |
//! | 12–19 | 2 | K only, transitions averaged |
//! | 20–30 | 4 | K with L2, L3 and the rest resolved |
//! | 31–36 | 5 | L average + K with L2, L3, K-alpha |
//! | >= 37 | 6 | as above with K-beta split off |
//!
//! Every edge's **jump ratio** `rho = sigma(0.9999 E_edge) / sigma(1.0001 E_edge)`
//! comes from MF=23/MT=522 on the photoatomic tape, not from the relaxation
//! file: the relaxation file says where the edges are and what comes off them,
//! the cross section says how much of the absorption is above each one.
//!
//! **Status.** This is a faithful translation that no data in this repository
//! exercises: `reference-data/endf/` holds no atomic-relaxation sublibrary
//! tape, so both photoatomic tapes here take upstream's `nlax = 0` path and
//! NJOY itself writes the JFLO block as zeros. The port is therefore
//! **verified against the Fortran by reading, not by running**; see
//! `verification_and_validation/acer_photoatomic_vs_njoy2016.md`. Anything
//! that depends on fluorescence should treat it as unmeasured.

use crate::endf::gety1::Gety1;
use crate::endf::records::{List, SectionCursor};
use crate::endf::tape::Tape;
use crate::NjoyError;

/// `dn` (`acepa.f90:521`) — just below an edge.
const DN: f64 = 0.9999;
/// `up` (`:522`) — just above an edge.
const UP: f64 = 1.0001;
/// `emev` (`:523`).
const EMEV: f64 = 1.0e6;

/// The number of fluorescence rows for atomic number `iz`
/// (`acepa.f90:154-165`).
pub fn nflo_for(iz: i32) -> usize {
    if iz < 12 {
        0
    } else if iz < 20 {
        2
    } else if iz < 31 {
        4
    } else if iz < 37 {
        5
    } else {
        6
    }
}

/// One MF=28 subshell record, indexed the way `alax` indexes it.
struct Subshell {
    list: List,
}

impl Subshell {
    /// `EBI` — the subshell binding energy \[eV\] (`a(jj+6)`).
    fn binding_energy(&self) -> f64 {
        self.list.data.first().copied().unwrap_or(0.0)
    }
    /// `NTR` — the transition count (`a(jj+5)` = the LIST's `N2`).
    fn ntr(&self) -> usize {
        self.list.head.n2.max(0) as usize
    }
    /// Transition `i` (1-based) as `(SUBJ, SUBK, ETR, FTR)`.
    ///
    /// Upstream reads these at `a(7+6i) .. a(10+6i)` relative to the record
    /// start, i.e. `jj = 8+6i` is `SUBK` and `jj-1` is `SUBJ`
    /// (`acepa.f90:604-612`). `SUBK = 0` marks a **radiative** transition —
    /// the ones that produce a fluorescent photon.
    fn transition(&self, i: usize) -> (i32, i32, f64, f64) {
        let b = 6 * i; // data[] is 0-based from EBI, so transition i starts at 6i
        let d = &self.list.data;
        let g = |k: usize| d.get(b + k).copied().unwrap_or(0.0);
        (g(0).round() as i32, g(1).round() as i32, g(2), g(3))
    }
}

/// Build the JFLO block and subtract the fluorescent photon energy from
/// `heat`.
///
/// * `relax` — the atomic-relaxation tape (MF=28/MT=533 for `mat`).
/// * `photo` — the photoatomic tape, for MF=23/MT=522.
/// * `ener` — the ACE energy grid \[MeV\], ascending.
/// * `photoelectric` — MT=522 at those energies \[barn\].
/// * `heat` — the running heating column \[MeV·barn\], modified in place.
///
/// Returns the `4 * nflo` XSS words, column-major.
///
/// # Errors
/// [`NjoyError::EndfParse`] when the relaxation tape has no MF=28/MT=533 for
/// `mat`, when it carries fewer subshells than the `Z` branch needs, or when a
/// branch's radiative probabilities sum to zero (upstream divides by that sum
/// unguarded and would return `NaN`).
pub fn alax(
    relax: &Tape,
    photo: &Tape,
    mat: i32,
    iz: i32,
    ener: &[f64],
    photoelectric: &[f64],
    heat: &mut [f64],
) -> Result<Vec<f64>, NjoyError> {
    let nflo = nflo_for(iz);
    if nflo == 0 {
        return Ok(Vec::new());
    }
    let nes = ener.len();

    // ── the relaxation file ─────────────────────────────────────────────────
    let sec = relax.section(mat, 28, 533).ok_or_else(|| {
        NjoyError::EndfParse(format!(
            "alax: the relaxation tape has no MF=28/MT=533 for MAT {mat}"
        ))
    })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let nss = head.n1.max(0) as usize;
    let mut shells: Vec<Subshell> = Vec::with_capacity(nss);
    for _ in 0..nss {
        shells.push(Subshell {
            list: cur.read_list()?,
        });
    }
    let need = if iz > 30 { 4 } else { 1 };
    if shells.len() < need {
        return Err(NjoyError::EndfParse(format!(
            "alax: Z = {iz} needs {need} subshells from MF=28, tape has {}",
            shells.len()
        )));
    }

    // ── the photoionisation cross section, for the edge jump ratios ─────────
    let pe = photo.section(mat, 23, 522).ok_or_else(|| {
        NjoyError::EndfParse(format!(
            "alax: the photoatomic tape has no MF=23/MT=522 for MAT {mat}"
        ))
    })?;
    let mut pc = SectionCursor::new(&pe.rows);
    pc.read_cont()?;
    let t522 = pc.read_tab1()?;
    let mut g = Gety1::new(&t522);
    let jump = |g: &mut Gety1, edge: f64| -> f64 {
        let lo = g.get(DN * edge).y;
        let hi = g.get(UP * edge).y;
        if hi == 0.0 {
            0.0
        } else {
            lo / hi
        }
    };

    // `enl`/`rhol` for L1, L2, L3, queried in ascending energy (L3 first).
    let mut enl = [0.0f64; 3];
    let mut rhol = [0.0f64; 3];
    if iz > 30 {
        for i in 1..=3usize {
            let sh = &shells[4 - i]; // loc(5-i), 1-based -> 0-based 4-i
            enl[3 - i] = sh.binding_energy();
            rhol[3 - i] = jump(&mut g, enl[3 - i]);
        }
    }
    // The K edge (`:576-581`).
    let k = &shells[0];
    let ek = k.binding_energy();
    let rhok = jump(&mut g, ek);

    let zero_sum = |what: &str| {
        NjoyError::EndfParse(format!(
            "alax: Z = {iz}: the {what} radiative probabilities sum to zero, so \
             upstream's mean transition energy would be 0/0"
        ))
    };

    let mut fluor = vec![0.0f64; 4 * nflo];
    // Column-major setter: row `r` (0-based), column `c` (0-based).
    macro_rules! set {
        ($r:expr, $c:expr, $v:expr) => {
            fluor[$r + nflo * $c] = $v
        };
    }

    if (12..20).contains(&iz) {
        // ── 11 < Z < 20: every K transition averaged (`:586-611`) ───────────
        let (mut sum1, mut sum2) = (0.0f64, 0.0f64);
        for i in 1..=k.ntr() {
            let (_subj, subk, etr, ftr) = k.transition(i);
            if subk == 0 {
                sum1 += ftr;
                sum2 += etr * ftr;
            }
        }
        if sum1 == 0.0 {
            return Err(zero_sum("K-shell"));
        }
        sum2 /= sum1;
        for i in 0..nes {
            if ener[i] > ek / EMEV {
                heat[i] -= photoelectric[i] * sum1 * sum2 / EMEV;
            }
        }
        set!(0, 0, ek / EMEV);
        set!(1, 0, ek / EMEV);
        set!(0, 1, rhok);
        set!(1, 1, 1.0);
        set!(0, 2, 0.0);
        set!(1, 2, (1.0 - rhok) * sum1);
        set!(0, 3, 0.0);
        set!(1, 3, sum2 / EMEV);
    } else if (20..31).contains(&iz) {
        // ── 19 < Z < 31: L2, L3 resolved, the rest averaged (`:613-671`) ────
        let (mut el2, mut pl2, mut el3, mut pl3) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let (mut sum1, mut sum2) = (0.0f64, 0.0f64);
        for i in 1..=k.ntr() {
            let (subj, subk, etr, ftr) = k.transition(i);
            if subk != 0 {
                continue;
            }
            if subj == 3 {
                el2 = etr;
                pl2 = ftr;
            } else if subj == 4 {
                el3 = etr;
                pl3 = ftr;
            } else if subj > 4 {
                sum1 += ftr;
                sum2 += etr * ftr;
            }
        }
        if sum1 == 0.0 {
            return Err(zero_sum("K-to-M-and-above"));
        }
        sum2 /= sum1;
        for i in 0..nes {
            if ener[i] > ek / EMEV {
                heat[i] -= photoelectric[i] * (sum1 * sum2 + el2 * pl2 + el3 * pl3) / EMEV;
            }
        }
        let tot = (pl2 + pl3 + sum1) / (1.0 - rhok);
        let mut y = 0.0f64;
        let mut phi = rhok;
        set!(0, 0, ek / EMEV);
        set!(0, 1, phi);
        set!(0, 2, y);
        set!(0, 3, 0.0);
        phi += pl3 / tot;
        y += (1.0 - rhok) * pl3;
        set!(1, 0, ek / EMEV);
        set!(1, 1, phi);
        set!(1, 2, y);
        set!(1, 3, el3 / EMEV);
        phi += pl2 / tot;
        y += (1.0 - rhok) * pl2;
        set!(2, 0, ek / EMEV);
        set!(2, 1, phi);
        set!(2, 2, y);
        set!(2, 3, el2 / EMEV);
        y += (1.0 - rhok) * sum1;
        set!(3, 0, ek / EMEV);
        set!(3, 1, 1.0);
        set!(3, 2, y);
        set!(3, 3, sum2 / EMEV);
    } else {
        // ── Z >= 31: an averaged L line plus the resolved K lines (`:673-799`)
        let rholt = rhol[0] * rhol[1] * rhol[2];
        let elav = (enl[0] + enl[1] + enl[2]) / 3.0;
        // Weights from the successive L jumps (`:676-684`).
        let mut wtl = [0.0f64; 3];
        wtl[0] = 1.0 / rhol[0];
        wtl[1] = wtl[0] / rhol[1];
        wtl[2] = wtl[1] / rhol[2];
        let denom = wtl[2] - 1.0;
        wtl[2] = (wtl[2] - wtl[1]) / denom;
        wtl[1] = (wtl[1] - wtl[0]) / denom;
        wtl[0] = (wtl[0] - 1.0) / denom;
        // Average yield and energy of L fluorescence (`:686-702`).
        let (mut sum1, mut sum2) = (0.0f64, 0.0f64);
        for iss in 1..=3usize {
            let sh = &shells[iss]; // loc(2..4) -> 0-based 1..3
            let wt = wtl[iss - 1];
            for i in 1..=sh.ntr() {
                let (_subj, subk, etr, ftr) = sh.transition(i);
                if subk == 0 {
                    sum1 += ftr * wt;
                    sum2 += etr * ftr * wt;
                }
            }
        }
        if sum1 == 0.0 {
            return Err(zero_sum("L-shell"));
        }
        sum2 /= sum1;
        let ylt = sum1;
        let flt = sum2;
        // K-alpha1/2 and K-beta1/2 (`:709-742`).
        let (mut el2, mut pl2, mut el3, mut pl3) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let (mut sum11, mut sum12, mut sum21, mut sum22) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for i in 1..=k.ntr() {
            let (subj, subk, etr, ftr) = k.transition(i);
            if subk != 0 {
                continue;
            }
            match subj {
                3 => {
                    el2 = etr;
                    pl2 = ftr;
                }
                4 => {
                    el3 = etr;
                    pl3 = ftr;
                }
                5..=9 => {
                    sum11 += ftr;
                    sum12 += etr * ftr;
                }
                10..=16 => {
                    sum21 += ftr;
                    sum22 += etr * ftr;
                }
                _ => {}
            }
        }
        if iz >= 37 {
            if sum21 == 0.0 {
                return Err(zero_sum("K-beta"));
            }
            sum22 /= sum21;
        } else {
            sum11 += sum21;
            sum12 += sum22;
            sum21 = 0.0;
        }
        if sum11 == 0.0 {
            return Err(zero_sum("K-alpha"));
        }
        sum12 /= sum11;
        for i in 0..nes {
            if ener[i] > elav / EMEV {
                heat[i] -= photoelectric[i] * ylt * flt / EMEV;
            }
            if ener[i] > ek / EMEV {
                heat[i] -= photoelectric[i]
                    * (sum11 * sum12 + sum21 * sum22 + el2 * pl2 + el3 * pl3)
                    / EMEV;
            }
        }
        set!(0, 0, elav / EMEV);
        set!(0, 1, rholt);
        set!(0, 2, 0.0);
        set!(0, 3, 0.0);
        let mut y = (1.0 - rholt) * ylt;
        set!(1, 0, elav / EMEV);
        set!(1, 1, 1.0);
        set!(1, 2, y);
        set!(1, 3, flt / EMEV);
        let phik = 1.0 / rhok - 1.0;
        let tot = (pl2 + pl3 + sum11 + sum21) / phik;
        let mut phi = 1.0f64;
        phi += pl3 / tot;
        y += phik * pl3;
        set!(2, 0, ek / EMEV);
        set!(2, 1, phi);
        set!(2, 2, y);
        set!(2, 3, el3 / EMEV);
        phi += pl2 / tot;
        y += phik * pl2;
        set!(3, 0, ek / EMEV);
        set!(3, 1, phi);
        set!(3, 2, y);
        set!(3, 3, el2 / EMEV);
        phi += sum11 / tot;
        y += phik * sum11;
        set!(4, 0, ek / EMEV);
        set!(4, 1, phi);
        set!(4, 2, y);
        set!(4, 3, sum12 / EMEV);
        if iz >= 37 {
            phi += sum21 / tot;
            y += phik * sum21;
            set!(5, 0, ek / EMEV);
            set!(5, 1, phi);
            set!(5, 2, y);
            set!(5, 3, sum22 / EMEV);
        }
    }
    Ok(fluor)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `nflo` table is what fixes the JFLO block's length, and the length
    /// is what every locator above it depends on.
    #[test]
    fn nflo_follows_upstreams_z_bands() {
        assert_eq!(nflo_for(1), 0);
        assert_eq!(nflo_for(11), 0);
        assert_eq!(nflo_for(12), 2);
        assert_eq!(nflo_for(19), 2);
        assert_eq!(nflo_for(20), 4);
        assert_eq!(nflo_for(30), 4);
        assert_eq!(nflo_for(31), 5);
        assert_eq!(nflo_for(36), 5);
        assert_eq!(nflo_for(37), 6);
        assert_eq!(nflo_for(92), 6);
    }
}
