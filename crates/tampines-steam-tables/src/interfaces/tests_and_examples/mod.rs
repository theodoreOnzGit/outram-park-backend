/// these are tests to check the functionality
/// of ph flash regions
pub mod ph_flash_regions;

/// V&V + regression tests for two (p,h)-flash edge cases that used to
/// todo!()-panic: the p_sat(273.15 K) triple-point-pressure trap, and the
/// ~~deliberately-unsupported~~ Region 5 (p,h) flash. **CORRECTED 2026-09-27** —
/// the Region 5 `(p,h)` flash is **supported** since commit `2ab91fefc3`
/// (2026-09-14), by an in-house Chebyshev fit rather than an IAPWS backward
/// equation (IAPWS publishes none); that module's test asserts the round trip,
/// not a panic. Verified by running `cargo test --release -j 3
/// -p tampines-steam-tables --lib ph_flash_region4_edge_and_region5`.
/// See the module doc comment.
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

/// aims to reproduce steam tables using the `(rho,h)` flash, and to pin the
/// steam-quality convention that flash reports outside the two-phase dome
#[cfg(test)]
pub mod rho_h_flash_steam_table;

/// aims to reproduce Region 5 `(p,h)`, `(p,s)` and `(h,s)` flashing by round
/// trip against the Region 5 forward equations — IAPWS publishes no backward
/// equations there, so there is no table to compare against
#[cfg(test)]
pub mod region_5_flash_steam_table;
