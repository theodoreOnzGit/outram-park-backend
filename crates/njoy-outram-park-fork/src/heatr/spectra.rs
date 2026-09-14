//! H5 emitted-neutron spectra and the per-MT heating-model dispatch.
//!
//! Split out of `heatr/mod.rs` (see the module doc there for the physics of
//! each phase H1–H5).

use crate::acer::energy::Mf6Neutron;
use crate::endf::MtReaction;
use crate::nuclear_data::secondary::FissionSpectrum;

/// The secondary-neutron energy spectrum of an H5 multi-neutron / continuum
/// reaction, in whichever ENDF file the evaluation stores it. HEATR only needs
/// its mean outgoing energy `⟨E'⟩(E)` (the energy each escaping neutron carries
/// away), so this enum exposes exactly that via [`Self::mean_energy`].
///
/// Enum dispatch (not a trait object) per the workspace design rules — the set
/// of ENDF secondary-energy representations is closed.
#[derive(Debug, Clone)]
pub enum EmissionSpectrum {
    /// ENDF **MF=5** secondary-energy law (Maxwell / evaporation / Watt /
    /// tabulated χ) — the [`FissionSpectrum`] type, reused as a general emission
    /// spectrum (it is one). Its `mean_energy` is closed-form for the analytic
    /// laws, quadrature for the tabulated ones.
    Mf5(FissionSpectrum),
    /// ENDF **MF=6** LAW=1 tabulated neutron emission ([`Mf6Neutron`]) — the
    /// modern representation for (n,2n)/(n,3n)/continuum. Mean is the first
    /// moment of the emission pdf (exact in the lab frame; a documented
    /// approximation in the CM frame — see [`Mf6Neutron::mean_energy`]).
    Mf6(Mf6Neutron),
}

impl EmissionSpectrum {
    /// Mean outgoing-neutron energy `⟨E'⟩` \[eV\] at incident energy `e_in`
    /// \[eV\].
    pub fn mean_energy(&self, e_in: f64) -> f64 {
        match self {
            EmissionSpectrum::Mf5(chi) => chi.mean_energy(e_in),
            EmissionSpectrum::Mf6(m) => m.mean_energy(e_in),
        }
    }
}

/// The H5 reaction MTs (multi-neutron-exit + continuum inelastic) that need an
/// emitted-neutron [`EmissionSpectrum`] for their KERMA contribution.
pub(super) const H5_MTS: [i32; 10] = [11, 16, 17, 24, 25, 30, 37, 41, 42, 91];

/// Collect the H5 emission spectra a full KERMA needs from an ENDF `tape` (for
/// material `mat`): for each H5 reaction with secondary data, prefer its ENDF
/// **MF=6** LAW=1 neutron emission (the modern representation), falling back to
/// its **MF=5** law. Reactions with no readable emission spectrum (absent, or in
/// an unported MF=6 LAW / MF=5 LF) are simply omitted — HEATR then leaves their
/// heating at 0 rather than guessing.
///
/// The result is ready to pass as the `emission` argument of
/// [`Kerma::from_reconr`].
pub fn build_emission_spectra(
    tape: &crate::endf::tape::Tape,
    mat: i32,
) -> Vec<(MtReaction, EmissionSpectrum)> {
    let mut out = Vec::new();
    for &mt in &H5_MTS {
        // Prefer MF=6 LAW=1 (modern (n,2n)/(n,3n)/continuum).
        if let Some(sec) = tape.section(mat, 6, mt) {
            if let Ok(m) = crate::acer::energy::parse_mf6_law1_neutron(sec) {
                out.push((MtReaction::from_any(mt), EmissionSpectrum::Mf6(m)));
                continue;
            }
        }
        // Fall back to an MF=5 secondary law.
        if let Ok(Some(chi)) = FissionSpectrum::from_endf_mf5_mt(tape, mat, mt) {
            out.push((MtReaction::from_any(mt), EmissionSpectrum::Mf5(chi)));
        }
    }
    out
}

