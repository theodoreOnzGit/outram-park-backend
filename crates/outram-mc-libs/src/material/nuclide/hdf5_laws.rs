// SPDX-License-Identifier: GPL-3.0

//! **The secondary-neutron laws of an OpenMC `.h5`, mapped onto the SAME
//! representation `Nuclide::from_ace` builds** (GitHub #303).
//!
//! An OpenMC library made by `IncidentNeutron.from_ace(...).export_to_hdf5(...)`
//! holds, per product, the very objects `AngleEnergy.from_ace` made from the
//! ACE DLW block. So the inverse is mechanical, and this module does it by
//! rebuilding the decoded-ACE value [`AceEnergyLaw`] (and the rows
//! [`LawEnergyRow`] inside it) from the `.h5`, after which the code below is a
//! port of the reaction loop of `Nuclide::from_ace`
//! (`nuclide.rs`, "Angular: LAND index 0 is elastic ...") with the ACE table
//! replaced by the read library. **Nothing about sampling is re-derived:** the
//! kernels that run are the ACE route's.
//!
//! | `.h5` `type` | rebuilt as | OpenMC writer |
//! |---|---|---|
//! | `uncorrelated` + `level` | `AceEnergyLaw::TwoBodyLevel` (LAW 3) | `energy_distribution.py:937` |
//! | `uncorrelated` + `watt` / `maxwell` / `evaporation` | `Analytic` (LAW 11 / 7 / 9) | `:548`, `:269`, `:402` |
//! | `uncorrelated` + `continuous` | `Tabulated { law: 4 }` | `:1066` |
//! | `kalbach-mann` | `Tabulated { law: 44 }` | `kalbach_mann.py:360` |
//! | `correlated` | `Tabulated { law: 61 }` | `correlated.py:107` |
//! | `nbody` | `PhaseSpace` (LAW 66) | `nbody.py:85` |
//! | several distributions + `applicability` | `Mixture` (an LNW chain) | `product.py:118` |
//!
//! # What does not survive the trip bit for bit, and why
//!
//! - The incident-energy `(NBT, INT)` of a law: OpenMC always writes one
//!   region (`[n], [2]` when ACE had `NR = 0`), so a single lin-lin region is
//!   mapped back to the empty list ACE's `NR = 0` is spelt with.
//! - **Discrete lines' `p`.** OpenMC keeps a discrete line's probability
//!   unscaled and divides only the continuous part by 1e6, where
//!   `read_tabulated_law` divides every `p`. The discrete lines are divided
//!   here, so the rows equal the ACE route's; the sampler reads the `cdf` for
//!   them.
//! - Delayed-neutron yields: OpenMC stores `nu_d(E) * p_k(E)` per group and
//!   renormalises, so `nu_d` and `p_k` are rebuilt as a sum and a ratio
//!   (exact at the stored nodes, see [`delayed_data`]).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::hdf5_read::{elastic_angular, tab1};
use super::{
    ace_lnw_fission_mixture, is_other_neutron_mt, partial_fission_chi, take_fission_law,
    AceLawContext, ContinuumLaws, DelayedData, OtherLaw, OtherYield,
};
use njoy_outram_park_fork::acer::angular::ElasticAngular;
use njoy_outram_park_fork::acer::ce_decode::CeNeutronAce;
use njoy_outram_park_fork::acer::ce_laws::{
    ace_phase_space_chi, to_chi_and_angular, AceEnergyLaw, LawEnergyRow,
};
use njoy_outram_park_fork::endf::records::Tab1;
use njoy_outram_park_fork::hdf5::nuclide_laws::{
    CosineRow, EnergyDist, Function1D, Tabulated1D, TabulatedEnergyOut,
};
use njoy_outram_park_fork::hdf5::nuclide_read::{
    ReadDistribution, ReadNuclide, ReadProduct, ReadReaction,
};
use njoy_outram_park_fork::nuclear_data::secondary::{
    ChiEout, ChiTabular, ContinuumAngular, ContinuumAngularRow, ContinuumBranch,
    ContinuumEmission, ContinuumKalbachRow, FissionSpectrum, UncorrelatedEmission,
};
use njoy_outram_park_fork::NjoyError;

const EV_PER_MEV: f64 = 1.0e6;

fn refuse(msg: String) -> NjoyError {
    NjoyError::Hdf5(msg)
}

/// What the reaction loop produces, for `Nuclide::from_hdf5` to assemble.
pub(super) struct SecondaryLaws {
    /// MT=2 angular table.
    pub elastic: ElasticAngular,
    /// MT=51..90 angular tables that are not all isotropic.
    pub inelastic_angular: Vec<(u32, ElasticAngular)>,
    /// The fission spectrum (a mixture when fission is carried by partials).
    pub chi: FissionSpectrum,
    /// MT=91 / 16 / 17 / 5 emission laws.
    pub continuum: ContinuumLaws,
    /// `(mt, q, yield, law)` for the other neutron-emitting reactions.
    pub other_raw: Vec<(i32, f64, OtherYield, OtherLaw)>,
    /// Whether any fission reaction was seen.
    pub fissile: bool,
}

