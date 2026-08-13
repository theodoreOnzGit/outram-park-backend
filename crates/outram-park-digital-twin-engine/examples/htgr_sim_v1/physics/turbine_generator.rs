//! Turbine-generator shaft -- the rotating machine between the steam path and
//! the electrical load.
//!
//! This module exists for one reason: the schematic's [`TurbineVisual`] draws
//! its rotor at `theta = omega * t`, and it will only do so if something in the
//! plant actually computes an `omega`. Everything here is about producing that
//! number honestly rather than picking one that looks right.
//!
//! [`TurbineVisual`]: outram_park_digital_twin_engine::components::TurbineVisual
//!
//! ## The model is not new physics
//!
//! The rotor dynamics are `tampines-steam-tables`'
//! [`ThreePhaseElectricGeneratorTurbine`] -- the same lumped three-phase
//! synchronous-generator torque balance the `fhr_sim_v2` example drives. It is
//! reused, not reimplemented. What this module adds is the *coupling*: where
//! the driving torque comes from, what electrical load the machine works
//! into, and how the machine is sized.
//!
//! The torque balance it advances is
//!
//! ```text
//!   omega^{t+dt} ( I/dt + (N B A)^2 / (eta R) * sum_j cos^2(theta + b_j) )
//!       = I omega^t / dt + T_source
//! ```
//!
//! with the three stator phases at 0, 120 and 240 degrees. That phase sum is
//! **analytically 3/2** for any `theta` (a trigonometric identity; in floating
//! point it holds to about 1e-13 relative, measured in
//! [`tests::the_generator_is_time_invariant`]), so the electrical reaction is a
//! plain viscous damping torque `T_load = k * omega` with
//!
//! ```text
//!   k = 1.5 (N B A)^2 / (eta R)      [N m s / rad]
//! ```
//!
//! and the machine is a first-order system, not a time-varying one.
//!
//! ## Where the driving torque comes from
//!
//! From the enthalpy drop, and from nowhere else. [`super::secondary_loop`]
//! already computes the turbine's mechanical power as
//! `m_dot (h_in - h_out)` through a real IAPWS-IF97 `(p,s)` expansion; this
//! module divides that power by the *current* shaft speed to get the torque:
//!
//! ```text
//!   T_source = P_enthalpy_drop / omega
//! ```
//!
//! so shaft power and enthalpy-drop power are **the same number by
//! construction** -- `T_source * omega == P_enthalpy_drop` identically, not to
//! within a tolerance. There is no second, independent estimate of turbine
//! power anywhere in this plant.
//!
//! The cost of that choice is that the turbine is modelled as a **constant-
//! power prime mover**: torque rises hyperbolically as the shaft slows, because
//! the enthalpy drop in the one-node steam model does not know the blade speed.
//! A real machine's torque falls as the blade-speed ratio `U/C` rises, so this
//! model cannot show the `U/C ~ 0.5` power optimum, and it overstates torque at
//! low speed. A blade-kinematics turbine model (workspace bead `op-dt3.18`) is
//! what removes that.
//!
//! ## Why the speed comes out near synchronous -- read this before quoting it
//!
//! The steady state of `I domega/dt = P/omega - k omega` is
//!
//! ```text
//!   omega_eq = sqrt(P_shaft / k)
//! ```
//!
//! The load resistance is sized by **Ohm's law at the machine's rated point**
//! ([`load_resistance`]): a machine that delivers its rated power at its rated
//! terminal EMF fixes `R = 1.5 (N B A omega_sync)^2 / (eta P_rated)`, which
//! makes `k = P_rated / omega_sync^2` and hence
//!
//! ```text
//!   omega_eq = omega_sync * sqrt(P_shaft / P_rated)
//! ```
//!
//! -- in which `N`, `B`, `A` *and* `eta` have all cancelled out entirely. (An
//! earlier draft of this module predicted a residual `sqrt(eta)` factor. That
//! was wrong; the efficiency cancels, and the measured sweep in
//! [`tests::equilibrium_speed_follows_the_square_root_of_load`] shows exactly
//! 3000.0000 rpm at rated load rather than the predicted 2969.8 rpm. It is
//! recorded here because it is the kind of error that would otherwise look
//! like a physics result.)
//!
//! **This model therefore does not predict 3000 rpm; it reproduces it by
//! construction at rated load.** The only things setting the speed scale are
//! the machine rating and the chosen synchronous speed. Anyone quoting the
//! shaft speed as evidence that the model "gets generator speed right" has
//! misread it. What the model *does* compute, and what is not put in by hand,
//! is the trajectory, the time constants, and how far the speed sits off
//! synchronous when the plant is off its rating.
//!
//! The same cancellation makes the two time constants clean functions of the
//! inertia constant alone -- coast-down `tau = I/k = 2H = 8.0 s`, small-signal
//! `tau = I/2k = H = 4.0 s` -- both confirmed by measurement below.
//!
//! ## This is an ISLANDED machine, not a grid-connected one
//!
//! The `sqrt(P)` law above is the behaviour of a machine feeding a **fixed
//! resistive load with no governor and no automatic voltage regulator**: its
//! speed rises and falls with load. A generator synchronised to a stiff grid
//! does the opposite -- it is *pinned* at grid frequency, and a change in load
//! moves the rotor torque angle, not the speed.
//!
//! The islanded case is modelled deliberately, because it is the case in which
//! the shaft speed is an **output of a torque balance** rather than a boundary
//! condition imposed by the grid. A grid-synchronous model would draw the same
//! rotor at a constant speed that no equation in this plant had computed, which
//! is exactly the fabricated animation this module exists to avoid. The
//! consequence, stated plainly: **the shaft speed here responds to plant load,
//! and a real grid-connected 10 MWth plant's would not.**
//!
//! ## The drawn rotor is temporally ALIASED, and cannot not be
//!
//! Measured on screen 2026-08-12: with the plant at 2576 rpm the shaft turns
//! **43 revolutions per second**, and the GUI repaints at 60 Hz. The rotor
//! phase therefore advances about **4.5 rad (258 degrees) between consecutive
//! frames**, far past the Nyquist limit for a 20-blade rotor whose pattern
//! repeats every 18 degrees. What a viewer sees is a wagon-wheel rendering:
//! the blade pattern drifts a few degrees per frame while the single white
//! marker blade jumps to an essentially unrelated position each frame.
//!
//! Objectively confirmed rather than assumed: the mean absolute luma
//! difference over the rotor region between consecutive 60 fps frames measured
//! **1.3 to 5.4 of 255** across a 1.5 s capture, i.e. never zero -- the rotor
//! is animating every frame, it is simply undersampled.
//!
//! **This is not fixable by choosing a nicer number.** Any real steam-turbine
//! speed (3000 or 3600 rpm synchronous, faster still for a small geared
//! machine) is one to two orders of magnitude above a 60 Hz display, exactly as
//! filming a real turbine at 60 fps would be. The alternative -- feeding the
//! widget a slowed "display clock" -- would draw a shaft speed no equation
//! computed, which is the thing this module exists to avoid. The aliasing is
//! therefore accepted and documented, and the `n_shaft` readout beside the
//! machine is what to read the speed from.
//!
//! A second, smaller display caveat: `TurbineVisual` defines its rotor phase as
//! `theta = omega(t) * t`, which is the true accumulated angle only while
//! `omega` is constant. During a speed transient the drawn phase advances at
//! `omega + t domega/dt`, not `omega`. The shaft is started at synchronous
//! speed partly to keep the machine in the near-constant-speed regime where
//! that matters least, but it is a property of the widget, not something this
//! module can correct.
//!
//! ## What is published and what is invented
//!
//! **Nothing here is published.** IAEA-TECDOC-1382 is a reactor-physics
//! benchmark and carries no turbine-generator detail whatsoever -- no rating,
//! no speed, no inertia, no electrical data. Every constant below is an
//! illustrative balance-of-plant choice in the same sense as the condenser
//! pressure and the turbine efficiency in [`super::secondary_loop`], and none
//! of it may be cited as HTR-10 data. The machine *rating* is the one quantity
//! traceable to the published steam conditions, and even that is filtered
//! through the invented condenser pressure and turbine efficiency (see
//! [`super::secondary_loop::design_point_turbine_power`]).
//!
//! This is a demonstration model, not a validated turbine-generator model.

