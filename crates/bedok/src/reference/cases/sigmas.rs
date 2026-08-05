//! Cross-section tables and the feedback-derivative tables the coupled solver
//! interpolates them with.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source files | the `sigmavalues` and `constants` blocks of `iaea3ds.m`, `neacrpa2.m`, `neacrpa2t.m`, `neacrpa1t.m`, `neacrpd1.m`, `neacrpd1t.m`, `geom2dxycase1.m` |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//!
//! # Units
//!
//! Macroscopic cross sections are per centimetre \[1/cm\]; `kappa_fission` is
//! an energy release times a cross section \[J/cm\]. A feedback table holds the
//! **derivative** of each of those with respect to its state variable, so its
//! units are the cross section's divided by ppm, K or g/cm³ as appropriate.
//!
//! # Group index convention for scattering
//!
//! MATLAB writes the scattering table as `sigmavalues.s(material, gt, g)` and
//! reads it in `makesigmadfxyz.m` as "from group `g` into group `gt`" — the
//! **second** index is the destination. The rows the case files assign,
//! `s(m,:,:) = [s_11 s_12; s_21 s_22]`, therefore put the down-scatter cross
//! section at `s(m,2,1)`. [`ScatterTable`] keeps that ordering.

use crate::error::{BedokError, Result};

/// Group-to-group scattering cross sections, per material \[1/cm\].
///
/// MATLAB `sigmavalues.s`, a `(materials × G × G)` array indexed
/// `s(material, to, from)`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScatterTable {
    materials: usize,
    ngroups: usize,
    values: Vec<f64>,
}

impl ScatterTable {
    /// An all-zero table for `materials` materials and `ngroups` groups.
    #[must_use]
    pub fn zeros(materials: usize, ngroups: usize) -> Self {
        Self {
            materials,
            ngroups,
            values: vec![0.0; materials * ngroups * ngroups],
        }
    }

    /// Number of materials the table covers.
    #[must_use]
    pub const fn materials(&self) -> usize {
        self.materials
    }

    /// Number of energy groups.
    #[must_use]
    pub const fn ngroups(&self) -> usize {
        self.ngroups
    }

    fn offset(&self, material: usize, to: usize, from: usize) -> usize {
        debug_assert!(material < self.materials);
        debug_assert!(to < self.ngroups && from < self.ngroups);
        (material * self.ngroups + to) * self.ngroups + from
    }

    /// Cross section for scattering **from** group `from` **into** group `to`,
    /// all indices 0-based \[1/cm\]. MATLAB `s(material+1, to+1, from+1)`.
    ///
    /// # Panics
    ///
    /// In debug builds, if any index is out of range.
    #[must_use]
    pub fn get(&self, material: usize, to: usize, from: usize) -> f64 {
        self.values[self.offset(material, to, from)]
    }

    /// Set the cross section for `from` → `to` \[1/cm\], all indices 0-based.
    ///
    /// # Panics
    ///
    /// In debug builds, if any index is out of range.
    pub fn set(&mut self, material: usize, to: usize, from: usize, value: f64) {
        let o = self.offset(material, to, from);
        self.values[o] = value;
    }

    /// Assign a whole material's 2 × 2 block in MATLAB source order.
    ///
    /// `block` is `[[s(1,1), s(1,2)], [s(2,1), s(2,2)]]`, i.e. exactly the
    /// literal written as `sigmavalues.s(m,:,:) = [a b; c d]`, so a reader can
    /// check the port against the MATLAB line without transposing anything.
    ///
    /// # Panics
    ///
    /// If the table does not have exactly two groups.
    pub fn set_block_2x2(&mut self, material: usize, block: [[f64; 2]; 2]) {
        assert_eq!(self.ngroups, 2, "set_block_2x2 needs a two-group table");
        for (to, row) in block.iter().enumerate() {
            for (from, v) in row.iter().enumerate() {
                self.set(material, to, from, *v);
            }
        }
    }
}