/// The incident-energy regions of a law, with a single lin-lin region mapped
/// back to ACE's `NR = 0` (empty).
fn incident_interp(t: &TabulatedEnergyOut) -> Vec<(u32, u32)> {
    let n = t.energy.len() as i64;
    let regions: Vec<(i64, i64)> = t
        .breakpoints
        .iter()
        .copied()
        .zip(t.interpolation.iter().copied())
        .collect();
    if regions.is_empty() || regions == [(n, 2)] {
        return Vec::new();
    }
    regions.into_iter().map(|(b, i)| (b as u32, i as u32)).collect()
}

/// A tabulated cosine as the row `read_cosine_table` builds (`ce_laws.rs`),
/// including its `mubar`.
fn cosine_row(c: &CosineRow) -> Result<ContinuumAngularRow, NjoyError> {
    // OpenMC spells ACE's isotropic outgoing cosine `Uniform(-1, 1)`; the ACE
    // route spells it `[-1, 1]`, `[1/2, 1/2]`, `[0, 1]`, lin-lin, `mubar = 0`.
    if c.mu == [-1.0, 1.0] && c.p == [0.5, 0.5] && c.c == [0.0, 1.0] {
        return Ok(ContinuumAngularRow {
            cosines: vec![-1.0, 1.0],
            cdf: vec![0.0, 1.0],
            pdf: vec![0.5, 0.5],
            histogram: false,
            mubar: 0.0,
        });
    }
    let histogram = match c.interpolation {
        1 => true,
        2 => false,
        other => {
            return Err(refuse(format!(
                "a correlated cosine table with interpolation code {other}: only 1 \
                 (histogram) and 2 (lin-lin) are defined (a discrete cosine is not \
                 read by the ACE route either)"
            )))
        }
    };
    let mut mubar = 0.0;
    for i in 1..c.mu.len() {
        let (m0, m1, p0, p1) = (c.mu[i - 1], c.mu[i], c.p[i - 1], c.p[i]);
        let h = m1 - m0;
        mubar += if histogram {
            p0 * h * 0.5 * (m0 + m1)
        } else {
            h * (m0 * (2.0 * p0 + p1) + m1 * (p0 + 2.0 * p1)) / 6.0
        };
    }
    Ok(ContinuumAngularRow {
        cosines: c.mu.clone(),
        cdf: c.c.clone(),
        pdf: c.p.clone(),
        histogram,
        mubar,
    })
}

/// The rows `read_tabulated_law` (`ce_laws.rs`) builds, from the read law.
fn law_rows(t: &TabulatedEnergyOut, law: i32) -> Result<Vec<LawEnergyRow>, NjoyError> {
    let mut rows = Vec::with_capacity(t.rows.len());
    for (k, r) in t.rows.iter().enumerate() {
        let nd = r.n_discrete_lines.max(0) as usize;
        // See the module docs: the ACE route divides every `p` by 1e6.
        let pdf: Vec<f64> = r
            .p
            .iter()
            .enumerate()
            .map(|(i, &p)| if i < nd { p / EV_PER_MEV } else { p })
            .collect();
        let kalbach = (law == 44).then(|| {
            r.kalbach_r
                .iter()
                .zip(r.kalbach_a.iter())
                .map(|(&r, &a)| ContinuumKalbachRow { r, a })
                .collect()
        });
        let cosines = if law == 61 {
            let mut v = Vec::with_capacity(r.cosines.len());
            for c in &r.cosines {
                v.push(cosine_row(c)?);
            }
            Some(v)
        } else {
            None
        };
        rows.push(LawEnergyRow {
            e_in: t.energy[k],
            eout: ChiEout {
                e_out: r.e_out.clone(),
                pdf,
                cdf: r.c.clone(),
                linlin: r.interpolation == 2,
                n_discrete: nd,
            },
            kalbach,
            cosines,
        });
    }
    Ok(rows)
}

fn tabulated(t: &TabulatedEnergyOut, law: i32) -> Result<AceEnergyLaw, NjoyError> {
    Ok(AceEnergyLaw::Tabulated {
        law,
        rows: law_rows(t, law)?,
        incident_interp: incident_interp(t),
    })
}

