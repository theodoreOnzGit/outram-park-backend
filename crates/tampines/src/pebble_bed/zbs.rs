//! # Zehner-Bauer-Schlunder (ZBS) pebble-bed effective thermal conductivity
//!
//! Analytic effective thermal conductivity of a packed bed of spheres,
//! summing four heat-transfer paths through a unit cell:
//!
//! - conduction through the stagnant gas filling the voids,
//! - conduction through the touching solid spheres,
//! - conduction through the finite flattened *contact areas* between
//!   spheres (the `phi` term), and
//! - thermal radiation between sphere surfaces (the `k_r` term, growing as
//!   `T^3` — the dominant decay-heat path at loss-of-forced-cooling
//!   temperatures; never drop it).
//!
//! ## Homogenisation assumption
//!
//! Everything here treats the bed as a **homogeneous effective medium**: a
//! single scalar conductivity that a continuum energy equation may use in
//! place of resolving individual pebbles, voids and contact points. The
//! correlation returns a volume-averaged quantity, so it says nothing about
//! the pebble-scale temperature field — local peaking at contact points,
//! the pebble-interior radial profile (that is [`super::pebble`]'s job) and
//! the near-wall channelling of a real bed are all averaged out. It is also
//! **isotropic and stagnant**: no directional dependence, and no
//! convective enhancement from through-flow.
//!
//! ## Formulation and provenance
//!
//! The correlation is Bauer & Schlunder's (1978) extension of the
//! Zehner-Schlunder (1970) unit-cell model, in the dimensionless form given
//! in the review of van Antwerpen, du Toit & Rousseau, *Nucl. Eng. Des.*
//! 240 (2010) 1803-1818 (**proprietary tier** — cited and implemented from,
//! not reproduced at length) and presented for gas-cooled reactors in
//! IAEA-TECDOC-1163 (**Open tier**). With the Knudsen (Smoluchowski) factor
//! set to one — valid for helium at reactor pressures, where the molecular
//! mean free path (tens of nanometres) is vanishingly small against a 6 cm
//! pebble — the equations are, with `eps` the porosity, `kappa = k_s/k_f`,
//! `d` the pebble diameter, `e_r` the surface emissivity and `sigma` the
//! Stefan-Boltzmann constant:
//!
//! ```text
//! B    = C * ((1 - eps)/eps)^(10/9)                    (deformation factor)
//! k_r  = 4*sigma*T^3*d / ((2/e_r - 1) * k_f)           (radiation ratio)
//! N    = 1 + (k_r - B)/kappa
//! k_c  = (2/N) * [ B*(kappa + k_r - 1)/(N^2 * kappa) * ln((kappa + k_r)/B)
//!                  + (B + 1)/(2B) * (k_r - B)
//!                  - (B - 1)/N ]
//! k_eff/k_f = (1 - sqrt(1-eps)) * (1 + eps*k_r)
//!             + sqrt(1-eps) * ( phi*kappa + (1 - phi)*k_c )
//! ```
//!
//! `C = 1.25` is the sphere shape factor and `phi = 0.0077` the standard
//! contact-area fraction, both from Bauer & Schlunder as quoted in the van
//! Antwerpen review. At `k_r = 0` the unit-cell term `k_c` reduces exactly
//! to the classic Zehner-Schlunder stagnant-bed form — a limit the test
//! `tests::zero_radiation_degenerates_to_the_classic_zehner_schlunder_form`
//! exercises. (The `tests` module is `#[cfg(test)]`, so the test names
//! quoted throughout these docs are code spans rather than doc links —
//! rustdoc cannot resolve into a test-only module.)
//!
//! ## Verification status (measured 2026-08-11)
//!
//! **Transcription caveat (honesty per `RESPONSIBLE_USE.md`).** The
//! dimensionless form above was implemented without page-level access to
//! the printed originals. The analytic-limit checks in `tests` were
//! written and **run** on 2026-08-11; their measured outcomes are:
//!
//! - **Uniform-medium collapse** (`k_s = k_f` must give `k_eff = k_f`) is
//!   *exact* where the unit cell is defined — maximum relative deviation
//!   `4.4e-16`, i.e. f64 roundoff, over porosities 0.6 and 0.8 at
//!   `k = 0.15`, `1.0` and `26.0` W/(m K). It is **not** defined at the
//!   HTR-10 porosity: `kappa = 1` requires `B < 1`, which for `C = 1.25`
//!   needs `eps` above about 0.55, and at `eps = 0.39` (`B = 2.0548`) the
//!   correlation correctly returns [`TampinesError::Unphysical`] instead
//!   of a number. An earlier revision of these docs claimed this check
//!   passed unconditionally; it does not, and the qualifier above is the
//!   measured behaviour.
//! - **Zehner-Schlunder degeneration** at `k_r -> 0`: agrees with the
//!   classic closed form to `1.398e-16` relative. This is an *algebraic*
//!   cross-check — the same published expression regrouped — so it
//!   verifies the limit and the arithmetic, **not** the transcription of
//!   the source equations.
//! - **Wiener bounds**: the conduction-only result lies strictly between
//!   the series and parallel bounds over a 27-point `(eps, k_s, k_f)`
//!   grid; the tightest margins measured were `k_eff/k_series = 2.3754`
//!   and `k_eff/k_parallel = 0.3721`. Radiation legitimately pushes the
//!   *full* result above the conduction-only parallel bound at high
//!   temperature (28.9649 vs 16.0896 W/(m K) at 2000 K), so the bound
//!   check deliberately covers the conduction-only regime.
//! - **Radiation monotonicity and `d`-scaling**: at fixed `k_f`, `k_eff`
//!   rises strictly monotonically over 1701 samples from 300 K to 2000 K
//!   (2.1132 to 28.6049 W/(m K)), and `k_r` is proportional to `d` to
//!   f64 exactness (measured deviation 0.0 on doubling and halving).
//!
//! A human should still verify the transcription against van Antwerpen et
//! al. (2010) eqs. (12)-(16) before this module is promoted past Prototype
//! in the V&V pipeline. **Nothing here is validated** — no comparison
//! against measurement has been made, and the one reference tabulation
//! available is *not* reproduced (next section).
//!
//! ## The VTB 18-point table is NOT reproduced — measured finding
//!
//! The Virtual Test Bed generic pebble-bed deck
//! (`reference-data/virtual_test_bed/htgr/generic-pbr/pbr.i`, block
//! `keff_pebble_bed`, Open tier) carries an 18-point `k_eff` tabulation
//! (300-2000 K, 11.940293-44.9504677 W/(m K)) described there as
//! "calculated from the ZBS correlation". This implementation, evaluated at
//! that deck's own stated inputs (eps = 0.39, d = 0.06 m, e_r = 0.8,
//! graphite k_s = 26 W/(m K), helium at 6 MPa — all four read back out of
//! the deck on 2026-08-11), does **not** reproduce it. Measured on
//! 2026-08-11: the model lies **below the table at all 18 points**, by a
//! factor of **5.65** at 300 K (2.1126 vs 11.9403 W/(m K), the worst
//! point) narrowing to **1.55** at 2000 K (28.9649 vs 44.9505 W/(m K), the
//! best point); the model/table ratio rises monotonically from 0.17693 to
//! 0.64437.
//!
//! The gap is not a tuning matter. Solving this implementation for the pore
//! conductivity that *would* land on the table's 300 K value gives
//! **k_f = 3.8367 W/(m K)** — **24.0 times** the 0.15992 W/(m K) that
//! `outram-park-fork-coolprop` gives for helium at 300 K and 6 MPa — and no
//! gas has such a conductivity. (An earlier revision of these docs also
//! appealed to "order 1-2 W/(m K) at ambient in SANA/HTTU-class
//! experiments". That is an **uncited recollection**, not checked against a
//! source here; it is retained only as a hypothesis for the human reviewer,
//! and none of the measured numbers above depend on it.) The full
//! quantitative comparison is pinned in
//! `tests::vtb_table_is_not_reproduced_by_zbs_with_helium`, and the
//! finding is tracked in the beads issue tracker. The table remains a
//! faithful transcription of what the VTB/SAM models *ran with*; whether it
//! is what ZBS *produces* is the open question that test documents.
//!
//! ## Near-wall region
//!
//! Bed voidage rises toward a containing wall, which locally changes the
//! effective conductivity. [`ZbsBed::wall_region_porosity`] provides an
//! exponential bulk-to-wall porosity profile so callers can evaluate the
//! correlation with the local voidage; its coefficients are flagged
//! provisional in its own doc comment.
//!
//! **Belongs here:** the ZBS correlation and its unit tests. **Does not
//! belong here:** graphite property data ([`tuas_boussinesq_solver`]'s
//! solid database), pressure drop ([`crate::gas_phase::KtaBed`]),
//! pebble-internal conduction ([`super::pebble`]).

