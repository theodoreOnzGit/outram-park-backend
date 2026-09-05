// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to the boundary-conductivity and interface-temperature loop of
// `corrosion/corrosionModel/zircaloyOuterCorrosion.C::correct(...)`.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! What the oxide layer does to heat transfer.
//!
//! # Why corrosion is a thermal problem, not only a chemical one
//!
//! Zirconia conducts heat about sixteen times worse than the Zircaloy it grows
//! on — roughly 0.94 W/(m·K) against 15. A 100 µm oxide layer is thin compared
//! with a 600 µm cladding wall, but its thermal resistance is nearly three
//! times the whole wall's, so it raises the temperature of
//! everything inside it — the metal, the gap, and the fuel. Since the oxidation
//! rate is Arrhenius in the metal/oxide **interface** temperature, a thicker
//! oxide makes the interface hotter, which makes the oxide grow faster. The
//! loop is closed, and this module is the part of it that turns thickness into
//! temperature.
//!
//! # What upstream does, and what this module ports
//!
//! Upstream OFFBEAT does not mesh the oxide layer. Instead, in
//! `zircaloyOuterCorrosion::correct`, it *modifies the boundary thermal
//! conductivity* of the outermost metal cell so that the same finite-volume
//! discretisation reproduces the extra resistance, and reconstructs the
//! interface temperature from the blend. That calculation is a pure function of
//! five scalars, so it ports cleanly, and it is here.
//!
//! Everything around it — the surface fields, the mesh registry lookups, the
//! `const_cast` write-back into the `k` patch field — does not port and is not
//! attempted; see the [module documentation](super).
//!
//! # Units
//!
//! Temperatures \[K\], conductivities \[W/(m·K)\], lengths \[m\]. Raw `f64`,
//! strict SI.

// NaN-safe guards. Throughout this module a rejection test is written
// `!(x > 0.0)` rather than `x <= 0.0`, deliberately: the negated form is TRUE
// for NaN, so one comparison rejects negatives, zero and NaN together. Clippy's
// `neg_cmp_op_on_partial_ord` suggests the positive form, which would let a NaN
// through and propagate it into a physical result. The idiom is intentional.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

/// OpenFOAM's `SMALL`, used by upstream to keep the oxide resistance finite
/// when the layer has zero thickness.
const SMALL: f64 = 1.0e-15;

/// Constant term \[W/(m·K)\] of upstream's ZrO2 conductivity fit.
const ZRO2_K_CONSTANT: f64 = 0.835;

/// Temperature slope \[W/(m·K²)\] of upstream's ZrO2 conductivity fit.
const ZRO2_K_SLOPE: f64 = 1.81e-4;

/// Thermal conductivity of the zirconia layer \[W/(m·K)\] at `temperature`
/// \[K\].
///
/// Upstream's hard-coded linear fit, from `zircaloyOuterCorrosion.C`:
///
/// `k_ox = 0.835 + 1.81e-4 · T`
///
/// Weakly increasing with temperature, unlike a fully dense ceramic — the fit
/// is for the porous, cracked, in-reactor oxide, not for laboratory zirconia.
///
/// # Values, for scale
///
/// `0.9255` W/(m·K) at 500 K; `0.9436` at 600 K; `0.9617` at 700 K; `1.0160`
/// at 1000 K. Compare Zircaloy at roughly 15 W/(m·K): the oxide is about
/// **16 times** the thermal resistance per unit thickness, so 60 µm of oxide
/// is thermally worth about 1 mm of metal — more than the whole cladding wall.
///
/// # Valid range and assumptions
///
/// Upstream states no range. The fit is used wherever corrosion is, i.e.
/// roughly 500–1500 K; it is a straight line and will keep returning finite,
/// slowly-rising numbers outside that, which is extrapolation, not physics.
/// Assumes an adherent in-reactor oxide of the kind LWR waterside corrosion
/// produces; it is not a model of the thick, spalled oxide of a severe
/// accident.
///
/// The temperature to pass is the **oxide's outer-surface** temperature, which
/// is what upstream uses (`Tb`, the patch field of `T`).
///
/// ```
/// use outram_park_fork_offbeat::corrosion::oxide_conductivity;
///
/// let k = oxide_conductivity(600.0);
/// assert!((k - 0.9436).abs() < 1.0e-4);
/// // Far worse a conductor than the metal underneath it.
/// assert!(k < 2.0);
/// ```
#[must_use]
pub fn oxide_conductivity(temperature: f64) -> f64 {
    ZRO2_K_CONSTANT + ZRO2_K_SLOPE * temperature
}

