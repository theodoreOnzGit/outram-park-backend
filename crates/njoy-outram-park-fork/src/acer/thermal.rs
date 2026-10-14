//! Thermal scattering **S(α,β)** ACE table writer (`…t` tables).
//!
//! Assembles the thermal ACE table a Monte-Carlo code uses for bound-atom
//! thermal scattering — graphite, H/D in water, ZrH, Al, … — where the free-gas
//! model no longer applies and the scattering law S(α,β) governs the kinematics.
//! Ports the load path of `aceth.f90` (`thrlod`) for the `IFENG=0/1/2`
//! inelastic forms (equiprobable, skewed, continuous; ~~`IFENG=0` only~~,
//! corrected 2026-10-04) plus coherent-elastic (Bragg) data.
//!
//! The physics comes from [`crate::thermr`]: [`IncoherentInelastic`] provides
//! the inelastic cross section and equiprobable emission bins, and
//! [`CoherentElastic`] provides the Bragg `S(E)`.
//!
//! ## Thermal NXS / JXS (distinct from the continuous-energy table)
//!
//! - **NXS**: `IDPNI` (inelastic mode = 3), `NIL` (= nang−1 discrete cosines),
//!   `NIEB` (outgoing energies per incident energy), `IDPNC` (elastic mode: 4
//!   coherent, 0 none), `NCL` (−1 for coherent), `IFENG` (0 equiprobable, 1 skewed, 2 continuous).
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
//! On the incident PENDF (`thermal_from_mf7_on_pendf`) the `IDPNC=3` blocks
//! are NJOY2016's word for word on H in ZrH (GitHub #866); `IDPNC=5` is
//! written the same way but no evaluation here exercises it.
//! ~~Not yet handled: the skewed/continuous `IFENG=1/2` inelastic forms, and~~
//! ~~multi-atom mixing (`nmix`, taken as 1).~~ **CORRECTED 2026-10-04:** all
//! three `IFENG` forms are written since 2026-09-22 (`InelasticForm`,
//! `ThermalAceOptions::form`; `tests/acer_thermal_ifeng_vs_njoy2016.rs`).
//! Still not handled: multi-atom mixing (`nmix`, taken as 1).
//!
//! ## Prerequisite provenance
//!
//! - **THERMR** (`thermr.f90`) — the physics ported in [`crate::thermr`].
//! - **LEAPR** (`leapr.f90`) — *generates* MF=7 when an evaluation lacks it;
//!   optional, since the ENDF/B thermal sublibrary ships MF=7.

use crate::acer::acesix::{
    acesix_equiprobable, acesix_tabulated, pendf_text, tpend_rows, AcesixPoint, BinWeights,
};
use crate::thermr::calcem::iform0::compute_iform0;
use crate::thermr::calcem::types::{Iform0Table, IncidentEnergyRecord};
use crate::thermr::inelastic::OutgoingBin;
use crate::thermr::mf7::{IncoherentInelastic, Mf7};
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

/// The incident energies ACER keeps from THERMR's grid (GitHub #865):
/// `calcem` runs on [`EGRID`](crate::thermr::calcem::egrid::EGRID) up to and
/// including the first energy above `emax`, and `acesix` stops at the first
/// incident energy above `emax` or within `eps = 1e-5` eV of it
/// (`aceth.f90:329-330`). This is the `energy_grid` to pass for NJOY's table.
pub fn ace_incident_grid(emax_ev: f64) -> Vec<f64> {
    const EPS: f64 = 1.0e-5;
    crate::thermr::calcem::egrid::EGRID
        .iter()
        .copied()
        .take_while(|&e| !(e > emax_ev || (e - emax_ev).abs() < EPS))
        .collect()
}

/// The incident nuclide's PENDF at the thermal temperature, as NJOY's thermal
/// deck has it before THERMR: its MAT and its elastic (MT=2) cross section.
/// Build it with RECONR + BROADR at the S(α,β) temperature.
#[derive(Debug, Clone)]
pub struct IncidentPendf {
    /// The incident evaluation's MAT (NJOY writes it in the header).
    pub mat: i32,
    /// ACER card 8a: the moderator component ZA values (at most 16) NJOY
    /// writes into the header's IZ entries, e.g. `[13027]` for Al-27 or
    /// `[1001]` for H in H2O.
    pub moderator_za: Vec<i32>,
    /// The incident evaluation's AWR: ACER's header AWR is the PENDF's
    /// (`aw0`, `aceth.f90:121`), not the thermal tape's (graphite: 11.89365
    /// for C-12 against 11.898 on the MF=7 tape).
    pub awr: f64,
    /// MT=2 `(E [eV], σ [b])`, ascending.
    pub elastic: Vec<(f64, f64)>,
}

