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
use njoy_outram_park_fork::groupr::{AnalyticWeight, ThermalFissionParams};

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
#[derive(Debug, Clone)]
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
    /// One-group energy \[eV\] at which cross sections are evaluated when
    /// [`Self::weighting`] is [`OneGroupWeighting::SingleEnergy`]
    /// (0.0253 eV = 2200 m/s thermal point by default).
    pub one_group_energy_ev: f64,
    /// How the one-group cross sections are formed — see
    /// [`OneGroupWeighting`]. Defaults to
    /// [`OneGroupWeighting::SingleEnergy`], which is what this module did
    /// unconditionally before 2026-09-16.
    pub weighting: OneGroupWeighting,
}

/// How [`BurnupSettings`] forms its one-group cross sections.
///
/// # Why this exists
///
/// Depletion needs `σ` averaged over the flux the fuel actually sees. Until
/// 2026-09-16 this module evaluated every cross section at a **single energy**
/// (0.0253 eV by default) and called the result one-group. For a purely
/// thermal spectrum that is defensible; for anything else it is not, and it
/// misses **resonance absorption entirely** — U-238's capture cross section is
/// ~2.7 b at 0.0253 eV, while its flux-weighted value in a real lattice is
/// several times that, because the resonance integral dominates.
///
/// The survey in `docs/neutronics-physics-coverage.md` recorded this as a
/// coupling gap rather than a fidelity knob, and it is: a burnup calculation
/// whose one-group data is wrong depletes the wrong nuclides.
#[derive(Debug, Clone, PartialEq)]
pub enum OneGroupWeighting {
    /// Evaluate at [`BurnupSettings::one_group_energy_ev`] and nowhere else.
    ///
    /// The historical behaviour, kept as the default so existing results
    /// reproduce **bit-identically**. Correct only for a spectrum concentrated
    /// at that one energy.
    SingleEnergy,
    /// Collapse `σ_g = ∫σ(E)φ(E)dE / ∫φ(E)dE` against NJOY's **`iwt = 4`**
    /// analytic weighting spectrum: a Maxwellian thermal peak, a `1/E` slowing-
    /// down region, and a fission-spectrum fast tail.
    ///
    /// Reuses [`AnalyticWeight::ThermalFission`] from `njoy-outram-park-fork`'s
    /// GROUPR port rather than re-deriving the shape — it is the same weight
    /// NJOY uses to collapse multigroup libraries, already ported and tested.
    ///
    /// The integral is a log-spaced trapezoid over `[e_min_ev, e_max_ev]` with
    /// `n_points` nodes. It is **not** a transport-calculated flux: it is a
    /// representative spectrum, which is a large improvement on a single point
    /// and still an approximation. Collapsing against the *actual* flux from a
    /// transport solve is the remaining step.
    ThermalFissionSpectrum {
        /// Lower integration bound \[eV\].
        e_min_ev: f64,
        /// Upper integration bound \[eV\].
        e_max_ev: f64,
        /// Number of log-spaced quadrature nodes.
        n_points: usize,
    },
    /// Collapse against a **flux spectrum the caller measured**, typically
    /// tallied from a transport solve of the actual geometry.
    ///
    /// This is the variant the other two are approximations of. The analytic
    /// `iwt = 4` shape is a *representative* spectrum — it knows nothing about
    /// this geometry's moderation, leakage, or self-shielding — and
    /// `SingleEnergy` knows nothing at all. A tallied spectrum carries the
    /// resonance dips that make U-238's effective capture several times its
    /// 0.0253 eV value, which is the whole reason the one-group data matters.
    ///
    /// # Fields
    ///
    /// - `e_grid_ev` — ascending group **boundaries** \[eV\], length `n + 1`.
    /// - `phi` — the flux in each group, length `n`. Units are arbitrary and
    ///   cancel; only the *shape* is used.
    ///
    /// # How the collapse treats a group
    ///
    /// `sigma_g = sum_g phi_g * sigma_bar_g / sum_g phi_g`, with `sigma_bar_g`
    /// the cross section averaged across the group on a log-spaced sub-grid.
    /// Averaging within the group rather than evaluating at its midpoint
    /// matters wherever a resonance sits inside one — which, on any practical
    /// group structure, is most of the resolved range.
    TabulatedFlux {
        /// Ascending group boundaries \[eV\]; length is `phi.len() + 1`.
        e_grid_ev: Vec<f64>,
        /// Flux per group (arbitrary units — only the shape is used).
        phi: Vec<f64>,
        /// Sub-samples per group used to average `sigma` inside it. `1` reduces
        /// to evaluating at the group's geometric midpoint.
        sub_points: usize,
    },
}

