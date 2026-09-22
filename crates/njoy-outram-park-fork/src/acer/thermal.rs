//! Thermal scattering **S(α,β)** ACE table writer (`…t` tables).
//!
//! Assembles the thermal ACE table a Monte-Carlo code uses for bound-atom
//! thermal scattering — graphite, H/D in water, ZrH, Al, … — where the free-gas
//! model no longer applies and the scattering law S(α,β) governs the kinematics.
//! Ports the load path of `aceth.f90` (`thrlod`) for the `IFENG=0` (equiprobable)
//! inelastic form plus coherent-elastic (Bragg) data.
//!
//! The physics comes from [`crate::thermr`]: [`IncoherentInelastic`] provides
//! the inelastic cross section and equiprobable emission bins, and
//! [`CoherentElastic`] provides the Bragg `S(E)`.
//!
//! ## Thermal NXS / JXS (distinct from the continuous-energy table)
//!
//! - **NXS**: `IDPNI` (inelastic mode = 3), `NIL` (= nang−1 discrete cosines),
//!   `NIEB` (outgoing energies per incident energy), `IDPNC` (elastic mode: 4
//!   coherent, 0 none), `NCL` (−1 for coherent), `IFENG` (= 0, equiprobable).
//! - **JXS**: `ITIE`/`ITIX` (inelastic incident-energy grid + cross section),
//!   `ITXE` (energy-angle distributions), `ITCE`/`ITCX` (coherent-elastic Bragg
//!   energies + cumulative cross section).
//!
//! ## Block layout (`thrlod`, IFENG=0)
//!
//! ```text
//! ITIE : NEI, E_inc(1..NEI)                 [MeV]
//! ITIX : σ_inel(1..NEI)                      [barn]
//! ITXE : per incident energy, NIEB bins of  [E'(MeV), μ(1..nang)]
//! ITCE : NEE, E_bragg(1..NEE)               [MeV]   (coherent elastic)
//! ITCX : cumulative S(1..NEE)               [MeV·b]
//! ```
//!
//! Incoherent-elastic is handled: ITCE/ITCX/ITCA when it is the only elastic
//! mode (`IDPNC=3`), or ITCEI/ITCXI/ITCAI alongside coherent (`IDPNC=5`).
//! Not yet handled: the skewed/continuous `IFENG=1/2` inelastic forms, and
//! multi-atom mixing (`nmix`, taken as 1).
//!
//! ## Prerequisite provenance
//!
//! - **THERMR** (`thermr.f90`) — the physics ported in [`crate::thermr`].
//! - **LEAPR** (`leapr.f90`) — *generates* MF=7 when an evaluation lacks it;
//!   optional, since the ENDF/B thermal sublibrary ships MF=7.

use crate::acer::acesix::{
    acesix_equiprobable, acesix_tabulated, normalized_rows, AcesixPoint, BinWeights,
};
use crate::thermr::calcem::iform0::compute_iform0;
use crate::thermr::calcem::types::{Iform0Table, IncidentEnergyRecord};
use crate::thermr::inelastic::OutgoingBin;
use crate::thermr::mf7::Mf7;
use crate::NjoyError;

use super::AceTable;

/// eV → MeV.
const EMEV: f64 = 1.0e6;

/// Which inelastic secondary-energy form the table stores — `NXS(7)`, and the
/// `iwt` that selects it on ACER card 9 (`aceth.f90:674-676`).
///
/// This is **not** three flavours of the same block. The first two share a
/// layout and differ only in how the fixed `NIEB` bins are weighted; the third
/// changes the layout, the meaning of `NIL`, and the length of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InelasticForm {
    /// `iwt = 1`, `IFENG = 0` — `NIEB` bins of equal probability. `NIL` is
    /// `nang - 1`. What production `tsl` libraries have historically shipped.
    #[default]
    Equiprobable,
    /// `iwt = 0`, `IFENG = 1` — the same `NIEB` bins under NJOY's default
    /// `1 4 10 … 10 4 1` weighting, so the outlying bins carry a tenth of the
    /// probability of an interior one. **Same layout and same `NIL`**; only
    /// the bin edges move, and `NXS(7)` tells a sampler not to draw uniformly.
    Skewed,
    /// `iwt = 2`, `IFENG = 2` — the continuous form. The fixed bin count is
    /// abandoned: each incident energy stores its own number of
    /// `(E', pdf, cdf, mu_1..mu_nang)` points, `NIL` becomes `nang + 1`, and a
    /// `2 * NEI` table of (offset, count) pairs is written ahead of `ITXE`
    /// (`aceth.f90:815-851`).
    Continuous,
}

