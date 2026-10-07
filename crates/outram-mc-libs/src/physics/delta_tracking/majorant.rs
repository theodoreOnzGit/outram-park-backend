//! **The majorant** `Σ_maj(E)`: the bound every delta-tracking flight is
//! sampled on, and the one property delta tracking cannot do without.
//!
//! # The invariant
//!
//! For every material `m` a region can present and every energy `E` a
//! neutron can have there,
//!
//! ```text
//! Σ_maj(E) ≥ Σ_t,m(E)
//! ```
//!
//! with `Σ_t` the total the flight's accept/reject step reads: the URR band
//! total when a URR stream is in use ([`Material::macro_xs_total_urr`]), the
//! S(α,β)-rebuilt total below a thermal cutoff, the Doppler-broadened total of
//! a multipole nuclide **at the material's temperature**. DBRC does not
//! change `Σ_t` (it reweights the elastic target velocity only), so it needs
//! no term here.
//!
//! **An under-bound is a silent bias, not an error.** The accept probability
//! `Σ_t/Σ_maj` saturates at 1 where the bound fails, so collisions there are
//! lost exactly where the cross section is largest. Every constructor below
//! is therefore built to bound by construction where the data allows it, and
//! [`Majorant::audit`] exists to check one against the data at far more
//! energies than it was built on.
//!
//! **The bound holds only for the material state it was built from.** A
//! majorant is a table of numbers; it does not know the temperatures or
//! densities it bounded. Change a multipole material's temperature, or a
//! density, after building one and the bound is no longer guaranteed
//! (`tests/delta_tracking_audit.rs`: a 293.6 K majorant is under the same
//! fuel at 1200 K by 1.34x at 6.5 eV). Rebuild it.
//!
//! Moved here from `pebble_beds::delta_tracking` on 2026-10-06 (gh:#599
//! audit); the old path re-exports everything.

use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::mathf::RealMath;

/// An energy-dependent majorant cross section `Σ_maj(E) ≥ Σ_t(E)` over a set of
/// materials — the sampling bound for delta tracking.
///
/// Stored as a tabulated `(energy [eV], Σ_maj [cm⁻¹])` curve on an ascending grid.
/// [`Self::at`] returns a value that is conservative (never below the true
/// tabulated majorant) by taking the larger of the two bracketing grid points, so
/// a coarse grid stays valid — it just loosens the bound (more virtual collisions).
#[derive(Debug, Clone, Default)]
pub struct Majorant {
    /// Ascending energy grid \[eV\].
    energy: Vec<f64>,
    /// Majorant Σ_maj \[cm⁻¹\] at each grid energy.
    sigma: Vec<f64>,
}

impl Majorant {
    /// A single flat majorant `sigma_max` \[cm⁻¹\] valid at all energies.
    ///
    /// The simplest bound: one conservative constant. Correct as long as
    /// `sigma_max ≥ Σ_t` everywhere the neutron can travel; looser than an
    /// energy-dependent majorant, so it produces more virtual collisions.
    pub fn uniform(sigma_max: f64) -> Self {
        Majorant {
            energy: vec![0.0, f64::INFINITY],
            sigma: vec![sigma_max, sigma_max],
        }
    }