impl OneGroupWeighting {
    /// NJOY's standard `iwt = 4` breakpoints: thermal Maxwellian below
    /// 0.1 eV at `kT = 0.025` eV, `1/E` up to 820.3 keV, then a fission
    /// Maxwellian at `T = 1.4` MeV (`groupr.f90`'s documented defaults).
    pub fn thermal_fission_default() -> Self {
        Self::ThermalFissionSpectrum {
            e_min_ev: 1.0e-5,
            e_max_ev: 2.0e7,
            n_points: 2000,
        }
    }
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
            // Unchanged default: existing results reproduce bit-identically.
            weighting: OneGroupWeighting::SingleEnergy,
        }
    }
}

/// One-group microscopic cross sections \[barn\] for a chain nuclide.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OneGroupXs {
    /// Fission σ_f \[barn\].
    pub fission: f64,
    /// Radiative capture σ_(n,γ) = σ_a − σ_f \[barn\].
    pub gamma: f64,
    /// Fission production ν̄·σ_f \[barn\] (for the k_inf numerator).
    pub nu_fission: f64,
    /// Absorption σ_a = capture + fission \[barn\] (for the k_inf denominator).
    pub absorption: f64,
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
pub fn one_group_cross_sections(chain: &DepletionChain, settings: &BurnupSettings) -> Vec<OneGroupXs> {
    chain
        .nuclide_names()
        .iter()
        .map(|name| match Nuclide::from_core(name) {
            Ok(nuc) => collapse_one_group(&nuc, settings),
            Err(_) => OneGroupXs::default(),
        })
        .collect()
}

/// Form one nuclide's one-group cross sections under `settings.weighting`.
///
/// For [`OneGroupWeighting::SingleEnergy`] this is a single
/// [`Nuclide::xs_at_energy`] call — bit-identical to what this module did
/// before the weighting option existed.
///
/// For [`OneGroupWeighting::ThermalFissionSpectrum`] it is
/// `σ_g = ∫σ(E)φ(E)dE / ∫φ(E)dE` by log-spaced trapezoid, with `φ` the
/// GROUPR `iwt = 4` analytic spectrum. Integrating in `ln E` is deliberate:
/// the `1/E` region is flat in lethargy, so a log grid resolves the whole
/// range with a tractable node count where a linear grid would waste every
/// node above 1 keV.
fn collapse_one_group(nuc: &Nuclide, settings: &BurnupSettings) -> OneGroupXs {
    let t = settings.temperature_k;
    match &settings.weighting {
        OneGroupWeighting::SingleEnergy => {
            let xs = nuc.xs_at_energy(settings.one_group_energy_ev, t);
            OneGroupXs {
                fission: xs.fission,
                gamma: (xs.absorption - xs.fission).max(0.0),
                nu_fission: xs.nu_fission,
                absorption: xs.absorption,
            }
        }
        OneGroupWeighting::ThermalFissionSpectrum {
            e_min_ev,
            e_max_ev,
            n_points,
        } => {
            let (e_min_ev, e_max_ev, n_points) = (*e_min_ev, *e_max_ev, *n_points);
            let weight = AnalyticWeight::ThermalFission(ThermalFissionParams {
                thermal_break_ev: 0.1,
                thermal_temp_ev: 0.025,
                fission_break_ev: 8.203e5,
                fission_temp_ev: 1.4e6,
            });
            let n = n_points.max(2);
            let ln_lo = e_min_ev.max(1.0e-11).ln();
            let ln_hi = e_max_ev.ln();
            let (mut num, mut den) = ([0.0f64; 4], 0.0f64);
            let (mut prev_e, mut prev_w, mut prev_x) = (0.0f64, 0.0f64, [0.0f64; 4]);
            for i in 0..n {
                let e = (ln_lo + (ln_hi - ln_lo) * i as f64 / (n - 1) as f64).exp();
                let xs = nuc.xs_at_energy(e, t);
                let x = [
                    xs.fission,
                    (xs.absorption - xs.fission).max(0.0),
                    xs.nu_fission,
                    xs.absorption,
                ];
                // phi(E) dE = phi(E) * E * d(ln E): the Jacobian of the log grid.
                let w = weight.evaluate(e, t) * e;
                if i > 0 {
                    let dlog = e.ln() - prev_e.ln();
                    den += 0.5 * dlog * (w + prev_w);
                    for k in 0..4 {
                        num[k] += 0.5 * dlog * (w * x[k] + prev_w * prev_x[k]);
                    }
                }
                prev_e = e;
                prev_w = w;
                prev_x = x;
            }
            if den > 0.0 {
                OneGroupXs {
                    fission: num[0] / den,
                    gamma: num[1] / den,
                    nu_fission: num[2] / den,
                    absorption: num[3] / den,
                }
            } else {
                OneGroupXs::default()
            }
        }
        OneGroupWeighting::TabulatedFlux {
            e_grid_ev,
            phi,
            sub_points,
        } => {
            // A malformed spectrum collapses to nothing rather than to a
            // plausible wrong number: `phi` must have exactly one entry per
            // group, and there must be at least one group.
            if e_grid_ev.len() < 2 || phi.len() + 1 != e_grid_ev.len() {
                return OneGroupXs::default();
            }
            let sub = (*sub_points).max(1);
            let (mut num, mut den) = ([0.0f64; 4], 0.0f64);
            for (g, &w) in phi.iter().enumerate() {
                if !(w > 0.0) {
                    continue;
                }
                let (lo, hi) = (e_grid_ev[g], e_grid_ev[g + 1]);
                if !(hi > lo) || lo <= 0.0 {
                    continue;
                }
                // Average sigma across the group on a log grid -- the same
                // reasoning as the analytic path: the 1/E region is flat in
                // lethargy, so log sub-sampling resolves a group evenly.
                let (ln_lo, ln_hi) = (lo.ln(), hi.ln());
                let mut acc = [0.0f64; 4];
                for i in 0..sub {
                    let f = (i as f64 + 0.5) / sub as f64;
                    let e = (ln_lo + (ln_hi - ln_lo) * f).exp();
                    let xs = nuc.xs_at_energy(e, t);
                    acc[0] += xs.fission;
                    acc[1] += (xs.absorption - xs.fission).max(0.0);
                    acc[2] += xs.nu_fission;
                    acc[3] += xs.absorption;
                }
                den += w;
                for k in 0..4 {
                    num[k] += w * acc[k] / sub as f64;
                }
            }
            if den > 0.0 {
                OneGroupXs {
                    fission: num[0] / den,
                    gamma: num[1] / den,
                    nu_fission: num[2] / den,
                    absorption: num[3] / den,
                }
            } else {
                OneGroupXs::default()
            }
        }
    }
}