use uom::si::f64::{Length, Ratio, ThermalConductivity, ThermodynamicTemperature};
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use crate::TampinesError;

/// Stefan-Boltzmann constant, W m^-2 K^-4 (CODATA 2018 exact value).
const SIGMA_W_PER_M2_K4: f64 = 5.670374419e-8;

/// A packed bed of monosized spheres, described by the four geometric and
/// surface parameters the Zehner-Bauer-Schlunder correlation needs. Plain
/// data; the physics lives in [`ZbsBed::effective_conductivity`].
///
/// Construct with [`ZbsBed::new`] (standard contact/shape factors) or
/// [`ZbsBed::htr10`] (the cited HTR-10 bed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZbsBed {
    /// Bed porosity (void fraction), dimensionless, strictly between 0
    /// and 1. Random close-packed sphere beds sit near 0.36-0.42; the
    /// HTR-10 bed is 0.39 (filling fraction 0.61, IAEA HTGR benchmark
    /// document, Open tier).
    pub porosity: Ratio,
    /// Pebble (sphere) diameter, metres. Sets the radiation length scale:
    /// the `4 sigma T^3 d` radiation conductivity is proportional to it.
    /// HTR-10: 0.06 m.
    pub pebble_diameter: Length,
    /// Total hemispherical surface emissivity of the spheres,
    /// dimensionless in (0, 1]. Graphite in the pebble-bed literature is
    /// taken as 0.8 (NEA PBMR-400 benchmark assumption, as quoted in the
    /// VTB generic pebble-bed deck, Open tier).
    pub emissivity: Ratio,
    /// Flattened contact-area fraction `phi` of the unit cell,
    /// dimensionless. Standard value 0.0077 for spheres (Bauer &
    /// Schlunder, as quoted in the van Antwerpen 2010 review). Governs the
    /// solid-solid contact conduction path, which matters most when the
    /// gas conducts poorly (vacuum/depressurised accident conditions).
    pub contact_area_fraction: Ratio,
    /// Sphere deformation/shape factor coefficient `C` in
    /// `B = C ((1-eps)/eps)^(10/9)`, dimensionless. 1.25 for spheres
    /// (Zehner & Schlunder 1970).
    pub shape_factor_c: Ratio,
}

impl ZbsBed {
    /// A ZBS bed with the standard sphere constants (`phi` = 0.0077,
    /// `C` = 1.25) and caller-supplied porosity (dimensionless, in (0,1)),
    /// pebble diameter (metres) and emissivity (dimensionless, in (0,1]).
    pub fn new(porosity: Ratio, pebble_diameter: Length, emissivity: Ratio) -> Self {
        Self {
            porosity,
            pebble_diameter,
            emissivity,
            contact_area_fraction: Ratio::new::<ratio>(0.0077),
            shape_factor_c: Ratio::new::<ratio>(1.25),
        }
    }

    /// The HTR-10 pebble bed: porosity 0.39 and pebble diameter 6.0 cm from
    /// the IAEA HTGR benchmark document (Open tier), graphite emissivity
    /// 0.8 per the NEA PBMR-400 benchmark assumption quoted in the VTB
    /// generic pebble-bed deck (Open tier), standard sphere contact/shape
    /// factors.
    pub fn htr10() -> Self {
        Self::new(
            Ratio::new::<ratio>(0.39),
            Length::new::<meter>(0.06),
            Ratio::new::<ratio>(0.8),
        )
    }

    /// Deformation factor `B = C ((1-eps)/eps)^(10/9)`, dimensionless.
    /// For the HTR-10 bed (eps = 0.39, C = 1.25) this is 2.0548 (measured
    /// 2026-08-11).
    ///
    /// `B` sets the model's domain: the unit cell is only defined for
    /// `kappa + k_r > B`, so a bed with `B > 1` (any porosity below about
    /// 0.55 at `C = 1.25`) cannot be evaluated with a pore fluid that
    /// conducts as well as the solid.
    pub fn deformation_factor(&self) -> Ratio {
        let eps = self.porosity.get::<ratio>();
        let c = self.shape_factor_c.get::<ratio>();
        Ratio::new::<ratio>(c * ((1.0 - eps) / eps).powf(10.0 / 9.0))
    }

    /// Dimensionless radiation-to-fluid-conduction ratio
    /// `k_r = 4 sigma T^3 d / ((2/e_r - 1) k_f)`, with `T` the local bed
    /// temperature (kelvin), `d` the pebble diameter and `k_f` the pore
    /// fluid conductivity (W/(m K)). Grows as `T^3`; at HTR-10
    /// loss-of-forced-cooling temperatures it dominates the bed
    /// conductivity.
    pub fn radiation_ratio(
        &self,
        fluid_conductivity: ThermalConductivity,
        temperature: ThermodynamicTemperature,
    ) -> Ratio {
        let t = temperature.get::<kelvin>();
        let d = self.pebble_diameter.get::<meter>();
        let e = self.emissivity.get::<ratio>();
        let kf = fluid_conductivity.get::<watt_per_meter_kelvin>();
        Ratio::new::<ratio>(4.0 * SIGMA_W_PER_M2_K4 * t.powi(3) * d / ((2.0 / e - 1.0) * kf))
    }