    /// Build the majorant `Σ_maj(E) ≥ max_m Σ_t,m(E)` over `materials`, on the
    /// supplied energy grid **plus every breakpoint of every nuclide the
    /// materials use** ([`Nuclide::majorant_breakpoints`]), times `1 + margin`.
    ///
    /// Each node reads `Σ_t` at itself and one ulp either side, so a step in
    /// the data is inside the bound. Each node is also floored by the
    /// point-sampled majorant on `energies` alone (the pre-#589
    /// construction), so this is never below it. On pointwise and S(α,β)
    /// data `Σ_t` is linear or convex between breakpoints, and [`Self::at`]
    /// takes the larger bracketing node, so this is a bound by construction
    /// there. On WMP (LOW tier) data the pole peaks are nodes, but the shape
    /// between them is only sampled. See [`Self::bounding`] for the argument
    /// and `tests/majorant_bounds_endf.rs` for the pins.
    ///
    /// ~~For each grid energy this takes the maximum macroscopic total over
    /// every material, then multiplies by `1 + margin`~~ **CHANGED 2026-10-05
    /// (GitHub #589):** grid points alone under-bound resonance data, and by a
    /// lot. The HTR-10 bed majorant of `nee_soon`'s k-vs-height record (4096
    /// log points, margin 0.3, ENDF/B-VIII.0) left the UO₂ kernel's `Σ_t` at
    /// **14.0×** the majorant at 661 eV. The CORE-data `fhr_pebble_quickstart`
    /// (150 points, margin 0.05) was 97× under. The old construction is kept
    /// as the ablation [`Self::from_materials_without_breakpoints`].
    ///
    /// The grid is clipped to `[energies.first(), energies.last()]`. Below it,
    /// [`Self::at`] extrapolates as 1/v. Cost: about three `Σ_t` evaluations
    /// per material per node, and on ENDF data a few hundred thousand nodes.
    ///
    /// # Parameters
    /// - `materials` — every material the neutron might enter (fuel, matrix, …).
    /// - `nuclides` — the global nuclide array the materials index into.
    /// - `energies` — ascending energy grid \[eV\]. It sets the range and adds
    ///   nodes; the breakpoints are added on top.
    /// - `margin` — non-negative safety fraction added to each majorant value.
    pub fn from_materials(
        materials: &[Material],
        nuclides: &[Nuclide],
        energies: &[f64],
        margin: f64,
    ) -> Self {
        let floor = Self::from_materials_without_breakpoints(materials, nuclides, energies, 0.0);
        let (Some(&lo), Some(&hi)) = (energies.first(), energies.last()) else {
            return floor;
        };
        let scale = 1.0 + margin.max(0.0);
        let sigma_t_max = |e: f64| {
            materials
                .iter()
                .map(|m| m.macro_xs_total_upper_bound(e, nuclides))
                .fold(0.0_f64, f64::max)
        };
        let mut grid = energies.to_vec();
        for i in used_nuclides(materials) {
            if let Some(n) = nuclides.get(i) {
                grid.extend(n.majorant_breakpoints(lo, hi));
            }
        }
        grid.retain(|&e| e.is_finite() && e >= lo && e <= hi);
        grid.sort_by(|a, b| a.partial_cmp(b).expect("finite energies"));
        grid.dedup();
        let sigma = grid
            .iter()
            .map(|&e| {
                let own = sigma_t_max(e.next_down())
                    .max(sigma_t_max(e))
                    .max(sigma_t_max(e.next_up()));
                own.max(floor.at(e)) * scale
            })
            .collect();
        Majorant {
            energy: grid,
            sigma,
        }
    }

    /// **ABLATION — not a bound on resonance data.** The pre-#589
    /// [`Self::from_materials`]: `Σ_t` at the supplied grid points only, times
    /// `1 + margin`. On ENDF/B-VIII.0 the HTR-10 bed majorant built this way
    /// (4096 log points, margin 0.3) was 14× under at 661 eV (GitHub #589).
    /// Kept so a recorded number taken with it can be reproduced. **Do not
    /// transport on it.**
    pub fn from_materials_without_breakpoints(
        materials: &[Material],
        nuclides: &[Nuclide],
        energies: &[f64],
        margin: f64,
    ) -> Self {
        let scale = 1.0 + margin.max(0.0);
        let sigma = energies
            .iter()
            .map(|&e| {
                let m = materials
                    .iter()
                    .map(|mat| mat.macro_xs_total_upper_bound(e, nuclides))
                    .fold(0.0_f64, f64::max);
                m * scale
            })
            .collect();
        Majorant {
            energy: energies.to_vec(),
            sigma,
        }
    }

