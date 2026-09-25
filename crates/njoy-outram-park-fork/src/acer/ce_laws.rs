// SPDX-License-Identifier: GPL-3.0

//! **The ACE secondary-distribution blocks: AND, DLW and NU.**
//!
//! [`super::ce_decode`] turns ESZ/MTR/LQR/LSIG/SIG into cross sections. Cross
//! sections alone cannot transport: a collision also needs an outgoing energy
//! and a scattering cosine. This module decodes the blocks that carry them.
//!
//! # Upstream is the specification
//!
//! OpenMC's C++ reads HDF5, so its ACE reader is Python, read at `afa7a14`:
//!
//! | block | upstream |
//! |---|---|
//! | AND (tabulated cosines) | `openmc/data/angle_distribution.py::from_ace` |
//! | DLW LAW=4 | `openmc/data/energy_distribution.py::ContinuousTabular.from_ace` |
//! | DLW LAW=44 | `openmc/data/kalbach_mann.py::KalbachMann.from_ace` |
//! | DLW LAW=61 | `openmc/data/correlated.py::CorrelatedAngleEnergy.from_ace` |
//! | DLW LAW=2 | `openmc/data/energy_distribution.py::DiscretePhoton.from_ace` |
//! | DLW LAW=3, 33 | `openmc/data/energy_distribution.py::LevelInelastic.from_ace` |
//! | DLW LAW=7 | `openmc/data/energy_distribution.py::MaxwellEnergy.from_ace` |
//! | DLW LAW=9 | `openmc/data/energy_distribution.py::Evaporation.from_ace` |
//! | DLW LAW=11 | `openmc/data/energy_distribution.py::WattEnergy.from_ace` |
//! | DLW LAW=66 | `openmc/data/nbody.py::NBodyPhaseSpace.from_ace` |
//! | the `LNW` chain | `openmc/data/reaction.py:1082-1092` |
//!
//! The dispatch those rows hang off is `angle_energy.py::AngleEnergy.from_ace`,
//! which accepts exactly `{2, 3, 33, 4, 5, 7, 9, 11, 44, 61, 66}` and raises on
//! anything else. **LAW=5's own `from_ace` then raises `NotImplementedError`**
//! (`energy_distribution.py:192-194`), so upstream dispatches law 5 and cannot
//! read it; this module refuses it too, and says so in the error rather than
//! implying the gap is ours alone.
//!
//! # The three laws share one structure
//!
//! LAW=4, 44 and 61 have an **identical outer layout** -- interpolation
//! regions, the incident-energy grid, and a locator per incident energy -- and
//! identical per-energy headers. They differ only in how many parallel arrays
//! follow the outgoing-energy grid:
//!
//! | law | arrays | what the extra ones are |
//! |---|---|---|
//! | 4 | 3 | `E'`, pdf, cdf. The cosine comes from the AND block. |
//! | 44 | 5 | + Kalbach `r` and `a` (pre-compound fraction and slope) |
//! | 61 | 4 | + a locator per `E'` to its own tabulated cosine distribution |
//!
//! So this is one reader with an array count, not three readers. Writing it as
//! three would have triplicated the discrete-line handling, which is the part
//! that is easy to get wrong.
//!
//! Measured over the NJOY2016 reference library in `reference-data/ace`, those
//! three plus LAW=3 are the only laws the **actinide** tables use: U-235 and
//! U-238 use `{LAW3: 39, LAW4: 1, LAW61: 4}`, U-234 `{LAW3: 40, LAW4: 4,
//! LAW44: 4}`, and the delayed-neutron block (DNED) is `{LAW4: 6}` on all
//! three. LAW=3 is discrete two-body level scattering and carries no tabulated
//! data at all -- just `Q` and the mass ratio, from which the kinematics are
//! analytic.
//!
//! # ~~Only 3, 4, 44 and 61 occur~~ **CORRECTED 2026-09-25** -- four more do
//!
//! That census was true of the actinides it was taken over and was read as
//! though it were true of ACE. It is not. Running NJOY2016 (the same
//! `RECONR+ACER` deck, 0 K) over four more tapes already committed in
//! `reference-data/endf/` produces laws this module used to refuse outright:
//!
//! | tape | MT | ACE law | from |
//! |---|---|---|---|
//! | H-2 ENDF/B-VIII.0 | 16 | **66** (`n`-body phase space) | MF=6 LAW=6 |
//! | C-12 ENDF/B-VIII.0 | 28, 91 | **9** (evaporation) | MF=5 LF=9 |
//! | Na-23 ENDF/B-VIII.0 | 16 | **9** | MF=5 LF=9 |
//! | Na-23 ENDF/B-VIII.0 | 91 | **9, 9** -- a two-law `LNW` chain | MF=5 LF=9, NK=2 |
//! | Be-9 ENDF/B-VIII.0 | 16 | 61 | MF=6 **LAW=7** |
//!
//! The Be-9 row is the useful negative: its evaluation is MF=6 **LAW=7**
//! (laboratory angle-energy), the one representation an ACE reader might expect
//! to meet as law 67 -- and NJOY's ACER does not write law 67, it converts the
//! section to ACE **law 61**, which this module already read. (This port reads
//! the ENDF side of that law too, through
//! [`crate::acer::energy::parse_mf6_law7_lab_angle_energy`] and the conversion
//! in `ContinuumEmission::from_endf_mf6`; upstream OpenMC reads MF=6 LAW=7 from
//! ENDF but refuses ACE law 67.) So no held table needs law 67.
//!
//! The Na-23 row is the other useful one: `LNW != 0` is not hypothetical.
//! MT=91 carries two evaporation laws with applicability functions that switch
//! over at 12 MeV (`p` goes 1 -> 0 on the first, 0 -> 1 on the second), so
//! refusing the chain refused a common light-nuclide reaction, not an exotic
//! one.
//!
//! # Units
//!
//! ACE stores energies in **MeV** and pdfs per MeV. `ChiEout` and `NuBar` are
//! eV-based, so every energy is scaled on the way out and every pdf is scaled
//! by the inverse.