/// One read distribution as the decoded ACE law it came from; `None` for a
/// distribution that is an angle only (elastic).
fn energy_law(d: &ReadDistribution) -> Result<Option<AceEnergyLaw>, NjoyError> {
    Ok(Some(match d {
        ReadDistribution::Uncorrelated { energy: None, .. } => return Ok(None),
        ReadDistribution::Uncorrelated { energy: Some(e), .. } => match e {
            EnergyDist::Level { threshold, mass_ratio } => AceEnergyLaw::TwoBodyLevel {
                ldat1: *threshold,
                ldat2: *mass_ratio,
            },
            EnergyDist::Watt { u, a, b } => AceEnergyLaw::Analytic {
                law: 11,
                spectrum: FissionSpectrum::WattEnergyDependent {
                    a: tab1(a),
                    b: tab1(b),
                    u: *u,
                },
            },
            EnergyDist::Maxwell { u, theta } => AceEnergyLaw::Analytic {
                law: 7,
                spectrum: FissionSpectrum::Maxwell { theta: tab1(theta), u: *u },
            },
            EnergyDist::Evaporation { u, theta } => AceEnergyLaw::Analytic {
                law: 9,
                spectrum: FissionSpectrum::Evaporation { theta: tab1(theta), u: *u },
            },
            EnergyDist::DiscretePhoton { primary_flag, energy, .. } => {
                AceEnergyLaw::DiscretePhoton { lp: *primary_flag, eg: *energy }
            }
            EnergyDist::Continuous(t) => tabulated(t, 4)?,
            EnergyDist::MadlandNix { .. } => {
                return Err(refuse(
                    "a madland-nix energy law: ACE has no such law, so the ACE route has \
                     nothing to be bit-identical to and it is not sampled here"
                        .to_string(),
                ))
            }
        },
        ReadDistribution::Correlated(t) => tabulated(t, 61)?,
        ReadDistribution::KalbachMann(t) => tabulated(t, 44)?,
        ReadDistribution::NBody { total_mass, n_particles, .. } => AceEnergyLaw::PhaseSpace {
            npsx: *n_particles,
            apsx: *total_mass,
        },
        ReadDistribution::Undecoded(t) => {
            return Err(refuse(format!(
                "a '{t}' law, which this converter does not decode; refusing rather than \
                 sampling another distribution"
            )))
        }
    }))
}

/// A product's law: one distribution, or an `LNW` chain with applicability.
fn product_law(p: &ReadProduct) -> Result<Option<AceEnergyLaw>, NjoyError> {
    match p.distributions.as_slice() {
        [] => Ok(None),
        [d] => energy_law(d),
        many => {
            if p.applicability.len() != many.len() {
                return Err(refuse(format!(
                    "a product with {} distributions but {} applicability tables",
                    many.len(),
                    p.applicability.len()
                )));
            }
            let mut parts = Vec::with_capacity(many.len());
            for (d, a) in many.iter().zip(&p.applicability) {
                let Some(law) = energy_law(d)? else {
                    return Err(refuse("a chain link without an energy law".to_string()));
                };
                parts.push((tab1(a), law));
            }
            Ok(Some(AceEnergyLaw::Mixture(parts)))
        }
    }
}

/// The angle a distribution carries (ACE's AND block for that reaction).
fn distribution_angle(
    mt: i32,
    d: &ReadDistribution,
    cm: bool,
) -> Result<Option<ElasticAngular>, NjoyError> {
    elastic_angular(mt, d, cm)
}

fn prompt_neutron(rx: &ReadReaction) -> Vec<&ReadProduct> {
    rx.products
        .iter()
        .filter(|p| p.particle == "neutron" && p.emission_mode != "delayed")
        .collect()
}

/// A function's `(NBT, INT)` regions, with a single lin-lin region mapped back
/// to the empty list ACE's `NR = 0` is spelt with (OpenMC always writes `[n],
/// [2]` for it).
fn lin_lin_regions(t: &Tabulated1D) -> Vec<(u32, u32)> {
    let regions: Vec<(i64, i64)> = t
        .breakpoints
        .iter()
        .copied()
        .zip(t.interpolation.iter().copied())
        .collect();
    if regions.is_empty() || regions == [(t.x.len() as i64, 2)] {
        return Vec::new();
    }
    regions.into_iter().map(|(b, i)| (b as u32, i as u32)).collect()
}

/// A constant integer yield (ACE `|TY| <= 100`), if the product has one.
///
/// OpenMC spells `|TY| = n` as `Polynomial((n,))` (`reaction.py`, "yield_ =
/// Polynomial((abs(ty),))"), and a constant tabulated yield is read the same.
fn constant_yield(p: &ReadProduct) -> Option<u32> {
    let y0 = match &p.yield_fn {
        Some(Function1D::Polynomial(poly)) if poly.coef.len() == 1 => poly.coef[0],
        Some(Function1D::Tabulated(t)) => {
            let y0 = *t.y.first()?;
            if !t.y.iter().all(|&y| y == y0) {
                return None;
            }
            y0
        }
        _ => return None,
    };
    (y0 >= 1.0 && y0.fract() == 0.0).then_some(y0 as u32)
}