    /// **A majorant bounding only the materials a REGION can reach.**
    ///
    /// The spatial counterpart to `DhUniverse::reachable_materials`, which
    /// already narrows by material *set*. This is what makes per-region delta
    /// tracking ([`crate::geometry::cell::TrackingMethod`]) worth having: the
    /// bound is taken over the region's own materials, so a strong absorber
    /// elsewhere in the model cannot raise the cost inside it.
    ///
    /// # Why it matters, measured
    ///
    /// `examples/majorant_absorber_price.rs` (2026-09-17) added one
    /// illustrative B4C control rod to the set bounded for an HTR-10 pebble:
    /// **26.3x more tracking steps at the thermal peak**, 27.8x worst, and
    /// exactly 1.00x above ~1 keV. That cost lands everywhere the majorant
    /// applies, including reflector graphite far from the rod. Scoping the
    /// bound to the region is what recovers it.
    ///
    /// # SAFETY OF THE BOUND — read this
    ///
    /// An **under-bound majorant is a silent bias**, not a crash: delta
    /// tracking would reject collisions it should have accepted and quietly
    /// return the wrong answer. So `indices` must list **every** material the
    /// region's `material_at` can return — derive it from the geometry, never
    /// guess it, and never prune it to "the ones that matter". Over-bounding
    /// only costs time.
    ///
    /// # Parameters
    /// - `materials` — the global material table.
    /// - `indices` — indices into it that the region can actually reach.
    /// - `energies` — the grid to tabulate on; since GitHub #589 every nuclide
    ///   breakpoint is added to it ([`Self::from_materials`]).
    /// - `margin` — fractional headroom, e.g. `0.3` for 30 %.
    ///
    /// Indices outside `materials` are ignored rather than panicking, because a
    /// majorant that is too *small* is the dangerous direction and a stale
    /// index should not be able to produce one by aborting a build halfway.
    pub fn over_indices(
        materials: &[Material],
        indices: &[usize],
        nuclides: &[Nuclide],
        energies: &[f64],
        margin: f64,
    ) -> Self {
        let subset: Vec<Material> = indices
            .iter()
            .filter_map(|&i| materials.get(i).cloned())
            .collect();
        Majorant::from_materials(&subset, nuclides, energies, margin)
    }

    /// **ABLATION — not a bound on resonance data.** [`Self::over_indices`] on
    /// [`Self::from_materials_without_breakpoints`], the pre-#589
    /// construction. Kept to reproduce old records. **Do not transport on it.**
    pub fn over_indices_without_breakpoints(
        materials: &[Material],
        indices: &[usize],
        nuclides: &[Nuclide],
        energies: &[f64],
        margin: f64,
    ) -> Self {
        let subset: Vec<Material> = indices
            .iter()
            .filter_map(|&i| materials.get(i).cloned())
            .collect();
        Majorant::from_materials_without_breakpoints(&subset, nuclides, energies, margin)
    }

