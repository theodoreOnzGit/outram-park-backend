//! One control volume's fission products and aerosol: the sectional aerosol
//! (`aerosol`), cesium as vapour, on the airborne aerosol and on each
//! surface, and the transfers between them in one step (gh:#1047, third
//! deliverable; not yet wired into the plant).
//!
//! The step follows MELCOR's order and coupling (Vol. 2, SAND2023-10994 O,
//! RN §2.4 and §2.5.2; no source used): released aerosol enters the
//! **smallest section** (RN §2.4.3: "consistent with the production of
//! small particles by gas-to-particle conversion"); vapour condenses onto
//! the aerosol sections (Sherwood number 2) and onto the surfaces at their
//! own temperatures by TRAP-MELT2's analytic step; the aerosol agglomerates
//! and deposits.
//!
//! **Simplification, stated:** cesium on the airborne aerosol is tracked as
//! one mass, sharing the aerosol's size distribution, which is MELCOR's rule
//! for classes assigned to one component (RN §2.4.1). MELCOR's default puts
//! the volatiles in a component of their own; that is not followed here.
//! Evaporation from a surface is limited to what it holds (TRAP-MELT2's
//! step does not limit it; the excess returns to the vapour).

use super::aerosol::{Medium, Sections, Surface};
use super::wire::{Reader, Writer};
use super::vapour::{condense, BulkGas, CondensingSurface, FpClass};

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
    /// Cesium (as CsOH) in the gas as vapour, and on the airborne aerosol,
    /// kg.
    pub cs_vapour_kg: f64,
    pub cs_on_aerosol_kg: f64,
    /// Per surface: cesium on it, kg (by deposition and by condensation),
    /// and aerosol deposited on it, kg.
    pub cs_on_surface_kg: Vec<f64>,
    pub aerosol_on_surface_kg: Vec<f64>,
}

impl RnVolume {
    pub fn new(volume_m3: f64, sections: Sections, n_surfaces: usize) -> Self {
        Self {
            volume_m3,
            sections,
            cs_vapour_kg: 0.0,
            cs_on_aerosol_kg: 0.0,
            cs_on_surface_kg: vec![0.0; n_surfaces],
            aerosol_on_surface_kg: vec![0.0; n_surfaces],
        }
    }

    /// Airborne aerosol mass, kg, at material density `rho_p`.
    pub fn aerosol_kg(&self, rho_p: f64) -> f64 {
        self.sections.conc.iter().sum::<f64>() * self.volume_m3 * rho_p
    }

    /// Add released aerosol, kg, into the smallest section, and cesium
    /// vapour, kg.
    pub fn add(&mut self, aerosol_kg: f64, rho_p: f64, cs_vapour_kg: f64) {
        self.sections.conc[0] += aerosol_kg / rho_p / self.volume_m3;
        self.cs_vapour_kg += cs_vapour_kg;
    }

    /// Take the fraction `f` of the volume's gas (with its aerosol, the
    /// cesium on it and the vapour) out, as a flow carries it: returns the
    /// aerosol volume concentration moved per section times this volume,
    /// m^3 of particle, the cesium vapour and the cesium on aerosol, kg.
    pub fn take(&mut self, f: f64) -> (Vec<f64>, f64, f64) {
        let f = f.clamp(0.0, 1.0);
        let moved: Vec<f64> = self.sections.conc.iter().map(|c| c * f * self.volume_m3).collect();
        for c in self.sections.conc.iter_mut() {
            *c *= 1.0 - f;
        }
        let (cv, ca) = (self.cs_vapour_kg * f, self.cs_on_aerosol_kg * f);
        self.cs_vapour_kg -= cv;
        self.cs_on_aerosol_kg -= ca;
        (moved, cv, ca)
    }

    /// Receive what `take` returned from another volume (same sections).
    pub fn receive(&mut self, moved: &[f64], cs_vapour_kg: f64, cs_on_aerosol_kg: f64) {
        for (c, m) in self.sections.conc.iter_mut().zip(moved) {
            *c += m / self.volume_m3;
        }
        self.cs_vapour_kg += cs_vapour_kg;
        self.cs_on_aerosol_kg += cs_on_aerosol_kg;
    }

    /// Write the state (the section volumes are fixed by construction).
    pub fn write(&self, w: &mut Writer) {
        w.f(&[self.volume_m3]);
        w.f(&self.sections.conc);
        w.f(&[self.sections.fallout, self.cs_vapour_kg, self.cs_on_aerosol_kg]);
        w.f(&self.cs_on_surface_kg);
        w.f(&self.aerosol_on_surface_kg);
    }
    /// Read the state into a volume built like this one.
    pub fn read_into(&mut self, r: &mut Reader) {
        self.volume_m3 = r.f();
        for c in self.sections.conc.iter_mut() {
            *c = r.f();
        }
        self.sections.fallout = r.f();
        self.cs_vapour_kg = r.f();
        self.cs_on_aerosol_kg = r.f();
        for x in self.cs_on_surface_kg.iter_mut() {
            *x = r.f();
        }
        for x in self.aerosol_on_surface_kg.iter_mut() {
            *x = r.f();
        }
    }

