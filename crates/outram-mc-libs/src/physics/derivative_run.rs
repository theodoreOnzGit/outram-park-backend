// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence; see
// LICENSE.openmc): src/tallies/derivative.cpp (`score_track_derivative`,
// `score_collision_derivative`, `apply_derivative_to_score`).

//! **Tally derivatives wired into the transport** (gh:#683, #263).
//!
//! One run gives a tally score and its derivative with respect to a material
//! perturbation (density or nuclide density), by accumulating the logarithmic
//! flux derivative along each history: a **track term** per flight
//! ([`TallyDerivative::accumulate_along_track`]) and a **collision term** at each
//! real collision ([`TallyDerivative::accumulate_at_collision`]). The score of a
//! segment is multiplied by the accumulated derivative plus the score's own
//! explicit dependence (`1/rho` for a density derivative of a reaction rate;
//! `sigma_i / Sigma` for a nuclide-density derivative of a material total;
//! none for the flux).
//!
//! It is built on the transport kernel's [`TraceEvent`] observer, so the kernel
//! and its random stream are untouched: the histories are those of the
//! underlying driver bit for bit. The collision term of a segment's ending
//! collision is added **before** that segment is scored, because the segment's
//! length was sampled from a density that contains the collision.
//!
//! **Scope.** Surface tracking, analog. Scores: flux, total, absorption,
//! fission, scatter, and nu-fission for a DENSITY derivative (a nuclide-density
//! derivative of nu-fission is refused: `MicroXS` has no nu-fission). ~~Cross sections are the infinitely dilute ones
//! (`Material::macro_xs`), not the sampled URR bands.~~ **CORRECTED 2026-11-20
//! (gh:#263):** the score, the density and nuclide-density track terms and the
//! explicit term use the URR bands the kernel followed (`TraceEvent::Located`
//! carries the flight's `urr_seed`; `Material::macro_xs_urr`). The temperature
//! finite difference still differences the ordinary cross sections. Temperature derivatives
//! use a central finite difference of the nuclides' cross sections over the
//! temperature argument (OpenMC's multipole derivative, the same quantity) and
//! are available only where the cross sections depend on it (the WMP `Core`
//! tier); on the pointwise tier the derivative is exactly zero and the run
//! reports that as an error rather than a result. **Eigenvalue runs** differentiate the
//! tally for a FIXED fission source, as OpenMC's differential tallies do: the
//! change of the source distribution itself is not included, and an (n,xn)
//! secondary, which the kernel starts as a new particle, does not inherit its
//! parent's accumulated derivative (a fixed-source fission secondary does).

use crate::geometry::geometry::Geometry;
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::physics::fixed_source::{FixedSource, FixedSourceSettings};
use crate::physics::keff::KeffSettings;
use crate::physics::tracking_trace::TraceEvent;
use crate::physics::transport_csg::{CsgPowerIteration, Site, SourceBox};
use crate::tally::derivative::{DerivativeVariable, FluxDerivative, TallyDerivative};
use crate::tally::tally::ScoreType;

/// What to differentiate: a tally derivative, the score, and the material the
/// score is accumulated in.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivativeSpec {
    /// The perturbation.
    pub derivative: TallyDerivative,
    /// The score (flux, total, absorption, fission or scatter).
    pub score: ScoreType,
    /// The material whose flights are scored.
    pub score_material: usize,
}

/// A score and its derivative, per source particle, with the standard error of
/// each over the batches.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivativeResult {
    /// The score per source particle.
    pub flux: f64,
    /// Standard error of [`Self::flux`] over the batches.
    pub flux_std: f64,
    /// `d score / d p` per source particle.
    pub d_flux: f64,
    /// Standard error of [`Self::d_flux`] over the batches.
    pub d_flux_std: f64,
    /// Batches the statistics are over.
    pub n_batches: usize,
}

/// The microscopic cross sections nuclide `idx` presents to the kernel at `e`:
/// its **sampled unresolved-resonance band** (the draw `urr_xi(idx, urr_seed)`) when
/// the energy is in its unresolved range with probability tables on, else the
/// ordinary ones (gh:#263, as `Material::macro_xs_urr`).
fn micro_urr(nuclides: &[Nuclide], idx: usize, e: f64, temperature: f64, urr_seed: u64) -> crate::material::nuclide::MicroXS {
    let nuc = &nuclides[idx];
    if nuc.needs_urr_draw(e) {
        nuc.xs_at_energy_urr(e, temperature, crate::material::material::urr_xi(idx, urr_seed))
    } else {
        nuc.xs_at_energy(e, temperature)
    }
}