    /// Build a majorant that bounds `Σ_t` **by construction** wherever the data
    /// is pointwise, thermal (S(α,β)) or multigroup, and by dense sampling where
    /// it is analytic (WMP) or a URR band total.
    ///
    /// # How the bound is made (GitHub #585, 2026-10-05)
    ///
    /// Pointwise ENDF cross sections are linear-linear between their own grid
    /// points; this crate evaluates them that way (`recon.eval_mt`), as OpenMC
    /// does (`Nuclide::calculate_xs`, `src/nuclide.cpp:748`). On the **union**
    /// of every nuclide's own breakpoints
    /// ([`Nuclide::majorant_breakpoints`]: section grids, S(α,β) grids, Bragg
    /// edges, the thermal cutoff, URR table energies and range ends, group
    /// bounds), a mixture's `Σ_t` is therefore a sum of linear pieces and at
    /// most one convex `s_k/E` Bragg term in every interval. A convex function
    /// takes its maximum over an interval at an end. [`Self::at`] returns the
    /// larger of the two bracketing nodes, so tabulating `Σ_t` at every node
    /// bounds those parts with no sampling at all. Where the data **steps** (a
    /// Bragg edge, the S(α,β) cutoff, a URR range end, a group bound), each
    /// node also reads `Σ_t` one ulp below and one ulp above itself, so both
    /// one-sided limits are inside the bound.
    ///
    /// Two parts are not piecewise linear and stay **sampled**. These are the
    /// WMP analytic resonances (LOW tier) and the URR band totals between table
    /// energies, which are products of two interpolants. For those the old
    /// construction is kept as a floor: a log grid of `n_bins` bins over
    /// `[e_min, e_max]`, `Σ_t` sampled at `subsamples` energies per bin, and
    /// the bin maximum written to both bin edges. Every node of the union grid
    /// takes the larger of its own value and that envelope, so this majorant is
    /// **never below the pre-#585 one** at any energy. A `1 + margin` factor
    /// multiplies the result. On the pointwise and thermal parts it only
    /// covers round-off; on the sampled parts it covers structure narrower
    /// than the sampling. WMP pole nodes (`Nuclide::majorant_breakpoints`) narrow that.
    ///
    /// The bound is pinned on ENDF/B-VIII.0 data by
    /// `tests/majorant_bounds_endf.rs`, through [`Self::audit`].
    ///
    /// ~~Build a **provably bounding** majorant by taking, for each energy bin,
    /// the maximum of `Σ_t` over a dense sub-sample of that bin.~~ **CORRECTED
    /// 2026-10-05 (GitHub #585):** sampling is not a proof. On ENDF/B-VIII.0
    /// data the old bin sampling (4096 × 32, margin 0.1) left the true `Σ_t`
    /// at **1.18×** the majorant at 1.689 MeV in the HTR-10 UO₂ kernel: a
    /// resonance narrower than the ~340 eV sampling step fell between samples.
    /// That is a silent delta-tracking bias. The bin sampling survives above
    /// as the floor for the analytic parts.
    ///
    /// # Cost
    ///
    /// About three `Σ_t` evaluations per material per union-grid node, plus
    /// `n_bins · subsamples` for the envelope, once at construction. On
    /// ENDF/B-VIII.0 uranium the union grid is a few hundred thousand nodes, so
    /// [`Self::at`]'s binary search is a few steps deeper than on the 4097-point
    /// grid this used to return.
    ///
    /// # Parameters
    /// - `materials` / `nuclides` — every material the neutron might enter.
    /// - `e_min` / `e_max` — energy span \[eV\] to bound (cover the whole range the
    ///   histories visit — birth energy down to the lowest energy reached).
    ///   Below `e_min`, [`Self::at`] extrapolates as 1/v.
    /// - `n_bins` — number of log-spaced bins in the sampled envelope.
    /// - `subsamples` — sub-energies evaluated per bin (`≥ 2`).
    /// - `margin` — non-negative safety fraction multiplying the final values.
    pub fn bounding(
        materials: &[Material],
        nuclides: &[Nuclide],
        e_min: f64,
        e_max: f64,
        n_bins: usize,
        subsamples: usize,
        margin: f64,
    ) -> Self {
        let scale = 1.0 + margin.max(0.0);
        let sigma_t_max = |e: f64| {
            materials
                .iter()
                .map(|m| m.macro_xs_total_upper_bound(e, nuclides))
                .fold(0.0_f64, f64::max)
        };

        // 1. The sampled envelope (the pre-#585 construction, unscaled) and
        //    the energies it sampled.
        let (envelope, mut grid) =
            sampled_envelope(materials, nuclides, e_min, e_max, n_bins, subsamples);
        let edges = &envelope.energy;

        // 2. The union grid: the envelope's own samples and edges, plus every
        //    breakpoint of every nuclide the materials use.
        let (g_lo, g_hi) = (edges[0], edges[edges.len() - 1]);
        grid.extend_from_slice(edges);
        for i in used_nuclides(materials) {
            if let Some(n) = nuclides.get(i) {
                grid.extend(n.majorant_breakpoints(g_lo, g_hi));
            }
        }
        grid.retain(|&e| e.is_finite() && e >= g_lo && e <= g_hi);
        grid.sort_by(|a, b| a.partial_cmp(b).expect("finite energies"));
        // Exact duplicates only: two distinct nodes, however close, may sit
        // either side of a step.
        grid.dedup();

        // 3. Each node: its own value and both one-sided limits, floored by
        //    the envelope, times the margin.
        let sigma = grid
            .iter()
            .map(|&e| {
                let own = sigma_t_max(e.next_down())
                    .max(sigma_t_max(e))
                    .max(sigma_t_max(e.next_up()));
                own.max(envelope.at(e)) * scale
            })
            .collect();
        Majorant {
            energy: grid,
            sigma,
        }
    }