    /// Effective bed thermal conductivity, W/(m K), from the full ZBS
    /// correlation (stagnant gas + solid + contact + radiation; see the
    /// module docs for the equations, provenance and the homogeneous
    /// effective-medium assumption).
    ///
    /// Inputs: the solid (pebble) conductivity `k_s`, the pore fluid
    /// conductivity `k_f` (both W/(m K), both strictly positive) and the
    /// local bed temperature (kelvin, strictly positive; enters only
    /// through the `T^3` radiation term). This models a **stagnant** bed —
    /// convective enhancement by through-flow is a separate closure and is
    /// deliberately not smuggled in here.
    ///
    /// Errors with [`TampinesError::InvalidInput`] for out-of-domain inputs
    /// (non-positive conductivities or temperature, porosity outside
    /// (0,1), emissivity outside (0,1]) and with
    /// [`TampinesError::Unphysical`] if the unit-cell denominator `N` or
    /// the logarithm argument leaves its valid region (reachable only when
    /// the solid conducts *worse* than about `B` times the fluid,
    /// `kappa < B`, with negligible radiation — not a gas-cooled-bed
    /// regime, but it *is* what the uniform-medium case `k_s = k_f` hits at
    /// pebble-bed porosities; see the module docs).
    pub fn effective_conductivity(
        &self,
        solid_conductivity: ThermalConductivity,
        fluid_conductivity: ThermalConductivity,
        temperature: ThermodynamicTemperature,
    ) -> Result<ThermalConductivity, TampinesError> {
        let eps = self.porosity.get::<ratio>();
        let e = self.emissivity.get::<ratio>();
        let phi = self.contact_area_fraction.get::<ratio>();
        let ks = solid_conductivity.get::<watt_per_meter_kelvin>();
        let kf = fluid_conductivity.get::<watt_per_meter_kelvin>();
        let t = temperature.get::<kelvin>();

        if !(eps > 0.0 && eps < 1.0) {
            return Err(TampinesError::InvalidInput(format!(
                "ZBS: porosity must be in (0,1), got {eps}"
            )));
        }
        if !(e > 0.0 && e <= 1.0) {
            return Err(TampinesError::InvalidInput(format!(
                "ZBS: emissivity must be in (0,1], got {e}"
            )));
        }
        if !(ks > 0.0) || !(kf > 0.0) || !(t > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "ZBS: k_s, k_f, T must be positive and finite, got k_s={ks}, k_f={kf}, T={t}"
            )));
        }

        let b = self.deformation_factor().get::<ratio>();
        let kappa = ks / kf;
        let kr = self
            .radiation_ratio(fluid_conductivity, temperature)
            .get::<ratio>();

        let n = 1.0 + (kr - b) / kappa;
        let ln_arg = (kappa + kr) / b;
        if !(n > 0.0) || !(ln_arg > 0.0) {
            return Err(TampinesError::Unphysical(format!(
                "ZBS unit cell left its valid region: N={n}, (kappa+k_r)/B={ln_arg} \
                 (kappa={kappa}, k_r={kr}, B={b}); the correlation requires the solid \
                 to conduct better than ~B times the fluid"
            )));
        }

        let kc = (2.0 / n)
            * (b * (kappa + kr - 1.0) / (n * n * kappa) * ln_arg.ln()
                + (b + 1.0) / (2.0 * b) * (kr - b)
                - (b - 1.0) / n);

        let one_minus_sqrt = 1.0 - (1.0 - eps).sqrt();
        let sqrt_term = (1.0 - eps).sqrt();
        let keff_over_kf =
            one_minus_sqrt * (1.0 + eps * kr) + sqrt_term * (phi * kappa + (1.0 - phi) * kc);

        let keff = keff_over_kf * kf;
        if !(keff.is_finite() && keff > 0.0) {
            return Err(TampinesError::Numerical(format!(
                "ZBS produced a non-finite or non-positive k_eff = {keff} \
                 (kappa={kappa}, k_r={kr}, N={n})"
            )));
        }
        Ok(ThermalConductivity::new::<watt_per_meter_kelvin>(keff))
    }

    /// Stagnant-bed conductivity: the ZBS evaluation with the radiation
    /// path removed (`k_r = 0`), i.e. the classic Zehner-Schlunder
    /// conduction-only result plus the contact term. Exposed so tests and
    /// callers can separate the conduction and radiation contributions;
    /// same input domain and errors as
    /// [`ZbsBed::effective_conductivity`].
    pub fn stagnant_conductivity(
        &self,
        solid_conductivity: ThermalConductivity,
        fluid_conductivity: ThermalConductivity,
    ) -> Result<ThermalConductivity, TampinesError> {
        // T -> 0+ sends the radiation ratio to zero; 1e-6 K keeps the
        // input-domain checks satisfied while making 4*sigma*T^3*d
        // numerically zero (~1e-27) against any physical conductivity.
        self.effective_conductivity(
            solid_conductivity,
            fluid_conductivity,
            ThermodynamicTemperature::new::<kelvin>(1e-6),
        )
    }

    /// Local porosity a distance `y` from a containing wall, rising from
    /// the bulk value toward the wall:
    /// `eps(y) = eps_bulk * (1 + 1.36 * exp(-5 * y / d))`, clamped to at
    /// most 1. At `y = 0` this gives `2.36 * eps_bulk` (0.92 for the
    /// HTR-10 bulk 0.39), decaying to the bulk value within about one
    /// pebble diameter — the well-known near-wall voidage rise that lowers
    /// the local solid/contact conduction and must be accounted for in the
    /// wall heat-transfer path.
    ///
    /// **Provisional coefficients.** The exponential form with (1.36, 5.0)
    /// is the Hunt & Tien-type fit as commonly quoted in the packed-bed
    /// literature (e.g. in the van Antwerpen 2010 review's near-wall
    /// discussion); the coefficients were *not* verified against the
    /// original paper in this session. Treat as a placeholder profile —
    /// adequate for exercising the local-porosity mechanism, not for
    /// quantitative wall-region V&V.
    pub fn wall_region_porosity(&self, distance_from_wall: Length) -> Ratio {
        let y = distance_from_wall.get::<meter>().max(0.0);
        let d = self.pebble_diameter.get::<meter>();
        let eps_bulk = self.porosity.get::<ratio>();
        let eps = eps_bulk * (1.0 + 1.36 * (-5.0 * y / d).exp());
        Ratio::new::<ratio>(eps.min(1.0))
    }

    /// A copy of this bed with a different porosity — the intended way to
    /// evaluate the correlation with the local near-wall voidage from
    /// [`ZbsBed::wall_region_porosity`]:
    /// `bed.with_porosity(bed.wall_region_porosity(y))`.
    pub fn with_porosity(&self, porosity: Ratio) -> Self {
        Self { porosity, ..*self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_fork_coolprop::{conductivity, density_pt, Fluid};

    // ---------------------------------------------------------------
    // Reference tabulation (transcribed 2026-08-11)
    //
    // Source file : reference-data/virtual_test_bed/htgr/generic-pbr/pbr.i
    // Deck        : NRIC/INL Virtual Test Bed, generic pebble-bed reactor
    //               (SAM), block `keff_pebble_bed`, whose own comment reads
    //               "Pebble bed effective thermal conductivity calculated
    //               from the ZBS correlation".
    // Access tier : Open (public Virtual Test Bed release).
    // Processing  : none — the `x` and `y` rows are copied verbatim, with
    //               only whitespace/line-wrapping changed.
    //
    // The same 18 pairs are carried, independently transcribed, by
    // `outram-park-digital-twin-engine`'s `htr10::zbs` table module. That
    // crate is deliberately NOT a dependency here (it is a GUI crate,
    // Android-hostile, and sits downstream of `tampines`), so the numbers
    // are duplicated rather than imported.
    // ---------------------------------------------------------------

    /// Temperatures (K) of the VTB `keff_pebble_bed` tabulation, row `x`.
    const VTB_TEMPERATURE_KELVIN: [f64; 18] = [
        300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0,
        1500.0, 1600.0, 1700.0, 1800.0, 1900.0, 2000.0,
    ];

    /// Effective bed conductivities (W/(m K)) of the VTB `keff_pebble_bed`
    /// tabulation, row `y`, matching [`VTB_TEMPERATURE_KELVIN`] point for
    /// point.
    const VTB_CONDUCTIVITY_W_PER_M_K: [f64; 18] = [
        11.940293,
        12.87749357,
        14.41727341,
        16.46031467,
        18.89508767,
        21.59454026,
        24.42480955,
        27.25852959,
        29.98755338,
        32.53156707,
        34.84117769,
        36.89601027,
        38.69951358,
        40.272406,
        41.64628551,
        42.8582912,
        43.94713743,
        44.9504677,
    ];

    /// Graphite conductivity (W/(m K)) the VTB deck assigns to the pebble
    /// solid: `pbr.i` block `[graphite-mat]`, `k = 26`, commented "From NEA
    /// report [2]". Read back out of the deck 2026-08-11.
    const VTB_SOLID_CONDUCTIVITY_W_PER_M_K: f64 = 26.0;

    /// System pressure (Pa) of the VTB deck: `global_init_P = 6e6` and the
    /// 6 MPa outlet-pressure function. Read back 2026-08-11.
    const VTB_PRESSURE_PA: f64 = 6.0e6;

    /// Helium thermal conductivity (W/(m K)) at `temperature_kelvin` and
    /// the VTB deck's 6 MPa, from `outram-park-fork-coolprop`: a
    /// pressure-temperature density flash ([`density_pt`], Ortiz-Vega et
    /// al. Helmholtz EOS) followed by the Hands & Arp hardcoded helium
    /// conductivity model ([`conductivity`]). The property therefore comes
    /// from code, not a hand-picked constant.
    ///
    /// Panics if either call fails — a helium property failure at 300-2000 K
    /// and 6 MPa (well clear of the critical region) means the property
    /// backend is broken, and silently substituting a fallback would
    /// corrupt the comparison this module documents.
    fn helium_conductivity_at(temperature_kelvin: f64) -> f64 {
        let rho = density_pt(Fluid::Helium, temperature_kelvin, VTB_PRESSURE_PA)
            .expect("coolprop: helium density at (T, 6 MPa)");
        conductivity(Fluid::Helium, temperature_kelvin, rho)
            .expect("coolprop: helium thermal conductivity at (T, rho)")
    }

    /// Convenience: `k` W/(m K) as a `uom` [`ThermalConductivity`].
    fn wmk(k: f64) -> ThermalConductivity {
        ThermalConductivity::new::<watt_per_meter_kelvin>(k)
    }

    /// Convenience: `t` K as a `uom` [`ThermodynamicTemperature`].
    fn kelv(t: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(t)
    }

    /// V&V (characterization): this ZBS implementation does **not**
    /// reproduce the VTB generic-PBR 18-point `k_eff` tabulation with
    /// helium in the pores.
    ///
    /// **Methodology.** [`ZbsBed::htr10`] (eps = 0.39, d = 0.06 m,
    /// e_r = 0.8, phi = 0.0077, C = 1.25) is evaluated at each of the 18
    /// tabulated temperatures with the deck's own inputs: solid
    /// conductivity [`VTB_SOLID_CONDUCTIVITY_W_PER_M_K`] = 26 W/(m K)
    /// (`[graphite-mat]`) and pore-fluid conductivity from
    /// [`helium_conductivity_at`] at [`VTB_PRESSURE_PA`] = 6 MPa. Three
    /// things are then asserted:
    ///
    /// 1. the helium conductivities are pinned to 1e-6 relative, so a
    ///    change in the `outram-park-fork-coolprop` property model shows up
    ///    as a failure of *this* test rather than silently shifting the
    ///    documented finding;
    /// 2. the 18 model outputs are pinned to 1e-6 relative (regression
    ///    pinning — these are the numbers the module docs quote);
    /// 3. the model is strictly below the table at every point, by at least
    ///    a factor 5.6 at 300 K.
    ///
    /// Finally the pore conductivity that *would* place the model on the
    /// table's 300 K value is found by bisection over the model's valid
    /// region (`k_f` in [0.16, 12.6] W/(m K), where `kappa > B`; the model
    /// is monotonic in `k_f` there and errors above ~12.65 W/(m K) where
    /// `kappa` falls below `B` = 2.0548).
    ///
    /// **Results (measured 2026-08-11, release build).** The model lies
    /// below the table at all 18 points. Ratios model/table rise
    /// monotonically from **0.176932 at 300 K** (2.112625 vs 11.940293
    /// W/(m K); the table is 5.65x higher — the worst point) to **0.644374
    /// at 2000 K** (28.964901 vs 44.950468 W/(m K); 1.55x — the best
    /// point). Reproducing the 300 K table value would require a pore
    /// conductivity of **3.8367 W/(m K)**, **23.99 times** the 0.159924
    /// W/(m K) coolprop gives for helium at 300 K / 6 MPa.
    ///
    /// **Interpretation.** No gas conducts at ~3.8 W/(m K), so the table
    /// cannot be recovered from this formulation by any admissible choice
    /// of pore fluid. The VTB table is therefore either computed with a
    /// different ZBS variant/parameter set than the one implemented here,
    /// or is not a pore-gas ZBS result at all. This test does **not**
    /// decide which; it pins what this code produces so the discrepancy
    /// cannot be quietly lost, and no tuning has been applied to close it.
    /// Whether 2.11 W/(m K) at 300 K is itself the physically right answer
    /// is a separate, open question — it has **not** been compared against
    /// any measurement, and the "SANA/HTTU-class beds measure order 1-2
    /// W/(m K)" remark in the module docs is an uncited recollection
    /// awaiting a human check, not evidence produced here.
    #[test]
    fn vtb_table_is_not_reproduced_by_zbs_with_helium() {
        /// Helium conductivity (W/(m K)) at 6 MPa from
        /// `outram-park-fork-coolprop`, measured 2026-08-11.
        const MEASURED_HELIUM_K_W_PER_M_K: [f64; 18] = [
            0.159923908,
            0.193988103,
            0.225734235,
            0.255706081,
            0.284261401,
            0.311649494,
            0.338052186,
            0.363606982,
            0.388420947,
            0.412579458,
            0.436151966,
            0.459195912,
            0.481759496,
            0.503883656,
            0.525603544,
            0.546949625,
            0.567948524,
            0.588623686,
        ];
        /// This implementation's `k_eff` (W/(m K)) at the 18 VTB points,
        /// measured 2026-08-11. Regression pin, not a reference value.
        const MEASURED_MODEL_K_W_PER_M_K: [f64; 18] = [
            2.112625197426,
            2.722907382001,
            3.509789511385,
            4.506085143543,
            5.723188706794,
            7.151681996787,
            8.764326469007,
            10.521526514271,
            12.378171016630,
            14.290281840354,
            16.220143004252,
            18.139268544705,
            20.029280419149,
            21.881225457503,
            23.693998467257,
            25.472448379055,
            27.225557736780,
            28.964901024492,
        ];

        let bed = ZbsBed::htr10();
        let ks = wmk(VTB_SOLID_CONDUCTIVITY_W_PER_M_K);

        let mut min_ratio = f64::INFINITY;
        let mut max_ratio = f64::NEG_INFINITY;
        let (mut min_at, mut max_at) = (0.0, 0.0);

        for i in 0..18 {
            let t = VTB_TEMPERATURE_KELVIN[i];
            let kf = helium_conductivity_at(t);
            let rel_kf =
                (kf - MEASURED_HELIUM_K_W_PER_M_K[i]).abs() / MEASURED_HELIUM_K_W_PER_M_K[i];
            assert!(
                rel_kf < 1e-6,
                "helium k_f at {t} K drifted from the pinned value: {kf} vs \
                 {} (rel {rel_kf:.3e}); the coolprop property model changed, so \
                 the documented VTB comparison must be re-measured",
                MEASURED_HELIUM_K_W_PER_M_K[i]
            );

            let k = bed
                .effective_conductivity(ks, wmk(kf), kelv(t))
                .expect("HTR-10 bed with helium is inside the ZBS domain")
                .get::<watt_per_meter_kelvin>();
            let rel_k = (k - MEASURED_MODEL_K_W_PER_M_K[i]).abs() / MEASURED_MODEL_K_W_PER_M_K[i];
            assert!(
                rel_k < 1e-6,
                "model k_eff at {t} K drifted from the pinned value: {k} vs {} (rel {rel_k:.3e})",
                MEASURED_MODEL_K_W_PER_M_K[i]
            );

            let model_over_table = k / VTB_CONDUCTIVITY_W_PER_M_K[i];
            assert!(
                model_over_table < 1.0,
                "model unexpectedly reached or exceeded the VTB table at {t} K \
                 ({k} vs {}); the documented finding must be rewritten",
                VTB_CONDUCTIVITY_W_PER_M_K[i]
            );
            if model_over_table < min_ratio {
                min_ratio = model_over_table;
                min_at = t;
            }
            if model_over_table > max_ratio {
                max_ratio = model_over_table;
                max_at = t;
            }
            println!(
                "T={t:6.1} K  k_f={kf:.9}  model={k:.9}  table={:.6}  model/table={model_over_table:.6}",
                VTB_CONDUCTIVITY_W_PER_M_K[i]
            );
        }

        println!(
            "worst ratio {min_ratio:.6} at {min_at} K, best ratio {max_ratio:.6} at {max_at} K"
        );
        assert!(
            (min_ratio - 0.176932).abs() < 1e-5 && min_at == 300.0,
            "worst-point ratio moved: {min_ratio} at {min_at} K"
        );
        assert!(
            (max_ratio - 0.644374).abs() < 1e-5 && max_at == 2000.0,
            "best-point ratio moved: {max_ratio} at {max_at} K"
        );

        // Pore conductivity that would land on the table's 300 K value.
        // Bisection inside the valid, monotonic region kappa > B.
        let (mut lo, mut hi) = (0.16_f64, 12.6_f64);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            let v = bed
                .effective_conductivity(ks, wmk(mid), kelv(300.0))
                .expect("bisection bracket stays inside the ZBS domain")
                .get::<watt_per_meter_kelvin>();
            if v < VTB_CONDUCTIVITY_W_PER_M_K[0] {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let kf_needed = 0.5 * (lo + hi);
        let helium_300 = helium_conductivity_at(300.0);
        println!(
            "k_f needed at 300 K = {kf_needed:.6} W/(m K) = {:.3}x helium's {helium_300:.6}",
            kf_needed / helium_300
        );
        assert!(
            (kf_needed - 3.8367).abs() < 1e-3,
            "required pore conductivity moved: {kf_needed}"
        );
        assert!(
            (kf_needed / helium_300 - 23.99).abs() < 0.05,
            "required pore-conductivity multiple moved: {}",
            kf_needed / helium_300
        );
    }

    /// V&V: the radiation path vanishes in both limits that must kill it —
    /// zero surface emissivity and zero temperature.
    ///
    /// **Methodology.** Two independent limits on the HTR-10 bed with
    /// k_s = 26, k_f = 0.16 W/(m K):
    ///
    /// 1. *Emissivity.* `k_r` carries the factor `1/(2/e_r - 1)`, so
    ///    `e_r -> 0+` must send it to zero. `effective_conductivity` at
    ///    1000 K is evaluated at e_r = 1e-3, 1e-6 and 1e-9 and compared to
    ///    [`ZbsBed::stagnant_conductivity`] for the same bed. Pass
    ///    criterion: the relative excess falls linearly with `e_r`, and is
    ///    below 1e-8 at e_r = 1e-9.
    /// 2. *Temperature.* `k_r` carries `T^3`, so a low-temperature
    ///    evaluation must coincide with the stagnant result. Pass
    ///    criterion: at 1 K the relative excess is below 1e-8.
    ///
    /// **Results (measured 2026-08-11).** Emissivity limit: relative
    /// excess 4.177e-3 at e_r = 1e-3, 4.176e-6 at 1e-6, **4.176e-9 at
    /// 1e-9** — exactly the linear-in-`e_r` decay `k_r ~ e_r/2` predicts.
    /// Temperature limit: at 1 K the relative excess is **5.836e-9**
    /// against a stagnant 1.761972 W/(m K) (k_f = 0.15), rising to 5.83e-3
    /// by 100 K and 1.56e-1 by 300 K as radiation switches on. Both limits
    /// behave as the formulation requires.
    #[test]
    fn radiation_vanishes_in_the_zero_emissivity_and_low_temperature_limits() {
        let ks = wmk(26.0);
        let kf = wmk(0.16);

        let mut prev_rel = f64::INFINITY;
        for (e_r, bound) in [(1e-3, 5e-3), (1e-6, 5e-6), (1e-9, 1e-8)] {
            let bed = ZbsBed::new(
                Ratio::new::<ratio>(0.39),
                Length::new::<meter>(0.06),
                Ratio::new::<ratio>(e_r),
            );
            let hot = bed
                .effective_conductivity(ks, kf, kelv(1000.0))
                .expect("in domain")
                .get::<watt_per_meter_kelvin>();
            let stagnant = bed
                .stagnant_conductivity(ks, kf)
                .expect("in domain")
                .get::<watt_per_meter_kelvin>();
            let rel = (hot - stagnant).abs() / stagnant;
            println!("e_r={e_r:e}: eff(1000 K)={hot:.12}, stagnant={stagnant:.12}, rel={rel:.3e}");
            assert!(
                rel < bound,
                "radiation did not vanish at e_r={e_r}: rel={rel:.3e}"
            );
            assert!(rel < prev_rel, "relative excess must fall with e_r");
            prev_rel = rel;
        }

        let bed = ZbsBed::htr10();
        let kf_cold = wmk(0.15);
        let stagnant = bed
            .stagnant_conductivity(ks, kf_cold)
            .expect("in domain")
            .get::<watt_per_meter_kelvin>();
        let cold = bed
            .effective_conductivity(ks, kf_cold, kelv(1.0))
            .expect("in domain")
            .get::<watt_per_meter_kelvin>();
        let rel = (cold - stagnant).abs() / stagnant;
        println!("T=1 K: eff={cold:.12}, stagnant={stagnant:.12}, rel={rel:.3e}");
        assert!(rel < 1e-8, "radiation did not vanish at 1 K: rel={rel:.3e}");
    }

    /// V&V: the conduction-only (stagnant) result lies strictly between the
    /// Wiener series and parallel bounds.
    ///
    /// **Methodology.** For a two-phase medium of volume fractions
    /// `(1 - eps)` solid and `eps` fluid, any effective conductivity must
    /// satisfy the Wiener bounds
    /// `k_series <= k_eff <= k_parallel`, with
    /// `k_series = 1/((1-eps)/k_s + eps/k_f)` (harmonic mix) and
    /// `k_parallel = (1-eps) k_s + eps k_f` (arithmetic mix). The check
    /// sweeps a 27-point grid — eps in {0.36, 0.39, 0.42}, k_s in {26, 60,
    /// 100} W/(m K), k_f in {0.02, 0.15, 1.0} W/(m K) — evaluating
    /// [`ZbsBed::stagnant_conductivity`], i.e. `k_r = 0`.
    ///
    /// **The bound applies to conduction only.** Radiation is a *third*
    /// transport mechanism, not a phase, so the full
    /// [`ZbsBed::effective_conductivity`] may and does exceed the
    /// conduction-only parallel bound at high temperature — measured
    /// 28.9649 W/(m K) at 2000 K against a parallel bound of 16.0896
    /// W/(m K) for the HTR-10 bed with helium. This test therefore covers
    /// the conduction-only regime, and asserts that high-temperature
    /// excursion separately so the distinction is recorded rather than
    /// assumed.
    ///
    /// **Results (measured 2026-08-11).** All 27 grid points lie strictly
    /// inside the bounds. Tightest margins: `k_eff/k_series` = **2.3754**
    /// (its closest approach to the series bound) and `k_eff/k_parallel` =
    /// **0.3721** (its closest approach to the parallel bound) — the
    /// stagnant bed sits, as expected for a poorly-conducting gas in the
    /// voids, in the lower part of the admissible band. The
    /// high-temperature radiative excursion above the conduction-only
    /// parallel bound was confirmed at 2000 K: 28.964901 W/(m K) against a
    /// parallel bound of 16.089563 W/(m K), a factor 1.800.
    #[test]
    fn conduction_only_result_lies_between_the_wiener_bounds() {
        let mut min_series_margin = f64::INFINITY;
        let mut max_parallel_margin = f64::NEG_INFINITY;

        for eps in [0.36_f64, 0.39, 0.42] {
            let bed = ZbsBed::new(
                Ratio::new::<ratio>(eps),
                Length::new::<meter>(0.06),
                Ratio::new::<ratio>(0.8),
            );
            for ks in [26.0_f64, 60.0, 100.0] {
                for kf in [0.02_f64, 0.15, 1.0] {
                    let k = bed
                        .stagnant_conductivity(wmk(ks), wmk(kf))
                        .expect("kappa > B on this grid")
                        .get::<watt_per_meter_kelvin>();
                    let series = 1.0 / ((1.0 - eps) / ks + eps / kf);
                    let parallel = (1.0 - eps) * ks + eps * kf;
                    assert!(
                        k > series,
                        "below the series bound: eps={eps} k_s={ks} k_f={kf} \
                         k_eff={k} series={series}"
                    );
                    assert!(
                        k < parallel,
                        "above the parallel bound: eps={eps} k_s={ks} k_f={kf} \
                         k_eff={k} parallel={parallel}"
                    );
                    min_series_margin = min_series_margin.min(k / series);
                    max_parallel_margin = max_parallel_margin.max(k / parallel);
                }
            }
        }
        println!(
            "tightest margins: k_eff/k_series={min_series_margin:.4}, \
             k_eff/k_parallel={max_parallel_margin:.4}"
        );
        assert!(min_series_margin > 1.0 && max_parallel_margin < 1.0);

        // Radiation legitimately exceeds the conduction-only parallel bound.
        let bed = ZbsBed::htr10();
        let kf = helium_conductivity_at(2000.0);
        let hot = bed
            .effective_conductivity(wmk(VTB_SOLID_CONDUCTIVITY_W_PER_M_K), wmk(kf), kelv(2000.0))
            .expect("in domain")
            .get::<watt_per_meter_kelvin>();
        let parallel = 0.61 * VTB_SOLID_CONDUCTIVITY_W_PER_M_K + 0.39 * kf;
        println!("2000 K: k_eff={hot:.6} vs conduction-only parallel bound {parallel:.6}");
        assert!(
            hot > parallel,
            "radiation should carry k_eff past the conduction-only parallel bound at 2000 K"
        );
    }

    /// V&V: a uniform medium (`k_s = k_f`) collapses to that conductivity
    /// exactly — wherever the unit cell is defined.
    ///
    /// **Methodology.** Setting `k_s = k_f` makes `kappa = 1`, for which
    /// the `k_r = 0` unit-cell term is analytically `k_c = 1`, so
    /// `k_eff/k_f = (1 - sqrt(1-eps)) + sqrt(1-eps)(phi + (1-phi)) = 1`.
    /// The check evaluates [`ZbsBed::stagnant_conductivity`] with
    /// `k_s = k_f` at k = 0.15, 1.0 and 26.0 W/(m K), for porosities 0.39,
    /// 0.5 (both `B >= 1`) and 0.6, 0.8 (both `B < 1`). Pass criterion:
    /// relative deviation below 1e-14 where `B < 1`, and a
    /// [`TampinesError::Unphysical`] where `B >= 1`.
    ///
    /// **Domain caveat — this is the real finding.** `kappa = 1` only
    /// satisfies the model's `kappa + k_r > B` requirement when `B < 1`,
    /// which at `C = 1.25` means `eps` above about 0.55. Packed sphere beds
    /// (eps ~ 0.36-0.42) are therefore *outside* the regime where this
    /// collapse can be evaluated at all: at eps = 0.39, `B` = 2.0548 and
    /// the correlation returns `Unphysical`. An earlier revision of this
    /// module's docs listed uniform-medium collapse among checks that
    /// "all pass" without this qualifier; the qualifier is the measured
    /// truth.
    ///
    /// **Results (measured 2026-08-11).** Where `B < 1` the collapse is
    /// exact to f64 roundoff: maximum relative deviation **4.441e-16**
    /// (eps = 0.6, B = 0.796623) and **0.0** (eps = 0.8, B = 0.267889)
    /// across all three conductivity levels. Where `B >= 1` (eps = 0.39,
    /// B = 2.054756; eps = 0.5, B = 1.25) every evaluation returned
    /// `Unphysical` with `N` = -1.054756 and -0.25 respectively, as the
    /// documented domain guard requires.
    #[test]
    fn uniform_medium_collapses_to_the_fluid_conductivity_where_the_unit_cell_is_valid() {
        let mut worst_rel: f64 = 0.0;
        for eps in [0.39_f64, 0.5, 0.6, 0.8] {
            let bed = ZbsBed::new(
                Ratio::new::<ratio>(eps),
                Length::new::<meter>(0.06),
                Ratio::new::<ratio>(0.8),
            );
            let b = bed.deformation_factor().get::<ratio>();
            for k in [0.15_f64, 1.0, 26.0] {
                let result = bed.stagnant_conductivity(wmk(k), wmk(k));
                if b < 1.0 {
                    let out = result
                        .expect("kappa = 1 > B is inside the domain when B < 1")
                        .get::<watt_per_meter_kelvin>();
                    let rel = (out - k).abs() / k;
                    println!("eps={eps} B={b:.6} k={k}: k_eff={out:.15} rel={rel:.3e}");
                    assert!(
                        rel < 1e-14,
                        "uniform medium did not collapse: rel={rel:.3e}"
                    );
                    worst_rel = worst_rel.max(rel);
                } else {
                    println!("eps={eps} B={b:.6} k={k}: {:?}", result.as_ref().err());
                    assert!(
                        matches!(result, Err(TampinesError::Unphysical(_))),
                        "expected Unphysical for kappa = 1 < B = {b} at eps = {eps}"
                    );
                }
            }
        }
        println!("worst collapse deviation where B < 1: {worst_rel:.3e}");
        assert!(worst_rel < 1e-14);
    }

    /// V&V: at `k_r = 0` the unit-cell term degenerates to the classic
    /// Zehner-Schlunder closed form.
    ///
    /// **Methodology.** The test re-assembles the stagnant result from the
    /// Zehner-Schlunder expression as it is usually printed,
    ///
    /// ```text
    /// N     = 1 - B/kappa
    /// k_c   = (2/N) * [ B (1 - 1/kappa) / N^2 * ln(kappa/B)
    ///                   - (B + 1)/2 - (B - 1)/N ]
    /// k_eff = k_f * [ (1 - sqrt(1-eps)) + sqrt(1-eps) (phi kappa + (1-phi) k_c) ]
    /// ```
    ///
    /// and compares it to [`ZbsBed::stagnant_conductivity`] over eps in
    /// {0.36, 0.39, 0.42}, k_s in {26, 100} W/(m K) and k_f in {0.02, 0.15}
    /// W/(m K). Pass criterion: relative difference below 1e-12.
    ///
    /// **This is an algebraic cross-check, not an independent reference.**
    /// The two expressions are the same published formula in different
    /// groupings, so agreement verifies the `k_r -> 0` limit and the
    /// implementation's arithmetic. It does **not** verify that the
    /// formula was transcribed correctly from van Antwerpen et al. (2010) —
    /// that check still needs a human with the paper in hand.
    ///
    /// **Results (measured 2026-08-11).** Maximum relative difference over
    /// the 12-point grid: **1.398e-16** (f64 roundoff). The residual
    /// `k_r ~ 1e-26` that [`ZbsBed::stagnant_conductivity`]'s 1e-6 K
    /// evaluation leaves behind is far below this.
    #[test]
    fn zero_radiation_degenerates_to_the_classic_zehner_schlunder_form() {
        let mut worst: f64 = 0.0;
        for eps in [0.36_f64, 0.39, 0.42] {
            let bed = ZbsBed::new(
                Ratio::new::<ratio>(eps),
                Length::new::<meter>(0.06),
                Ratio::new::<ratio>(0.8),
            );
            let b = bed.deformation_factor().get::<ratio>();
            let phi = bed.contact_area_fraction.get::<ratio>();
            for ks in [26.0_f64, 100.0] {
                for kf in [0.02_f64, 0.15] {
                    let kappa = ks / kf;
                    let n = 1.0 - b / kappa;
                    let kc = (2.0 / n)
                        * (b * (1.0 - 1.0 / kappa) / (n * n) * (kappa / b).ln()
                            - (b + 1.0) / 2.0
                            - (b - 1.0) / n);
                    let s = (1.0 - eps).sqrt();
                    let reference = kf * ((1.0 - s) + s * (phi * kappa + (1.0 - phi) * kc));

                    let implemented = bed
                        .stagnant_conductivity(wmk(ks), wmk(kf))
                        .expect("in domain")
                        .get::<watt_per_meter_kelvin>();
                    let rel = (implemented - reference).abs() / reference;
                    worst = worst.max(rel);
                }
            }
        }
        println!("worst Zehner-Schlunder degeneration deviation: {worst:.3e}");
        assert!(
            worst < 1e-12,
            "k_r -> 0 limit does not match the classic form: {worst:.3e}"
        );
    }

    /// V&V: with radiation active, `k_eff` rises strictly with temperature,
    /// and the radiation ratio scales linearly with pebble diameter.
    ///
    /// **Methodology.** Two properties of the `k_r = 4 sigma T^3 d /
    /// ((2/e_r - 1) k_f)` term:
    ///
    /// 1. *Monotonicity.* The HTR-10 bed is evaluated at fixed
    ///    k_s = 26 W/(m K) and fixed k_f = 0.16 W/(m K) — `k_f` held
    ///    constant deliberately, so the rise is attributable to radiation
    ///    alone and not to the pore gas conducting better when hot — at
    ///    every whole kelvin from 300 K to 2000 K (1701 samples). Pass
    ///    criterion: each sample strictly exceeds the previous one.
    /// 2. *`d`-scaling.* [`ZbsBed::radiation_ratio`] is evaluated at 1000 K
    ///    for d = 0.03, 0.06 and 0.12 m. Pass criterion: exact
    ///    proportionality to `d` within 1e-12 relative.
    ///
    /// **Results (measured 2026-08-11).** Strictly increasing over all
    /// 1701 samples, from **2.113196 W/(m K) at 300 K** through 3.071282
    /// (500 K), 9.697962 (1000 K), 19.432299 (1500 K) to **28.604940
    /// W/(m K) at 2000 K** — a 13.5x rise driven entirely by the `T^3`
    /// term. `d`-scaling: k_r(1000 K) = 28.351872, 56.703744 and
    /// 113.407488 at k_f = 0.16 W/(m K) for d = 0.03, 0.06 and 0.12 m,
    /// i.e. proportional to `d` to **f64 exactness** (measured deviation
    /// 0.0 — the ratio `k_r/d` is bit-identical across the three).
    #[test]
    fn radiation_grows_monotonically_with_temperature_and_linearly_with_diameter() {
        let bed = ZbsBed::htr10();
        let ks = wmk(26.0);
        let kf = wmk(0.16);

        let mut previous = 0.0_f64;
        for t in 300..=2000 {
            let k = bed
                .effective_conductivity(ks, kf, kelv(t as f64))
                .expect("in domain")
                .get::<watt_per_meter_kelvin>();
            assert!(
                k > previous,
                "k_eff did not increase at {t} K: {k} <= {previous}"
            );
            previous = k;
        }
        println!("k_eff(2000 K, fixed k_f) = {previous:.6} W/(m K)");

        let mut ratios = Vec::new();
        for d in [0.03_f64, 0.06, 0.12] {
            let b = ZbsBed::new(
                Ratio::new::<ratio>(0.39),
                Length::new::<meter>(d),
                Ratio::new::<ratio>(0.8),
            );
            let kr = b.radiation_ratio(kf, kelv(1000.0)).get::<ratio>();
            println!("d={d} m: k_r(1000 K) = {kr:.9}");
            ratios.push(kr / d);
        }
        let worst = ratios
            .iter()
            .map(|r| (r - ratios[0]).abs() / ratios[0])
            .fold(0.0_f64, f64::max);
        println!("worst deviation from k_r proportional to d: {worst:.3e}");
        assert!(worst < 1e-12, "k_r is not proportional to d: {worst:.3e}");
    }

    /// V&V: out-of-domain inputs are rejected with the documented error
    /// variants rather than returning a plausible-looking number.
    ///
    /// **Methodology.** Each documented domain violation is exercised once
    /// against [`ZbsBed::effective_conductivity`]:
    /// porosity 0 and 1, emissivity 0 and 1.5, negative and zero
    /// conductivities, zero and negative temperature — all of which must
    /// give [`TampinesError::InvalidInput`] — and `kappa < B`
    /// (k_s = k_f = 1 W/(m K) on the HTR-10 bed at 300 K), which must give
    /// [`TampinesError::Unphysical`]. Two in-domain controls
    /// (emissivity exactly 1.0, and the nominal HTR-10 case) must succeed.
    ///
    /// **Results (measured 2026-08-11).** All eight violations returned the
    /// expected variant, e.g. porosity 0 gives
    /// `InvalidInput("ZBS: porosity must be in (0,1), got 0")` and
    /// k_s = k_f = 1 gives `Unphysical(... N=-1.054756..., kappa=1,
    /// B=2.054756 ...)`. Both controls returned finite positive
    /// conductivities.
    #[test]
    fn out_of_domain_inputs_are_rejected() {
        let d = Length::new::<meter>(0.06);
        let e_ok = Ratio::new::<ratio>(0.8);
        let ks = wmk(26.0);
        let kf = wmk(0.15);
        let t = kelv(300.0);

        let invalid: [(
            &str,
            ZbsBed,
            ThermalConductivity,
            ThermalConductivity,
            ThermodynamicTemperature,
        ); 8] = [
            (
                "porosity 0",
                ZbsBed::new(Ratio::new::<ratio>(0.0), d, e_ok),
                ks,
                kf,
                t,
            ),
            (
                "porosity 1",
                ZbsBed::new(Ratio::new::<ratio>(1.0), d, e_ok),
                ks,
                kf,
                t,
            ),
            (
                "emissivity 0",
                ZbsBed::new(Ratio::new::<ratio>(0.39), d, Ratio::new::<ratio>(0.0)),
                ks,
                kf,
                t,
            ),
            (
                "emissivity 1.5",
                ZbsBed::new(Ratio::new::<ratio>(0.39), d, Ratio::new::<ratio>(1.5)),
                ks,
                kf,
                t,
            ),
            ("negative k_s", ZbsBed::htr10(), wmk(-26.0), kf, t),
            ("zero k_f", ZbsBed::htr10(), ks, wmk(0.0), t),
            ("zero temperature", ZbsBed::htr10(), ks, kf, kelv(0.0)),
            ("negative temperature", ZbsBed::htr10(), ks, kf, kelv(-5.0)),
        ];
        for (name, bed, s, f, temp) in invalid {
            let result = bed.effective_conductivity(s, f, temp);
            println!("{name}: {:?}", result.as_ref().err());
            assert!(
                matches!(result, Err(TampinesError::InvalidInput(_))),
                "{name} should be rejected as InvalidInput, got {result:?}"
            );
        }

        // kappa < B: the unit cell leaves its valid region.
        let degenerate = ZbsBed::htr10().effective_conductivity(wmk(1.0), wmk(1.0), t);
        println!("kappa < B: {:?}", degenerate.as_ref().err());
        assert!(
            matches!(degenerate, Err(TampinesError::Unphysical(_))),
            "kappa < B should be reported as Unphysical, got {degenerate:?}"
        );

        // In-domain controls.
        let unit_emissivity = ZbsBed::new(Ratio::new::<ratio>(0.39), d, Ratio::new::<ratio>(1.0))
            .effective_conductivity(ks, kf, t)
            .expect("emissivity of exactly 1.0 is in domain");
        assert!(unit_emissivity.get::<watt_per_meter_kelvin>() > 0.0);
        let nominal = ZbsBed::htr10()
            .effective_conductivity(ks, kf, t)
            .expect("nominal HTR-10 case is in domain");
        assert!(nominal.get::<watt_per_meter_kelvin>() > 0.0);
    }
}
