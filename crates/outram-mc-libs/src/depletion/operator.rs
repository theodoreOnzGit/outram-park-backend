//! # Burnup loop (predictor / forward-Euler integrator)
//!
//! Couples cross-section data to the transmutation solver to evolve a fuel
//! inventory over burnup steps, mirroring OpenMC's
//! `openmc/deplete/coupled_operator.py` + `openmc/deplete/integrators.py`
//! `PredictorIntegrator` (MIT) at a reduced fidelity documented below.
//!
//! ## Algorithm (predictor / forward Euler)
//!
//! For each burnup step of length `dt`:
//! 1. Evaluate one-group microscopic cross sections `sigma_f`, `sigma_gamma`
//!    for every chain nuclide from the `njoy-outram-park-fork` provider
//!    ([`Nuclide::from_core`] + [`Nuclide::xs_at_energy`]).
//! 2. Normalise the scalar flux so the fission power in the modelled fuel volume
//!    equals the requested power (`P = flux * V * sum_j N_j sigma_f_j Q_j`).
//! 3. Freeze the reaction rates over the step (the "predictor" assumption) and
//!    assemble the burnup matrix `A` from the [`DepletionChain`].
//! 4. Advance the densities `N(t+dt) = exp(A dt) N(t)` with order-16 CRAM
//!    ([`super::cram::cram16`]).
//! 5. Record the inventory and a one-group infinite-medium `k_inf` estimate.
//!
//! ## Fidelity caveats (honest scope — READ THIS)
//!
//! This is a **one-group, infinite-medium** burnup demonstration, not a
//! spectrum-resolved transport-coupled depletion:
//!
//! * Reaction rates use cross sections evaluated at a **single thermal energy**
//!   (0.0253 eV by default), not a transport-tallied multi-group flux spectrum.
//!   So resonance self-shielding and the fast/epithermal parts of the spectrum
//!   are not represented.
//! * `k_inf` is the one-group ratio `sum(N nu sigma_f) / sum(N sigma_a)` over the
//!   **chain nuclides only** (diluents/moderator/clad are omitted, since a
//!   one-group infinite-medium estimate ignores moderation anyway). It is a
//!   *relative trend* indicator, **not** comparable in absolute value to the
//!   notebook's continuous-energy pin-cell Monte Carlo `k`.
//! * A true transport-coupled `k` (matching the notebook's ~1.46 absolute
//!   values) needs a moderated pin-cell geometry and spectrum-averaged
//!   multi-group rates — tracked as a follow-up (see the V&V doc / bead).
//!
//! What this loop *does* verify against the notebook is the **physically-required
//! trends**: U-235 depletes monotonically, the fission products (I-135, Xe-135,
//! Cs-135) build up, Xe-135 rises then saturates (xenon poisoning), and `k_inf`
//! falls. See [`crate::depletion`] and the `depletion` verification test.
//!
//! A real Monte Carlo `k` on the evolved actinide inventory is available via
//! [`mc_keff_of_actinide_sphere`] to demonstrate the transport path is wired.

use super::chain::DepletionChain;
use super::cram::cram16;
use super::{MicroRate, ReactionRates};
use crate::material::material::{Material, NuclideComponent};
use crate::material::nuclide::Nuclide;
use crate::physics::keff::{run_keff, KeffResult, KeffSettings};

/// 1 barn expressed in cm² (`sigma[cm^2] = sigma[barn] * BARN_CM2`).
const BARN_CM2: f64 = 1.0e-24;
/// Electron-volt in joules (CODATA 2018 exact).
const EV_TO_JOULE: f64 = 1.602_176_634e-19;
/// Seconds per day.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// Thermal-fission recoverable energy `Q` per fissioning actinide, in eV.
///
/// Values transcribed from OpenMC `examples/pincell_depletion/chain_simple.xml`
/// (MIT) — the same `Q` the `depletion.ipynb` notebook uses. Any actinide not
/// listed falls back to the U-235 value.
fn fission_q_ev(name: &str) -> f64 {
    match name {
        "U234" => 191_840_000.0,
        "U235" => 193_410_000.0,
        "U238" => 197_790_000.0,
        _ => 193_410_000.0,
    }
}