use tampines_steam_tables::steam_turbine_equations::generator::ThreePhaseElectricGeneratorTurbine;
use uom::si::angular_velocity::{radian_per_second, revolution_per_minute};
use uom::si::area::square_meter;
use uom::si::electrical_resistance::ohm;
use uom::si::f64::{
    AngularVelocity, ElectricalResistance, MagneticFluxDensity, MomentOfInertia, Power, Time,
    Torque,
};
use uom::si::magnetic_flux_density::tesla;
use uom::si::moment_of_inertia::kilogram_square_meter;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::torque::newton_meter;

// ---------------------------------------------------------------------------
// ILLUSTRATIVE MACHINE PARAMETERS -- none of this is published HTR-10 data.
// See the module docs.
// ---------------------------------------------------------------------------

/// Electrical frequency the machine is sized around \[Hz\] (**illustrative**),
/// 50 Hz.
///
/// Only used to define [`synchronous_speed`], i.e. the speed the machine is
/// *rated* at. The shaft is not locked to it -- see the module docs on why this
/// is an islanded machine.
const GRID_FREQUENCY_HZ: f64 = 50.0;

/// Rotor pole pairs (**illustrative**), 1 -- a two-pole machine, so
/// synchronous speed is one electrical cycle per revolution: 3000 rpm at
/// [`GRID_FREQUENCY_HZ`].
const POLE_PAIRS: f64 = 1.0;

/// Generator efficiency (**illustrative**), 0.98 -- the fraction of shaft
/// power delivered to the electrical load. Matches the value the
/// `tampines-steam-tables` preset carries.
const GENERATOR_EFFICIENCY: f64 = 0.98;

