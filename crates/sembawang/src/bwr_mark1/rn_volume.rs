//! One control volume's fission products and aerosol: the sectional aerosol
//! (`aerosol`), each condensable class (cesium as CsOH and iodine as I2, in
//! [`CLASSES`]) as vapour, on the airborne aerosol and on each surface, and
//! the transfers between them in one step (gh:#1047).
//!
//! The step follows MELCOR's order and coupling (Vol. 2, SAND2023-10994 O,
//! RN §2.4 and §2.5.2; no source used): released aerosol enters the
//! **smallest section** (RN §2.4.3: "consistent with the production of
//! small particles by gas-to-particle conversion"); vapour condenses onto
//! the aerosol sections (Sherwood number 2) and onto the surfaces at their
//! own temperatures by TRAP-MELT2's analytic step; the aerosol agglomerates
//! and deposits.
//!
//! **Simplification, stated:** each class on the airborne aerosol is tracked
//! as one mass, sharing the aerosol's size distribution, which is MELCOR's rule
//! for classes assigned to one component (RN §2.4.1). MELCOR's default puts
//! the volatiles in a component of their own; that is not followed here.
//! Evaporation from a surface is limited to what it holds (TRAP-MELT2's
//! step does not limit it; the excess returns to the vapour).

use super::aerosol::{Medium, Sections, Surface};
use super::wire::{Reader, Writer};
use super::vapour::{condense, BulkGas, CondensingSurface, FpClass};

/// The condensable classes tracked, in the order of every per-class array.
pub const CLASSES: [FpClass; 2] = [FpClass::Cs, FpClass::I2];
/// Index of cesium and of iodine in the per-class arrays.
pub const CS: usize = 0;
pub const IODINE: usize = 1;
/// Per-class masses, kg.
pub type PerClass = [f64; 2];

/// A deposition surface as this volume sees it: the aerosol surface (area,
/// facing, phoretic velocities) and, for the vapour, its temperature, K,
/// and mass-transfer coefficient, m/s.
#[derive(Clone, Debug, PartialEq)]
pub struct RnSurface {
    pub surface: Surface,
    pub t_k: f64,
    pub k_vapour: f64,
}

/// What is in the volume.
#[derive(Clone, Debug, PartialEq)]
pub struct RnVolume {
    pub volume_m3: f64,
    pub sections: Sections,
    /// Each class in the gas as vapour, and on the airborne aerosol, kg.
    pub vapour_kg: PerClass,
    pub on_aerosol_kg: PerClass,
    /// Per surface: each class on it, kg (by deposition and by
    /// condensation), and aerosol deposited on it, kg.
    pub on_surface_kg: Vec<PerClass>,
    pub aerosol_on_surface_kg: Vec<f64>,
}

impl RnVolume {
    pub fn new(volume_m3: f64, sections: Sections, n_surfaces: usize) -> Self {
        Self {
            volume_m3,
            sections,
            vapour_kg: [0.0; 2],
            on_aerosol_kg: [0.0; 2],
            on_surface_kg: vec![[0.0; 2]; n_surfaces],
            aerosol_on_surface_kg: vec![0.0; n_surfaces],
        }
    }

    /// Airborne aerosol mass, kg, at material density `rho_p`.
    pub fn aerosol_kg(&self, rho_p: f64) -> f64 {
        self.sections.conc.iter().sum::<f64>() * self.volume_m3 * rho_p
    }

    /// One class's mass everywhere in the volume (vapour, on aerosol, on
    /// every surface), kg.
    pub fn total_kg(&self, class: usize) -> f64 {
        self.vapour_kg[class] + self.on_aerosol_kg[class] + self.on_surface_kg.iter().map(|x| x[class]).sum::<f64>()
    }

    /// Add released aerosol, kg, into the smallest section, and vapour of
    /// each class, kg.
    pub fn add(&mut self, aerosol_kg: f64, rho_p: f64, vapour_kg: PerClass) {
        self.sections.conc[0] += aerosol_kg / rho_p / self.volume_m3;
        for (v, a) in self.vapour_kg.iter_mut().zip(vapour_kg) {
            *v += a;
        }
    }

    /// Take the fraction `f` of the volume's gas (with its aerosol and what
    /// is on it, and the vapour) out, as a flow carries it: returns the
    /// particle volume moved per section, m^3, the vapour and the mass on
    /// aerosol of each class, kg.
    pub fn take(&mut self, f: f64) -> (Vec<f64>, PerClass, PerClass) {
        let f = f.clamp(0.0, 1.0);
        let moved: Vec<f64> = self.sections.conc.iter().map(|c| c * f * self.volume_m3).collect();
        for c in self.sections.conc.iter_mut() {
            *c *= 1.0 - f;
        }
        let vap = self.vapour_kg.map(|x| x * f);
        let aer = self.on_aerosol_kg.map(|x| x * f);
        for k in 0..2 {
            self.vapour_kg[k] -= vap[k];
            self.on_aerosol_kg[k] -= aer[k];
        }
        (moved, vap, aer)
    }

