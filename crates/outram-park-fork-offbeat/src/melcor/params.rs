//! The generic physical constants the [`super`] modules read (gh:#1335).
//!
//! Moved from `sembawang::bwr_mark1::params` on 2026-11-11, value
//! unchanged; that module re-exports it so the plant reads the same `f64`.
//! Plant data (geometry, sequence, tagged placeholders) stays in
//! `sembawang::bwr_mark1::params`.

/// The molar gas constant, J/(mol K), to the precision the Fukushima plant
/// model has always used (the CODATA 2018 exact value is 8.314 462 618 153 24;
/// the truncation is kept so results stay bit-identical across the move).
pub const R_GAS: f64 = 8.314_462;

#[cfg(test)]
mod tests {
    use super::*;

    /// The constant is the one the plant was built with: within 1e-7
    /// relative of CODATA 2018's exact 8.314 462 618 J/(mol K).
    #[test]
    fn r_gas_is_codata_to_its_stated_precision() {
        assert!((R_GAS / 8.314_462_618 - 1.0).abs() < 1e-7);
    }
}