/// Inertia constant `H` \[s\] (**invented**): stored rotor kinetic energy at
/// rated speed divided by the machine rating, `H = 0.5 I omega_s^2 / S`.
///
/// Sets the rotor moment of inertia via [`rotor_inertia`] and therefore the
/// run-up and coast-down time, but **not** the steady speed. 4 s is in the
/// range usually quoted for steam turbine-generator sets; it is chosen here as
/// a plausible round number, not taken from a specific machine or a cited
/// source.
const INERTIA_CONSTANT_S: f64 = 4.0;

/// Stator coil turns (**illustrative**).
const COIL_TURNS: usize = 34;

/// Rotor air-gap flux density \[T\] (**illustrative**).
const FLUX_DENSITY_T: f64 = 1.0;

/// Stator coil area \[m^2\] (**illustrative**).
const COIL_AREA_M2: f64 = 0.5;

/// Smallest shaft speed \[rad/s\] the constant-power torque law is evaluated
/// at -- a **numerical guard**, not physics.
///
/// `T = P / omega` is singular at standstill. Below this floor the torque is
/// held at `P / omega_floor` instead, which means the injected shaft power
/// `T * omega` is *less* than the enthalpy-drop power there. That is the one
/// place in this model where the two disagree, and it is confined to a
/// practically-stopped shaft.
///
/// 0.1 rad/s is about 1 rpm. Since the equilibrium speed is
/// `omega_sync sqrt(P/P_rated)`, reaching it needs the turbine power to fall
/// below `P_rated (0.1/314.16)^2` -- **under half a watt** on a 3.43 MW
/// machine. An earlier draft used 1.0 rad/s, which measurement showed was
/// within a factor of 5 of the equilibrium speed at 1 kW of shaft power; it was
/// lowered rather than left to bind. [`tests::the_torque_floor_never_binds_in_normal_operation`]
/// checks it stays clear across the plant's reachable range.
const OMEGA_FLOOR_RAD_PER_S: f64 = 0.1;

/// Synchronous speed the machine is rated at: `2 pi f / pole_pairs`.
///
/// 314.159 rad/s = 3000 rpm for a two-pole 50 Hz machine. This is a **rating**,
/// not a constraint on the shaft -- see the module docs.
pub fn synchronous_speed() -> AngularVelocity {
    AngularVelocity::new::<radian_per_second>(
        2.0 * std::f64::consts::PI * GRID_FREQUENCY_HZ / POLE_PAIRS,
    )
}

/// Peak per-phase back-EMF constant `N B A` \[Wb\] of the illustrative
/// machine. Instantaneous phase EMF is `N B A omega cos(theta + b_j)`.
fn emf_constant() -> f64 {
    COIL_TURNS as f64 * FLUX_DENSITY_T * COIL_AREA_M2
}

/// Resistive load \[ohm\] the generator works into, sized by **Ohm's law at
/// the machine's rated point**.
///
/// # Method
///
/// The three-phase electrical power the model delivers is
/// `P_elec = 1.5 (N B A omega)^2 / R`, and because `1.5 E_peak^2 = 3 E_rms^2`
/// that is exactly the textbook `3 E_rms^2 / R`. Requiring the machine to
/// absorb its rated *shaft* power at synchronous speed, `P_rated = P_elec/eta`,
/// gives
///
/// ```text
///   R = 1.5 (N B A omega_sync)^2 / (eta P_rated)
/// ```
///
/// This is a **sizing calculation, not a fit to a desired animation speed**;
/// it is the same step a designer takes when matching a machine to its load
/// bank. It is nonetheless the reason the shaft settles near synchronous --
/// see the module docs, which spell out that the coil parameters cancel and
/// the model cannot independently predict 3000 rpm.
pub fn load_resistance(rated_shaft_power: Power) -> ElectricalResistance {
    let peak_emf = emf_constant() * synchronous_speed().get::<radian_per_second>();
    ElectricalResistance::new::<ohm>(
        1.5 * peak_emf * peak_emf / (GENERATOR_EFFICIENCY * rated_shaft_power.get::<watt>()),
    )
}

/// Combined turbine + generator rotor moment of inertia \[kg m^2\], from the
/// inertia constant: `I = 2 H S / omega_sync^2`.
///
/// The load is purely resistive, so the power factor is unity and the machine
/// rating `S` in VA equals the rated power in W -- no separate apparent-power
/// figure is needed or invented.
pub fn rotor_inertia(rated_shaft_power: Power) -> MomentOfInertia {
    let omega_s = synchronous_speed().get::<radian_per_second>();
    MomentOfInertia::new::<kilogram_square_meter>(
        2.0 * INERTIA_CONSTANT_S * rated_shaft_power.get::<watt>() / (omega_s * omega_s),
    )
}

/// Electrical damping coefficient `k = 1.5 (N B A)^2 / (eta R)`
/// \[N m s / rad\] -- the constant of proportionality in the load torque
/// `T_load = k omega`.
///
/// Exposed because it is what actually sets the steady speed
/// (`omega_eq = sqrt(P/k)`) and the mechanical time constant
/// (`tau = I / 2k`), and both are worth being able to check from outside.
#[allow(dead_code)] // used by the V&V tests; a diagnostics candidate for the GUI
pub fn electrical_damping(load_resistance: ElectricalResistance) -> f64 {
    let nba = emf_constant();
    1.5 * nba * nba / (GENERATOR_EFFICIENCY * load_resistance.get::<ohm>())
}

