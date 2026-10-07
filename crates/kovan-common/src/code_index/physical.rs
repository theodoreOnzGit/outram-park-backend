//! **Physical interface** of a function (asked by the #765 engine for the
//! review wizard's units question, 2026-10-07): `true` when a physical
//! quantity crosses the function's signature, so the question applies
//! deterministically.
//!
//! Decided from the signature's tokens only (cheap, no type checking):
//!
//! - the path segment `uom`, or `Quantity` (uom's generic quantity type);
//! - a quantity name of `uom::si` ([`UOM_QUANTITIES`]: `Length`, `Pressure`,
//!   `ThermodynamicTemperature`, …);
//! - a **workspace alias** of any of those: `type X = …;` whose right-hand
//!   side names one, found by [`QuantityNames::learn`] over the sources
//!   indexed, to a fixed point (an alias of an alias counts).
//!
//! Known limits, by design of a token test: a workspace type that happens
//! to share a uom quantity's name (a `struct Length`) counts as physical; a
//! quantity wrapped in a struct the signature names only by the struct's
//! name does not. The signature is the tokens from `fn` to the body's `{`
//! (or the `;` of a declaration), parameters and return type included.

use std::collections::BTreeSet;

/// The quantity type names of `uom::si` (uom 0.36), plus `Quantity`.
pub const UOM_QUANTITIES: &[&str] = &[
    "Quantity", "AbsorbedDose", "Acceleration", "Action", "AmountOfSubstance", "Angle",
    "AngularAcceleration", "AngularJerk", "AngularVelocity", "Area", "ArealDensityOfStates",
    "ArealMassDensity", "ArealNumberDensity", "ArealNumberRate", "AvailableEnergy", "Capacitance",
    "CatalyticActivity", "CatalyticActivityConcentration", "Curvature", "DiffusionCoefficient",
    "DynamicViscosity", "ElectricCharge", "ElectricChargeArealDensity", "ElectricChargeLinearDensity",
    "ElectricChargeVolumetricDensity", "ElectricCurrent", "ElectricCurrentDensity",
    "ElectricDipoleMoment", "ElectricDisplacementField", "ElectricField", "ElectricFlux",
    "ElectricPermittivity", "ElectricPotential", "ElectricQuadrupoleMoment", "ElectricalConductance",
    "ElectricalConductivity", "ElectricalMobility", "ElectricalResistance", "ElectricalResistivity",
    "Energy", "Force", "Frequency", "FrequencyDrift", "HeatCapacity", "HeatFluxDensity",
    "HeatTransfer", "Inductance", "Information", "InformationRate", "InverseVelocity", "Jerk",
    "Length", "LinearDensityOfStates", "LinearMassDensity", "LinearNumberDensity",
    "LinearNumberRate", "LinearPowerDensity", "Luminance", "LuminousIntensity", "MagneticFieldStrength",
    "MagneticFlux", "MagneticFluxDensity", "MagneticMoment", "MagneticPermeability", "Mass",
    "MassConcentration", "MassDensity", "MassFlux", "MassPerEnergy", "MassRate", "Molality",
    "MolarConcentration", "MolarEnergy", "MolarFlux", "MolarHeatCapacity", "MolarMass", "MolarRadioactivity",
    "MolarVolume", "MomentOfInertia", "Momentum", "Power", "PowerRate", "Pressure", "RadiantExposure",
    "Radioactivity", "Ratio", "ReciprocalLength", "SolidAngle", "SpecificArea", "SpecificHeatCapacity",
    "SpecificPower", "SpecificRadioactivity", "SpecificVolume", "SurfaceElectricCurrentDensity",
    "TemperatureCoefficient", "TemperatureGradient", "TemperatureInterval", "ThermalConductance",
    "ThermalConductivity", "ThermalResistance", "ThermodynamicTemperature", "Time", "Torque",
    "Velocity", "Volume", "VolumeRate", "VolumetricDensityOfStates", "VolumetricHeatCapacity",
    "VolumetricNumberDensity", "VolumetricNumberRate", "VolumetricPowerDensity",
];

