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
//! ~~**Simplification, stated:** each class on the airborne aerosol is
//! tracked as one mass, sharing the aerosol's size distribution, which is
//! MELCOR's rule for classes assigned to one component (RN §2.4.1). MELCOR's
//! default puts the volatiles in a component of their own; that is not
//! followed here.~~ **Changed 2026-10-29:** each class on the airborne
//! aerosol is tracked **by section**. It condenses onto each section in
//! proportion to that section's mass-transfer coefficient (MELCOR's
//! condensation onto the aerosol is per section, RN §2.5.2). It moves
//! through agglomeration by the same coefficients as the particle volume
//! (Jacobson et al.'s multicomponent scheme) and deposits with its
//! section. The condensed mass still does not change the particles' size,
//! which MELCOR's separate volatile component would. The lumped model
//! before survives as the ablation (`lumped`).
//!
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
/// Per-class masses by aerosol section, kg (`[class][section]`).
pub type PerClassSection = [Vec<f64>; 2];

/// What a flow carries from one volume to another (`RnVolume::take`):
/// particle volume per section, m^3, each class's vapour, kg, and each
/// class on the aerosol by section, kg. Empty when nothing is carried this
/// way.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RnFlow {
    pub moved: Vec<f64>,
    pub vapour_kg: PerClass,
    pub on_aerosol_kg: PerClassSection,
}

impl RnFlow {
    /// The flow scaled by `f`.
    pub fn scaled(&self, f: f64) -> Self {
        RnFlow {
            moved: self.moved.iter().map(|m| m * f).collect(),
            vapour_kg: self.vapour_kg.map(|x| x * f),
            on_aerosol_kg: [0, 1].map(|c| self.on_aerosol_kg[c].iter().map(|x| x * f).collect::<Vec<f64>>()),
        }
    }
    pub fn write(&self, w: &mut Writer) {
        w.vec(&self.moved);
        w.f(&self.vapour_kg);
        w.vec(&self.on_aerosol_kg[0]);
        w.vec(&self.on_aerosol_kg[1]);
    }
    pub fn read(r: &mut Reader) -> Self {
        let moved = r.vec();
        let vapour_kg = [r.f(), r.f()];
        let a = r.vec();
        let b = r.vec();
        RnFlow { moved, vapour_kg, on_aerosol_kg: [a, b] }
    }
}

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
    pub on_aerosol_kg: PerClassSection,
    /// The ablation (not state; set by the owner before each use): each
    /// class on the aerosol is spread over the sections in proportion to
    /// the aerosol's volume before every operation, which is the lumped
    /// model before 2026-10-29 to round-off.
    pub lumped: bool,
    /// Whether a class's mass condensed on (or evaporated from) the aerosol
    /// adds to (or takes from) its section's particle volume, at the
    /// aerosol density, as MELCOR's sectional mass does (RN §2.5.2: the
    /// condensed mass on an aerosol section is that section's aerosol mass,
    /// and its area follows the section's mass). Not state; set by the
    /// owner before each use. False is the condensate riding on the
    /// particles weightless (the model before 2026-10-29).
    pub condensate_mass: bool,
    /// Per surface: each class on it, kg (by deposition and by
    /// condensation), and aerosol deposited on it, kg.
    pub on_surface_kg: Vec<PerClass>,
    pub aerosol_on_surface_kg: Vec<f64>,
}

impl RnVolume {
    pub fn new(volume_m3: f64, sections: Sections, n_surfaces: usize) -> Self {
        let n = sections.v.len();
        Self {
            volume_m3,
            sections,
            vapour_kg: [0.0; 2],
            on_aerosol_kg: [vec![0.0; n], vec![0.0; n]],
            lumped: false,
            condensate_mass: false,
            on_surface_kg: vec![[0.0; 2]; n_surfaces],
            aerosol_on_surface_kg: vec![0.0; n_surfaces],
        }
    }

    /// Resize the gas volume to `volume_m3` (a vessel's gas space as its
    /// level moves), keeping the particles: the concentrations scale.
    pub fn set_volume(&mut self, volume_m3: f64) {
        if volume_m3 != self.volume_m3 && self.volume_m3 > 0.0 {
            let r = self.volume_m3 / volume_m3;
            for c in self.sections.conc.iter_mut() {
                *c *= r;
            }
        }
        self.volume_m3 = volume_m3;
    }

    /// Airborne aerosol mass, kg, at material density `rho_p`.
    pub fn aerosol_kg(&self, rho_p: f64) -> f64 {
        self.sections.conc.iter().sum::<f64>() * self.volume_m3 * rho_p
    }

