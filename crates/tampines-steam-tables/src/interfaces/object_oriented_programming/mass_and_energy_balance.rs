//! Transient mass-and-energy balance for a [`TampinesSteamTableCV`] control
//! volume.
//!
//! Design (human-authored — see the README changelog entry "Transient mass &
//! energy balance"):
//!
//! A control volume has a **fixed geometric volume**. Over a timestep, streams
//! add or remove mass, each stream carrying its own specific enthalpy. The
//! caller accumulates these `(mass, specific-enthalpy)` source/sink terms in a
//! [`CvMassEnthalpyChanges`] ledger (built *outside* the control volume so the
//! `Copy` control-volume stays `Copy`), then calls
//! [`TampinesSteamTableCV::advance_timestep`] to apply them.
//!
//! Applying a timestep is a conservation step followed by a flash:
//!
//! 1. **Mass:** `m_new = m_old + Σ dm_i`, where `m_old = V / v` (volume over
//!    specific volume). Removal terms carry a negative `dm_i`.
//! 2. **Energy:** `H_new = H_old + Σ dm_i · h_i`, where `H_old = m_old · h_old`.
//!    The new specific enthalpy is `h_new = H_new / m_new`.
//! 3. **New state point:** the volume is fixed, so the new specific volume is
//!    `v_new = V / m_new` (equivalently the new density `ρ_new = m_new / V`).
//!    The system now sits at a new `(ρ, h)` thermodynamic point. IF97 forward
//!    flashes are `(p, h)`-based, so we recover the pressure by solving
//!    `v(p, h_new) = v_new` for `p` with **regula falsi** (false position),
//!    then rebuild the control volume from `(p, h_new)`.

use uom::si::f64::*;
use uom::si::pressure::{megapascal, pascal};
use uom::si::specific_volume::cubic_meter_per_kilogram;

use crate::interfaces::functional_programming::ph_flash_eqm::v_ph_eqm;

/// Ordered ledger of mass source/sink terms applied to a control volume over a
/// single timestep.
///
/// Each entry is a `(mass, specific-enthalpy)` pair: the amount of mass crossing
/// the boundary and the specific enthalpy that mass carries. Mass added to the
/// system is stored with a positive mass; mass removed is stored with a negative
/// mass (use [`add_mass`](Self::add_mass) / [`remove_mass`](Self::remove_mass) so
/// the sign convention is handled for you).
///
/// This is deliberately a separate, owned (non-`Copy`) type: the control volume
/// itself stays a small `Copy` value, and the variable-length list of pending
/// changes lives here. Build one per timestep (or reuse it and
/// [`clear`](Self::clear) between timesteps), then hand it to
/// [`TampinesSteamTableCV::advance_timestep`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CvMassEnthalpyChanges {
    /// `(signed mass, specific enthalpy of that mass)` — positive mass = added,
    /// negative mass = removed.
    changes: Vec<(Mass, AvailableEnergy)>,
}

impl CvMassEnthalpyChanges {
    /// Creates an empty ledger.
    pub fn new() -> Self {
        Self { changes: Vec::new() }
    }

    /// Records mass **added** to the control volume.
    ///
    /// * `mass` — amount of mass entering (must be ≥ 0; it is stored as a
    ///   positive term).
    /// * `specific_enthalpy` — the specific enthalpy carried by the incoming
    ///   stream (J/kg).
    pub fn add_mass(&mut self, mass: Mass, specific_enthalpy: AvailableEnergy) {
        self.changes.push((mass, specific_enthalpy));
    }

    /// Records mass **removed** from the control volume.
    ///
    /// * `mass` — amount of mass leaving, given as a positive quantity; it is
    ///   stored internally as a negative term.
    /// * `specific_enthalpy` — the specific enthalpy carried out by the leaving
    ///   stream (J/kg). For an outflow that is at the control volume's own
    ///   state, pass `cv.get_specific_enthalpy()`.
    pub fn remove_mass(&mut self, mass: Mass, specific_enthalpy: AvailableEnergy) {
        self.changes.push((-mass, specific_enthalpy));
    }

    /// Removes all recorded changes, leaving an empty ledger ready for the next
    /// timestep.
    pub fn clear(&mut self) {
        self.changes.clear();
    }

    /// `true` if no changes have been recorded.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Number of recorded source/sink terms.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Net mass change `Σ dm_i` over all recorded terms (positive = net gain).
    pub fn total_mass_change(&self) -> Mass {
        self.changes
            .iter()
            .fold(Mass::default(), |acc, &(dm, _)| acc + dm)
    }