/// Settings for a [`deplete_predictor`] burnup run.
///
/// Defaults mirror the `depletion.ipynb` notebook case: a 4.25%-enriched UO₂ pin
/// at 174 W (unit height), six 30-day steps.
#[derive(Debug, Clone, Copy)]
pub struct BurnupSettings {
    /// Total thermal fission power in the modelled fuel volume \[W\].
    pub power_watts: f64,
    /// Modelled fuel volume \[cm³\] the power is produced in.
    pub fuel_volume_cm3: f64,
    /// Length of each burnup step \[days\].
    pub step_days: f64,
    /// Number of burnup steps.
    pub n_steps: usize,
    /// Data/lookup temperature \[K\] for the cross-section evaluation.
    pub temperature_k: f64,
    /// One-group energy \[eV\] at which cross sections are evaluated
    /// (0.0253 eV = 2200 m/s thermal point by default).
    pub one_group_energy_ev: f64,
}

impl Default for BurnupSettings {
    fn default() -> Self {
        // Notebook: linear power 174 W/cm; fuel radius 0.42 cm ⇒ volume per unit
        // height π r² = 0.5542 cm³; six 30-day steps for a six-month cycle.
        Self {
            power_watts: 174.0,
            fuel_volume_cm3: std::f64::consts::PI * 0.42 * 0.42,
            step_days: 30.0,
            n_steps: 6,
            temperature_k: 293.6,
            one_group_energy_ev: 0.0253,
        }
    }
}

/// One-group microscopic cross sections \[barn\] for a chain nuclide.
#[derive(Debug, Clone, Copy, Default)]
struct OneGroupXs {
    /// Fission σ_f \[barn\].
    fission: f64,
    /// Radiative capture σ_(n,γ) = σ_a − σ_f \[barn\].
    gamma: f64,
    /// Fission production ν̄·σ_f \[barn\] (for the k_inf numerator).
    nu_fission: f64,
    /// Absorption σ_a = capture + fission \[barn\] (for the k_inf denominator).
    absorption: f64,
}

/// The inventory and reactor state recorded at one burnup step.
#[derive(Debug, Clone)]
pub struct BurnupStep {
    /// Step index (0 = beginning of life, before any depletion).
    pub step: usize,
    /// Cumulative burnup time at this point \[days\].
    pub time_days: f64,
    /// One-group scalar flux over the step just taken \[neutrons/(cm²·s)\]
    /// (0.0 at the beginning-of-life record).
    pub flux: f64,
    /// One-group infinite-medium multiplication factor `k_inf` (chain nuclides
    /// only — a relative trend indicator, see the module fidelity caveats).
    pub k_inf: f64,
    /// Nuclide inventory, `(name, atom_density)` in atoms/(barn·cm), in chain order.
    pub densities: Vec<(String, f64)>,
}

impl BurnupStep {
    /// The atom density \[atoms/(barn·cm)\] of `nuclide` at this step, or 0.0 if
    /// the nuclide is not tracked.
    pub fn density(&self, nuclide: &str) -> f64 {
        self.densities
            .iter()
            .find(|(n, _)| n == nuclide)
            .map(|(_, d)| *d)
            .unwrap_or(0.0)
    }
}

/// The full trajectory of a [`deplete_predictor`] run: one [`BurnupStep`] per
/// recorded point (beginning-of-life plus one per burnup step).
#[derive(Debug, Clone)]
pub struct BurnupResult {
    /// Recorded steps, `steps[0]` is beginning-of-life.
    pub steps: Vec<BurnupStep>,
}

impl BurnupResult {
    /// The beginning-of-life (fresh fuel) record.
    pub fn bol(&self) -> &BurnupStep {
        &self.steps[0]
    }