use crate::acer::read::RawAceTable;
use crate::acer::{jxs, nxs};
use crate::acer::angular::{ElasticAngular, EnergyAngular};
use crate::acer::ce_decode::EV_PER_MEV;
use crate::error::NjoyError;
use crate::endf::records::{Cont, Tab1};
use crate::nuclear_data::secondary::{
    ChiEout, ChiTabular, ContinuumAngular, ContinuumAngularRow, ContinuumAngularTable,
    ContinuumKalbachRow, ContinuumKalbachTable, FissionSpectrum, NuBar,
};

fn need(t: &RawAceTable, at: usize, n: usize, what: &str) -> Result<(), NjoyError> {
    if at + n > t.xss.len() {
        return Err(NjoyError::EndfParse(format!(
            "ACE {what}: words {at}..{} run past XSS ({})",
            at + n,
            t.xss.len()
        )));
    }
    Ok(())
}

/// One incident-energy row of a tabulated outgoing-energy law.
#[derive(Debug, Clone)]
pub struct LawEnergyRow {
    /// Incident energy \[eV\].
    pub e_in: f64,
    /// Outgoing-energy table, already in eV.
    pub eout: ChiEout,
    /// Kalbach `r`/`a` per outgoing energy (LAW=44 only).
    pub kalbach: Option<Vec<ContinuumKalbachRow>>,
    /// Tabulated cosine per outgoing energy (LAW=61 only).
    pub cosines: Option<Vec<ContinuumAngularRow>>,
}

/// A decoded DLW entry.
#[derive(Debug, Clone)]
pub enum AceEnergyLaw {
    /// **LAW=3** -- discrete two-body level scattering. Carries no table: the
    /// outgoing energy follows from `Q` and the mass ratio, and the cosine
    /// comes from the AND block.
    TwoBodyLevel {
        /// `(A+1)/A * |Q|` as ACE stores it \[eV\].
        ldat1: f64,
        /// `(A/(A+1))^2` as ACE stores it.
        ldat2: f64,
    },
    /// **LAW=4/44/61** -- a tabulated outgoing-energy law, optionally carrying
    /// its own correlated angular data.
    Tabulated {
        /// 4, 44 or 61, kept so a consumer can tell a correlated law from one
        /// that defers its cosine to the AND block.
        law: i32,
        rows: Vec<LawEnergyRow>,
        /// The evaluation's `(NBT, INT)` over the incident grid; empty means
        /// ACE's `NR = 0` default of a single lin-lin range.
        incident_interp: Vec<(u32, u32)>,
    },
    /// **LAW=2** -- a single discrete line.
    ///
    /// A *neutron* DLW block does not use it; it is the photon-production
    /// (DLWP) representation of a discrete gamma, and upstream dispatches it
    /// from the same `AngleEnergy.from_ace` this module mirrors
    /// (`angle_energy.py:83-85`). Decoded rather than refused so the two blocks
    /// can share one reader, and because a two-word law is not where a port
    /// should draw a line.
    DiscretePhoton {
        /// `LP`: `0`/`1` the line sits at [`eg`](Self::DiscretePhoton::eg);
        /// `2` it is a primary photon at `eg + AWR/(AWR+1) * E`.
        lp: i32,
        /// `EG` \[eV\].
        eg: f64,
    },
    /// **LAW=7/9/11** -- an analytic outgoing-energy law whose parameters are
    /// themselves tabulated against the *incident* energy.
    ///
    /// Carried as this crate's [`FissionSpectrum`], which despite the name is
    /// simply its representation of an ENDF **MF=5** section: ACE law 7/9/11
    /// and ENDF LF=7/9/11 are the same three laws with the same parameters,
    /// differing only in that ACE stores MeV. Reusing the type means the
    /// transport crate's existing exact samplers
    /// (`sample_maxwell_lf7`/`sample_evaporation_lf9`/`sample_watt_lf11`, each a
    /// port of the matching OpenMC C++ sampler) apply to the ACE route with no
    /// second implementation to drift.
    Analytic {
        /// The ACE law code (7, 9 or 11), kept so a consumer can report what
        /// the file said rather than inferring it back from the variant.
        law: i32,
        /// The law itself, parameters in eV (and eV^-1 for Watt's `b`).
        spectrum: FissionSpectrum,
    },
    /// **LAW=66** -- `n`-body phase space.
    ///
    /// ACE stores only the two numbers ENDF's LAW=6 CONT record carries; the
    /// shape in `x = E'/E'_max` is *computed*, by ENDF-6 formula 6.21 on the
    /// grid `acefc.f90`'s `acelf6` uses -- already ported as
    /// [`crate::acer::energy::law66_shape_table`]. `E'_max(E)` additionally
    /// needs the reaction `Q` and the target mass, which live in LQR and the
    /// table header, so the conversion to a tabulated spectrum is
    /// [`ace_phase_space_chi`] rather than something this variant can do alone.
    PhaseSpace {
        /// `NPSX`: the number of particles sharing the phase space (3, 4 or 5).
        npsx: i32,
        /// `APSX`: their total mass in neutron masses.
        apsx: f64,
    },
    /// **An `LNW` chain** -- two or more laws for one reaction, each with an
    /// applicability `p_k(E)` over the incident energy, `sum_k p_k(E) = 1`.
    ///
    /// Upstream walks this chain unconditionally (`reaction.py:1082-1092`); this
    /// port used to refuse it outright, which refused Na-23's MT=91 (two
    /// evaporation laws switching over at 12 MeV). The `Tab1` is the
    /// applicability, read from the words that follow the three-word DLW header.
    Mixture(Vec<(Tab1, AceEnergyLaw)>),
}

