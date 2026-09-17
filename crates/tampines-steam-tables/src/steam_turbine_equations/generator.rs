use uom::si::angle::degree;
use uom::si::area::square_meter;
use uom::si::energy::joule;
#[allow(non_snake_case)]
use uom::si::f64::*;
use uom::si::magnetic_flux_density::tesla;
use uom::si::moment_of_inertia::kilogram_square_meter;
use uom::si::ratio::ratio;
use uom::si::torque::newton_meter;
use uom::ConstZero;
/// Lumped-parameter model of a three-phase synchronous generator driven by a
/// steam turbine rotor. The three stator windings are phase-shifted by
/// 0 degrees, 120 degrees, and 240 degrees respectively (this corrects an
/// earlier version of this doc comment, which said 60 degrees).
///
/// Rotor angular velocity advances under an explicit torque balance
/// (`calculate_new_angular_velocity` / `advance_timestep`); per-phase EMF,
/// current, and total electrical power are then read out from that angular
/// velocity and a supplied load resistance.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreePhaseElectricGeneratorTurbine {
    /// I
    I: MomentOfInertia,

    /// omega
    omega: AngularVelocity,

    /// no of turns for the coil
    N: usize,

    /// magnetic flux density for generator
    B: MagneticFluxDensity,

    /// area of coil
    A: Area,

    /// turbine efficiency
    eta: Ratio,
}

/// these are defaults for a three phase generator
impl ThreePhaseElectricGeneratorTurbine {
    /// Builds a generator preset sized for an illustrative 250 MW steam
    /// turbine-generator set with a stiff shaft (no separate shaft
    /// spring/damping terms). All parameter values (moment of inertia, coil
    /// turns, coil area, flux density, efficiency) are placeholder estimates,
    /// not measurements from a real plant. Initial angular velocity is zero.
    pub fn new_250_megawatt_generator() -> Self {
        // For 250 MW Steam Turbine-Generator Set with STIFF shaft
        // (AI Generated)
        let B = MagneticFluxDensity::new::<tesla>(1.0);
        let A = Area::new::<square_meter>(0.65);
        let N: usize = 70;
        let I = MomentOfInertia::new::<kilogram_square_meter>(530_000.0); // Combined!
        let eta = Ratio::new::<ratio>(0.98);
        let omega = AngularVelocity::ZERO;

        // No need for K_shaft or D_shaft
        // Optional: system damping coefficient D_system ≈ 30,000 N·m·s/rad

        return Self {
            I,
            omega,
            N,
            B,
            A,
            eta,
        };
    }
    /// Constructs a generator from explicit parameters: magnetic flux
    /// density `B` (tesla), coil area `A` (m^2), number of coil turns `N`,
    /// combined rotor moment of inertia `I` (kg*m^2), turbine efficiency
    /// `eta` (dimensionless ratio), and initial angular velocity `omega`
    /// (rad/s).
    pub fn new(
        B: MagneticFluxDensity,
        A: Area,
        N: usize,
        I: MomentOfInertia,
        eta: Ratio,
        omega: AngularVelocity,
    ) -> Self {
        return Self {
            I,
            omega,
            N,
            B,
            A,
            eta,
        };
    }
}

impl ThreePhaseElectricGeneratorTurbine {
    /// this immutably calculates new angular velocity
    /// in an explicit manner, given a source term
    ///     \begin{equation*}
    /// 	\omega^{t+ \Delta t} \left(
    /// 		\frac{I}{\Delta t}
    /// 		+ \frac{( N^2 B^2 A^2 )}{\eta R_{load} } \sum_j \cos^2 (\omega^t t + b_j)  
    /// \right)
    /// 	=
    /// 	I \frac{\omega^{t } }{\Delta t}
    /// 	+ \text{source}
    /// \end{equation*}
    ///
    /// omega^{t + Delta t} (I/delta_t + (NBA)^2/(eta R_load)
    /// sum (cos^2 (omega^t t + b_j))
    ///
    pub fn calculate_new_angular_velocity(
        &self,
        source: Torque,
        load_resistance: ElectricalResistance,
        current_time: Time,
        delta_t: Time,
    ) -> AngularVelocity {
        let t = current_time;

        let theta: Angle = (self.omega * t).into();

        let phase_shift_1 = Angle::ZERO;
        let phase_shift_2 = Angle::new::<degree>(120.0);
        let phase_shift_3 = Angle::new::<degree>(240.0);

        let cos_angle_1: Ratio = (theta + phase_shift_1).cos();
        let cos_angle_2: Ratio = (theta + phase_shift_2).cos();
        let cos_angle_3: Ratio = (theta + phase_shift_3).cos();

        let cosine_summation: Ratio =
            cos_angle_1 * cos_angle_1 + cos_angle_2 * cos_angle_2 + cos_angle_3 * cos_angle_3;

        let nba: MagneticFlux = self.N as f64 * self.B * self.A;

        let coeff = self.I / delta_t + (nba * nba) / self.eta / load_resistance * cosine_summation;

        let mut rhs: Torque = source;

        let momentum_torque: Energy = self.I * self.omega / delta_t;
        // note that both torque and energy have the units of newton
        // meter
        //
        // Work done (Joule) = F * ds (also newton meter)
        // uom distinguishes both of them though

        rhs += Torque::new::<newton_meter>(momentum_torque.get::<joule>());

        return (rhs / coeff).into();
    }
    /// this mutably calculates new angular velocity
    /// in an explicit manner, given a source term
    ///     \begin{equation*}
    /// 	\omega^{t+ \Delta t} \left(
    /// 		\frac{I}{\Delta t}
    /// 		+ \frac{( N^2 B^2 A^2 )}{\eta R_{load} } \sum_j \cos^2 (\omega^t t + b_j)  
    /// \right)
    /// 	=
    /// 	I \frac{\omega^{t } }{\Delta t}
    /// 	+ \text{source}
    /// \end{equation*}
    ///
    /// omega^{t + Delta t} (I/delta_t + (NBA)^2/(eta R_load)
    /// sum (cos^2 (omega^t t + b_j))
    pub fn advance_timestep(
        &mut self,
        torque_source: Torque,
        load_resistance: ElectricalResistance,
        current_time: Time,
        delta_t: Time,
    ) {
        let new_angular_velocity = self.calculate_new_angular_velocity(
            torque_source,
            load_resistance,
            current_time,
            delta_t,
        );

        self.omega = new_angular_velocity;
    }

