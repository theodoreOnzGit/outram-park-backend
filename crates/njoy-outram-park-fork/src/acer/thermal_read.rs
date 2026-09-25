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
//! elastic mode (0 none, 3 incoherent, 4 coherent, 5 mixed), and
//! `NXS(7) = IFENG` the inelastic form.
//!
//! # IFENG=2 is REFUSED, not approximated
//!
//! `IFENG = 0` (equiprobable) and `1` (skewed) both store, per incident energy,
//! a fixed `NIEB` outgoing energies each with `nang` cosines — which is exactly
//! the discrete form the transport side holds. `IFENG = 2` is **continuous**:
//! per outgoing energy it stores a pdf and cdf and the bin count varies with
//! incident energy. Squeezing that into the discrete representation would
//! silently resample somebody's carefully tabulated distribution, so it is
//! refused with a message saying which form the file carries.
//!
//! The writer supports IFENG=2 (that was its own task), so this is a limit of
//! the **transport-side representation**, not of the ACE port.

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
    /// One emission table per `inel_energy` point.
    pub emission: Vec<AceThermalEmission>,
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
    if ifeng == 2 {
        return Err(NjoyError::EndfParse(
            "this thermal table is IFENG = 2 (CONTINUOUS inelastic emission): per \
             outgoing energy it carries a pdf and cdf, and the bin count varies \
             with incident energy. The transport side holds the DISCRETE form \
             (a fixed number of equiprobable bins, each with its own cosines), so \
             mapping IFENG=2 onto it would resample the evaluation's own \
             distribution silently. Refused. This is a limit of the transport \
             representation, not of the ACE port -- the writer emits IFENG=2."
                .into(),
        ));
    }
    if !(0..=1).contains(&ifeng) {
        return Err(NjoyError::EndfParse(format!(
            "thermal ACE: IFENG = {ifeng} is not a form this reads (0 equiprobable, \
             1 skewed). Refusing rather than guessing at the block layout."
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

    // ── ITCE / ITCX / ITCA: the elastic channel ────────────────────────────
    let idpnc = t.nxs[nxs::IDPNC];
    let elastic = match idpnc {
        0 => AceThermalElastic::None,
        3 | 4 | 5 => {
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
        elastic,
        ifeng,
    })
}