/// Build the illustrative machine at a given shaft speed.
///
/// Used by the plant to construct its own shaft, and by the schematic to hand
/// [`outram_park_digital_twin_engine::components::TurbineVisual`] a generator
/// carrying the speed the physics thread computed (the shared snapshot is
/// scalar-only, so the widget's model is rebuilt from the published `omega`
/// rather than shared across threads).
pub fn generator_at_speed(
    omega: AngularVelocity,
    rated_shaft_power: Power,
) -> ThreePhaseElectricGeneratorTurbine {
    ThreePhaseElectricGeneratorTurbine::new(
        MagneticFluxDensity::new::<tesla>(FLUX_DENSITY_T),
        uom::si::f64::Area::new::<square_meter>(COIL_AREA_M2),
        COIL_TURNS,
        rotor_inertia(rated_shaft_power),
        uom::si::f64::Ratio::new::<ratio>(GENERATOR_EFFICIENCY),
        omega,
    )
}

/// The turbine-generator rotor: a real torque balance driven by the secondary
/// loop's enthalpy-drop power, working into a resistive load.
pub struct TurbineGeneratorShaft {
    /// The reused `tampines-steam-tables` rotor + three-phase stator model.
    generator: ThreePhaseElectricGeneratorTurbine,
    /// Resistive electrical load \[ohm\], sized by [`load_resistance`].
    load_resistance: ElectricalResistance,
    /// Machine rating \[W\] -- the design-point shaft power (see
    /// [`super::secondary_loop::design_point_turbine_power`]).
    rated_shaft_power: Power,
    /// Driving torque applied on the most recent step.
    shaft_torque: Torque,
    /// Three-phase electrical power delivered to the load on the most recent
    /// step.
    electrical_power: Power,
}

impl TurbineGeneratorShaft {
    /// Construct the machine **already at synchronous speed**.
    ///
    /// The rest of this simulator opens at the plant's nominal operating point
    /// rather than cold (see [`crate::app::state::HtgrSnapshot::default`]), so
    /// a turbine-generator at rest would be the odd one out. It also keeps the
    /// shaft in the regime where the widget's `theta = omega * t` is a faithful
    /// accumulated angle: that identity is only exact for constant `omega`, so
    /// starting from rest and rolling up would draw a rotor phase advancing at
    /// `omega + t domega/dt`, faster than the shaft actually turns.
    ///
    /// The first few seconds are still a transient -- the secondary loop starts
    /// at zero turbine power and its feedwater controller takes tens of seconds
    /// to settle -- so the shaft dips below synchronous and recovers. That is
    /// the model starting up, not the plant.
    pub fn new() -> Self {
        let rated_shaft_power = super::secondary_loop::design_point_turbine_power();
        let load_resistance = load_resistance(rated_shaft_power);
        Self {
            generator: generator_at_speed(synchronous_speed(), rated_shaft_power),
            load_resistance,
            rated_shaft_power,
            shaft_torque: Torque::new::<newton_meter>(0.0),
            electrical_power: Power::new::<watt>(0.0),
        }
    }

    /// Advance the shaft by `dt` under the turbine's `shaft_power`.
    ///
    /// `shaft_power` must be the secondary loop's own enthalpy-drop power
    /// ([`super::secondary_loop::SteamSecondaryLoop::turbine_power`]); passing
    /// anything else would put two disagreeing turbine powers in the plant.
    ///
    /// `current_time` is the plant clock the generator evaluates its phase
    /// angles at. The three-phase sum is identically 3/2, so the result does
    /// not actually depend on it -- it is passed through for the model's
    /// signature and pinned by [`tests::the_generator_is_time_invariant`].
    pub fn step(&mut self, dt: Time, shaft_power: Power, current_time: Time) {
        let omega_rad_s = self
            .generator
            .get_omega()
            .get::<radian_per_second>()
            .max(OMEGA_FLOOR_RAD_PER_S);

        // T = P / omega. `uom` gives Power/AngularVelocity the dimension of
        // energy (radians are dimensionless), so the newton-metre value is
        // moved across explicitly -- the same conversion the generator model
        // itself makes internally for its momentum term.
        self.shaft_torque =
            Torque::new::<newton_meter>(shaft_power.get::<watt>().max(0.0) / omega_rad_s);

        self.generator
            .advance_timestep(self.shaft_torque, self.load_resistance, current_time, dt);

        self.electrical_power = self.generator.get_power(self.load_resistance, current_time);
    }

    /// Current shaft angular velocity -- the quantity the turbine widget draws
    /// its rotor phase from.
    pub fn angular_velocity(&self) -> AngularVelocity {
        self.generator.get_omega()
    }

    /// Current shaft speed in rpm, for display.
    pub fn speed_rpm(&self) -> f64 {
        self.generator.get_omega().get::<revolution_per_minute>()
    }

    /// Three-phase electrical power delivered into the resistive load.
    ///
    /// At steady state this is `eta` times the turbine's mechanical power; the
    /// difference during a transient is what accelerates or decelerates the
    /// rotor.
    pub fn electrical_power(&self) -> Power {
        self.electrical_power
    }

