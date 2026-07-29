// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream `offbeatLib/corrosion/` and
// `offbeatLib/accelerationSchemes/`:
//   corrosion.{C,H}                              -> CorrosionModel::Constant
//   corrosionByPatch.{C,H}                       -> (driver; not ported, see below)
//   corrosionModel/corrosionModel.{C,H}          -> CorrosionModel::Constant
//   corrosionModel/zircaloyOuterCorrosion.{C,H}  -> CorrosionModel::ZircaloyOuter
//                                                   + `thermal`
//   corrosionModel/oxidationKineticsModel/*      -> `kinetics`
//   layerAdditionRemovalPolyTopoChanger/*        -> DEFERRED, see below
//   accelerationSchemes/*                        -> `acceleration`
// and, for the hydrogen-pickup coupling only,
//   fvPatchFields/zeroCurrent/oxidePickupFractionFvPatchScalarField.{C,H}
//                                                -> `hydrogen`
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Cladding waterside corrosion, hydrogen pickup, and nonlinear-solver
//! acceleration.
//!
//! # What waterside corrosion is, for a reader with no fuel-performance
//! background
//!
//! A light-water reactor fuel rod is a thin zirconium-alloy (Zircaloy) tube
//! holding a stack of uranium-dioxide pellets. Its outside is bathed in hot
//! water — roughly 560–620 K at 15.5 MPa in a PWR — and zirconium is
//! thermodynamically unstable in water. It oxidises:
//!
//! ```text
//! Zr + 2 H2O  ->  ZrO2 + 2 H2
//! ```
//!
//! A layer of zirconia (ZrO2) therefore grows on the outer surface over the
//! rod's four-to-six-year life. It is not a cosmetic effect; it matters for
//! three separate reasons, and this module computes all three.
//!
//! 1. **The oxide is a thermal insulator.** ZrO2 conducts about 2 W/(m·K)
//!    against Zircaloy's ~15 W/(m·K), so every micron of oxide adds thermal
//!    resistance between the fuel and the coolant and pushes the whole rod
//!    hotter. Hotter metal oxidises faster, so the effect is self-reinforcing.
//!    See [`thermal`].
//! 2. **It eats load-bearing wall.** The metal consumed to make the oxide is
//!    gone from the pressure boundary. Because ZrO2 occupies more volume than
//!    the zirconium it came from — the **Pilling–Bedworth ratio**, 1.56 for
//!    Zr — a layer of oxide `S` thick has consumed only `S/1.56` of metal.
//!    See [`CorrosionModel::metal_loss`].
//! 3. **Some of the hydrogen goes into the metal.** The reaction above
//!    liberates two H2 per zirconium atom. Most of it leaves with the coolant,
//!    but a *pickup fraction* — 15–25% depending on the alloy — dissolves into
//!    the cladding, where above its solubility limit it precipitates as
//!    zirconium hydride platelets. Hydrides are brittle, so hydrogen pickup is
//!    the mechanism by which corrosion turns into a **cladding-failure**
//!    problem rather than merely a heat-transfer one. See [`hydrogen`].
//!
//! # Sub-transition and post-transition kinetics
//!
//! Oxide growth is not a single power law. While the layer is thin it is dense
//! and adherent, and it is itself the diffusion barrier that limits further
//! oxidation, so growth *decelerates* — approximately cubic in time,
//! `S^3 ∝ t`. At a **transition thickness** of about 2 µm the layer cracks and
//! develops interconnected porosity, the diffusion barrier stops thickening in
//! any useful sense, and growth becomes approximately **linear** in time and
//! much faster. Every model in [`kinetics`] has this two-regime structure, and
//! the acceleration at transition is the single most important qualitative
//! feature to get right.
//!
//! At accident temperatures (above ~673 K) the mechanism changes again to fast
//! **parabolic** high-temperature steam oxidation, which is what the
//! [`CathcartPawel`](kinetics::OxidationKinetics::CathcartPawel) branch
//! describes.
//!
//! # What is in this module
//!
//! | Submodule | Contents |
//! |---|---|
//! | [`kinetics`] | [`OxidationKinetics`] — the oxide-growth laws themselves |
//! | [`model`] | [`CorrosionModel`] — a whole patch-level corrosion model |
//! | [`state`] | [`CorrosionState`] / [`CorrosionStep`] — inputs and results of one step |
//! | [`hydrogen`] | [`HydrogenPickupModel`] — hydrogen ingress into the metal |
//! | [`thermal`] | oxide conductivity and the metal/oxide interface temperature |
//! | [`acceleration`] | [`AccelerationScheme`] — Anderson mixing, a general nonlinear-solver accelerator |
//!
//! # Units — raw `f64`, strict SI
//!
//! Everything crossing a public boundary in this module is raw `f64` in strict
//! SI, and every item states its unit. Three conversions are done **once**, at
//! the boundary, rather than being left to each correlation — which is where
//! upstream does them, and where they are easy to get wrong:
//!
//! - **Time is seconds.** Upstream's low-temperature kinetics divides by
//!   `3600*24` internally because its rate constants are quoted per day.
//! - **Fast flux is n/(m²·s).** Upstream's `fastFlux` field is documented as
//!   **n/(cm²·s)** (see `constantFastFlux.H`), and the EPRI/KWU/C-E flux term
//!   is fitted on that basis. This port takes SI and multiplies by `1e-4`
//!   inside the correlation. Getting this wrong changes the post-transition
//!   rate substantially, so it is called out here rather than buried.
//! - **Hydrogen concentration is wt-ppm**, matching
//!   [`MaterialState::hydrogen_content`](crate::materials::MaterialState::hydrogen_content).
//!   This is a mass fraction times 1e6, not an SI unit, and is the unit the
//!   entire hydride literature uses.
//!
//! # What is deliberately NOT ported: the layer addition/removal topology changer
//!
//! Upstream's `corrosion/layerAdditionRemovalPolyTopoChanger/` is **not**
//! translated here, and no stand-in for it is provided.
//!
//! **What it does.** As the oxide grows, the metal wall thins. Upstream
//! represents that by physically moving the mesh: `corrosion::updateMesh()`
//! displaces the boundary points inward by the metal thickness lost
//! (`-DMetalThickness * n_f`), and the topology changer watches the resulting
//! near-wall cell layer. When that layer is squashed below a minimum thickness
//! it **removes** the layer from the mesh; when it is stretched past a maximum
//! it **adds** one. Upstream sets those bounds automatically per patch — a
//! quarter of the initial face-to-cell-centre distance for the minimum, four
//! times it for the maximum — and wraps one OpenFOAM `layerAdditionRemoval`
//! mesh modifier per boundary patch in a `polyTopoChanger`.
//!
//! **Why it is deferred.** It is not a correlation with a mesh-shaped
//! interface; it *is* a mesh operation. Executing it needs a live mutable
//! `polyMesh` — face zones, a `polyTopoChange` engine that can renumber points,
//! faces, cells and boundary patches, and a `mapPolyMesh` to carry every
//! existing field across the renumbering. `outram-foam-basic-lib` provides the
//! finite-volume substrate this crate builds on, but not runtime topology
//! modification. Writing a plausible-looking `add_layer` / `remove_layer` here
//! would produce something that compiles, has no mesh to act on, and could
//! never be tested — the opposite of useful.
//!
//! **What you get instead.** The *kinetics* are ported as pure functions, and
//! [`CorrosionStep::metal_loss`] gives the inward wall displacement in metres
//! that a caller with a real mesh must apply. Wiring that displacement into a
//! moving mesh, and adding or removing layers when cells become degenerate, is
//! left to whoever owns the mesh. This is stated as deferred work rather than
//! quietly stubbed.
//!
//! Two smaller pieces of upstream are also left out, for the same "needs a live
//! mesh" reason: `corrosionByPatch` (the per-patch driver that owns the
//! `oxideThickness`/`DOxideThickness` surface fields, under-relaxes them
//! between outer iterations, and prints the area-averaged summary), and the
//! `oxidePickupFraction` boundary condition's role as an actual finite-volume
//! flux boundary condition. The physics inside both is here; the field
//! plumbing is not.
//!
//! # Status
//!
//! **AI-assisted translation, reviewed by no human.** Per `RESPONSIBLE_USE.md`
//! this is untrusted draft material. The tests in this module establish
//! internal consistency with upstream's algebra and with conservation of the
//! hydrogen the reaction liberates — they are **not** validation against
//! measured oxide-thickness data, and nothing here may be described as
//! validated. Three of them exist specifically to pin **upstream behaviour that
//! this port reproduces deliberately** — two demonstrable arithmetic defects
//! (the Cathcart–Pawel 1800–1900 K interpolation, in [`kinetics`], and the
//! `volFactor` numerator, in [`hydrogen`]) and one surprising-but-intended
//! discontinuity (the 673 K model switch, in [`kinetics`]). Those tests are
//! labelled as such and are not an endorsement.
//!
//! [`OxidationKinetics`]: kinetics::OxidationKinetics
//! [`CorrosionModel`]: model::CorrosionModel
//! [`CorrosionModel::metal_loss`]: model::CorrosionModel::metal_loss
//! [`CorrosionState`]: state::CorrosionState
//! [`CorrosionStep`]: state::CorrosionStep
//! [`CorrosionStep::metal_loss`]: state::CorrosionStep::metal_loss
//! [`HydrogenPickupModel`]: hydrogen::HydrogenPickupModel
//! [`AccelerationScheme`]: acceleration::AccelerationScheme