impl InelasticForm {
    /// `NXS(7)`.
    pub fn ifeng(self) -> i32 {
        match self {
            InelasticForm::Equiprobable => 0,
            InelasticForm::Skewed => 1,
            InelasticForm::Continuous => 2,
        }
    }
    /// `NIL` (`aceth.f90:816-820`): `nang - 1` for the binned forms and
    /// `nang + 1` for the continuous one, which carries a density and a
    /// cumulative beside each cosine set.
    pub fn nil(self, nang: usize) -> i32 {
        match self {
            InelasticForm::Continuous => (nang + 1) as i32,
            _ => (nang - 1) as i32,
        }
    }
}

/// Options for the thermal inelastic table dimensions.
#[derive(Debug, Clone, Copy)]
pub struct ThermalAceOptions {
    /// Number of equally-probable outgoing energies per incident energy (`NIEB`).
    pub n_outgoing: usize,
    /// Number of equally-probable scattering cosines per outgoing energy (`nang`;
    /// `NIL = nang − 1`). Also sizes the incoherent-elastic angular bins (`NEA`,
    /// `NCL = nang − 1`) when the evaluation has incoherent-elastic data.
    pub n_cosines: usize,
    /// Number of principal scattering atoms in the material (`B(6)` records it;
    /// `1` for a monatomic scatterer). Used for `σ_b` and the elastic mixing.
    pub natom: f64,
    /// Upper limit of the thermal treatment \[eV\] — THERMR card-4 `emax`,
    /// which bounds the `calcem` incident-energy grid the bins are built from.
    pub emax_ev: f64,
    /// Which inelastic secondary-energy form to write (`NXS(7)`).
    pub form: InelasticForm,
}

/// THERMR card-4 `tol`, 0.05 in the standard thermal decks.
const CALCEM_TOL: f64 = 0.05;

/// The tabulated `calcem` record covering `ev` — the ACE block stores one
/// emission entry per tabulated incident energy, so the grid is used directly.
fn nearest_calcem_record(table: &Iform0Table, ev: f64) -> Option<&IncidentEnergyRecord> {
    let recs = &table.records;
    if recs.is_empty() {
        return None;
    }
    let i = match recs
        .binary_search_by(|r| r.e_in_ev.partial_cmp(&ev).expect("finite grid energies"))
    {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) if i >= recs.len() => recs.len() - 1,
        Err(i) => {
            if (ev - recs[i - 1].e_in_ev).abs() <= (recs[i].e_in_ev - ev).abs() {
                i - 1
            } else {
                i
            }
        }
    };
    Some(&recs[i])
}

impl Default for ThermalAceOptions {
    fn default() -> Self {
        // NJOY-typical dimensions: 16 outgoing energies, 8 equiprobable cosines.
        ThermalAceOptions {
            n_outgoing: 16,
            n_cosines: 8,
            emax_ev: 4.0,
            natom: 1.0,
            form: InelasticForm::Equiprobable,
        }
    }
}

/// Named thermal **NXS** indices (0-based).
pub mod nxs {
    /// NXS(1): length of the XSS block.
    pub const LEN_XSS: usize = 0;
    /// NXS(2): IDPNI — inelastic scattering mode (`3` = S(α,β) distributions).
    pub const IDPNI: usize = 1;
    /// NXS(3): NIL — discrete cosines minus one (`nang − 1`).
    pub const NIL: usize = 2;
    /// NXS(4): NIEB — outgoing energies per incident energy.
    pub const NIEB: usize = 3;
    /// NXS(5): IDPNC — elastic mode (`4` = coherent, `0` = none).
    pub const IDPNC: usize = 4;
    /// NXS(6): NCL — elastic angular dimensioning for the primary elastic block
    /// (`−1` for coherent, `nbin − 1` for incoherent-elastic).
    pub const NCL: usize = 5;
    /// NXS(7): IFENG — inelastic energy-distribution form (`0` = equiprobable).
    pub const IFENG: usize = 6;
    /// NXS(8): NCLI — angular dimensioning for the *secondary* (incoherent)
    /// elastic block in the mixed coherent+incoherent case (`nbin − 1`).
    pub const NCLI: usize = 7;
}