/// How an oxide layer couples the metal to its outer surface, thermally.
///
/// The result of [`oxide_thermal_coupling`]. All three fields come out of the
/// same blending factor, and are returned together because a caller reproducing
/// upstream needs both the temperature (to drive the kinetics) and the modified
/// conductivity (to feed back into the heat-conduction solve).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OxideThermalCoupling {
    /// Metal/oxide interface temperature \[K\] — **this is what the oxidation
    /// kinetics must be evaluated at**.
    ///
    /// Lies between the oxide's outer-surface temperature and the first metal
    /// cell's temperature, and moves towards the *cell* temperature (hotter, in
    /// an operating rod) as the oxide thickens.
    pub interface_temperature: f64,

    /// Effective boundary thermal conductivity \[W/(m·K)\] that reproduces the
    /// oxide's resistance without meshing it — upstream writes this back into
    /// the `k` patch field.
    ///
    /// Always less than the metal conductivity passed in, and it falls as the
    /// oxide thickens. That reduction *is* the insulating effect.
    pub boundary_conductivity: f64,

    /// The blending factor `β` \[-\], in `[0, 1]`.
    ///
    /// `β = α_ox / (α_ox + α_m)`, the oxide's share of the total conductance.
    /// `β = 1` means no oxide at all (the interface is the surface);
    /// `β → 0` means an oxide so resistive that the interface sits at the metal
    /// cell temperature. Exposed because it is the single number that says how
    /// much the oxide matters at this face.
    pub blending_factor: f64,
}

