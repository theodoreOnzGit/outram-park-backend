//! # Pebble-bed thermal physics for high-temperature gas-cooled reactors
//!
//! The nested conduction scales of a pebble-bed core (a *doubly
//! heterogeneous* medium — TRISO particles inside pebbles inside a packed
//! bed), being built as one coherent stack under the `op-jyyp` HTR-10 epic.
//!
//! ## The three nested conduction scales
//!
//! Each level's effective property is the next level's input, and each level's
//! temperature is the one below it's boundary condition:
//!
//! - [`triso`] — **level 1**, coated-particle conduction through the five
//!   concentric regions (UO2 kernel, porous carbon buffer, IPyC, SiC, OPyC):
//!   analytic series resistance for concentric shells with volumetric heat
//!   generation confined to the kernel, temperature- and fluence-dependent
//!   layer conductivities from the VTB HTR-PM pebble model, and an effective
//!   particle conductivity for level 2. Geometry is reused from `boon-lay`'s
//!   `TrisoCell` (maintainer-approved dependency edge); `boon-lay`'s
//!   fission-product *release* model is deliberately not consumed.
//! - [`pebble`] — **level 2**, two-zone pebble radial conduction: a fuelled
//!   zone whose effective conductivity comes from level 1 through a
//!   Maxwell-Eucken or Chiew-Glandt dispersion model, inside an unfuelled
//!   graphite shell. The double heterogeneity is kept explicit rather than
//!   homogenised; see the module docs for what homogenising would cost.
//! - [`cht`] — **level 3**, bed-to-helium conjugate heat transfer: the
//!   **correct** Wakao-Funazkri particle-to-fluid Nusselt correlation
//!   `Nu = 2 + 1.1 Pr^(1/3) Re^0.6` on the pebble diameter, the heat transfer
//!   coefficient it implies, and the bed volumetric form via
//!   `a_v = 6(1 - eps)/d`. **Warning, still current:** the TUAS `WakaoData`
//!   implementation has the Re and Pr exponents *swapped* relative to the
//!   published correlation (bead `op-4542`), diverging by a factor of about
//!   5.8 at Re = 1000, Pr = 0.71 — `cht` therefore implements the correlation
//!   independently and must **not** be cross-wired to TUAS until that bead is
//!   resolved.
//!
//! ## Also present
//!
//! - [`zbs`] — Zehner-Bauer-Schlunder packed-bed effective thermal
//!   conductivity: stagnant-gas, solid, particle-contact and thermal
//!   radiation contributions, with a near-wall porosity hook. The
//!   formulation follows the dimensionless form in the van Antwerpen et al.
//!   (2010) review; the transcription has **not** been human-verified
//!   against the printed originals (tracked as `op-qoy4`), though its
//!   analytic limits are test-gated with measured tolerances. See the
//!   module docs for the measured finding (2026-08-11) that the VTB
//!   generic-pbr 18-point reference table is *not* reproduced by ZBS with
//!   helium in the pores — the model sits below the table at all 18 points
//!   (ratio 0.177 at 300 K to 0.644 at 2000 K), tracked as `op-jvua`.
//! - [`feedback`] — the graphite/moderator reactivity channel, held as its
//!   **own** state with its own coefficient and its own lumped thermal-mass
//!   ODE rather than folded into fuel Doppler, because the large graphite mass
//!   is what gives HTR-10 its long thermal time constant and self-limiting
//!   response. **No moderator temperature coefficient is supplied** — it must
//!   come from the caller's neutronics; the published HTR-10 *isothermal*
//!   coefficients are provided separately and clearly labelled as not being
//!   that quantity. Wiring the channel into point kinetics belongs in an
//!   example or in `nee_soon`, not in this library, which stays free of
//!   `teh-o-prke`.
//! - [`temperature_difference`] — the one shared helper, converting a pair of
//!   absolute temperatures into the `uom` `TemperatureInterval` that `uom`
//!   deliberately will not produce with `-`.
//!
//! ## Related but housed elsewhere
//!
//! - KTA packed-bed pressure drop (the friction side of the bed) landed in
//!   [`crate::gas_phase`] as `KtaBed` (2026-08-11), alongside the helium
//!   circuit components it serves. Whether a `pebble_bed::kta` home is also
//!   wanted is the maintainer's call — tracked as `op-afz4`.
//!
//! ## Status
//!
//! **NOT VALIDATED.** Every correlation carries its citation and access
//! tier; nothing here has been compared against HTR-10 measurements.
//! AI-assisted draft pending human review per `RESPONSIBLE_USE.md`.

use uom::si::f64::{TemperatureInterval, ThermodynamicTemperature};
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::kelvin;

pub mod cht;
pub mod feedback;
pub mod pebble;
pub mod triso;
pub mod zbs;

pub use cht::PackedBedConvection;
pub use feedback::GraphiteModeratorFeedback;
pub use pebble::{DispersionModel, Pebble, PebbleTemperatureProfile};
pub use triso::{TrisoLayer, TrisoParticle, TrisoTemperatureProfile};
pub use zbs::ZbsBed;

/// The difference between two absolute temperatures, as a `uom`
/// [`TemperatureInterval`] in kelvin.
///
/// `uom` deliberately refuses to subtract one [`ThermodynamicTemperature`]
/// from another with the `-` operator: absolute temperatures carry a
/// `TemperatureKind` marker, so that `20 degC - 10 degC` cannot be silently
/// mistaken for `10 degC` when it is really a 10 K *interval*. Every module in
/// this stack needs that interval — a conduction temperature rise, a
/// moderator temperature excursion above a reference — so the conversion is
/// written once, here, rather than open-coded per module.
///
/// Returns `hotter - colder` in kelvin; the result is negative if `colder` is
/// in fact the hotter of the two.
pub fn temperature_difference(
    hotter: ThermodynamicTemperature,
    colder: ThermodynamicTemperature,
) -> TemperatureInterval {
    TemperatureInterval::new::<kelvin_interval>(hotter.get::<kelvin>() - colder.get::<kelvin>())
}