/// `(d Sigma_s/dT, d Sigma_a/dT, d Sigma_f/dT)` of `material` at `e` \[cm^-1 K^-1\]: a
/// central finite difference of each nuclide's microscopic cross sections over
/// `+-TEMPERATURE_STEP_K` times its atom density (OpenMC uses the multipole
/// derivative, `evaluate_deriv`; the finite difference is the same quantity,
/// `O(dT^2)`). All three are exactly zero on a tier whose cross sections do not
/// depend on the temperature argument (the pointwise tier).
fn dxs_dt(material: &Material, nuclides: &[Nuclide], e: f64) -> (f64, f64, f64) {
    let h = TallyDerivative::TEMPERATURE_STEP_K;
    let t = material.temperature;
    let (mut ds, mut da, mut df) = (0.0, 0.0, 0.0);
    for c in &material.components {
        let nuc = &nuclides[c.nuclide_idx];
        let (lo, hi) = (nuc.xs_at_energy(e, t - h), nuc.xs_at_energy(e, t + h));
        ds += c.atom_density * ((hi.total - hi.absorption) - (lo.total - lo.absorption)) / (2.0 * h);
        da += c.atom_density * (hi.absorption - lo.absorption) / (2.0 * h);
        df += c.atom_density * (hi.fission - lo.fission) / (2.0 * h);
    }
    (ds, da, df)
}

/// Per-history accumulation state, driven by [`TraceEvent`]s.
struct Accum {
    fd: FluxDerivative,
    err: Option<String>,
    cur_mat: Option<usize>,
    /// The URR stream seed of the current flight (from `Located`).
    urr_seed: u64,
    e_cur: f64,
    /// A collision-limited segment waiting for its collision:
    /// `(material, d * Sigma_x score value, explicit term)`.
    pending: Option<(usize, f64, f64)>,
    sum: f64,
    dsum: f64,
    /// A temperature derivative saw a non-zero `dSigma/dT` at least once.
    t_dep: bool,
}

impl Accum {
    fn new() -> Self {
        Self { fd: FluxDerivative::default(), err: None, cur_mat: None, urr_seed: 0, e_cur: 0.0, pending: None, sum: 0.0, dsum: 0.0, t_dep: false }
    }

    fn flush(&mut self, spec: &DerivativeSpec) {
        if let Some((m, s, explicit)) = self.pending.take() {
            if m == spec.score_material {
                self.sum += s;
                self.dsum += s * (self.fd.value + explicit);
            }
        }
    }

    /// `(score value of a flight of length d, explicit derivative term)`.
    fn segment_score(
        spec: &DerivativeSpec,
        material: &Material,
        nuclides: &[Nuclide],
        e: f64,
        d: f64,
        urr_seed: u64,
    ) -> Result<(f64, f64), String> {
        if spec.score == ScoreType::Flux {
            return Ok((d, 0.0));
        }
        let x = material.macro_xs_urr(e, nuclides, urr_seed);
        let sigma_x = match spec.score {
            ScoreType::Total => x.total,
            ScoreType::Absorption => x.absorption,
            ScoreType::Fission => x.fission,
            ScoreType::NuFission => x.nu_fission,
            ScoreType::Scatter => (x.total - x.absorption).max(0.0),
            other => return Err(format!("a derivative of {other:?} is not supported here")),
        };
        let explicit = match spec.derivative.variable {
            DerivativeVariable::Density => {
                let rho: f64 = material.components.iter().map(|c| c.atom_density).sum();
                if rho > 0.0 { 1.0 / rho } else { 0.0 }
            }
            DerivativeVariable::NuclideDensity { .. } if spec.score == ScoreType::NuFission => {
                return Err("a nuclide-density derivative of nu-fission is not supported: MicroXS carries no nu-fission".into())
            }
            DerivativeVariable::NuclideDensity { nuclide_idx } => {
                let micro = micro_urr(nuclides, nuclide_idx, e, material.temperature, urr_seed);
                let sigma_i = match spec.score {
                    ScoreType::Total => micro.total,
                    ScoreType::Absorption => micro.absorption,
                    ScoreType::Fission => micro.fission,
                    _ => (micro.total - micro.absorption).max(0.0),
                };
                if sigma_x > 0.0 { sigma_i / sigma_x } else { 0.0 }
            }
            DerivativeVariable::Temperature => {
                let (ds, da, df) = dxs_dt(material, nuclides, e);
                let d_x = match spec.score {
                    ScoreType::Total => ds + da,
                    ScoreType::Absorption => da,
                    ScoreType::Fission => df,
                    _ => ds,
                };
                if sigma_x > 0.0 { d_x / sigma_x } else { 0.0 }
            }
        };
        Ok((d * sigma_x, explicit))
    }

