//! Step 10's coupled solve: an r-z multi-channel porous-core march with a
//! prescribed power shape and lumped temperature feedback.
//!
//! ```text
//!   helium in (top plenum, T_in, p_out)          common plenum: every ring
//!   ┌──────┬──────┬──────┬──────┬──────┐          sees the same Δp
//!   │ ring │ ring │ ring │ ring │ ring │  ↓ m_i
//!   │  0   │  1   │  2   │  3   │  4   │  each node (i, j):
//!   │      │      │      │      │      │    h_out = h_in + Q_ij / m_i     (energy)
//!   │  ↓   │  ↓   │  ↓   │  ↓   │  ↓   │    T_s  = T_he + q_peb / (h π d²) (Wakao h)
//!   │      │      │      │      │      │    T_c, T_kernel from the pebble's
//!   │      │      │      │      │      │      two-zone conduction + TRISO
//!   └──────┴──────┴──────┴──────┴──────┘    Δp_ij = KTA(m_i, ρ, μ) dz − ρ g dz
//!   r = 0                     r = R (adiabatic wall)
//!   helium out (bottom plenum): rings mixed, then bypass mixed in at T_in
//! ```
//!
//! **What is solved (each item is listed in the Step 9 element list).**
//! The pebble bed is split into equal-area rings; each ring is a 1-D
//! channel marched in the flow direction with exact enthalpy bookkeeping
//! (helium `(p, h)` flash, `tampines::gas_phase::properties`, which adapts
//! the `outram-park-fork-coolprop` helium EOS). The pebble-to-helium film
//! uses `tampines::pebble_bed::PackedBedConvection` (Wakao); the pebble
//! interior uses `tampines::pebble_bed::Pebble::htr10` (two-zone conduction
//! plus the hottest TRISO particle at the pebble centre); the bed friction
//! uses `tampines::gas_phase::kta_bed::KtaBed` (KTA 3102.3). The **outer
//! (coupling) iteration** redistributes the core flow among the rings until
//! they all see the same plenum-to-plenum pressure drop, re-evaluating
//! every property and temperature, and re-evaluates the lumped reactivity
//! feedback each pass.
//!
//! **What is not (stated loudly in the UI):** the power shape is
//! *prescribed* (not a neutronics solve); no radial conduction or
//! radiation between rings (ZBS effective conductivity is not used); no
//! heat crosses the side wall (adiabatic); the reflector, bypass channels
//! and plenums are not modelled thermally (the bypass is mixed at the inlet
//! temperature); properties are evaluated at the outlet pressure; k is a
//! lumped isothermal-coefficient estimate, not an eigenvalue.