/// Named thermal **JXS** indices (0-based).
pub mod jxs {
    /// JXS(1): ITIE — inelastic incident-energy grid.
    pub const ITIE: usize = 0;
    /// JXS(2): ITIX — inelastic cross section.
    pub const ITIX: usize = 1;
    /// JXS(3): ITXE — inelastic energy-angle distributions.
    pub const ITXE: usize = 2;
    /// JXS(4): ITCE — primary elastic energies (coherent Bragg, or incoherent).
    pub const ITCE: usize = 3;
    /// JXS(5): ITCX — primary elastic cross section (coherent cumulative `S·E`,
    /// or incoherent σ).
    pub const ITCX: usize = 4;
    /// JXS(6): ITCA — primary elastic equally-probable cosines (incoherent only;
    /// `0` for coherent, whose angles are the discrete Bragg cosines).
    pub const ITCA: usize = 5;
    /// JXS(7): ITCEI — secondary (incoherent) elastic energies, mixed case.
    pub const ITCEI: usize = 6;
    /// JXS(8): ITCXI — secondary (incoherent) elastic cross section, mixed case.
    pub const ITCXI: usize = 7;
    /// JXS(9): ITCAI — secondary (incoherent) elastic cosines, mixed case.
    pub const ITCAI: usize = 8;
}

