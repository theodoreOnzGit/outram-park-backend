use t_ps_flash::{t_ps_3a, t_ps_3b};
use uom::si::{f64::*, specific_heat_capacity::kilojoule_per_kilogram_kelvin};
use v_ps_flash::{v_ps_3a, v_ps_3b};

/// The 3a/3b subregion boundary entropy for the region-3 `(p,s)` backward
/// equations: `s3a3b ~= 4.412 kJ/(kg K)`. Below this value use subregion
/// 3a, above it use subregion 3b.
#[inline]
pub fn s_3a3b_backwards_ps_boundary() -> SpecificHeatCapacity {
    let s = SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(4.412_021_482_234_76);
    s
}

/// region 3a/3b temperature backward equation, `(p,s) -> T`
pub mod t_ps_flash;
/// region 3a/3b specific-volume backward equation, `(p,s) -> v`
pub mod v_ps_flash;

/// Region-3 backward equation: returns specific volume given pressure and
/// specific entropy, `(p,s) -> v`.
/// Assumes `(p,s)` is already known to be in region 3; dispatches to the
/// 3a or 3b subregion equation via `s_3a3b_backwards_ps_boundary`.
#[inline]
pub fn v_ps_3(p: Pressure, s: SpecificHeatCapacity) -> SpecificVolume {
    let s_boundary = s_3a3b_backwards_ps_boundary();

    if s > s_boundary {
        return v_ps_3b(p, s);
    } else {
        return v_ps_3a(p, s);
    };
}

/// Region-3 backward equation: returns temperature given pressure and
/// specific entropy, `(p,s) -> T`.
/// Assumes `(p,s)` is already known to be in region 3; dispatches to the
/// 3a or 3b subregion equation via `s_3a3b_backwards_ps_boundary`.
#[inline]
pub fn t_ps_3(p: Pressure, s: SpecificHeatCapacity) -> ThermodynamicTemperature {
    let s_boundary = s_3a3b_backwards_ps_boundary();

    if s > s_boundary {
        return t_ps_3b(p, s);
    } else {
        return t_ps_3a(p, s);
    };
}