impl AceEnergyLaw {
    /// The law as an **MF=5-style outgoing-energy spectrum**, when it is one.
    ///
    /// `Some` for the analytic laws (7/9/11), for LAW=4 (which is ENDF MF=5
    /// LF=1 in ACE's layout, and whose cosine is the AND block's, not its own),
    /// and for a [`Mixture`](Self::Mixture) every member of which converts.
    ///
    /// `None` -- deliberately, not as a failure -- for the laws whose angle is
    /// *correlated with the outgoing energy* (44 and 61) and for those that are
    /// not a spectrum at all (3, 2, 66). Flattening a correlated law into this
    /// representation would silently drop the correlation, which is the one
    /// mistake this method exists to make impossible: a caller that wants those
    /// must handle the variant.
    pub fn as_fission_spectrum(&self) -> Option<FissionSpectrum> {
        match self {
            Self::Analytic { spectrum, .. } => Some(spectrum.clone()),
            Self::Tabulated {
                law: 4,
                rows,
                incident_interp,
            } => Some(FissionSpectrum::ContinuousTabular(
                to_chi_and_angular(rows, 4, incident_interp).0,
            )),
            Self::Mixture(parts) => {
                let mut out = Vec::with_capacity(parts.len());
                for (p, law) in parts {
                    out.push((p.clone(), law.as_fission_spectrum()?));
                }
                Some(FissionSpectrum::Mixture(out))
            }
            _ => None,
        }
    }

    /// The ACE law code, for reporting. A [`Mixture`](Self::Mixture) reports its
    /// first member's, with the chain length, e.g. `9 (x2)`.
    pub fn code(&self) -> String {
        match self {
            Self::TwoBodyLevel { .. } => "3".into(),
            Self::Tabulated { law, .. } => law.to_string(),
            Self::DiscretePhoton { .. } => "2".into(),
            Self::Analytic { law, .. } => law.to_string(),
            Self::PhaseSpace { .. } => "66".into(),
            Self::Mixture(parts) => match parts.first() {
                Some((_, l)) => format!("{} (x{})", l.code(), parts.len()),
                None => "empty chain".into(),
            },
        }
    }
}

/// Read a tabulated cosine distribution (32 equiprobable bins are `intt = 1`
/// histogram; tabulated is `intt = 2`), as stored in AND and in LAW=61.
fn read_cosine_table(t: &RawAceTable, at: usize) -> Result<ContinuumAngularRow, NjoyError> {
    need(t, at, 2, "cosine table header")?;
    let _intt = t.xss[at] as i32;
    let n = t.xss[at + 1] as usize;
    need(t, at + 2, 3 * n, "cosine table body")?;
    let cosines = t.xss[at + 2..at + 2 + n].to_vec();
    let pdf = t.xss[at + 2 + n..at + 2 + 2 * n].to_vec();
    let cdf = t.xss[at + 2 + 2 * n..at + 2 + 3 * n].to_vec();
    // `mubar` is the mean of the tabulated density; computing it here keeps the
    // consumer from having to re-derive it on every sample.
    let mut mubar = 0.0;
    for i in 1..n {
        let dmu = cosines[i] - cosines[i - 1];
        mubar += 0.5 * (cosines[i] * pdf[i] + cosines[i - 1] * pdf[i - 1]) * dmu;
    }
    Ok(ContinuumAngularRow {
        cosines,
        cdf,
        mubar,
    })
}