/// The names that make a signature physical: uom's and the workspace's
/// aliases of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantityNames {
    names: BTreeSet<String>,
}

impl Default for QuantityNames {
    fn default() -> Self {
        QuantityNames {
            names: UOM_QUANTITIES.iter().map(|s| s.to_string()).collect(),
        }
    }
}

fn idents(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|s| !s.is_empty())
}

impl QuantityNames {
    /// Whether `name` is a quantity name.
    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// Learn the `type X = …;` aliases of quantities in `sources`, to a
    /// fixed point.
    pub fn learn<'s>(&mut self, sources: impl Iterator<Item = &'s str> + Clone) {
        let mut aliases: Vec<(String, String)> = Vec::new();
        for s in sources {
            // `[pub[(…)]] type Name[<…>] = rhs;`, the rhs possibly spanning lines.
            let mut rest = s;
            while let Some(k) = rest.find("type ") {
                let line_start = rest[..k].rfind('\n').map_or(0, |n| n + 1);
                // Top-level items only (column 0): an associated type in an
                // impl (`type Output = Pressure;`) must not make `Output` a
                // quantity name.
                let lead = &rest[line_start..k];
                let decl_ok = lead.is_empty() || (lead.starts_with("pub") && lead.trim_end() == lead.trim_end().trim_start());
                let after = &rest[k + 5..];
                let end = after.find(';').unwrap_or(after.len());
                if decl_ok {
                    if let Some((lhs, rhs)) = after[..end].split_once('=') {
                        if let Some(name) = idents(lhs).next() {
                            aliases.push((name.to_string(), rhs.to_string()));
                        }
                    }
                }
                rest = &after[end.min(after.len())..];
            }
        }
        loop {
            let before = self.names.len();
            for (name, rhs) in &aliases {
                if !self.names.contains(name) && (rhs.contains("uom") || idents(rhs).any(|i| self.names.contains(i))) {
                    self.names.insert(name.clone());
                }
            }
            if self.names.len() == before {
                break;
            }
        }
    }

    /// Whether the function whose normalised code text (the hasher's
    /// space-separated tokens) is `code` has a physical interface.
    pub fn is_physical(&self, code: &str) -> bool {
        let toks: Vec<&str> = code.split(' ').collect();
        let Some(start) = toks.iter().position(|t| *t == "fn") else {
            return false;
        };
        toks[start..]
            .iter()
            .take_while(|t| **t != "{" && **t != ";")
            .any(|t| *t == "uom" || self.names.contains(*t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::hash::hash_functions;

    /// Methodology: signatures with a uom quantity, a uom path, a workspace
    /// alias (and an alias of an alias), a quantity only in the body, and
    /// plain `f64`s, through the real hasher's token text.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn physical_interfaces_are_read_from_signature_tokens() {
        let src = "use uom::si::f64::*;
pub type HeatFlux = HeatFluxDensity;
pub type Flux2 = HeatFlux;
type Plain = f64;
pub fn a(p: Pressure) -> f64 { 0.0 }
pub fn b(x: f64) -> uom::si::f64::Length { todo!() }
pub fn c(q: Flux2) {}
pub fn d(x: f64) -> f64 { let _t = Time::default(); x }
pub fn e(x: Plain) -> Plain { x }
impl std::ops::Neg for W {
    type Output = Pressure;
    fn neg(self) -> Output { todo!() }
}
";
        let mut q = QuantityNames::default();
        q.learn(std::iter::once(src));
        assert!(q.contains("HeatFlux") && q.contains("Flux2") && !q.contains("Plain"));
        assert!(!q.contains("Output"));
        let got: Vec<(String, bool)> = hash_functions(src)
            .unwrap()
            .into_iter()
            .map(|h| (h.entry.name.clone(), q.is_physical(&h.entry.code)))
            .collect();
        let want: Vec<(String, bool)> = [("a", true), ("b", true), ("c", true), ("d", false), ("e", false), ("neg", false)]
            .iter()
            .map(|(n, b)| (n.to_string(), *b))
            .collect();
        assert_eq!(got, want);
    }
}