    fn event(&mut self, ev: TraceEvent, spec: &DerivativeSpec, materials: &[Material], nuclides: &[Nuclide]) {
        match ev {
            TraceEvent::Start { e, .. } => {
                self.flush(spec);
                self.e_cur = e;
            }
            TraceEvent::Located { material, urr_seed, .. } => {
                self.flush(spec);
                self.cur_mat = material;
                self.urr_seed = urr_seed;
            }
            TraceEvent::Segment { d_collision, d_boundary, .. } => {
                let d = d_collision.min(d_boundary);
                if let (Some(m), true) = (self.cur_mat, d.is_finite() && d > 0.0) {
                    if matches!(spec.derivative.variable, DerivativeVariable::Temperature) {
                        // Track term: -d * dSigma_t/dT of the perturbed material.
                        if m == spec.derivative.material_idx {
                            let (ds, da, _) = dxs_dt(&materials[m], nuclides, self.e_cur);
                            self.t_dep |= ds != 0.0 || da != 0.0;
                            self.fd.value -= d * (ds + da);
                        }
                    } else if m == spec.derivative.material_idx {
                        // Track term with the SAMPLED cross sections the flight used
                        // (gh:#263): density -d * Sigma_t / rho; nuclide density
                        // -d * sigma_t,i.
                        match spec.derivative.variable {
                            DerivativeVariable::Density => {
                                let rho: f64 = materials[m].components.iter().map(|c| c.atom_density).sum();
                                if rho > 0.0 {
                                    let st = materials[m].macro_xs_urr(self.e_cur, nuclides, self.urr_seed).total;
                                    self.fd.value -= d * st / rho;
                                } else {
                                    self.err.get_or_insert(format!("material {m} has zero atom density"));
                                }
                            }
                            DerivativeVariable::NuclideDensity { nuclide_idx } => {
                                let st = micro_urr(nuclides, nuclide_idx, self.e_cur, materials[m].temperature, self.urr_seed).total;
                                self.fd.value -= d * st;
                            }
                            DerivativeVariable::Temperature => {}
                        }
                    }
                    match Self::segment_score(spec, &materials[m], nuclides, self.e_cur, d, self.urr_seed) {
                        Ok((s, explicit)) => {
                            if d_collision <= d_boundary {
                                self.pending = Some((m, s, explicit));
                            } else if m == spec.score_material {
                                self.sum += s;
                                self.dsum += s * (self.fd.value + explicit);
                            }
                        }
                        Err(e) => {
                            self.err.get_or_insert(e);
                        }
                    }
                }
            }
            TraceEvent::Collision { material, nuclide, e, .. } => {
                if matches!(spec.derivative.variable, DerivativeVariable::Temperature) {
                    // Collision term (OpenMC `score_collision_derivative`):
                    // (dSigma_s/dT) / Sigma_s of the whole material.
                    if material == spec.derivative.material_idx {
                        let (ds, _, _) = dxs_dt(&materials[material], nuclides, e);
                        let x = materials[material].macro_xs(e, nuclides);
                        let sig_s = (x.total - x.absorption).max(0.0);
                        if sig_s > 0.0 {
                            self.fd.value += ds / sig_s;
                        }
                    }
                } else if let Err(er) = spec.derivative.accumulate_at_collision(&mut self.fd, material, &materials[material], nuclides, nuclide, e) {
                    self.err.get_or_insert(er);
                }
                self.flush(spec);
                self.e_cur = e;
            }
            TraceEvent::State(st) => {
                self.flush(spec);
                self.e_cur = st.energy;
            }
            _ => {}
        }
    }
}

fn check(spec: &DerivativeSpec) -> Result<(), String> {
    if !matches!(spec.score, ScoreType::Flux | ScoreType::Total | ScoreType::Absorption | ScoreType::Fission | ScoreType::NuFission | ScoreType::Scatter) {
        return Err(format!("a derivative of {:?} is not supported here", spec.score));
    }
    Ok(())
}

/// A temperature derivative on cross sections that never moved with the
/// temperature argument is exactly zero, not a result (the pointwise tier).
fn temperature_dependence_check(spec: &DerivativeSpec, t_dep: bool) -> Result<(), String> {
    if matches!(spec.derivative.variable, DerivativeVariable::Temperature) && !t_dep {
        return Err("the cross sections of this material do not depend on the temperature argument (pointwise tier, or outside the windowed-multipole range): a temperature derivative is exactly zero there, not a physical result. Use the WMP (Core) tier.".into());
    }
    Ok(())
}