/// THERMR card-4 `tol` used by [`AceTable::thermal_from_mf7`]: 0.05, the value
/// the byte-parity comparisons in `verification_and_validation/` were taken at.
///
/// **Not every deck uses it.** OpenMC's `openmc.data.njoy.make_ace_thermal`
/// passes `error = 0.001` as THERMR's `tol` (`njoy.py`, `_THERMAL_TEMPLATE_THERMR`),
/// so a table meant to match an OpenMC-library NJOY2016 table must be built with
/// [`AceTable::thermal_from_mf7_with_tolerance`]. Found 2026-09-29: the five-route
/// ICSBEP study's Rust H(H2O) table had been built at 0.05 against an NJOY2016
/// reference at 0.001.
pub const CALCEM_TOL: f64 = 0.05;

/// The Bragg edges NJOY's ACER writes, and the cumulative `E·σ` at each
/// \[eV, eV·b\], from the evaluation's cumulative `S(E)` table (GitHub #865).
///
/// THERMR's `coh`/`sigcoh` (`thermr.f90:748-922`, `1146-1195`) puts a PENDF
/// point one 7th-figure unit either side of every edge, `sigfig(elim,7,∓1)`
/// with `elim = (E·econ)·recon`, where `σ = (Σ f_i)·(1/E)` over the edges
/// below the point, `f_i` being the tape's increments; `tpend` writes each
/// point as `sigfig(E,7)`, `sigfig(σ,7)` (6 below 1e-9, `thermr.f90:3217-
/// 3221`). ACER (`aceth.f90:188-214`) walks that MF=3 back from text and
/// keeps a point only where `cnow = E·σ` rises above the last kept value by
/// more than `(clast + 1)/10⁶`, stopping when the next point reaches `emax`.
/// Edges under a millionth of the running total are folded into a later one,
/// and where that happens the 7-figure rounding of both points decides which.
///
/// The PENDF's other points (the incident tape's grid and `coh`'s bisection
/// points) are not rebuilt: between two edges they carry the same `S` and
/// only rounding noise. Checked on Al-27 at 20 K: ACER's loop over NJOY2016's
/// own PENDF and over these two points per edge keep the same 309 edges.
///
/// # Errors
/// As [`pendf_text`].
pub fn aceth_bragg_edges(s_of_e: &[(f64, f64)], emax_ev: f64) -> Result<Vec<(f64, f64)>, NjoyError> {
    use crate::common::phys::{AMASSN_AMU, AMU_G, EV_ERG, HBAR_ERG_S};
    use crate::mixr::mix::sigfig;
    let amne = AMASSN_AMU * AMU_G;
    let econ = EV_ERG * 8.0 * (amne / HBAR_ERG_S) / HBAR_ERG_S;
    let recon = 1.0 / econ;
    let point = |e: f64, sum: f64| -> Result<(f64, f64), NjoyError> {
        let sigma = sum * (1.0 / e);
        let sigma = if sigma >= 1.0e-9 { sigfig(sigma, 7, 0) } else { sigfig(sigma, 6, 0) };
        Ok((pendf_text(sigfig(e, 7, 0))?, pendf_text(sigma)?))
    };
    // The PENDF points below emax: just under each edge (the sum without it)
    // and just over it (the sum with it).
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(2 * s_of_e.len());
    let (mut sum, mut blast) = (0.0_f64, 0.0_f64);
    for &(e_tape, s_cum) in s_of_e {
        let elim = (e_tape * econ) * recon;
        let (below, above) = (sigfig(elim, 7, -1), sigfig(elim, 7, 1));
        if above >= emax_ev {
            break;
        }
        pts.push(point(below, sum)?);
        sum += s_cum - blast; // `fl(l+1) = S_i - S_(i-1)`, summed in order
        blast = s_cum;
        pts.push(point(above, sum)?);
    }
    let mut kept = Vec::new();
    let mut clast = 0.0_f64;
    for (k, &(e, x)) in pts.iter().enumerate() {
        // `aceth.f90:200`. After the last edge `coh` still has points below
        // emax, so only a following edge point can end the walk.
        if let Some(&(enext, _)) = pts.get(k + 1) {
            if enext > emax_ev || (enext - emax_ev).abs() <= 1.0e-5 {
                break;
            }
        }
        let cnow = e * x;
        if cnow - clast > (clast + 1.0) / 1_000_000.0 {
            kept.push((e, cnow));
            clast = cnow;
        }
    }
    Ok(kept)
}