    /// Net enthalpy change `Σ dm_i · h_i` over all recorded terms — the total
    /// (extensive) enthalpy carried across the boundary this timestep.
    pub fn total_enthalpy_change(&self) -> Energy {
        self.changes
            .iter()
            .fold(Energy::default(), |acc, &(dm, h)| acc + dm * h)
    }
}

impl super::TampinesSteamTableCV {
    /// Applies one timestep of mass-and-energy exchange recorded in `changes`,
    /// updating the control volume in place to its new thermodynamic state.
    ///
    /// The geometric volume is held fixed (control-volume definition). See the
    /// [module documentation](self) for the full derivation. In brief:
    ///
    /// 1. `m_new = m_old + Σ dm_i`
    /// 2. `h_new = (m_old · h_old + Σ dm_i · h_i) / m_new`
    /// 3. `v_new = V / m_new`, then solve `v(p, h_new) = v_new` for `p` by
    ///    regula falsi and rebuild the state from `(p, h_new)`.
    ///
    /// # Panics
    ///
    /// Panics if the resulting mass is not strictly positive (the control volume
    /// would be emptied or driven negative), or if no pressure in the IF97 range
    /// reproduces the target `(v_new, h_new)` state — both indicate
    /// non-physical inputs rather than a recoverable condition.
    pub fn advance_timestep(&mut self, changes: &CvMassEnthalpyChanges) {
        let volume = self.get_volume();
        let v_old = self.get_specific_volume();
        let h_old = self.get_specific_enthalpy();

        // current extensive state
        let m_old: Mass = volume / v_old;
        let big_h_old: Energy = m_old * h_old;

        // accumulated changes this timestep
        let m_new: Mass = m_old + changes.total_mass_change();
        let big_h_new: Energy = big_h_old + changes.total_enthalpy_change();

        assert!(
            m_new.get::<uom::si::mass::kilogram>() > 0.0,
            "advance_timestep: resulting control-volume mass is non-positive \
             ({m_new:?}); cannot remove more mass than is present"
        );

        // new intensive state point (ρ, h): fixed volume sets the density
        let h_new: AvailableEnergy = big_h_new / m_new;
        let v_new: SpecificVolume = volume / m_new;

        // recover pressure from (v_new, h_new) by regula falsi (seeded at the
        // current pressure, which is a known-valid point), then rebuild
        let p_new =
            pressure_from_specific_volume_and_enthalpy(v_new, h_new, self.get_pressure());
        *self = Self::new_from_ph(p_new, h_new, volume);
    }
}

