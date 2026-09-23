// SPDX-License-Identifier: GPL-3.0
//
// puff port — provenance
// ----------------------
// Upstream project : puff — "Simulate and Visualize the Gaussian Puff Forward
//                    Atmospheric Model" (R package, CRAN)
// Upstream URL     : https://github.com/Hammerling-Research-Group/puff
// Upstream version : 0.1.1, commit 5213d58
// Original licence : MIT (YEAR: 2025, COPYRIGHT HOLDER: Hammerling Research
//                    Group). MIT is GPL-3.0-compatible one-way; this port is
//                    distributed under the crate's GPL-3.0.
// Upstream authors : Teagan Ward, Philip Waggoner, Will Daniels, Meng Jia,
//                    Dorit Hammerling (Colorado School of Mines)
// Reference        : Jia, M., Fish, R., Daniels, W., Sprinkle, B. and
//                    Hammerling, D. (2024), doi:10.26434/chemrxiv-2023-hc95q-v3
// See LICENSE.puff and NOTICE.puff at the crate root. Independent fork; not
// affiliated with or endorsed by the Hammerling Research Group.

//! # `puff` — Gaussian puff forward dispersion
//!
//! A Gaussian *puff* model discretises a continuous release into a train of
//! discrete puffs. Each puff is emitted at a fixed interval, carries a fixed
//! mass, is advected by the wind sampled at its moment of emission, and spreads
//! by a Pasquill–Gifford dispersion coefficient that grows with the distance it
//! has travelled. The concentration at a receptor is the sum over all puffs
//! still alive.
//!
//! This complements [`crate::flexpart`] rather than duplicating it. FLEXPART is
//! a Lagrangian *particle* model driven by gridded meteorology; this is an
//! analytic puff model driven by a single wind time series. The puff model is
//! cheap enough to run interactively over a site-sized domain and needs no
//! meteorological files, which makes it the natural near-field complement to a
//! particle model built for synoptic scales.
//!
//! ## Module map (R → Rust)
//!
//! | Upstream `R/` | Rust | Content |
//! |---|---|---|
//! | `helpers.R` — `is_day`, `get_stab_class` | [`stability`] | Pasquill stability classification |
//! | `helpers.R` — `compute_sigma_vals` | [`dispersion`] | Pasquill–Gifford `sigma_y`, `sigma_z` |
//! | `helpers.R` — `wind_vector_convert`, `interpolate_wind_data` | [`wind`] | Met-convention wind handling |
//! | `helpers.R` — `gpuff` | [`concentration`] | The Gaussian puff kernel itself |
//! | `simulate_sensor_mode.R`, `simulate_grid_mode.R` | [`simulate`] | The two run modes |
//!
//! [`climatology`] has **no upstream** — it is this crate's own, holding the
//! illustrative Singapore wind conditions the examples run at, with their
//! provenance. It is not part of the ported physics and is not covered by the
//! code-to-code verification.
//!
//! ## What is NOT ported
//!
//! **`R/plots.R` (1 077 of upstream's 2 296 lines — 47 % of the package).**
//! Upstream describes itself as "primarily a visualization-focused package";
//! everything in `plots.R` is `ggplot2`/`plotly` chart construction, with no
//! physics. Porting it would contradict two workspace rules at once — non-GUI
//! library code must build for Android and for `wasm32`, and a headless library
//! does not own its caller's plotting. The physics is the whole of the
//! remaining 1 219 lines, and that is what is here.
//!
//! Also not ported: R's `POSIXct` handling. Upstream classifies day/night by
//! formatting a timestamp with `"%H"`; this port takes an hour-of-day integer
//! directly, which is the only part of the timestamp that the physics reads.
//!
//! ## Intended use
//!
//! Research, education and verification/validation only, exactly as for the
//! rest of this crate — see the crate-level documentation, whose scope limits
//! are binding. Upstream's application domain is oil-and-gas methane leak
//! detection; CHANGI's is radionuclide transport. The dispersion mathematics is
//! species-independent, but **the unit conversion is not**: upstream's
//! `gpuff` returns parts-per-million *of methane*. See
//! [`concentration::METHANE_PPM_PER_KG_PER_M3`] for why that factor must not be
//! reused for another species.
//!
//! ## Status
//!
//! **Untrusted AI-assisted draft. No human V&V.** Verified code-to-code against
//! the upstream R at commit `5213d58` — see `docs/puff-code-to-code.md`. That
//! establishes the translation is faithful; it says nothing about whether the
//! model reproduces measured dispersion.

pub mod climatology;
pub mod concentration;
pub mod dispersion;
pub mod simulate;
pub mod stability;
pub mod wind;
