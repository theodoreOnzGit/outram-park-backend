// Ported from NJOY2016 `src/wimsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine resint`, l.423-676 — effective resonance integrals per
//     resonance group, temperature and sigma-zero.
//   - `subroutine rsiout`, l.678-869 — the `nscr1` records (and the listing).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `resint` — the WIMS resonance tables: for each resonance group `jg`
//! (`nfg+1 ..= nfg+nrg`), temperature `it` (`1..=ires`) and sigma-zero
//! `iz`, the absorption (and ν-fission) resonance integral
//! `RI = σ_b σ_a / (σ_b + σ_a)` with `σ_b = σ_0 + λ σ_pot`, where `σ_a`
//! is the shielded group absorption (capture + fission + the non-resonant
//! `abs2` base) at that dilution. Sigma-zeros are stored **ascending**
//! (the GENDF lists them descending), the flux at the reference group
//! normalises the listing's "flux per unit lethargy".
//!
//! Upstream idioms kept: a temperature whose block carries no `MF=3/102`
//! (`abs(it) = 0`) inherits the previous temperature's absorption
//! (`wimsr.f90:601-612`); a zero flux is filled from the previous
//! temperature (l.583-599); the reference-group flux is only taken when
//! the reaction is `MT=1`.

use super::gendf::GendfMaterial;
use super::input::WimsrInput;
use super::xsecs::{Counts, XsecsResult};

/// One resonance group's tables (`rsiout`, `wimsr.f90:703-722`): the
/// `(temperatures, σ_0 + λ σ_pot, RI[it][iz])` records.
#[derive(Debug, Clone, Default)]
pub struct ResonanceGroup {
    /// `sigz(j) + siglam` for `j = 1..=nsigz` (ascending σ_0).
    pub sigb: Vec<f64>,
    /// `sabs[it][iz]` — absorption resonance integrals \[b\].
    pub absorption: Vec<Vec<f64>>,
    /// `snux[it][iz]` — ν-fission resonance integrals (when `ifiss == 3`).
    pub nu_fission: Option<Vec<Vec<f64>>>,
    /// `elas[it][iz]` — shielded elastic (WIMS-E only).
    pub elastic: Vec<Vec<f64>>,
    /// The listing's flux per unit lethargy normalised at `igref`,
    /// `[it][iz]`.
    pub flux_per_lethargy: Vec<Vec<f64>>,
}

/// `resint`'s output.
#[derive(Debug, Clone, Default)]
pub struct ResintResult {
    /// `tempr(1..=ires)`.
    pub temps: Vec<f64>,
    /// `sigz(1..=nsigz)` ascending.
    pub sigz: Vec<f64>,
    /// `ifiss` as `rsiout` receives it (`jfiss`: 1 no fission, 3 fission).
    pub jfiss: i32,
    pub groups: Vec<ResonanceGroup>,
}