use dhoby_ghaut::workbench::multiphysics::{FeedbackWeighting, MultiphysicsSetup, PowerShape};
use tampines::gas_phase::kta_bed::KtaBed;
use tampines::gas_phase::properties::{helium_state, helium_state_ph};
use tampines::pebble_bed::{PackedBedConvection, Pebble};
use uom::si::area::square_meter;
use uom::si::f64::{
    Area, AvailableEnergy as SpecificEnthalpy, Length, MassRate, Power, Pressure, Ratio,
    ThermodynamicTemperature, Velocity,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;
use uom::si::available_energy::joule_per_kilogram;

/// First zero of J0.
const J0_ZERO: f64 = 2.404_825_557_695_773;
/// Standard gravity \[m/s²\].
const G: f64 = 9.806_65;

/// J0(x) and J1(x) by their power series (|x| <= J0_ZERO here, where 30
/// terms are exact to machine precision).
fn bessel_j0_j1(x: f64) -> (f64, f64) {
    let h = 0.5 * x;
    let (mut j0, mut j1) = (0.0, 0.0);
    let mut t0 = 1.0; // (-1)^k (x/2)^{2k} / (k!)^2
    for k in 0..30 {
        let kf = k as f64;
        j0 += t0;
        j1 += t0 * h / (kf + 1.0);
        t0 *= -h * h / ((kf + 1.0) * (kf + 1.0));
    }
    (j0, j1)
}

/// Node fields, ring-major (`i * n_z + j`, `j = 0` at the top).
#[derive(Debug, Clone, Default)]
pub struct Fields {
    pub n_r: usize,
    pub n_z: usize,
    /// Ring outer radii \[m\].
    pub ring_outer_m: Vec<f64>,
    /// Core height \[m\].
    pub height_m: f64,
    /// Power density \[W/m³\].
    pub q_w_m3: Vec<f64>,
    /// Helium, node mean \[K\].
    pub t_helium_k: Vec<f64>,
    /// Fuel-pebble surface \[K\].
    pub t_surface_k: Vec<f64>,
    /// Fuel-pebble centre \[K\].
    pub t_centre_k: Vec<f64>,
    /// Hottest kernel centre (TRISO at the pebble centre) \[K\].
    pub t_kernel_k: Vec<f64>,
    /// Fuel-pebble volume average \[K\].
    pub t_pebble_avg_k: Vec<f64>,
    /// Particle Reynolds number.
    pub reynolds: Vec<f64>,
}

/// One outer iteration, streamed to the UI.
#[derive(Debug, Clone)]
pub struct IterationReport {
    pub iteration: usize,
    /// (max − min) / mean of the ring pressure drops.
    pub flow_residual: f64,
    /// Largest temperature change of any node since the last pass \[K\].
    pub temperature_change_k: f64,
    /// Ring mass flows \[kg/s\].
    pub ring_flow_kg_s: Vec<f64>,
    /// Ring plenum-to-plenum pressure drops \[Pa\].
    pub ring_dp_pa: Vec<f64>,
    /// Feedback temperature \[K\].
    pub t_feedback_k: f64,
    /// Reactivity relative to the cold reference \[pcm\].
    pub rho_pcm: f64,
    /// k relative to a cold-critical reference state.
    pub k_vs_cold_critical: f64,
    pub max_kernel_k: f64,
    pub max_helium_k: f64,
    pub core_exit_k: f64,
}

/// Converged (or stopped) result.
#[derive(Debug, Clone)]
pub struct Summary {
    pub converged: bool,
    pub iterations: usize,
    /// Mixed bed exit \[°C\].
    pub core_exit_c: f64,
    /// Bed exit mixed with the bypass \[°C\].
    pub vessel_outlet_c: f64,
    /// Hottest helium (a ring's node outlet) \[°C\].
    pub max_helium_c: f64,
    /// Hottest kernel \[°C\].
    pub max_kernel_c: f64,
    /// Hottest fuel-pebble surface \[°C\].
    pub max_surface_c: f64,
    /// Hottest pebble centre \[°C\].
    pub max_centre_c: f64,
    /// Power-weighted mean fuel-pebble temperature \[°C\].
    pub mean_fuel_pebble_c: f64,
    /// Bed pressure drop, plenum to plenum \[kPa\].
    pub dp_kpa: f64,
    /// Mean superficial velocity at the bed inlet \[m/s\].
    pub inlet_superficial_velocity_m_s: f64,
    /// Largest superficial velocity at a ring exit \[m/s\].
    pub max_exit_superficial_velocity_m_s: f64,
    /// Bed porosity (to turn superficial into interstitial velocity).
    pub porosity: f64,
    /// Feedback temperature and reactivity.
    pub t_feedback_c: f64,
    pub rho_pcm: f64,
    pub k_vs_cold_critical: f64,
    /// Heat carried off by the helium / thermal power − 1.
    pub energy_balance_rel: f64,
    /// Extrapolation length the shape was solved for \[cm\].
    pub shape_delta_cm: f64,
    /// Peak / mean power density of the shape as built.
    pub peak_to_mean: f64,
    /// Nodes whose particle Re is outside Wakao's [15, 8500].
    pub wakao_out_of_range: usize,
    pub re_min: f64,
    pub re_max: f64,
}

/// The solver state.
pub struct PorousCore {
    setup: MultiphysicsSetup,
    n_r: usize,
    n_z: usize,
    ring_area_m2: Vec<f64>,
    dz_m: f64,
    /// Node power \[W\].
    node_power_w: Vec<f64>,
    ring_flow: Vec<f64>,
    /// The flows the last march used (the update then moves `ring_flow`).
    march_flow: Vec<f64>,
    fields: Fields,
    ring_dp: Vec<f64>,
    ring_exit_h: Vec<f64>,
    h_in: f64,
    p_pa: f64,
    iteration: usize,
    delta_m: f64,
    peak_to_mean: f64,
    pebble: Pebble,
    convection: PackedBedConvection,
}

fn tk(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}

impl PorousCore {
    /// Set up the grid, the power shape and the initial (area-proportional)
    /// flow split.
    ///
    /// # Errors
    ///
    /// A sentence on an impossible input.
    pub fn new(setup: &MultiphysicsSetup) -> Result<Self, String> {
        let f = &setup.foam;
        let n = &setup.neutronics;
        let (n_r, n_z) = (f.radial_rings.max(1), f.axial_nodes.max(2));
        let r = f.core_radius_cm / 100.0;
        let h = f.core_height_cm / 100.0;
        if r <= 0.0 || h <= 0.0 {
            return Err("core radius and height must be positive".into());
        }
        if !(0.0 < f.filling_fraction && f.filling_fraction < 1.0) {
            return Err("filling fraction must lie in (0, 1)".into());
        }
        if !(0.0 < f.fuel_pebble_fraction && f.fuel_pebble_fraction <= 1.0) {
            return Err("fuel pebble fraction must lie in (0, 1]".into());
        }
        if f.core_mass_flow_kg_s <= 0.0 || f.core_mass_flow_kg_s > f.total_mass_flow_kg_s {
            return Err("core mass flow must be positive and at most the total flow".into());
        }
        if n.thermal_power_mw <= 0.0 {
            return Err("thermal power must be positive".into());
        }
        // Equal-area rings.
        let ring_outer_m: Vec<f64> = (1..=n_r)
            .map(|i| r * (i as f64 / n_r as f64).sqrt())
            .collect();
        let area = std::f64::consts::PI * r * r;
        let ring_area_m2 = vec![area / n_r as f64; n_r];
        let dz_m = h / n_z as f64;
        // Shape.
        let (delta_m, node_shape) = match n.shape {
            PowerShape::Uniform => (f64::INFINITY, vec![1.0; n_r * n_z]),
            PowerShape::J0Cosine { peak_to_mean } => {
                let max = Self::peak_to_mean_of(r, h, 0.0);
                if !(1.0 < peak_to_mean && peak_to_mean < max) {
                    return Err(format!(
                        "peak-to-mean {peak_to_mean} is outside what a J0 x cosine shape gives (1, {max:.3})"
                    ));
                }
                // Bisection on the extrapolation length (peak/mean falls as it grows).
                let (mut lo, mut hi) = (0.0, 1.0e3);
                for _ in 0..200 {
                    let mid = 0.5 * (lo + hi);
                    if Self::peak_to_mean_of(r, h, mid) > peak_to_mean {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let d = 0.5 * (lo + hi);
                (d, Self::node_integrals(r, h, d, &ring_outer_m, n_z))
            }
        };
        let total: f64 = node_shape.iter().sum();
        let p_w = n.thermal_power_mw * 1e6;
        let node_power_w: Vec<f64> = node_shape.iter().map(|s| p_w * s / total).collect();
        let peak_to_mean = if delta_m.is_finite() {
            Self::peak_to_mean_of(r, h, delta_m)
        } else {
            1.0
        };
        let p_pa = f.outlet_pressure_mpa * 1e6;
        let st_in = helium_state(
            tk(f.inlet_temperature_c + 273.15),
            Pressure::new::<pascal>(p_pa),
        )
        .map_err(|e| format!("inlet helium state: {e:?}"))?;
        let h_in = st_in.specific_enthalpy.get::<joule_per_kilogram>();
        let ring_flow = vec![f.core_mass_flow_kg_s / n_r as f64; n_r];
        let porosity = Ratio::new::<ratio>(1.0 - f.filling_fraction);
        let convection =
            PackedBedConvection::new(Length::new::<meter>(f.pebble_diameter_cm / 100.0), porosity)
                .map_err(|e| format!("packed bed: {e:?}"))?;
        let nn = n_r * n_z;
        let mut fields = Fields {
            n_r,
            n_z,
            ring_outer_m: ring_outer_m.clone(),
            height_m: h,
            q_w_m3: node_power_w
                .iter()
                .enumerate()
                .map(|(k, q)| q / (ring_area_m2[k / n_z] * dz_m))
                .collect(),
            ..Default::default()
        };
        for v in [
            &mut fields.t_helium_k,
            &mut fields.t_surface_k,
            &mut fields.t_centre_k,
            &mut fields.t_kernel_k,
            &mut fields.t_pebble_avg_k,
            &mut fields.reynolds,
        ] {
            *v = vec![0.0; nn];
        }
        Ok(Self {
            setup: setup.clone(),
            n_r,
            n_z,
            ring_area_m2,
            dz_m,
            node_power_w,
            march_flow: ring_flow.clone(),
            ring_flow,
            fields,
            ring_dp: vec![0.0; n_r],
            ring_exit_h: vec![h_in; n_r],
            h_in,
            p_pa,
            iteration: 0,
            delta_m,
            peak_to_mean,
            pebble: Pebble::htr10(),
            convection,
        })
    }

    /// Peak / mean of `J0(2.405 r/R_e) cos(π (z − H/2)/H_e)` over the
    /// cylinder, `R_e = R + d`, `H_e = H + 2d`.
    fn peak_to_mean_of(r: f64, h: f64, d: f64) -> f64 {
        let re = r + d;
        let he = h + 2.0 * d;
        let a = J0_ZERO / re;
        let (_, j1) = bessel_j0_j1(a * r);
        let radial_mean = 2.0 * j1 / (a * r);
        let axial_mean =
            2.0 * he / (std::f64::consts::PI * h) * (std::f64::consts::PI * h / (2.0 * he)).sin();
        1.0 / (radial_mean * axial_mean)
    }

    /// Exact integrals of the shape over each node (ring-major).
    fn node_integrals(r: f64, h: f64, d: f64, ring_outer: &[f64], n_z: usize) -> Vec<f64> {
        let re = r + d;
        let he = h + 2.0 * d;
        let a = J0_ZERO / re;
        let radial = |x: f64| {
            if x <= 0.0 {
                0.0
            } else {
                2.0 * std::f64::consts::PI * x * bessel_j0_j1(a * x).1 / a
            }
        };
        let axial =
            |z: f64| he / std::f64::consts::PI * (std::f64::consts::PI * (z - 0.5 * h) / he).sin();
        let dz = h / n_z as f64;
        let mut out = Vec::with_capacity(ring_outer.len() * n_z);
        let mut r_in = 0.0;
        for &r_out in ring_outer {
            let rad = radial(r_out) - radial(r_in);
            for j in 0..n_z {
                out.push(rad * (axial((j + 1) as f64 * dz) - axial(j as f64 * dz)));
            }
            r_in = r_out;
        }
        out
    }

    /// The shape's extrapolation length \[m\] (infinite for uniform).
    pub fn delta_m(&self) -> f64 {
        self.delta_m
    }

    /// Node power \[W\] (ring-major).
    #[cfg(test)]
    pub fn node_power_w(&self) -> &[f64] {
        &self.node_power_w
    }

    /// Current fields.
    pub fn fields(&self) -> &Fields {
        &self.fields
    }

    /// March every ring once at the current flow split.
    fn march(&mut self) -> Result<(), String> {
        let f = self.setup.foam.clone();
        let p = Pressure::new::<pascal>(self.p_pa);
        let d = f.pebble_diameter_cm / 100.0;
        let v_peb = std::f64::consts::PI * d * d * d / 6.0;
        let a_peb = std::f64::consts::PI * d * d;
        let fluence = Ratio::new::<ratio>(f.fast_fluence_1e25_per_m2);
        let porosity = Ratio::new::<ratio>(1.0 - f.filling_fraction);
        self.march_flow = self.ring_flow.clone();
        for i in 0..self.n_r {
            let m = self.ring_flow[i];
            let area = self.ring_area_m2[i];
            let bed = KtaBed::new(
                porosity,
                Length::new::<meter>(d),
                Area::new::<square_meter>(area),
            );
            let mut h = self.h_in;
            let mut t_in = helium_state_ph(p, SpecificEnthalpy::new::<joule_per_kilogram>(h))
                .map_err(|e| format!("{e:?}"))?
                .temperature
                .get::<kelvin>();
            let mut dp = 0.0;
            let order: Vec<usize> = if f.flow_downward {
                (0..self.n_z).collect()
            } else {
                (0..self.n_z).rev().collect()
            };
            for j in order {
                let k = i * self.n_z + j;
                let q = self.node_power_w[k];
                let h_out = h + q / m;
                let t_out = helium_state_ph(p, SpecificEnthalpy::new::<joule_per_kilogram>(h_out))
                    .map_err(|e| format!("ring {i} node {j}: helium (p,h) flash: {e:?}"))?
                    .temperature
                    .get::<kelvin>();
                let t_mean = 0.5 * (t_in + t_out);
                let st =
                    helium_state(tk(t_mean), p).map_err(|e| format!("ring {i} node {j}: {e:?}"))?;
                let rho = st.density.get::<kilogram_per_cubic_meter>();
                let u = m / (area * rho);
                let re = self.convection.reynolds_number(
                    Velocity::new::<meter_per_second>(u),
                    st.density,
                    st.dynamic_viscosity,
                );
                let htc = self
                    .convection
                    .heat_transfer_coefficient(re, st.prandtl(), st.thermal_conductivity)
                    .map_err(|e| format!("ring {i} node {j}: Wakao: {e:?}"))?
                    .get::<watt_per_square_meter_kelvin>();
                let n_peb = f.filling_fraction * area * self.dz_m / v_peb;
                let q_peb = q / (n_peb * f.fuel_pebble_fraction);
                let t_s = t_mean + q_peb / (htc * a_peb);
                let prof = self
                    .pebble
                    .steady_state_temperatures(Power::new::<watt>(q_peb), tk(t_s), fluence)
                    .map_err(|e| format!("ring {i} node {j}: pebble conduction: {e:?}"))?;
                let avg = self
                    .pebble
                    .volume_average_temperature(&prof)
                    .get::<kelvin>();
                let grad = bed
                    .pressure_gradient(
                        MassRate::new::<kilogram_per_second>(m),
                        st.density,
                        st.dynamic_viscosity,
                    )
                    .map_err(|e| format!("ring {i} node {j}: KTA: {e:?}"))?
                    .value; // SI: Pa/m
                            // Plenum-to-plenum drop: friction, less the static head
                            // gained flowing down (added flowing up).
                let head = rho * G * self.dz_m;
                dp += grad * self.dz_m + if f.flow_downward { -head } else { head };
                let fl = &mut self.fields;
                fl.t_helium_k[k] = t_mean;
                fl.t_surface_k[k] = t_s;
                fl.t_centre_k[k] = prof.centre.get::<kelvin>();
                fl.t_kernel_k[k] = prof.peak_kernel_centre.get::<kelvin>();
                fl.t_pebble_avg_k[k] = avg;
                fl.reynolds[k] = re.get::<ratio>();
                h = h_out;
                t_in = t_out;
            }
            self.ring_dp[i] = dp;
            self.ring_exit_h[i] = h;
        }
        Ok(())
    }

    /// One outer (coupling) iteration: march at the current split, then
    /// move flow from rings with a high pressure drop to those with a low
    /// one. Returns the report of the march just done.
    ///
    /// # Errors
    ///
    /// A property, correlation or conduction failure, named by node.
    pub fn iterate(&mut self) -> Result<IterationReport, String> {
        let before = self.fields.t_kernel_k.clone();
        let before_he = self.fields.t_helium_k.clone();
        self.march()?;
        self.iteration += 1;
        let change = before
            .iter()
            .zip(&self.fields.t_kernel_k)
            .chain(before_he.iter().zip(&self.fields.t_helium_k))
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        let mean_dp = self.ring_dp.iter().sum::<f64>() / self.n_r as f64;
        let (lo, hi) = self
            .ring_dp
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        let flow_residual = if mean_dp.abs() > 0.0 {
            (hi - lo) / mean_dp.abs()
        } else {
            0.0
        };
        let report_flows = self.ring_flow.clone();
        let report_dp = self.ring_dp.clone();
        // Flow update: Δp ~ m^n with n in [1, 2]; the square-root update is
        // stable across both, and relaxed.
        if self.n_r > 1 {
            let w = self.setup.coupling.flow_relaxation.clamp(0.05, 1.0);
            let mut new: Vec<f64> = self
                .ring_flow
                .iter()
                .zip(&self.ring_dp)
                .map(|(m, dp)| m * (mean_dp / dp.max(1e-9)).max(0.0).sqrt())
                .collect();
            let s: f64 = new.iter().sum();
            let target = self.setup.foam.core_mass_flow_kg_s;
            for v in &mut new {
                *v *= target / s;
            }
            for (m, n) in self.ring_flow.iter_mut().zip(new) {
                *m = (1.0 - w) * *m + w * n;
            }
        }
        let (t_fb, rho) = self.feedback();
        let core_exit_k = self.core_exit_k()?;
        Ok(IterationReport {
            iteration: self.iteration,
            flow_residual,
            temperature_change_k: change,
            ring_flow_kg_s: report_flows,
            ring_dp_pa: report_dp,
            t_feedback_k: t_fb,
            rho_pcm: rho * 1e5,
            k_vs_cold_critical: 1.0 / (1.0 - rho),
            max_kernel_k: self
                .fields
                .t_kernel_k
                .iter()
                .copied()
                .fold(f64::MIN, f64::max),
            max_helium_k: self.max_helium_k()?,
            core_exit_k,
        })
    }

    /// Whether the last report meets the Step 9 tolerances.
    pub fn converged(&self, r: &IterationReport) -> bool {
        let c = &self.setup.coupling;
        r.iteration > 1
            && r.flow_residual < c.pressure_tolerance
            && r.temperature_change_k < c.temperature_tolerance_k
    }

    /// The bed temperature for feedback, and the reactivity it gives
    /// relative to the cold reference: `rho = alpha (T_fb − T_ref)`. The
    /// node temperature is the fuel pebbles' volume average mixed with the
    /// unfuelled pebbles (at the helium temperature) by count.
    fn feedback(&self) -> (f64, f64) {
        let n = &self.setup.neutronics;
        let ff = self.setup.foam.fuel_pebble_fraction;
        let (mut num, mut den) = (0.0, 0.0);
        for k in 0..self.n_r * self.n_z {
            let t = ff * self.fields.t_pebble_avg_k[k] + (1.0 - ff) * self.fields.t_helium_k[k];
            let w = match n.weighting {
                FeedbackWeighting::Power => self.node_power_w[k],
                FeedbackWeighting::Volume => self.ring_area_m2[k / self.n_z] * self.dz_m,
            };
            num += w * t;
            den += w;
        }
        let t_fb = num / den;
        (
            t_fb,
            n.isothermal_coefficient_per_k * (t_fb - n.reference_temperature_k),
        )
    }

    fn t_of_h(&self, h: f64) -> Result<f64, String> {
        Ok(helium_state_ph(
            Pressure::new::<pascal>(self.p_pa),
            SpecificEnthalpy::new::<joule_per_kilogram>(h),
        )
        .map_err(|e| format!("{e:?}"))?
        .temperature
        .get::<kelvin>())
    }

    fn core_exit_h(&self) -> f64 {
        let m: f64 = self.ring_flow_at_march().iter().sum();
        self.ring_flow_at_march()
            .iter()
            .zip(&self.ring_exit_h)
            .map(|(m, h)| m * h)
            .sum::<f64>()
            / m
    }

    fn ring_flow_at_march(&self) -> Vec<f64> {
        self.march_flow.clone()
    }

    fn core_exit_k(&self) -> Result<f64, String> {
        self.t_of_h(self.core_exit_h())
    }

    fn max_helium_k(&self) -> Result<f64, String> {
        let h = self.ring_exit_h.iter().copied().fold(f64::MIN, f64::max);
        self.t_of_h(h)
    }

    /// The result after the last iteration.
    ///
    /// # Errors
    ///
    /// A property failure.
    pub fn summary(&self, converged: bool) -> Result<Summary, String> {
        let f = &self.setup.foam;
        let c = |k: f64| k - 273.15;
        let flows = self.ring_flow_at_march();
        let m_core: f64 = flows.iter().sum();
        let h_core = self.core_exit_h();
        let m_tot = f.total_mass_flow_kg_s;
        let h_vessel =
            (m_core * h_core + (m_tot - m_core).max(0.0) * self.h_in) / m_tot.max(m_core);
        let p = Pressure::new::<pascal>(self.p_pa);
        let rho_in = helium_state(tk(f.inlet_temperature_c + 273.15), p)
            .map_err(|e| format!("{e:?}"))?
            .density
            .get::<kilogram_per_cubic_meter>();
        let area: f64 = self.ring_area_m2.iter().sum();
        let mut u_exit_max: f64 = 0.0;
        for (i, &h) in self.ring_exit_h.iter().enumerate() {
            let st = helium_state(tk(self.t_of_h(h)?), p).map_err(|e| format!("{e:?}"))?;
            u_exit_max = u_exit_max.max(
                flows[i] / (self.ring_area_m2[i] * st.density.get::<kilogram_per_cubic_meter>()),
            );
        }
        let p_w: f64 = self.node_power_w.iter().sum();
        let carried: f64 = flows
            .iter()
            .zip(&self.ring_exit_h)
            .map(|(m, h)| m * (h - self.h_in))
            .sum();
        let (t_fb, rho) = self.feedback();
        let fl = &self.fields;
        let max = |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max);
        let mean_fuel = fl
            .t_pebble_avg_k
            .iter()
            .zip(&self.node_power_w)
            .map(|(t, q)| t * q)
            .sum::<f64>()
            / p_w;
        let out_of_range = fl
            .reynolds
            .iter()
            .filter(|&&re| !(15.0..=8500.0).contains(&re))
            .count();
        let dp = self.ring_dp.iter().sum::<f64>() / self.n_r as f64;
        Ok(Summary {
            converged,
            iterations: self.iteration,
            core_exit_c: c(self.t_of_h(h_core)?),
            vessel_outlet_c: c(self.t_of_h(h_vessel)?),
            max_helium_c: c(self.max_helium_k()?),
            max_kernel_c: c(max(&fl.t_kernel_k)),
            max_surface_c: c(max(&fl.t_surface_k)),
            max_centre_c: c(max(&fl.t_centre_k)),
            mean_fuel_pebble_c: c(mean_fuel),
            dp_kpa: dp / 1e3,
            inlet_superficial_velocity_m_s: m_core / (area * rho_in),
            max_exit_superficial_velocity_m_s: u_exit_max,
            porosity: 1.0 - f.filling_fraction,
            t_feedback_c: c(t_fb),
            rho_pcm: rho * 1e5,
            k_vs_cold_critical: 1.0 / (1.0 - rho),
            energy_balance_rel: carried / p_w - 1.0,
            shape_delta_cm: self.delta_m * 100.0,
            peak_to_mean: self.peak_to_mean,
            wakao_out_of_range: out_of_range,
            re_min: fl.reynolds.iter().copied().fold(f64::MAX, f64::min),
            re_max: max(&fl.reynolds),
        })
    }
}

/// Run to convergence (or `max_iterations`, or `stop`), calling `each`
/// after every iteration.
///
/// # Errors
///
/// As [`PorousCore::iterate`].
pub fn solve(
    setup: &MultiphysicsSetup,
    stop: impl Fn() -> bool,
    mut each: impl FnMut(&IterationReport, &Fields),
) -> Result<(Summary, Fields), String> {
    let mut core = PorousCore::new(setup)?;
    let mut converged = false;
    for _ in 0..setup.coupling.max_iterations.max(1) {
        let r = core.iterate()?;
        each(&r, core.fields());
        if core.converged(&r) {
            converged = true;
            break;
        }
        if stop() {
            break;
        }
    }
    Ok((core.summary(converged)?, core.fields().clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// J0 and J1 against tabulated values (Abramowitz & Stegun Table 9.1):
    /// J0(1) = 0.7651976866, J1(1) = 0.4400505857, J0(2.4048) ≈ 0.
    #[test]
    fn bessel_series_matches_tables() {
        let (j0, j1) = bessel_j0_j1(1.0);
        assert!((j0 - 0.765_197_686_6).abs() < 1e-9);
        assert!((j1 - 0.440_050_585_7).abs() < 1e-9);
        assert!(bessel_j0_j1(J0_ZERO).0.abs() < 1e-12);
    }

    /// With no extrapolation the bare-cylinder peak/mean is the textbook
    /// 1/(2 J1(2.405)/2.405 · 2/π) = 2.3161 × 1.5708 = 3.638.
    #[test]
    fn bare_cylinder_peak_to_mean_is_the_textbook_value() {
        let p = PorousCore::peak_to_mean_of(0.9, 1.97, 0.0);
        assert!((p - 3.638).abs() < 2e-3, "{p}");
    }
}