/// The multiplicity of an "other" reaction (`OtherYield`), as `from_ace`
/// builds it from `TY`.
fn other_yield(p: &ReadProduct) -> Result<OtherYield, NjoyError> {
    match &p.yield_fn {
        None => Ok(OtherYield::Fixed(1)),
        Some(Function1D::Polynomial(_)) => match constant_yield(p) {
            Some(n) => Ok(OtherYield::Fixed(n)),
            None => Err(refuse(
                "a polynomial reaction yield that is not a constant integer (ACE writes \
                 only `Polynomial((|TY|,))`)"
                    .to_string(),
            )),
        },
        Some(Function1D::Tabulated(t)) => Ok(match constant_yield(p) {
            Some(n) => OtherYield::Fixed(n),
            None => OtherYield::Tabulated {
                pairs: t.x.iter().copied().zip(t.y.iter().copied()).collect(),
                interp: lin_lin_regions(t),
            },
        }),
    }
}

/// `ace_other_law` (`nuclide.rs`) with the AND cosine taken from the read
/// distribution instead of the ACE table. A port, line for line.
fn other_law(
    decoded: &AceEnergyLaw,
    angular: &Option<ElasticAngular>,
    link_angles: &[Option<ElasticAngular>],
    cm: bool,
    ctx: &AceLawContext,
) -> Result<OtherLaw, NjoyError> {
    let single = |spectrum: ChiTabular, angular: ContinuumAngular| {
        OtherLaw::Correlated(ContinuumEmission {
            branches: vec![ContinuumBranch {
                spectrum,
                yield_pairs: Vec::new(),
                yield_interp: Vec::new(),
                applicability: None,
                angular,
            }],
            cm_frame: cm,
        })
    };
    let lct = if cm { 2 } else { 1 };
    Ok(match decoded {
        AceEnergyLaw::Tabulated { law: code @ (44 | 61), rows, incident_interp } => {
            let (chi, ang) = to_chi_and_angular(rows, *code, incident_interp);
            single(chi, ang)
        }
        AceEnergyLaw::PhaseSpace { npsx, apsx } => {
            match ace_phase_space_chi(*npsx, *apsx, ctx.awr, ctx.q, ctx.e_lo, ctx.e_hi) {
                Some(chi) => single(chi, ContinuumAngular::EvaluatedIsotropic),
                None => OtherLaw::StandIn,
            }
        }
        AceEnergyLaw::TwoBodyLevel { .. } => {
            OtherLaw::TwoBody(angular.clone().unwrap_or_default())
        }
        AceEnergyLaw::DiscretePhoton { .. } => OtherLaw::StandIn,
        other => match other.as_fission_spectrum() {
            Some(energy) => OtherLaw::Uncorrelated(UncorrelatedEmission {
                energy,
                angular: angular.clone().unwrap_or_default(),
                lct,
                yield_n: 1,
            }),
            None => match other {
                AceEnergyLaw::Mixture(parts) => OtherLaw::Correlated(ContinuumEmission {
                    branches: lnw_mixture(parts, link_angles, ctx)?,
                    cm_frame: cm,
                }),
                _ => OtherLaw::StandIn,
            },
        },
    })
}

/// `ace_lnw_mixture` (`nuclide.rs`; read its table of which links convert) with
/// each link's own angle from the `.h5` in place of the ACE AND block.
fn lnw_mixture(
    parts: &[(Tab1, AceEnergyLaw)],
    link_angles: &[Option<ElasticAngular>],
    ctx: &AceLawContext,
) -> Result<Vec<ContinuumBranch>, NjoyError> {
    let mut branches = Vec::with_capacity(parts.len());
    for (k, (applicability, law)) in parts.iter().enumerate() {
        let (spectrum, angular) = match law {
            AceEnergyLaw::Tabulated { law: code @ (44 | 61), rows, incident_interp } => {
                to_chi_and_angular(rows, *code, incident_interp)
            }
            AceEnergyLaw::Tabulated { law: 4, rows, incident_interp } => {
                if let Some(Some(and)) = link_angles.get(k) {
                    if !and.is_all_isotropic() {
                        return Err(NjoyError::NotPorted(
                            "LNW chain with a continuous (LAW=4) link whose cosine is \
                             anisotropic, alongside a correlated law: no branch \
                             representation carries it (no NJOY-written table does this)",
                        ));
                    }
                }
                to_chi_and_angular(rows, 4, incident_interp)
            }
            AceEnergyLaw::PhaseSpace { npsx, apsx } => {
                let Some(chi) =
                    ace_phase_space_chi(*npsx, *apsx, ctx.awr, ctx.q, ctx.e_lo, ctx.e_hi)
                else {
                    return Err(NjoyError::NotPorted(
                        "LNW chain with a phase-space link that is empty over the \
                         reaction's range; dropping one link would renormalise the others",
                    ));
                };
                (chi, ContinuumAngular::EvaluatedIsotropic)
            }
            AceEnergyLaw::Analytic { .. } => {
                return Err(NjoyError::NotPorted(
                    "LNW chain mixing an analytic law (7/9/11) with a correlated or \
                     phase-space law: no exact tabulated form",
                ))
            }
            _ => {
                return Err(NjoyError::NotPorted(
                    "LNW chain containing a two-body, discrete-photon or nested chain \
                     link: not a continuum neutron law",
                ))
            }
        };
        branches.push(ContinuumBranch {
            spectrum,
            yield_pairs: Vec::new(),
            yield_interp: Vec::new(),
            applicability: Some(applicability.clone()),
            angular,
        });
    }
    Ok(branches)
}

