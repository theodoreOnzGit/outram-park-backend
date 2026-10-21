// SPDX-License-Identifier: GPL-3.0

//! **Reading a thermal S(α,β) ACE table back into physics** — the mirror of
//! [`super::thermal`]'s writer (ACE gap 2).
//!
//! This crate wrote thermal ACE tables byte-for-byte against NJOY2016 (see
//! `verification_and_validation/acer_thermal_vs_njoy2016.md` and the IFENG=1/2
//! record beside it) but could not **read** one. So
//! `outram_mc_libs::ThermalScattering` had exactly two ways in — a published
//! `tsl-*.endf` tape, or regenerating the law with LEAPR — and a thermal
//! lattice built from an ACE library had no bound-atom scattering at all.
//!
//! # Layout, from the writer beside it
//!
//! ```text
//! ITIE : NEI, E_inc(1..NEI)                 [MeV]
//! ITIX : sigma_inel(1..NEI)                 [barn]
//! ITXE : per incident energy, NIEB bins of  [E'(MeV), mu(1..nang)]
//! ITCE : NEE, E_elastic(1..NEE)             [MeV]
//! ITCX : coherent: cumulative S*E [MeV.b]; incoherent: sigma_el [barn]
//! ITCA : incoherent only, equally-probable cosines
//! ```
//!
//! `NXS(3) = NIL` is `nang - 1`, `NXS(4) = NIEB`, `NXS(5) = IDPNC` selects the
//! elastic mode (0 none, 3 incoherent, 4 coherent, 5 mixed; 5 was silently
//! decoded as 3 until 2026-09-29, then refused, and is now read as OpenMC reads
//! it — see the IDPNC match below), and
//! `NXS(7) = IFENG` the inelastic form.
//!
//! # ~~IFENG=2 is REFUSED, not approximated~~ **CORRECTED 2026-09-29 — read (GitHub #365 audit)**
//!
//! ~~`IFENG = 2` is **continuous** ... refused with a message saying which form
//! the file carries.~~ `IFENG = 2` (continuous: per outgoing energy a pdf, cdf
//! and cosines, with the point count varying by incident energy) is now decoded
//! into [`AceThermalContinuous`], and
//! `outram_mc_libs::material::thermal::ThermalScattering` samples it with a port
//! of OpenMC's `IncoherentInelasticAE::sample` rather than squeezing it into the
//! binned form. This is the form OpenMC's own libraries use.
//!
//! `IFENG = 0` (equiprobable) and `1` (skewed) both store, per incident energy,
//! a fixed `NIEB` outgoing energies each with `nang` cosines. ~~— which is
//! exactly the discrete form the transport side holds.~~ **CORRECTED
//! 2026-09-29**: the layout is the same, but skewed bins are not equiprobable
//! and the transport side's form has no bin weights, so
//! `outram_mc_libs::material::thermal::ThermalScattering::from_ace` refuses
//! IFENG = 1. This decoder still reads it faithfully (`ifeng` is carried).

use crate::acer::read::RawAceTable;
use crate::acer::thermal::{jxs, nxs};
use crate::error::NjoyError;

/// eV per MeV.
const EV_PER_MEV: f64 = 1.0e6;

/// One incident energy's discrete emission table.
#[derive(Debug, Clone, PartialEq)]
pub struct AceThermalEmission {
    /// Representative outgoing energies \[eV\], one per equiprobable bin.
    pub e_out: Vec<f64>,
    /// Cosines per outgoing-energy bin, row-major (`bin * n_mu + j`).
    pub cosines: Vec<f64>,
    /// Cosines per bin.
    pub n_mu: usize,
}

/// One incident energy's **continuous** (IFENG = 2) emission law: per outgoing
/// point `E'` \[eV\], its pdf \[eV⁻¹\] and cdf, and `n_mu` equiprobable cosines
/// (sorted), row-major (`point * n_mu + k`). See `decode_continuous_emission`.
#[derive(Debug, Clone, PartialEq)]
pub struct AceThermalContinuous {
    /// Outgoing energies \[eV\], ascending, starting at 0.
    pub e_out: Vec<f64>,
    /// pdf \[eV⁻¹\] at each `e_out`.
    pub pdf: Vec<f64>,
    /// cdf at each `e_out`.
    pub cdf: Vec<f64>,
    /// Cosines, row-major.
    pub cosines: Vec<f64>,
    /// Cosines per point.
    pub n_mu: usize,
}