    /// **ABLATION — not a bound on pointwise data.** The pre-#585
    /// [`Self::bounding`]: `Σ_t` sampled at `subsamples` points in each of
    /// `n_bins` log bins, the bin maximum written to both bin edges, times
    /// `1 + margin`. No data breakpoints.
    ///
    /// Kept only so a recorded number taken with the old construction can be
    /// reproduced, and so its under-bound can be measured
    /// (`examples/majorant_bound_audit.rs`, `OUTRAM_HTR10_MAJORANT=bounding`).
    /// On ENDF/B-VIII.0 data it left the true `Σ_t` at 1.18× the majorant at
    /// 1.689 MeV in the HTR-10 UO₂ kernel (GitHub #585). **Do not transport
    /// on it.**
    pub fn bounding_without_breakpoints(
        materials: &[Material],
        nuclides: &[Nuclide],
        e_min: f64,
        e_max: f64,
        n_bins: usize,
        subsamples: usize,
        margin: f64,
    ) -> Self {
        let scale = 1.0 + margin.max(0.0);
        let (mut m, _) = sampled_envelope(materials, nuclides, e_min, e_max, n_bins, subsamples);
        for s in &mut m.sigma {
            *s *= scale;
        }
        m
    }

    /// Check this majorant against `Σ_t` of `materials`, at far more energies
    /// than it was built on, and return the worst ratio `Σ_t / Σ_maj` found.
    ///
    /// A ratio above 1 is an under-bound: delta tracking silently loses
    /// collisions there. The energies checked are:
    ///
    /// - `n_log + 1` log-spaced energies over `[e_lo, e_hi]`;
    /// - every [`Nuclide::majorant_breakpoints`] node of every nuclide the
    ///   materials use, with its one-ulp neighbours either side;
    /// - the **midpoint of every interval** between consecutive breakpoints.
    ///   This is the falsifiable part for the construction in
    ///   [`Self::bounding`]: if `Σ_t` were not linear or convex between
    ///   breakpoints, the midpoint is where it would show.
    ///
    /// `Σ_t` is [`Material::macro_xs_total_upper_bound`], the largest URR band.
    /// Diagnostic only; it changes nothing.
    pub fn audit(
        &self,
        materials: &[Material],
        nuclides: &[Nuclide],
        e_lo: f64,
        e_hi: f64,
        n_log: usize,
    ) -> MajorantAudit {
        let n_log = n_log.max(1);
        let mut bp: Vec<f64> = Vec::new();
        for i in used_nuclides(materials) {
            if let Some(n) = nuclides.get(i) {
                bp.extend(n.majorant_breakpoints(e_lo, e_hi));
            }
        }
        bp.sort_by(|a, b| a.partial_cmp(b).expect("finite energies"));
        bp.dedup();
        let mut energies: Vec<f64> = (0..=n_log)
            .map(|i| e_lo * (e_hi / e_lo).r_powf(i as f64 / n_log as f64))
            .collect();
        for w in bp.windows(2) {
            energies.push(0.5 * (w[0] + w[1]));
        }
        for &e in &bp {
            energies.extend([e.next_down(), e, e.next_up()]);
        }
        let mut out = MajorantAudit {
            worst_ratio: 0.0,
            energy_ev: f64::NAN,
            material: 0,
            energies_checked: energies.len(),
        };
        for &e in &energies {
            let m = self.at(e);
            for (k, mat) in materials.iter().enumerate() {
                let st = mat.macro_xs_total_upper_bound(e, nuclides);
                let r = if m > 0.0 {
                    st / m
                } else if st > 0.0 {
                    f64::INFINITY
                } else {
                    0.0
                };
                if r > out.worst_ratio {
                    out.worst_ratio = r;
                    out.energy_ev = e;
                    out.material = k;
                }
            }
        }
        out
    }