impl AceTable {
    /// Assemble a thermal **S(α,β)** ACE table from a parsed MF=7 evaluation at
    /// temperature `temp_k` \[K\].
    ///
    /// `name` is the thermal ZAID stem (e.g. `"al27"`), `suffix` the temperature
    /// tag in hundredths (`0` → `.00t`). `energy_grid` is the inelastic
    /// incident-energy grid \[eV\] (ascending). `opts` sets the table dimensions.
    ///
    /// Writes the inelastic ITIE/ITIX/ITXE blocks and, when the evaluation has
    /// coherent-elastic data, the ITCE/ITCX Bragg blocks.
    ///
    /// # Errors
    /// [`NjoyError::NotPorted`] if the evaluation has no incoherent-inelastic
    /// (MT=4) data (the required inelastic table);
    /// [`NjoyError::TemperatureOutOfRange`] if the evaluation has
    /// coherent-elastic data but `temp_k` is outside its tabulated temperature
    /// range (beyond the NJOY `T/1000 + 5` K tolerance).
    ///
    /// # Known gaps (not silently missing — tracked, not yet ported)
    /// - **All three `IFENG` forms are written** — equiprobable (0), skewed
    ///   (1) and continuous (2) — selected by
    ///   [`ThermalAceOptions::form`]. ~~IFENG is always 0.~~ **CORRECTED
    ///   2026-09-22.**
    /// - **`opts.natom` is a single scalar** — multi-scatterer mixing (`nmix` >
    ///   1 in `aceth.f90`, e.g. a material with two distinct bound-atom
    ///   populations contributing to the same thermal table) is not supported;
    ///   every evaluation this writer handles is treated as `nmix = 1`.
    pub fn thermal_from_mf7(
        mf7: &Mf7,
        temp_k: f64,
        name: &str,
        suffix: u32,
        energy_grid: &[f64],
        opts: ThermalAceOptions,
    ) -> Result<Self, NjoyError> {
        let ii = mf7
            .incoherent_inelastic
            .as_ref()
            .ok_or(NjoyError::NotPorted(
                "thermal ACE without incoherent-inelastic (MT=4) data",
            ))?;

        let nei = energy_grid.len();
        let nieb = opts.n_outgoing;
        let nang = opts.n_cosines;
        // TODO(nmix>1): `natom` is a single scalar — no support for mixing
        // multiple distinct bound-atom populations into one thermal table
        // (`nmix` in `aceth.f90`). Every material here is treated as nmix=1.
        let natom = opts.natom;

        let xs: Vec<f64> = energy_grid
            .iter()
            .map(|&e| ii.cross_section(e, temp_k, natom))
            .collect();
        // The emission law comes from `calcem` through the ported `acesix`,
        // which is what NJOY does: THERMR writes the MF=6 emission matrix and
        // ACER turns it into bins or into a tabulated density.
        //
        // The weighting is **fixed by the form, not chosen**: `IFENG = 0`'s
        // bins are sampled uniformly so they must be equal-area
        // (`BinWeights::Constant`, `iwt = 1`), while `IFENG = 1` exists
        // precisely to make them unequal ("outlying bins with smaller
        // probabilities", `acer.f90:131`) and so takes the variable
        // `1 4 10 ... 10 4 1` pattern. Pairing either with the other flag
        // would write a table whose NXS(7) lies about its own contents.
        let calcem = compute_iform0(ii, natom, nang, opts.emax_ev, CALCEM_TOL)?;
        let mut emission: Vec<Vec<OutgoingBin>> = Vec::new();
        let mut tabulated: Vec<Vec<AcesixPoint>> = Vec::new();
        if opts.form == InelasticForm::Continuous {
            tabulated = energy_grid
                .iter()
                .map(|&e| match nearest_calcem_record(&calcem, e) {
                    Some(rec) => acesix_tabulated(&normalized_rows(&rec.rows), nang),
                    None => Vec::new(),
                })
                .collect();
        } else {
            let weights = match opts.form {
                InelasticForm::Skewed => BinWeights::Variable,
                _ => BinWeights::Constant,
            };
            emission = energy_grid
                .iter()
                .map(|&e| {
                    let Some(rec) = nearest_calcem_record(&calcem, e) else {
                        return Vec::new();
                    };
                    acesix_equiprobable(&normalized_rows(&rec.rows), nieb, weights)
                        .into_iter()
                        .map(|b| OutgoingBin {
                            e_out_ev: b.e_out_ev,
                            cosines: b.cosines,
                        })
                        .collect()
                })
                .collect();
        }

        let mut xss: Vec<f64> = Vec::new();
        let mut is_int: Vec<bool> = Vec::new();
        let real = |v: f64, xss: &mut Vec<f64>, m: &mut Vec<bool>| {
            xss.push(v);
            m.push(false);
        };

        // ── ITIE: NEI, E_inc(MeV) ──────────────────────────────────────────
        let itie = 1;
        xss.push(nei as f64);
        is_int.push(true);
        for &e in energy_grid {
            real(e / EMEV, &mut xss, &mut is_int);
        }
        // ── ITIX: σ_inel ───────────────────────────────────────────────────
        let itix = xss.len() as i32 + 1;
        for &s in &xs {
            real(s, &mut xss, &mut is_int);
        }
        // ── ITXE ────────────────────────────────────────────────────────────
        // IFENG 0/1: NEI x NIEB x [E'(MeV), mu(1..nang)].
        // IFENG 2  : a 2*NEI table of (offset, point count) first, then each
        //            incident energy's own [E'(MeV), pdf(1/MeV), cdf,
        //            mu(1..nang)] points (`aceth.f90:815-851`).
        let itxe = xss.len() as i32 + 1;
        if opts.form == InelasticForm::Continuous {
            // Reserve the locator/count table, then fill it as the data lands.
            let counts: Vec<usize> = tabulated.iter().map(|p| p.len()).collect();
            let table_start = xss.len();
            for _ in 0..2 * nei {
                xss.push(0.0);
                is_int.push(true);
            }
            for (i, points) in tabulated.iter().enumerate() {
                // Upstream stores the index **before** the first word, not the
                // word's own locator (`:825`, `xss(itxe-1+i) = indx` with the
                // data written at `xss(indx+k)` for `k` from 1).
                xss[table_start + i] = xss.len() as f64;
                xss[table_start + nei + i] = counts[i] as f64;
                for p in points {
                    real(p.e_out_ev / EMEV, &mut xss, &mut is_int);
                    // A density per eV becomes a density per MeV (`:838`).
                    real(p.pdf * EMEV, &mut xss, &mut is_int);
                    real(p.cdf, &mut xss, &mut is_int);
                    for j in 0..nang {
                        real(p.cosines.get(j).copied().unwrap_or(0.0), &mut xss, &mut is_int);
                    }
                }
            }
        } else {
            for bins in &emission {
                for k in 0..nieb {
                    // A missing bin (zero cross section) degrades to E'=E, isotropic.
                    let (ep, cos) = match bins.get(k) {
                        Some(b) => (b.e_out_ev, b.cosines.clone()),
                        None => (energy_grid[0], uniform_cosines(nang)),
                    };
                    real(ep / EMEV, &mut xss, &mut is_int);
                    for j in 0..nang {
                        let mu = cos.get(j).copied().unwrap_or(0.0);
                        real(mu, &mut xss, &mut is_int);
                    }
                }
            }
        }

        // ── Elastic blocks (coherent Bragg and/or incoherent) ──────────────
        // Mirrors `aceth.f90::thrlod`: coherent goes to ITCE/ITCX (IDPNC=4);
        // incoherent-only reuses ITCE/ITCX/ITCA (IDPNC=3); when both are present
        // the incoherent set moves to ITCEI/ITCXI/ITCAI (IDPNC=5).
        let (mut itce, mut itcx, mut itca) = (0i32, 0i32, 0i32);
        let (mut itcei, mut itcxi, mut itcai) = (0i32, 0i32, 0i32);
        let (mut idpnc, mut ncl, mut ncli) = (0i32, 0i32, 0i32);

        // Coherent-elastic ITCE/ITCX (Bragg): cumulative S·E, discrete cosines.
        // The S(E) table is resolved at `temp_k` (tabulated / LI-interpolated /
        // refused per the thermr::mf7 temperature policy), so a hot table gets
        // the Debye-Waller-suppressed structure factors, not the base-T ones.
        if let Some(ce) = &mf7.coherent_elastic {
            let s_of_e = ce.s_of_e_at(temp_k)?;
            let nee = s_of_e.len();
            itce = xss.len() as i32 + 1;
            xss.push(nee as f64);
            is_int.push(true);
            for &(e, _) in &s_of_e {
                real(e / EMEV, &mut xss, &mut is_int);
            }
            itcx = xss.len() as i32 + 1;
            for &(_, s) in &s_of_e {
                real(s / EMEV / natom, &mut xss, &mut is_int); // cumulative S·E [MeV·b]
            }
            idpnc = 4; // coherent elastic
            ncl = -1;
        }

        // Incoherent-elastic: NEE energies, σ, and NEA equally-probable cosines.
        if let Some(ie) = &mf7.incoherent_elastic {
            let nea = nang; // reuse the inelastic cosine count for the elastic bins
            let nee = energy_grid.len();
            let e_start = xss.len() as i32 + 1;
            xss.push(nee as f64);
            is_int.push(true);
            for &e in energy_grid {
                real(e / EMEV, &mut xss, &mut is_int);
            }
            let x_start = xss.len() as i32 + 1;
            for &e in energy_grid {
                real(ie.cross_section(e, temp_k, natom), &mut xss, &mut is_int);
            }
            let a_start = xss.len() as i32 + 1;
            for &e in energy_grid {
                for mu in ie.equiprobable_cosines(e, temp_k, nea) {
                    real(mu, &mut xss, &mut is_int);
                }
            }
            if idpnc == 0 {
                // Incoherent only → primary elastic slots.
                itce = e_start;
                itcx = x_start;
                itca = a_start;
                idpnc = 3;
                ncl = (nea - 1) as i32;
            } else {
                // Mixed (coherent already written) → secondary slots.
                itcei = e_start;
                itcxi = x_start;
                itcai = a_start;
                idpnc = 5;
                ncli = (nea - 1) as i32;
            }
        }

        // ── NXS / JXS ───────────────────────────────────────────────────────
        let mut nxs_arr = [0i32; 16];
        nxs_arr[nxs::LEN_XSS] = xss.len() as i32;
        nxs_arr[nxs::IDPNI] = 3;
        nxs_arr[nxs::NIL] = opts.form.nil(nang);
        nxs_arr[nxs::NIEB] = nieb as i32;
        nxs_arr[nxs::IDPNC] = idpnc;
        nxs_arr[nxs::NCL] = ncl;
        nxs_arr[nxs::IFENG] = opts.form.ifeng();
        nxs_arr[nxs::NCLI] = ncli;

        let mut jxs_arr = [0i32; 32];
        jxs_arr[jxs::ITIE] = itie;
        jxs_arr[jxs::ITIX] = itix;
        jxs_arr[jxs::ITXE] = itxe;
        jxs_arr[jxs::ITCE] = itce;
        jxs_arr[jxs::ITCX] = itcx;
        jxs_arr[jxs::ITCA] = itca;
        jxs_arr[jxs::ITCEI] = itcei;
        jxs_arr[jxs::ITCXI] = itcxi;
        jxs_arr[jxs::ITCAI] = itcai;

        let kt_mev = crate::common::phys::BK_EV_PER_K * temp_k / EMEV;
        let zaid = format!("{name}.{suffix:02}t");
        let mat_id = format!("{:>10}", "mat  ");

        Ok(AceTable {
            zaid,
            awr: mf7.awr,
            kt_mev,
            date: "  njoy-rust".to_string(),
            // Same provenance stamp as the continuous-energy path; see
            // `acer::build` for why it lives in the `hk` comment field.
            comment: format!(
                "S(a,b) {name} {temp_k}K njoy-op v{} {} {}",
                env!("CARGO_PKG_VERSION"),
                env!("NJOY_OP_GIT_SHA"),
                env!("NJOY_OP_BUILD_DATE"),
            ),
            mat_id,
            nxs: nxs_arr,
            jxs: jxs_arr,
            xss,
            xss_is_int: is_int,
        })
    }
}

/// Uniform equiprobable cosines at the bin midpoints (isotropic fallback).
fn uniform_cosines(nang: usize) -> Vec<f64> {
    (0..nang)
        .map(|j| -1.0 + 2.0 * (j as f64 + 0.5) / nang as f64)
        .collect()
}
