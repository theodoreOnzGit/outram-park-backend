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
//! to the classic Zehner-Schlunder stagnant-bed form — a limit the tests
//! exercise.
//!
//! **Transcription caveat (honesty per `RESPONSIBLE_USE.md`).** The
//! dimensionless form above was implemented without page-level access to
//! the printed originals in this session. The falsifiable checks below
//! (uniform-medium collapse, exact Zehner-Schlunder degeneration, Wiener
//! series/parallel bounds, radiation monotonicity and `d`-scaling) all
//! pass, but a human should still verify the transcription against van
//! Antwerpen et al. (2010) eqs. (12)-(16) before this module is promoted
//! past Prototype in the V&V pipeline.
//!
//! ## The VTB 18-point table is NOT reproduced — documented finding
//!
//! The Virtual Test Bed generic pebble-bed deck
//! (`reference-data/virtual_test_bed/htgr/generic-pbr/pbr.i`, block
//! `keff_pebble_bed`, Open tier) carries an 18-point `k_eff` tabulation
//! (300-2000 K, 11.94-44.95 W/(m K)) described there as "calculated from
//! the ZBS correlation". This implementation, evaluated at that deck's own
//! stated inputs (eps = 0.39, d = 0.06 m, e_r = 0.8, graphite k_s = 26
//! W/(m K), helium at 6 MPa), does **not** reproduce it — and no
//! ZBS-family evaluation with a gas in the pores can: the table's 11.94
//! W/(m K) at 300 K would require a pore-fluid conductivity near 4 W/(m K)
//! (25 times helium's ~0.16), and stagnant-helium graphite beds measure
//! order 1-2 W/(m K) at ambient in SANA/HTTU-class experiments. The full
//! quantitative comparison is recorded in
//! [`tests::vtb_table_is_not_reproduced_by_zbs_with_helium`], and the
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
//! solid database), pressure drop ([`super::kta`]), pebble-internal
//! conduction ([`super::pebble`]).

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
    /// For the HTR-10 bed (eps = 0.39, C = 1.25) this is about 2.06.
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
    /// module docs for the equations and provenance).
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
    /// the solid conducts *worse* than about twice the fluid, `kappa < B`,
    /// with negligible radiation — not a gas-cooled-bed regime).
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
        let kr = self.radiation_ratio(fluid_conductivity, temperature).get::<ratio>();

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
