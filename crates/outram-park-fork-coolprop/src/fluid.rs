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
}

impl Fluid {
    /// The hardcoded Helmholtz EOS for this fluid.
    pub fn eos(self) -> &'static FluidEos {
        match self {
            Fluid::Water => &fluids::water::WATER,
        }
    }

    /// The fluid's name (as in CoolProp).
    pub fn name(self) -> &'static str {
        self.eos().name
    }
}