/// The elastic channel an ACE thermal table carries.
#[derive(Debug, Clone, PartialEq)]
pub enum AceThermalElastic {
    /// `IDPNC = 0` — no thermal elastic law (light water).
    None,
    /// `IDPNC = 4` — coherent (Bragg). `cumulative` is the cumulative `S*E`
    /// \[eV·b\], from which σ_el(E) = cumulative(E)/E between edges.
    Coherent {
        /// Bragg edge energies \[eV\], ascending.
        energy: Vec<f64>,
        /// Cumulative `S*E` \[eV·b\] at each edge.
        cumulative: Vec<f64>,
    },
    /// `IDPNC = 5` — **mixed**: coherent (ITCE/ITCX) plus incoherent
    /// (ITCEI/ITCXI/ITCAI, `NXS(8)` = cosines − 1). GitHub #365 audit.
    Mixed {
        /// Coherent Bragg edges \[eV\].
        coh_energy: Vec<f64>,
        /// Coherent cumulative `S*E` \[eV·b\].
        coh_cumulative: Vec<f64>,
        /// Incoherent incident energies \[eV\].
        inc_energy: Vec<f64>,
        /// Incoherent σ_el \[barn\].
        inc_xs: Vec<f64>,
        /// Incoherent cosines, row-major.
        inc_cosines: Vec<f64>,
        /// Incoherent cosines per energy.
        inc_n_mu: usize,
    },
    /// `IDPNC = 3` — incoherent elastic: σ_el(E) plus equally-probable cosines.
    Incoherent {
        /// Incident energies \[eV\], ascending.
        energy: Vec<f64>,
        /// σ_el \[barn\] at each energy.
        xs: Vec<f64>,
        /// Cosines, row-major (`energy_index * n_mu + j`).
        cosines: Vec<f64>,
        /// Cosines per incident energy.
        n_mu: usize,
    },
}

/// A decoded thermal ACE table.
#[derive(Debug, Clone, PartialEq)]
pub struct AceThermal {
    /// `kT` \[eV\] the table represents.
    pub kt_ev: f64,
    /// Incident energies \[eV\] for σ_inel, ascending.
    pub inel_energy: Vec<f64>,
    /// σ_inel \[barn\] per principal atom at each `inel_energy`.
    pub inel_xs: Vec<f64>,
    /// One emission table per `inel_energy` point, for the binned forms
    /// (IFENG = 0, 1). Empty for IFENG = 2.
    pub emission: Vec<AceThermalEmission>,
    /// One continuous emission law per `inel_energy` point, for IFENG = 2
    /// (GitHub #365 audit). Empty for the binned forms.
    pub continuous: Vec<AceThermalContinuous>,
    /// The elastic channel.
    pub elastic: AceThermalElastic,
    /// `IFENG` as stored, for provenance.
    pub ifeng: i32,
}

fn need(t: &RawAceTable, at: usize, n: usize, what: &str) -> Result<(), NjoyError> {
    if at + n > t.xss.len() {
        return Err(NjoyError::EndfParse(format!(
            "thermal ACE: {what} needs words {at}..{} but XSS has {}",
            at + n,
            t.xss.len()
        )));
    }
    Ok(())
}

/// The IFENG = 0/1 (binned) ITXE block; see [`decode_thermal`].
fn decode_discrete_emission(
    t: &RawAceTable,
    n_energy: usize,
) -> Result<Vec<AceThermalEmission>, NjoyError> {
    let n_mu = (t.nxs[nxs::NIL] + 1) as usize;
    let n_eout = t.nxs[nxs::NIEB] as usize;
    if n_mu == 0 || n_eout == 0 {
        return Err(NjoyError::EndfParse(format!(
            "thermal ACE: NIL+1 = {n_mu} cosines and NIEB = {n_eout} outgoing \
             energies; neither can be zero for a discrete emission table"
        )));
    }
    let itxe = t.jxs[jxs::ITXE];
    if itxe <= 0 {
        return Err(NjoyError::EndfParse(
            "thermal ACE: JXS(3) (ITXE) is zero, so there are cross sections but no \
             emission distributions -- a scatterer that removes neutrons and emits \
             nothing"
                .into(),
        ));
    }
    let stride = 1 + n_mu;
    let ebase = (itxe - 1) as usize;
    need(t, ebase, n_energy * n_eout * stride, "ITXE tables")?;
    let mut emission = Vec::with_capacity(n_energy);
    for i in 0..n_energy {
        let mut e_out = Vec::with_capacity(n_eout);
        let mut cosines = Vec::with_capacity(n_eout * n_mu);
        for b in 0..n_eout {
            let o = ebase + (i * n_eout + b) * stride;
            e_out.push(t.xss[o] * EV_PER_MEV);
            cosines.extend_from_slice(&t.xss[o + 1..o + 1 + n_mu]);
        }
        emission.push(AceThermalEmission {
            e_out,
            cosines,
            n_mu,
        });
    }
    Ok(emission)
}

