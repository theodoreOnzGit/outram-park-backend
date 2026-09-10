// Ported from NJOY2016 `src/dtfr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The full DTFR accumulation loop for one material, dilution and
//! temperature (`dtfr.f90:275-575`): every neutron table `il = 1..nlmax`
//! and every photon table `ip = 1..nptabl`, walking the GENDF sections in
//! tape order.
//!
//! Per neutron pass (`:281-575`): MF=3 totals into `iptotl` (and the P0
//! absorption seed), the capture/fission self-shielding factors
//! `fcap/ffis = sigma(jz)/sigma(inf)` (`:352-361`), the user edits
//! (`:365-382`), the thermal corrections (`:385-395`), the transfer
//! matrices packed into the reduced band (`:398-427`), and — P0 only — the
//! fission matrices into `nu*sigma_f` and `chi` (`:430-487`): full rows add
//! `flux*nu sigma_f` to the sink groups, the constant-spectrum rows below
//! `econst` (`ig2lo = 0`) accumulate `cnm` and, after the last group, spread
//! it with the `ig = 0` spectrum record; delayed data (`MT=455`) feeds
//! `nu_d sigma_f` into the same position, `dnorm`, and the `chid`/`chi`
//! positions from the MF=5 spectra (`:489-520`). `chi` is normalised by
//! `cnorm` at the end (`:559-565`). The photon pass (`:522-556`) fills an
//! `ngp x ng` table, scaled by the reaction's self-shielding factor.
//!
//! Two upstream index conventions are replicated literally because the
//! oracle (`tests/dtfr_u238_claw_full_golden.rs`) contains them: the
//! `ig = 0` spectrum is taken from the record's first `ng` raw words
//! (`:451-456`) even when the record carries `nz > 1` copies, and the
//! constant-spectrum production word is `a(lz+il+nl*(jz-1)+jz)` (`:471`).

use crate::dtfr::gendf::{read_header, read_section_records, GendfGroupRecord};
use crate::dtfr::input::{DtfrInput, EditSpec};
use crate::dtfr::table::{dtf_group, DtfTable};
use crate::endf::tape::Tape;
use crate::NjoyError;

/// A photon-production table: `sig(igp + ngp*(jg-1))`, DTF photon group
/// `igp` (1 = highest energy) by DTF neutron group `jg` (`:538-551`).
#[derive(Debug, Clone, PartialEq)]
pub struct PhotonTable {
    pub ng: i32,
    pub ngp: i32,
    pub sig: Vec<f64>,
}

impl PhotonTable {
    pub fn get(&self, igp: i32, jg: i32) -> f64 {
        self.sig[(igp - 1 + self.ngp * (jg - 1)) as usize]
    }
    fn add(&mut self, igp: i32, jg: i32, v: f64) {
        self.sig[(igp - 1 + self.ngp * (jg - 1)) as usize] += v;
    }
}

/// Every table DTFR writes for one material card.
#[derive(Debug, Clone, PartialEq)]
pub struct DtfrTables {
    /// Neutron tables for `il = 1..=nlmax` (index 0 is P0).
    pub neutron: Vec<DtfTable>,
    /// `ilmax`: the highest `il` any transfer matrix carried (`:405`).
    pub ilmax: i32,
    /// Photon tables for `ip = 1..=nptabl`.
    pub photon: Vec<PhotonTable>,
    /// `ipmax` (`:541`).
    pub ipmax: i32,
    /// `ids(ned+2)`: a fission channel set `nu*sigma_f` (`:435`, `:495`).
    pub has_fission: bool,
}

/// One material's sections in tape order with their `(mf, mt, nl, nz)`.
struct Sec {
    mf: i32,
    mt: i32,
    nl: i32,
    nz: i32,
    records: Vec<GendfGroupRecord>,
}