/// The incoherent-elastic rows ACER writes, `(E [eV], σ [b], cosines)`, as
/// `acesix` reads them back from THERMR's PENDF (`aceth.f90:229-279`,
/// GitHub #866).
///
/// THERMR's `iel` (`thermr.f90:1244-1425`) writes one MF=6 LIST per `calcem`
/// energy `esi` holding `E` and its `nbin` equally probable cosines, and an
/// MF=3 section on THERMR's grid (`iel_mf3`, from
/// [`crate::thermr::mf3_grid::incoherent_elastic_mf3`]). ACER walks the MF=6
/// energies as read from the text, stopping at the first above `emax` or
/// within `eps = 1e-5` eV of it, takes the cross section by `gety1` on that
/// MF=3 (linear between its 7-figure points, not `iel`'s own value), and the
/// cosines as written: through the ENDF text, unrounded and unclamped.
///
/// # Errors
/// As [`pendf_text`].
fn incoherent_elastic_ace(
    ie: &crate::thermr::mf7::IncoherentElastic,
    iel_mf3: &[(f64, f64)],
    esi: &[f64],
    temp_k: f64,
    natom: f64,
    nbin: usize,
    emax_ev: f64,
) -> Result<Vec<(f64, f64, Vec<f64>)>, NjoyError> {
    const EPS: f64 = 1.0e-5;
    let dwa = ie.iel_dwa(temp_k)?;
    let mut rows = Vec::new();
    for &e_raw in esi {
        let e = pendf_text(e_raw)?;
        if e > emax_ev || (e - emax_ev).abs() < EPS {
            break;
        }
        let x = crate::thermr::mf3_grid::itix_at(iel_mf3, e);
        let mus = ie.iel_record(e_raw, dwa, natom, nbin).1;
        let mus = mus.into_iter().map(pendf_text).collect::<Result<Vec<_>, _>>()?;
        rows.push((e, x, mus));
    }
    Ok(rows)
}

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