/// One complete set of macroscopic cross sections, or of their derivatives
/// with respect to a feedback variable.
///
/// MATLAB spreads these over `sigmavalues.tot` / `.f` / `.fp` / `.a` / `.s`,
/// and repeats the same five fields inside each feedback sub-struct
/// (`sigmavalues.boron.tot`, `sigmavalues.fueltemp.tot`, …). One type serves
/// both roles here, exactly as it does there.
///
/// # Absent fields
///
/// A MATLAB struct may simply lack a field — `iaea3ds.m` defines no
/// `sigmavalues.a` and no `sigmavalues.fp`, and `makesigmadfxyz.m` tests
/// `isfield(sigmavalues,'fp')` and substitutes zeros. An **empty** `Vec` here
/// means the same thing: the field was not set. Use
/// [`absorption_or_zero`](Self::absorption_or_zero) /
/// [`kappa_fission_or_zero`](Self::kappa_fission_or_zero) to get the
/// zero-filled form.
#[derive(Debug, Clone, PartialEq)]
pub struct SigmaSet {
    /// Total (removal) cross section per material and group \[1/cm\]. MATLAB
    /// `sigmavalues.tot`; indexed `[material][group]`, both 0-based.
    pub total: Vec<Vec<f64>>,
    /// Fission production cross section, `nu*Sigma_f` \[1/cm\]. MATLAB
    /// `sigmavalues.f`. The `nu` of the benchmarks is folded in already —
    /// `sigmavalues.nu` is all ones.
    pub nu_fission: Vec<Vec<f64>>,
    /// Fission energy-release cross section, `kappa*Sigma_f` \[J/cm\]. MATLAB
    /// `sigmavalues.fp`. Empty where the case does not define it.
    pub kappa_fission: Vec<Vec<f64>>,
    /// Absorption cross section \[1/cm\]. MATLAB `sigmavalues.a`. Empty where
    /// the case does not define it.
    pub absorption: Vec<Vec<f64>>,
    /// Group-to-group scattering \[1/cm\]. MATLAB `sigmavalues.s`.
    pub scatter: ScatterTable,
}

impl SigmaSet {
    /// An all-zero set for `materials` materials and `ngroups` groups, with
    /// `absorption` and `kappa_fission` present and zeroed.
    #[must_use]
    pub fn zeros(materials: usize, ngroups: usize) -> Self {
        Self {
            total: vec![vec![0.0; ngroups]; materials],
            nu_fission: vec![vec![0.0; ngroups]; materials],
            kappa_fission: vec![vec![0.0; ngroups]; materials],
            absorption: vec![vec![0.0; ngroups]; materials],
            scatter: ScatterTable::zeros(materials, ngroups),
        }
    }

    /// Number of materials.
    #[must_use]
    pub fn materials(&self) -> usize {
        self.total.len()
    }

    /// Number of energy groups.
    #[must_use]
    pub fn ngroups(&self) -> usize {
        self.scatter.ngroups()
    }

    /// Absorption, or zeros if the case left the field unset.
    #[must_use]
    pub fn absorption_or_zero(&self) -> Vec<Vec<f64>> {
        if self.absorption.is_empty() {
            vec![vec![0.0; self.ngroups()]; self.materials()]
        } else {
            self.absorption.clone()
        }
    }

    /// `kappa*Sigma_f`, or zeros if the case left the field unset — the
    /// substitution `makesigmadfxyz.m` makes.
    #[must_use]
    pub fn kappa_fission_or_zero(&self) -> Vec<Vec<f64>> {
        if self.kappa_fission.is_empty() {
            vec![vec![0.0; self.ngroups()]; self.materials()]
        } else {
            self.kappa_fission.clone()
        }
    }