fn stats(v: &[f64]) -> (f64, f64) {
    let m = v.iter().sum::<f64>() / v.len().max(1) as f64;
    let var = if v.len() > 1 { v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64 } else { 0.0 };
    (m, (var / v.len().max(1) as f64).sqrt())
}

fn finish(score_b: &[f64], d_b: &[f64]) -> DerivativeResult {
    let (flux, flux_std) = stats(score_b);
    let (d_flux, d_flux_std) = stats(d_b);
    DerivativeResult { flux, flux_std, d_flux, d_flux_std, n_batches: score_b.len() }
}

/// A [`DerivativeSpec`] score and its derivative from one **fixed-source** run
/// (see the module docs). A fission secondary inherits its parent's accumulated
/// derivative.
///
/// # Errors
///
/// A temperature derivative, an unsupported score, or an accumulator error.
pub fn run_fixed_source_derivative(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    source: &FixedSource,
    settings: &FixedSourceSettings,
    spec: &DerivativeSpec,
) -> Result<DerivativeResult, String> {
    check(spec)?;
    let n = settings.n_particles.max(1);
    let n_batches = settings.n_batches.max(1);
    let (base, rem) = (n / n_batches, n % n_batches);
    let mut seed = settings.seed;
    let (mut score_b, mut d_b) = (Vec::with_capacity(n_batches), Vec::with_capacity(n_batches));
    let mut dummy: Vec<f64> = Vec::new();
    let mut t_dep = false;
    for b in 0..n_batches {
        let this_batch = base + usize::from(b < rem);
        let (mut sum, mut dsum) = (0.0, 0.0);
        for _ in 0..this_batch {
            let (birth, birth_w) = source.sample(&mut seed);
            let mut hseed = seed;
            let mut bank = vec![birth];
            let mut is_source = true;
            let mut acc = Accum::new();
            while let Some(site) = bank.pop() {
                let mut next: Vec<Site> = Vec::new();
                crate::physics::transport_csg::transport_history_vr_urr(
                    site, geom, materials, nuclides, &[], 1.0, &mut next, &mut hseed, None, &mut dummy,
                    &[], &mut [], None, &settings.variance_reduction,
                    crate::physics::transport_csg::DeltaTallyEstimator::default(),
                    None, None, None, if is_source { birth_w } else { 1.0 }, None,
                    &mut |ev| acc.event(ev, spec, materials, nuclides),
                );
                is_source = false;
                bank.extend(next);
            }
            if let Some(e) = acc.err {
                return Err(e);
            }
            t_dep |= acc.t_dep;
            sum += acc.sum;
            dsum += acc.dsum;
            seed = hseed;
        }
        let w = this_batch.max(1) as f64;
        score_b.push(sum / w);
        d_b.push(dsum / w);
    }
    temperature_dependence_check(spec, t_dep)?;
    Ok(finish(&score_b, &d_b))
}

/// A [`DerivativeSpec`] score and its derivative from a **k-eigenvalue** run, over
/// the active generations, per source particle: each generation's histories are
/// transported by [`CsgPowerIteration`] and observed. The derivative is for a
/// fixed fission source (module docs).
///
/// # Errors
///
/// A temperature derivative, an unsupported score, or an accumulator error.
pub fn run_keff_derivative(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    source_box: SourceBox,
    settings: &KeffSettings,
    spec: &DerivativeSpec,
) -> Result<DerivativeResult, String> {
    check(spec)?;
    let mut it = CsgPowerIteration::new(geom, materials, nuclides, source_box, settings);
    let (mut score_b, mut d_b) = (Vec::new(), Vec::new());
    let mut gen = 0usize;
    let n_per = settings.n_particles.max(1) as f64;
    let mut t_dep = false;
    loop {
        let mut acc = Accum::new();
        let mut first = true;
        let report = it.step_traced(geom, materials, nuclides, |ev| {
            // Every Start is a new particle; a derivative does not carry across.
            if matches!(ev, TraceEvent::Start { .. }) && !first {
                acc.fd = FluxDerivative::default();
            }
            first = false;
            acc.event(ev, spec, materials, nuclides);
        });
        if report.is_none() {
            break;
        }
        if let Some(e) = acc.err {
            return Err(e);
        }
        t_dep |= acc.t_dep;
        if gen >= settings.n_inactive {
            score_b.push(acc.sum / n_per);
            d_b.push(acc.dsum / n_per);
        }
        gen += 1;
    }
    if score_b.is_empty() {
        return Err("no active generation ran".into());
    }
    temperature_dependence_check(spec, t_dep)?;
    Ok(finish(&score_b, &d_b))
}
