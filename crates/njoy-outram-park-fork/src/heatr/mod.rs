//! `HEATR` — heating (KERMA) cross section, ENDF MT=301.
//!
//! Computes the **kinematic-limit KERMA**: the heating cross section under the
//! assumption that every escaping *neutron* carries its kinetic energy away
//! from the local region, and everything else (nuclear recoil, charged
//! particles, and — per NJOY's own documented fallback — photon energy when no
//! photon-production data is processed) deposits locally. `heatr.f90` computes
//! this exact quantity as a **check** (`kchk`) against its full photon
//! energy-balance method; here it is the primary (and, for now, only) result.
//!
//! Ported in phases (`docs/porting-plan.md` §HEATR sub-phases) — see the
//! module's own progress:
//!
//! - **H1** (done): elastic (MT=2).
//! - **H2** (done): local-deposition reactions — capture (MT=102) and
//!   charged-particle-only exits (MT=103–117), `H(E) = σ(E)·(E+Q)`.
//! - **H3** (done): single-escaping-neutron reactions — discrete inelastic
//!   levels (MT=51–90, MT=4) and the `(n,n'X)` family (MT=22, 23, 28, 29,
//!   32–36, 44, 45), `H(E) = σ(E)·[E·2A/(A+1)² + Q/(A+1)]` (see
//!   [`single_neutron_factor`] for the derivation).
//! - **H4** (done): fission (MT=18, 19–21, 38),
//!   `H(E) = σ_f(E)·[E + Q_fission − ν̄(E)·⟨E'⟩]`, reusing [`NuBar`] and
//!   [`FissionSpectrum::mean_energy`].
//! - **H5** (done): multi-neutron-exit + continuum inelastic (MT=11, 16, 17,
//!   24, 25, 30, 37, 41, 42, 91), `H(E) = σ(E)·[E + Q − ȳ·⟨E'⟩]`, where `ȳ` is
//!   the emitted-neutron multiplicity (fixed by the MT) and `⟨E'⟩` the mean
//!   emitted-neutron energy from the reaction's own secondary spectrum — from
//!   ENDF **MF=5 or MF=6** ([`EmissionSpectrum`]; the modern (n,2n)/(n,3n) data
//!   is MF=6). This is the direct generalization of H3/H4's balance: `ȳ`
//!   neutrons each escape carrying the emission spectrum's mean energy, and
//!   `E + Q` minus that total deposits locally. A reaction whose emission
//!   spectrum is not supplied contributes 0 (excluded, not guessed).
//! - **H7** (in progress): damage-energy production, ENDF **MT=444** — the
//!   Lindhard-Robinson partition of recoil kinetic energy between atomic
//!   displacements and electronic excitation, [`DamageEnergy`]. The two-body
//!   neutron-scattering recoil channels are ported: **elastic** (MT=2) and
//!   **discrete inelastic levels** (MT=51–90), both via the uniform-recoil
//!   isotropic-CM integral. MF=4 anisotropy and the continuum/(n,xn)/capture
//!   channels share the same [`lindhard_damage`] partition and follow. Distinct
//!   output quantity from the MT=301 heating KERMA above.
//! - **H6** (deferred): the full photon energy-balance method.
//!
//! ## Elastic kinematics (H1)
//!
//! For isotropic scattering in the centre-of-mass frame off a target of mass
//! ratio `A` (`= AWR`, target/neutron mass) initially at rest, the *average*
//! post-collision lab-frame neutron energy is the standard two-body result
//!
//! ```text
//!   ⟨E'⟩ = E · (1 + A²) / (A + 1)²
//! ```
//!
//! so the average energy transferred to the recoiling nucleus — and thus
//! deposited locally — is
//!
//! ```text
//!   H(E) = E − ⟨E'⟩ = E · 2A / (A + 1)².
//! ```
//!
//! For `A = 1` (hydrogen) this gives `H = E/2`: an elastic collision with a
//! free proton loses on average exactly half its energy — a textbook result,
//! and the module's primary correctness check.