    /// One step of `dt` s: condensation, agglomeration, deposition.
    pub fn step(&mut self, medium: &Medium, gases: &[(BulkGas, f64)], surfaces: &[RnSurface], dt: f64) {
        let v = self.volume_m3;
        let n = self.sections.v.len();
        let cs = FpClass::Cs;
        // 1. Vapour onto the aerosol sections (at the gas temperature) and
        //    the surfaces (at theirs).
        let d_vap = cs.diffusivity(gases, medium.t_k, medium.p_pa);
        // The aerosol sections share one saturation concentration (they are
        // at the gas temperature), so they lump exactly into one surface.
        let total_ak: f64 = (0..n)
            .map(|k| {
                let d = self.sections.diameter(k);
                self.sections.number(k) * std::f64::consts::PI * d * d * v * 2.0 * d_vap / d
            })
            .sum();
        let mut lumped = vec![CondensingSurface { ak: total_ak, c_sat: cs.saturation_concentration(medium.t_k), mass: self.cs_on_aerosol_kg }];
        for (s, held) in surfaces.iter().zip(&self.cs_on_surface_kg) {
            lumped.push(CondensingSurface { ak: s.surface.area * s.k_vapour, c_sat: cs.saturation_concentration(s.t_k), mass: *held });
        }
        let c_a = condense(self.cs_vapour_kg / v, v, &mut lumped, dt);
        self.cs_vapour_kg = c_a * v;
        // Evaporation limited to what each holds.
        for s in lumped.iter_mut() {
            if s.mass < 0.0 {
                self.cs_vapour_kg += s.mass;
                s.mass = 0.0;
            }
        }
        self.cs_on_aerosol_kg = lumped[0].mass;
        for (held, s) in self.cs_on_surface_kg.iter_mut().zip(&lumped[1..]) {
            *held = s.mass;
        }
        // 2. Agglomeration.
        let d: Vec<f64> = (0..n).map(|k| self.sections.diameter(k)).collect();
        let before: f64 = self.sections.conc.iter().sum();
        let fallout_before = self.sections.fallout;
        if before > 0.0 {
            let table = medium.kernel_table(&d);
            self.sections.coagulate(dt, |i, j| table[i][j]);
        }
        // 3. Deposition, and the fallout onto the first floor.
        let surf: Vec<Surface> = surfaces.iter().map(|s| s.surface.clone()).collect();
        let deposited = self.sections.deposit(medium, &surf, v, dt);
        let fallout = self.sections.fallout - fallout_before;
        if before > 0.0 {
            let floor = surfaces.iter().position(|s| s.surface.facing == super::aerosol::Facing::Floor);
            let mut shares: Vec<f64> = deposited.iter().map(|x| x / before).collect();
            if let Some(i) = floor {
                shares[i] += fallout / before;
            }
            let cs_air = self.cs_on_aerosol_kg;
            for (i, share) in shares.iter().enumerate() {
                self.cs_on_surface_kg[i] += cs_air * share;
                self.aerosol_on_surface_kg[i] += share * before * v * medium.rho_p;
                self.cs_on_aerosol_kg -= cs_air * share;
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
                vol.add(1e-3, medium.rho_p, 2e-3);
                cs_in += 2e-3;
                aer_in += 1e-3;
            }
            vol.step(&medium, &gases, &s, 1.0);
            if step % 100 == 0 {
                let (moved, cv, ca) = vol.take(1e-3);
                cs_out += cv + ca;
                aer_out += moved.iter().sum::<f64>() * medium.rho_p;
            }
        }
        println!(
            "Cs: vapour {:.4} on aerosol {:.4} on floor {:.4} on wall {:.4} out {:.4} of {:.3} kg; aerosol by section {:?}",
            vol.cs_vapour_kg, vol.cs_on_aerosol_kg, vol.cs_on_surface_kg[0], vol.cs_on_surface_kg[1], cs_out, cs_in,
            vol.sections.conc.iter().map(|c| format!("{:.1e}", c)).collect::<Vec<_>>()
        );
        let cs_now = vol.cs_vapour_kg + vol.cs_on_aerosol_kg + vol.cs_on_surface_kg.iter().sum::<f64>();
        assert!((cs_now + cs_out - cs_in).abs() < 1e-9 * cs_in, "{cs_now} {cs_out} {cs_in}");
        let aer_now = vol.aerosol_kg(medium.rho_p) + vol.aerosol_on_surface_kg.iter().sum::<f64>();
        assert!((aer_now + aer_out - aer_in).abs() < 1e-9 * aer_in, "{aer_now} {aer_out} {aer_in}");
        assert!(vol.sections.conc[0] < vol.sections.conc[1..].iter().sum::<f64>(), "{:?}", vol.sections.conc);
        assert!(vol.cs_on_aerosol_kg > 0.8 * cs_in && vol.cs_vapour_kg < 1e-6, "{vol:?}");
        let peak = (0..20).max_by(|&a, &b| vol.sections.conc[a].total_cmp(&vol.sections.conc[b])).unwrap();
        assert!((9..=13).contains(&peak), "{peak}");
        let hot = FpClass::Cs.saturation_concentration(1500.0) * 3400.0;
        assert!(hot > 1.0, "{hot}");
    }
}