    /// Number of tabulated nodes (the length of the energy grid).
    pub fn len(&self) -> usize {
        self.energy.len()
    }

    /// Whether the majorant has no tabulated nodes.
    pub fn is_empty(&self) -> bool {
        self.energy.is_empty()
    }

    /// **ABLATION — breaks the bound on purpose.** This majorant with every
    /// value multiplied by `factor` (gh:#784).
    ///
    /// `factor < 1` makes an under-bound: where `Σ_t > factor·Σ_maj` the
    /// accept probability saturates at 1 and the excess collisions are
    /// silently lost, which is the bias the rung-5 tutorial demo's "majorant
    /// too low" switch shows. `factor > 1` only loosens the bound (more
    /// virtual collisions, the same answer). **Do not transport on a factor
    /// below 1** except to measure that bias.
    pub fn scaled(&self, factor: f64) -> Majorant {
        Majorant {
            energy: self.energy.clone(),
            sigma: self.sigma.iter().map(|s| s * factor).collect(),
        }
    }

    /// The majorant Σ_maj \[cm⁻¹\] at energy `e` \[eV\] — conservative (takes the
    /// larger bracketing grid value so it never under-bounds between points).
    ///
    /// # Below the grid floor
    ///
    /// A neutron is not confined to the range the majorant was built over, and
    /// **a flat clamp at the floor under-bounds**. Below thermal, `Σ_t` is
    /// dominated by 1/v absorption and keeps rising as `E` falls, so the value
    /// at the lowest grid point stops being a bound almost immediately.
    ///
    /// That is a *silent* bias, not an error: delta tracking accepts a collision
    /// with probability `Σ_t/Σ_maj`, so where `Σ_t > Σ_maj` the excess
    /// collisions are simply never sampled. They are lost exactly where the
    /// cross section is largest, which preferentially removes absorption and
    /// fission.
    ///
    /// Audited 2026-09-14 (`examples/majorant_bound_audit.rs`): on the
    /// openmc-notebook TRISO lattice materials, a majorant floored at 1e-4 eV
    /// under-bounded the HEU kernel by **9.1x at 1e-6 eV** with the flat clamp.
    ///
    /// Below the floor this therefore extrapolates as **1/v**,
    /// `Σ_maj(E) = Σ_maj(E_floor) · sqrt(E_floor / E)`, which is exact for 1/v
    /// behaviour and conservative for anything rising no faster than it. It
    /// cannot bound a resonance below the floor — build the grid low enough for
    /// that — but no evaluation this crate reads has one there.
    pub fn at(&self, e: f64) -> f64 {
        match self.sigma.len() {
            0 => 0.0,
            1 => self.sigma[0],
            _ => {
                // First grid point strictly above e; the bracket is [i-1, i].
                let i = self.energy.partition_point(|&g| g <= e);
                if i == 0 {
                    let e_floor = self.energy[0];
                    if e > 0.0 && e < e_floor {
                        self.sigma[0] * (e_floor / e).r_powf(0.5)
                    } else {
                        self.sigma[0]
                    }
                } else if i >= self.sigma.len() {
                    *self.sigma.last().unwrap()
                } else {
                    self.sigma[i - 1].max(self.sigma[i])
                }
            }
        }
    }
}

/// The result of [`Majorant::audit`]: the worst `Σ_t / Σ_maj` found and where.
#[derive(Debug, Clone, Copy)]
pub struct MajorantAudit {
    /// Largest `Σ_t / Σ_maj` over every energy and material checked. Above 1
    /// is an under-bound.
    pub worst_ratio: f64,
    /// Energy \[eV\] of the worst ratio.
    pub energy_ev: f64,
    /// Index, into the `materials` slice audited, of the worst material.
    pub material: usize,
    /// How many energies were checked (each against every material).
    pub energies_checked: usize,
}