    /// Driving torque applied on the most recent step.
    #[allow(dead_code)] // diagnostics candidate -- not yet surfaced in the GUI
    pub fn shaft_torque(&self) -> Torque {
        self.shaft_torque
    }

    /// The machine rating the load and inertia were sized against.
    pub fn rated_shaft_power(&self) -> Power {
        self.rated_shaft_power
    }

    /// The resistive load the generator works into.
    #[allow(dead_code)] // diagnostics candidate -- not yet surfaced in the GUI
    pub fn load_resistance(&self) -> ElectricalResistance {
        self.load_resistance
    }
}

impl Default for TurbineGeneratorShaft {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::power::megawatt;
    use uom::si::time::second;

    fn dt() -> Time {
        Time::new::<second>(1.0e-3)
    }

    /// Run the shaft to steady state at a fixed shaft power and return it.
    fn settled(shaft_power: Power, seconds: f64) -> TurbineGeneratorShaft {
        let mut shaft = TurbineGeneratorShaft::new();
        let steps = (seconds / dt().get::<second>()) as usize;
        let mut t = Time::new::<second>(0.0);
        for _ in 0..steps {
            t += dt();
            shaft.step(dt(), shaft_power, t);
        }
        shaft
    }

    /// The three stator phases are 120 degrees apart, so
    /// `sum_j cos^2(theta + b_j) = 3/2` for every `theta`. Both the torque
    /// balance and the power readout multiply by that sum, so if the identity
    /// holds the machine is time-invariant -- which matters, because the
    /// generator evaluates its phase from `omega * t` and this simulator runs
    /// for hours of plant time, at which point the cosine arguments are
    /// hundreds of thousands of radians.
    ///
    /// **Methodology.** Step two identical shafts through identical power
    /// histories, one with the clock starting at 0 s and one at 9999 s, and
    /// compare the resulting speed and electrical power. Pass criterion: 1e-9
    /// relative, i.e. far tighter than anything physical but loose enough to
    /// admit floating-point drift in the phase sum.
    ///
    /// **Result (2026-08-12, measured).** The two shafts reached
    /// 299.68178842996525 and 299.6817884299838 rad/s -- agreeing to
    /// **6.2e-14 relative**, but NOT bit-identical, which is what an earlier
    /// draft of this test wrongly asserted. The residual is round-off in
    /// `sum cos^2`, which evaluates to 3/2 only to about 1e-16 absolute and at
    /// a different phase in each run; 5000 explicit steps accumulate that into
    /// the 14th digit.
    ///
    /// **Interpretation.** The model has no *physical* time dependence, so
    /// `current_time` is a formality in this coupling and long run times cannot
    /// degrade it. But it is not exactly time-invariant in floating point, and
    /// nothing here should be described as bit-reproducible across a clock
    /// offset.
    #[test]
    fn the_generator_is_time_invariant() {
        let p = Power::new::<megawatt>(3.0);
        let mut early = TurbineGeneratorShaft::new();
        let mut late = TurbineGeneratorShaft::new();
        let mut t_early = Time::new::<second>(0.0);
        let mut t_late = Time::new::<second>(9999.0);
        for _ in 0..5_000 {
            t_early += dt();
            t_late += dt();
            early.step(dt(), p, t_early);
            late.step(dt(), p, t_late);
        }
        let (a, b) = (
            early.angular_velocity().get::<radian_per_second>(),
            late.angular_velocity().get::<radian_per_second>(),
        );
        assert!(
            (a - b).abs() / a < 1e-9,
            "clock offset changed the shaft speed: {a} vs {b} rad/s"
        );
        let (pa, pb) = (
            early.electrical_power().get::<watt>(),
            late.electrical_power().get::<watt>(),
        );
        assert!(
            (pa - pb).abs() / pa < 1e-9,
            "clock offset changed the electrical power: {pa} vs {pb} W"
        );
    }

    /// The shaft power the rotor is driven by must be **the same number** as
    /// the enthalpy-drop power the steam cycle computes -- not a second
    /// estimate of it. Two independent turbine powers that disagree would be
    /// worse than one.
    ///
    /// **Methodology.** The coupling is `T = P/omega`, so the identity to check
    /// is `T * omega == P`. Step the shaft at a representative 3 MW and compare
    /// the product of the applied torque and the speed it was evaluated at
    /// against the power handed in. Pass criterion: agreement to 1e-12
    /// relative (i.e. round-off, not a tolerance).
    ///
    /// **Result (2026-08-12, measured):** `T * omega` reproduced the 3.000000 MW
    /// input to a relative error of **1.6e-16** -- one unit in the last place,
    /// since the division and multiplication invert. The mechanical power
    /// entering the torque balance *is* the IAPWS-IF97 enthalpy-drop power;
    /// there is no second figure anywhere in this plant.
    #[test]
    fn shaft_power_is_exactly_the_enthalpy_drop_power() {
        let p = Power::new::<megawatt>(3.0);
        let mut shaft = TurbineGeneratorShaft::new();
        let omega_before = shaft.angular_velocity().get::<radian_per_second>();
        shaft.step(dt(), p, Time::new::<second>(0.0));

        let mechanical = shaft.shaft_torque().get::<newton_meter>() * omega_before;
        let relative = (mechanical - p.get::<watt>()).abs() / p.get::<watt>();
        assert!(
            relative < 1e-12,
            "shaft power {mechanical} W disagrees with the enthalpy-drop power \
             {} W (relative {relative})",
            p.get::<watt>()
        );
    }