/// Which heating model a reaction MT uses in the kinematic-limit KERMA — the
/// closed dispatch [`heating_model`] returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HeatingModel {
    /// Elastic (MT=2): `H(E) = σ(E)·E·2A/(A+1)²` — see the module docs (H1).
    Elastic,
    /// **H2.** No escaping neutron — pure capture (MT=102) or capture plus
    /// charged particle(s) that stay local in matter (MT=103–117):
    /// `H(E) = σ(E)·(E+Q)`. All of the incident energy plus the reaction
    /// Q-value is deposited, since nothing escapes to carry energy away —
    /// this is NJOY's own documented behaviour for materials/reactions
    /// without photon-production data taken to its logical conclusion.
    Local,
    /// **H3.** Exactly one escaping neutron, with the reaction's own Q-value
    /// (discrete inelastic levels MT=51–90 and the lumped MT=4; the
    /// `(n,n'X)`-family reactions MT=22, 23, 28, 29, 32–36, 44, 45, whose
    /// charged-particle byproduct(s) stay local like H2's):
    /// `H(E) = σ(E)·[E·2A/(A+1)² + Q/(A+1)]` — the two-body-with-Q
    /// generalization of H1's elastic formula (reduces to it at `Q=0`; see
    /// the derivation in [`single_neutron_factor`]).
    SingleNeutron,
    /// **H4.** Fission (MT=18 and the chance-fission partials MT=19–21, 38):
    /// `H(E) = σ_f(E)·[E + Q_fission − ν̄(E)·⟨E'⟩]`, where `ν̄(E)` is the total
    /// (prompt+delayed) neutron yield and `⟨E'⟩` the mean fission-neutron
    /// energy from χ(E→E') at incident energy `E` — the multi-neutron
    /// generalization of H2/H3's single-particle energy balance: `ν̄` escaping
    /// neutrons each carry away the birth spectrum's mean energy, and
    /// everything else (fragment KE, prompt/delayed γ and β) deposits locally.
    Fission,
    /// **H5.** Multi-neutron-exit (MT=11, 16, 17, 24, 25, 30, 37, 41, 42) and
    /// continuum inelastic (MT=91): `ȳ` neutrons escape, each carrying the mean
    /// energy `⟨E'⟩` of the reaction's MF=5/MF=6 secondary-neutron spectrum, so
    /// `H(E) = σ(E)·[E + Q − ȳ·⟨E'⟩]`. Unlike H1–H4 the mean has no closed
    /// kinematic form, so the emission spectrum must be supplied (via the
    /// `emission` argument of [`Kerma::from_reconr`]); the multiplicity `ȳ`
    /// comes from the MT ([`neutron_multiplicity`]). If no spectrum is supplied
    /// for the reaction, it contributes 0 (falls through to [`Self::NotModeled`]
    /// behaviour rather than guessing an energy loss).
    MultiNeutron,
    /// Not modeled by the kinematic-limit KERMA — everything left to H6/H7 (the
    /// full photon energy-balance method and MT=444 damage energy), plus any
    /// H5 reaction whose emission spectrum was not supplied: contributes 0.
    NotModeled,
}

/// Dispatch an [`MtReaction`] to its [`HeatingModel`].
pub(super) fn heating_model(mt: MtReaction) -> HeatingModel {
    use MtReaction::*;
    match mt {
        Mt2Elastic => HeatingModel::Elastic,
        Mt102Capture | Mt103Np | Mt104Nd | Mt105Nt | Mt106NHe3 | Mt107NAlpha | Mt108N2Alpha
        | Mt109N3Alpha | Mt111N2Proton | Mt112NProtonAlpha | Mt113NT2Alpha | Mt114ND2Alpha
        | Mt115NProtonD | Mt116NProtonT | Mt117NDAlpha => HeatingModel::Local,
        Mt4Inelastic | Mt22NnAlpha | Mt23Nn3Alpha | Mt28NnProton | Mt29Nn2Alpha | Mt32NnD
        | Mt33NnT | Mt34NnHe3 | Mt35NnD2Alpha | Mt36NnT2Alpha | Mt44Nn2Proton
        | Mt45NnProtonAlpha => HeatingModel::SingleNeutron,
        // MT=51–90: discrete inelastic levels. Matched by number, not name —
        // each level has its own named variant (`Mt51NnLevel1`, …, up to
        // `Mt90…`), too many to enumerate individually here.
        _ if (51..=90).contains(&mt.number()) => HeatingModel::SingleNeutron,
        Mt18Fission
        | Mt19FirstChanceFission
        | Mt20SecondChanceFission
        | Mt21ThirdChanceFission
        | Mt38FourthChanceFission => HeatingModel::Fission,
        Mt11N2nD | Mt16N2n | Mt17N3n | Mt24N2nAlpha | Mt25N3nAlpha | Mt30N2n2Alpha | Mt37N4n
        | Mt41N2nProton | Mt42N3nProton | Mt91NnContinuum => HeatingModel::MultiNeutron,
        _ => HeatingModel::NotModeled,
    }
}

