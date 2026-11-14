// SPDX-License-Identifier: GPL-3.0

//! **A nuclide read from an OpenMC-format neutron `.h5`** — GitHub #303.
//!
//! [`Nuclide::from_hdf5`] converts what
//! [`njoy_outram_park_fork::hdf5::nuclide_read::read_nuclide`] returns into a
//! transport [`Nuclide`], beside [`Nuclide::from_ace`] and
//! [`Nuclide::from_tape`].
//!
//! # Same internals as the ACE route, not a parallel copy
//!
//! An OpenMC library built from NJOY's ACE
//! (`IncidentNeutron.from_ace(...).export_to_hdf5(...)`) carries the ACE
//! table's own arrays: MT=1 is ESZ's total, MT=2 is ESZ's elastic, every other
//! reaction is its SIG block from its threshold. So the file is first re-cast
//! as the decoded ACE table, [`CeNeutronAce`], and its cross sections are then
//! built by [`ace_cross_sections`], **the function `from_ace` itself calls**:
//! one rule for which MTs are sampled as channels, one rule for a missing
//! absorption lump, one for partial fission. Nothing about the cross sections
//! is re-derived here, so an `.h5` whose arrays are the ACE table's reproduces
//! `from_ace`'s cross sections bit for bit by construction (a `f64` survives
//! the HDF5 round trip exactly), and the test that checks it is a check on the
//! plumbing, not on a second implementation.
//!
//! # What is decoded, and what is REFUSED
//!
//! The reader unpacks the **uncorrelated** laws (`level`, `watt`, `maxwell`,
//! `evaporation`) with their angle tables, and `total_nu`. That is enough for
//! elastic scattering, the discrete inelastic levels and an evaluated-spectrum
//! fission, which is a complete transportable nuclide for those channels.
//!
//! Everything else is **refused by name** and never approximated, because a
//! secondary law that is skipped is a reaction whose neutrons are wrong while
//! every cross section is right:
//!
//! - any neutron-emitting channel outside elastic, fission and MT=51..90
//!   (continuum inelastic, `(n,2n)`, `(n,3n)`, the others), whatever its law;
//! - a fission spectrum that is `correlated`, `continuous`, `kalbach-mann`,
//!   `madland-nix` or otherwise not a Watt, Maxwell or evaporation law;
//! - unresolved-resonance probability tables (`urr` group): they change the
//!   physics by default, so a file carrying them is refused rather than
//!   transported as infinitely dilute;
//! - delayed-neutron products (the delayed-group data is not decoded);
//! - more than one temperature, a polynomial `total_nu`.
//!
//! So a **real** OpenMC U-235 library (continuous and correlated fission and
//! continuum laws) is refused today, with the first offending MT named; the
//! cross-section parity of such a file is what `ReadNuclide` itself serves
//! (`tests/band_xs_from_hdf5_reader.rs`). Decoding those laws is the open part
//! of GitHub #303.

use super::{
    build_inelastic_levels, elastic_0k_from_ace, partial_fission_chi, take_fission_law,
    InelasticLevel,
    ContinuumLaws, Nuclide, SectionIndex, SpeedTier, XsSource, DBRC_DEFAULT_E_MAX_EV,
};
use super::nubar_for;
use njoy_outram_park_fork::reconr::{ReconrResult, ReconrSection};
use njoy_outram_park_fork::MtReaction;
use njoy_outram_park_fork::acer::angular::{ElasticAngular, EnergyAngular};
use njoy_outram_park_fork::acer::ce_decode::{
    AceReaction, CeNeutronAce, ALWAYS_REDUNDANT_MTS, PARTIAL_FISSION_MTS,
};
use njoy_outram_park_fork::endf::records::{Cont, Tab1};
use njoy_outram_park_fork::hdf5::nuclide_laws::{EnergyDist, Tabulated1D};
use njoy_outram_park_fork::hdf5::nuclide_read::{
    ReadDistribution, ReadNuclide, ReadProduct, ReadReaction,
};
use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, NuBar};
use njoy_outram_park_fork::NjoyError;

