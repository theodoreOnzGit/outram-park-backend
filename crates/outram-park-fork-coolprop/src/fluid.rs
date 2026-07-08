//! The fluid selector — an **enum**, matching each fluid to its hardcoded
//! [`FluidEos`]. This is the enum-dispatch replacement for CoolProp's
//! string-keyed fluid lookup / backend polymorphism: adding a fluid is a new
//! variant, and every `match` on `Fluid` becomes exhaustive.

use crate::eos::FluidEos;
use crate::fluids;

/// A supported pure fluid.
///
/// The set grows as fluids are ported from CoolProp (bead op-kbc). Each variant
/// maps to a hardcoded `const` [`FluidEos`] via [`Fluid::eos`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Fluid {
    /// Water (IAPWS-95). CoolProp `Water`.
    Water,
    /// Helium (Ortiz-Vega et al.). CoolProp `Helium`. Its EOS has only
    /// Power + Gaussian residual terms (no non-analytic), so it is fully
    /// accurate — including near the critical point — with the current engine.
    Helium,
    /// Nitrogen (Span et al.). CoolProp `Nitrogen`. Exercises the ideal-gas
    /// `Power` and `PlanckEinsteinFunctionT` terms.
    Nitrogen,
    /// Fluorine (de Reuck). CoolProp `Fluorine`. Exercises the residual
    /// `Exponential` and the ideal `Power` + `PlanckEinsteinGeneralized` terms.
    Fluorine,
    /// Methanol (de Reuck & Craven). CoolProp `Methanol`. Exercises the
    /// residual `DoubleExponential` + `Exponential` and the ideal `CP0PolyT`.
    Methanol,
    /// R125 / pentafluoroethane (Lemmon & Jacobsen 2005). CoolProp `R125`.
    /// Exercises the residual `DoubleExponential` (Lemmon2005 lowering).
    R125,
    /// Ammonia (Tillner-Roth et al.). CoolProp `Ammonia`. Exercises the
    /// residual `GaoB` bell term (and `Exponential`).
    Ammonia,
    /// R22 / chlorodifluoromethane. CoolProp `R22`. Exercises the ideal
    /// `CP0Constant` term.
    R22,
    /// n-Heptane. CoolProp `n-Heptane`. Exercises the ideal `CP0AlyLee` term
    /// (lowered to `CP0PolyT` + `PlanckEinsteinGeneralized`).
    Heptane,
}

impl Fluid {
    /// The hardcoded Helmholtz EOS for this fluid.
    pub fn eos(self) -> &'static FluidEos {
        match self {
            Fluid::Water => &fluids::water::WATER,
            Fluid::Helium => &fluids::helium::HELIUM,
            Fluid::Nitrogen => &fluids::nitrogen::NITROGEN,
            Fluid::Fluorine => &fluids::fluorine::FLUORINE,
            Fluid::Methanol => &fluids::methanol::METHANOL,
            Fluid::R125 => &fluids::r125::R125,
            Fluid::Ammonia => &fluids::ammonia::AMMONIA,
            Fluid::R22 => &fluids::r22::R22,
            Fluid::Heptane => &fluids::n_heptane::N_HEPTANE,
        }
    }

    /// The fluid's name (as in CoolProp).
    pub fn name(self) -> &'static str {
        self.eos().name
    }
}