/// Interface temperature and effective boundary conductivity for an oxidised
/// wall.
///
/// Direct translation of the per-face loop in upstream's
/// `zircaloyOuterCorrosion::correct`:
///
/// ```text
/// k_ox  = 0.835 + 1.81e-4 · T_surface
/// α_ox  = k_ox / max(d_ox, SMALL)          // oxide conductance per unit area
/// α_m   = k_metal / distance               // metal-cell conductance per unit area
/// β     = α_ox / (α_ox + α_m)
/// k_b   = β · k_metal                      // effective boundary conductivity
/// T_i   = β · T_surface + (1 − β) · T_cell // interface temperature
/// ```
///
/// The two conductances are in series; `β` is the fraction of the total
/// temperature drop that falls across the *metal* half-cell, so the interface
/// temperature is that fraction of the way from the cell centre to the surface.
///
/// # Parameters
///
/// - `surface_temperature` — temperature \[K\] at the oxide's outer face, i.e.
///   the coolant-side wall temperature. Upstream's `Tb`.
/// - `first_cell_temperature` — temperature \[K\] at the centre of the metal
///   cell adjoining the wall. Upstream's `Tp`.
/// - `metal_conductivity` — thermal conductivity \[W/(m·K)\] of the cladding
///   metal in that cell; about 15 W/(m·K) for Zircaloy. Must be `>= 0`.
/// - `cell_to_face_distance` — distance \[m\] from that cell's centre to the
///   wall face. Upstream passes its reciprocal (`patch.deltaCoeffs()`); this
///   port takes the distance itself because that is the quantity a human can
///   picture. Must be `> 0`.
/// - `mean_oxide_thickness` — oxide thickness \[m\] to charge the resistance
///   against. Upstream uses the **mid-step average**,
///   `0.5·(S_old + S_new)`, floored at `SMALL`; pass the same if you are
///   reproducing an OFFBEAT run.
///
/// # Behaviour at the edges
///
/// - **Zero oxide** — `α_ox` becomes `k_ox/1e-15`, astronomically larger than
///   `α_m`, so `β → 1`, the boundary conductivity is unchanged and the
///   interface temperature is the surface temperature. Correct, and it is why
///   upstream's `SMALL` floor is there.
/// - **A non-positive `cell_to_face_distance` or a negative
///   `metal_conductivity`** returns `β = 1` and the surface temperature rather
///   than dividing by zero. This guard is this port's; upstream has none.
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::corrosion::oxide_thermal_coupling;
///
/// // Bare metal: the interface IS the surface.
/// let bare = oxide_thermal_coupling(600.0, 620.0, 15.0, 5.0e-5, 0.0);
/// assert!((bare.interface_temperature - 600.0).abs() < 1.0e-6);
/// assert!((bare.blending_factor - 1.0).abs() < 1.0e-9);
///
/// // 60 um of oxide: the interface is pulled towards the metal cell.
/// let oxidised = oxide_thermal_coupling(600.0, 620.0, 15.0, 5.0e-5, 6.0e-5);
/// assert!(oxidised.interface_temperature > 600.0);
/// assert!(oxidised.interface_temperature < 620.0);
/// assert!(oxidised.boundary_conductivity < 15.0);
/// ```
#[must_use]
pub fn oxide_thermal_coupling(
    surface_temperature: f64,
    first_cell_temperature: f64,
    metal_conductivity: f64,
    cell_to_face_distance: f64,
    mean_oxide_thickness: f64,
) -> OxideThermalCoupling {
    if !(cell_to_face_distance > 0.0) || metal_conductivity < 0.0 {
        return OxideThermalCoupling {
            interface_temperature: surface_temperature,
            boundary_conductivity: metal_conductivity.max(0.0),
            blending_factor: 1.0,
        };
    }

    let k_oxide = oxide_conductivity(surface_temperature);
    let thickness = mean_oxide_thickness.max(SMALL);
    let alpha_oxide = k_oxide / thickness;
    let alpha_metal = metal_conductivity / cell_to_face_distance;

    let total = alpha_oxide + alpha_metal;
    let beta = if total > 0.0 {
        alpha_oxide / total
    } else {
        1.0
    };

    OxideThermalCoupling {
        interface_temperature: beta * surface_temperature + (1.0 - beta) * first_cell_temperature,
        boundary_conductivity: beta * metal_conductivity,
        blending_factor: beta,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Self-consistency check against the closed form, not validation: the
    /// conductivity fit is a straight line and must reproduce its own
    /// coefficients exactly.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// `0.9436` W/(m·K) at 600 K, `0.9617` at 700 K, `1.0160` at 1000 K —
    /// monotonically increasing, and everywhere around one sixteenth of
    /// Zircaloy's ~15 W/(m·K).
    #[test]
    fn oxide_conductivity_follows_its_linear_fit() {
        assert_eq!(oxide_conductivity(0.0), 0.835);
        for t in [500.0, 600.0, 700.0, 1000.0, 1500.0] {
            assert!((oxide_conductivity(t) - (0.835 + 1.81e-4 * t)).abs() < 1.0e-15);
        }
        assert!((oxide_conductivity(600.0) - 0.9436).abs() < 1.0e-4);
        assert!((oxide_conductivity(700.0) - 0.9617).abs() < 1.0e-4);
        assert!((oxide_conductivity(1000.0) - 1.0160).abs() < 1.0e-4);

        // Monotone increasing, and always far below the metal.
        let mut previous = 0.0;
        for t in [400.0, 600.0, 800.0, 1000.0, 1200.0] {
            let k = oxide_conductivity(t);
            assert!(k > previous);
            assert!(k < 15.0);
            previous = k;
        }
    }

    /// Self-consistency check, not validation: with no oxide the interface must
    /// coincide with the surface and the boundary conductivity must be
    /// untouched, because there is nothing in between.
    #[test]
    fn zero_oxide_leaves_the_boundary_alone() {
        let c = oxide_thermal_coupling(600.0, 620.0, 15.0, 5.0e-5, 0.0);
        assert!((c.blending_factor - 1.0).abs() < 1.0e-9);
        assert!((c.interface_temperature - 600.0).abs() < 1.0e-6);
        assert!((c.boundary_conductivity - 15.0).abs() < 1.0e-6);
    }

    /// Self-consistency check, not validation: the interface temperature must
    /// stay bracketed by the two temperatures it blends, and must move
    /// monotonically towards the metal cell as the oxide thickens. This is the
    /// self-reinforcing part of the corrosion loop, so its sign matters.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// Surface 600 K, first cell 620 K, `k_metal = 15` W/(m·K), cell-to-face
    /// distance 50 µm:
    ///
    /// | oxide \[µm\] | β \[-\] | `T_interface` \[K\] | `k_boundary` \[W/(m·K)\] |
    /// |---|---|---|---|
    /// | 0 | 1.000000 | 600.000 | 15.000 |
    /// | 10 | 0.239274 | 615.214 | 3.589 |
    /// | 30 | 0.094895 | 618.102 | 1.423 |
    /// | 60 | 0.049811 | 619.004 | 0.747 |
    /// | 100 | 0.030494 | 619.390 | 0.457 |
    ///
    /// A 100 µm layer raises the interface by **19.39 K** over the bare case,
    /// nearly the whole 20 K available across this wall — because the oxide's
    /// resistance so far exceeds the 50 µm metal half-cell's that essentially
    /// the entire drop moves into the oxide. Since the sub-transition kinetics
    /// are Arrhenius with `Q1/R = 16266` K, that 19.39 K speeds growth by
    /// `exp(16266·(1/600 − 1/619.39)) = 2.34`. **That factor of 2.34 is the
    /// self-reinforcing feedback** the module documentation describes, and its
    /// sign is what this test exists to protect.
    ///
    /// The numbers depend on `cell_to_face_distance`: a coarser near-wall cell
    /// has more metal resistance and so a larger β. They are recorded for this
    /// geometry as a regression baseline, not as a property of real cladding.
    #[test]
    fn thicker_oxide_pushes_the_interface_towards_the_metal() {
        let surface = 600.0;
        let cell = 620.0;
        let mut previous_beta = f64::INFINITY;
        let mut previous_interface = -f64::INFINITY;

        for micron in [0.0, 10.0, 30.0, 60.0, 100.0] {
            let c = oxide_thermal_coupling(surface, cell, 15.0, 5.0e-5, micron * 1.0e-6);
            assert!(
                c.interface_temperature >= surface - 1.0e-9
                    && c.interface_temperature <= cell + 1.0e-9,
                "{micron} um: interface {} escaped [{surface}, {cell}]",
                c.interface_temperature
            );
            assert!((0.0..=1.0).contains(&c.blending_factor));
            assert!(c.blending_factor <= previous_beta);
            assert!(c.interface_temperature >= previous_interface);
            assert!(c.boundary_conductivity <= 15.0 + 1.0e-12);
            // The two are the same statement, expressed twice by upstream.
            assert!((c.boundary_conductivity - c.blending_factor * 15.0).abs() < 1.0e-12);
            previous_beta = c.blending_factor;
            previous_interface = c.interface_temperature;
        }

        // The recorded table.
        for (micron, beta, interface, k_boundary) in [
            (10.0, 0.239_274, 615.214, 3.589),
            (30.0, 0.094_895, 618.102, 1.423),
            (60.0, 0.049_811, 619.004, 0.747),
            (100.0, 0.030_494, 619.390, 0.457),
        ] {
            let c = oxide_thermal_coupling(surface, cell, 15.0, 5.0e-5, micron * 1.0e-6);
            assert!(
                (c.blending_factor - beta).abs() < 1.0e-5,
                "{micron} um: beta {} vs recorded {beta}",
                c.blending_factor
            );
            assert!(
                (c.interface_temperature - interface).abs() < 1.0e-2,
                "{micron} um: T_i {} vs recorded {interface}",
                c.interface_temperature
            );
            assert!(
                (c.boundary_conductivity - k_boundary).abs() < 1.0e-2,
                "{micron} um: k_b {} vs recorded {k_boundary}",
                c.boundary_conductivity
            );
        }

        // The recorded Arrhenius feedback factor at 100 um.
        let hot = oxide_thermal_coupling(surface, cell, 15.0, 5.0e-5, 1.0e-4);
        let speedup =
            (16_266.103_059_581_319 * (1.0 / surface - 1.0 / hot.interface_temperature)).exp();
        assert!(
            (speedup - 2.3366).abs() < 1.0e-3,
            "recorded corrosion feedback drifted: {speedup}x"
        );
    }

    /// Self-consistency check against the closed form: `β` is the series
    /// combination of the two conductances, exactly.
    #[test]
    fn blending_factor_is_the_series_conductance_ratio() {
        let surface = 640.0;
        let distance = 8.0e-5;
        let k_metal = 14.0;
        let thickness = 4.5e-5;

        let c = oxide_thermal_coupling(surface, 660.0, k_metal, distance, thickness);
        let alpha_oxide = oxide_conductivity(surface) / thickness;
        let alpha_metal = k_metal / distance;
        let expected = alpha_oxide / (alpha_oxide + alpha_metal);
        assert!((c.blending_factor - expected).abs() < 1.0e-15);
        assert!(
            (c.interface_temperature - (expected * surface + (1.0 - expected) * 660.0)).abs()
                < 1.0e-12
        );
    }

    /// Degenerate geometry is reported as "no oxide effect" rather than as a
    /// `NaN` or an infinity. This guard is this port's; upstream has none.
    #[test]
    fn degenerate_geometry_degrades_safely() {
        for distance in [0.0, -1.0e-5, f64::NAN] {
            let c = oxide_thermal_coupling(600.0, 620.0, 15.0, distance, 5.0e-5);
            assert_eq!(c.interface_temperature, 600.0);
            assert_eq!(c.blending_factor, 1.0);
            assert!(c.boundary_conductivity.is_finite());
        }
        let c = oxide_thermal_coupling(600.0, 620.0, -3.0, 5.0e-5, 5.0e-5);
        assert_eq!(c.boundary_conductivity, 0.0);
        assert_eq!(c.interface_temperature, 600.0);

        // A perfectly insulating metal cell still gives finite numbers.
        let c = oxide_thermal_coupling(600.0, 620.0, 0.0, 5.0e-5, 5.0e-5);
        assert!(c.interface_temperature.is_finite());
        assert_eq!(c.blending_factor, 1.0);
    }
}
