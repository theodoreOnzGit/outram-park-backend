// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
//   `offbeatLib/gapGasModel/{gapGasModel,gapFRAPCON,gapGasTimeTabulated}.{H,C}`
//   `offbeatLib/sliceMapper/*`
//   `offbeatLib/fvPatchFields/temperatureCoupled/{fuelRodGap,trisoGap,resistiveGap}FvPatchScalarField.C`
//   `offbeatLib/fvPatchFields/tractionDisplacement/{contact,gapContact}FvPatchVectorField.C`
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Fuel/cladding gap: gas composition, gap conductance, contact and axial
//! slicing.
//!
//! # What this module is for
//!
//! The gap between the fuel pellet and the cladding is where fuel performance is
//! decided. Heat leaves the pellet by three parallel paths across it:
//!
//! 1. **Conduction through the fill gas** — a helium fill at beginning of life,
//!    progressively diluted by released xenon and krypton, which conduct roughly
//!    twenty times worse.
//! 2. **Radiation** between the two surfaces, which matters only once they are
//!    hot.
//! 3. **Solid conduction through the contact spots**, once thermal expansion,
//!    swelling and creep have closed the gap and the surfaces bear on each other.
//!
//! The resulting gap conductance swings over orders of magnitude through life,
//! and it feeds straight back into the temperature field that drove the closure.
//! Getting the closure logic wrong makes the whole rod history wrong.
//!
//! # Gap conventions — read this before using anything here
//!
//! Upstream OFFBEAT is not uniform about whether a "gap" is a radial or a
//! diametral quantity, and [`crate::materials::behavioral::relocation`] already
//! had to flag one such ambiguity (its `cold_gap` is **diametral**). This module
//! does not reintroduce it. The rules here are absolute:
//!
//! - **Every gap width, roughness, jump distance and radius in this module is
//!   RADIAL** — a surface-to-surface normal separation, not a diameter
//!   difference. If your input deck quotes a diametral gap, halve it before
//!   passing it in.
//! - **The sign convention for a radial gap width differs between the thermal and
//!   the mechanical side, and both are reproduced faithfully:**
//!   - [`conductance`] takes an **unsigned, open-only** radial gap width: `0`
//!     means the surfaces touch, positive means open. Upstream's
//!     `fuelRodGapFvPatchScalarField::gapWidth()` clips at zero
//!     (`max((nbrCf - Cf) & nf, 0)`), so a closed gap carries no information
//!     about *how hard* it is closed — that arrives separately as the interface
//!     pressure.
//!   - [`contact`] takes a **signed** radial gap width: positive is open,
//!     **negative is interpenetration**, which is exactly what the penalty
//!     formulation needs. Upstream's `contactFvPatchVectorField::gapWidth()`
//!     does *not* clip.
//! - **Roughness is a per-surface radial arithmetic-mean roughness \[m\]**, one
//!   value for the fuel surface and one for the cladding surface. Upstream
//!   combines them two different ways inside the same routine (an arithmetic
//!   mean for the empirical exponent, a root-sum-square for the divisor); both
//!   are reproduced.
//! - **Temperatures are surface temperatures**, not bulk or cell-centre
//!   temperatures.
//!
//! # Units
//!
//! Raw `f64` in strict SI throughout, per the crate-level units note: metre,
//! kelvin, pascal, W/m²K for a conductance, W/m/K for a conductivity, kilogram,
//! mole, m³. The **one** deliberate exception is
//! [`gas::GapGasSpecies::molar_mass_g_per_mol`], which is g/mol because that is
//! how upstream tabulates it and how fuel-performance input decks quote it; the
//! SI companion [`gas::GapGasSpecies::molar_mass`] sits right beside it.
//!
//! # Module map
//!
//! | Submodule | What it holds | Upstream origin |
//! |---|---|---|
//! | [`gas`] | Gas species, mass/mole composition, mixture conductivity, accommodation coefficient, fission-gas dilution | `gapGasModel.C`, `gapFRAPCON::kappa/a` |
//! | [`conductance`] | The three parallel heat paths and their sum; the series interface resistance | `fuelRodGap`, `trisoGap`, `resistiveGap` patch fields |
//! | [`contact`] | Penalty contact: interface pressure from interpenetration, boundary stiffness | `contactFvPatchVectorField`, `gapContactFvPatchVectorField` |
//! | [`free_volume`] | Rod free volume and the ideal-gas pressure `p = nR / Σ(Vᵢ/Tᵢ)` | `gapFRAPCON::correct`, `correctDish`, `correctCrack` |
//! | [`mod@slice`] | 1.5D axial slicing (the `sliceMapper` concepts) and volume-weighted slice averaging | `sliceMapper/*` |
//!
//! # What is deferred, and why
//!
//! This port covers the **pure functions** of upstream's gap physics. Several
//! pieces of upstream are not functions of their arguments at all — they are
//! traversals of an OpenFOAM mesh, an AMI (arbitrary mesh interface) between two
//! regions, or the multi-region solver's patch-to-patch coupling. Those are
//! **deferred**, not silently approximated:
//!
//! - **Gap and plenum volume by the Gauss–Green surface integral**
//!   (`gapFRAPCON::correctGap`, `correctHole`, `correctPlena`): upstream computes
//!   `V = ⅓ ∮_S (r_s · n) dS` over the deformed bounding patches. It needs face
//!   centres, face normals and the displacement field on both sides of the gap.
//!   [`free_volume`] takes the resulting per-region volumes and temperatures as
//!   **inputs** and does the thermodynamics; it does not compute them.
//! - **The gap/plenum scaling factors** (`gapFRAPCON::correctScalingFactors`):
//!   upstream builds them by intersecting cutting planes with cladding-patch
//!   edges, precisely because the AMI `weightSum` cannot distinguish "separated
//!   by a gap" from "partially overlapping axially" on a cylinder. This is
//!   irreducibly a mesh-topology algorithm. Deferred.
//! - **AMI interpolation between the fuel-outer and clad-inner patches**, and the
//!   owner/neighbour averaging in `updateCoeffs()`. The *formulae* evaluated on
//!   each face are ported; the interpolation that supplies the neighbour values
//!   is not.
//! - **Cell-to-material addressing in the slice mappers** (`mat_.matAddrList()`,
//!   `isA<fuelMaterial>`, the `sliceID` `volScalarField`, and the parallel
//!   `Pstream` gather/scatter). [`mod@slice`] ports the axial binning arithmetic and
//!   the volume-weighted average, taking cell axial coordinates and volumes as
//!   plain slices.
//! - **Friction and the tangential contact traction** (`contactFvPatchVectorField`'s
//!   slip/stick update). Only the normal penalty pressure is ported.
//!
//! Everything deferred is called out again in the doc comment of the item that
//! would have used it. Nothing here silently substitutes an approximation for a
//! mesh operation.
//!
//! # Status
//!
//! **Untrusted draft material.** Per `RESPONSIBLE_USE.md` this is AI-assisted
//! output that has had no human verification or validation. Tests in this module
//! are labelled either *reference-checked* (against a value stated in upstream's
//! own source) or *self-consistency* (an internal invariant — monotonicity, a
//! limit, an exact reduction). **No test here is a validation against measured
//! fuel-rod data**, and nothing in this module may be described as validated.

pub mod conductance;
pub mod contact;
pub mod free_volume;
pub mod gas;
pub mod slice;

pub use conductance::{GapConductance, GapConductanceModel, GapConductanceScaling, GapSurfaces};
pub use contact::PenaltyContact;
pub use free_volume::{FreeVolumeRegion, GasPressureModel, RodFreeVolume};
pub use gas::{GapGasMixture, GapGasSpecies};
pub use slice::{AxialSlicing, SliceAverage};
