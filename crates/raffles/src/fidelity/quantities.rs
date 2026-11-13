// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-11-04 for GitHub issue #1194 (epic #1177)
// at the WORKSPACE MAINTAINER's direction; the crate owner's (Adolphus Lye's)
// review is outstanding — see this crate's CLAUDE.md, "Scope boundaries".

//! The unit layer: named `uom` quantities in a fixed order, mapped one to one
//! onto SI base values.
//!
//! - [`SiQuantity`] is implemented once, for every `uom::si::Quantity<_, _,
//!   f64>`: its SI base value is the quantity's own `value` field (uom stores
//!   every quantity in the base units of the system), and its unit symbol is
//!   composed from the dimension's base-unit exponents.
//! - [`Quantities`] is a whole record of them (an input `x`, an output `y`,
//!   a parameter vector `θ`), and [`quantities!`](crate::quantities) derives it
//!   for a struct of uom fields so no caller writes the mapping by hand.
//!
//! **Exactness.** `to_si` copies `value` out and `from_si` copies it back in,
//! with no conversion factor in between, so `from_si(to_si(q)) == q` bit for
//! bit whatever unit `q` was built in (mm, °C, bar). The conversion from the
//! caller's unit to SI happens once, inside uom, when the quantity is built.
//! Temperature is the absolute `ThermodynamicTemperature`, stored in kelvin.

use core::marker::PhantomData;

use crate::{RafflesError, Result};

/// One channel of a [`Quantities`] record: the field's name and the SI unit
/// symbol of its value as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    /// The field name, as written in the struct.
    pub name: String,
    /// SI base-unit symbol of the stored value, composed from the dimension's
    /// exponents, e.g. `"m kg s^-3 K^-1"` for a thermal conductivity
    /// (W/(m K)), `"K"` for a temperature and `"1"` for a ratio.
    pub unit: String,
}

impl Channel {
    /// The channel named `name` carrying quantity type `Q`.
    #[must_use]
    pub fn of<Q: SiQuantity>(name: &str) -> Self {
        Self {
            name: name.to_string(),
            unit: Q::si_unit(),
        }
    }
}

/// A single physical quantity whose SI base value can be read and written
/// exactly. Implemented for every `uom::si::Quantity<D, U, f64>`.
pub trait SiQuantity: Copy {
    /// The value in SI base units (uom's stored `value`).
    fn si_value(&self) -> f64;
    /// The quantity whose SI base value is `value`.
    fn from_si_value(value: f64) -> Self;
    /// The SI base-unit symbol (see [`Channel::unit`]).
    fn si_unit() -> String;
}

impl<D, U> SiQuantity for uom::si::Quantity<D, U, f64>
where
    D: uom::si::Dimension + ?Sized,
    U: uom::si::Units<f64> + ?Sized,
{
    fn si_value(&self) -> f64 {
        self.value
    }

    fn from_si_value(value: f64) -> Self {
        uom::si::Quantity {
            dimension: PhantomData,
            units: PhantomData,
            value,
        }
    }

    fn si_unit() -> String {
        use uom::typenum::Integer;
        si_symbol(&[
            ("m", <D::L as Integer>::to_i32()),
            ("kg", <D::M as Integer>::to_i32()),
            ("s", <D::T as Integer>::to_i32()),
            ("A", <D::I as Integer>::to_i32()),
            ("K", <D::Th as Integer>::to_i32()),
            ("mol", <D::N as Integer>::to_i32()),
            ("cd", <D::J as Integer>::to_i32()),
        ])
    }
}

/// `"m kg s^-3 K^-1"` from `[("m", 1), ("kg", 1), ("s", -3), ..]`; `"1"`
/// when every exponent is zero.
fn si_symbol(exponents: &[(&str, i32)]) -> String {
    let parts: Vec<String> = exponents
        .iter()
        .filter(|(_, e)| *e != 0)
        .map(|(s, e)| {
            if *e == 1 {
                (*s).to_string()
            } else {
                format!("{s}^{e}")
            }
        })
        .collect();
    if parts.is_empty() {
        "1".to_string()
    } else {
        parts.join(" ")
    }
}

/// A fixed set of named uom quantities that maps one to one onto SI base
/// values: an input `x`, an output `y` or a parameter vector `θ`.
///
/// Derive it with [`quantities!`](crate::quantities) rather than by hand.
pub trait Quantities: Sized + Clone {
    /// Name and SI unit of each channel, in the fixed order of
    /// [`to_si`](Self::to_si).
    fn channels() -> Vec<Channel>;

    /// Each field's SI base value, in channel order.
    fn to_si(&self) -> Vec<f64>;

    /// The record whose SI base values are `values`, in channel order.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if `values` does not have one entry
    /// per channel.
    fn from_si(values: &[f64]) -> Result<Self>;

    /// Number of channels.
    fn width() -> usize {
        Self::channels().len()
    }
}

/// Checks a slice handed to [`Quantities::from_si`] has `expected` entries.
/// Used by the [`quantities!`](crate::quantities) expansion.
///
/// # Errors
///
/// [`RafflesError::DimensionMismatch`] otherwise.
pub fn check_width(values: &[f64], expected: usize) -> Result<()> {
    if values.len() == expected {
        Ok(())
    } else {
        Err(RafflesError::DimensionMismatch {
            expected,
            found: values.len(),
        })
    }
}