/// Solves `v(p, h) = v_target` for pressure by regula falsi (false position),
/// where `v(p, h)` is the IF97 equilibrium specific volume [`v_ph_eqm`].
///
/// At fixed enthalpy the specific volume decreases monotonically as pressure
/// rises (a denser state), so the residual `v(p, h) − v_target` has a single
/// sign change and false position converges quickly.
///
/// The bracket is grown **outward from `p_guess`** — the control volume's
/// current pressure, which is a known-valid `(p, h)` point — toward the root,
/// rather than from a blind high-pressure endpoint. This keeps every evaluation
/// inside the validated IF97 range: the `(p, h)` flashes are not implemented for
/// region 5 (T > 800 °C), so marching down from 100 MPa could land there and
/// panic. The expansion is still clamped to the steam-table range
/// `[triple point, 100 MPa]` as a hard safety net.
///
/// # Panics
///
/// Panics if no pressure within the steam-table range reproduces `v_target` at
/// this enthalpy (the requested `(v, h)` point is outside the validated range).
fn pressure_from_specific_volume_and_enthalpy(
    v_target: SpecificVolume,
    h: AvailableEnergy,
    p_guess: Pressure,
) -> Pressure {
    // hard safety net: triple-point pressure (with the usual margin) to IF97 max
    let p_min = Pressure::new::<megapascal>(0.000_611_212_677 * 1.01).get::<pascal>();
    let p_max = Pressure::new::<megapascal>(100.0).get::<pascal>();

    let v_target_val = v_target.get::<cubic_meter_per_kilogram>();

    // residual in m³/kg; positive means the state at p is lighter than target
    let residual = |p_pa: f64| -> f64 {
        let p = Pressure::new::<pascal>(p_pa);
        v_ph_eqm(p, h).get::<cubic_meter_per_kilogram>() - v_target_val
    };

    let p0 = p_guess.get::<pascal>().clamp(p_min, p_max);
    let f0 = residual(p0);

    // grow the bracket from p_guess toward the root. residual decreases with p,
    // so f0 > 0 ⇒ the root is at higher pressure (denser), f0 < 0 ⇒ lower.
    let factor = 1.4_f64;
    let (mut p_lo, mut f_lo, mut p_hi, mut f_hi);
    if f0 >= 0.0 {
        p_lo = p0;
        f_lo = f0;
        p_hi = (p0 * factor).min(p_max);
        f_hi = residual(p_hi);
        while f_lo * f_hi > 0.0 && p_hi < p_max {
            p_lo = p_hi;
            f_lo = f_hi;
            p_hi = (p_hi * factor).min(p_max);
            f_hi = residual(p_hi);
        }
    } else {
        p_hi = p0;
        f_hi = f0;
        p_lo = (p0 / factor).max(p_min);
        f_lo = residual(p_lo);
        while f_lo * f_hi > 0.0 && p_lo > p_min {
            p_hi = p_lo;
            f_hi = f_lo;
            p_lo = (p_lo / factor).max(p_min);
            f_lo = residual(p_lo);
        }
    }

    assert!(
        f_lo * f_hi <= 0.0,
        "pressure_from_specific_volume_and_enthalpy: no pressure in the \
         steam-table range reproduces v_target={v_target:?} at h={h:?} \
         (started from p_guess={p_guess:?})"
    );

    // regula falsi (false position) with a bisection safeguard against stalling
    let tol = (v_target_val.abs() * 1e-9).max(1e-12); // m³/kg residual tolerance
    let mut p_root = p_lo;
    let mut stalls_lo = 0;
    let mut stalls_hi = 0;
    for _ in 0..200 {
        // false-position estimate
        let denom = f_hi - f_lo;
        p_root = if denom.abs() < f64::MIN_POSITIVE {
            0.5 * (p_lo + p_hi)
        } else {
            p_hi - f_hi * (p_hi - p_lo) / denom
        };
        // keep the estimate inside the bracket
        if !(p_root > p_lo && p_root < p_hi) {
            p_root = 0.5 * (p_lo + p_hi);
        }

        let f_root = residual(p_root);
        if f_root.abs() < tol || (p_hi - p_lo) < 1e-6 {
            break;
        }

        if f_root * f_lo > 0.0 {
            p_lo = p_root;
            f_lo = f_root;
            stalls_lo = 0;
            stalls_hi += 1;
            // Illinois weighting: relax the stuck high endpoint
            if stalls_hi >= 2 {
                f_hi *= 0.5;
                stalls_hi = 0;
            }
        } else {
            p_hi = p_root;
            f_hi = f_root;
            stalls_hi = 0;
            stalls_lo += 1;
            if stalls_lo >= 2 {
                f_lo *= 0.5;
                stalls_lo = 0;
            }
        }
    }

    Pressure::new::<pascal>(p_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::object_oriented_programming::TampinesSteamTableCV;
    use uom::si::available_energy::kilojoule_per_kilogram;
    use uom::si::mass::kilogram;
    use uom::si::pressure::megapascal;
    use uom::si::volume::cubic_meter;

    /// Adding mass at the control volume's own specific enthalpy must leave the
    /// specific enthalpy unchanged, raise the mass by exactly the amount added,
    /// drop the specific volume to `V / m_new` (regula falsi must recover that
    /// density), and — since density rises at constant enthalpy — raise the
    /// pressure.
    #[test]
    fn add_mass_at_same_enthalpy_conserves_h_and_recovers_density() {
        let volume = Volume::new::<cubic_meter>(10.0);
        let p0 = Pressure::new::<megapascal>(5.0);
        let h0 = AvailableEnergy::new::<kilojoule_per_kilogram>(3400.0); // superheated steam

        let mut cv = TampinesSteamTableCV::new_from_ph(p0, h0, volume);
        let m_old = cv.get_mass();
        let v0 = cv.get_specific_volume();

        let dm = 0.1 * m_old;
        let mut ledger = CvMassEnthalpyChanges::new();
        ledger.add_mass(dm, h0);
        cv.advance_timestep(&ledger);

        approx::assert_relative_eq!(
            cv.get_specific_enthalpy().get::<kilojoule_per_kilogram>(),
            h0.get::<kilojoule_per_kilogram>(),
            max_relative = 1e-4
        );
        approx::assert_relative_eq!(
            cv.get_mass().get::<kilogram>(),
            (m_old + dm).get::<kilogram>(),
            max_relative = 1e-4
        );
        // V / m_new = v0 / 1.1 — confirms the regula-falsi pressure reproduces
        // the target density
        approx::assert_relative_eq!(
            cv.get_specific_volume().get::<cubic_meter_per_kilogram>(),
            (v0 / 1.1).get::<cubic_meter_per_kilogram>(),
            max_relative = 1e-3
        );
        assert!(cv.get_pressure() > p0, "compressing at constant h must raise p");
    }

    /// Removing mass at the control volume's own enthalpy must lower the mass,
    /// keep the specific enthalpy, raise the specific volume, and drop the
    /// pressure.
    #[test]
    fn remove_mass_at_same_enthalpy_drops_pressure() {
        let volume = Volume::new::<cubic_meter>(2.0);
        let p0 = Pressure::new::<megapascal>(8.0);
        let h0 = AvailableEnergy::new::<kilojoule_per_kilogram>(3200.0);

        let mut cv = TampinesSteamTableCV::new_from_ph(p0, h0, volume);
        let m_old = cv.get_mass();

        let dm = 0.2 * m_old;
        let mut ledger = CvMassEnthalpyChanges::new();
        ledger.remove_mass(dm, cv.get_specific_enthalpy());
        cv.advance_timestep(&ledger);

        approx::assert_relative_eq!(
            cv.get_specific_enthalpy().get::<kilojoule_per_kilogram>(),
            h0.get::<kilojoule_per_kilogram>(),
            max_relative = 1e-4
        );
        approx::assert_relative_eq!(
            cv.get_mass().get::<kilogram>(),
            (m_old - dm).get::<kilogram>(),
            max_relative = 1e-4
        );
        assert!(cv.get_pressure() < p0, "expanding at constant h must lower p");
    }

    /// Mixing in mass at a different enthalpy must land the new specific enthalpy
    /// at the exact mass-weighted average, and total (extensive) enthalpy must be
    /// conserved.
    #[test]
    fn mixing_lands_mass_weighted_enthalpy() {
        let volume = Volume::new::<cubic_meter>(5.0);
        let p0 = Pressure::new::<megapascal>(3.0);
        let h0 = AvailableEnergy::new::<kilojoule_per_kilogram>(3000.0);

        let mut cv = TampinesSteamTableCV::new_from_ph(p0, h0, volume);
        let m_old = cv.get_mass();
        let big_h_old: Energy = m_old * h0;

        let dm = 0.5 * m_old;
        let h_add = AvailableEnergy::new::<kilojoule_per_kilogram>(3600.0);
        let mut ledger = CvMassEnthalpyChanges::new();
        ledger.add_mass(dm, h_add);

        let m_new = m_old + dm;
        let h_expected = (big_h_old + dm * h_add) / m_new;

        cv.advance_timestep(&ledger);

        approx::assert_relative_eq!(
            cv.get_specific_enthalpy().get::<kilojoule_per_kilogram>(),
            h_expected.get::<kilojoule_per_kilogram>(),
            max_relative = 1e-4
        );
        // extensive enthalpy conservation: m_new * h_new == H_old + dm*h_add
        approx::assert_relative_eq!(
            (cv.get_mass() * cv.get_specific_enthalpy()).get::<uom::si::energy::joule>(),
            (big_h_old + dm * h_add).get::<uom::si::energy::joule>(),
            max_relative = 1e-4
        );
    }

    /// Ledger bookkeeping: signed sums are correct and `clear` resets it.
    #[test]
    fn ledger_totals_and_clear() {
        let h = AvailableEnergy::new::<kilojoule_per_kilogram>(2500.0);
        let mut ledger = CvMassEnthalpyChanges::new();
        ledger.add_mass(Mass::new::<kilogram>(3.0), h);
        ledger.remove_mass(Mass::new::<kilogram>(1.0), h);

        approx::assert_relative_eq!(
            ledger.total_mass_change().get::<kilogram>(),
            2.0,
            max_relative = 1e-12
        );
        approx::assert_relative_eq!(
            ledger.total_enthalpy_change().get::<uom::si::energy::joule>(),
            (2.0 * h.get::<uom::si::available_energy::joule_per_kilogram>()),
            max_relative = 1e-12
        );
        ledger.clear();
        assert!(ledger.is_empty());
    }
}