    /// Fill the within-group scattering diagonal from the total and absorption
    /// cross sections.
    ///
    /// Rust translation of the two lines every NEACRP case ends its scattering
    /// block with:
    ///
    /// ```text
    /// s(:,1,1) = tot(:,1) - a(:,1) - s(:,2,1);
    /// s(:,2,2) = tot(:,2) - a(:,2) - s(:,1,2);
    /// ```
    ///
    /// i.e. self-scatter is whatever the total is not accounted for by
    /// absorption and out-scatter. This makes `total` a *removal-consistent*
    /// total rather than an independent datum, which is why the feedback
    /// tables can apply the same identity to their derivatives.
    ///
    /// Generalised over `G` groups: `s(m,g,g) = tot(m,g) - a(m,g) - sum over
    /// gt /= g of s(m,gt,g)`. For the two-group tables of every ported case
    /// this is exactly the pair of lines above.
    ///
    /// # Errors
    ///
    /// [`BedokError::Fixture`] if `absorption` is unset, since the identity
    /// cannot then be evaluated.
    pub fn close_self_scatter(&mut self) -> Result<()> {
        if self.absorption.is_empty() {
            return Err(BedokError::Fixture {
                path: "sigmavalues".to_string(),
                reason: "close_self_scatter needs sigmavalues.a to be set".to_string(),
            });
        }
        let ngroups = self.ngroups();
        for m in 0..self.materials() {
            for g in 0..ngroups {
                let out: f64 = (0..ngroups)
                    .filter(|gt| *gt != g)
                    .map(|gt| self.scatter.get(m, gt, g))
                    .sum();
                let value = self.total[m][g] - self.absorption[m][g] - out;
                self.scatter.set(m, g, g, value);
            }
        }
        Ok(())
    }
}

/// The derivative of every cross section with respect to one feedback
/// variable, plus the state that variable is referenced to.
///
/// MATLAB `sigmavalues.boron`, `.fueltemp`, `.cooltemp`, `.coolden`, `.crod`.
/// `sigmavalupd3d.m` applies them as
/// `sigma <- sigma + d(sigma)/dx * (x - x_ref)` per node.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackTable {
    /// The state value the base cross sections were generated at: boron
    /// \[ppm\], temperature \[K\] or density \[g/cm³\]. MATLAB
    /// `sigmavalues.<var>.ref`.
    ///
    /// `None` for the control-rod table, which the case files leave unset —
    /// `sigmavalup3d_handler.m` assigns `sigmavaluesref.crod.ref = 0` before
    /// use, because the "state" there is a rodded *fraction* running from 0
    /// to 1.
    pub reference: Option<f64>,
    /// The derivatives themselves, one value per material and group.
    pub derivative: SigmaSet,
    /// Per-spatial-node mask: `1` where this feedback is applied, `0` where it
    /// is not. MATLAB `sigmavalues.<var>.upd`, flattened
    /// `ix*ny*nz + iy*nz + iz`.
    ///
    /// Every case sets it to "the node is fissile", i.e. `sum(f(m,:)) > 0`,
    /// then reuses the same mask for the other feedbacks. Empty where the
    /// MATLAB does not define the field (the control-rod table).
    pub update_mask: Vec<f64>,
}

/// The complete `sigmavalues` struct of a case.
#[derive(Debug, Clone, PartialEq)]
pub struct SigmaValues {
    /// The base cross sections, at the reference state.
    pub base: SigmaSet,
    /// Average neutrons per fission per material and group
    /// \[dimensionless\]. MATLAB `sigmavalues.nu`, copied from
    /// `constants.nu`; all ones in every case, because `sigmavalues.f`
    /// already carries `nu*Sigma_f`.
    pub nu: Vec<Vec<f64>>,
    /// Fission emission spectrum per material and group \[dimensionless\].
    /// MATLAB `sigmavalues.chi`, copied from `constants.chi`; all fission
    /// neutrons are born in group 1 in every case.
    pub chi: Vec<Vec<f64>>,
    /// Boron-concentration feedback \[per ppm\]. MATLAB `sigmavalues.boron`.
    pub boron: Option<FeedbackTable>,
    /// Fuel-temperature (Doppler) feedback \[per K\]. MATLAB
    /// `sigmavalues.fueltemp`.
    pub fuel_temperature: Option<FeedbackTable>,
    /// Coolant-temperature feedback \[per K\]. MATLAB
    /// `sigmavalues.cooltemp`.
    pub coolant_temperature: Option<FeedbackTable>,
    /// Coolant-density feedback \[per g/cm³\]. MATLAB
    /// `sigmavalues.coolden`.
    pub coolant_density: Option<FeedbackTable>,
    /// Fully-inserted-control-rod increment \[1/cm per unit rodded
    /// fraction\]. MATLAB `sigmavalues.crod`.
    pub control_rod: Option<FeedbackTable>,
}