    /// The end-of-life (final) record.
    pub fn eol(&self) -> &BurnupStep {
        self.steps
            .last()
            .expect("at least the BOL record is always present")
    }

    /// The time history `(time_days, atom_density)` of one nuclide across every
    /// recorded step — the primary comparison quantity against the notebook.
    pub fn history(&self, nuclide: &str) -> Vec<(f64, f64)> {
        self.steps
            .iter()
            .map(|s| (s.time_days, s.density(nuclide)))
            .collect()
    }
}

/// Evaluate the one-group cross sections for each chain nuclide from the
/// `njoy-outram-park-fork` CORE provider.
///
/// Returns a vector aligned with `chain.nuclide_names()`. A nuclide the provider
/// does not carry is treated as cross-section-free (all zeros) and noted by the
/// caller; every `chain_simple` nuclide is in the CORE set, so in practice this
/// never happens for the default chain.
fn one_group_cross_sections(chain: &DepletionChain, settings: &BurnupSettings) -> Vec<OneGroupXs> {
    chain
        .nuclide_names()
        .iter()
        .map(|name| match Nuclide::from_core(name) {
            Ok(nuc) => {
                let xs = nuc.xs_at_energy(settings.one_group_energy_ev, settings.temperature_k);
                let gamma = (xs.absorption - xs.fission).max(0.0);
                OneGroupXs {
                    fission: xs.fission,
                    gamma,
                    nu_fission: xs.nu_fission,
                    absorption: xs.absorption,
                }
            }
            Err(_) => OneGroupXs::default(),
        })
        .collect()
}

/// The flux \[neutrons/(cm²·s)\] that makes the fission power in `fuel_volume_cm3`
/// equal `power_watts`, given the current densities and one-group fission data.
///
/// `P = flux * V * sum_j N_j sigma_f_j Q_j` with `N` in atoms/(barn·cm),
/// `sigma_f` in barn (so `N sigma_f` is a macroscopic 1/cm), `Q` in joules, and
/// `V` in cm³. Returns 0.0 if there is no fissile material left.
fn flux_for_power(
    names: &[&str],
    densities: &[f64],
    xs: &[OneGroupXs],
    settings: &BurnupSettings,
) -> f64 {
    let mut energy_per_flux = 0.0; // W per unit flux, before the volume factor
    for i in 0..names.len() {
        let q_joule = fission_q_ev(names[i]) * EV_TO_JOULE;
        // N_j [atoms/(barn·cm)] * sigma_f [barn] = macroscopic [1/cm];
        // times Q [J] gives J/cm per unit flux (× flux[1/cm²/s] × V[cm³] = W).
        energy_per_flux += densities[i] * xs[i].fission * q_joule;
    }
    let denom = energy_per_flux * settings.fuel_volume_cm3;
    if denom <= 0.0 {
        0.0
    } else {
        settings.power_watts / denom
    }
}

/// One-group infinite-medium `k_inf = sum(N nu sigma_f) / sum(N sigma_a)` over
/// the chain nuclides (relative trend indicator — see module fidelity caveats).
fn k_inf(densities: &[f64], xs: &[OneGroupXs]) -> f64 {
    let mut production = 0.0;
    let mut absorption = 0.0;
    for i in 0..densities.len() {
        production += densities[i] * xs[i].nu_fission;
        absorption += densities[i] * xs[i].absorption;
    }
    if absorption <= 0.0 {
        0.0
    } else {
        production / absorption
    }
}

/// Build the frozen one-group [`ReactionRates`] for a step from the flux and the
/// per-nuclide cross sections (`rate[1/s] = flux * sigma[barn] * 1e-24`).
fn reaction_rates(names: &[&str], flux: f64, xs: &[OneGroupXs]) -> ReactionRates {
    let mut rates = ReactionRates {
        flux,
        ..ReactionRates::zero()
    };
    for i in 0..names.len() {
        rates.set(
            names[i],
            MicroRate {
                gamma: flux * xs[i].gamma * BARN_CM2,
                fission: flux * xs[i].fission * BARN_CM2,
                n2n: 0.0, // LOW-tier WMP reports no (n,2n); negligible thermal.
            },
        );
    }
    rates
}