    /// Receive what `take` returned from another volume (same sections).
    pub fn receive(&mut self, moved: &[f64], vapour_kg: PerClass, on_aerosol_kg: PerClass) {
        for (c, m) in self.sections.conc.iter_mut().zip(moved) {
            *c += m / self.volume_m3;
        }
        for k in 0..2 {
            self.vapour_kg[k] += vapour_kg[k];
            self.on_aerosol_kg[k] += on_aerosol_kg[k];
        }
    }

    /// Write the state (the section volumes are fixed by construction).
    pub fn write(&self, w: &mut Writer) {
        w.f(&[self.volume_m3]);
        w.f(&self.sections.conc);
        w.f(&[self.sections.fallout]);
        w.f(&self.vapour_kg);
        w.f(&self.on_aerosol_kg);
        for x in &self.on_surface_kg {
            w.f(x);
        }
        w.f(&self.aerosol_on_surface_kg);
    }
    /// Read the state into a volume built like this one.
    pub fn read_into(&mut self, r: &mut Reader) {
        self.volume_m3 = r.f();
        for c in self.sections.conc.iter_mut() {
            *c = r.f();
        }
        self.sections.fallout = r.f();
        self.vapour_kg = [r.f(), r.f()];
        self.on_aerosol_kg = [r.f(), r.f()];
        for x in self.on_surface_kg.iter_mut() {
            *x = [r.f(), r.f()];
        }
        for x in self.aerosol_on_surface_kg.iter_mut() {
            *x = r.f();
        }
    }

