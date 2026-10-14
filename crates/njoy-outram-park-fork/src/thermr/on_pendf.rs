// Ported from NJOY2016 `src/thermr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
// the order in which `subroutine thermr` runs `calcem`, `coh`/`iel` and `tpend`
// on one temperature of the incident PENDF. The arithmetic itself lives in
// `calcem`, `mf3_grid`, `mf7` and `incoherent_elastic`; this file only gathers
// what THERMR writes so ACER (`acer::thermal`) can read it.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **What THERMR writes**, as data: the stage between an MF=7 scattering law
//! and the thermal ACE table (GitHub #871).
//!
//! NJOY's thermal deck runs `thermr` and then `acer`, and the two talk through
//! a PENDF tape. [`thermr_tables`] is the first half: `calcem`'s inelastic
//! table, THERMR's MF=3 on the incident nuclide's grid, `rdelas`'s Bragg
//! `S(E)` and the incoherent-elastic law, at one temperature.
//! [`crate::acer::AceTable::thermal_from_thermr`] is the second half. The two
//! together are exactly [`crate::acer::AceTable::thermal_from_mf7_on_pendf`],
//! which is gated word for word against NJOY2016
//! (`tests/acer_thermal_word_parity.rs`, `tests/acer_thermal_temperature_parity.rs`).
//!
//! Splitting them lets a caller keep THERMR's result: the high-level
//! [`crate::process::thermal_scattering`] returns it inside
//! [`crate::process::ThermalScatteringLaw`], and
//! [`crate::process::thermal_ace`] then writes the ACE table from it.
//!
//! **Without the incident PENDF** (`incident = None`) the tables are what
//! [`crate::acer::AceTable::thermal_from_mf7_with_tolerance`] has always
//! used: `calcem` on the MF=7 as parsed (temperature-interpolated when the
//! parser interpolated) and no MF=3 grid. That path is this crate's own and
//! has no NJOY2016 counterpart.

use crate::acer::thermal::IncidentPendf;
use crate::thermr::calcem::iform0::compute_iform0;
use crate::thermr::calcem::types::Iform0Table;
use crate::thermr::mf7::{IncoherentElastic, IncoherentInelastic, Mf7};
use crate::NjoyError;

/// THERMR's output at one temperature: what ACER reads back from its PENDF.
///
/// Every energy is in eV, every cross section in barns per principal atom
/// (`natom` principal atoms per molecule, THERMR card 2).
#[derive(Debug, Clone)]
pub struct ThermrTables {
    /// The requested temperature \[K\] (THERMR card 3).
    pub temperature_k: f64,
    /// The S(α,β) law `calcem` read: at the tabulated temperature it matched
    /// (and with `T_eff` read at the request) on the incident PENDF.
    pub inelastic_law: IncoherentInelastic,
    /// The MF=7 AWR (the principal scatterer's): the table's AWR when there
    /// is no incident PENDF.
    pub mf7_awr: f64,
    /// THERMR card 2 `natom`: principal scattering atoms per molecule.
    pub natom: f64,
    /// THERMR card 2 `nbin`: equally probable cosines per outgoing energy.
    pub n_cosines: usize,
    /// THERMR card 4 `emax` \[eV\]: the top of the thermal treatment.
    pub emax_ev: f64,
    /// THERMR card 4 `tol`: the tolerance `calcem` and `coh` linearise to.
    pub tol: f64,
    /// `calcem`'s `iform = 0` table: per incident energy, `σ_inel(E)` and its
    /// outgoing-energy rows with equally probable cosines (THERMR's MF=6).
    pub inelastic: Iform0Table,
    /// The incident nuclide's PENDF THERMR ran on; `None` without one.
    pub incident: Option<IncidentPendf>,
    /// THERMR's inelastic MF=3 (`mtref`) on the incident PENDF's grid,
    /// rounded as `tpend` writes it. `None` without an incident PENDF.
    pub inelastic_mf3: Option<Vec<(f64, f64)>>,
    /// The cumulative Bragg `S(E)` table, `(E [eV], S [eV·b])`: `rdelas`'s
    /// on the incident PENDF, temperature-resolved without it. `None` without
    /// coherent elastic.
    pub coherent_elastic_s: Option<Vec<(f64, f64)>>,
    /// The incoherent-elastic law (`LTHR = 2` or `3`), whose `iel` records
    /// ACER reads; `None` without incoherent elastic.
    pub incoherent_elastic: Option<IncoherentElastic>,
    /// THERMR's incoherent-elastic MF=3 on the incident PENDF's grid (`iel`,
    /// rounded as `tpend` writes it). `None` without incoherent elastic or
    /// without an incident PENDF.
    pub incoherent_elastic_mf3: Option<Vec<(f64, f64)>>,
}