/// Each link's angle, for a product (one per distribution).
fn link_angles(
    mt: i32,
    p: &ReadProduct,
    cm: bool,
) -> Result<Vec<Option<ElasticAngular>>, NjoyError> {
    p.distributions
        .iter()
        .map(|d| distribution_angle(mt, d, cm))
        .collect()
}

/// **The reaction loop of `Nuclide::from_ace`, over the read library.**
///
/// Ported from `nuclide.rs` (`from_ace`, "Angular: LAND index 0 is elastic ..."
/// to the partial-fission mixture): the same match arms in the same order, with
/// `decode_energy_law(table, i)` replaced by [`product_law`], the AND cosine by
/// the distribution's own angle table and `TY`'s sign by `center_of_mass`.
pub(super) fn secondary_laws(
    read: &ReadNuclide,
    ace: &CeNeutronAce,
) -> Result<SecondaryLaws, NjoyError> {
    let mut elastic = ElasticAngular::default();
    let mut inelastic_angular: Vec<(u32, ElasticAngular)> = Vec::new();
    let mut chi = FissionSpectrum::default();
    let mut mt91 = None;
    let mut mt16 = None;
    let mut mt17 = None;
    let mut mt5 = None;
    let mut uncorr91: Option<UncorrelatedEmission> = None;
    let mut uncorr16: Option<UncorrelatedEmission> = None;
    let mut uncorr17: Option<UncorrelatedEmission> = None;
    let mut partial_chi: Vec<(i32, FissionSpectrum)> = Vec::new();
    let mut other_raw: Vec<(i32, f64, OtherYield, OtherLaw)> = Vec::new();
    let mut fissile = false;

    if let Some(rx) = read.reactions.get(&2) {
        let prods = prompt_neutron(rx);
        let Some(first) = prods.first().and_then(|p| p.distributions.first()) else {
            return Err(refuse("MT=2: no neutron distribution".to_string()));
        };
        elastic = elastic_angular(2, first, true)?.unwrap_or_default();
    }

    let has = |m: i32| ace.reactions.iter().any(|r| r.mt == m);
    let e_hi = ace.energy.last().copied().unwrap_or(2.0e7);
    for (&mt, rx) in &read.reactions {
        if mt == 1 || mt == 2 {
            continue;
        }
        let prods = prompt_neutron(rx);
        let [p] = prods.as_slice() else {
            if prods.is_empty() {
                continue;
            }
            return Err(refuse(format!(
                "MT={mt}: {} prompt neutron products; one is expected",
                prods.len()
            )));
        };
        let p = *p;
        let cm = rx.center_of_mass;
        let decoded = match product_law(p) {
            Ok(Some(d)) => d,
            Ok(None) => continue,
            Err(e) => return Err(refuse(format!("MT={mt}: {e}"))),
        };
        let angular = match p.distributions.first() {
            Some(d) => distribution_angle(mt, d, cm)?,
            None => None,
        };
        let is_fission = mt == 18 || [19, 20, 21, 38].contains(&mt);
        if is_fission {
            fissile = true;
        }
        if (51..=90).contains(&mt) {
            if let Some(ang) = &angular {
                if !ang.is_all_isotropic() {
                    inelastic_angular.push((mt as u32, ang.clone()));
                }
            }
        }
        let ctx = AceLawContext {
            awr: ace.awr,
            q: rx.q_value,
            e_lo: ace.energy.get(rx.threshold_idx).copied().unwrap_or(1.0e-5).max(1.0e-5),
            e_hi,
        };
        if is_other_neutron_mt(mt, &has) {
            let links = link_angles(mt, p, cm)?;
            let law = other_law(&decoded, &angular, &links, cm, &ctx)?;
            other_raw.push((mt, rx.q_value, other_yield(p)?, law));
        }
        let mut place = |emission: ContinuumEmission| match mt {
            91 => mt91 = Some(emission),
            16 => mt16 = Some(emission),
            17 => mt17 = Some(emission),
            5 => mt5 = Some(emission),
            _ => {}
        };
        match decoded {
            AceEnergyLaw::TwoBodyLevel { .. } | AceEnergyLaw::DiscretePhoton { .. } => {}
            AceEnergyLaw::Tabulated { law, ref rows, ref incident_interp }
                if law != 4 || is_fission || ![91, 16, 17].contains(&mt) =>
            {
                if law == 4 && mt == 5 {
                    if angular.as_ref().is_some_and(|a| !a.is_all_isotropic()) {
                        return Err(NjoyError::NotPorted(
                            "MT=5 continuous law with an anisotropic cosine: MT=5 has no \
                             uncorrelated emission slot to carry it",
                        ));
                    }
                }
                let (chi_tab, ang) = to_chi_and_angular(rows, law, incident_interp);
                if is_fission {
                    take_fission_law(
                        &mut chi,
                        &mut partial_chi,
                        mt,
                        FissionSpectrum::ContinuousTabular(chi_tab),
                    );
                } else {
                    place(ContinuumEmission {
                        branches: vec![ContinuumBranch {
                            spectrum: chi_tab,
                            yield_pairs: Vec::new(),
                            yield_interp: Vec::new(),
                            applicability: None,
                            angular: ang,
                        }],
                        cm_frame: cm,
                    });
                }
            }
            AceEnergyLaw::PhaseSpace { npsx, apsx } => {
                if let Some(spectrum) =
                    ace_phase_space_chi(npsx, apsx, ace.awr, rx.q_value, ctx.e_lo, ctx.e_hi)
                {
                    place(ContinuumEmission {
                        branches: vec![ContinuumBranch {
                            spectrum,
                            yield_pairs: Vec::new(),
                            yield_interp: Vec::new(),
                            applicability: None,
                            angular: ContinuumAngular::EvaluatedIsotropic,
                        }],
                        cm_frame: cm,
                    });
                }
            }
            ref other => {
                let Some(spectrum) = other.as_fission_spectrum() else {
                    let AceEnergyLaw::Mixture(parts) = other else {
                        return Err(NjoyError::NotPorted(
                            "law with no transport representation",
                        ));
                    };
                    if is_fission {
                        take_fission_law(
                            &mut chi,
                            &mut partial_chi,
                            mt,
                            ace_lnw_fission_mixture(parts)?,
                        );
                    } else {
                        let links = link_angles(mt, p, cm)?;
                        place(ContinuumEmission {
                            branches: lnw_mixture(parts, &links, &ctx)?,
                            cm_frame: cm,
                        });
                    }
                    continue;
                };
                if is_fission {
                    take_fission_law(&mut chi, &mut partial_chi, mt, spectrum);
                } else if [91, 16, 17].contains(&mt) {
                    let lct = if cm { 2 } else { 1 };
                    let yield_n = match mt {
                        16 => 2,
                        17 => 3,
                        _ => 1,
                    };
                    let emission = UncorrelatedEmission {
                        energy: spectrum,
                        angular: angular.clone().unwrap_or_default(),
                        lct,
                        yield_n,
                    };
                    match mt {
                        91 => uncorr91 = Some(emission),
                        16 => uncorr16 = Some(emission),
                        17 => uncorr17 = Some(emission),
                        _ => {}
                    }
                }
            }
        }
    }

    // Energy-dependent MT=5 multiplicity: carried in the branches' yield pairs,
    // as `from_ace` does for |TY| > 100. A constant integer yield is `|TY| <=
    // 100` on the ACE side and carries no pairs.
    if let Some(law) = mt5.as_mut() {
        if let Some(rx) = read.reactions.get(&5) {
            if let Some(p) = prompt_neutron(rx).first() {
                if constant_yield(p).is_none() {
                    if let Some(Function1D::Tabulated(t)) = &p.yield_fn {
                        let pairs: Vec<(f64, f64)> =
                            t.x.iter().copied().zip(t.y.iter().copied()).collect();
                        let interp = lin_lin_regions(t);
                        for b in &mut law.branches {
                            b.yield_pairs = pairs.clone();
                            b.yield_interp = interp.clone();
                        }
                    }
                }
            }
        }
    }

    if !partial_chi.is_empty() {
        chi = partial_fission_chi(ace, partial_chi);
    }

    Ok(SecondaryLaws {
        elastic,
        inelastic_angular,
        chi,
        continuum: ContinuumLaws {
            uncorrelated: [(91, uncorr91), (16, uncorr16), (17, uncorr17)],
            mt91,
            mt16,
            mt17,
            mt5,
        },
        other_raw,
        fissile,
    })
}

