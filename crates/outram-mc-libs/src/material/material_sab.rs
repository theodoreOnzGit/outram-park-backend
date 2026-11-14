//! **S(α,β) associated with a material, as OpenMC does** (GitHub #462).
//!
//! OpenMC lists a material's thermal scattering tables on the material
//! (`Material::thermal_tables_`, each an `index_nuclide`, an `index_table`
//! and a `fraction`; applied per nuclide in `Material::calculate_neutron_xs`,
//! `src/material.cpp:847-885`, and mixed by `sab_frac` in
//! `Nuclide::calculate_sab_xs`, `src/nuclide.cpp:838-860`). This crate hangs
//! the law on the [`Nuclide`] instead ([`Nuclide::with_thermal_scattering`]),
//! so every material that references that nuclide index gets it.
//!
//! [`assign_material_sab`] takes OpenMC's per-material association and
//! lowers it onto the nuclide array the transport kernels read, without
//! touching them (the collision sites are kovan-reviewed and closed to
//! modification):
//!
//! - a material that names an S(α,β) table for a nuclide gets its own bound
//!   copy of that nuclide (one copy per `(nuclide, table)` pair, shared by
//!   every material that names the same pair);
//! - a material that names none keeps the original, unbound entry;
//! - `fraction < 1` splits the component: `fraction · N` on the bound copy
//!   and `(1 - fraction) · N` on the free entry.
//!
//! **The split is equivalent to upstream's `sab_frac`, not bit-identical.**
//! Upstream mixes within one nuclide: `σ_t = σ_t,free + f (σ_sab − σ_el)`,
//! then picks the thermal channel with probability `f σ_sab / ...`. Two
//! components at `f N` and `(1 − f) N` give the same macroscopic total and the
//! same probability of every channel, so the same transport in distribution;
//! the random numbers used to pick the channel differ.
//!
//! NEW WORK in the sense of the code (a lowering), a port in the sense of the
//! semantics.

use crate::material::material::{Material, NuclideComponent};
use crate::material::nuclide::Nuclide;
use crate::material::thermal::ThermalScattering;

/// One entry of a material's thermal-table list: OpenMC's
/// `ThermalTable { index_table, index_nuclide, fraction }`
/// (`include/openmc/material.h`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialSab {
    /// Index of the material in the material array.
    pub material: usize,
    /// Index of the nuclide in the nuclide array (the one the material's
    /// component references).
    pub nuclide: usize,
    /// Index of the table in the `tables` slice passed to
    /// [`assign_material_sab`].
    pub table: usize,
    /// Fraction of that nuclide's atoms that are bound, in `(0, 1]`
    /// (OpenMC's `fraction`, default 1).
    pub fraction: f64,
}

/// Lower per-material S(α,β) associations onto the nuclide array (see the
/// module docs). Returns new material and nuclide arrays; the input nuclides
/// keep their indices, and bound copies are appended after them.
///
/// # Errors
///
/// - an index out of range;
/// - a fraction outside `(0, 1]`;
/// - a material that does not contain the named nuclide;
/// - a nuclide named here that already carries a law attached with
///   [`Nuclide::with_thermal_scattering`]: that is the shared-nuclide
///   association #462 is about, and mixing the two would be ambiguous;
/// - two entries for the same `(material, nuclide)`.
pub fn assign_material_sab(
    materials: &[Material],
    nuclides: &[Nuclide],
    tables: &[ThermalScattering],
    sab: &[MaterialSab],
) -> Result<(Vec<Material>, Vec<Nuclide>), String> {
    let mut out_mats = materials.to_vec();
    let mut out_nucs = nuclides.to_vec();
    // (nuclide, table) -> index of its bound copy.
    let mut copies: Vec<((usize, usize), usize)> = Vec::new();
    for (k, s) in sab.iter().enumerate() {
        if s.material >= materials.len() || s.nuclide >= nuclides.len() || s.table >= tables.len()
        {
            return Err(format!("S(a,b) entry {k} has an index out of range: {s:?}"));
        }
        if !(s.fraction > 0.0 && s.fraction <= 1.0) {
            return Err(format!(
                "S(a,b) entry {k}: fraction {} is outside (0, 1]",
                s.fraction
            ));
        }
        if nuclides[s.nuclide].has_thermal_scattering() {
            return Err(format!(
                "S(a,b) entry {k}: nuclide {} already carries a thermal law attached \
                 with Nuclide::with_thermal_scattering, which every material sharing it \
                 would get (GitHub #462). Pass it unbound and associate the law here.",
                s.nuclide
            ));
        }
        if sab[..k]
            .iter()
            .any(|o| o.material == s.material && o.nuclide == s.nuclide)
        {
            return Err(format!(
                "S(a,b) entry {k}: material {} already has a table for nuclide {}",
                s.material, s.nuclide
            ));
        }
        let bound = match copies.iter().find(|(key, _)| *key == (s.nuclide, s.table)) {
            Some((_, i)) => *i,
            None => {
                let i = out_nucs.len();
                out_nucs.push(
                    nuclides[s.nuclide]
                        .clone()
                        .with_thermal_scattering(tables[s.table].clone()),
                );
                copies.push(((s.nuclide, s.table), i));
                i
            }
        };
        let mat = &mut out_mats[s.material];
        let mut found = false;
        let mut split: Vec<NuclideComponent> = Vec::with_capacity(mat.components.len() + 1);
        for c in &mat.components {
            if c.nuclide_idx == s.nuclide {
                found = true;
                split.push(NuclideComponent {
                    nuclide_idx: bound,
                    atom_density: s.fraction * c.atom_density,
                });
                if s.fraction < 1.0 {
                    split.push(NuclideComponent {
                        nuclide_idx: s.nuclide,
                        atom_density: (1.0 - s.fraction) * c.atom_density,
                    });
                }
            } else {
                split.push(c.clone());
            }
        }
        if !found {
            return Err(format!(
                "S(a,b) entry {k}: material {} ({}) has no component for nuclide {}",
                s.material, mat.name, s.nuclide
            ));
        }
        mat.components = split;
    }
    Ok((out_mats, out_nucs))
}
