//! # Solid material property database
//!
//! Temperature-dependent thermophysical property correlations for the solid
//! materials used in the CIET / FHR thermal-hydraulics models: structural
//! metals, insulation, and heating-element candidates.
//!
//! Each submodule bundles the correlations for one material — mass density
//! (kg/m^3), specific heat capacity (J/(kg·K)), thermal conductivity
//! (W/(m·K)), specific enthalpy (J/kg), surface roughness (m), and the inverse
//! specific-enthalpy -> temperature map — together with that material's coded
//! validity temperature range and its literature source. All inputs and
//! outputs are `uom` dimensioned quantities.
//!
//! Active submodules: [`ss_304_l`] (SS-304L stainless steel, Zou/Zweibaum
//! lineage, 250-1000 K), [`ss_304_l_high_temp`] (the same alloy on the Kim
//! ANL-75-55 lineage, 300-1700 K, for HTGR work), [`copper`],
//! [`fiberglass`], [`pyrogel_hps`] (silica-aerogel insulation),
//! [`nuclear_graphite`] (HTR-10 / HTR-PM A3 pebble-matrix graphite and
//! IG-110 reflector graphite), and [`custom_solid_material`] (user-supplied
//! correlations). The `fecral` and `generic_heating_element` modules are
//! experimental scaffolding and are currently commented out (not part of the
//! build).

/// stainless steel 304L
pub mod ss_304_l;

/// stainless steel 304L, high-temperature correlation set (300 K to 1700 K)
///
/// Kim, C. S. (1975). Thermophysical Properties of Stainless Steels.
/// ANL-75-55, Argonne National Laboratory. Open tier (US Government work,
/// public domain), catalogued in the KOVAN archive. Extends the envelope
/// beyond the 1000 K ceiling of the Zou/Zweibaum correlations in [`ss_304_l`],
/// for HTGR / HTR-10 component modelling. Does not replace [`ss_304_l`].
pub mod ss_304_l_high_temp;

/// copper
pub mod copper;

/// fiberglass
pub mod fiberglass;

/// custom material for solid
pub mod custom_solid_material;

/// nuclear graphite (HTR-10 / HTR-PM A3 pebble matrix, and IG-110)
///
/// Correlations transcribed from the openly licensed Virtual Test Bed decks
/// vendored under `reference-data/virtual_test_bed/` (CC-BY-4.0, Open tier),
/// with cp cited to Butland & Maddison, J. Nucl. Mater. 49 (1973/74) 45-56.
pub mod nuclear_graphite;

/// pyrogel hps
///
/// This is an aerogel with silica fibres.
///
/// Most information comes from:
///
/// Kovács, Z., Csík, A., & Lakatos, Á. (2023).
/// Thermal stability investigations of different
/// aerogel insulation materials at elevated temperature.
/// Thermal Science and Engineering Progress, 42, 101906.
pub mod pyrogel_hps;

// standby code for heating elements and radiative heaters

///// generic heating element for
///// heater, based roughly on tungsten
//#[cfg(test)]
//pub mod generic_heating_element;
//
///// FeCrAl, used as a heating element or for alloys in LWR
//pub mod fecral;