    /// At steady state the generator must deliver exactly `eta` times the
    /// turbine's mechanical power -- no more (which would be a free-energy
    /// machine) and no less unexplained.
    ///
    /// **Methodology.** Hold the shaft at a fixed 3.0 MW for 60 s of plant time
    /// (about 15 mechanical time constants) and compare the electrical power
    /// against `eta * P_shaft`. Pass criterion: within 0.1%. At steady state
    /// the rotor stores no further energy, so the whole shaft power must appear
    /// in the load, less generator losses.
    ///
    /// **Result (2026-08-12, measured):** shaft 3.000000 MW in, **2.940000 MW**
    /// electrical out, against the expected `0.98 * 3.0 = 2.940000 MW` --
    /// agreement to **4.4e-8 relative**, the residual being how far from
    /// converged 60 s leaves a 4 s time constant. The first law closes across
    /// the machine.
    ///
    /// **Interpretation.** The 2% shortfall is the stated generator efficiency
    /// and nothing else; no power is created or lost numerically in the
    /// coupling.
    #[test]
    fn electrical_output_is_efficiency_times_shaft_power_at_steady_state() {
        let p = Power::new::<megawatt>(3.0);
        let shaft = settled(p, 60.0);
        let expected = GENERATOR_EFFICIENCY * p.get::<watt>();
        let actual = shaft.electrical_power().get::<watt>();
        let relative = (actual - expected).abs() / expected;
        assert!(
            relative < 1e-3,
            "electrical output {actual} W is not eta * shaft power {expected} W \
             (relative {relative})"
        );
    }

    /// The equilibrium speed must follow the analytical
    /// `omega_eq = omega_sync sqrt(P / P_rated)`, which is what makes the speed
    /// a *derived* quantity rather than a chosen one -- and what shows the coil
    /// parameters and the generator efficiency all cancel out of it.
    ///
    /// **Methodology.** Settle the shaft at 25%, 50%, 75% and 100% of the
    /// machine rating (120 s of plant time each, 15 mechanical time constants)
    /// and compare against the closed form. Pass criterion: within 0.5% at
    /// each load.
    ///
    /// **Results (2026-08-12, measured), rating 3.4333 MW, synchronous
    /// 3000.0000 rpm:**
    ///
    /// | Shaft power | Measured speed | Analytical | Error |
    /// |---|---|---|---|
    /// | 0.8583 MW (25%) | 1500.0000 rpm | 1500.0000 rpm | 0.000% |
    /// | 1.7166 MW (50%) | 2121.3203 rpm | 2121.3203 rpm | 0.000% |
    /// | 2.5750 MW (75%) | 2598.0762 rpm | 2598.0762 rpm | 0.000% |
    /// | 3.4333 MW (100%) | 3000.0000 rpm | 3000.0000 rpm | 0.000% |
    ///
    /// **Interpretation.** At rated load the shaft sits at **exactly**
    /// synchronous speed, and the generator efficiency does not shift it: `eta`
    /// enters both the load resistance and the damping coefficient and cancels.
    /// (This test originally asserted `omega_sync sqrt(eta P/P_rated)` and
    /// failed by 1.015% at every load -- the discrepancy that exposed the
    /// algebra error now recorded in the module docs.)
    ///
    /// Off rating the speed falls as the **square root of load**: at a quarter
    /// power the shaft is at half speed. That is the islanded, ungoverned
    /// behaviour described in the module docs and is emphatically **not** how a
    /// grid-connected machine behaves -- a synchronised generator would hold
    /// 3000 rpm at all four of these loads.
    #[test]
    fn equilibrium_speed_follows_the_square_root_of_load() {
        let rated = super::super::secondary_loop::design_point_turbine_power();
        let omega_s = synchronous_speed().get::<radian_per_second>();
        for fraction in [0.25, 0.5, 0.75, 1.0] {
            let p = Power::new::<watt>(rated.get::<watt>() * fraction);
            let shaft = settled(p, 120.0);
            let analytical = omega_s * fraction.sqrt();
            let measured = shaft.angular_velocity().get::<radian_per_second>();
            let relative = (measured - analytical).abs() / analytical;
            assert!(
                relative < 5e-3,
                "at {fraction} of rating the shaft settled at {measured} rad/s, \
                 against the analytical {analytical} rad/s (relative {relative})"
            );
        }
    }

