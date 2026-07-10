//! Hardcoded per-fluid coefficient blocks for the incompressible backend
//! (`const IncompressibleFluid`) — the incompressible analogue of
//! [`crate::fluids`].
//!
//! **Only T66 is ported so far** (transcribed by hand from
//! `reference/CoolProp/dev/incompressible_liquids/json/T66.json`, MIT). The
//! other 125 CoolProp incompressible fluids are a follow-up (bead op-kbc.15) —
//! ideally via a `dev/gen_incompressible.py` codegen script mirroring
//! `dev/gen_fluid.py`, given the number of fluids and the risk of hand-
//! transcription error at that scale.

use super::{IncompressibleFluid, IncompressibleKind, PropertyFit, PropertyForm};

/// Therminol 66 (a pure synthetic heat-transfer oil), `T ∈ [273.15, 653.15] K`.
/// CoolProp `dev/incompressible_liquids/json/T66.json`.
pub const T66_INCOMP: IncompressibleFluid = IncompressibleFluid {
    name: "T66",
    kind: IncompressibleKind::Pure,
    t_range: (273.15, 653.15),
    x_range: (0.0, 0.0),
    t_base: 463.15,
    x_base: 0.0,
    density: PropertyFit {
        form: PropertyForm::Polynomial,
        coeffs: &[&[892.4651], &[-0.7182484], &[-0.0003391502], &[-7.434511e-07]],
    },
    heat_capacity: PropertyFit {
        form: PropertyForm::Polynomial,
        coeffs: &[&[2157.819], &[3.630114], &[0.0009171095], &[1.004461e-06]],
    },
    conductivity: PropertyFit {
        form: PropertyForm::Polynomial,
        coeffs: &[&[0.1065831], &[-9.17879e-05], &[-1.57373e-07], &[1.361587e-11]],
    },
    viscosity: PropertyFit { form: PropertyForm::Exponential, coeffs: &[&[653.8723, -206.0943, 9.556995]] },
};
