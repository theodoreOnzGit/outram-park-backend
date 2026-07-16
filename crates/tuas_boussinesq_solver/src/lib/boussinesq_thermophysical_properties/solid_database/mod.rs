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
//! Active submodules: [`ss_304_l`] (SS-304L stainless steel), [`copper`],
//! [`fiberglass`], [`pyrogel_hps`] (silica-aerogel insulation), and
//! [`custom_solid_material`] (user-supplied correlations). The `fecral` and
//! `generic_heating_element` modules are experimental scaffolding and are
//! currently commented out (not part of the build).

/// stainless steel 304L
pub mod ss_304_l;

/// copper 
pub mod copper;

/// fiberglass 
pub mod fiberglass;

/// custom material for solid 
pub mod custom_solid_material;


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