/// The caller audit for GitHub #585, run on a caller's own materials and
/// settings. It builds [`Majorant::bounding`] and the pre-#585
/// [`Majorant::bounding_without_breakpoints`] with the same arguments and
/// audits both ([`Majorant::audit`], 2 000 001 log energies over
/// 1e-5 eV to 20 MeV plus every breakpoint, its neighbours and the interval
/// midpoints). It returns one printable line, prefixed `MAJORANT-AUDIT`.
///
/// Examples call this when `OUTRAM_MAJORANT_AUDIT` is set and then stop. A
/// worst ratio above 1 for the OLD construction means a number recorded with
/// that caller was measured on an under-bound majorant.
#[allow(clippy::too_many_arguments)]
pub fn bounding_audit_line(
    label: &str,
    materials: &[Material],
    nuclides: &[Nuclide],
    e_min: f64,
    e_max: f64,
    n_bins: usize,
    subsamples: usize,
    margin: f64,
) -> String {
    let (lo, hi, n) = (1.0e-5, 2.0e7, 2_000_000);
    let new = Majorant::bounding(
        materials, nuclides, e_min, e_max, n_bins, subsamples, margin,
    );
    let a_new = new.audit(materials, nuclides, lo, hi, n);
    let old = Majorant::bounding_without_breakpoints(
        materials, nuclides, e_min, e_max, n_bins, subsamples, margin,
    );
    let a_old = old.audit(materials, nuclides, lo, hi, n);
    let name = |a: &MajorantAudit| {
        materials
            .get(a.material)
            .map(|m| m.name.clone())
            .unwrap_or_default()
    };
    format!(
        "MAJORANT-AUDIT {label} | margin {margin} | OLD worst {:.4} at {:.5e} eV in '{}' | \
         NEW worst {:.4} at {:.5e} eV in '{}' | nodes old {} new {} | {} energies",
        a_old.worst_ratio,
        a_old.energy_ev,
        name(&a_old),
        a_new.worst_ratio,
        a_new.energy_ev,
        name(&a_new),
        old.len(),
        new.len(),
        a_new.energies_checked
    )
}