fn refuse(msg: String) -> NjoyError {
    NjoyError::Hdf5(msg)
}

/// A `Tabulated1D` as the ENDF-shaped `Tab1` the fission-spectrum laws hold.
fn tab1(t: &Tabulated1D) -> Tab1 {
    let n = t.x.len();
    let interp: Vec<(u32, u32)> = if t.breakpoints.is_empty() {
        vec![(n as u32, 2)]
    } else {
        t.breakpoints
            .iter()
            .zip(&t.interpolation)
            .map(|(&b, &i)| (b as u32, i as u32))
            .collect()
    };
    Tab1 {
        head: Cont {
            c1: 0.0,
            c2: 0.0,
            l1: 0,
            l2: 0,
            n1: interp.len() as i32,
            n2: n as i32,
        },
        interp,
        pairs: t.x.iter().copied().zip(t.y.iter().copied()).collect(),
    }
}

/// The prompt neutron products of a reaction.
fn neutron_products(rx: &ReadReaction) -> Vec<&ReadProduct> {
    rx.products
        .iter()
        .filter(|p| p.particle == "neutron" && p.emission_mode != "delayed")
        .collect()
}

/// A single-law fission spectrum, or a refusal naming what was found.
fn fission_spectrum(mt: i32, rx: &ReadReaction) -> Result<FissionSpectrum, NjoyError> {
    let prompt = neutron_products(rx);
    let [p] = prompt.as_slice() else {
        return Err(refuse(format!(
            "MT={mt}: {} prompt neutron products; a fission spectrum needs exactly one",
            prompt.len()
        )));
    };
    let [d] = p.distributions.as_slice() else {
        return Err(refuse(format!(
            "MT={mt}: a fission product with {} distributions (an applicability \
             mixture) is not decoded",
            p.distributions.len()
        )));
    };
    match d {
        ReadDistribution::Uncorrelated {
            energy: Some(EnergyDist::Watt { u, a, b }),
            ..
        } => Ok(FissionSpectrum::WattEnergyDependent {
            a: tab1(a),
            b: tab1(b),
            u: *u,
        }),
        ReadDistribution::Uncorrelated {
            energy: Some(EnergyDist::Maxwell { u, theta }),
            ..
        } => Ok(FissionSpectrum::Maxwell {
            theta: tab1(theta),
            u: *u,
        }),
        ReadDistribution::Uncorrelated {
            energy: Some(EnergyDist::Evaporation { u, theta }),
            ..
        } => Ok(FissionSpectrum::Evaporation {
            theta: tab1(theta),
            u: *u,
        }),
        ReadDistribution::Uncorrelated { energy, .. } => Err(refuse(format!(
            "MT={mt}: the fission spectrum is {energy:?}, which is not a Watt, Maxwell \
             or evaporation law; refusing rather than sampling another spectrum"
        ))),
        ReadDistribution::Undecoded(t) => Err(refuse(format!(
            "MT={mt}: the fission spectrum is a '{t}' law, which this converter does \
             not decode (GitHub #303); refusing rather than sampling another spectrum"
        ))),
    }
}