fn lin_at(x: &[f64], y: &[f64], e: f64) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    if e <= x[0] {
        return y[0];
    }
    if e >= x[x.len() - 1] {
        return y[y.len() - 1];
    }
    let k = x.partition_point(|&v| v <= e);
    let (x0, x1, y0, y1) = (x[k - 1], x[k], y[k - 1], y[k]);
    if x1 > x0 { y0 + (y1 - y0) * (e - x0) / (x1 - x0) } else { y1 }
}

/// The delayed-neutron data the `.h5` carries, as `DelayedData::from_ace`
/// builds it from DNU / BDD / DNED.
///
/// OpenMC stores, per precursor group `k`, a product with `decay_rate`
/// (`xss * 1e8`, `reaction.py:319`), the yield `nu_d(E) * p_k(E)` renormalised
/// by the sum of the group probabilities (`:362-365`) and the group's spectrum.
/// So `nu_d(E) = sum_k y_k(E)` and `p_k(E) = y_k(E) / nu_d(E)`, **exact at the
/// stored nodes**; between nodes `p_k` is interpolated lin-lin where OpenMC
/// interpolates `y_k` (the same second-order difference as `partial_fission_chi`
/// documents). When every group shares one grid (the usual case, the group
/// probabilities being constant) the grid is the file's; otherwise the union.
///
/// The groups are taken from the lowest-MT fission reaction that has delayed
/// products (OpenMC attaches the same set to MT=18 or to each partial fission).
///
/// `Ok(None)` when no fission reaction has a delayed product.
pub(super) fn delayed_data(read: &ReadNuclide) -> Result<Option<DelayedData>, NjoyError> {
    let Some(rx) = [18, 19, 20, 21, 38].iter().find_map(|m| {
        read.reactions
            .get(m)
            .filter(|r| r.products.iter().any(|p| p.emission_mode == "delayed"))
    }) else {
        return Ok(None);
    };
    let groups: Vec<&ReadProduct> = rx
        .products
        .iter()
        .filter(|p| p.particle == "neutron" && p.emission_mode == "delayed")
        .collect();
    let mut tabs: Vec<&Tabulated1D> = Vec::with_capacity(groups.len());
    for g in &groups {
        match &g.yield_fn {
            Some(Function1D::Tabulated(t)) if !t.x.is_empty() => tabs.push(t),
            _ => {
                return Err(refuse(format!(
                    "MT={}: a delayed-neutron group without a tabulated yield",
                    rx.mt
                )))
            }
        }
    }
    let same_grid = tabs.iter().all(|t| t.x == tabs[0].x);
    let grid: Vec<f64> = if same_grid {
        tabs[0].x.clone()
    } else {
        if tabs.iter().any(|t| lin_lin_regions(t).iter().any(|&(_, i)| i != 2)) {
            return Err(refuse(
                "delayed-neutron groups on different grids with a non-lin-lin region: their \
                 sum is not one tabulated function"
                    .to_string(),
            ));
        }
        let mut g: Vec<f64> = tabs.iter().flat_map(|t| t.x.iter().copied()).collect();
        g.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        g.dedup();
        g
    };
    let y_at = |t: &Tabulated1D, j: usize| {
        if same_grid { t.y[j] } else { lin_at(&t.x, &t.y, grid[j]) }
    };
    let nu_delayed: Vec<f64> = (0..grid.len())
        .map(|j| tabs.iter().map(|t| y_at(t, j)).sum())
        .collect();
    let interp = if same_grid { lin_lin_regions(tabs[0]) } else { Vec::new() };
    let mut group_fraction = Vec::with_capacity(tabs.len());
    for t in &tabs {
        group_fraction.push(
            (0..grid.len())
                .map(|j| {
                    let nu = nu_delayed[j];
                    (grid[j], if nu > 0.0 { y_at(t, j) / nu } else { 0.0 })
                })
                .collect::<Vec<_>>(),
        );
    }
    let mut spectra = Vec::new();
    for g in &groups {
        let Some(d) = g.distributions.first() else {
            spectra.clear();
            break;
        };
        match energy_law(d)?.and_then(|l| l.as_fission_spectrum()) {
            Some(s) => spectra.push(s),
            None => {
                spectra.clear();
                break;
            }
        }
    }
    if spectra.len() != groups.len() {
        spectra = Vec::new();
    }
    Ok(Some(DelayedData {
        lambda: groups.iter().map(|g| g.decay_rate).collect(),
        energy: grid,
        nu_delayed,
        group_fraction,
        nu_delayed_interp: interp.clone(),
        group_fraction_interp: vec![interp; tabs.len()],
        lambda_is_lowest_energy_only: false,
        spectra,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lin(x: &[f64], y: &[f64]) -> Tabulated1D {
        Tabulated1D::lin_lin(x.to_vec(), y.to_vec()).unwrap()
    }

    fn group(rate: f64, y: &[f64]) -> ReadProduct {
        ReadProduct {
            particle: "neutron".into(),
            emission_mode: "delayed".into(),
            decay_rate: rate,
            yield_fn: Some(Function1D::Tabulated(lin(&[1.0, 10.0], y))),
            distributions: vec![],
            applicability: vec![],
        }
    }

    fn reaction_with(products: Vec<ReadProduct>) -> ReadReaction {
        ReadReaction {
            mt: 18,
            q_value: 0.0,
            center_of_mass: false,
            redundant: false,
            xs: vec![],
            threshold_idx: 0,
            product_laws: vec![],
            product_particles: vec![],
            products,
            other_temperature_xs: Default::default(),
        }
    }

    fn nuclide_with(rx: ReadReaction) -> ReadNuclide {
        let mut reactions = std::collections::BTreeMap::new();
        reactions.insert(rx.mt, rx);
        ReadNuclide {
            name: "X".into(),
            z: 92,
            a: 235,
            metastable: 0,
            atomic_weight_ratio: 233.0,
            kts: Default::default(),
            energy: Default::default(),
            reactions,
            total_nu: None,
            total_nu_regions: vec![],
            urr_temperatures: vec![],
            urr: Default::default(),
            primary_temperature: String::new(),
        }
    }

    /// Group yields `nu_d * p_k` rebuild to `nu_d` (their sum) and `p_k` (their
    /// ratio), exact at the nodes; no spectra are claimed when a group has none.
    #[test]
    fn delayed_yields_rebuild_total_and_shares() {
        let rx = reaction_with(vec![
            group(0.0125, &[0.003, 0.006]),
            group(0.03, &[0.009, 0.018]),
        ]);
        let d = delayed_data(&nuclide_with(rx)).unwrap().unwrap();
        assert_eq!(d.energy, vec![1.0, 10.0]);
        assert_eq!(d.nu_delayed, vec![0.012, 0.024]);
        assert_eq!(d.lambda, vec![0.0125, 0.03]);
        assert!((d.group_fraction[0][0].1 - 0.25).abs() < 1e-15);
        assert!((d.group_fraction[1][1].1 - 0.75).abs() < 1e-15);
        assert!(d.spectra.is_empty());
        assert!((d.nu_delayed_at(5.0) - (0.012 + 0.012 * 4.0 / 9.0)).abs() < 1e-15);
    }

    /// Groups on different grids are summed on the union grid.
    #[test]
    fn delayed_groups_on_different_grids_use_the_union() {
        let mut a = group(0.1, &[1.0, 1.0]);
        let mut b = group(0.2, &[2.0, 2.0]);
        b.yield_fn = Some(Function1D::Tabulated(lin(&[1.0, 5.0, 10.0], &[2.0, 2.0, 2.0])));
        a.decay_rate = 0.1;
        let d = delayed_data(&nuclide_with(reaction_with(vec![a, b]))).unwrap().unwrap();
        assert_eq!(d.energy, vec![1.0, 5.0, 10.0]);
        assert_eq!(d.nu_delayed, vec![3.0, 3.0, 3.0]);
    }

    /// No delayed product means no delayed data, not an error.
    #[test]
    fn no_delayed_product_gives_none() {
        assert!(delayed_data(&nuclide_with(reaction_with(vec![]))).unwrap().is_none());
    }

    /// A single lin-lin incident region is ACE's `NR = 0`; a real second region
    /// is kept.
    #[test]
    fn incident_regions_map_back_to_ace_spelling() {
        let mut t = TabulatedEnergyOut {
            energy: vec![1.0, 2.0, 3.0],
            breakpoints: vec![3],
            interpolation: vec![2],
            rows: vec![],
        };
        assert!(incident_interp(&t).is_empty());
        t.breakpoints = vec![2, 3];
        t.interpolation = vec![1, 2];
        assert_eq!(incident_interp(&t), vec![(2, 1), (3, 2)]);
    }

    /// OpenMC's `Uniform(-1, 1)` cosine becomes ACE's isotropic row; a real
    /// table keeps its mean cosine (a flat-then-sloped lin-lin table).
    #[test]
    fn cosine_rows_follow_the_ace_spelling() {
        let iso = cosine_row(&CosineRow {
            mu: vec![-1.0, 1.0],
            p: vec![0.5, 0.5],
            c: vec![0.0, 1.0],
            interpolation: 1,
        })
        .unwrap();
        assert!(!iso.histogram && iso.mubar == 0.0);
        let row = cosine_row(&CosineRow {
            mu: vec![-1.0, 1.0],
            p: vec![0.25, 0.75],
            c: vec![0.0, 1.0],
            interpolation: 2,
        })
        .unwrap();
        assert!((row.mubar - 1.0 / 6.0).abs() < 1e-15, "{}", row.mubar);
        assert!(cosine_row(&CosineRow { mu: vec![0.0], p: vec![1.0], c: vec![1.0], interpolation: 0 })
            .is_err());
    }

    /// The discrete lines' `p` is divided by 1e6 as the ACE reader does.
    #[test]
    fn discrete_lines_are_scaled_like_the_ace_reader() {
        use njoy_outram_park_fork::hdf5::nuclide_laws::EnergyOutRow;
        let t = TabulatedEnergyOut {
            energy: vec![1.0e6],
            breakpoints: vec![1],
            interpolation: vec![2],
            rows: vec![EnergyOutRow {
                e_out: vec![1.0, 2.0, 3.0],
                p: vec![0.4, 3.0e-7, 3.0e-7],
                c: vec![0.4, 0.7, 1.0],
                interpolation: 2,
                n_discrete_lines: 1,
                kalbach_r: vec![],
                kalbach_a: vec![],
                cosines: vec![],
            }],
        };
        let rows = law_rows(&t, 4).unwrap();
        assert_eq!(rows[0].eout.pdf, vec![0.4 / 1.0e6, 3.0e-7, 3.0e-7]);
        assert_eq!(rows[0].eout.n_discrete, 1);
    }
}