/// The distinct nuclide indices `materials` refer to, ascending.
fn used_nuclides(materials: &[Material]) -> Vec<usize> {
    let mut v: Vec<usize> = materials
        .iter()
        .flat_map(|m| m.components.iter().map(|c| c.nuclide_idx))
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// The sampled bin envelope shared by [`Majorant::bounding`] (as its floor)
/// and [`Majorant::bounding_without_breakpoints`]: `n_bins` log bins over
/// `[e_min, e_max]`, `Σ_t` sampled at `subsamples` points per bin, and the bin
/// maximum written to both bin edges. Unscaled. Also returns every energy it
/// sampled.
fn sampled_envelope(
    materials: &[Material],
    nuclides: &[Nuclide],
    e_min: f64,
    e_max: f64,
    n_bins: usize,
    subsamples: usize,
) -> (Majorant, Vec<f64>) {
    let n_bins = n_bins.max(1);
    let subsamples = subsamples.max(2);
    let ln_lo = e_min.max(f64::MIN_POSITIVE).r_ln();
    let ln_hi = e_max.max(e_min * 1.0001).r_ln();
    let edge = |i: usize| (ln_lo + (ln_hi - ln_lo) * i as f64 / n_bins as f64).r_exp();
    let sigma_t_max = |e: f64| {
        materials
            .iter()
            .map(|m| m.macro_xs_total_upper_bound(e, nuclides))
            .fold(0.0_f64, f64::max)
    };
    let energy: Vec<f64> = (0..=n_bins).map(edge).collect();
    let mut sigma = vec![0.0_f64; n_bins + 1];
    let mut sampled = Vec::with_capacity(n_bins * subsamples);
    for b in 0..n_bins {
        let (lo, hi) = (energy[b].r_ln(), energy[b + 1].r_ln());
        let mut bin_max = 0.0_f64;
        for s in 0..subsamples {
            let e = (lo + (hi - lo) * s as f64 / (subsamples - 1) as f64).r_exp();
            bin_max = bin_max.max(sigma_t_max(e));
            sampled.push(e);
        }
        // Write the bin peak to both edges so `at`'s bracket-max bounds the bin.
        sigma[b] = sigma[b].max(bin_max);
        sigma[b + 1] = sigma[b + 1].max(bin_max);
    }
    (Majorant { energy, sigma }, sampled)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The majorant must bound `Sigma_t` below its own grid floor**, because a
    /// neutron is not confined to the range it was built over.
    ///
    /// A flat clamp under-bounds: `Sigma_t` is 1/v-dominated below thermal and
    /// keeps rising. Delta tracking then silently loses collisions wherever
    /// `Sigma_t > Sigma_maj` — no error, just missing absorption and fission.
    ///
    /// Measured before the 1/v extrapolation was added (2026-09-14), on the
    /// openmc-notebook TRISO lattice materials with a floor of 1e-4 eV: the HEU
    /// kernel was under-bounded by **9.09x at 1e-6 eV**.
    #[test]
    fn majorant_bounds_sigma_t_below_its_grid_floor() {
        let nuclides: Vec<Nuclide> = ["U235", "U238", "H1"]
            .iter()
            .map(|n| Nuclide::from_core(n).expect("embedded evaluation"))
            .collect();
        let fuel = Material {
            id: 1,
            name: "HEU kernel".into(),
            temperature: 293.6,
            components: vec![
                crate::material::material::NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: 4.4994e-2,
                },
                crate::material::material::NuclideComponent {
                    nuclide_idx: 1,
                    atom_density: 2.4984e-3,
                },
            ],
        };
        let matrix = Material {
            id: 2,
            name: "H matrix".into(),
            temperature: 293.6,
            components: vec![crate::material::material::NuclideComponent {
                nuclide_idx: 2,
                atom_density: 4.0e-2,
            }],
        };
        let materials = [fuel, matrix];
        let floor = 1.0e-4;
        let maj = Majorant::bounding(&materials, &nuclides, floor, 2.0e7, 2048, 16, 0.1);

        // Scan two decades BELOW the floor -- the region a flat clamp gets wrong.
        let mut worst = 0.0_f64;
        let mut worst_e = 0.0;
        for i in 0..=400 {
            let e = 1.0e-6 * (floor / 1.0e-6).r_powf(i as f64 / 400.0);
            let m = maj.at(e);
            for mat in &materials {
                let ratio = mat.macro_xs_total(e, &nuclides) / m;
                if ratio > worst {
                    worst = ratio;
                    worst_e = e;
                }
            }
        }
        assert!(
            worst <= 1.0,
            "majorant UNDER-BOUNDS by {worst:.3}x at E = {worst_e:.3e} eV, below its {floor:.0e} eV \
             floor. Delta tracking loses collisions there and the bias is silent."
        );
    }

    /// The majorant must bound every material's Σ_t at every tabulated energy —
    /// the correctness precondition of delta tracking. Uses the CORE-data Godiva
    /// uranium mix plus a light "matrix" so the max is non-trivially one material.
    #[test]
    fn majorant_bounds_every_material() {
        use crate::material::material::NuclideComponent;
        let nuclides = vec![
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("H1").unwrap(),
        ];
        let fuel = Material {
            id: 1,
            name: "fuel".into(),
            temperature: 293.6,
            components: vec![NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.8e-2,
            }],
        };
        let matrix = Material {
            id: 2,
            name: "matrix".into(),
            temperature: 293.6,
            components: vec![NuclideComponent {
                nuclide_idx: 1,
                atom_density: 6.0e-2,
            }],
        };
        let mats = [fuel, matrix];
        let grid: Vec<f64> = (0..40).map(|i| 1.0e3 * 1.4_f64.powi(i)).collect();
        let maj = Majorant::from_materials(&mats, &nuclides, &grid, 0.01);

        for &e in &grid {
            let m = maj.at(e);
            for mat in &mats {
                let st = mat.macro_xs_total(e, &nuclides);
                assert!(m >= st, "majorant {m} < Σ_t {st} at {e} eV");
            }
        }
    }
}