pub mod acceleration;
pub mod hydrogen;
pub mod kinetics;
pub mod model;
pub mod state;
pub mod thermal;

pub use acceleration::{AccelerationOutcome, AccelerationScheme, AndersonMixing, FixedPointReport};
pub use hydrogen::{HydrogenPickupModel, PickupScaling};
pub use kinetics::OxidationKinetics;
pub use model::CorrosionModel;
pub use state::{CorrosionState, CorrosionStep};
pub use thermal::{oxide_conductivity, oxide_thermal_coupling, OxideThermalCoupling};

/// Pilling–Bedworth ratio of zirconium \[-\] — the volume of oxide formed per
/// unit volume of metal consumed.
///
/// `1.56` is upstream's hard-coded value in `zircaloyOuterCorrosion.C`, and is
/// the standard figure for ZrO2 on Zr. Because it is greater than one, the
/// oxide layer is always thicker than the metal it ate: a 60 µm oxide has
/// consumed 60/1.56 = 38.5 µm of wall.
///
/// Used by [`CorrosionModel::metal_loss`](model::CorrosionModel::metal_loss)
/// and by the hydrogen-pickup model, which needs the *metal* consumed to know
/// how much hydrogen the reaction released.
pub const PILLING_BEDWORTH_ZIRCONIUM: f64 = 1.56;