/// Decode the **AND** block for one reaction index, returning `None` when the
/// distribution is isotropic (`LAND = 0`) and an error when it is stored in
/// DLW (`LAND = -1`, only legal for a correlated law).
///
/// `i` is 0 for elastic and `1..=NR` for the reactions with secondary neutrons,
/// matching the LAND block's own ordering.
/// `lct` is the reference frame ENDF-style: `2` centre of mass, `1` laboratory.
/// ACE does not store it in the AND block -- it is the **sign of TYR** for a
/// reaction, and CM by definition for elastic. Passing it in rather than
/// defaulting keeps the caller honest: a cosine sampled in CM and used as
/// though it were lab is a wrong scattering angle that nothing reports.
pub fn decode_angular(
    t: &RawAceTable,
    i: usize,
    lct: i32,
) -> Result<Option<ElasticAngular>, NjoyError> {
    decode_angular_block(t, t.jxs[jxs::LAND], t.jxs[jxs::AND], i, lct)
}

/// [`decode_angular`] against **named** locator and data blocks, so the
/// photon-production pair (`LANDP`/`ANDP`, `JXS(16..17)`) reads through the same
/// code as the neutron one (`LAND`/`AND`).
///
/// `land`/`and` are the `JXS` values as stored — 1-based into `XSS`, `0` meaning
/// the block is absent. Upstream reads both pairs through one
/// `AngleDistribution.from_ace` for exactly this reason
/// (`reaction.py:686` passes `ace.jxs[17]`, `:357` passes `ace.jxs[9]`).
pub fn decode_angular_block(
    t: &RawAceTable,
    land: i32,
    and: i32,
    i: usize,
    lct: i32,
) -> Result<Option<ElasticAngular>, NjoyError> {
    if land <= 0 {
        return Ok(None);
    }
    need(t, (land - 1) as usize + i, 1, "LAND")?;
    let loc = t.xss[(land - 1) as usize + i] as i64;
    if loc == 0 {
        return Ok(None); // isotropic at every energy
    }
    if loc < 0 {
        // Angle is carried inside the DLW law itself (LAW=44/61).
        return Ok(None);
    }
    if and <= 0 {
        return Err(NjoyError::EndfParse("LAND points into an absent AND block".into()));
    }
    let base = (and - 1) as usize + (loc as usize) - 1;
    need(t, base, 1, "AND header")?;
    let ne = t.xss[base] as usize;
    need(t, base + 1, 2 * ne, "AND energy/locator grid")?;
    let e_grid = t.xss[base + 1..base + 1 + ne].to_vec();
    let locs = t.xss[base + 1 + ne..base + 1 + 2 * ne].to_vec();

    let mut energies = Vec::with_capacity(ne);
    for k in 0..ne {
        let lc = locs[k] as i64;
        if lc == 0 {
            // Isotropic at this incident energy: empty cosine grid is how
            // `EnergyAngular` spells that.
            energies.push(EnergyAngular {
                e_mev: e_grid[k],
                cosines: Vec::new(),
                pdf: Vec::new(),
                cdf: Vec::new(),
            });
            continue;
        }
        // A NEGATIVE locator means tabulated; positive means 32 equiprobable
        // bins. Both are offsets from AND, and both are read from the same
        // place -- the sign is the format flag, not part of the address.
        let at = (and - 1) as usize + (lc.unsigned_abs() as usize) - 1;
        if lc < 0 {
            let row = read_cosine_table(t, at)?;
            need(t, at, 2, "AND tabulated header")?;
            let n = t.xss[at + 1] as usize;
            let pdf = t.xss[at + 2 + n..at + 2 + 2 * n].to_vec();
            energies.push(EnergyAngular {
                e_mev: e_grid[k],
                cosines: row.cosines,
                pdf,
                cdf: row.cdf,
            });
        } else {
            // 32 equiprobable bins: 33 boundaries, uniform within each.
            need(t, at, 33, "AND equiprobable bins")?;
            let b = t.xss[at..at + 33].to_vec();
            let mut cosines = Vec::with_capacity(33);
            let mut pdf = Vec::with_capacity(33);
            let mut cdf = Vec::with_capacity(33);
            for (j, &mu) in b.iter().enumerate() {
                cosines.push(mu);
                cdf.push(j as f64 / 32.0);
            }
            for j in 0..33 {
                // Density of a bin is 1/32 divided by its width; the endpoints
                // take their neighbouring bin's density.
                let (lo, hi) = (j.max(1) - 1, (j + 1).min(32));
                let w = (b[hi] - b[lo]).max(f64::MIN_POSITIVE);
                pdf.push(((hi - lo) as f64 / 32.0) / w);
            }
            energies.push(EnergyAngular {
                e_mev: e_grid[k],
                cosines,
                pdf,
                cdf,
            });
        }
    }
    Ok(Some(ElasticAngular { energies, lct }))
}

/// Decode the **DLW** entry for reaction index `i` (0-based over the `NR`
/// reactions that emit neutrons).
pub fn decode_energy_law(t: &RawAceTable, i: usize) -> Result<AceEnergyLaw, NjoyError> {
    let ldlw = t.jxs[jxs::LDLW];
    let dlw = t.jxs[jxs::DLW];
    if ldlw <= 0 || dlw <= 0 {
        return Err(NjoyError::EndfParse("ACE table has no DLW block".into()));
    }
    need(t, (ldlw - 1) as usize + i, 1, "LDLW")?;
    let loc = t.xss[(ldlw - 1) as usize + i] as i64;
    decode_law_chain(t, (dlw - 1) as usize, loc)
}