    /// One class's mass everywhere in the volume (vapour, on aerosol, on
    /// every surface), kg.
    pub fn total_kg(&self, class: usize) -> f64 {
        self.vapour_kg[class] + self.on_aerosol(class) + self.on_surface_kg.iter().map(|x| x[class]).sum::<f64>()
    }

    /// One class's mass on the airborne aerosol, all sections, kg.
    pub fn on_aerosol(&self, class: usize) -> f64 {
        self.on_aerosol_kg[class].iter().sum()
    }

    /// The ablation's spreading: each class's aerosol-borne mass over the
    /// sections in proportion to their volume (nothing when there is no
    /// aerosol).
    fn lump(&mut self) {
        let total: f64 = self.sections.conc.iter().sum();
        if !self.lumped || total <= 0.0 {
            return;
        }
        for c in 0..2 {
            let m = self.on_aerosol(c);
            for (x, v) in self.on_aerosol_kg[c].iter_mut().zip(&self.sections.conc) {
                *x = m * v / total;
            }
        }
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
    pub fn take(&mut self, f: f64) -> (Vec<f64>, PerClass, PerClassSection) {
        self.lump();
        let f = f.clamp(0.0, 1.0);
        let moved: Vec<f64> = self.sections.conc.iter().map(|c| c * f * self.volume_m3).collect();
        for c in self.sections.conc.iter_mut() {
            *c *= 1.0 - f;
        }
        let vap = self.vapour_kg.map(|x| x * f);
        let aer: PerClassSection = [0, 1].map(|c| self.on_aerosol_kg[c].iter().map(|x| x * f).collect::<Vec<f64>>());
        for k in 0..2 {
            self.vapour_kg[k] -= vap[k];
            for (x, a) in self.on_aerosol_kg[k].iter_mut().zip(&aer[k]) {
                *x -= a;
            }
        }
        (moved, vap, aer)
    }

    /// Receive what `take` returned from another volume (same sections).
    pub fn receive(&mut self, moved: &[f64], vapour_kg: PerClass, on_aerosol_kg: &PerClassSection) {
        for (c, m) in self.sections.conc.iter_mut().zip(moved) {
            *c += m / self.volume_m3;
        }
        for k in 0..2 {
            self.vapour_kg[k] += vapour_kg[k];
            for (x, a) in self.on_aerosol_kg[k].iter_mut().zip(&on_aerosol_kg[k]) {
                *x += a;
            }
        }
    }

    /// Write the state (the section volumes are fixed by construction).
    pub fn write(&self, w: &mut Writer) {
        w.f(&[self.volume_m3]);
        w.f(&self.sections.conc);
        w.f(&[self.sections.fallout]);
        w.f(&self.vapour_kg);
        w.f(&self.on_aerosol_kg[0]);
        w.f(&self.on_aerosol_kg[1]);
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
        for c in 0..2 {
            for x in self.on_aerosol_kg[c].iter_mut() {
                *x = r.f();
            }
        }
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
            let ak: Vec<f64> = (0..n)
                .map(|k| {
                    let d = self.sections.diameter(k);
                    self.sections.number(k) * std::f64::consts::PI * d * d * v * 2.0 * d_vap / d
                })
                .collect();
            let total_ak: f64 = ak.iter().sum();
            let held = self.on_aerosol(c);
            let mut lumped = vec![CondensingSurface { ak: total_ak, c_sat: class.saturation_concentration(medium.t_k), mass: held }];
            for (s, held) in surfaces.iter().zip(&self.on_surface_kg) {
                lumped.push(CondensingSurface { ak: s.surface.area * s.k_vapour, c_sat: class.saturation_concentration(s.t_k), mass: held[c] });
            }
            // A surface holding nothing whose saturation is above the gas
            // concentration can neither take vapour nor give any, so it
            // takes no part (left in, it would enter as a huge evaporation
            // that the limit below takes back: exact in arithmetic, but a
            // cancellation of large numbers in floating point, found with
            // I2 on warm surfaces).
            // A holding certain to empty within the step evaporates whole,
            // up front (found 2026-10-29: CsOH on aerosol in 1500 K gas).
            // In the solve its unlimited evaporation would run far past what
            // it holds, and the limit below would take the excess back: a
            // cancellation of large numbers that lost 6e-8 of the vapour.
            // The criterion is exact: the gas can never exceed `c_max`, its
            // concentration with every holding evaporated, so a holding
            // whose evaporation at that concentration takes all it has in
            // `dt` empties for certain.
            let c_max = (self.vapour_kg[c] + lumped.iter().map(|s| s.mass).sum::<f64>()) / v;
            for s in lumped.iter_mut() {
                if s.mass > 0.0 && s.c_sat > c_max && s.ak * (s.c_sat - c_max) * dt >= s.mass {
                    self.vapour_kg[c] += s.mass;
                    s.mass = 0.0;
                }
            }
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
            let d = self.spread(c, lumped[0].mass - held, &ak);
            if self.condensate_mass {
                for (x, dk) in self.sections.conc.iter_mut().zip(&d) {
                    *x = (*x + dk / (medium.rho_p * v)).max(0.0);
                }
            }
            for (held, s) in self.on_surface_kg.iter_mut().zip(&lumped[1..]) {
                held[c] = s.mass;
            }
        }
        // 2. Agglomeration, carrying each class with its particles.
        self.lump();
        let d: Vec<f64> = (0..n).map(|k| self.sections.diameter(k)).collect();
        let before: f64 = self.sections.conc.iter().sum();
        let fallout_before = self.sections.fallout;
        let mut fallout_c = vec![0.0; 2];
        if before > 0.0 {
            let table = medium.kernel_table(&d);
            fallout_c = self.sections.coagulate_with(dt, |i, j| table[i][j], &mut self.on_aerosol_kg);
        }
        // 3. Deposition, and the fallout onto the first floor; what is on
        //    the aerosol goes with it, section by section.
        let surf: Vec<Surface> = surfaces.iter().map(|s| s.surface.clone()).collect();
        let (deposited, deposited_c) = self.sections.deposit_with(medium, &surf, v, dt, &mut self.on_aerosol_kg);
        let fallout = self.sections.fallout - fallout_before;
        if before > 0.0 {
            let floor = surfaces.iter().position(|s| s.surface.facing == super::aerosol::Facing::Floor);
            let mut shares: Vec<f64> = deposited.iter().map(|x| x / before).collect();
            if let Some(i) = floor {
                shares[i] += fallout / before;
            }
            for (i, share) in shares.iter().enumerate() {
                for c in 0..2 {
                    self.on_surface_kg[i][c] += deposited_c[c][i] + if Some(i) == floor { fallout_c[c] } else { 0.0 };
                }
                self.aerosol_on_surface_kg[i] += share * before * v * medium.rho_p;
            }
        }
    }