    /// The torque floor is a numerical guard against `P/omega` at standstill,
    /// and it is the only place shaft power and enthalpy-drop power can
    /// disagree. It must therefore never bind anywhere the plant actually
    /// operates.
    ///
    /// **Methodology.** Sweep shaft power from 1 kW (0.03% of rating) to twice
    /// the machine rating and, from the synchronous-speed start, step 30 s at
    /// each, recording the lowest speed reached anywhere. Pass criterion: the
    /// minimum stays above [`OMEGA_FLOOR_RAD_PER_S`] by at least a factor of
    /// ten.
    ///
    /// **Result (2026-08-12, measured):** the lowest speed seen over the whole
    /// sweep was **9.129 rad/s** (87.2 rpm), at the 1 kW end -- **91x** the
    /// 0.1 rad/s floor. The floor never bound, so `T * omega == P` held
    /// everywhere in the sweep.
    ///
    /// **Interpretation.** The guard exists for the pathological case only; in
    /// the operating range the constant-power coupling is exact. Two honest
    /// caveats. First, this measurement is *why* the floor is 0.1 rad/s and not
    /// the 1.0 rad/s originally written: at 1 kW the equilibrium speed is
    /// `314.16 sqrt(1e3/3.4333e6) = 5.4 rad/s`, only five times the old floor,
    /// so the guard was closer to binding than intended. Second, the sweep
    /// starts at synchronous speed, which is how this plant runs -- a machine
    /// started from rest *would* meet the floor on its very first step.
    #[test]
    fn the_torque_floor_never_binds_in_normal_operation() {
        let rated = super::super::secondary_loop::design_point_turbine_power().get::<watt>();
        let mut lowest = f64::INFINITY;
        for i in 0..=20 {
            let p = Power::new::<watt>(1.0e3 + (2.0 * rated - 1.0e3) * i as f64 / 20.0);
            let mut shaft = TurbineGeneratorShaft::new();
            let mut t = Time::new::<second>(0.0);
            for _ in 0..30_000 {
                t += dt();
                shaft.step(dt(), p, t);
                lowest = lowest.min(shaft.angular_velocity().get::<radian_per_second>());
            }
        }
        assert!(
            lowest > 10.0 * OMEGA_FLOOR_RAD_PER_S,
            "shaft reached {lowest} rad/s, within reach of the {OMEGA_FLOOR_RAD_PER_S} rad/s \
             torque floor -- the constant-power coupling is no longer exact there"
        );
    }

    /// Losing the steam supply must coast the shaft down rather than stop it
    /// dead or leave it turning forever, and the coast-down must follow the
    /// electrical drag alone.
    ///
    /// **Methodology.** Settle at rated power, then drop the shaft power to
    /// zero and integrate 30 s. With no driving torque the balance reduces to
    /// `I domega/dt = -k omega`, giving an exponential decay with
    /// `tau = I/k`. Compare the measured speed after 30 s against
    /// `omega_0 exp(-30/tau)`. Pass criterion: within 1%.
    ///
    /// **Result (2026-08-12, measured):** `tau = I/k = 278.291/34.7864 =
    /// **8.0000 s**. From 3000.0000 rpm the shaft fell to **70.5698 rpm** after
    /// 30 s, against the analytical 70.5532 rpm -- 0.023% high, the expected
    /// first-order bias of the semi-implicit integration at `dt/tau = 1.3e-4`.
    ///
    /// **Interpretation.** The rotor's stored kinetic energy is dissipated in
    /// the electrical load with the right time constant, so a loss of steam
    /// produces a physical run-down rather than an instantaneous stop. The
    /// 8.0000 s is not a coincidence: with the load sized at the rated point,
    /// `k = P_rated/omega_sync^2` and `I = 2 H P_rated/omega_sync^2`, so
    /// `tau = I/k = 2H` exactly -- the coast-down time is **twice the inertia
    /// constant** and depends on nothing else. Which also means it is set by
    /// the invented `H = 4 s` and is not a measurement of anything.
    #[test]
    fn losing_steam_coasts_the_shaft_down_on_the_electrical_drag() {
        let rated = super::super::secondary_loop::design_point_turbine_power();
        let mut shaft = settled(rated, 120.0);
        let omega_0 = shaft.angular_velocity().get::<radian_per_second>();

        let k = electrical_damping(shaft.load_resistance());
        let inertia = rotor_inertia(rated).get::<kilogram_square_meter>();
        let tau = inertia / k;

        let mut t = Time::new::<second>(120.0);
        for _ in 0..30_000 {
            t += dt();
            shaft.step(dt(), Power::new::<watt>(0.0), t);
        }

        let measured = shaft.angular_velocity().get::<radian_per_second>();
        let analytical = omega_0 * (-30.0 / tau).exp();
        let relative = (measured - analytical).abs() / analytical;
        assert!(
            relative < 1e-2,
            "after 30 s of coast-down the shaft was at {measured} rad/s against \
             the analytical {analytical} rad/s (tau = {tau} s, relative {relative})"
        );
    }