/// Walk an `LNW` chain starting at the DLW-relative locator `loc`, where `ldis`
/// is the 0-based first word of the distribution block (DLW for neutrons, DNED
/// for delayed neutrons, DLWP for photons -- the layout is the same).
///
/// Each link is `[LNW, LAW, IDAT]`, the applicability `p_k(E)` TAB1 immediately
/// after it, and the law's own data at the DLW-relative `IDAT`. `LNW` is the
/// locator of the *next* link, `0` ending the chain -- so a single-law reaction
/// is a chain of length one, not a special case. Ported from
/// `openmc/data/reaction.py:1082-1092`, whose `while lnw > 0` loop is the same
/// walk; the one difference is that upstream keeps the applicability on the
/// product and this returns it alongside its law, because nothing here owns a
/// product.
pub fn decode_law_chain(
    t: &RawAceTable,
    ldis: usize,
    mut loc: i64,
) -> Result<AceEnergyLaw, NjoyError> {
    let mut parts: Vec<(Tab1, AceEnergyLaw)> = Vec::new();
    while loc > 0 {
        let base = ldis + (loc as usize) - 1;
        need(t, base, 3, "DLW header")?;
        let next = t.xss[base] as i64;
        let law = t.xss[base + 1] as i32;
        let idat = t.xss[base + 2] as i64;
        // The applicability TAB1 follows the three-word header; the law's data
        // starts at IDAT, which is past it. Upstream reads it at
        // `jxs[11] + lnw + 2`, i.e. exactly here (`reaction.py:1085-1086`).
        let (applicability, _) = read_tab1_full(t, base + 3, 1.0, "DLW applicability")?;
        let decoded = decode_one_law(t, ldis, law, idat)?;
        parts.push((applicability, decoded));
        if next == loc {
            return Err(NjoyError::EndfParse(
                "ACE DLW LNW chain points at itself".into(),
            ));
        }
        loc = next;
    }
    match parts.len() {
        0 => Err(NjoyError::EndfParse(
            "ACE DLW locator is zero: the reaction emits neutrons but carries no law".into(),
        )),
        1 => Ok(parts.pop().expect("length checked").1),
        _ => Ok(AceEnergyLaw::Mixture(parts)),
    }
}

/// Decode one law of a chain. `idat` is the DLW-relative 1-based locator of the
/// law's own data, as the header's third word gives it.
fn decode_one_law(
    t: &RawAceTable,
    ldis: usize,
    law: i32,
    idat: i64,
) -> Result<AceEnergyLaw, NjoyError> {
    let at = ldis + (idat as usize) - 1;
    match law {
        // LAW=33 is LAW=3 with the same two words; upstream reads them through
        // the same `LevelInelastic.from_ace` (`angle_energy.py:86-88`).
        3 | 33 => {
            need(t, at, 2, "LAW=3 data")?;
            Ok(AceEnergyLaw::TwoBodyLevel {
                ldat1: t.xss[at] * EV_PER_MEV,
                ldat2: t.xss[at + 1],
            })
        }
        2 => {
            need(t, at, 2, "LAW=2 data")?;
            Ok(AceEnergyLaw::DiscretePhoton {
                lp: t.xss[at] as i32,
                eg: t.xss[at + 1] * EV_PER_MEV,
            })
        }
        4 | 44 | 61 => {
            let n_arrays = match law {
                4 => 3,
                61 => 4,
                _ => 5,
            };
            let (rows, incident_interp) = read_tabulated_law(t, at, ldis, n_arrays, law)?;
            Ok(AceEnergyLaw::Tabulated {
                law,
                rows,
                incident_interp,
            })
        }
        7 | 9 => {
            // theta(E) [MeV -> eV], then the restriction energy U. Upstream
            // reads U at `idx + 2 + 2*nr + 2*ne`
            // (`energy_distribution.py:324-326`), which is the word the TAB1
            // reader stops on.
            let (theta, next) = read_tab1_full(t, at, EV_PER_MEV, "LAW=7/9 theta(E)")?;
            need(t, next, 1, "LAW=7/9 restriction energy")?;
            let u = t.xss[next] * EV_PER_MEV;
            let spectrum = if law == 7 {
                FissionSpectrum::Maxwell { theta, u }
            } else {
                FissionSpectrum::Evaporation { theta, u }
            };
            Ok(AceEnergyLaw::Analytic { law, spectrum })
        }
        11 => {
            // a(E) is an energy [MeV -> eV]; b(E) is an inverse energy, so it
            // scales by the INVERSE (`energy_distribution.py:601-611`). Scaling
            // both the same way is the classic error here and it does not fail,
            // it just hardens the spectrum by 12 orders of magnitude.
            let (a, next) = read_tab1_full(t, at, EV_PER_MEV, "LAW=11 a(E)")?;
            let (b, next) = read_tab1_full(t, next, 1.0 / EV_PER_MEV, "LAW=11 b(E)")?;
            need(t, next, 1, "LAW=11 restriction energy")?;
            let u = t.xss[next] * EV_PER_MEV;
            Ok(AceEnergyLaw::Analytic {
                law,
                spectrum: FissionSpectrum::WattEnergyDependent { a, b, u },
            })
        }
        66 => {
            need(t, at, 2, "LAW=66 data")?;
            Ok(AceEnergyLaw::PhaseSpace {
                npsx: t.xss[at] as i32,
                apsx: t.xss[at + 1],
            })
        }
        5 => Err(NjoyError::NotPorted(
            "ACE DLW LAW=5 (general evaporation). Upstream dispatches it and then \
             raises NotImplementedError in `GeneralEvaporation.from_ace` \
             (openmc/data/energy_distribution.py:192-194), and this crate's ENDF \
             route does not read the identical MF=5 LF=5 either. NJOY2016 does \
             support LF=5 (groupr.f90:12355, acefc.f90:2251) but linearises the \
             one place it occurs in reference-data/endf -- the MT=455 delayed \
             spectra of U-234/235/238 -- into ACE LAW=4, measured as {LAW4: 6} \
             in every DNED block of the reference library. So no held table \
             carries law 5 and a reader for it could not be verified against \
             anything",
        )),
        67 => Err(NjoyError::NotPorted(
            "ACE DLW LAW=67 (laboratory angle-energy). Upstream refuses it too \
             (`angle_energy.py:113` raises on any law outside {2,3,33,4,5,7,9,11,\
             44,61,66}). Measured 2026-09-25: NJOY2016's ACER converts the only \
             held evaluation with the ENDF form this comes from -- Be-9's MF=6 \
             LAW=7 on MT=16 -- into ACE LAW=61, so no table generated from \
             reference-data/endf carries law 67",
        )),
        other => Err(NjoyError::EndfParse(format!(
            "ACE DLW LAW={other} is not a law upstream reads either; \
             `AngleEnergy.from_ace` accepts only 2, 3, 33, 4, 5, 7, 9, 11, 44, \
             61 and 66"
        ))),
    }
}