use crate::acer::energy::Mf6Neutron;
use crate::endf::MtReaction;
use crate::nuclear_data::secondary::{FissionSpectrum, NuBar};
use crate::reconr::{eval_lin_lin, ReconrResult};

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
const H5_MTS: [i32; 10] = [11, 16, 17, 24, 25, 30, 37, 41, 42, 91];

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
enum HeatingModel {
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
fn heating_model(mt: MtReaction) -> HeatingModel {
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
fn neutron_multiplicity(mt: MtReaction) -> f64 {
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
fn emission_spectrum(
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
fn single_neutron_factor(awr: f64) -> f64 {
    2.0 * awr / (awr + 1.0).powi(2)
}

/// Heating (KERMA) cross section, ENDF **MT=301**, vs incident energy \[eV\].
///
/// Built once from a [`ReconrResult`] via [`Kerma::from_reconr`]; evaluated
/// with [`Kerma::eval`]. Units are \[eV·barn\] (energy × cross section — the
/// standard ENDF heating-cross-section convention), summed over every
/// reaction [`heating_model`] currently covers.
#[derive(Debug, Clone, Default)]
pub struct Kerma {
    /// Union incident-energy grid \[eV\], ascending, deduplicated.
    pub energy: Vec<f64>,
    /// Σ over covered reactions of `σ_mt(E)·H_mt(E)` \[eV·barn\], aligned with
    /// `energy`.
    pub h: Vec<f64>,
}

impl Kerma {
    /// Compute the kinematic-limit KERMA from a reconstructed evaluation.
    ///
    /// **H1–H5** are covered (elastic; capture + charged-particle-only exits;
    /// single-escaping-neutron reactions; fission; multi-neutron-exit + continuum
    /// inelastic); every other reaction contributes 0 (see the module docs).
    ///
    /// - `awr` is the target's mass ratio (`ReconrResult::material::awr`).
    /// - `nu`/`chi` are the fission neutron yield and birth spectrum (from
    ///   [`crate::nuclear_data::secondary`]) used only by the `Fission` model —
    ///   pass [`NuBar::default`]/[`FissionSpectrum::default`] for a
    ///   non-fissionable material (no MT=18 section ⇒ they are never evaluated).
    /// - `emission` maps each H5 multi-neutron / continuum-inelastic reaction
    ///   (MT=11, 16, 17, 24, 25, 30, 37, 41, 42, 91) to its emitted-neutron
    ///   spectrum ([`EmissionSpectrum`], from ENDF MF=5 *or* MF=6); the mean of
    ///   that spectrum is subtracted `ȳ` times (once per escaping neutron). A
    ///   reaction present in `recon` but absent from `emission` contributes 0 —
    ///   pass `&[]` to skip all of H5.
    pub fn from_reconr(
        recon: &ReconrResult,
        nu: &NuBar,
        chi: &FissionSpectrum,
        emission: &[(MtReaction, EmissionSpectrum)],
    ) -> Self {
        let awr = recon.material.awr;
        // A section contributes to the grid only if its heating is actually
        // modeled — for H5 that additionally requires a supplied emission spectrum.
        let contributes = |mt: MtReaction| match heating_model(mt) {
            HeatingModel::NotModeled => false,
            HeatingModel::MultiNeutron => emission_spectrum(emission, mt).is_some(),
            _ => true,
        };
        let mut energy: Vec<f64> = recon
            .sections
            .iter()
            .filter(|s| contributes(s.mt))
            .flat_map(|s| s.pairs.iter().map(|&(e, _)| e))
            .collect();
        energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12 * b.abs().max(1.0));

        let two_body_factor = single_neutron_factor(awr);
        let mut h = vec![0.0; energy.len()];
        for sec in &recon.sections {
            let model = heating_model(sec.mt);
            if model == HeatingModel::NotModeled {
                continue;
            }
            // Resolve the H5 emission spectrum once per section; skip the whole
            // section (contributes 0) if it is a MultiNeutron reaction with none.
            let multi_spec = if model == HeatingModel::MultiNeutron {
                match emission_spectrum(emission, sec.mt) {
                    Some(spec) => Some((spec, neutron_multiplicity(sec.mt))),
                    None => continue,
                }
            } else {
                None
            };
            for (i, &e) in energy.iter().enumerate() {
                let sigma = eval_lin_lin(&sec.pairs, e);
                if sigma == 0.0 {
                    continue;
                }
                let per_event = match model {
                    HeatingModel::Elastic => e * two_body_factor,
                    HeatingModel::Local => e + sec.qi,
                    HeatingModel::SingleNeutron => e * two_body_factor + sec.qi / (awr + 1.0),
                    HeatingModel::Fission => e + sec.qi - nu.at(e) * chi.mean_energy(e),
                    HeatingModel::MultiNeutron => {
                        let (spec, yld) = multi_spec.unwrap();
                        e + sec.qi - yld * spec.mean_energy(e)
                    }
                    HeatingModel::NotModeled => unreachable!(),
                };
                h[i] += sigma * per_event;
            }
        }
        Kerma { energy, h }
    }

    /// Apply HEATR's **energy-balance correction**: subtract the escaping-photon
    /// energy production ([`PhotonProduction`], ENDF MT=442) from this
    /// kinematic-limit KERMA, giving the energy-balance MT=301 heating.
    ///
    /// The kinematic limit deposits *all* photon energy locally; wherever the
    /// evaluation carries photon-production data those photons actually escape,
    /// so their energy `Σ E_γ,prod(E)` must be removed (`heatr.f90`'s `gheat`).
    /// The subtraction happens on this KERMA's own energy grid (dense — the union
    /// of the modeled reactions' grids, which already includes the
    /// photon-producing reactions). The result is clamped at 0: heating is a
    /// physical (non-negative) energy deposition, and until the **capture
    /// momentum-recoil** refinement (`disgam`) lands, a capture reaction with
    /// MF=12/13 photon data could otherwise over-subtract (its photons carry
    /// nearly all of `E+Q`, leaving only the small recoil the clamp preserves as
    /// 0 rather than a spurious negative).
    ///
    /// A no-op when `photon` is empty (no photon files ⇒ the kinematic limit is
    /// already the energy-balance answer, per NJOY's documented fallback).
    pub fn with_energy_balance(
        mut self,
        photon: &crate::photon::PhotonProduction,
        recon: &ReconrResult,
    ) -> Self {
        if photon.is_empty() {
            return self;
        }
        for (i, &e) in self.energy.iter().enumerate() {
            self.h[i] = (self.h[i] - photon.eval(e, recon)).max(0.0);
        }
        self
    }

    /// Evaluate the heating cross section \[eV·barn\] at incident energy `e`
    /// \[eV\] (lin-lin interpolated, clamped at the tabulated ends).
    pub fn eval(&self, e: f64) -> f64 {
        if self.energy.is_empty() {
            return 0.0;
        }
        let pairs: Vec<(f64, f64)> = self
            .energy
            .iter()
            .copied()
            .zip(self.h.iter().copied())
            .collect();
        eval_lin_lin(&pairs, e)
    }
}

// ── H7: damage-energy production (MT=444) ───────────────────────────────────

/// Default atomic displacement threshold energy `E_d` \[eV\] for element `z`
/// (proton number), NJOY `heatr.f90`'s built-in table (used when the user does
/// not override it). Recoils below `E_d` produce no lattice damage. Values are
/// the standard ASTM/NJOY defaults: 25 eV for most elements, higher for the
/// refractory / heavy metals. `z=0` (unknown) falls back to 25 eV.
///
/// | Z | element(s) | `E_d` \[eV\] |
/// |---|---|---|
/// | 4, 6 | Be, C | 31 |
/// | 12, 14 | Mg, Si | 25 |
/// | 13 | Al | 27 |
/// | 20, 22–29, 40, 41 | Ca, Ti–Cu, Zr, Nb | 40 |
/// | 42, 47 | Mo, Ag | 60 |
/// | 73, 74 | Ta, W | 90 |
/// | 79 | Au | 30 |
/// | 82 | Pb | 25 |
/// | (all others) | — | 25 |
pub fn default_displacement_energy(z: u32) -> f64 {
    match z {
        4 | 6 => 31.0,
        12 | 14 => 25.0,
        13 => 27.0,
        20 | 22..=29 | 40 | 41 => 40.0,
        42 | 47 => 60.0,
        73 | 74 => 90.0,
        79 => 30.0,
        82 => 25.0,
        _ => 25.0,
    }
}

/// Lindhard-Robinson damage-partition function `df(E_R)` \[eV\]: of a recoiling
/// atom's kinetic energy `e_recoil` \[eV\], the portion that goes into further
/// atomic displacements (damage energy) rather than electronic excitation
/// (which does not displace atoms). Faithful port of `heatr.f90`'s `df`.
///
/// The recoiling atom has proton/mass numbers `(z_r, a_r)` and travels through a
/// lattice of atoms `(z_l, a_l)` (`a_*` are mass ratios, `= AWR`; for a
/// self-recoil — e.g. elastic scattering — recoil and lattice are the same
/// atom). `e_d` is the displacement threshold \[eV\]: recoils below it produce
/// no damage, so `df = 0`. The Robinson analytic fit to Lindhard's partition is
///
/// ```text
///   ε   = E_R / E_L                          (reduced recoil energy)
///   df  = E_R / [ 1 + f_L·(3.4008·ε^{1/6} + 0.40244·ε^{3/4} + ε) ]
/// ```
///
/// with `E_L` and `f_L` the Lindhard screening constants built from the Z's and
/// A's. As `E_R → E_d⁺` the bracket → 1 so `df → E_R` (nearly all recoil energy
/// displaces atoms); as `E_R ≫ E_L` electronic stopping dominates and `df ≪ E_R`.
fn lindhard_damage(e_recoil: f64, z_r: f64, a_r: f64, z_l: f64, a_l: f64, e_d: f64) -> f64 {
    const C1: f64 = 30.724;
    const C2: f64 = 0.0793;
    const C3: f64 = 3.4008;
    const C4: f64 = 0.40244;
    if z_r == 0.0 || e_recoil < e_d {
        return 0.0;
    }
    let zr23 = z_r.powf(2.0 / 3.0);
    let zl23 = z_l.powf(2.0 / 3.0);
    let e_l = C1 * z_r * z_l * (zr23 + zl23).sqrt() * (a_r + a_l) / a_l;
    let denom = (zr23 + zl23).powf(0.75) * a_r.powf(1.5) * a_l.sqrt();
    let f_l = C2 * zr23 * z_l.sqrt() * (a_r + a_l).powf(1.5) / denom;
    let ep = e_recoil / e_l;
    e_recoil / (1.0 + f_l * (C3 * ep.powf(1.0 / 6.0) + C4 * ep.powf(0.75) + ep))
}

/// Damage-energy production cross section, ENDF **MT=444**, vs incident energy
/// \[eV\], in \[eV·barn\] (damage energy × cross section — same convention as the
/// MT=301 [`Kerma`]).
///
/// Built from a [`ReconrResult`] via [`DamageEnergy::from_reconr`]; evaluated
/// with [`DamageEnergy::eval`]. Covers the **two-body neutron-scattering**
/// recoil channels — **elastic** (MT=2) and **discrete inelastic levels**
/// (MT=51–90), the residual nucleus recoiling against the single emitted
/// neutron. Per incident energy `E`, summed over these reactions,
///
/// ```text
///   σ_444(E) = Σ σ_r(E) · ⟨df(E_R)⟩_r,
/// ```
///
/// each reaction's cross section times its recoil-averaged Lindhard damage
/// energy. For **isotropic** centre-of-mass scattering the recoil energy is
/// *uniform* on `[E_min, E_max]` (it is linear in the CM cosine), with
///
/// ```text
///   E_min = C·(1−g)²,   E_max = C·(1+g)²,   C = A/(A+1)²·E,
///   g = √(1 − E_thr/E),   E_thr = (A+1)/A·|Q|
/// ```
///
/// so `⟨df⟩ = (1/(E_max−E_min))·∫ df(E_R) dE_R` (with `df = 0` below `E_d`).
/// Elastic is the `Q = 0` case: `g = 1`, `E_min = 0`, `E_max = 4A/(A+1)²·E`
/// (backscatter maximum). MF=4 angular anisotropy (which reweights the recoil
/// distribution — `heatr.f90`'s 64-point Gauss-Legendre `disbar`) and the
/// continuum / (n,xn) / capture-recoil channels are the remaining H7 sub-steps.
#[derive(Debug, Clone, Default)]
pub struct DamageEnergy {
    /// Union incident-energy grid \[eV\], ascending.
    pub energy: Vec<f64>,
    /// `Σ σ_r(E)·⟨df⟩_r` \[eV·barn\], aligned with `energy`.
    pub d: Vec<f64>,
}

impl DamageEnergy {
    /// Compute the damage-energy cross section (MT=444) from a reconstructed
    /// evaluation, summed over the elastic (MT=2) and discrete-inelastic
    /// (MT=51–90) two-body recoil channels.
    ///
    /// The target's proton number `z` and displacement threshold `e_d` \[eV\]
    /// (pass [`default_displacement_energy`]`(z)` for the built-in default) set
    /// the Lindhard partition; mass ratio `A` comes from `recon.material.awr`.
    /// Returns an empty table if the material has none of these channels.
    pub fn from_reconr(recon: &ReconrResult, z: u32, e_d: f64) -> Self {
        let awr = recon.material.awr;
        let zf = z as f64;
        let contributes =
            |mt: MtReaction| mt == MtReaction::Mt2Elastic || (51..=90).contains(&mt.number());
        let mut energy: Vec<f64> = recon
            .sections
            .iter()
            .filter(|s| contributes(s.mt))
            .flat_map(|s| s.pairs.iter().map(|&(e, _)| e))
            .collect();
        energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12 * b.abs().max(1.0));

        let mut d = vec![0.0; energy.len()];
        for sec in &recon.sections {
            if !contributes(sec.mt) {
                continue;
            }
            // Elastic has Q=0 by definition; a discrete level uses its own QI.
            let q = if sec.mt == MtReaction::Mt2Elastic {
                0.0
            } else {
                sec.qi
            };
            for (i, &e) in energy.iter().enumerate() {
                let sigma = eval_lin_lin(&sec.pairs, e);
                if sigma == 0.0 {
                    continue;
                }
                let (e_min, e_max) = two_body_recoil_bounds(e, q, awr);
                d[i] += sigma * mean_recoil_damage(e_min, e_max, zf, awr, e_d);
            }
        }
        DamageEnergy { energy, d }
    }

    /// Evaluate the damage-energy cross section \[eV·barn\] at incident energy
    /// `e` \[eV\] (lin-lin interpolated, clamped at the tabulated ends).
    pub fn eval(&self, e: f64) -> f64 {
        if self.energy.is_empty() {
            return 0.0;
        }
        let pairs: Vec<(f64, f64)> = self
            .energy
            .iter()
            .copied()
            .zip(self.d.iter().copied())
            .collect();
        eval_lin_lin(&pairs, e)
    }
}

/// Recoil-energy bounds `[E_min, E_max]` \[eV\] of an isotropic-CM two-body
/// neutron-scattering reaction (elastic or a discrete inelastic level) at
/// incident energy `e` \[eV\] with reaction Q-value `q` \[eV\] (`0` for elastic,
/// negative for a level). The recoil energy is linear in the CM cosine, hence
/// *uniform* on this interval:
///
/// ```text
///   C = A/(A+1)²·E,   g = √(max(0, 1 − E_thr/E)),   E_thr = (A+1)/A·(−Q),
///   E_min = C·(1−g)²,   E_max = C·(1+g)².
/// ```
///
/// For `q = 0` this is `g = 1`, `[0, 4C]` — the elastic backscatter range. Ports
/// the recoil kinematics of `heatr.f90::disbar` (its `afact`, `arat`, `r`, `g`
/// with `awp = 1` for a neutron exit).
fn two_body_recoil_bounds(e: f64, q: f64, awr: f64) -> (f64, f64) {
    let c = awr / (awr + 1.0).powi(2) * e;
    let e_thr = (awr + 1.0) / awr * (-q); // 0 for elastic; >0 for a level
    let g = (1.0 - e_thr / e).max(0.0).sqrt();
    (c * (1.0 - g).powi(2), c * (1.0 + g).powi(2))
}

/// Recoil-averaged Lindhard damage energy `⟨df⟩` \[eV\] for a recoil energy
/// *uniform* on `[e_min, e_max]`: the mean of [`lindhard_damage`] over that
/// range. Since `df = 0` below `e_d`, the average is
/// `(1/(e_max−e_min))·∫_{max(e_min,e_d)}^{e_max} df dE_R`, computed by composite
/// Simpson's rule over the smooth region (integrating from `max(e_min, e_d)`
/// avoids the `df` discontinuity at `e_d`). Zero if the range is empty or lies
/// entirely below `e_d`.
fn mean_recoil_damage(e_min: f64, e_max: f64, z: f64, awr: f64, e_d: f64) -> f64 {
    let lo = e_min.max(e_d);
    if e_max <= lo {
        return 0.0;
    }
    // Composite Simpson over [lo, e_max]; df is smooth there, so a fixed even
    // panel count is accurate and cheap.
    const N: usize = 400; // even
    let h = (e_max - lo) / N as f64;
    let mut acc =
        lindhard_damage(lo, z, awr, z, awr, e_d) + lindhard_damage(e_max, z, awr, z, awr, e_d);
    for i in 1..N {
        let er = lo + i as f64 * h;
        let w = if i % 2 == 1 { 4.0 } else { 2.0 };
        acc += w * lindhard_damage(er, z, awr, z, awr, e_d);
    }
    let integral = acc * h / 3.0;
    integral / (e_max - e_min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconr::{MaterialInfo, ReconrSection};

    fn material(awr: f64) -> MaterialInfo {
        MaterialInfo {
            za: 1001.0,
            awr,
            lrp: 0,
            lfi: 0,
            nlib: 0,
            elis: 0.0,
            nfor: 6,
            emax: 2.0e7,
        }
    }

    fn recon(awr: f64, sections: Vec<ReconrSection>) -> ReconrResult {
        ReconrResult {
            material: material(awr),
            sections,
        }
    }

    /// Hydrogen (A=1) elastic scattering loses on average exactly half the
    /// neutron's energy per collision — the textbook check for this formula.
    #[test]
    fn hydrogen_elastic_loses_half_its_energy() {
        let sec = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e5, 20.0), (1.0e6, 20.0), (1.0e7, 20.0)], // flat 20 b
        };
        let kerma = Kerma::from_reconr(
            &recon(1.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        for &e in &[1.0e5, 1.0e6, 1.0e7] {
            let h = kerma.eval(e);
            let expected = 20.0 * e * 0.5; // σ·E/2
            assert!(
                (h - expected).abs() / expected < 1.0e-9,
                "H({e})={h}, want {expected}"
            );
        }
    }

    /// A heavy target (A≫1) transfers only a small fraction of the incident
    /// energy per elastic collision — `H(E)/(:σE) → 2/A` as `A → ∞`.
    #[test]
    fn heavy_target_transfers_small_fraction() {
        let sec = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e6, 5.0), (2.0e6, 5.0)],
        };
        let awr = 238.0;
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e6);
        let sigma_e = 5.0 * 1.0e6;
        let frac = h / sigma_e;
        let approx_2_over_a = 2.0 / awr;
        assert!(
            frac > 0.0 && frac < 0.01,
            "heavy-target fraction {frac} should be small"
        );
        assert!(
            (frac - approx_2_over_a).abs() / approx_2_over_a < 0.02,
            "frac={frac} ≈ 2/A={approx_2_over_a}"
        );
    }

    /// **H5, no-spectrum guard.** A multi-neutron reaction (MT=16 (n,2n)) with no
    /// emission spectrum supplied contributes nothing — H5 has no closed-form
    /// mean, so without the secondary spectrum the dispatch excludes the section
    /// entirely (empty grid) rather than guessing an energy loss. Passing the
    /// spectrum (see [`n2n_heating_subtracts_two_neutron_means`]) turns it on.
    #[test]
    fn multineutron_without_spectrum_contributes_nothing() {
        let sec = ReconrSection {
            mt: MtReaction::Mt16N2n,
            qi: -8.0e6,
            pairs: vec![(1.0e5, 1.0), (1.0e6, 1.0)],
        };
        let kerma = Kerma::from_reconr(
            &recon(56.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[], // no emission spectrum for MT=16
        );
        assert!(
            kerma.energy.is_empty(),
            "(n,2n) with no supplied spectrum ⇒ empty grid"
        );
        assert_eq!(kerma.eval(5.0e5), 0.0);
    }

    /// **H2.** Radiative capture (MT=102) deposits all of `E+Q` locally — no
    /// escaping neutron, so nothing is subtracted from the energy balance.
    #[test]
    fn capture_deposits_e_plus_q() {
        let q = 6.0e6; // representative (n,γ) Q-value
        let sec = ReconrSection {
            mt: MtReaction::Mt102Capture,
            qi: q,
            pairs: vec![(1.0e5, 2.0), (1.0e6, 2.0)],
        };
        let kerma = Kerma::from_reconr(
            &recon(56.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        for &e in &[1.0e5, 1.0e6] {
            let h = kerma.eval(e);
            let expected = 2.0 * (e + q);
            assert!(
                (h - expected).abs() / expected < 1.0e-9,
                "H({e})={h}, want {expected}"
            );
        }
    }

    /// **H2.** A charged-particle-only exit, e.g. MT=107 `(n,α)`, uses the same
    /// `E+Q` local-deposition formula as capture — no escaping neutron.
    #[test]
    fn charged_particle_only_exit_deposits_e_plus_q() {
        let q = 2.0e6;
        let sec = ReconrSection {
            mt: MtReaction::Mt107NAlpha,
            qi: q,
            pairs: vec![(1.0e6, 0.5)],
        };
        let kerma = Kerma::from_reconr(
            &recon(27.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e6);
        let expected = 0.5 * (1.0e6 + q);
        assert!(
            (h - expected).abs() / expected < 1.0e-9,
            "H={h}, want {expected}"
        );
    }

    /// **H3, `Q=0` reduction.** A discrete "level" with `qi=0.0` (unphysical for
    /// a real level, but a clean check) must reproduce H1's pure elastic
    /// formula exactly — confirming the two-body-with-Q formula's `Q→0` limit.
    #[test]
    fn single_neutron_at_q_zero_matches_elastic_formula() {
        let awr = 12.0;
        let level = ReconrSection {
            mt: MtReaction::from_any(52),
            qi: 0.0,
            pairs: vec![(1.0e6, 3.0)],
        };
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![level]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e6);
        let expected = 3.0 * 1.0e6 * 2.0 * awr / (awr + 1.0).powi(2);
        assert!(
            (h - expected).abs() / expected < 1.0e-9,
            "H={h}, want {expected}"
        );
    }

    /// **H3.** A discrete inelastic level with a real (negative) Q-value: the
    /// per-event heating `H(E)/σ(E) = E·2A/(A+1)² + Q/(A+1)` is *less* than the
    /// pure-elastic term because the excitation energy `|Q|` is carried away by
    /// the residual nucleus's internal state — consistent with `E+Q < E`
    /// available for redistribution.
    #[test]
    fn discrete_level_heating_is_reduced_by_negative_q() {
        let awr = 12.0;
        let q = -4.4e6; // e.g. carbon-12's first excited state
        let level = ReconrSection {
            mt: MtReaction::from_any(51),
            qi: q,
            pairs: vec![(1.0e7, 1.0)],
        };
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![level]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e7);
        let expected = 1.0 * (1.0e7 * 2.0 * awr / (awr + 1.0).powi(2) + q / (awr + 1.0));
        assert!(
            (h - expected).abs() / expected.abs() < 1.0e-9,
            "H={h}, want {expected}"
        );
        let h_elastic_only = 1.0 * 1.0e7 * 2.0 * awr / (awr + 1.0).powi(2);
        assert!(
            h < h_elastic_only,
            "negative Q must reduce heating below the Q=0 term"
        );
    }

    /// **H3.** An `(n,n'α)`-family reaction (MT=22): single escaping neutron,
    /// the alpha stays local — same formula as a discrete level, using MT=22's
    /// own Q-value.
    #[test]
    fn nn_alpha_family_uses_single_neutron_formula() {
        let awr = 27.0;
        let q = -3.0e6;
        let sec = ReconrSection {
            mt: MtReaction::Mt22NnAlpha,
            qi: q,
            pairs: vec![(8.0e6, 0.2)],
        };
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(8.0e6);
        let expected = 0.2 * (8.0e6 * 2.0 * awr / (awr + 1.0).powi(2) + q / (awr + 1.0));
        assert!(
            (h - expected).abs() / expected.abs() < 1.0e-9,
            "H={h}, want {expected}"
        );
    }

    /// H1+H2+H3 all sum additively on the shared union grid.
    #[test]
    fn all_three_phases_sum_additively() {
        let awr = 56.0;
        let elastic = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e6, 10.0)],
        };
        let capture = ReconrSection {
            mt: MtReaction::Mt102Capture,
            qi: 6.0e6,
            pairs: vec![(1.0e6, 1.0)],
        };
        let level = ReconrSection {
            mt: MtReaction::from_any(51),
            qi: -1.0e6,
            pairs: vec![(1.0e6, 0.3)],
        };
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![elastic, capture, level]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e6);
        let f = 2.0 * awr / (awr + 1.0).powi(2);
        let h_elastic = 10.0 * 1.0e6 * f;
        let h_capture = 1.0 * (1.0e6 + 6.0e6);
        let h_level = 0.3 * (1.0e6 * f + (-1.0e6) / (awr + 1.0));
        let expected = h_elastic + h_capture + h_level;
        assert!(
            (h - expected).abs() / expected < 1.0e-9,
            "got {h}, want {expected}"
        );
    }

    /// H1 and H2 sum additively on the same union grid — a material with both
    /// elastic and capture data gets both contributions at each energy.
    #[test]
    fn elastic_and_capture_sum_additively() {
        let elastic = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e6, 10.0)],
        };
        let capture = ReconrSection {
            mt: MtReaction::Mt102Capture,
            qi: 6.0e6,
            pairs: vec![(1.0e6, 1.0)],
        };
        let awr = 56.0;
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![elastic, capture]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h = kerma.eval(1.0e6);
        let h_elastic = 10.0 * 1.0e6 * 2.0 * awr / (awr + 1.0).powi(2);
        let h_capture = 1.0 * (1.0e6 + 6.0e6);
        assert!(
            (h - (h_elastic + h_capture)).abs() < 1.0,
            "got {h}, want {}",
            h_elastic + h_capture
        );
    }

    /// Union grid + summation across two elastic-tagged sections at different
    /// energy grids (only one physical elastic section normally exists per
    /// material, but the union-grid machinery must still be exercised).
    #[test]
    fn eval_interpolates_between_grid_points() {
        let sec = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e6, 10.0), (2.0e6, 10.0)],
        };
        let kerma = Kerma::from_reconr(
            &recon(1.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[],
        );
        let h_mid = kerma.eval(1.5e6);
        // σ is flat at 10 b, so H(E) = 10*E/2 exactly (linear in E), and lin-lin
        // interpolation between the two grid points must land on that line.
        assert!((h_mid - 10.0 * 1.5e6 * 0.5).abs() < 1.0, "got {h_mid}");
    }

    /// **H4.** Fission heating matches `σ_f·[E + Q_fission − ν̄·⟨E'⟩]` exactly for
    /// a constant ν̄ and the fixed-parameter Watt χ (closed-form mean).
    #[test]
    fn fission_heating_matches_energy_balance_formula() {
        let q_fission = 200.0e6; // representative U-235 fission Q-value
        let sigma_f = 2.0; // barn, flat for simplicity
        let sec = ReconrSection {
            mt: MtReaction::Mt18Fission,
            qi: q_fission,
            pairs: vec![(1.0e5, sigma_f), (1.0e6, sigma_f)],
        };
        let nu = NuBar {
            energy: vec![1.0e5, 1.0e6],
            nu_total: vec![2.4, 2.6],
        };
        let chi = FissionSpectrum::Watt {
            a: 0.988e6,
            b: 2.249e-6,
        };
        let kerma = Kerma::from_reconr(&recon(235.0, vec![sec]), &nu, &chi, &[]);

        let mean_e_prime = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
        for &e in &[1.0e5, 1.0e6] {
            let h = kerma.eval(e);
            let nu_at_e = nu.at(e);
            let expected = sigma_f * (e + q_fission - nu_at_e * mean_e_prime);
            assert!(
                (h - expected).abs() / expected.abs() < 1.0e-9,
                "H({e})={h}, want {expected}"
            );
        }
    }

    /// **H4.** Fission heating is *positive and order-190 MeV/fission* for a
    /// realistic case (Q≫ν̄·⟨E'⟩ at typical incident energies) — a basic
    /// physical sanity bound distinct from the exact-formula check above
    /// (catches a sign error that the exact check, using the same formula,
    /// structurally cannot).
    #[test]
    fn fission_heating_is_positive_and_order_200_mev() {
        let q_fission = 200.0e6;
        let sigma_f = 1.5;
        let sec = ReconrSection {
            mt: MtReaction::Mt18Fission,
            qi: q_fission,
            pairs: vec![(1.0e5, sigma_f), (1.0e6, sigma_f)],
        };
        let nu = NuBar {
            energy: vec![1.0e5, 1.0e6],
            nu_total: vec![2.44, 2.44],
        };
        let chi = FissionSpectrum::default(); // thermal-Watt stand-in
        let kerma = Kerma::from_reconr(&recon(235.0, vec![sec]), &nu, &chi, &[]);
        let h = kerma.eval(1.0e5);
        let heating_number_ev = h / sigma_f; // H(E)/σ(E), eV per fission
        assert!(
            heating_number_ev > 0.0,
            "fission heating must be positive, got {heating_number_ev}"
        );
        // ~200 MeV per fission minus a few MeV carried off by ~2.4 fission
        // neutrons at ~2 MeV each — should land well within [150, 200] MeV.
        assert!(
            (150.0e6..200.0e6).contains(&heating_number_ev),
            "heating number {heating_number_ev} eV should be ~190 MeV/fission"
        );
    }

    /// H1+H2+H3+H4 all sum additively on the shared union grid — the full
    /// tonight-scoped kinematic-limit KERMA.
    #[test]
    fn all_four_phases_sum_additively() {
        let awr = 235.0;
        let elastic = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e6, 10.0)],
        };
        let capture = ReconrSection {
            mt: MtReaction::Mt102Capture,
            qi: 6.0e6,
            pairs: vec![(1.0e6, 1.0)],
        };
        let level = ReconrSection {
            mt: MtReaction::from_any(51),
            qi: -1.0e6,
            pairs: vec![(1.0e6, 0.3)],
        };
        let fission = ReconrSection {
            mt: MtReaction::Mt18Fission,
            qi: 200.0e6,
            pairs: vec![(1.0e6, 1.2)],
        };
        let nu = NuBar {
            energy: vec![1.0e6],
            nu_total: vec![2.5],
        };
        let chi = FissionSpectrum::Watt {
            a: 0.988e6,
            b: 2.249e-6,
        };
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![elastic, capture, level, fission]),
            &nu,
            &chi,
            &[],
        );
        let h = kerma.eval(1.0e6);

        let f = 2.0 * awr / (awr + 1.0).powi(2);
        let h_elastic = 10.0 * 1.0e6 * f;
        let h_capture = 1.0 * (1.0e6 + 6.0e6);
        let h_level = 0.3 * (1.0e6 * f + (-1.0e6) / (awr + 1.0));
        let mean_e_prime = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
        let h_fission = 1.2 * (1.0e6 + 200.0e6 - 2.5 * mean_e_prime);
        let expected = h_elastic + h_capture + h_level + h_fission;
        assert!(
            (h - expected).abs() / expected < 1.0e-9,
            "got {h}, want {expected}"
        );
    }

    /// **H5.** An (n,2n) reaction (MT=16) subtracts the mean energy of *two*
    /// escaping neutrons from the `E + Q` energy balance: with a fixed-parameter
    /// Watt emission spectrum (closed-form mean `1.5a + ¼a²b`) the heating is
    /// `σ·(E + Q − 2·⟨E'⟩)` exactly.
    #[test]
    fn n2n_heating_subtracts_two_neutron_means() {
        let q = -6.0e6; // representative (n,2n) threshold Q
        let sigma = 0.7; // barn, flat
        let sec = ReconrSection {
            mt: MtReaction::Mt16N2n,
            qi: q,
            pairs: vec![(1.0e7, sigma), (1.4e7, sigma)],
        };
        // Reuse the general MF=5 machinery: a Watt-shaped emitted-neutron spectrum.
        let (a, b) = (0.5e6, 4.0e-6);
        let spectrum = FissionSpectrum::Watt { a, b };
        let mean_e_prime = 1.5 * a + 0.25 * a * a * b;
        let kerma = Kerma::from_reconr(
            &recon(56.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[(MtReaction::Mt16N2n, EmissionSpectrum::Mf5(spectrum))],
        );
        for &e in &[1.0e7, 1.4e7] {
            let h = kerma.eval(e);
            let expected = sigma * (e + q - 2.0 * mean_e_prime);
            assert!(
                (h - expected).abs() / expected.abs() < 1.0e-9,
                "H({e})={h}, want {expected}"
            );
        }
    }

    /// **H5, multiplicity.** (n,3n) (MT=17) removes *three* neutron means, (n,4n)
    /// (MT=37) *four* — the multiplicity comes from the MT, so with the same
    /// emission spectrum the (n,3n) heating is lower than (n,2n)'s by exactly one
    /// extra `σ·⟨E'⟩`.
    #[test]
    fn n3n_and_n4n_use_higher_multiplicity() {
        let q = -12.0e6;
        let sigma = 0.4;
        let (a, b) = (0.6e6, 3.0e-6);
        let mean_e_prime = 1.5 * a + 0.25 * a * a * b;
        let e = 2.0e7;
        for (mt, yld) in [(MtReaction::Mt17N3n, 3.0), (MtReaction::Mt37N4n, 4.0)] {
            let sec = ReconrSection {
                mt,
                qi: q,
                pairs: vec![(e, sigma)],
            };
            let kerma = Kerma::from_reconr(
                &recon(90.0, vec![sec]),
                &NuBar::default(),
                &FissionSpectrum::default(),
                &[(mt, EmissionSpectrum::Mf5(FissionSpectrum::Watt { a, b }))],
            );
            let expected = sigma * (e + q - yld * mean_e_prime);
            let h = kerma.eval(e);
            assert!(
                (h - expected).abs() / expected.abs() < 1.0e-9,
                "MT={:?}: H={h}, want {expected}",
                mt
            );
        }
    }

    /// **H5.** Continuum inelastic (MT=91) is the yield-1 member of the family:
    /// a single escaping neutron carrying the emission spectrum's mean, using the
    /// section's own Q — the continuum analogue of a discrete level (H3), but with
    /// the *spectrum* mean instead of the two-body kinematic mean.
    #[test]
    fn continuum_inelastic_is_single_neutron_with_spectrum_mean() {
        let q = -2.0e6;
        let sigma = 1.1;
        let (a, b) = (0.8e6, 2.0e-6);
        let mean_e_prime = 1.5 * a + 0.25 * a * a * b;
        let e = 5.0e6;
        let sec = ReconrSection {
            mt: MtReaction::Mt91NnContinuum,
            qi: q,
            pairs: vec![(e, sigma)],
        };
        let kerma = Kerma::from_reconr(
            &recon(28.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[(
                MtReaction::Mt91NnContinuum,
                EmissionSpectrum::Mf5(FissionSpectrum::Watt { a, b }),
            )],
        );
        let h = kerma.eval(e);
        let expected = sigma * (e + q - 1.0 * mean_e_prime);
        assert!(
            (h - expected).abs() / expected.abs() < 1.0e-9,
            "H={h}, want {expected}"
        );
    }

    /// **H5, physical sanity.** A 14-MeV (n,2n) on a mid-mass nucleus deposits a
    /// small *positive* heating: after the ~8 MeV threshold Q and two ~1–2 MeV
    /// neutrons escape, only a few MeV of recoil + charged-particle energy is
    /// left local — distinct from the exact-formula check (catches a sign or
    /// multiplicity blunder the algebra-mirroring check structurally cannot).
    #[test]
    fn n2n_heating_is_small_and_positive_at_14_mev() {
        let q = -8.0e6;
        let sigma = 0.5;
        let e = 1.4e7;
        let sec = ReconrSection {
            mt: MtReaction::Mt16N2n,
            qi: q,
            pairs: vec![(e, sigma)],
        };
        // Evaporation-like emitted neutrons, ~1.5 MeV mean (θ ≈ 0.75 MeV, ⟨E'⟩=2θ).
        let spectrum = FissionSpectrum::Watt {
            a: 0.5e6,
            b: 3.0e-6,
        };
        let kerma = Kerma::from_reconr(
            &recon(56.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[(MtReaction::Mt16N2n, EmissionSpectrum::Mf5(spectrum))],
        );
        let heating_number_ev = kerma.eval(e) / sigma; // H/σ, eV per event
        assert!(
            heating_number_ev > 0.0,
            "(n,2n) heating must be positive, got {heating_number_ev}"
        );
        assert!(
            heating_number_ev < e + q,
            "escaping neutrons must reduce heating below E+Q={}",
            e + q
        );
        assert!(
            (0.0..5.0e6).contains(&heating_number_ev),
            "14-MeV (n,2n) heating {heating_number_ev} eV should be a few MeV"
        );
    }

    /// **H5, additive with H1–H4.** A section list mixing elastic, capture, a
    /// discrete level, fission, and an (n,2n) sums all five models on the shared
    /// union grid.
    #[test]
    fn h5_sums_additively_with_all_prior_phases() {
        let awr = 235.0;
        let e = 1.4e7;
        let elastic = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(e, 5.0)],
        };
        let capture = ReconrSection {
            mt: MtReaction::Mt102Capture,
            qi: 6.0e6,
            pairs: vec![(e, 0.1)],
        };
        let level = ReconrSection {
            mt: MtReaction::from_any(51),
            qi: -1.0e6,
            pairs: vec![(e, 0.2)],
        };
        let fission = ReconrSection {
            mt: MtReaction::Mt18Fission,
            qi: 200.0e6,
            pairs: vec![(e, 1.0)],
        };
        let n2n = ReconrSection {
            mt: MtReaction::Mt16N2n,
            qi: -8.0e6,
            pairs: vec![(e, 0.6)],
        };
        let nu = NuBar {
            energy: vec![e],
            nu_total: vec![2.6],
        };
        let chi = FissionSpectrum::Watt {
            a: 0.988e6,
            b: 2.249e-6,
        };
        let (a, b) = (0.5e6, 4.0e-6);
        let kerma = Kerma::from_reconr(
            &recon(awr, vec![elastic, capture, level, fission, n2n]),
            &nu,
            &chi,
            &[(
                MtReaction::Mt16N2n,
                EmissionSpectrum::Mf5(FissionSpectrum::Watt { a, b }),
            )],
        );
        let h = kerma.eval(e);

        let f = 2.0 * awr / (awr + 1.0).powi(2);
        let mean_chi = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
        let mean_n2n = 1.5 * a + 0.25 * a * a * b;
        let expected = 5.0 * e * f
            + 0.1 * (e + 6.0e6)
            + 0.2 * (e * f + (-1.0e6) / (awr + 1.0))
            + 1.0 * (e + 200.0e6 - 2.6 * mean_chi)
            + 0.6 * (e - 8.0e6 - 2.0 * mean_n2n);
        assert!(
            (h - expected).abs() / expected.abs() < 1.0e-9,
            "got {h}, want {expected}"
        );
    }

    // ── H7: damage energy (MT=444) ─────────────────────────────────────────

    /// **H7.** The default displacement-threshold table matches NJOY's built-in
    /// values (C=31, Al=27, Fe=40, W=90, Pb=25, and the 25 eV fallback).
    #[test]
    fn default_displacement_energies_match_njoy_table() {
        assert_eq!(default_displacement_energy(6), 31.0); // carbon
        assert_eq!(default_displacement_energy(13), 27.0); // aluminium
        assert_eq!(default_displacement_energy(26), 40.0); // iron
        assert_eq!(default_displacement_energy(74), 90.0); // tungsten
        assert_eq!(default_displacement_energy(82), 25.0); // lead
        assert_eq!(default_displacement_energy(1), 25.0); // fallback
    }

    /// **H7.** The Lindhard partition is 0 below the displacement threshold, and
    /// above it never exceeds the recoil energy (electronic losses only ever
    /// *remove* energy from the displacement channel).
    #[test]
    fn lindhard_partition_bounds() {
        let (z, a, e_d) = (26.0, 56.0, 40.0); // iron
        assert_eq!(lindhard_damage(30.0, z, a, z, a, e_d), 0.0, "below E_d ⇒ 0");
        for &er in &[50.0, 1.0e3, 1.0e5, 1.0e7] {
            let df = lindhard_damage(er, z, a, z, a, e_d);
            assert!(df > 0.0 && df <= er, "0 < df({er})={df} ≤ E_R");
        }
    }

    /// **H7.** At low recoil energy (just above threshold) almost all energy
    /// stays in the displacement channel (`df ≈ E_R`); at very high recoil energy
    /// electronic stopping dominates and the damage fraction collapses
    /// (`df/E_R ≪ 1`) — the qualitative signature of the Lindhard partition.
    #[test]
    fn lindhard_damage_fraction_falls_with_energy() {
        let (z, a, e_d) = (26.0, 56.0, 40.0);
        let low = lindhard_damage(100.0, z, a, z, a, e_d) / 100.0;
        let high = lindhard_damage(1.0e7, z, a, z, a, e_d) / 1.0e7;
        assert!(low > 0.7, "near threshold df/E_R={low} should be ≈1");
        assert!(high < 0.1, "at 10 MeV df/E_R={high} should be ≪1");
        assert!(low > high, "damage fraction must fall with recoil energy");
    }

    /// **H7.** Elastic damage cross section: zero at incident energies so low the
    /// maximum recoil `E_max = 4A/(A+1)²·E` cannot reach `E_d`, positive above,
    /// and monotonically increasing — and the per-collision damage energy never
    /// exceeds the per-collision *heating* (H1), since displacement energy is a
    /// partition of the recoil energy that heating already counts in full.
    #[test]
    fn elastic_damage_is_bounded_by_heating() {
        let awr = 56.0; // iron
        let z = 26;
        let e_d = default_displacement_energy(z);
        let sigma = 3.0; // barn, flat
        let sec = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![
                (1.0e2, sigma),
                (1.0e5, sigma),
                (1.0e6, sigma),
                (1.4e7, sigma),
            ],
        };
        let dmg = DamageEnergy::from_reconr(&recon(awr, vec![sec]), z, e_d);

        // E_max at 100 eV: 4·56/57²·100 ≈ 6.9 eV < E_d=40 ⇒ no damage.
        assert_eq!(dmg.eval(1.0e2), 0.0, "below-threshold recoil ⇒ zero damage");

        let f = 2.0 * awr / (awr + 1.0).powi(2); // H1 heating factor
        let mut prev = 0.0;
        for &e in &[1.0e5, 1.0e6, 1.4e7] {
            let d = dmg.eval(e);
            let heating = sigma * e * f; // H1 elastic heating (eV·barn)
            assert!(d > 0.0, "damage must be positive at {e} eV, got {d}");
            assert!(
                d < heating,
                "damage {d} must be < heating {heating} at {e} eV"
            );
            assert!(d > prev, "damage must increase with energy ({e} eV)");
            prev = d;
        }
    }

    /// **H7.** A material with no elastic section yields an empty MT=444 table
    /// (only the two-body recoil channels are ported so far).
    #[test]
    fn damage_empty_without_recoil_channels() {
        // Pure capture (MT=102): no two-body neutron-scattering recoil channel.
        let sec = ReconrSection {
            mt: MtReaction::Mt102Capture,
            qi: 6.0e6,
            pairs: vec![(1.0e5, 2.0), (1.0e6, 2.0)],
        };
        let dmg = DamageEnergy::from_reconr(&recon(56.0, vec![sec]), 26, 40.0);
        assert!(dmg.energy.is_empty());
        assert_eq!(dmg.eval(5.0e5), 0.0);
    }

    /// **H7, recoil bounds.** Elastic (`Q=0`) recoil spans `[0, 4C]`; a discrete
    /// level (`Q<0`) narrows the window to `[C(1−g)², C(1+g)²]` with `g<1`, so its
    /// maximum recoil is strictly below the elastic backscatter maximum — the
    /// residual nucleus keeps `|Q|` as excitation, leaving less for recoil.
    #[test]
    fn two_body_recoil_bounds_narrow_with_negative_q() {
        let awr = 56.0_f64;
        let e = 5.0e6_f64;
        let c = awr / (awr + 1.0).powi(2) * e;
        let (el_min, el_max) = two_body_recoil_bounds(e, 0.0, awr);
        assert!((el_min).abs() < 1.0, "elastic E_min≈0, got {el_min}");
        assert!(
            (el_max - 4.0 * c).abs() / (4.0 * c) < 1e-12,
            "elastic E_max=4C"
        );

        let (lv_min, lv_max) = two_body_recoil_bounds(e, -2.0e6, awr);
        assert!(lv_min > 0.0, "level E_min>0, got {lv_min}");
        assert!(lv_max < el_max, "level E_max {lv_max} < elastic {el_max}");
    }

    /// **H7.** A discrete inelastic level (MT=51) produces a positive damage
    /// cross section above its threshold, and it sums additively with elastic on
    /// the shared union grid — the total MT=444 exceeds the elastic-only part.
    #[test]
    fn discrete_level_adds_to_damage() {
        let awr = 56.0; // iron
        let z = 26;
        let e_d = default_displacement_energy(z);
        let elastic = ReconrSection {
            mt: MtReaction::Mt2Elastic,
            qi: 0.0,
            pairs: vec![(1.0e6, 3.0), (5.0e6, 3.0)],
        };
        let level = ReconrSection {
            mt: MtReaction::from_any(51),
            qi: -0.85e6, // Fe-56 first level ~0.85 MeV
            pairs: vec![(1.0e6, 1.0), (5.0e6, 1.0)],
        };
        let elastic_only = DamageEnergy::from_reconr(&recon(awr, vec![elastic]), z, e_d);
        let both = DamageEnergy::from_reconr(
            &recon(
                awr,
                vec![
                    ReconrSection {
                        mt: MtReaction::Mt2Elastic,
                        qi: 0.0,
                        pairs: vec![(1.0e6, 3.0), (5.0e6, 3.0)],
                    },
                    level,
                ],
            ),
            z,
            e_d,
        );
        let e = 5.0e6; // well above the ~0.85 MeV level threshold
        let d_el = elastic_only.eval(e);
        let d_both = both.eval(e);
        assert!(d_el > 0.0, "elastic damage positive, got {d_el}");
        assert!(
            d_both > d_el,
            "discrete level must add damage: both={d_both} > elastic-only={d_el}"
        );
    }
}

/// Run the HEATR card-input driver (NJOY module entry point).
///
/// **Status:** this module's processing physics is ported (see its `README.md`
/// and the typed API above); the NJOY *card-input driver* itself is not yet
/// ported, so this returns [`crate::NjoyError::NotPorted`]. Use the module's
/// typed API directly rather than this driver.
pub fn run() -> Result<(), crate::NjoyError> {
    Err(crate::NjoyError::NotPorted(
        "heatr driver (physics ported — use the module API)",
    ))
}
