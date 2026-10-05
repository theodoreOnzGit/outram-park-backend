//! `HEATR` — heating (KERMA) cross section, ENDF MT=301.
//!
//! Computes the **kinematic-limit KERMA**: the heating cross section under the
//! assumption that every escaping *neutron* carries its kinetic energy away
//! from the local region, and everything else (nuclear recoil, charged
//! particles, and — per NJOY's own documented fallback — photon energy when no
//! photon-production data is processed) deposits locally. `heatr.f90` computes
//! this exact quantity as a **check** (`kchk`) against its full photon
//! energy-balance method; here it is the primary ~~(and, for now, only)~~ result.
//! **CORRECTED 2026-10-05 (GitHub #535):** not the only one.
//! [`Kerma::with_energy_balance`] subtracts the evaluation's photon energy
//! production (MT=442, from [`crate::photon`]: MF=12 `LO=1`, MF=13, MF=15)
//! from it, and the ACE route (`acer`, when the deck runs HEATR) uses that
//! corrected value. ~~It is a partial H6: no MF=12 `LO=2` cascades, no MF=6
//! photons, no capture recoil (`disgam`), and the neutron side stays the
//! kinematic limit. It has no NJOY comparison of its own.~~ **CORRECTED
//! 2026-10-05 (#535, H6a):** the photon side is complete (LO=2 cascades,
//! MF=6 photons, capture by energy balance with `disgam`'s recoil) and
//! matches NJOY's MT=442 at its 7-figure print precision on Fe-58 and Si-28
//! (`tests/heatr_mt442_vs_njoy2016.rs`). The kinematic arm now deposits
//! HEATR's Q per reaction ([`Kerma::from_endf`]; `QI` alone left a discrete
//! level's excitation out). ~~The neutron side is still the kinematic
//! estimate (H6b), which leaves the energy-balance MT=301 1.9–2.9× NJOY's
//! between 2 and 5 MeV on both nuclides.~~ **CORRECTED 2026-10-05 (#535, H6b
//! part 1):** [`Kerma::from_endf`] now follows `nheat` for the two-body
//! channels: `disbar`'s MF=4 mean outgoing energy for elastic and the
//! discrete levels without MF=6 (`twobody.rs`), `σ·(E + q0)` for MT=600-849,
//! and `nheat`'s skip list (MT=4 was heated beside its own levels). Above the
//! first inelastic threshold the energy-balance MT=301 is now within 0.39 %
//! (Fe-58) and 0.74 % (Si-28) of NJOY's in median, and Si-28 below it matches
//! at print precision. The continuum and MF=6 neutron means (`conbar`,
//! `sixbar`, H6b part 2) are still the kinematic estimate and carry the
//! 14-150 MeV residual.
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
//!   [`spectra::single_neutron_factor`] for the derivation).
//! - **H4** (done): fission (MT=18, 19–21, 38),
//!   `H(E) = σ_f(E)·[E + Q_fission − ν̄(E)·⟨E'⟩]`, reusing [`NuBar`](crate::nuclear_data::secondary::NuBar) and
//!   [`FissionSpectrum::mean_energy`](crate::nuclear_data::secondary::FissionSpectrum::mean_energy).
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
//!   channels share the same [`damage::lindhard_damage`] partition and follow. Distinct
//!   output quantity from the MT=301 heating KERMA above.
//! - **H6** (in progress): the full ~~photon~~ energy-balance method (`nheat`
//!   with `disbar`/`conbar`/`sixbar` for the neutron side and `hconvr`/`gheat`
//!   for photons). **H6a, the photon side, is done** (2026-10-05, see above),
//!   together with `nheat`'s deposited-Q rule. **H6b part 1, the two-body
//!   neutron side (`disbar`), is done** (2026-10-05). H6b part 2 (`conbar`,
//!   `sixbar`: continuum and MF=6 neutron means) and H6c (MF=6 capture
//!   recoil, `kchk`) are planned on GitHub #535.
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

//!
//! ## Layout
//!
//! Split by functional group (crate file-size rule, `docs/porting-plan.md` §5):
//!
//! - [`spectra`](self) — H5 emitted-neutron spectra ([`EmissionSpectrum`],
//!   [`build_emission_spectra`]) and the per-MT heating-model dispatch shared
//!   by the KERMA sum.
//! - [`Kerma`] (`kerma.rs`) — the MT=301 kinematic-limit heating cross section
//!   (H1–H5).
//! - [`DamageEnergy`] (`damage.rs`) — MT=444 damage-energy production (H7),
//!   with the Lindhard partition and the NJOY `E_d` table.

mod damage;
mod kerma;
mod spectra;
mod twobody;
#[cfg(test)]
mod tests;

pub use damage::{default_displacement_energy, DamageEnergy};
pub use kerma::Kerma;
pub use spectra::{build_emission_spectra, EmissionSpectrum};

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