/// Number of neutrons `ȳ` emitted by an [`HeatingModel::MultiNeutron`] reaction —
/// the multiplicity in NJOY's `H = σ·(E + Q − ȳ·⟨E'⟩)` (`heatr.f90::nheat`,
/// where it is `yld`). Fixed by the MT's exit channel: (n,2n)-type → 2,
/// (n,3n)-type → 3, (n,4n) → 4, continuum inelastic (n,n') → 1. Returns 0 for
/// any MT that is not an H5 reaction (never reached — [`heating_model`] gates it).
pub(super) fn neutron_multiplicity(mt: MtReaction) -> f64 {
    use MtReaction::*;
    match mt {
        Mt91NnContinuum => 1.0,
        Mt11N2nD | Mt16N2n | Mt24N2nAlpha | Mt30N2n2Alpha | Mt41N2nProton => 2.0,
        Mt17N3n | Mt25N3nAlpha | Mt42N3nProton => 3.0,
        Mt37N4n => 4.0,
        _ => 0.0,
    }
}

/// The emitted-neutron energy spectrum supplied for reaction `mt`, if any.
/// H5 has no closed-form kinematic mean, so [`Kerma::from_reconr`] needs the
/// actual secondary spectrum to take its first moment `⟨E'⟩`; a reaction absent
/// from `emission` contributes 0 (see [`HeatingModel::MultiNeutron`]).
pub(super) fn emission_spectrum(
    emission: &[(MtReaction, EmissionSpectrum)],
    mt: MtReaction,
) -> Option<&EmissionSpectrum> {
    emission.iter().find(|(m, _)| *m == mt).map(|(_, s)| s)
}

/// The `2A/(A+1)²` factor from the two-body elastic recoil formula (H1),
/// shared by [`HeatingModel::Elastic`] (`Q=0`) and [`HeatingModel::SingleNeutron`]
/// (`Q≠0`, see the derivation below).
///
/// **Derivation (2-body reaction with Q-value, isotropic CM scattering).**
/// For neutron mass 1, target mass `A` at rest, incident lab energy `E`: the
/// CM-frame kinetic energy available before the reaction is `A/(A+1)·E`; after
/// the reaction it is `A/(A+1)·E + Q`. Splitting that between the outgoing
/// neutron (mass 1) and residual (mass `A`) inversely by mass, then
/// transforming back to the lab frame and averaging the neutron's lab energy
/// over an isotropic CM cosine (the cross term vanishes on average) gives
///
/// ```text
///   ⟨E'⟩ = E·(1+A²)/(A+1)² + A·Q/(A+1)
/// ```
///
/// so the energy deposited locally, `H = E + Q − ⟨E'⟩`, simplifies to
///
/// ```text
///   H(E) = E·2A/(A+1)² + Q/(A+1).
/// ```
///
/// At `Q=0` this is exactly H1's elastic formula — confirming the two are the
/// same physics with (H1) and without (H3) an excitation/reaction Q-value.
pub(super) fn single_neutron_factor(awr: f64) -> f64 {
    2.0 * awr / (awr + 1.0).powi(2)
}
