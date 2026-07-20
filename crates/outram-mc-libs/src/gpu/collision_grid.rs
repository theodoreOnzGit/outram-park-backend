//! Per-nuclide reaction cross sections tabulated on a shared union energy grid —
//! the data the GPU **collision** kernel needs to resolve a collision entirely on
//! device (which nuclide, which reaction, secondary energy/angle) without a
//! per-event CPU round-trip.
//!
//! ## Why this exists (the op-u6s.8 fix)
//!
//! The batched-flight path ([`crate::gpu::batched_flight`]) puts the per-event
//! *flight* on the GPU but keeps the branchy *collision* physics on the CPU,
//! forcing a CPU↔GPU round-trip **per event** (op-u6s.7's measured bottleneck).
//! To keep the whole generation GPU-resident, the collision kernel must sample the
//! nuclide, partition the reaction, and apply scatter kinematics on device. It
//! cannot call [`crate::material::nuclide::Nuclide::xs_at_energy`] there (that is a
//! WMP / MGXS evaluation with no GPU form), so — exactly as
//! [`crate::gpu::union_grid::UnionTotalXs`] does for the macroscopic total — the
//! per-nuclide reaction cross sections are **pre-tabulated once on the CPU** on a
//! dense union grid and uploaded; the kernel then does a binary search + linear
//! interpolation per channel.
//!
//! ## What is tabulated
//!
//! On a shared ascending energy grid (the native-breakpoint union of
//! [`crate::gpu::union_grid::UnionTotalXs::tabulate_native`], so grid points land
//! on real data features), for **each** nuclide component of the material:
//! its microscopic `total`, `fission`, `absorption`, `inelastic`, `nu_fission`
//! (all barn), and elastic mean cosine `mubar` (dimensionless). Plus the material's
//! macroscopic total Σ_t (cm⁻¹) and, per nuclide, its atom density `N` \[atoms/
//! barn·cm\], atomic weight ratio `awr`, and CE↔MG seam energy `e_max` \[eV\].
//!
//! Every column mirrors the CPU collision partition in
//! [`crate::physics::keff`] `collide_batched`, so the on-device collision is the
//! same physical model as the trusted CPU path — differing only through the f32
//! tabulation/interpolation (acceleration only, judged against the CPU reference,
//! never trusted above it; see `crate::gpu` `mod.rs`).
//!
//! ## Fidelity scope (stated honestly)
//!
//! This is built to reproduce the **LOW (`Core`) tier** collision physics on the
//! GPU (the Godiva bare-sphere benchmark): WMP + fast-MGXS data, `(n,2n)` = 0, and
//! elastic anisotropy carried by a single per-group μ̄. For a HIGH (`Pointwise`)
//! nuclide the `inelastic` column is still tabulated but the elastic angular law
//! degrades to isotropic-CM on the GPU (`mubar = 0`; see
//! [`Nuclide::elastic_mubar`](crate::material::nuclide::Nuclide::elastic_mubar)) —
//! the full MF=4 distribution stays on the trusted CPU backends.

// UNTRUSTED AI-DRAFT: this code has been drafted with AI assistance and is
// verified by the V&V gate in `mod tests` (a per-channel round-trip check against
// direct `Nuclide` evaluation and a macroscopic-total consistency check), but
// still requires human review before it is promoted past the "Unit Tested" V&V
// stage.

use crate::gpu::union_grid::UnionTotalXs;
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;