/// Defines a struct of uom fields and derives [`Quantities`] for it.
///
/// Attributes (derives, doc comments) pass through to the struct and its
/// fields; the struct must be `Clone` (derive it). Every field type must be a
/// `uom::si::Quantity<_, _, f64>`, e.g. `uom::si::f64::Length` or a named
/// alias of one.
///
/// ```
/// use uom::si::f64::{Length, ThermodynamicTemperature};
/// use uom::si::length::millimeter;
/// use uom::si::thermodynamic_temperature::degree_celsius;
/// use raffles::fidelity::Quantities;
///
/// raffles::quantities! {
///     /// A pipe at a temperature.
///     #[derive(Debug, Clone, Copy, PartialEq)]
///     pub struct Pipe {
///         /// Bore.
///         pub diameter: Length,
///         /// Wall temperature (absolute).
///         pub wall: ThermodynamicTemperature,
///     }
/// }
///
/// let p = Pipe {
///     diameter: Length::new::<millimeter>(25.0),
///     wall: ThermodynamicTemperature::new::<degree_celsius>(80.0),
/// };
/// let si = p.to_si(); // metres and kelvin
/// assert_eq!(Pipe::from_si(&si)?, p);
/// assert_eq!(Pipe::channels()[1].unit, "K");
/// # Ok::<(), raffles::RafflesError>(())
/// ```
#[macro_export]
macro_rules! quantities {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $( $(#[$fmeta:meta])* $fvis:vis $field:ident : $ty:ty ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $( $(#[$fmeta])* $fvis $field: $ty ),+
        }

        impl $crate::fidelity::Quantities for $name {
            fn channels() -> ::std::vec::Vec<$crate::fidelity::Channel> {
                ::std::vec![ $( $crate::fidelity::Channel::of::<$ty>(::core::stringify!($field)) ),+ ]
            }

            fn to_si(&self) -> ::std::vec::Vec<f64> {
                ::std::vec![ $( $crate::fidelity::SiQuantity::si_value(&self.$field) ),+ ]
            }

            fn from_si(values: &[f64]) -> $crate::Result<Self> {
                let names: &[&str] = &[ $( ::core::stringify!($field) ),+ ];
                $crate::fidelity::check_width(values, names.len())?;
                let mut it = values.iter().copied();
                // `check_width` guarantees one value per field, so the
                // fallback never fires; it exists only to avoid a panic path.
                ::core::result::Result::Ok(Self {
                    $( $field: <$ty as $crate::fidelity::SiQuantity>::from_si_value(it.next().unwrap_or(f64::NAN)) ),+
                })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{Length, Pressure, Ratio, ThermalConductivity, ThermodynamicTemperature};
    use uom::si::length::{meter, millimeter};
    use uom::si::pressure::{bar, pascal};
    use uom::si::ratio::ratio;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;
    use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

    crate::quantities! {
        /// A test record in deliberately non-SI units.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Probe {
            /// A bore.
            pub bore: Length,
            /// An absolute temperature.
            pub temperature: ThermodynamicTemperature,
            /// A pressure.
            pub pressure: Pressure,
            /// A conductivity.
            pub k: ThermalConductivity,
            /// A dimensionless number.
            pub re: Ratio,
        }
    }

    fn probe() -> Probe {
        Probe {
            bore: Length::new::<millimeter>(12.7),
            temperature: ThermodynamicTemperature::new::<degree_celsius>(-17.3),
            pressure: Pressure::new::<bar>(1.013_25),
            k: ThermalConductivity::new::<watt_per_meter_kelvin>(16.2),
            re: Ratio::new::<ratio>(2300.0),
        }
    }

    /// **Methodology — the unit round trip is exact.** A record built in mm,
    /// °C and bar goes through `to_si` / `from_si` and must come back equal
    /// field for field, bit for bit (`f64::to_bits`), and `to_si` must equal
    /// uom's own SI reading of each field (`get::<meter>()`, `get::<kelvin>()`
    /// on the absolute temperature, `get::<pascal>()`).
    ///
    /// **Prediction:** exact (no arithmetic between the two). **Result
    /// (2026-11-04):** exact (`to_bits`) on all five
    /// channels.
    #[test]
    fn non_si_construction_round_trips_exactly() {
        let p = probe();
        let si = p.to_si();
        assert_eq!(si.len(), 5);
        assert_eq!(si[0].to_bits(), p.bore.get::<meter>().to_bits());
        assert_eq!(si[1].to_bits(), p.temperature.get::<kelvin>().to_bits());
        assert_eq!(si[2].to_bits(), p.pressure.get::<pascal>().to_bits());
        assert_eq!(si[3], 16.2);
        assert_eq!(si[4], 2300.0);
        // Absolute temperature: -17.3 °C is 255.85 K, not -17.3.
        assert!((si[1] - 255.85).abs() < 1e-12, "{}", si[1]);
        let back = Probe::from_si(&si).unwrap();
        assert_eq!(back, p);
        assert_eq!(
            back.to_si().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            si.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(Probe::width(), 5);
    }

    /// The channel table: names in field order, SI symbols from the
    /// dimension exponents.
    #[test]
    fn channels_name_the_fields_and_their_si_units() {
        let c = Probe::channels();
        let names: Vec<&str> = c.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["bore", "temperature", "pressure", "k", "re"]);
        let units: Vec<&str> = c.iter().map(|c| c.unit.as_str()).collect();
        assert_eq!(units, ["m", "K", "m^-1 kg s^-2", "m kg s^-3 K^-1", "1"]);
        assert_eq!(
            Channel::of::<Length>("x"),
            Channel {
                name: "x".into(),
                unit: "m".into()
            }
        );
    }

    /// A slice of the wrong width is refused, not padded or truncated.
    #[test]
    fn wrong_width_is_refused() {
        assert_eq!(
            Probe::from_si(&[1.0; 4]),
            Err(RafflesError::DimensionMismatch {
                expected: 5,
                found: 4
            })
        );
        assert!(Probe::from_si(&[1.0; 6]).is_err());
        assert!(check_width(&[], 0).is_ok());
    }
}