/// Decode the **IFENG = 2 continuous** incoherent-inelastic emission block —
/// GitHub #365 audit. A port of OpenMC's reader
/// (`openmc/data/thermal.py:809-887`):
///
/// - `NIL = NXS(3) = nang + 1`, so each point carries `nang = NIL - 1` cosines;
/// - at `JXS(3)`: `NEI` locators, then `NEI` point counts. A locator is the
///   **absolute** position such that the first `E'` is `XSS(loc + 1)`, i.e.
///   0-based `xss[loc]` (measured on an NJOY2016 `iwt = 2` H-in-H2O table:
///   `JXS(3) = 214`, first locator `425 = 214 + 2·106 - 1`);
/// - per point: `E'` \[MeV\], pdf \[MeV⁻¹\], cdf, then `nang` cosines;
/// - the cosines of each point are **sorted** (they are equiprobable, and
///   NJOY's are not always in order; OpenMC sorts them because the smearing in
///   its sampler assumes neighbours are neighbours);
/// - when a table's cdf does not start at 0 (NJOY's never does), a point at
///   `E' = 0` with pdf = cdf = 0 and isotropic midpoint cosines is prepended,
///   exactly as OpenMC does, so no draw can extrapolate to a negative energy.
fn decode_continuous_emission(
    t: &RawAceTable,
    n_energy: usize,
) -> Result<Vec<AceThermalContinuous>, NjoyError> {
    let nil = t.nxs[nxs::NIL];
    if nil < 2 {
        return Err(NjoyError::EndfParse(format!(
            "thermal ACE IFENG = 2: NIL = {nil}, but the continuous form needs NIL = nang + 1 >= 2"
        )));
    }
    let n_mu = (nil - 1) as usize;
    let itxe = t.jxs[jxs::ITXE];
    if itxe <= 0 {
        return Err(NjoyError::EndfParse(
            "thermal ACE IFENG = 2: JXS(3) (ITXE) is zero".into(),
        ));
    }
    let base = (itxe - 1) as usize;
    need(t, base, 2 * n_energy, "IFENG=2 locators and counts")?;
    let stride = n_mu + 3;
    let mut out = Vec::with_capacity(n_energy);
    for i in 0..n_energy {
        let loc = t.xss[base + i] as usize;
        let n = t.xss[base + n_energy + i] as usize;
        if n < 2 {
            return Err(NjoyError::EndfParse(format!(
                "thermal ACE IFENG = 2: incident energy {i} has {n} outgoing points"
            )));
        }
        need(t, loc, n * stride, "IFENG=2 points")?;
        let mut e_out = Vec::with_capacity(n + 1);
        let mut pdf = Vec::with_capacity(n + 1);
        let mut cdf = Vec::with_capacity(n + 1);
        let mut cosines = Vec::with_capacity((n + 1) * n_mu);
        for j in 0..n {
            let o = loc + j * stride;
            e_out.push(t.xss[o] * EV_PER_MEV);
            pdf.push(t.xss[o + 1] / EV_PER_MEV);
            cdf.push(t.xss[o + 2]);
            let mut mu = t.xss[o + 3..o + 3 + n_mu].to_vec();
            mu.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            cosines.extend_from_slice(&mu);
        }
        if cdf[0] > 0.0 {
            e_out.insert(0, 0.0);
            pdf.insert(0, 0.0);
            cdf.insert(0, 0.0);
            let dmu = 2.0 / n_mu as f64;
            let iso: Vec<f64> = (0..n_mu).map(|k| -1.0 + (k as f64 + 0.5) * dmu).collect();
            cosines.splice(0..0, iso);
        }
        out.push(AceThermalContinuous {
            e_out,
            pdf,
            cdf,
            cosines,
            n_mu,
        });
    }
    Ok(out)
}