/// An angle table as the ACE-shaped [`ElasticAngular`] the kernels sample.
///
/// A table that is `mu = [-1, 1]`, `p = [1/2, 1/2]` is how OpenMC spells
/// isotropic (`AngleDistribution.from_ace` writes it for ACE's `LOC = 0`), and
/// becomes the empty cosine grid that `EnergyAngular` spells it with, so the
/// isotropic fast path is taken exactly as on the ACE route.
fn elastic_angular(
    mt: i32,
    d: &ReadDistribution,
    centre_of_mass: bool,
) -> Result<Option<ElasticAngular>, NjoyError> {
    let ReadDistribution::Uncorrelated {
        angle: Some(angle),
        angle_cdf,
        ..
    } = d
    else {
        return Ok(None);
    };
    let mut energies = Vec::with_capacity(angle.energy.len());
    for (i, &e_ev) in angle.energy.iter().enumerate() {
        let (mu, p) = (&angle.mu[i], &angle.p[i]);
        let iso = mu.as_slice() == [-1.0, 1.0] && p.as_slice() == [0.5, 0.5];
        let histogram = angle.interpolation.get(i).copied() == Some(1);
        if iso {
            energies.push(EnergyAngular {
                e_mev: e_ev / 1.0e6,
                cosines: Vec::new(),
                pdf: Vec::new(),
                cdf: Vec::new(),
                histogram: false,
            });
            continue;
        }
        let Some(cdf) = angle_cdf.get(i) else {
            return Err(refuse(format!("MT={mt}: an angle table without its cumulative row")));
        };
        energies.push(EnergyAngular {
            e_mev: e_ev / 1.0e6,
            cosines: mu.clone(),
            pdf: p.clone(),
            cdf: cdf.clone(),
            histogram,
        });
    }
    Ok(Some(ElasticAngular {
        energies,
        lct: if centre_of_mass { 2 } else { 1 },
    }))
}

/// The table `CeNeutronAce` the rest of `outram-mc-libs` already reads, from
/// the `.h5`'s arrays.
fn as_decoded_ace(read: &ReadNuclide) -> Result<CeNeutronAce, NjoyError> {
    if read.kts.len() != 1 || read.energy.len() != 1 {
        return Err(refuse(format!(
            "{}: {} temperatures in the file; this converter takes a single-temperature \
             file (the reader returns the first temperature's cross sections)",
            read.name,
            read.kts.len().max(read.energy.len())
        )));
    }
    let energy = read
        .any_energy()
        .cloned()
        .ok_or_else(|| refuse(format!("{} carries no energy grid", read.name)))?;
    let n = energy.len();
    let on_grid = |mt: i32| read.reactions.get(&mt).map(|r| r.xs_on_grid(n));
    // **MT=1 is usually NOT in an OpenMC library.** `export_to_hdf5` skips a
    // redundant reaction unless its MT is in `keep_mts` (1 is not,
    // `neutron.py:423-428`), and OpenMC itself builds the total by summing the
    // non-redundant reactions (`nuclide.cpp:409`). So a file that does carry
    // MT=1 gives it exactly (the bit-identical route to ACE's ESZ total);
    // otherwise the total is OpenMC's own sum, which equals ESZ only to the
    // rounding ACER applied to the partials.
    let total = match on_grid(1) {
        Some(t) => t,
        None => {
            let mut sum = vec![0.0; n];
            for r in read.reactions.values().filter(|r| !r.redundant) {
                for (s, x) in sum.iter_mut().zip(r.xs_on_grid(n)) {
                    *s += x;
                }
            }
            sum
        }
    };
    let elastic = on_grid(2)
        .ok_or_else(|| refuse(format!("{}: no MT=2 (elastic) reaction", read.name)))?;
    let absorption = on_grid(101).unwrap_or_else(|| vec![0.0; n]);
    let reactions: Vec<AceReaction> = read
        .reactions
        .values()
        .filter(|r| r.mt != 1 && r.mt != 2 && !ALWAYS_REDUNDANT_MTS.contains(&r.mt))
        .map(|r| AceReaction {
            mt: r.mt,
            q_value: r.q_value,
            // TYR carries only the frame sign and the fission flag here; the
            // multiplicities are not read by the cross-section builder.
            ty: if PARTIAL_FISSION_MTS.contains(&r.mt) || r.mt == 18 {
                19
            } else if r.center_of_mass {
                -1
            } else {
                1
            },
            threshold_index: r.threshold_idx,
            xs: r.xs.clone(),
        })
        .collect();
    let kt_ev = read.kts.values().next().copied().unwrap_or(0.0);
    Ok(CeNeutronAce {
        zaid: read.name.clone(),
        za: 1000 * read.z + read.a,
        awr: read.atomic_weight_ratio,
        kt_ev,
        energy,
        total,
        absorption,
        elastic,
        heating: vec![0.0; n],
        reactions,
    })
}