fn load_sections(tape: &Tape, mat: i32) -> Result<Vec<Sec>, NjoyError> {
    let mut out = Vec::new();
    for s in tape.sections() {
        if s.key.mat != mat || s.key.mf == 1 {
            continue;
        }
        let records = read_section_records(tape, mat, s.key.mf, s.key.mt)?;
        let (nl, nz) = records.first().map(|r| (r.nl, r.nz)).unwrap_or((1, 1));
        out.push(Sec {
            mf: s.key.mf,
            mt: s.key.mt,
            nl,
            nz,
            records,
        });
    }
    Ok(out)
}

/// `a(lz + w)` for a 1-based raw data word `w` (0 when past the record).
fn raw(rec: &GendfGroupRecord, w: i32) -> f64 {
    if w >= 1 {
        rec.data.get((w - 1) as usize).copied().unwrap_or(0.0)
    } else {
        0.0
    }
}

/// Assemble every DTFR table for `(mat, jz)` from a GENDF tape — the
/// `:275-575` loop for one material card. `jz` is the 1-based sigma-zero
/// index (`jsigz`); a section with fewer dilutions uses `jz = 1` (`:302`).
///
/// # Errors
/// [`NjoyError::EndfParse`] when the GENDF group counts disagree with the
/// deck (`:216-221`), or on a truncated record.
pub fn assemble_tables(
    tape: &Tape,
    mat: i32,
    deck: &DtfrInput,
    jz: i32,
) -> Result<DtfrTables, NjoyError> {
    let header = read_header(tape, mat)?;
    let n = &deck.neutron;
    let ng = n.ng;
    if header.ngn != ng {
        return Err(NjoyError::EndfParse(
            "dtfr: number of neutron groups disagrees with number requested".into(),
        ));
    }
    let ngp = deck.ngp;
    if deck.nptabl > 0 && header.egg.len() as i32 - 1 != ngp {
        return Err(NjoyError::EndfParse(
            "dtfr: number of gamma groups disagrees with number requested".into(),
        ));
    }
    let (iptotl, ipingp, itabl) = (n.iptotl, n.ipingp, n.itabl);
    let jgthrm = ng + 1 - n.ntherm;
    let (mti, mtc) = deck
        .thermal
        .as_ref()
        .map(|t| (t.mti, t.mtc))
        .unwrap_or((0, 0));
    // kpos/lpos: the chi (MT=470) and delayed-chi (MT=471) edit positions (`:730-733`).
    let kpos = deck
        .edits
        .iter()
        .find(|e| e.mt == 470)
        .map(|e| e.jpos)
        .unwrap_or(0);
    let lpos = deck
        .edits
        .iter()
        .find(|e| e.mt == 471)
        .map(|e| e.jpos)
        .unwrap_or(0);
    let edits: &[EditSpec] = &deck.edits;
    let secs = load_sections(tape, mat)?;

    let ngu = ng as usize;
    let mut ffis = vec![1.0f64; ngu + 1];
    let mut fcap = vec![1.0f64; ngu + 1];
    let mut spect = vec![0.0f64; ngu.max(ngp as usize) + 1];
    let (mut cnorm, mut cnm, mut dnorm) = (0.0f64, 0.0f64, 0.0f64);
    let mut ilmax = 0;
    let mut has_fission = false;
    let mut neutron = Vec::new();

    for il in 1..=n.nlmax {
        let mut sig = DtfTable::new(ng, itabl);
        for sec in &secs {
            if il > sec.nl {
                continue; // `:298`, `:308-309`
            }
            let jz = if jz > sec.nz { 1 } else { jz };
            let nl = sec.nl;
            for rec in &sec.records {
                let ig = rec.ig;
                let ng2 = rec.ng2;
                let ig2lo = rec.ig2lo;
                // `:324`: delayed neutrons.
                if il == 1 && sec.mt == 455 {
                    if sec.mf == 5 {
                        // Label 440.
                        for kg in 2..=ng2 {
                            let jg = ng - ig2lo - kg + 3;
                            if jg < 1 || jg > ng {
                                continue;
                            }
                            for id in 1..=nl {
                                let a = raw(rec, id + nl * (kg - 1));
                                if lpos > 0 {
                                    sig.add(lpos, jg, a);
                                }
                                if kpos > 0 {
                                    if dnorm == 0.0 {
                                        return Err(NjoyError::EndfParse(
                                            "dtfr: delayed nubar required to add delayed chi to total"
                                                .into(),
                                        ));
                                    }
                                    sig.add(kpos, jg, dnorm * a);
                                    cnorm += dnorm * a;
                                }
                            }
                        }
                        continue;
                    }
                    // Label 400 (MF=3): [flux, nu_d, sigma_f].
                    let jg = dtf_group(ig, ng);
                    if jg >= 1 && jg <= ng {
                        let f = ffis[jg as usize];
                        sig.add(iptotl - 1, jg, raw(rec, 2) * raw(rec, 3) * f);
                        has_fission = true;
                        dnorm += raw(rec, 1) * raw(rec, 2) * raw(rec, 3) * f;
                    }
                    // then `go to 200`
                }
                if sec.mf == 3 || sec.mf == 23 {
                    // Label 200.
                    let jg = dtf_group(ig, ng);
                    if jg < 1 || jg > ng {
                        continue;
                    }
                    let mt = sec.mt;
                    let mut do_edits = n.ned > 0;
                    if mt == 1 || mt == 501 {
                        // Label 210.
                        let total = rec.value(il, jz, 2);
                        sig.set(iptotl, jg, total);
                        if il > 1 {
                            continue;
                        }
                        sig.add(iptotl - 2, jg, total);
                        if n.ned == 0 {
                            continue;
                        }
                    } else if mt == 102 || mt == 18 {
                        // Label 220: self-shielding factors.
                        let inf = rec.value(il, 1, 2);
                        let shielded = rec.value(il, jz, 2);
                        let f = if inf != 0.0 { shielded / inf } else { 1.0 };
                        if mt == 18 {
                            ffis[jg as usize] = f;
                        } else {
                            fcap[jg as usize] = f;
                        }
                    }
                    if do_edits {
                        // Label 240.
                        if il > 1 {
                            continue;
                        }
                        for e in edits {
                            if mt == 1 && e.mt == 300 {
                                sig.add(e.jpos, jg, rec.value(il, jz, 1) * f64::from(e.mult));
                            } else if mt == e.mt {
                                sig.add(e.jpos, jg, f64::from(e.mult) * rec.value(il, jz, 2));
                            }
                        }
                        do_edits = false;
                    }
                    let _ = do_edits;
                    // Label 260: thermal correction for total and absorption.
                    if il > 1 || (mt != 2 && mt != mti && mt != mtc) || jg < jgthrm {
                        continue;
                    }
                    let v = rec.value(il, jz, 2);
                    if mt == 2 {
                        sig.add(iptotl, jg, -v);
                        sig.add(iptotl - 2, jg, -v);
                    } else {
                        sig.add(iptotl, jg, v);
                        sig.add(iptotl - 2, jg, v);
                    }
                } else if sec.mf == 6 || sec.mf == 26 {
                    let mt = sec.mt;
                    if matches!(mt, 18 | 19 | 20 | 21 | 38) {
                        // Label 340: fission matrices, P0 only.
                        if il > 1 {
                            continue;
                        }
                        has_fission = true;
                        if ig == 0 {
                            // Label 360: the constant spectrum, first ng raw words.
                            for k in 1..=ng {
                                let jg = ng - k + 1;
                                spect[jg as usize] = 0.0;
                                if k >= ig2lo && k <= ig2lo + ng2 - 1 {
                                    spect[jg as usize] = raw(rec, k - ig2lo + 1);
                                }
                            }
                            continue;
                        }
                        let jg = dtf_group(ig, ng);
                        if ig2lo == 0 {
                            // Label 380: constant-spectrum row.
                            if kpos == 0 {
                                continue;
                            }
                            if jg >= 1 && jg <= ng {
                                let mut sss = raw(rec, il + nl * (jz - 1) + jz);
                                let flux = raw(rec, il);
                                if mt == 18 || mt == 19 {
                                    sss *= ffis[jg as usize];
                                }
                                sig.add(iptotl - 1, jg, sss);
                                cnm += sss * flux;
                            }
                        } else if jg >= 1 && jg <= ng {
                            for k in 2..=ng2 {
                                let mut sss = rec.value(il, jz, k);
                                if mt == 18 || mt == 19 {
                                    sss *= ffis[jg as usize];
                                }
                                sig.add(iptotl - 1, jg, sss);
                                if kpos > 0 {
                                    let flux = rec.value(il, jz, 1);
                                    let jg2 = ng - ig2lo - k + 3;
                                    if jg2 >= 1 && jg2 <= ng {
                                        sig.add(kpos, jg2, flux * sss);
                                    }
                                    cnorm += flux * sss;
                                }
                            }
                        }
                        // Label 390.
                        if ig >= ng && cnm != 0.0 {
                            for k in 1..=ng {
                                sig.add(kpos, k, cnm * spect[k as usize]);
                                cnorm += cnm * spect[k as usize];
                            }
                        }
                        continue;
                    }
                    // Label 300/305: transfer matrices.
                    ilmax = il;
                    let jg = dtf_group(ig, ng);
                    if jg < 1 || jg > ng {
                        continue;
                    }
                    if mt == 2 && jg >= jgthrm {
                        continue;
                    }
                    if (200..=250).contains(&mt) {
                        if jg < jgthrm {
                            continue;
                        }
                        let coherent_order = mt == mtc && mt - mtc + 1 == il;
                        if mt != mti && !coherent_order {
                            continue;
                        }
                    }
                    let values: Vec<f64> = (2..=ng2).map(|k| rec.value(il, jz, k)).collect();
                    sig.add_scatter_record(jg, ig2lo, ipingp, iptotl, il, &values);
                }
            }
        }
        // Label 800: normalise chi.
        if il == 1 && kpos != 0 && cnorm != 0.0 {
            for k in 1..=ng {
                let v = sig.get(kpos, k) / cnorm;
                sig.set(kpos, k, v);
            }
        }
        neutron.push(sig);
    }

    // Photon passes (`:329`, `:522-556`).
    let mut photon = Vec::new();
    let mut ipmax = 0;
    for ip in 1..=deck.nptabl {
        let mut sig = PhotonTable {
            ng,
            ngp,
            sig: vec![0.0; (ngp * ng) as usize],
        };
        for sec in &secs {
            if sec.mf != 16 && sec.mf != 17 {
                continue;
            }
            if ip > sec.nl {
                continue;
            }
            let jz = if jz > sec.nz { 1 } else { jz };
            let nl = sec.nl;
            let nz = sec.nz;
            for rec in &sec.records {
                let (ig, ng2, ig2lo) = (rec.ig, rec.ng2, rec.ig2lo);
                let jg = dtf_group(ig, ng);
                let ff = match sec.mt {
                    102 if jg >= 1 && jg <= ng => fcap[jg as usize],
                    18 if jg >= 1 && jg <= ng => ffis[jg as usize],
                    _ => 1.0,
                };
                if ig == 0 {
                    for k in 1..=ngp {
                        let jgp = ngp - k + 1;
                        spect[jgp as usize] = 0.0;
                        if k >= ig2lo && k <= ig2lo + ng2 - 1 {
                            spect[jgp as usize] = raw(rec, k - ig2lo + 1);
                        }
                    }
                } else if jg < 1 || jg > ng {
                    continue;
                } else if ig2lo == 0 {
                    let v = raw(rec, ip + nl * ((jz - 1) + nz));
                    for k in 1..=ngp {
                        sig.add(k, jg, v * ff * spect[k as usize]);
                    }
                } else {
                    ipmax = ip;
                    for k in 2..=ng2 {
                        let jpos = ngp - ig2lo - k + 3;
                        if jpos >= 1 && jpos <= ngp {
                            sig.add(jpos, jg, rec.value(ip, jz, k) * ff);
                        }
                    }
                }
            }
        }
        photon.push(sig);
    }

    Ok(DtfrTables {
        neutron,
        ilmax,
        photon,
        ipmax,
        has_fission,
    })
}