/// The `constants` struct: the fission spectrum, neutron yield, and prompt
/// fraction.
///
/// MATLAB copies `constants.chi` and `constants.nu` straight into
/// `sigmavalues`, so the two are always equal; both are kept so the port has
/// the same shape as the original.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseConstants {
    /// Fission emission spectrum per material and group \[dimensionless\].
    /// MATLAB `constants.chi`.
    pub chi: Vec<Vec<f64>>,
    /// Neutron yield per fission, per material and group \[dimensionless\].
    /// MATLAB `constants.nu`.
    pub nu: Vec<Vec<f64>>,
    /// Prompt fission fraction \[dimensionless\]. MATLAB `constants.frac_p`.
    ///
    /// `None` for the NEACRP cases, which never set it — a MATLAB struct with
    /// the field simply absent.
    pub frac_p: Option<f64>,
}

impl CaseConstants {
    /// The spectrum every 3-D case uses: all fission neutrons born in group 1,
    /// `nu` folded into `sigmavalues.f` so the table itself is all ones.
    ///
    /// MATLAB:
    ///
    /// ```text
    /// constants.chi = zeros(materials, G);
    /// constants.chi(:,1) = ones(materials, 1);
    /// constants.nu = ones(materials, G);
    /// ```
    #[must_use]
    pub fn fast_group_birth(materials: usize, ngroups: usize, frac_p: Option<f64>) -> Self {
        let mut chi = vec![vec![0.0; ngroups]; materials];
        for row in &mut chi {
            row[0] = 1.0;
        }
        Self {
            chi,
            nu: vec![vec![1.0; ngroups]; materials],
            frac_p,
        }
    }
}

/// Build the fissile-node mask a feedback table is applied over.
///
/// Rust translation of the loop every NEACRP case writes as
///
/// ```text
/// for ix … for iy … for iz
///     if whichsigma(ix,iy,iz)==0, continue, end
///     if sum(sigmavalues.f(whichsigma(ix,iy,iz),:))>0
///         upd(idx)=1;
///     end
/// ```
///
/// i.e. **1 at every node whose material produces fission neutrons, 0
/// elsewhere**. `which_sigma` holds 1-based material indices flattened
/// `ix*ny*nz + iy*nz + iz`; `nu_fission` is `[material][group]`.
///
/// # Errors
///
/// [`BedokError::Fixture`] if a node names a material outside `nu_fission`.
pub fn fissile_node_mask(which_sigma: &[usize], nu_fission: &[Vec<f64>]) -> Result<Vec<f64>> {
    let mut mask = vec![0.0f64; which_sigma.len()];
    for (idx, m) in which_sigma.iter().enumerate() {
        if *m == 0 {
            continue;
        }
        let row = nu_fission.get(*m - 1).ok_or_else(|| BedokError::Fixture {
            path: "sigmavalues.f".to_string(),
            reason: format!(
                "node {idx} names material {m}, but only {} exist",
                nu_fission.len()
            ),
        })?;
        if row.iter().sum::<f64>() > 0.0 {
            mask[idx] = 1.0;
        }
    }
    Ok(mask)
}

