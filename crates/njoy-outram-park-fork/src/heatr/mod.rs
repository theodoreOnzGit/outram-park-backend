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