    /// Add `delta` kg of class `c` to the aerosol (condensation, `delta >
    /// 0`) in proportion to each section's mass-transfer coefficient `ak`,
    /// or take it away (evaporation) in the same proportion, limited to
    /// what each section holds, the rest from the others in proportion to
    /// what they hold.
    /// Returns the change in each section, kg.
    fn spread(&mut self, c: usize, delta: f64, ak: &[f64]) -> Vec<f64> {
        let total_ak: f64 = ak.iter().sum();
        let before = self.on_aerosol_kg[c].clone();
        let change = |held: &Vec<f64>| held.iter().zip(&before).map(|(a, b)| a - b).collect::<Vec<f64>>();
        if delta == 0.0 {
            return vec![0.0; ak.len()];
        }
        let held = &mut self.on_aerosol_kg[c];
        if delta > 0.0 || total_ak <= 0.0 {
            if total_ak > 0.0 {
                for (x, a) in held.iter_mut().zip(ak) {
                    *x += delta * a / total_ak;
                }
            } else {
                // No aerosol area: nothing condensed onto it (the solve
                // gives delta = 0); kept for completeness.
                held[0] += delta;
            }
            return change(held);
        }
        let want = -delta;
        let mut taken = 0.0;
        for (x, a) in held.iter_mut().zip(ak) {
            let t = (want * a / total_ak).min(*x);
            *x -= t;
            taken += t;
        }
        let left = want - taken;
        let now: f64 = held.iter().sum();
        if left > 0.0 && now > 0.0 {
            let f = (left / now).min(1.0);
            for x in held.iter_mut() {
                *x -= *x * f;
            }
        }
        change(held)
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
                cs_out += cv[CS] + ca[CS].iter().sum::<f64>();
                aer_out += moved.iter().sum::<f64>() * medium.rho_p;
            }
        }
        println!(
            "Cs: vapour {:.4} on aerosol {:.4} on floor {:.4} on wall {:.4} out {:.4} of {:.3} kg; aerosol by section {:?}",
            vol.vapour_kg[CS], vol.on_aerosol(CS), vol.on_surface_kg[0][CS], vol.on_surface_kg[1][CS], cs_out, cs_in,
            vol.sections.conc.iter().map(|c| format!("{:.1e}", c)).collect::<Vec<_>>()
        );
        let cs_now = vol.total_kg(CS);
        assert!((cs_now + cs_out - cs_in).abs() < 1e-9 * cs_in, "{cs_now} {cs_out} {cs_in}");
        let aer_now = vol.aerosol_kg(medium.rho_p) + vol.aerosol_on_surface_kg.iter().sum::<f64>();
        assert!((aer_now + aer_out - aer_in).abs() < 1e-9 * aer_in, "{aer_now} {aer_out} {aer_in}");
        assert!(vol.sections.conc[0] < vol.sections.conc[1..].iter().sum::<f64>(), "{:?}", vol.sections.conc);
        assert!(vol.on_aerosol(CS) > 0.8 * cs_in && vol.vapour_kg[CS] < 1e-6, "{vol:?}");
        let peak = (0..20).max_by(|&a, &b| vol.sections.conc[a].total_cmp(&vol.sections.conc[b])).unwrap();
        assert!((9..=13).contains(&peak), "{peak}");
        let hot = FpClass::Cs.saturation_concentration(1500.0) * 3400.0;
        assert!(hot > 1.0, "{hot}");
    }

    /// Cesium condenses onto the aerosol by area, not by volume
    /// (2026-10-29): equal aerosol volumes at 0.1 um and 3.2 um take cesium
    /// in the ratio of their mass-transfer coefficients, which with
    /// Sherwood 2 is number x diameter, i.e. per volume 1/d^2: (3.2/0.1)^2
    /// = 1024 (the 0.1 um section's slip-free value). The lumped ablation
    /// gives the two the same share. Heated past CsOH's saturation, the
    /// aerosol gives it all back (the evaporation path), conserved with
    /// what the fallout took to the floor (a floor taking no vapour: with
    /// no floor at all, the cesium in the fallout has nowhere to go, as in
    /// the lumped model before; the plant's volumes all have one).
    #[test]
    fn classes_condense_by_area_unless_lumped() {
        let cold = Medium::air(400.0, 3.0e5, 2.3e-5);
        let gases = [(BulkGas::Steam, 0.5), (BulkGas::N2, 0.5)];
        let mut ratios = vec![];
        for lumped in [false, true] {
            let floor = [RnSurface { surface: Surface { area: 1.0, facing: Facing::Floor, phoretic: vec![0.0; 16] }, t_k: 400.0, k_vapour: 0.0 }];
            let mut vol = RnVolume::new(100.0, Sections::new(0.1e-6, 16, 2.0), 1);
            vol.lumped = lumped;
            vol.sections.conc[0] = 1e-9;
            vol.sections.conc[15] = 1e-9;
            vol.vapour_kg = [1e-6, 0.0];
            vol.step(&cold, &gases, &floor, 1e-3);
            // A step this short barely agglomerates: compare the two loaded
            // sections' cesium per aerosol volume.
            let per = |k: usize| vol.on_aerosol_kg[CS][k] / vol.sections.conc[k];
            ratios.push(per(0) / per(15));
            let total = vol.total_kg(CS);
            assert!((total - 1e-6).abs() < 1e-18, "{total}");
            let hot = Medium::air(1500.0, 3.0e5, 5.5e-5);
            vol.step(&hot, &gases, &floor, 10.0);
            assert!(vol.on_aerosol(CS).abs() < 1e-20 && (vol.total_kg(CS) - 1e-6).abs() < 1e-18, "{vol:?}");
        }
        assert!((ratios[0] / 1024.0 - 1.0).abs() < 0.02, "{ratios:?}");
        assert!((ratios[1] - 1.0).abs() < 1e-9, "{ratios:?}");
    }

    /// With `condensate_mass`, cesium condensed on the aerosol adds to
    /// the particles' mass exactly, section by section, and leaves it
    /// again on evaporation (2026-10-29): the particle mass less the cesium
    /// on it stays the UO2 put in, to 1e-12.
    #[test]
    fn condensate_adds_to_the_particles() {
        let cold = Medium::air(400.0, 3.0e5, 2.3e-5);
        let hot = Medium::air(1500.0, 3.0e5, 5.5e-5);
        let gases = [(BulkGas::Steam, 0.5), (BulkGas::N2, 0.5)];
        let mut vol = RnVolume::new(100.0, Sections::new(0.1e-6, 16, 2.0), 0);
        vol.condensate_mass = true;
        vol.sections.conc[3] = 1e-9;
        let uo2 = vol.aerosol_kg(cold.rho_p);
        vol.vapour_kg = [1e-4, 0.0];
        vol.step(&cold, &gases, &[], 1e-3);
        let on = vol.on_aerosol(CS);
        assert!(on > 1e-8, "{on}");
        assert!((vol.aerosol_kg(cold.rho_p) - on - uo2).abs() < 1e-12 * uo2.max(on), "{} {on} {uo2}", vol.aerosol_kg(cold.rho_p));
        vol.step(&hot, &gases, &[], 1e-3);
        assert!(vol.on_aerosol(CS) < 1e-15 && (vol.aerosol_kg(hot.rho_p) - uo2).abs() < 1e-12 * uo2.max(on), "{}", vol.aerosol_kg(hot.rho_p));
    }
}