/// Run a predictor (forward-Euler) burnup calculation on `chain`, starting from
/// the inventory `initial` (`(nuclide_name, atom_density)` in atoms/(barn·cm)).
///
/// Nuclides in `initial` that are not in `chain` are ignored; chain nuclides
/// absent from `initial` start at zero density. Returns one [`BurnupStep`] per
/// recorded point (beginning-of-life plus `settings.n_steps` steps).
///
/// This is the honest, one-group demonstration described in the module docs; the
/// transmutation step itself (CRAM) is verified to analytic accuracy in
/// [`super::cram`], and the inventory *trends* are checked against the notebook
/// in the `depletion` verification test.
pub fn deplete_predictor(
    chain: &DepletionChain,
    initial: &[(String, f64)],
    settings: &BurnupSettings,
) -> BurnupResult {
    let names: Vec<&str> = chain.nuclide_names();
    let xs = one_group_cross_sections(chain, settings);

    // Densities aligned to chain order.
    let mut densities = vec![0.0_f64; names.len()];
    for (name, dens) in initial {
        if let Some(idx) = chain.index_of(name) {
            densities[idx] = *dens;
        }
    }

    let record = |step: usize, time_days: f64, flux: f64, densities: &[f64]| BurnupStep {
        step,
        time_days,
        flux,
        k_inf: k_inf(densities, &xs),
        densities: names
            .iter()
            .zip(densities)
            .map(|(n, d)| (n.to_string(), *d))
            .collect(),
    };

    let mut steps = Vec::with_capacity(settings.n_steps + 1);
    steps.push(record(0, 0.0, 0.0, &densities));

    let dt_seconds = settings.step_days * SECONDS_PER_DAY;
    for step in 1..=settings.n_steps {
        let flux = flux_for_power(&names, &densities, &xs, settings);
        let rates = reaction_rates(&names, flux, &xs);
        let matrix = chain.build_matrix(&rates);
        densities = cram16(&matrix, &densities, dt_seconds);
        // Guard against tiny negative densities from round-off in the linear solves.
        for d in &mut densities {
            if *d < 0.0 {
                *d = 0.0;
            }
        }
        let time_days = step as f64 * settings.step_days;
        steps.push(record(step, time_days, flux, &densities));
    }

    BurnupResult { steps }
}

/// Run a real Monte Carlo `k_eff` power iteration on a **bare sphere** of the
/// given actinide/fission-product inventory — a demonstration that the evolved
/// depletion inventory feeds straight back into the transport kernel.
///
/// `inventory` is `(nuclide_name, atom_density)` in atoms/(barn·cm). Only
/// nuclides the CORE provider carries are included. `radius_cm` sets the sphere
/// size. **This is a fast-spectrum bare sphere**, so the absolute `k` is far
/// below a moderated pin cell's — it is not comparable to the notebook's `k`;
/// its value is that it exercises the genuine MC transport path on a depleted
/// inventory. Returns the [`KeffResult`].
pub fn mc_keff_of_actinide_sphere(
    inventory: &[(String, f64)],
    radius_cm: f64,
    settings: &KeffSettings,
) -> KeffResult {
    let mut nuclides: Vec<Nuclide> = Vec::new();
    let mut components: Vec<NuclideComponent> = Vec::new();
    for (name, dens) in inventory {
        if let Ok(nuc) = Nuclide::from_core(name) {
            components.push(NuclideComponent {
                nuclide_idx: nuclides.len(),
                atom_density: *dens,
            });
            nuclides.push(nuc);
        }
    }
    let material = Material {
        id: 1,
        name: "depleted_fuel".to_string(),
        components,
        temperature: settings.temperature_k,
    };
    run_keff(radius_cm, &material, &nuclides, settings)
}