    /// Sets the rotor magnetic flux density (tesla).
    pub fn set_magnetic_field(&mut self, B: MagneticFluxDensity) {
        self.B = B
    }

    /// Computes the instantaneous back-EMF (V) of phase 1 (0 degree phase
    /// shift) at time `t`, from the current angular velocity and coil
    /// parameters.
    pub fn get_emf_1(&self, t: Time) -> ElectricPotential {
        let nba: MagneticFlux = self.N as f64 * self.B * self.A;
        let omega = self.omega;

        let phase_shift_1 = Angle::ZERO;

        let theta: Angle = (self.omega * t).into();
        let cos_angle_1: Ratio = (theta + phase_shift_1).cos();

        let emf = -nba * omega * cos_angle_1;

        return emf;
    }

    /// Computes the instantaneous back-EMF (V) of phase 2 (120 degree phase
    /// shift) at time `t`, from the current angular velocity and coil
    /// parameters.
    pub fn get_emf_2(&self, t: Time) -> ElectricPotential {
        let nba: MagneticFlux = self.N as f64 * self.B * self.A;
        let omega = self.omega;

        let phase_shift_2 = Angle::new::<degree>(120.0);

        let theta: Angle = (self.omega * t).into();
        let cos_angle_2: Ratio = (theta + phase_shift_2).cos();

        let emf = -nba * omega * cos_angle_2;

        return emf;
    }
    /// Computes the instantaneous back-EMF (V) of phase 3 (240 degree phase
    /// shift) at time `t`, from the current angular velocity and coil
    /// parameters.
    pub fn get_emf_3(&self, t: Time) -> ElectricPotential {
        let nba: MagneticFlux = self.N as f64 * self.B * self.A;
        let omega = self.omega;

        let phase_shift_3 = Angle::new::<degree>(240.0);

        let theta: Angle = (self.omega * t).into();
        let cos_angle_3: Ratio = (theta + phase_shift_3).cos();

        let emf = -nba * omega * cos_angle_3;

        return emf;
    }

    /// Computes total instantaneous three-phase electrical power (W)
    /// delivered to a resistive load `load_resistance` (ohm) at time `t`, as
    /// the sum of each phase's EMF squared divided by the load resistance.
    pub fn get_power(&self, load_resistance: ElectricalResistance, t: Time) -> Power {
        let emf_1 = self.get_emf_1(t);
        let emf_2 = self.get_emf_2(t);
        let emf_3 = self.get_emf_3(t);

        let p: Power = load_resistance.recip() * (emf_1 * emf_1 + emf_2 * emf_2 + emf_3 * emf_3);

        return p;
    }

    /// Computes phase-1 instantaneous current (A) delivered into
    /// `load_resistance` (ohm) at time `t`, from Ohm's law applied to the
    /// phase-1 EMF.
    pub fn get_current_1(&self, load_resistance: ElectricalResistance, t: Time) -> ElectricCurrent {
        self.get_emf_1(t) / load_resistance
    }

    /// Computes phase-2 instantaneous current (A) delivered into
    /// `load_resistance` (ohm) at time `t`, from Ohm's law applied to the
    /// phase-2 EMF.
    pub fn get_current_2(&self, load_resistance: ElectricalResistance, t: Time) -> ElectricCurrent {
        self.get_emf_2(t) / load_resistance
    }
    /// Computes phase-3 instantaneous current (A) delivered into
    /// `load_resistance` (ohm) at time `t`, from Ohm's law applied to the
    /// phase-3 EMF.
    pub fn get_current_3(&self, load_resistance: ElectricalResistance, t: Time) -> ElectricCurrent {
        self.get_emf_3(t) / load_resistance
    }

    /// Sets the rotor angular velocity (rad/s).
    pub fn set_omega(&mut self, omega: AngularVelocity) {
        self.omega = omega
    }

    /// Returns the current rotor angular velocity (rad/s).
    pub fn get_omega(&self) -> AngularVelocity {
        self.omega
    }
}