    /// One step of `dt` s: condensation of each class, agglomeration,
    /// deposition.
    pub fn step(&mut self, medium: &Medium, gases: &[(BulkGas, f64)], surfaces: &[RnSurface], dt: f64) {
        let v = self.volume_m3;
        let n = self.sections.v.len();
        // 1. Each class's vapour onto the aerosol sections (at the gas
        //    temperature) and the surfaces (at theirs).
        for (c, class) in CLASSES.iter().enumerate() {
            let d_vap = class.diffusivity(gases, medium.t_k, medium.p_pa);
            // The aerosol sections share one saturation concentration (they
            // are at the gas temperature), so they lump exactly into one
            // surface.
            let total_ak: f64 = (0..n)
                .map(|k| {
                    let d = self.sections.diameter(k);
                    self.sections.number(k) * std::f64::consts::PI * d * d * v * 2.0 * d_vap / d
                })
                .sum();
            let mut lumped = vec![CondensingSurface { ak: total_ak, c_sat: class.saturation_concentration(medium.t_k), mass: self.on_aerosol_kg[c] }];
            for (s, held) in surfaces.iter().zip(&self.on_surface_kg) {
                lumped.push(CondensingSurface { ak: s.surface.area * s.k_vapour, c_sat: class.saturation_concentration(s.t_k), mass: held[c] });
            }
            // A surface holding nothing whose saturation is above the gas
            // concentration can neither take vapour nor give any, so it
            // takes no part (left in, it would enter as a huge evaporation
            // that the limit below takes back: exact in arithmetic, but a
            // cancellation of large numbers in floating point, found with
            // I2 on warm surfaces).
            let c0 = self.vapour_kg[c] / v;
            let active: Vec<usize> = (0..lumped.len()).filter(|&i| lumped[i].mass > 0.0 || lumped[i].c_sat < c0).collect();
            let mut part: Vec<CondensingSurface> = active.iter().map(|&i| lumped[i]).collect();
            let c_a = condense(c0, v, &mut part, dt);
            self.vapour_kg[c] = c_a * v;
            for (&i, p) in active.iter().zip(&part) {
                lumped[i] = *p;
            }
            // Evaporation limited to what each holds.
            for s in lumped.iter_mut() {
                if s.mass < 0.0 {
                    self.vapour_kg[c] += s.mass;
                    s.mass = 0.0;
                }
            }
            self.on_aerosol_kg[c] = lumped[0].mass;
            for (held, s) in self.on_surface_kg.iter_mut().zip(&lumped[1..]) {
                held[c] = s.mass;
            }
        }
        // 2. Agglomeration.
        let d: Vec<f64> = (0..n).map(|k| self.sections.diameter(k)).collect();
        let before: f64 = self.sections.conc.iter().sum();
        let fallout_before = self.sections.fallout;
        if before > 0.0 {
            let table = medium.kernel_table(&d);
            self.sections.coagulate(dt, |i, j| table[i][j]);
        }
        // 3. Deposition, and the fallout onto the first floor; what is on
        //    the aerosol goes with it.
        let surf: Vec<Surface> = surfaces.iter().map(|s| s.surface.clone()).collect();
        let deposited = self.sections.deposit(medium, &surf, v, dt);
        let fallout = self.sections.fallout - fallout_before;
        if before > 0.0 {
            let floor = surfaces.iter().position(|s| s.surface.facing == super::aerosol::Facing::Floor);
            let mut shares: Vec<f64> = deposited.iter().map(|x| x / before).collect();
            if let Some(i) = floor {
                shares[i] += fallout / before;
            }
            let on_air = self.on_aerosol_kg;
            for (i, share) in shares.iter().enumerate() {
                for c in 0..2 {
                    self.on_surface_kg[i][c] += on_air[c] * share;
                    self.on_aerosol_kg[c] -= on_air[c] * share;
                }
                self.aerosol_on_surface_kg[i] += share * before * v * medium.rho_p;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::aerosol::Facing;

    fn surfaces() -> Vec<RnSurface> {
        vec![
            RnSurface { surface: Surface { area: 300.0, facing: Facing::Floor, phoretic: vec![0.0; 20] }, t_k: 400.0, k_vapour: 0.01 },
            RnSurface { surface: Surface { area: 1500.0, facing: Facing::Wall, phoretic: vec![0.0; 20] }, t_k: 400.0, k_vapour: 0.01 },
        ]
    }

    /// Cesium and aerosol are conserved through a 2 h sequence of
    /// condensation, agglomeration, deposition and an outflow; hot gas
    /// (1500 K) keeps CsOH a vapour (its saturation is far above the load)
    /// while 400 K walls take it; and the aerosol, entering the smallest
    /// section, grows: by the end its mass is spread over larger sections.
    ///
    /// Results (2026-10-29): of 1.2 kg of cesium, 1.086 kg is on the
    /// airborne aerosol, 0.019 on the floor, 0.016 on the walls, 0.078 left
    /// with the outflow and none remains as vapour: at 500 K the CsOH
    /// condenses, and onto the aerosol, whose area dwarfs the walls'. The
    /// aerosol, put into the 0.1 um section, has grown into one smooth peak
    /// at section 11 (about 1.3 um).
    #[test]
    fn a_volume_conserves_cesium_and_grows_its_aerosol() {
        let medium = Medium::air(500.0, 3.0e5, 2.6e-5);
        let gases = [(BulkGas::Steam, 0.5), (BulkGas::N2, 0.5)];
        let mut vol = RnVolume::new(3400.0, Sections::new(0.1e-6, 20, 2.0), 2);
        let s = surfaces();
        let (mut cs_in, mut cs_out, mut aer_in, mut aer_out) = (0.0, 0.0, 0.0, 0.0);
        for step in 0..7200 {
            if step < 600 {
                vol.add(1e-3, medium.rho_p, [2e-3, 0.0]);
                cs_in += 2e-3;
                aer_in += 1e-3;
            }
            vol.step(&medium, &gases, &s, 1.0);
            if step % 100 == 0 {
                let (moved, cv, ca) = vol.take(1e-3);
                cs_out += cv[CS] + ca[CS];
                aer_out += moved.iter().sum::<f64>() * medium.rho_p;
            }
        }
        println!(
            "Cs: vapour {:.4} on aerosol {:.4} on floor {:.4} on wall {:.4} out {:.4} of {:.3} kg; aerosol by section {:?}",
            vol.vapour_kg[CS], vol.on_aerosol_kg[CS], vol.on_surface_kg[0][CS], vol.on_surface_kg[1][CS], cs_out, cs_in,
            vol.sections.conc.iter().map(|c| format!("{:.1e}", c)).collect::<Vec<_>>()
        );
        let cs_now = vol.total_kg(CS);
        assert!((cs_now + cs_out - cs_in).abs() < 1e-9 * cs_in, "{cs_now} {cs_out} {cs_in}");
        let aer_now = vol.aerosol_kg(medium.rho_p) + vol.aerosol_on_surface_kg.iter().sum::<f64>();
        assert!((aer_now + aer_out - aer_in).abs() < 1e-9 * aer_in, "{aer_now} {aer_out} {aer_in}");
        assert!(vol.sections.conc[0] < vol.sections.conc[1..].iter().sum::<f64>(), "{:?}", vol.sections.conc);
        assert!(vol.on_aerosol_kg[CS] > 0.8 * cs_in && vol.vapour_kg[CS] < 1e-6, "{vol:?}");
        let peak = (0..20).max_by(|&a, &b| vol.sections.conc[a].total_cmp(&vol.sections.conc[b])).unwrap();
        assert!((9..=13).contains(&peak), "{peak}");
        let hot = FpClass::Cs.saturation_concentration(1500.0) * 3400.0;
        assert!(hot > 1.0, "{hot}");
    }
}
