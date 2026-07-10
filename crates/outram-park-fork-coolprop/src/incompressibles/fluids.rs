//! Hardcoded per-fluid coefficient blocks for the incompressible backend
//! (`const IncompressibleFluid`), to be generated from CoolProp's
//! `dev/incompressible_liquids/` + solution JSON by a codegen follow-up —
//! the incompressible analogue of [`crate::fluids`].
//!
//! **Scaffold only (bead op-kbc.15).** No coefficient tables are ported yet;
//! this file exists to fix the module path and the naming convention
//! (`<NAME>_INCOMP`). Populate it with `dev/gen_incompressible.py` (to be
//! written) the way `dev/regen_all.py` fills `crate::fluids`.
#![allow(dead_code)] // TODO(op-kbc.15): drop once data is generated

// TODO(op-kbc.15): emit one `pub const <NAME>_INCOMP: IncompressibleFluid = …`
// per CoolProp incompressible fluid here, and wire them into
// `Incompressible::data()`. Example shape (values are placeholders, not real):
//
// use super::{IncompressibleFluid, IncompressibleKind};
//
// pub const EXAMPLE_THERMAL_OIL_INCOMP: IncompressibleFluid = IncompressibleFluid {
//     name: "ExampleThermalOil",
//     kind: IncompressibleKind::Pure,
//     t_range: (273.15, 573.15),
//     x_range: (0.0, 0.0),
//     density:      &[&[/* c_i0 */]],
//     heat_capacity:&[&[/* c_i0 */]],
//     conductivity: &[&[/* c_i0 */]],
//     log_viscosity:&[&[/* c_i0 */]],
//     log_psat:     &[],
// };