/// The flux \[neutrons/(cm²·s)\] that makes the fission power in `fuel_volume_cm3`
/// equal `power_watts`, given the current densities and one-group fission data.
///
/// `P = flux * V * sum_j N_j sigma_f_j Q_j` with `N` in atoms/(barn·cm),
/// `sigma_f` in barn (so `N sigma_f` is a macroscopic 1/cm), `Q` in joules, and
/// `V` in cm³. Returns 0.0 if there is no fissile material left.
pub fn flux_for_power(
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
pub fn k_inf(densities: &[f64], xs: &[OneGroupXs]) -> f64 {
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
pub fn reaction_rates(names: &[&str], flux: f64, xs: &[OneGroupXs]) -> ReactionRates {
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

/// Run a burnup calculation in which the **flux spectrum is recomputed at every
/// step** from the current inventory, rather than frozen at beginning of life.
///
/// # Why this exists
///
/// [`deplete_predictor`] computes the one-group cross sections **once**, before
/// the loop. That is correct only if the spectrum does not move — and the whole
/// point of depletion is that it does: fissile material burns out, fission
/// products with large thermal absorption build in, and the spectrum hardens.
/// Cross sections collapsed against a beginning-of-life spectrum therefore drift
/// further from the truth at every step, and the error compounds because the
/// inventory they produce is the input to the next step.
///
/// `spectrum` is called before each step with the current inventory as
/// `(nuclide, atom density \[atoms/(barn*cm)\])` pairs, and returns
/// `(group boundaries \[eV\], flux per group)` — exactly what an
/// [`crate::tally::filter::EnergyFilter`] tally produces from a transport solve.
/// Returning `None` keeps the previous step's spectrum, which is what a caller
/// should do when a solve fails rather than silently substituting a different
/// weighting.
///
/// # The coupling this does and does not do
///
/// This is an **operator-splitting** (predictor) coupling: transport is solved
/// on the inventory at the start of a step, the resulting one-group data is held
/// constant across that step, and the inventory is advanced with CRAM. It is
/// **not** a predictor-corrector scheme — there is no second transport solve at
/// the end of the step and no averaging of the two — so it carries the usual
/// first-order splitting error in the step length. Shortening `step_days` is
/// what controls that, and a caller who needs more should say so rather than
/// assume this is second order.
///
/// `settings.weighting` is **overridden** for each step by the returned
/// spectrum; whatever it holds on entry is used only if `spectrum` returns
/// `None` on the very first call.
///
/// # Call count: `n_steps`, not `n_steps + 1`
///
/// `spectrum` is called once at beginning of life and then once before each of
/// steps `2..=n_steps` — `n_steps` calls in total. Step 1 reuses the
/// beginning-of-life spectrum because it starts from the **same inventory**, so
/// re-solving would repeat an identical calculation at the cost of a full
/// transport run. The arithmetic looks off by one until that is spelled out,
/// which is why it is.
///
/// # Generic, not a trait object
///
/// `spectrum` is an `impl FnMut` — a monomorphised parameter, not a
/// `Box<dyn Fn>` — per the workspace's Rust design rules, the same way
/// `physics::search` takes its predicate.
pub fn deplete_coupled(
    chain: &DepletionChain,
    initial: &[(String, f64)],
    settings: &BurnupSettings,
    mut spectrum: impl FnMut(&[(String, f64)]) -> Option<(Vec<f64>, Vec<f64>)>,
) -> BurnupResult {
    let names: Vec<&str> = chain.nuclide_names();

    let mut densities = vec![0.0_f64; names.len()];
    for (name, dens) in initial {
        if let Some(idx) = chain.index_of(name) {
            densities[idx] = *dens;
        }
    }
    let inventory = |d: &[f64]| -> Vec<(String, f64)> {
        names
            .iter()
            .zip(d)
            .map(|(n, x)| (n.to_string(), *x))
            .collect()
    };

    let mut step_settings = settings.clone();
    let mut xs = one_group_cross_sections(chain, &step_settings);

    let record =
        |step: usize, time_days: f64, flux: f64, d: &[f64], xs: &[OneGroupXs]| BurnupStep {
            step,
            time_days,
            flux,
            k_inf: k_inf(d, xs),
            densities: names
                .iter()
                .zip(d)
                .map(|(n, x)| (n.to_string(), *x))
                .collect(),
        };

    let mut steps = Vec::with_capacity(settings.n_steps + 1);
    // Beginning of life uses the spectrum of the INITIAL inventory, so the
    // recorded k_inf at step 0 is the one the transport solve actually saw.
    if let Some((e_grid_ev, phi)) = spectrum(&inventory(&densities)) {
        step_settings.weighting = OneGroupWeighting::TabulatedFlux {
            e_grid_ev,
            phi,
            sub_points: DEFAULT_GROUP_SUB_POINTS,
        };
        xs = one_group_cross_sections(chain, &step_settings);
    }
    steps.push(record(0, 0.0, 0.0, &densities, &xs));

    let dt_seconds = settings.step_days * SECONDS_PER_DAY;
    for step in 1..=settings.n_steps {
        // Re-solve the spectrum on the inventory entering this step. This is the
        // line that makes the coupling real; `deplete_predictor` has no analogue
        // of it.
        if step > 1 {
            if let Some((e_grid_ev, phi)) = spectrum(&inventory(&densities)) {
                step_settings.weighting = OneGroupWeighting::TabulatedFlux {
                    e_grid_ev,
                    phi,
                    sub_points: DEFAULT_GROUP_SUB_POINTS,
                };
                xs = one_group_cross_sections(chain, &step_settings);
            }
        }
        let flux = flux_for_power(&names, &densities, &xs, settings);
        let rates = reaction_rates(&names, flux, &xs);
        let matrix = chain.build_matrix(&rates);
        densities = cram16(&matrix, &densities, dt_seconds);
        for d in &mut densities {
            if *d < 0.0 {
                *d = 0.0;
            }
        }
        let time_days = step as f64 * settings.step_days;
        steps.push(record(step, time_days, flux, &densities, &xs));
    }

    BurnupResult { steps }
}

/// Sub-samples per energy group used when collapsing against a tallied flux.
///
/// Eight is enough that a group spanning a decade is sampled every ~0.29
/// lethargy units, which resolves the slowly varying part of `sigma` without
/// pretending to resolve individual resonances — a group structure that puts a
/// resonance inside a group has already made that decision.
const DEFAULT_GROUP_SUB_POINTS: usize = 8;

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