/// ν̄ from `total_nu`.
fn nu_bar(read: &ReadNuclide) -> Result<Option<NuBar>, NjoyError> {
    let Some((kind, x, y)) = &read.total_nu else {
        return Ok(None);
    };
    if kind == "Polynomial" {
        return Err(refuse(format!(
            "{}: a polynomial total_nu is not converted (GitHub #303)",
            read.name
        )));
    }
    let lin_lin = read.total_nu_regions.is_empty()
        || read.total_nu_regions == [(x.len() as i64, 2)];
    let interp = if lin_lin {
        Vec::new()
    } else {
        read.total_nu_regions
            .iter()
            .map(|&(b, i)| (b as u32, i as u32))
            .collect()
    };
    Ok(Some(NuBar {
        energy: x.clone(),
        nu_total: y.clone(),
        poly: None,
        interp,
    }))
}

impl Nuclide {
    /// Build a nuclide from an OpenMC-format neutron `.h5` already read with
    /// [`njoy_outram_park_fork::hdf5::nuclide_read::read_nuclide`].
    ///
    /// See the module docs for what is converted, why the cross sections are
    /// bit-identical to [`Self::from_ace`]'s on the same arrays, and the list
    /// of what is refused. The returned nuclide has DBRC attached by default
    /// where the file's elastic is at 0 K, exactly as [`Self::from_ace`] does.
    ///
    /// `name` is the transport name (`"U235"`), as for [`Self::from_ace`].
    ///
    /// # Errors
    ///
    /// [`NjoyError::Hdf5`] naming the first thing that cannot be converted
    /// faithfully: a refused law, URR tables, delayed products, a second
    /// temperature, a fissile nuclide without `total_nu`.
    pub fn from_hdf5(read: &ReadNuclide, name: &str) -> Result<Self, NjoyError> {
        if !read.urr_temperatures.is_empty() {
            return Err(refuse(format!(
                "{}: the file carries unresolved-resonance probability tables at {:?}, \
                 which this converter does not decode; transporting it without them \
                 would silently drop self-shielding that the ACE route applies by default",
                read.name, read.urr_temperatures
            )));
        }
        let ace = as_decoded_ace(read)?;
        let transport_mts = ace.transport_channel_mts();

        // Delayed-neutron data is not decoded.
        for rx in read.reactions.values() {
            if rx.products.iter().any(|p| p.emission_mode == "delayed" || p.decay_rate > 0.0) {
                return Err(refuse(format!(
                    "MT={}: delayed-neutron products are not decoded (GitHub #303); \
                     refusing rather than transporting every fission neutron as prompt",
                    rx.mt
                )));
            }
        }

        // Elastic, the levels and fission are the channels this converter can
        // sample; any other neutron emitter is refused by name.
        let mut elastic = ElasticAngular::default();
        let mut inelastic_angular: Vec<(u32, ElasticAngular)> = Vec::new();
        let mut chi = FissionSpectrum::default();
        let mut partial_chi: Vec<(i32, FissionSpectrum)> = Vec::new();
        let mut fissile = false;

        if let Some(rx) = read.reactions.get(&2) {
            let prods = neutron_products(rx);
            let Some(first) = prods.first().and_then(|p| p.distributions.first()) else {
                return Err(refuse("MT=2: no neutron distribution".to_string()));
            };
            elastic = elastic_angular(2, first, true)?.unwrap_or_default();
        }
        for &mt in &transport_mts {
            let Some(rx) = read.reactions.get(&mt) else {
                continue;
            };
            let prods = neutron_products(rx);
            if mt == 18 || PARTIAL_FISSION_MTS.contains(&mt) {
                fissile = true;
                take_fission_law(&mut chi, &mut partial_chi, mt, fission_spectrum(mt, rx)?);
            } else if (51..=90).contains(&mt) {
                let Some(d) = prods.first().and_then(|p| p.distributions.first()) else {
                    return Err(refuse(format!("MT={mt}: a level without a neutron distribution")));
                };
                match d {
                    ReadDistribution::Uncorrelated {
                        energy: Some(EnergyDist::Level { .. }),
                        ..
                    } => {}
                    other => {
                        return Err(refuse(format!(
                            "MT={mt}: an inelastic level whose law is {other:?}, not `level`"
                        )))
                    }
                }
                if let Some(ang) = elastic_angular(mt, d, rx.center_of_mass)? {
                    if !ang.is_all_isotropic() {
                        inelastic_angular.push((mt as u32, ang));
                    }
                }
            } else if !prods.is_empty() {
                let laws: Vec<String> = prods
                    .iter()
                    .flat_map(|p| p.distributions.iter())
                    .map(|d| match d {
                        ReadDistribution::Undecoded(t) => t.clone(),
                        ReadDistribution::Uncorrelated { energy, .. } => {
                            format!("uncorrelated {energy:?}")
                        }
                    })
                    .collect();
                return Err(refuse(format!(
                    "MT={mt}: a neutron-emitting channel with law {laws:?}. This converter \
                     samples elastic, MT=51..90 levels and fission only (GitHub #303); \
                     refusing rather than treating the reaction as elastic or capture"
                )));
            }
        }
        if !partial_chi.is_empty() {
            chi = partial_fission_chi(&ace, partial_chi);
        }

        let nu = match nu_bar(read)? {
            Some(nu) => nu,
            None if fissile => {
                return Err(refuse(format!(
                    "{}: a fission reaction but no total_nu; the nuclide would fission \
                     and emit nothing",
                    read.name
                )))
            }
            None => nubar_for(name, false),
        };

        let (recon, inel) = ace_cross_sections(&ace);
        Ok(Self {
            name: name.to_string(),
            awr: ace.awr,
            nu,
            chi,
            xs: XsSource::Pointwise {
                index: SectionIndex::new(&recon),
                recon,
                inel,
                elastic_angular: elastic,
                inelastic_angular,
            },
            thermal: None,
            continuum: ContinuumLaws {
                uncorrelated: [(91, None), (16, None), (17, None)],
                ..ContinuumLaws::default()
            },
            target_at_rest: false,
            legacy_scatter_kernels: false,
            data_kt_ev: Some(ace.kt_ev),
            nu_frozen_at: None,
            chi_frozen_at: None,
            n2n_yield_one: false,
            delayed_spectra_ablated: false,
            other_channels: Vec::new(),
            other_channels_ablated: false,
            urr: None,
            dbrc: None,
            elastic_0k: elastic_0k_from_ace(&ace),
            prompt_only: false,
            delayed: None,
            speed: SpeedTier::default(),
            multipole: None,
        }
        .with_dbrc(DBRC_DEFAULT_E_MAX_EV))
    }
}