/// Per-nuclide reaction cross sections on a shared union energy grid, laid out for
/// upload to the GPU collision kernel.
///
/// # Layout
/// `grid` is the ascending energy grid \[eV\], length `G`. Every per-channel field
/// is `n_nuclides * G` long, **flattened nuclide-major**: channel `c` for nuclide
/// `j` at grid point `i` lives at `field[j * G + i]`. The nuclide order matches
/// `material.components` (so nuclide `j` here is `material.components[j]`).
///
/// # Physical meaning / units
/// - `grid` — ascending energy \[eV\], length `G`.
/// - `macro_total` — material macroscopic total Σ_t \[cm⁻¹\] at each grid point,
///   `sum_j N_j * micro_total[j*G+i]`. Length `G`. (The flight kernel's Σ_t.)
/// - `micro_total` / `micro_fission` / `micro_absorption` / `micro_inelastic` /
///   `micro_nu_fission` — per-nuclide **microscopic** σ \[barn\], length
///   `n_nuclides * G`. `absorption = capture + fission`; `nu_fission = ν̄·σ_f`.
/// - `micro_mubar` — per-nuclide elastic mean cosine μ̄ (CM frame, dimensionless),
///   length `n_nuclides * G`; `0` ⇒ isotropic-CM elastic at that energy (see
///   [`Nuclide::elastic_mubar`](crate::material::nuclide::Nuclide::elastic_mubar)).
/// - `atom_density` — per-nuclide `N_j` \[atoms/barn·cm\], length `n_nuclides`.
/// - `awr` — per-nuclide atomic weight ratio `A_j`, length `n_nuclides`.
/// - `e_max` — per-nuclide CE↔MG seam energy \[eV\], length `n_nuclides` (elastic
///   below it is isotropic-CM, above it uses μ̄).
/// - `temperature_k` — material temperature \[K\] used for every lookup.
pub struct CollisionTables {
    /// Ascending union energy grid \[eV\], length `G`.
    pub grid: Vec<f64>,
    /// Macroscopic total Σ_t \[cm⁻¹\] at each grid point, length `G`.
    pub macro_total: Vec<f64>,
    /// Number of nuclide components (`= material.components.len()`).
    pub n_nuclides: usize,
    /// Per-nuclide microscopic total σ_t \[barn\], length `n_nuclides * G`.
    pub micro_total: Vec<f64>,
    /// Per-nuclide microscopic fission σ_f \[barn\], length `n_nuclides * G`.
    pub micro_fission: Vec<f64>,
    /// Per-nuclide microscopic absorption σ_a \[barn\], length `n_nuclides * G`.
    pub micro_absorption: Vec<f64>,
    /// Per-nuclide microscopic inelastic σ \[barn\], length `n_nuclides * G`.
    pub micro_inelastic: Vec<f64>,
    /// Per-nuclide microscopic ν̄·σ_f \[barn\], length `n_nuclides * G`.
    pub micro_nu_fission: Vec<f64>,
    /// Per-nuclide elastic mean cosine μ̄ (CM), length `n_nuclides * G`.
    pub micro_mubar: Vec<f64>,
    /// Per-nuclide atom density `N_j` \[atoms/barn·cm\], length `n_nuclides`.
    pub atom_density: Vec<f64>,
    /// Per-nuclide atomic weight ratio `A_j`, length `n_nuclides`.
    pub awr: Vec<f64>,
    /// Per-nuclide CE↔MG seam energy \[eV\], length `n_nuclides`.
    pub e_max: Vec<f64>,
    /// Material temperature \[K\] used for the tabulation.
    pub temperature_k: f64,
}

impl CollisionTables {
    /// Build the per-nuclide reaction tables on the material's
    /// **native-breakpoint union grid** over `[e_min_ev, e_max_ev]` \[eV\].
    ///
    /// The grid is exactly [`UnionTotalXs::tabulate_native`]'s (native nuclide
    /// breakpoints merged with a log-spaced backbone of `backbone_points`), so the
    /// flight kernel and the collision kernel share one grid and one binary search.
    /// At every grid node each nuclide's [`MicroXS`](crate::material::nuclide::MicroXS)
    /// is evaluated at the material temperature and split into the tabulated
    /// channels; `macro_total` is `sum_j N_j * σ_t,j` so it is consistent with the
    /// per-nuclide totals the kernel sums when sampling the collision nuclide.
    ///
    /// # Inputs / assumptions
    /// - `material` — the mixture; its `temperature` \[K\] drives every lookup and
    ///   its `components` set the nuclide order.
    /// - `nuclides` — the global nuclide array the components index into.
    /// - `e_min_ev`, `e_max_ev` — strictly positive energy bounds \[eV\],
    ///   `e_min_ev < e_max_ev` (the backbone is log-spaced).
    /// - `backbone_points` — log-spaced backbone floor size (guarded `>= 2`); the
    ///   final grid length `G >= backbone_points`.
    ///
    /// Pure — no RNG, no I/O.
    pub fn build(
        material: &Material,
        nuclides: &[Nuclide],
        e_min_ev: f64,
        e_max_ev: f64,
        backbone_points: usize,
    ) -> Self {
        // Reuse the native-breakpoint union grid + macroscopic total so the
        // collision kernel shares the flight kernel's grid and Σ_t exactly.
        let union = UnionTotalXs::tabulate_native(material, nuclides, e_min_ev, e_max_ev, backbone_points);
        let grid = union.grid;
        let macro_total = union.sigma_total;
        let g = grid.len();
        let n_nuclides = material.components.len();
        let temp = material.temperature;

        let mut micro_total = Vec::with_capacity(n_nuclides * g);
        let mut micro_fission = Vec::with_capacity(n_nuclides * g);
        let mut micro_absorption = Vec::with_capacity(n_nuclides * g);
        let mut micro_inelastic = Vec::with_capacity(n_nuclides * g);
        let mut micro_nu_fission = Vec::with_capacity(n_nuclides * g);
        let mut micro_mubar = Vec::with_capacity(n_nuclides * g);
        let mut atom_density = Vec::with_capacity(n_nuclides);
        let mut awr = Vec::with_capacity(n_nuclides);
        let mut e_max = Vec::with_capacity(n_nuclides);

        for c in &material.components {
            let nuc = &nuclides[c.nuclide_idx];
            atom_density.push(c.atom_density);
            awr.push(nuc.awr);
            e_max.push(nuc.e_max_ev());
            for &e in &grid {
                let x = nuc.xs_at_energy(e, temp);
                micro_total.push(x.total);
                micro_fission.push(x.fission);
                micro_absorption.push(x.absorption);
                micro_inelastic.push(x.inelastic);
                micro_nu_fission.push(x.nu_fission);
                micro_mubar.push(nuc.elastic_mubar(e));
            }
        }

        Self {
            grid,
            macro_total,
            n_nuclides,
            micro_total,
            micro_fission,
            micro_absorption,
            micro_inelastic,
            micro_nu_fission,
            micro_mubar,
            atom_density,
            awr,
            e_max,
            temperature_k: temp,
        }
    }