/// Decode a thermal S(α,β) ACE table.
///
/// # Errors
///
/// `IFENG = 2` (continuous), which the discrete transport representation cannot
/// hold — see the module docs; a block whose extent runs past `XSS`; a missing
/// `ITIE`; or an elastic mode this does not recognise. Every one is refused
/// rather than partially decoded, because a thermal law that is quietly half
/// right shifts `k` on exactly the lattices it is there to get right.
pub fn decode_thermal(t: &RawAceTable) -> Result<AceThermal, NjoyError> {
    let ifeng = t.nxs[nxs::IFENG];
    if !(0..=2).contains(&ifeng) {
        return Err(NjoyError::EndfParse(format!(
            "thermal ACE: IFENG = {ifeng} is not a form this reads (0 equiprobable, \
             1 skewed, 2 continuous). Refusing rather than guessing at the block layout."
        )));
    }

    // ── ITIE / ITIX: the inelastic cross section ───────────────────────────
    let itie = t.jxs[jxs::ITIE];
    if itie <= 0 {
        return Err(NjoyError::EndfParse(
            "thermal ACE: JXS(1) (ITIE) is zero, so the table carries no \
             incoherent-inelastic data. That is not a thermal scattering table \
             this can use."
                .into(),
        ));
    }
    let at = (itie - 1) as usize;
    need(t, at, 1, "ITIE count")?;
    let n_energy = t.xss[at] as usize;
    if n_energy == 0 {
        return Err(NjoyError::EndfParse(
            "thermal ACE: ITIE declares zero incident energies".into(),
        ));
    }
    need(t, at + 1, n_energy, "ITIE grid")?;
    let inel_energy: Vec<f64> = t.xss[at + 1..at + 1 + n_energy]
        .iter()
        .map(|e| e * EV_PER_MEV)
        .collect();
    if !inel_energy.windows(2).all(|w| w[1] > w[0]) {
        return Err(NjoyError::EndfParse(
            "thermal ACE: the ITIE incident-energy grid is not strictly ascending, \
             so a lookup over it would return an arbitrary table"
                .into(),
        ));
    }

    let itix = t.jxs[jxs::ITIX];
    let xat = if itix > 0 {
        (itix - 1) as usize
    } else {
        // NJOY writes ITIX immediately after ITIE's grid; a zero locator means
        // "contiguous", which is how the writer beside this lays it out.
        at + 1 + n_energy
    };
    need(t, xat, n_energy, "ITIX cross section")?;
    let inel_xs = t.xss[xat..xat + n_energy].to_vec();

    // ── ITXE: the discrete emission tables ─────────────────────────────────
    //
    // Per incident energy, NIEB bins of [E', mu(1..nang)]. NIL is nang - 1, so
    // the stride is 1 + nang = NIL + 2 -- the same `n_mu + 2` upstream uses
    // (`openmc/data/thermal.py::from_ace`).
    let (emission, continuous) = if ifeng == 2 {
        (Vec::new(), decode_continuous_emission(t, n_energy)?)
    } else {
        (decode_discrete_emission(t, n_energy)?, Vec::new())
    };

    // ── ITCE / ITCX / ITCA: the elastic channel ────────────────────────────
    let idpnc = t.nxs[nxs::IDPNC];
    let elastic = match idpnc {
        0 => AceThermalElastic::None,
        // **IDPNC = 5 (mixed coherent + incoherent)** -- GitHub #365 audit.
        // ~~refused~~ (and before that silently decoded as incoherent). Read
        // as OpenMC reads it (`openmc/data/thermal.py:897-942`): the coherent
        // part from ITCE/ITCX exactly as for IDPNC = 4, the incoherent part
        // from JXS(7) (count, energies, then sigma contiguous, as upstream reads
        // it from `jxs[7]` alone) with NXS(8) + 1 cosines per energy at JXS(9).
        5 => {
            let itce = t.jxs[jxs::ITCE];
            let (ce, cc) = {
                let eat = (itce - 1) as usize;
                need(t, eat, 1, "ITCE count")?;
                let n = t.xss[eat] as usize;
                need(t, eat + 1, 2 * n, "ITCE/ITCX coherent")?;
                let itcx = t.jxs[jxs::ITCX];
                let cat = if itcx > 0 {
                    (itcx - 1) as usize
                } else {
                    eat + 1 + n
                };
                need(t, cat, n, "ITCX values")?;
                (
                    t.xss[eat + 1..eat + 1 + n]
                        .iter()
                        .map(|e| e * EV_PER_MEV)
                        .collect(),
                    t.xss[cat..cat + n].iter().map(|v| v * EV_PER_MEV).collect(),
                )
            };
            let iat = t.jxs[jxs::ITCEI];
            if iat <= 0 {
                return Err(NjoyError::EndfParse(
                    "thermal ACE IDPNC = 5 with no incoherent block (JXS(7) = 0)".into(),
                ));
            }
            let iat = (iat - 1) as usize;
            need(t, iat, 1, "ITCEI count")?;
            let n = t.xss[iat] as usize;
            need(t, iat + 1, 2 * n, "ITCEI/ITCXI incoherent")?;
            let n_mu = (t.nxs[nxs::NCLI] + 1).max(0) as usize;
            let itcai = t.jxs[jxs::ITCAI];
            if n_mu == 0 || itcai <= 0 {
                return Err(NjoyError::EndfParse(
                    "thermal ACE IDPNC = 5: the incoherent part has no cosines".into(),
                ));
            }
            let aat = (itcai - 1) as usize;
            need(t, aat, n * n_mu, "ITCAI cosines")?;
            AceThermalElastic::Mixed {
                coh_energy: ce,
                coh_cumulative: cc,
                inc_energy: t.xss[iat + 1..iat + 1 + n]
                    .iter()
                    .map(|e| e * EV_PER_MEV)
                    .collect(),
                inc_xs: t.xss[iat + 1 + n..iat + 1 + 2 * n].to_vec(),
                inc_cosines: t.xss[aat..aat + n * n_mu].to_vec(),
                inc_n_mu: n_mu,
            }
        }
        3 | 4 => {
            let itce = t.jxs[jxs::ITCE];
            if itce <= 0 {
                AceThermalElastic::None
            } else {
                let eat = (itce - 1) as usize;
                need(t, eat, 1, "ITCE count")?;
                let n_el = t.xss[eat] as usize;
                need(t, eat + 1, n_el, "ITCE grid")?;
                let energy: Vec<f64> = t.xss[eat + 1..eat + 1 + n_el]
                    .iter()
                    .map(|e| e * EV_PER_MEV)
                    .collect();
                let itcx = t.jxs[jxs::ITCX];
                let cat = if itcx > 0 {
                    (itcx - 1) as usize
                } else {
                    eat + 1 + n_el
                };
                need(t, cat, n_el, "ITCX values")?;
                if idpnc == 4 {
                    // Coherent: ITCX is the cumulative `S*E` in MeV.b.
                    AceThermalElastic::Coherent {
                        energy,
                        cumulative: t.xss[cat..cat + n_el]
                            .iter()
                            .map(|s| s * EV_PER_MEV)
                            .collect(),
                    }
                } else {
                    // Incoherent: ITCX is sigma_el in barns, ITCA the cosines.
                    // NCL is nbin - 1, so nbin = NCL + 1; a negative NCL means
                    // no tabulated cosines (isotropic).
                    let ncl = t.nxs[nxs::NCL];
                    let n_mu_el = if ncl >= 0 { (ncl + 1) as usize } else { 0 };
                    let itca = t.jxs[jxs::ITCA];
                    let cosines = if n_mu_el > 0 && itca > 0 {
                        let aat = (itca - 1) as usize;
                        need(t, aat, n_el * n_mu_el, "ITCA cosines")?;
                        t.xss[aat..aat + n_el * n_mu_el].to_vec()
                    } else {
                        Vec::new()
                    };
                    AceThermalElastic::Incoherent {
                        energy,
                        xs: t.xss[cat..cat + n_el].to_vec(),
                        cosines,
                        n_mu: n_mu_el,
                    }
                }
            }
        }
        other => {
            return Err(NjoyError::EndfParse(format!(
                "thermal ACE: IDPNC = {other} is not an elastic mode this reads \
                 (0 none, 3 incoherent, 4 coherent, 5 mixed). Refusing rather than \
                 dropping the elastic channel silently, which would remove real \
                 scattering from a lattice."
            )));
        }
    };

    Ok(AceThermal {
        kt_ev: t.header.kt_mev * EV_PER_MEV,
        inel_energy,
        inel_xs,
        emission,
        continuous,
        elastic,
        ifeng,
    })
}