/// The shared LAW=4/44/61 reader. `ldis` is the 0-based start of the DLW block,
/// which the per-energy locators are relative to.
fn read_tabulated_law(
    t: &RawAceTable,
    mut at: usize,
    ldis: usize,
    n_arrays: usize,
    law: i32,
) -> Result<(Vec<LawEnergyRow>, Vec<(u32, u32)>), NjoyError> {
    need(t, at, 1, "law interpolation header")?;
    let n_regions = t.xss[at] as usize;
    need(t, at + 1, 2 * n_regions, "law interpolation ranges")?;
    // The evaluation's own (NBT, INT) over the incident grid. An empty list
    // means "one lin-lin range over everything", which is ACE's default when
    // NR = 0 (`energy_distribution.py:230-231`); it is recorded as such rather
    // than fabricated, so a consumer can tell a stated law from an assumed one.
    let mut incident_interp: Vec<(u32, u32)> = Vec::with_capacity(n_regions);
    for r in 0..n_regions {
        let nbt = t.xss[at + 1 + r] as u32;
        let int = t.xss[at + 1 + n_regions + r] as u32;
        incident_interp.push((nbt, int));
    }
    need(t, at + 1 + 2 * n_regions, 1, "law incident-energy count")?;
    let n_in = t.xss[at + 1 + 2 * n_regions] as usize;
    at += 1 + 2 * n_regions + 1;
    need(t, at, 2 * n_in, "law incident grid")?;
    let e_in = t.xss[at..at + n_in].to_vec();
    let loc_dist = t.xss[at + n_in..at + 2 * n_in].to_vec();

    let mut rows = Vec::with_capacity(n_in);
    for k in 0..n_in {
        let d = ldis + (loc_dist[k] as usize) - 1;
        need(t, d, 2, "law per-energy header")?;
        let inttp = t.xss[d] as i64;
        let intt = inttp % 10;
        let n_discrete = ((inttp - intt) / 10) as usize;
        let n_out = t.xss[d + 1] as usize;
        need(t, d + 2, n_arrays * n_out, "law per-energy body")?;
        let col = |j: usize| t.xss[d + 2 + j * n_out..d + 2 + (j + 1) * n_out].to_vec();
        let e_out: Vec<f64> = col(0).iter().map(|e| e * EV_PER_MEV).collect();
        // The pdf is per MeV in the file and per eV here, so it scales by the
        // inverse of the energy scaling -- the cdf, being dimensionless, does
        // not scale at all. Scaling all three alike is a classic way to get a
        // spectrum that integrates to 1e6.
        let pdf: Vec<f64> = col(1).iter().map(|p| p / EV_PER_MEV).collect();
        let cdf = col(2);
        if n_discrete > 0 {
            return Err(NjoyError::NotPorted(
                "ACE tabulated law with discrete lines (INTTp >= 10)",
            ));
        }
        let kalbach = (law == 44).then(|| {
            let r = col(3);
            let a = col(4);
            r.iter()
                .zip(a.iter())
                .map(|(&r, &a)| ContinuumKalbachRow { r, a })
                .collect()
        });
        let cosines = if law == 61 {
            let lc = col(3);
            let mut v = Vec::with_capacity(n_out);
            for &l in &lc {
                let l = l as i64;
                if l > 0 {
                    v.push(read_cosine_table(t, ldis + (l as usize) - 1)?);
                } else {
                    // Isotropic at this outgoing energy.
                    v.push(ContinuumAngularRow {
                        cosines: vec![-1.0, 1.0],
                        cdf: vec![0.0, 1.0],
                        mubar: 0.0,
                    });
                }
            }
            Some(v)
        } else {
            None
        };
        rows.push(LawEnergyRow {
            e_in: e_in[k] * EV_PER_MEV,
            eout: ChiEout {
                e_out,
                pdf,
                cdf,
                linlin: intt == 2,
            },
            kalbach,
            cosines,
        });
    }
    Ok((rows, incident_interp))
}

