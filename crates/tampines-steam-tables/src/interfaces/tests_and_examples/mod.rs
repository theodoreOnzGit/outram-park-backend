
/// these are tests to check the functionality
/// of ph flash regions
pub mod ph_flash_regions;

/// V&V + regression tests for two (p,h)-flash edge cases that used to
/// todo!()-panic: the p_sat(273.15 K) triple-point-pressure trap, and the
/// deliberately-unsupported Region 5 (p,h) flash. See the module doc comment.
#[cfg(test)]
pub mod ph_flash_region4_edge_and_region5;

/// these are tests to check the functionality 
/// of hs flash regions
/// note: does not include out of bounds just yet..
pub mod hs_flash_regions;

/// aims to reproduce steam tables using ph flash
#[cfg(test)]
pub mod ph_flash_steam_table;

/// aims to reproduce steam tables using pt flash
#[cfg(test)]
pub mod pt_flash_steam_table;
/// aims to reproduce steam tables using ps flash
#[cfg(test)]
pub mod ps_flash_steam_table;
///// aims to reproduce steam tables using hs flash
#[cfg(test)]
pub mod hs_flash_steam_table;