/// `resint` (`wimsr.f90:423-676`). Returns `None` when `ires == 0`.
#[allow(clippy::needless_range_loop)] // Fortran index loops kept verbatim
pub fn resint(
    inp: &WimsrInput,
    mat: &GendfMaterial,
    cnt: &Counts,
    xs: &XsecsResult,
) -> Option<ResintResult> {
    let ires = cnt.ires;
    if ires == 0 {
        return None;
    }
    let nsigz = cnt.nsigz;
    let ngnd = inp.ngnd;
    let nfg = inp.nfg;
    let nrg = inp.nrg;
    let igref = inp.igref;
    let nghi = ngnd - nfg;
    let nglo = nghi + 1 - nrg;
    // [jg][it][iz], 0-based; the Fortran index i = iz + nsigz*(it-1 + ires*(jg-1))
    let mut sabs = vec![vec![vec![0.0f64; nsigz]; ires]; nrg];
    let mut snux = vec![vec![vec![0.0f64; nsigz]; ires]; nrg];
    let mut snsf = vec![vec![vec![0.0f64; nsigz]; ires]; nrg];
    let mut elas = vec![vec![vec![0.0f64; nsigz]; ires]; nrg];
    let mut flux = vec![vec![vec![0.0f64; nsigz]; ires]; nrg];
    let mut flxr = vec![vec![0.0f64; nsigz]; ires];
    let mut abs_t = vec![0.0f64; ires];
    let mut sigz = vec![0.0f64; nsigz];
    for jg in 0..nrg {
        for it in 0..ires {
            for iz in 0..nsigz {
                sabs[jg][it][iz] = xs.abs2[nfg + jg + 1];
            }
        }
    }
    let mut jfiss = 1i32;
    let mut temps = Vec::new();

    // loop over temperatures (l.469-537)
    for (jt, block) in mat.blocks.iter().enumerate() {
        let jtemp = jt + 1;
        if jtemp > ires {
            break;
        }
        temps.push(block.temp);
        for i in 1..=nsigz {
            sigz[nsigz - i] = block.sigz.get(i - 1).copied().unwrap_or(0.0);
        }
        abs_t[jt] = 0.0;
        for sec in &block.sections {
            let mth = sec.mt;
            let mfh = sec.mf;
            let nl = sec.nl as i64;
            let nz = sec.nz as i64;
            for rec in &sec.records {
                // l.485-493: the reaction filter (applies per record, same result)
                if (18..=21).contains(&mth) && inp.inorf > 0 {
                    break;
                }
                if mth == 38 && inp.inorf > 0 {
                    break;
                }
                if mfh != 3 {
                    break;
                }
                let keep = mth == 1
                    || mth == 2
                    || ((18..=150).contains(&mth) && !(mth > 21 && mth < 102 && mth != 38));
                if !keep {
                    break;
                }
                let ig = rec.ig as i64;
                let in_ref = mth == 1 && ig == (ngnd - igref + 1) as i64;
                let in_res = ig >= nglo as i64 && ig <= nghi as i64;
                if !(in_ref || in_res) {
                    continue; // 290
                }
                let kg = (ngnd as i64 - ig + 1) as usize;
                let jg = kg as i64 - nfg as i64; // 1-based resonance group, may be out of range
                let lim = nsigz.min(nz.max(0) as usize);
                if mth != 1 && mth != 2 && mth != 18 && mth != 102 {
                    break; // 300
                }
                let d = |off: i64| rec.data.get(off as usize).copied().unwrap_or(0.0);
                let jgu = if (1..=nrg as i64).contains(&jg) {
                    Some((jg - 1) as usize)
                } else {
                    None
                };
                match mth {
                    102 => {
                        if let Some(jgu) = jgu {
                            for jz in 1..=lim {
                                // sabs(iadd-jz+1) += scr(nl*jz+loca), loca = l+lz+nl*(nz-1)
                                sabs[jgu][jt][nsigz - jz] += d(nl * jz as i64 + nl * (nz - 1));
                            }
                        }
                        abs_t[jt] = 1.0;
                    }
                    1 => {
                        if let Some(jgu) = jgu {
                            for jz in 1..=lim {
                                // flux(loc-jz) = scr(nl*jz+loca), loca = l+lz-nl
                                flux[jgu][jt][nsigz - jz] += d(nl * jz as i64 - nl);
                            }
                        }
                        if kg == igref {
                            for jz in 1..=lim {
                                flxr[jt][nsigz - jz] = d(nl * (jz as i64 - 1));
                            }
                        }
                    }
                    18 => {
                        if let Some(jgu) = jgu {
                            for jz in 1..=lim {
                                let v = d(nl * jz as i64 + nl * (nz - 1));
                                snsf[jgu][jt][nsigz - jz] += v;
                                sabs[jgu][jt][nsigz - jz] += v;
                            }
                        }
                        jfiss = 3;
                    }
                    2 => {
                        if let Some(jgu) = jgu {
                            for jz in 1..=lim {
                                elas[jgu][jt][nsigz - jz] += d(nl * jz as i64 + nl * (nz - 1));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // note: flux(...) upstream is *assigned* per record; a repeated MT=1
    // record for the same group does not occur on a GENDF, so the `+=`
    // above on a zeroed array is the same assignment.

    // nu times sigma f (l.568-580)
    if jfiss != 1 {
        for jg in 0..nrg {
            let locn = nfg + jg + 1;
            for it in 0..ires {
                for iz in 0..nsigz {
                    snux[jg][it][iz] = snsf[jg][it][iz] * xs.snu[locn];
                }
            }
        }
    }
    // temperature dependence fill-in (l.583-599)
    for iz in 0..nsigz {
        for it in 0..ires {
            if flxr[it][iz] == 0.0 && it > 0 {
                flxr[it][iz] = flxr[it - 1][iz];
            }
        }
    }
    for jg in 0..nrg {
        for iz in 0..nsigz {
            for it in 0..ires {
                if flux[jg][it][iz] == 0.0 && it > 0 {
                    flux[jg][it][iz] = flux[jg][it - 1][iz];
                }
            }
        }
    }
    // non-temperature-dependent absorption (l.602-612)
    if ires != 1 {
        for it in 0..ires {
            if abs_t[it] <= 0.0 && it > 0 {
                for jg in 0..nrg {
                    for jz in 0..nsigz {
                        let prev = sabs[jg][it - 1][jz];
                        sabs[jg][it][jz] += prev;
                    }
                }
            }
        }
    }
    // convert to resonance integrals (l.615-627)
    let mut groups = Vec::with_capacity(nrg);
    for jg in 0..nrg {
        let siglam = xs.spot[nfg + jg + 1] * inp.glam[jg];
        let mut g = ResonanceGroup {
            sigb: (0..nsigz).map(|j| sigz[j] + siglam).collect(),
            ..Default::default()
        };
        for it in 0..ires {
            for iz in 0..nsigz {
                let sigb = sigz[iz] + siglam;
                let siga = sabs[jg][it][iz];
                let sig = snux[jg][it][iz];
                sabs[jg][it][iz] = sigb * siga / (sigb + siga);
                snux[jg][it][iz] = sigb * sig / (sigb + siga);
            }
        }
        g.absorption = sabs[jg].clone();
        if jfiss == 3 {
            g.nu_fission = Some(snux[jg].clone());
        }
        g.elastic = elas[jg].clone();
        // listing: flux per unit lethargy normalised at igref (l.764-772)
        let lg = (mat.egb[igref - 1] / mat.egb[igref]).ln();
        let lj = (mat.egb[nfg + jg] / mat.egb[nfg + jg + 1]).ln();
        g.flux_per_lethargy = (0..ires)
            .map(|it| {
                (0..nsigz)
                    .map(|iz| {
                        let refflx = flxr[it][iz] / lg;
                        flux[jg][it][iz] / lj / refflx
                    })
                    .collect()
            })
            .collect();
        groups.push(g);
    }
    Some(ResintResult {
        temps,
        sigz,
        jfiss,
        groups,
    })
}