/// Copy a MATLAB literal of the form `[a b; c d; …]` into `[material][group]`
/// rows, starting at 1-based material `first_material`.
///
/// The case files assign their tables in slabs
/// (`sigmavalues.tot(1:6,:) = […]; sigmavalues.tot(7:11,:) = […]`) so that the
/// port can be diffed against the MATLAB line by line. This helper preserves
/// that structure.
///
/// # Errors
///
/// [`BedokError::Fixture`] if the rows do not fit inside `table`, or if a row
/// has the wrong number of groups.
pub fn assign_rows(table: &mut [Vec<f64>], first_material: usize, rows: &[[f64; 2]]) -> Result<()> {
    for (k, row) in rows.iter().enumerate() {
        let m = first_material + k;
        if m == 0 || m > table.len() {
            return Err(BedokError::Fixture {
                path: "sigmavalues".to_string(),
                reason: format!("material {m} outside 1..={}", table.len()),
            });
        }
        if table[m - 1].len() != 2 {
            return Err(BedokError::Fixture {
                path: "sigmavalues".to_string(),
                reason: format!("material {m} has {} groups, expected 2", table[m - 1].len()),
            });
        }
        table[m - 1][0] = row[0];
        table[m - 1][1] = row[1];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scatter_block_lands_in_matlab_order() {
        let mut s = ScatterTable::zeros(1, 2);
        // sigmavalues.s(1,:,:) = [0.2 0.0; 0.02 0.7]
        s.set_block_2x2(0, [[0.2, 0.0], [0.02, 0.7]]);
        // s(1,2,1) is the down-scatter: from group 1 into group 2.
        assert_eq!(s.get(0, 1, 0), 0.02);
        // s(1,1,2) is up-scatter, zero here.
        assert_eq!(s.get(0, 0, 1), 0.0);
        assert_eq!(s.get(0, 0, 0), 0.2);
        assert_eq!(s.get(0, 1, 1), 0.7);
    }

    #[test]
    fn self_scatter_closes_on_total_minus_absorption_minus_outscatter() {
        let mut set = SigmaSet::zeros(1, 2);
        set.total[0] = vec![0.222117, 0.803140];
        set.absorption[0] = vec![8.71774E-03, 6.52550E-02];
        set.scatter.set_block_2x2(0, [[0.0, 0.0], [0.0182498, 0.0]]);
        set.close_self_scatter().expect("absorption is set");
        let expect_11 = 0.222117 - 8.71774E-03 - 0.0182498;
        let expect_22 = 0.803140 - 6.52550E-02 - 0.0;
        assert!((set.scatter.get(0, 0, 0) - expect_11).abs() < 1e-15);
        assert!((set.scatter.get(0, 1, 1) - expect_22).abs() < 1e-15);
    }

    #[test]
    fn fissile_mask_marks_only_fissile_materials() {
        // material 1 fissile, material 2 not; node 2 is outside the core.
        let f = vec![vec![0.0, 0.1], vec![0.0, 0.0]];
        let mask = fissile_node_mask(&[1, 0, 2, 1], &f).expect("valid");
        assert_eq!(mask, vec![1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn constants_put_every_fission_neutron_in_group_one() {
        let c = CaseConstants::fast_group_birth(3, 2, Some(1.0));
        assert_eq!(c.chi[0], vec![1.0, 0.0]);
        assert_eq!(c.nu[2], vec![1.0, 1.0]);
        assert_eq!(c.frac_p, Some(1.0));
    }

    #[test]
    fn missing_fields_read_back_as_zeros() {
        let mut set = SigmaSet::zeros(2, 2);
        set.absorption.clear();
        set.kappa_fission.clear();
        assert_eq!(set.absorption_or_zero(), vec![vec![0.0; 2]; 2]);
        assert_eq!(set.kappa_fission_or_zero(), vec![vec![0.0; 2]; 2]);
        assert!(set.close_self_scatter().is_err());
    }
}