/// **The pointwise cross-section half of `Nuclide::from_ace`, as a function of the
/// decoded table** (GitHub #303): ACE's grid becomes a [`ReconrResult`] and the
/// inelastic level list is built from it.
///
/// Moved out of `from_ace` unchanged so that `Nuclide::from_hdf5` builds its
/// sections by the **same code**, which is what makes an `.h5` read through it
/// bit-identical to the ACE route on the cross sections: there is one rule for
/// which MTs are sampled as channels, how a missing absorption lump is
/// synthesised and how partial fissions collapse into MT=18, and both routes use
/// it.
fn ace_cross_sections(
    ace: &njoy_outram_park_fork::acer::ce_decode::CeNeutronAce,
) -> (ReconrResult, Vec<InelasticLevel>) {
    use njoy_outram_park_fork::acer::ce_decode::PARTIAL_FISSION_MTS;
    // ── Cross sections: ACE's grid becomes a ReconrResult ──────────────
    //
    // `ReconrSection.pairs` is a lin-lin (E, sigma) grid per MT, exactly
    // what ESZ + SIG supply. Elastic comes from ESZ, not MTR -- ACE does
    // not list MT=2 in MTR at all, and a reader looking for it there finds
    // nothing and builds a nuclide that cannot scatter.
    let mut sections: Vec<ReconrSection> = Vec::with_capacity(ace.reactions.len() + 2);
    // **MT=1 is required, and ACE does not list it in MTR either.**
    //
    // `xs_at_energy` reads the total as `eval_mt(Mt1Total)`. RECONR emits an
    // MT=1 section; ACE carries the total in ESZ instead. Without this the
    // nuclide reports `total = 0` at every energy while every partial is
    // correct -- which is worse than an obvious failure, because a
    // macroscopic total of zero makes the distance-to-collision sampler
    // return infinity and every particle streams straight out.
    sections.push(ReconrSection {
        lr: 0,
        mt: MtReaction::from_any(1),
        qi: 0.0,
        pairs: ace.energy.iter().copied().zip(ace.total.iter().copied()).collect(),
    });
    sections.push(ReconrSection {
        lr: 0,
        mt: MtReaction::from_any(2),
        qi: 0.0,
        pairs: ace.energy.iter().copied().zip(ace.elastic.iter().copied()).collect(),
    });
    // ~~`channel_mts` drops the lumps' own levels, so MT=4 and MT=51..91 are
    // never both present -- see `ce_decode::channel_mts` for why that
    // matters (U-235 sums 30 % high without it).~~
    // **CORRECTED 2026-09-29 (GitHub #366).** `channel_mts` is the rule for
    // reconstructing the TOTAL, and it keeps the lump: on U-235 it kept
    // MT=4 (`LQR = 0`, no law of its own) and dropped the 40 levels and the
    // MT=91 continuum, so every inelastic collision was sampled with Q = 0
    // -- +1853 pcm on Godiva against OpenMC on the same tables. Transport
    // takes `transport_channel_mts`, OpenMC's redundancy rule: the levels
    // when present (MT=4 then dropped, so the two are still never both
    // here), the partial fissions when present (collapsed to one MT=18
    // below). The total itself is ESZ's, above, and never summed from these.
    let transport_mts = ace.transport_channel_mts();
    let partial_fission: Vec<i32> = transport_mts
        .iter()
        .copied()
        .filter(|mt| PARTIAL_FISSION_MTS.contains(mt))
        .collect();
    for mt in transport_mts {
        // Partial fissions are collapsed into one MT=18 section below: the
        // pointwise tier has one fission channel, and ACE gives every
        // partial the same total nu-bar, so only their spectra differ --
        // and those become the chi mixture after the reaction loop.
        if PARTIAL_FISSION_MTS.contains(&mt) {
            continue;
        }
        let Some(rx) = ace.reactions.iter().find(|r| r.mt == mt) else {
            continue;
        };
        let pairs: Vec<(f64, f64)> = ace.energy[rx.threshold_index..]
            .iter()
            .copied()
            .zip(rx.xs.iter().copied())
            .collect();
        sections.push(ReconrSection {
            lr: 0,
            mt: MtReaction::from_any(mt),
            qi: rx.q_value,
            pairs,
        });
    }
    // **Absorption levels without their lump** -- GitHub #365 audit. The
    // pointwise tier counts (n,p)/(n,d)/(n,t)/(n,He-3)/(n,alpha) through
    // the lumps MT=103..107 only. A table carrying the levels 600..849
    // but not the lump (legal ACE; NJOY happens to write the lumps) used to
    // lose that absorption into the elastic remainder. OpenMC builds the
    // lump from its components in that case (`neutron.py:617-630`), so the
    // lump is synthesised here as the sum of the levels present.
    for (lump, lo, hi) in [
        (103, 600, 649),
        (104, 650, 699),
        (105, 700, 749),
        (106, 750, 799),
        (107, 800, 849),
    ] {
        if ace.reactions.iter().any(|r| r.mt == lump) {
            continue;
        }
        let levels: Vec<&njoy_outram_park_fork::acer::ce_decode::AceReaction> = ace
            .reactions
            .iter()
            .filter(|r| (lo..=hi).contains(&r.mt))
            .collect();
        let Some(first) = levels.iter().map(|r| r.threshold_index).min() else {
            continue;
        };
        let n = ace.energy.len();
        let mut sum = vec![0.0; n];
        for r in &levels {
            for (k, x) in r.xs.iter().enumerate() {
                sum[r.threshold_index + k] += x;
            }
        }
        sections.push(ReconrSection {
            lr: 0,
            mt: MtReaction::from_any(lump),
            qi: levels[0].q_value,
            pairs: ace.energy[first..n]
                .iter()
                .copied()
                .zip(sum[first..n].iter().copied())
                .collect(),
        });
    }

    // **Fission from the partials (GitHub #366).** U-234's ACE table has
    // MT=19/20/21/38 and no MT=18; the pointwise tier reads fission from
    // MT=18 only, so the nuclide could not fission at all on the ACE route
    // (1.09 b at 1 MeV on the ENDF route, 0 here). OpenMC marks MT=18
    // redundant whenever a partial is present and samples the partials;
    // one MT=18 section equal to their sum carries the same cross section,
    // and `ce_decode::fission_xs` already computes exactly that sum.
    if !partial_fission.is_empty() {
        if let Some(sum) = ace.fission_xs() {
            let first = partial_fission
                .iter()
                .filter_map(|&mt| ace.reactions.iter().find(|r| r.mt == mt))
                .map(|r| r.threshold_index)
                .min()
                .unwrap_or(0);
            let q = ace
                .reactions
                .iter()
                .find(|r| r.mt == partial_fission[0])
                .map(|r| r.q_value)
                .unwrap_or(0.0);
            sections.push(ReconrSection {
                lr: 0,
                mt: MtReaction::from_any(18),
                qi: q,
                pairs: ace.energy[first..]
                    .iter()
                    .copied()
                    .zip(sum[first..].iter().copied())
                    .collect(),
            });
        }
    }
    sections.sort_by_key(|sec| sec.mt.number());

    let recon = ReconrResult {
        material: njoy_outram_park_fork::reconr::mf1::MaterialInfo {
            za: ace.za as f64,
            awr: ace.awr,
            lrp: 0,
            lfi: i32::from(ace.is_fissionable()),
            nlib: 0,
            elis: 0.0,
            nfor: 6,
            emax: *ace.energy.last().unwrap_or(&2.0e7),
        },
        sections,
        resonance_upper_limit: None,
        // ACE carries the unresolved range as probability TABLES (the UNR
        // block), not as the MF=2 parameter list this field holds, so
        // there is nothing to put here. ~~`urr` below is `None` for the same
        // reason and is documented as an omission, not a zero.~~
        // **CORRECTED 2026-09-29**: `urr` is decoded from the UNR block
        // (GitHub #307) and attached below; only this MF=2-shaped field is
        // empty.
        unresolved_table: None,
        // The ACE table's own temperature (its `TEMP` word, kT in eV here).
        temperature_k: ace.kt_ev / crate::physics::scatter::K_BOLTZMANN_EV_PER_K,
    };
    let inel = build_inelastic_levels(&recon);
    (recon, inel)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn lin(x: &[f64], y: &[f64]) -> Tabulated1D {
        Tabulated1D::lin_lin(x.to_vec(), y.to_vec()).unwrap()
    }

    /// `tab1` keeps the points and states a single lin-lin region when the file
    /// gave none.
    #[test]
    fn a_tabulated_function_becomes_an_endf_tab1() {
        let t = lin(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]);
        let r = tab1(&t);
        assert_eq!(r.pairs, vec![(1.0, 4.0), (2.0, 5.0), (3.0, 6.0)]);
        assert_eq!(r.interp, vec![(3, 2)]);
    }
}