/// Turn a decoded tabulated law into the workspace's [`ChiTabular`] plus its
/// angular representation.
pub fn to_chi_and_angular(
    rows: &[LawEnergyRow],
    law: i32,
    incident_interp: &[(u32, u32)],
) -> (ChiTabular, ContinuumAngular) {
    let chi = ChiTabular {
        incident: rows.iter().map(|r| r.e_in).collect(),
        tables: rows.iter().map(|r| r.eout.clone()).collect(),
        incident_interp: incident_interp.to_vec(),
    };
    let ang = match law {
        44 => ContinuumAngular::KalbachMann(
            rows.iter()
                .map(|r| ContinuumKalbachTable {
                    rows: r.kalbach.clone().unwrap_or_default(),
                })
                .collect(),
        ),
        61 => ContinuumAngular::LabTabulated(
            rows.iter()
                .map(|r| ContinuumAngularTable {
                    rows: r.cosines.clone().unwrap_or_default(),
                })
                .collect(),
        ),
        // LAW=4 defers its cosine to the AND block, which for these reactions
        // is isotropic in the evaluation itself -- not "unported", which is a
        // different claim and one this must not make silently.
        _ => ContinuumAngular::EvaluatedIsotropic,
    };
    (chi, ang)
}

/// Decode the **NU** block: prompt or total nu-bar as a function of energy.
///
/// Two forms: `LNU = 1` is a polynomial in `E` \[MeV\], `LNU = 2` a TAB1. A
/// negative first word means the block holds *both* prompt and total, with the
/// total following the prompt.
/// Read an ACE **TAB1** record at `at`: `[NR, (NBT,INT)*NR, NE, x*NE, y*NE]`.
///
/// Returns `(x [eV], y, next_index)`. `x` is scaled from the file's MeV; `y` is
/// returned as stored, because what it means depends on the record — a yield, a
/// probability, a cross section — and only the caller knows.
///
/// Factored out of [`decode_nu`], which had it inline, so the delayed-neutron
/// decoder shares one implementation rather than adding a second. The
/// interpolation-region skip — `2 * NR` words that are read past, not used — is
/// exactly the arithmetic that drifts between two copies, and `decode_nu`'s own
/// comment records what it cost to get wrong once.
pub fn read_ace_tab1(
    t: &RawAceTable,
    at: usize,
    what: &str,
) -> Result<(Vec<f64>, Vec<f64>, usize), NjoyError> {
    let (tab, next) = read_tab1_full(t, at, 1.0, what)?;
    let (x, y) = tab.pairs.iter().copied().unzip();
    Ok((x, y, next))
}

/// [`read_tab1`] keeping the record's **interpolation regions**, as the
/// workspace's [`Tab1`].
///
/// `y_scale` multiplies every ordinate: `EV_PER_MEV` for a parameter that is an
/// energy (Maxwell/evaporation `theta`, Watt `a`), its reciprocal for one that
/// is an inverse energy (Watt `b`), and `1.0` for a dimensionless one (an
/// applicability probability, a yield). The abscissae are always incident
/// energies and always scale from MeV, so that is not a parameter.
///
/// # Why the regions matter here and not in [`read_tab1`]
///
/// [`read_tab1`] serves the NU block, whose consumer ([`NuBar::at`]) interpolates
/// lin-lin regardless. The analytic laws' parameters go into a [`Tab1`] the
/// transport crate evaluates with the full ENDF multi-region rule
/// (`eval_tab1`), so dropping `(NBT, INT)` here would silently turn a histogram
/// or log region into a linear one. ACE stores the regions in the same
/// one-based-breakpoint convention as ENDF, so they are carried across as they
/// stand.
pub(crate) fn read_tab1_full(
    t: &RawAceTable,
    at: usize,
    y_scale: f64,
    what: &str,
) -> Result<(Tab1, usize), NjoyError> {
    need(t, at, 1, &format!("{what} TAB1 NR"))?;
    let n_regions = t.xss[at] as usize;
    need(t, at + 1, 2 * n_regions, &format!("{what} TAB1 regions"))?;
    let mut interp = Vec::with_capacity(n_regions);
    for r in 0..n_regions {
        interp.push((
            t.xss[at + 1 + r] as u32,
            t.xss[at + 1 + n_regions + r] as u32,
        ));
    }
    let j = at + 1 + 2 * n_regions;
    need(t, j, 1, &format!("{what} TAB1 count"))?;
    let n = t.xss[j] as usize;
    if n == 0 {
        return Err(NjoyError::EndfParse(format!(
            "{what}: a TAB1 record with zero points cannot be interpolated"
        )));
    }
    need(t, j + 1, 2 * n, &format!("{what} TAB1 body"))?;
    let pairs: Vec<(f64, f64)> = (0..n)
        .map(|k| (t.xss[j + 1 + k] * EV_PER_MEV, t.xss[j + 1 + n + k] * y_scale))
        .collect();
    let tab = Tab1 {
        head: Cont {
            c1: 0.0,
            c2: 0.0,
            l1: 0,
            l2: 0,
            n1: n_regions as i32,
            n2: n as i32,
        },
        interp,
        pairs,
    };
    Ok((tab, j + 1 + 2 * n))
}