    /// Number of energy grid points `G`.
    #[inline]
    pub fn n_grid(&self) -> usize {
        self.grid.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::material::NuclideComponent;

    fn godiva() -> (Material, Vec<Nuclide>) {
        let nuclides = vec![
            Nuclide::from_core("U234").unwrap(),
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("U238").unwrap(),
        ];
        let material = Material {
            id: 1,
            name: "Godiva".into(),
            temperature: 293.6,
            components: vec![
                NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
                NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
                NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
            ],
        };
        (material, nuclides)
    }

    /// V&V (per-channel tabulation round-trip): the tabulated per-nuclide columns
    /// reproduce a direct [`Nuclide::xs_at_energy`] evaluation at the grid nodes.
    ///
    /// **Methodology.** Build the Godiva LOW-tier tables. For every nuclide and
    /// every grid node the stored `micro_total/fission/absorption/inelastic/
    /// nu_fission` must equal `nuclides[..].xs_at_energy(grid[i], T)` exactly (same
    /// f64 evaluation, no interpolation involved at a node). Pass: bit-exact at
    /// every node for every channel.
    ///
    /// **Results (2026-07-17).** All channels match `xs_at_energy` at every union
    /// node for U-234/235/238, confirming the tabulation stores the same numbers
    /// the CPU collision kernel evaluates.
    #[test]
    fn per_channel_columns_match_direct_eval() {
        let (material, nuclides) = godiva();
        let t = CollisionTables::build(&material, &nuclides, 1e-3, 2e7, 2048);
        let g = t.n_grid();
        assert_eq!(t.n_nuclides, 3);
        for (j, c) in material.components.iter().enumerate() {
            let nuc = &nuclides[c.nuclide_idx];
            for i in 0..g {
                let x = nuc.xs_at_energy(t.grid[i], material.temperature);
                let k = j * g + i;
                assert_eq!(t.micro_total[k], x.total);
                assert_eq!(t.micro_fission[k], x.fission);
                assert_eq!(t.micro_absorption[k], x.absorption);
                assert_eq!(t.micro_inelastic[k], x.inelastic);
                assert_eq!(t.micro_nu_fission[k], x.nu_fission);
            }
        }
    }

    /// V&V (macroscopic-total consistency): the shared `macro_total` equals the sum
    /// of the per-nuclide `N_j * micro_total_j` at every node — so the flight
    /// kernel's Σ_t and the collision kernel's nuclide-sampling denominator agree.
    ///
    /// **Methodology.** At each grid node compare `macro_total[i]` against
    /// `sum_j atom_density[j] * micro_total[j*G+i]`. Pass: relative agreement
    /// within 1e-12 (both are the same f64 arithmetic up to summation order).
    ///
    /// **Results (2026-07-17).** Max relative difference < 1e-12 across the Godiva
    /// union grid — the macroscopic total is a faithful sum of the tabulated
    /// per-nuclide totals.
    #[test]
    fn macro_total_is_sum_of_micro() {
        let (material, nuclides) = godiva();
        let t = CollisionTables::build(&material, &nuclides, 1e-3, 2e7, 2048);
        let g = t.n_grid();
        let mut max_rel = 0.0_f64;
        for i in 0..g {
            let mut s = 0.0;
            for j in 0..t.n_nuclides {
                s += t.atom_density[j] * t.micro_total[j * g + i];
            }
            let rel = (s - t.macro_total[i]).abs() / t.macro_total[i].abs().max(1e-30);
            max_rel = max_rel.max(rel);
        }
        assert!(max_rel < 1e-12, "macro_total vs sum micro rel diff {max_rel}");
    }
}
