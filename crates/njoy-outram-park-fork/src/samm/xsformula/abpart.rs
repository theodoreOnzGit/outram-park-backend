//! Breit-Wigner denominator terms at a given incident energy — ported
//! from `abpart`'s non-derivative core (`samm.f90:2936-2948`).
//!
//! **Scope note:** `abpart`'s `Want_Partial_Derivs`-gated second half
//! (`samm.f90:2950-3006`, generating `upr`/`upi`/`pr`/`pii` — the
//! energy-dependent pieces of the resonance-parameter partial
//! derivatives) is not ported here. It has no caller yet (RECONR disables
//! derivatives) and belongs with `derres`/`derext`, to be built alongside
//! `ERRORR` — see `src/samm/README.md`'s scope-history note and
//! [[njoy-no-orphaned-dead-code]] in the porting-plan.

use crate::samm::betset::ResonanceAmplitudes;
use crate::samm::mf2::RmlResonance;

/// Per-resonance Breit-Wigner terms at the current energy `E` — ported
/// from `abpart`'s `difen`/`xden`/`alphar`/`alphai` (`samm.f90:2939-2948`):
/// `difen = E_lambda - E`, `xden = 1/(difen^2 + (Gamma_gamma/2)^2)`,
/// `alphar = difen * xden`, `alphai = (Gamma_gamma/2) * xden` — the real
/// and imaginary parts of `1/(E_lambda - E - i*Gamma_gamma/2)` up to the
/// `xden` normalization, feeding directly into [`super::setr::setr`]'s
/// `R_{cc'} += alphar*beta_{cc'} + i*alphai*beta_{cc'}` accumulation.
#[derive(Debug, Clone, Copy, Default)]
pub struct AlphaTerms {
    pub difen: f64,
    pub xden: f64,
    pub alphar: f64,
    pub alphai: f64,
}

/// Compute [`AlphaTerms`] for every resonance in a spin group at incident
/// energy `energy` (eV) — ported from `abpart` (`samm.f90:2923-2948`).
/// `amplitudes` must be [`crate::samm::betset::compute_resonance_amplitudes`]'s
/// output for this same group (one entry per `resonances` entry, same
/// order) — only its `gbetpr` field (the eliminated-channel amplitude) is
/// used here.
pub fn abpart(
    resonances: &[RmlResonance],
    amplitudes: &[ResonanceAmplitudes],
    energy: f64,
) -> Vec<AlphaTerms> {
    resonances
        .iter()
        .zip(amplitudes)
        .map(|(res, amp)| {
            let difen = res.energy - energy;
            let aa = difen * difen + amp.gbetpr[2]; // gbetpr[2] = |Gamma_gamma/2|^2
            let xden = 1.0 / aa;
            let alphar = difen * xden;
            let alphai = amp.gbetpr[1] * xden; // gbetpr[1] = |Gamma_gamma/2|
            AlphaTerms {
                difen,
                xden,
                alphar,
                alphai,
            }
        })
        .collect()
}