/// Run THERMR's stages on `mf7` at `temp_k` \[K\]: `calcem` (`iform = 0`,
/// `n_cosines` cosines, up to `emax_ev`, tolerance `tol`), and, on an
/// incident PENDF, THERMR's MF=3 for the inelastic and incoherent-elastic
/// reactions on that PENDF's grid.
///
/// **Temperature, as THERMR resolves it** (GitHub #866). With `incident`,
/// `mf7` must be parsed at `temp_k`
/// ([`crate::thermr::mf7::parse_mf7_at_temperature`]) and `temp_k` must be
/// within `T/500` of a tabulated MF=7/MT=4 temperature
/// ([`crate::thermr::mf7::calcem_temperature_k`]); `S(α,β)` and `kT` are then
/// the tabulated temperature's and `T_eff` the requested one's.
///
/// # Errors
/// [`NjoyError::NotPorted`] without incoherent-inelastic (MT=4) data;
/// [`NjoyError::TemperatureNotTabulated`] where `calcem` stops;
/// [`NjoyError::ThermalTemperatureMismatch`] when `mf7` is not the table
/// `calcem` reads for `temp_k`; and those of `calcem` and the MF=3 builders.
#[allow(clippy::too_many_arguments)]
pub fn thermr_tables(
    mf7: &Mf7,
    temp_k: f64,
    natom: f64,
    n_cosines: usize,
    emax_ev: f64,
    tol: f64,
    incident: Option<&IncidentPendf>,
) -> Result<ThermrTables, NjoyError> {
    let ii = mf7
        .incoherent_inelastic
        .as_ref()
        .ok_or(NjoyError::NotPorted(
            "thermal ACE without incoherent-inelastic (MT=4) data",
        ))?;
    // On NJOY's deck the temperature is resolved as THERMR resolves it
    // (GitHub #866): `calcem`'s T/500 walk; S(alpha,beta) and kT at the
    // tabulated temperature it finds (`tev=t*bk`, `thermr.f90:1924`) but
    // T_eff at the requested one (`tempt=temp`, `:1812-1834`); and
    // `rdelas`'s Bragg table.
    let ii = match incident {
        Some(_) => {
            let calcem_k = crate::thermr::mf7::calcem_temperature_k(&ii.tabulated_temperatures_k, temp_k)?;
            if ii.temperature_bracket.is_some() || ii.temperature_k != calcem_k {
                return Err(NjoyError::ThermalTemperatureMismatch {
                    requested_k: temp_k,
                    calcem_k,
                    resolved_k: ii.temperature_k,
                    interpolated: ii.temperature_bracket.is_some(),
                });
            }
            IncoherentInelastic { teff_request_k: Some(temp_k), ..ii.clone() }
        }
        None => ii.clone(),
    };
    let bragg_s: Option<Vec<(f64, f64)>> = match (&mf7.coherent_elastic, incident) {
        (Some(ce), Some(_)) => Some(ce.s_of_e_rdelas(temp_k)),
        (Some(ce), None) => Some(ce.s_of_e_at(temp_k)?),
        (None, _) => None,
    };
    // TODO(nmix>1): `natom` is a single scalar — no support for mixing
    // multiple distinct bound-atom populations into one thermal table
    // (`nmix` in `aceth.f90`). Every material here is treated as nmix=1.
    let calcem = compute_iform0(&ii, natom, n_cosines, emax_ev, tol)?;
    // THERMR's MF=3 on the incident PENDF's grid (GitHub #865), for `mtref`
    // and, with incoherent elastic, for `mtei` (`iel`, GitHub #866).
    let (mut inelastic_mf3, mut iel_mf3) = (None, None);
    if let Some(inc) = incident {
        use crate::thermr::mf3_grid;
        let (esi, xsi): (Vec<f64>, Vec<f64>) =
            calcem.records.iter().map(|r| (r.e_in_ev, r.cross_section_b)).unzip();
        let grid = mf3_grid::thermr_grid(&inc.elastic, emax_ev, &esi, &xsi, bragg_s.as_deref(), tol)?;
        inelastic_mf3 = Some(mf3_grid::tpend_mf3(&grid)?);
        if let Some(ie) = &mf7.incoherent_elastic {
            let dwa = ie.iel_dwa(temp_k)?;
            let xie: Vec<f64> = esi.iter().map(|&e| ie.iel_record(e, dwa, natom, n_cosines).0).collect();
            iel_mf3 = Some(mf3_grid::incoherent_elastic_mf3(&grid, &esi, &xie)?);
        }
    }
    Ok(ThermrTables {
        temperature_k: temp_k,
        inelastic_law: ii,
        mf7_awr: mf7.awr,
        natom,
        n_cosines,
        emax_ev,
        tol,
        inelastic: calcem,
        incident: incident.cloned(),
        inelastic_mf3,
        coherent_elastic_s: bragg_s,
        incoherent_elastic: mf7.incoherent_elastic.clone(),
        incoherent_elastic_mf3: iel_mf3,
    })
}