/// Turn an ACE **LAW=66** (`n`-body phase space) entry into the tabulated
/// [`ChiTabular`] the transport samplers already consume, over the incident
/// range `[e_lo, e_hi]` \[eV\].
///
/// ACE stores only `NPSX`/`APSX`; the shape in `x = E'/E'_max` comes from
/// [`crate::acer::energy::law66_shape_table`] (the port of `acefc.f90`'s
/// `acelf6` grid, so the reader reconstructs the same table the writer would
/// have emitted), and the scaling from
/// [`crate::nuclear_data::secondary::phase_space_chi`] -- the same function the
/// ENDF MF=6 LAW=6 path uses, so the two routes cannot disagree about
/// `E'_max(E)`.
///
/// `q` is the reaction Q-value \[eV\] from LQR and `awr` the target mass ratio
/// from the table header. `None` when `NPSX` is outside the 3..=5 upstream
/// supports, or when the range yields fewer than two usable incident points.
pub fn ace_phase_space_chi(
    npsx: i32,
    apsx: f64,
    awr: f64,
    q: f64,
    e_lo: f64,
    e_hi: f64,
) -> Option<ChiTabular> {
    if !(3..=5).contains(&npsx) {
        return None;
    }
    let (x_frac, pdf, cdf) = crate::acer::energy::law66_shape_table(npsx);
    crate::nuclear_data::secondary::phase_space_chi(
        &x_frac, &pdf, &cdf, apsx, awr, q, e_lo, e_hi,
    )
}

pub fn decode_nu(t: &RawAceTable) -> Result<Option<NuBar>, NjoyError> {
    let nu = t.jxs[jxs::NU];
    if nu <= 0 {
        return Ok(None); // not fissionable
    }
    let mut at = (nu - 1) as usize;
    need(t, at, 1, "NU header")?;
    if t.xss[at] < 0.0 {
        // Both prompt and total present: the total block starts after the
        // prompt one, whose length is |first word|. Prefer TOTAL nu-bar --
        // using prompt as though it were total loses the delayed neutrons and
        // biases k low by roughly beta.
        let len = (-t.xss[at]) as usize;
        at += 1 + len;
    }
    need(t, at, 2, "NU LNU")?;
    let lnu = t.xss[at] as i32;
    match lnu {
        1 => {
            let nc = t.xss[at + 1] as usize;
            need(t, at + 2, nc, "NU polynomial")?;
            let c = t.xss[at + 2..at + 2 + nc].to_vec();
            // Tabulate the polynomial so the consumer sees one shape. The grid
            // is the one the transport crate uses for nu elsewhere.
            let mut energy = Vec::new();
            let mut nu_total = Vec::new();
            let mut e = 1.0e-5_f64;
            while e <= 2.0e7 {
                let e_mev = e / EV_PER_MEV;
                let v = c.iter().rev().fold(0.0, |acc, &ci| acc * e_mev + ci);
                energy.push(e);
                nu_total.push(v);
                e *= 1.2;
            }
            Ok(Some(NuBar { energy, nu_total }))
        }
        2 => {
            // The TAB1 begins immediately after LNU, so its NR is at `at + 1`
            // and its pair count at `at + 2 + 2*NR`
            // (`openmc/data/function.py:441-457`).
            //
            // An earlier draft read NR from `at + 2` and the pair count one
            // word further still. It did not fail -- it returned U-235's
            // nu-bar as a flat **53.0**, which is the pair count read as a
            // value. A fission yield of 53 neutrons is obviously wrong to a
            // human and completely invisible to a type system, which is why
            // the test below gates on the physical value and not merely on
            // decoding without error.
            // The arithmetic the comment above is about now lives in one
            // place, `read_tab1`, which starts at the record's NR word.
            let (energy, nu_total, _) = read_ace_tab1(t, at + 1, "NU")?;
            Ok(Some(NuBar { energy, nu_total }))
        }
        other => Err(NjoyError::EndfParse(format!("ACE NU block LNU={other}"))),
    }
}

/// How many reactions in MTR carry secondary neutrons, i.e. how many entries
/// LDLW and LAND (after elastic) have.
pub fn n_neutron_reactions(t: &RawAceTable) -> usize {
    t.nxs[nxs::NR].max(0) as usize
}
