//! Heat capacities of the non-condensable gases, N2 and H2.
//!
//! **Since 2026-11-01 the code lives in `tampines_steam_tables::gas_mixture::melcor_ncg`**
//! (gh:#1093; maintainer direction: `sembawang` depends on `tampines`, so the
//! thermal-hydraulic properties live on that side). It moved verbatim, with
//! the same constants and operation order, so every value is bit-identical;
//! this module forwards to it, and its test moved with it.
//!
//! Each has its own `c_v(T)`, from MELCOR 2.2's NCG library fits (Vol. 2,
//! SAND2023-10994 O, NCG/H2O-RM-33/34; the form is Vol. 1, SAND2017-0455 O,
//! NCG-UG-2, Eq 1.1; no source used):
//!
//! ```text
//! c_v(T) = cv0 + cv1 T + cv2 T^2 + cv3 T^3 + cvsqrt/sqrt(T) + cvm1/T + cvm2/T^2
//! ```
//!
//! The coefficients are mass-specific and apply inside `T_low..=T_up`. Outside
//! that range `c_v` is held at its boundary value. Internal energy is
//! measured from 0 K, the containment's existing convention, so it is
//! `c_v(T_low) T` below `T_low`, the integral of the fit inside the range,
//! and linear in `c_v(T_up)` above `T_up`. Before 2026-10-29 these were
//! constants, 20.8 (N2) and 20.5 (H2) J/(mol K), and the
//! `without_gas_cv_t` ablation restores them.

pub use tampines_steam_tables::gas_mixture::melcor_ncg::{
    cp_h2_per_kg, cv_h2, cv_n2, h2_enthalpy_above_298, h2_molar_enthalpy, ncg_c, ncg_u, u_h2, u_n2,
};

#[cfg(test)]
mod tests {
    use super::super::params::{M_H2, M_N2, R_GAS};
    use tampines_steam_tables::gas_mixture::melcor_ncg as moved;

    /// The moved fits use this crate's constants exactly, which is what
    /// keeps every value bit-identical to the code before the move.
    #[test]
    fn the_moved_fits_use_this_crates_constants() {
        assert_eq!((moved::M_H2, moved::M_N2, moved::R_GAS), (M_H2, M_N2, R_GAS));
        assert_eq!(super::cv_n2(300.0, true), moved::cv_n2(300.0, true));
    }
}