/// Port of THERMR's `terp` (`thermr.f90`): Lagrangian interpolation (or
/// extrapolation) of order `il` on an **increasing** table, returning the
/// tabulated value when `arg` matches a node to `1e-10` relative. `calcem`'s
/// incident grid is increasing, which is the only case this serves; the
/// decreasing-table branch of the Fortran is not needed and not ported.
pub(crate) fn thermr_terp(x: &[f64], y: &[f64], arg: f64, il: usize) -> f64 {
    const SMALL: f64 = 1.0e-10;
    let nl = x.len();
    if nl == 0 {
        return 0.0;
    }
    let il = il.min(nl);
    // 1-based start index `l` of the il-point stencil, as in the Fortran.
    let l: usize = if nl == il {
        1
    } else {
        let il2 = il / 2;
        let iadd = il % 2;
        let ilow = il2 + 1;
        let ihi = nl - il2 - iadd;
        let (iusel, iuseh, ibeg, iend) = (1, nl - il + 1, ilow + 1, ihi - 1);
        let last = iend + 1 - il2;
        let xx = |i: usize| x[i - 1];
        if (arg - xx(ilow)).abs() < SMALL * arg {
            return y[ilow - 1];
        }
        if arg <= xx(ilow) {
            iusel
        } else if (xx(ihi) - arg).abs() < SMALL * arg {
            return y[ihi - 1];
        } else if xx(ihi) <= arg {
            iuseh
        } else {
            let mut found = None;
            for m in ibeg..=iend {
                if (xx(m) - arg).abs() < SMALL * arg {
                    return y[m - 1];
                }
                if xx(m) > arg {
                    found = Some(m - il2);
                    break;
                }
            }
            found.unwrap_or(last)
        }
    };
    let mut sum = 0.0;
    for i in 1..=il {
        let (mut p, mut pk) = (1.0, 1.0);
        let inn = l + i - 1;
        for ip in 1..=il {
            if ip != i {
                let inp = l + ip - 1;
                p *= arg - x[inp - 1];
                pk *= x[inn - 1] - x[inp - 1];
            }
        }
        sum += p * y[inn - 1] / pk;
    }
    sum
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
        Self::thermal_from_mf7_with_tolerance(
            mf7,
            temp_k,
            name,
            suffix,
            energy_grid,
            opts,
            CALCEM_TOL,
        )
    }

    /// [`Self::thermal_from_mf7`] with THERMR's card-4 `tol` supplied by the
    /// caller, the convergence tolerance `calcem` builds its `E'`/`mu` grids to.
    /// Pass the tolerance of the NJOY deck being matched; see [`CALCEM_TOL`].
    ///
    /// # Errors
    /// As [`Self::thermal_from_mf7`].
    pub fn thermal_from_mf7_with_tolerance(
        mf7: &Mf7,
        temp_k: f64,
        name: &str,
        suffix: u32,
        energy_grid: &[f64],
        opts: ThermalAceOptions,
        tol: f64,
    ) -> Result<Self, NjoyError> {
        Self::thermal_build(mf7, temp_k, name, suffix, energy_grid, opts, tol, None)
    }

    /// [`Self::thermal_from_mf7_with_tolerance`] on the incident nuclide's
    /// PENDF, the way NJOY's `reconr -> broadr -> thermr -> acer` deck builds
    /// it (GitHub #865): `ITIX` is then interpolated from THERMR's MF=3 on
    /// that grid ([`crate::thermr::mf3_grid`]) and the header carries the
    /// incident MAT. With this, the Al-27 table at 20 K is NJOY2016's in every
    /// word (`tests/acer_thermal_word_parity.rs`).
    ///
    /// **Temperature, as THERMR resolves it** (GitHub #866). `mf7` must be
    /// parsed at `temp_k` (`parse_mf7_at_temperature(.., Some(temp_k))`), and
    /// `temp_k` must be one `calcem` accepts: within `T/500` of a tabulated
    /// MF=7/MT=4 temperature ([`crate::thermr::mf7::calcem_temperature_k`]).
    /// NJOY2016 never interpolates in temperature and stops otherwise; so does
    /// this path, where [`Self::thermal_from_mf7`] would interpolate. As in
    /// `calcem`, `S(α,β)` and `kT` are then the tabulated temperature's and
    /// `T_eff` the requested temperature's, and the Bragg `S(E)` is `rdelas`'s
    /// ([`crate::thermr::mf7::CoherentElastic::s_of_e_rdelas`]). Word for word
    /// NJOY2016's at 600 K and 600.5 K on graphite and at 293.6 K on H in
    /// H₂O (`tests/acer_thermal_temperature_parity.rs`).
    ///
    /// # Errors
    /// As [`Self::thermal_from_mf7`], plus
    /// [`NjoyError::TemperatureNotTabulated`] where `calcem` stops, and
    /// [`NjoyError::ThermalTemperatureMismatch`] when `mf7`'s S(α,β) is not
    /// the table `calcem` reads for `temp_k`.
    #[allow(clippy::too_many_arguments)]
    pub fn thermal_from_mf7_on_pendf(
        mf7: &Mf7,
        temp_k: f64,
        name: &str,
        suffix: u32,
        energy_grid: &[f64],
        opts: ThermalAceOptions,
        tol: f64,
        incident: &IncidentPendf,
    ) -> Result<Self, NjoyError> {
        Self::thermal_build(mf7, temp_k, name, suffix, energy_grid, opts, tol, Some(incident))
    }

    #[allow(clippy::too_many_arguments)]
    fn thermal_build(
        mf7: &Mf7,
        temp_k: f64,
        name: &str,
        suffix: u32,
        energy_grid: &[f64],
        opts: ThermalAceOptions,
        tol: f64,
        incident: Option<&IncidentPendf>,
    ) -> Result<Self, NjoyError> {
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
        let ii_at_request;
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
                ii_at_request = IncoherentInelastic { teff_request_k: Some(temp_k), ..ii.clone() };
                &ii_at_request
            }
            None => ii,
        };
        let bragg_s: Option<Vec<(f64, f64)>> = match (&mf7.coherent_elastic, incident) {
            (Some(ce), Some(_)) => Some(ce.s_of_e_rdelas(temp_k)),
            (Some(ce), None) => Some(ce.s_of_e_at(temp_k)?),
            (None, _) => None,
        };

        let nei = energy_grid.len();
        let nieb = opts.n_outgoing;
        let nang = opts.n_cosines;
        // TODO(nmix>1): `natom` is a single scalar — no support for mixing
        // multiple distinct bound-atom populations into one thermal table
        // (`nmix` in `aceth.f90`). Every material here is treated as nmix=1.
        let natom = opts.natom;

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
        let calcem = compute_iform0(ii, natom, nang, opts.emax_ev, tol)?;
        // ITIX is THERMR's own `xsi` -- `calcem`'s trapezoidal E'-integral of
        // the SAME rows the emission bins below come from (`thermr.f90:2166-
        // 2173`), put on the PENDF grid by `terp(esi, xsi, nne, enow, nlt = 5)`
        // (`thermr.f90:2459`) and copied by `acesix` (`aceth.f90:131`). It was
        // `IncoherentInelastic::cross_section`, an independent analytic
        // integral, until 2026-09-29: up to 6.6 % off NJOY2016's ITIX at
        // 1e-5 eV on H in H2O (median 0.4 %), which is what moved the five-route
        // study's thermal cases by ~95 pcm (route 2 vs route 1).
        let (esi, xsi): (Vec<f64>, Vec<f64>) = calcem
            .records
            .iter()
            .map(|r| (r.e_in_ev, r.cross_section_b))
            .unzip();
        // ACER reads ITIX back from THERMR's PENDF (`aceth.f90:355`, `gety1`
        // on MF=3), GitHub #865. On the incident nuclide's grid that is
        // THERMR's own MF=3; without it, `calcem`'s value at each energy
        // through the same 7-figure text, which differs between grid points.
        // THERMR's MF=3 for `mtei` too, when there is incoherent elastic and
        // the incident PENDF is known (`iel`, see `incoherent_elastic_ace`).
        let mut iel_mf3: Option<Vec<(f64, f64)>> = None;
        let xs: Vec<f64> = match incident {
            Some(inc) => {
                use crate::thermr::mf3_grid;
                let grid = mf3_grid::thermr_grid(&inc.elastic, opts.emax_ev, &esi, &xsi, bragg_s.as_deref(), tol)?;
                let mf3 = mf3_grid::tpend_mf3(&grid)?;
                if let Some(ie) = &mf7.incoherent_elastic {
                    let dwa = ie.iel_dwa(temp_k)?;
                    let xie: Vec<f64> = esi.iter().map(|&e| ie.iel_record(e, dwa, natom, nang).0).collect();
                    iel_mf3 = Some(mf3_grid::incoherent_elastic_mf3(&grid, &esi, &xie)?);
                }
                energy_grid
                    .iter()
                    .map(|&e| Ok(crate::thermr::mf3_grid::itix_at(&mf3, pendf_text(e)?)))
                    .collect::<Result<_, NjoyError>>()?
            }
            None => energy_grid
                .iter()
                .map(|&e| pendf_text(thermr_terp(&esi, &xsi, e, 5)))
                .collect::<Result<_, _>>()?,
        };
        let mut emission: Vec<Vec<OutgoingBin>> = Vec::new();
        let mut tabulated: Vec<Vec<AcesixPoint>> = Vec::new();
        if opts.form == InelasticForm::Continuous {
            tabulated = energy_grid
                .iter()
                .map(|&e| match nearest_calcem_record(&calcem, e) {
                    Some(rec) => Ok(acesix_tabulated(&tpend_rows(&rec.rows, rec.cross_section_b)?, nang)),
                    None => Ok(Vec::new()),
                })
                .collect::<Result<_, NjoyError>>()?;
        } else {
            let weights = match opts.form {
                InelasticForm::Skewed => BinWeights::Variable,
                _ => BinWeights::Constant,
            };
            emission = energy_grid
                .iter()
                .map(|&e| {
                    let Some(rec) = nearest_calcem_record(&calcem, e) else {
                        return Ok(Vec::new());
                    };
                    Ok(acesix_equiprobable(&tpend_rows(&rec.rows, rec.cross_section_b)?, nieb, weights)
                        .into_iter()
                        .map(|b| OutgoingBin {
                            e_out_ev: b.e_out_ev,
                            cosines: b.cosines,
                        })
                        .collect())
                })
                .collect::<Result<_, NjoyError>>()?;
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
                        real(
                            p.cosines.get(j).copied().unwrap_or(0.0),
                            &mut xss,
                            &mut is_int,
                        );
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
        // On NJOY's deck (`incident`) it is `rdelas`'s table instead, never
        // interpolated (GitHub #866).
        if let Some(bragg_s) = &bragg_s {
            // ~~Every edge on the tape is written.~~ **CORRECTED 2026-10-14
            // (GitHub #865):** the edges ACER keeps, as it finds them on
            // THERMR's PENDF ([`aceth_bragg_edges`]).
            let edges = aceth_bragg_edges(bragg_s, opts.emax_ev)?;
            let nee = edges.len();
            itce = xss.len() as i32 + 1;
            xss.push(nee as f64);
            is_int.push(true);
            for &(e, _) in &edges {
                real(e / EMEV, &mut xss, &mut is_int);
            }
            itcx = xss.len() as i32 + 1;
            for &(_, s) in &edges {
                real(s / EMEV / natom, &mut xss, &mut is_int); // cumulative S·E [MeV·b]
            }
            idpnc = 4; // coherent elastic
            ncl = -1;
        }

        // Incoherent-elastic: NEE energies, σ, and NEA equally-probable cosines.
        if let Some(ie) = &mf7.incoherent_elastic {
            let nea = nang; // reuse the inelastic cosine count for the elastic bins
            // `(E [eV], σ [b], cosines)` per incident energy. On the incident
            // PENDF, as ACER reads them back from THERMR's (GitHub #866):
            // `incoherent_elastic_ace`. Without it, the analytic values on
            // `energy_grid`, which are close to NJOY's but not its words.
            let rows: Vec<(f64, f64, Vec<f64>)> = match &iel_mf3 {
                Some(mf3) => incoherent_elastic_ace(ie, mf3, &esi, temp_k, natom, nea, opts.emax_ev)?,
                None => energy_grid
                    .iter()
                    .map(|&e| (e, ie.cross_section(e, temp_k, natom), ie.equiprobable_cosines(e, temp_k, nea)))
                    .collect(),
            };
            let nee = rows.len();
            let e_start = xss.len() as i32 + 1;
            xss.push(nee as f64);
            is_int.push(true);
            for (e, _, _) in &rows {
                real(e / EMEV, &mut xss, &mut is_int);
            }
            let x_start = xss.len() as i32 + 1;
            for (_, x, _) in &rows {
                // `aceth.f90:804`, `x/nmix`; `nmix = 1` here (see the TODO
                // on `natom` above).
                real(*x, &mut xss, &mut is_int);
            }
            let a_start = xss.len() as i32 + 1;
            for (_, _, mus) in &rows {
                for &mu in mus {
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
        // ACER right-justifies `tname` in 6 columns (`acer.f90:386-387`) and
        // writes `(a6,f3.2,'t')` (`aceth.f90:665`): `  al27.00t`.
        let zaid = format!("{name:>6}.{suffix:02}t");
        let mut iz = [0i32; 16];
        if let Some(inc) = incident {
            for (k, &za) in inc.moderator_za.iter().take(16).enumerate() {
                iz[k] = za;
            }
        }
        // `'   mat',i4` of the PENDF material (`aceth.f90`), when known.
        let mat_id = match incident {
            Some(inc) => format!("   mat{:4}", inc.mat),
            None => format!("{:>10}", "mat  "),
        };

        Ok(AceTable {
            zaid,
            awr: incident.map_or(mf7.awr, |inc| inc.awr),
            iz,
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