    /// **The plant-level check.** Coupled to this simulator's own steam cycle
    /// at its nominal duty, the shaft must settle at a speed defensible for a
    /// steam turbine-generator, and the electrical output must stay consistent
    /// with the enthalpy-drop power that drove it.
    ///
    /// **Methodology.** Drive the real [`super::secondary_loop::SteamSecondaryLoop`]
    /// at the published 10 MWth against a 973.15 K (700 degC, published core
    /// outlet) hot side for 200 s of plant time at `dt = 50 ms` -- 20 feedwater
    /// time constants and 25 shaft time constants -- feeding its turbine power
    /// straight into the shaft each step. Three pass criteria: the speed lands
    /// inside 2500-3100 rpm (a band around synchronous wide enough that a
    /// wrong answer fails, but not so tight that it is asserting the sizing
    /// back to itself); the electrical output equals `eta` times the turbine
    /// power to 0.5%; and the speed matches the closed-form
    /// `omega_sync sqrt(P/P_rated)` to 0.5%.
    ///
    /// **Results (2026-08-12, measured).**
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | Turbine shaft power (enthalpy drop) | 3.14986 MW |
    /// | Machine rating (design point) | 3.43328 MW |
    /// | Load fraction | 91.745% |
    /// | **Shaft speed** | **2873.5071 rpm** (300.897 rad/s) |
    /// | Closed form `omega_s sqrt(P/P_rated)` | 2873.5071 rpm |
    /// | Generator electrical output | 3.08686 MW |
    /// | `eta` x shaft power | 3.08686 MW |
    /// | Settled feed flow | 3.1856 kg/s |
    ///
    /// **Interpretation.** The plant settles at **2873.5 rpm, 4.22% below the
    /// 3000 rpm synchronous speed the machine was sized at**, because the
    /// steam cycle delivers 91.7% of the design-point shaft power: the model's
    /// feed flow settles at 3.1856 kg/s against the published 12.5 t/hr =
    /// 3.4722 kg/s the rating was taken at. That deficit is a *steam cycle*
    /// result, not a generator one.
    ///
    /// A real grid-connected machine would sit at exactly 3000 rpm here and
    /// simply deliver less power; this one slows down instead, which is the
    /// islanded, ungoverned behaviour the module docs describe. **The 2873.5
    /// rpm is therefore defensible as "a steam turbine-generator running a
    /// little under synchronous speed at part load", and is not defensible as a
    /// prediction of HTR-10's turbine speed** -- nothing about the machine is
    /// published, and the near-synchronous scale is put in by the sizing.
    ///
    /// Mechanical and electrical power agree by construction: the 2.0%
    /// difference between 3.14986 MW and 3.08686 MW is exactly the stated
    /// generator efficiency, and the shaft power is the same IAPWS-IF97
    /// enthalpy-drop number the cycle reports. There is no second turbine power
    /// in this plant.
    #[test]
    fn the_plant_settles_the_shaft_a_little_under_synchronous_speed() {
        use super::super::secondary_loop::SteamSecondaryLoop;
        use uom::si::f64::ThermodynamicTemperature;
        use uom::si::thermodynamic_temperature::kelvin;

        let step_dt = Time::new::<second>(0.05);
        let hot_side = ThermodynamicTemperature::new::<kelvin>(973.15);
        let duty = Power::new::<megawatt>(10.0);

        let mut cycle = SteamSecondaryLoop::new();
        let mut shaft = TurbineGeneratorShaft::new();
        let mut t = Time::new::<second>(0.0);
        for _ in 0..4_000 {
            t += step_dt;
            cycle.step(step_dt, duty, hot_side);
            shaft.step(step_dt, cycle.turbine_power(), t);
        }

        let rpm = shaft.speed_rpm();
        assert!(
            (2500.0..=3100.0).contains(&rpm),
            "shaft settled at {rpm} rpm, outside the 2500-3100 rpm band a steam \
             turbine-generator sized for 3000 rpm should land in"
        );

        // Mechanical and electrical power must agree through eta alone.
        let mechanical = cycle.turbine_power().get::<watt>();
        let electrical = shaft.electrical_power().get::<watt>();
        let expected = GENERATOR_EFFICIENCY * mechanical;
        assert!(
            (electrical - expected).abs() / expected < 5e-3,
            "generator output {electrical} W is not eta * the enthalpy-drop \
             power {mechanical} W (expected {expected} W)"
        );

        // And the speed must be the closed form, not something else that
        // happens to land in the band.
        let fraction = mechanical / shaft.rated_shaft_power().get::<watt>();
        let analytical = synchronous_speed().get::<radian_per_second>() * fraction.sqrt();
        let measured = shaft.angular_velocity().get::<radian_per_second>();
        assert!(
            (measured - analytical).abs() / analytical < 5e-3,
            "shaft speed {measured} rad/s departs from the closed-form \
             {analytical} rad/s at {fraction} of rating"
        );
    }

    /// A fresh shaft must start at synchronous speed, so the simulator opens at
    /// its nominal operating point like every other subsystem, and so the
    /// widget's `theta = omega t` is drawn in the near-constant-speed regime
    /// where it is faithful.
    #[test]
    fn a_fresh_shaft_starts_at_synchronous_speed() {
        let shaft = TurbineGeneratorShaft::new();
        assert_eq!(
            shaft.angular_velocity().get::<radian_per_second>(),
            synchronous_speed().get::<radian_per_second>()
        );
        assert!((shaft.speed_rpm() - 3000.0).abs() < 1e-9);
    }
}
