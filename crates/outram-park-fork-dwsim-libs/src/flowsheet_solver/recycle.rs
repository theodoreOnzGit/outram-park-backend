//! Recycle blocks — tear-stream convergence for flowsheets with loops.
//!
//! # What a recycle block is
//!
//! A process flowsheet with a loop cannot be solved in one pass: the mixer at
//! the head of the loop needs the stream that the loop's tail has not produced
//! yet. DWSIM's answer is a **recycle block** the user drops on the loop. It
//! does two things:
//!
//! 1. It **breaks the loop for ordering purposes** — the calculation-order walk
//!    refuses to step through it (see [`crate::flowsheet_solver::ordering`]), so
//!    its outlet stream becomes the **tear stream**: a stream the solver is
//!    allowed to *guess*.
//! 2. It **measures and corrects the guess**. Each outer iteration it compares
//!    what came back around the loop (its inlet) against what it guessed (its
//!    outlet), records the errors in temperature, pressure and per-compound mass
//!    flow, decides whether they are inside tolerance, and writes the next
//!    guess.
//!
//! The outer loop in [`crate::flowsheet_solver::solver`] keeps re-solving the
//! whole flowsheet until every recycle block reports
//! [`RecycleBlock::converged`].
//!
//! # The acceleration methods
//!
//! [`AccelerationMethod`] is DWSIM's `AccelMethod` (Enums.vb:238-243), ported in
//! full as an enum. What each one actually does upstream, at this commit:
//!
//! | Variant | Material recycle ([`RecycleBlock`]) | Energy recycle ([`EnergyRecycleBlock`]) |
//! |---|---|---|
//! | [`AccelerationMethod::None`] | successive substitution — copy the inlet onto the outlet (Recycle.vb:391-407) | successive substitution (EnergyRecycle.vb:231-233) |
//! | [`AccelerationMethod::Wegstein`] | **identical to `None`** — see "A word of warning" below | the Wegstein secant step (EnergyRecycle.vb:235-254) |
//! | [`AccelerationMethod::DominantEigenvalue`] | **identical to `None`** | **falls through with no arm** — see the quirk note on [`EnergyRecycleBlock::calculate`] |
//! | [`AccelerationMethod::GlobalBroyden`] | suppresses the local update; the solver runs one **global** quasi-Newton step across every Broyden-marked recycle at once ([`broydn`], FlowsheetSolver.vb:1537-1567) | falls through as above |
//!
//! ## A word of warning about the material recycle
//!
//! At upstream commit `1abf72d`, `Recycle.Calculate` (Recycle.vb:263-463)
//! **branches on `AccelerationMethod` exactly once**, at `:391`, and only to ask
//! whether it is `GlobalBroyden`. There is no Wegstein arm and no dominant-
//! eigenvalue arm in the material recycle at all: `None`, `Wegstein` and
//! `DominantEigenvalue` all take the same successive-substitution path. That is
//! upstream's state, not an omission in this port, and this port reproduces it —
//! see [`RecycleBlock::calculate`]. The only local acceleration a material
//! recycle offers at this commit is [`RecycleBlock::smoothing_factor`] on the
//! non-legacy path ([`RecycleBlock::legacy_mode`] = `false`, Recycle.vb:409-437),
//! which is a damped/relaxed successive substitution:
//! `x_new = sf * x_inlet + (1 - sf) * x_previous`.
//!
//! [`WegsteinParameters`] and the Wegstein step itself are ported and live,
//! because the **energy** recycle does use them.
//!
//! # Units
//!
//! Stored values keep DWSIM's internal units verbatim so the ported arithmetic
//! stays comparable to its source: **temperature K, pressure Pa, mass flow
//! kg/s, mass enthalpy kJ/kg, energy flow kW**. The `uom`-typed accessors on
//! [`RecycleConvergenceParameters`] and [`EnergyConvergenceParameters`] convert
//! for public consumption.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2025 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Primary sources:
//!
//! - `DWSIM.UnitOperations/LogicalBlocks/Recycle.vb` — `:31-209` (state and
//!   defaults), `:211-243` (`SetOutletStreamProperties`), `:245-261`
//!   (`RunDynamicModel`), `:263-463` (`Calculate`), `:465-469` (`DeCalculate`),
//!   `:693-805` (`ConvergenceParameters`, `ConvergenceHistory`,
//!   `WegsteinParameters` and their defaults).
//! - `DWSIM.UnitOperations/LogicalBlocks/EnergyRecycle.vb` — `:31-194` (state and
//!   defaults), `:196-198` (`RunDynamicModel`, empty upstream), `:200-289`
//!   (`Calculate`, including the Wegstein step), `:489-497`
//!   (`ConvergenceParametersE`, energy tolerance default 0.1 kW).
//! - `DWSIM.Math/Broyden.vb:5-82` (`broydn`).
//! - `DWSIM.Interfaces/Enums.vb:238-243` (`AccelMethod`).
//!
//! # Excluded DWSIM behavior
//!
//! - **XML/JSON serialization** — `SaveData`/`LoadData`/`CloneXML`/`CloneJSON`
//!   (Recycle.vb:78-121, EnergyRecycle.vb:51-106).
//! - **Property-grid reflection** — `GetPropertyValue`/`SetPropertyValue`/
//!   `GetProperties`/`GetPropertyUnit` and the `PROP_RY_*` / `PROP_ER_*`
//!   identifier scheme with its display-unit conversion
//!   (Recycle.vb:493-627, EnergyRecycle.vb:319-377). The fields are public and
//!   `uom`-typed instead.
//! - **Editing forms and icons** — `DisplayEditForm`, `UpdateEditForm`,
//!   `CloseEditForm`, `GetIconBitmap*`, `GetDisplayName`/`GetDisplayDescription`
//!   (Recycle.vb:629-688, EnergyRecycle.vb and the `EditingForm_*` classes).
//! - **The modal "maximum iterations reached, continue?" `MessageBox`**
//!   (EnergyRecycle.vb:269-278). Replaced by
//!   [`EnergyRecycleMaxIterationsPolicy`], which encodes the two answers.
//! - **`Inspector` narrative paragraphs** (Recycle.vb:265-295) — a documentation
//!   facility with no computational effect. Its content is folded into these
//!   doc comments instead.
//! - **`PropertyPackage.CurrentMaterialStream` bookkeeping** (Recycle.vb:225,
//!   :377). A .NET object-graph detail of DWSIM's property-package plumbing,
//!   which this port does not have.
//! - **`MAX()`** (Recycle.vb:471-491, EnergyRecycle.vb:297-317) — a dead helper,
//!   called from nowhere in either file.
//! - **`Snew` / entropy** (Recycle.vb:343). Computed upstream and then never
//!   used; the history field exists here for completeness but is likewise never
//!   read by the algorithm.
//!
//! # Honest scope
//!
//! AI-assisted draft with **no human V&V**. The tests below are *verification*
//! against the transcribed upstream logic and analytically-known fixed points —
//! they check "did we port it correctly?", not "does it represent physical
//! reality?". No DWSIM benchmark flowsheet has been run against this port.

use uom::si::f64::{MassRate, Power, Pressure, TemperatureInterval};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::kilowatt;
use uom::si::pressure::pascal;
use uom::si::temperature_interval::kelvin;

use crate::flowsheet::{Flowsheet, ObjectId, ObjectType, PhaseIndex};
use crate::flowsheet_solver::errors::SolverError;

// ---------------------------------------------------------------------------
// Acceleration methods and their parameters
// ---------------------------------------------------------------------------

/// How a recycle block accelerates its fixed-point iteration — DWSIM's
/// `AccelMethod` (Enums.vb:238-243).
///
/// Enum dispatch, not a trait object, per the workspace Rust design rules. See
/// the module documentation for what each variant actually does at this upstream
/// commit; the short version is that only the **energy** recycle implements
/// Wegstein, and only the **solver** implements Broyden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AccelerationMethod {
    /// Plain successive substitution: the next guess *is* the value that came
    /// back around the loop. Slow but, as upstream's own note puts it,
    /// "convergence is guaranteed" (Recycle.vb:279-283).
    ///
    /// This is [`RecycleBlock`]'s default (Recycle.vb:41).
    #[default]
    None,
    /// The Wegstein secant step. Upstream's guidance: use it "when there isn't a
    /// significant interaction between convergent variables"
    /// (Recycle.vb:279-283). This is [`EnergyRecycleBlock`]'s default
    /// (EnergyRecycle.vb:39).
    Wegstein,
    /// Dominant-eigenvalue acceleration. Upstream's guidance: use it when
    /// convergent variables *do* interact. **Neither recycle block implements an
    /// arm for it at this commit** — see the module table.
    DominantEigenvalue,
    /// Global Broyden: the recycle stops correcting itself locally and instead
    /// contributes its four variables to one flowsheet-wide quasi-Newton step
    /// driven by the solver ([`broydn`], FlowsheetSolver.vb:1537-1567).
    GlobalBroyden,
}

/// Tuning for the Wegstein step — DWSIM's `Helpers.Recycle.WegsteinParameters`
/// (Recycle.vb:796-803).
///
/// All dimensionless. The defaults below are upstream's literal field
/// initialisers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WegsteinParameters {
    /// How many iterations must pass between two accelerated steps
    /// (`AccelFreq`, default `4`). The step is taken only once
    /// `accel_freq <= internal_counter`.
    pub accel_freq: i64,
    /// Upper bound on the Wegstein factor `q` (`Qmax`, default `0`). A step is
    /// rejected unless `qmin < q < qmax`; with the default bounds
    /// `-20 < q < 0`, only *damping* steps are accepted.
    pub qmax: f64,
    /// Lower bound on the Wegstein factor `q` (`Qmin`, default `-20`).
    pub qmin: f64,
    /// How many iterations to run plainly before acceleration is allowed at all
    /// (`AccelDelay`, default `2`). Upstream stores it as a `Double` and
    /// compares `accel_delay <= iteration_count + 3`.
    pub accel_delay: f64,
}

impl Default for WegsteinParameters {
    fn default() -> Self {
        WegsteinParameters {
            accel_freq: 4,
            qmax: 0.0,
            qmin: -20.0,
            accel_delay: 2.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Material recycle
// ---------------------------------------------------------------------------

/// Convergence tolerances for a material recycle — DWSIM's
/// `Helpers.Recycle.ConvergenceParameters` (Recycle.vb:702-731).
///
/// A recycle is converged when the temperature, pressure and mass-flow errors
/// are all at or below their tolerance (Recycle.vb:444-457). The other four
/// fields are carried because upstream carries them; **the convergence test does
/// not read them.**
///
/// Field units are DWSIM's internal ones; the accessors are `uom`-typed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecycleConvergenceParameters {
    /// Temperature tolerance \[K\] (`Temperatura`, default `0.1`).
    pub temperature: f64,
    /// Pressure tolerance \[Pa\] (`Pressao`, default `0.1`).
    pub pressure: f64,
    /// Mass-flow tolerance \[kg/s\] (`VazaoMassica`, default `0.01`). Compared
    /// against the **sum of absolute per-compound mass-flow differences**, not
    /// against the difference of the totals (Recycle.vb:316-322).
    pub mass_flow: f64,
    /// Vapour-fraction tolerance \[dimensionless\] (`FracaoVapor`, default
    /// `0.01`). Not read by the convergence test.
    pub vapor_fraction: f64,
    /// Mass-enthalpy tolerance \[kJ/kg\] (`Entalpia`, default `1`). Not read by
    /// the convergence test.
    pub enthalpy: f64,
    /// Mass-entropy tolerance \[kJ/(kg K)\] (`Entropia`, default `0.01`). Not
    /// read by the convergence test.
    pub entropy: f64,
    /// Composition tolerance \[mole fraction\] (`Composicao`, default `0.001`).
    /// Not read by the convergence test.
    pub composition: f64,
}

impl Default for RecycleConvergenceParameters {
    fn default() -> Self {
        RecycleConvergenceParameters {
            temperature: 0.1,
            pressure: 0.1,
            mass_flow: 0.01,
            vapor_fraction: 0.01,
            enthalpy: 1.0,
            entropy: 0.01,
            composition: 0.001,
        }
    }
}

impl RecycleConvergenceParameters {
    /// The temperature tolerance as a `uom` [`TemperatureInterval`] \[K\].
    ///
    /// An *interval*, not an absolute temperature: it is a difference between
    /// two temperatures.
    #[must_use]
    pub fn temperature_tolerance(&self) -> TemperatureInterval {
        TemperatureInterval::new::<kelvin>(self.temperature)
    }

    /// The pressure tolerance as a `uom` [`Pressure`] \[Pa\].
    #[must_use]
    pub fn pressure_tolerance(&self) -> Pressure {
        Pressure::new::<pascal>(self.pressure)
    }

    /// The mass-flow tolerance as a `uom` [`MassRate`] \[kg/s\].
    #[must_use]
    pub fn mass_flow_tolerance(&self) -> MassRate {
        MassRate::new::<kilogram_per_second>(self.mass_flow)
    }
}

/// One iteration of history for a material recycle — DWSIM's
/// `Helpers.Recycle.ConvergenceHistory` (Recycle.vb:733-794).
///
/// For each of temperature, pressure, mass flow, enthalpy and entropy it keeps
/// four numbers: the current value, the previous value (`_prev`), the current
/// error (`_err`), and the previous error (`_err_prev`). A secant/Wegstein step
/// needs exactly that quartet.
///
/// Units are DWSIM's internal ones: K, Pa, kg/s, kJ/kg, kJ/(kg K).
/// Errors are differences in the same units. All fields default to `0`.
///
/// **Only the temperature, pressure and mass-flow quartets are written by
/// `Calculate` upstream** (Recycle.vb:326-340). Upstream keeps the inlet
/// enthalpy and entropy in the local variables `Hnew` / `Snew`
/// (Recycle.vb:342-343) and never reads them again; this port records them in
/// [`RecycleConvergenceHistory::enthalpy`] and
/// [`RecycleConvergenceHistory::entropy`] instead, which changes nothing
/// because nothing reads them either. The `*_err` fields for enthalpy and
/// entropy stay at `0` exactly as upstream leaves them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RecycleConvergenceHistory {
    /// Current inlet temperature \[K\] (`Temperatura`).
    pub temperature: f64,
    /// Previous inlet temperature \[K\] (`Temperatura0`).
    pub temperature_prev: f64,
    /// Current temperature error, inlet minus outlet \[K\] (`TemperaturaE`).
    pub temperature_err: f64,
    /// Previous temperature error \[K\] (`TemperaturaE0`).
    pub temperature_err_prev: f64,
    /// Current inlet pressure \[Pa\] (`Pressao`).
    pub pressure: f64,
    /// Previous inlet pressure \[Pa\] (`Pressao0`).
    pub pressure_prev: f64,
    /// Current pressure error, inlet minus outlet \[Pa\] (`PressaoE`).
    pub pressure_err: f64,
    /// Previous pressure error \[Pa\] (`PressaoE0`).
    pub pressure_err_prev: f64,
    /// Current inlet total mass flow \[kg/s\] (`VazaoMassica`).
    pub mass_flow: f64,
    /// Previous inlet total mass flow \[kg/s\] (`VazaoMassica0`).
    pub mass_flow_prev: f64,
    /// Current mass-flow error \[kg/s\] (`VazaoMassicaE`) — the **sum of
    /// absolute per-compound differences** (Recycle.vb:316-322), always
    /// non-negative.
    pub mass_flow_err: f64,
    /// Previous mass-flow error \[kg/s\] (`VazaoMassicaE0`).
    pub mass_flow_err_prev: f64,
    /// Current inlet mass enthalpy \[kJ/kg\] (`Entalpia`).
    pub enthalpy: f64,
    /// Previous inlet mass enthalpy \[kJ/kg\] (`Entalpia0`).
    pub enthalpy_prev: f64,
    /// Current enthalpy error \[kJ/kg\] (`EntalpiaE`).
    pub enthalpy_err: f64,
    /// Previous enthalpy error \[kJ/kg\] (`EntalpiaE0`).
    pub enthalpy_err_prev: f64,
    /// Current inlet mass entropy \[kJ/(kg K)\] (`Entropia`). Never read by the
    /// algorithm — see the module's "Excluded DWSIM behavior".
    pub entropy: f64,
    /// Previous inlet mass entropy \[kJ/(kg K)\] (`Entropia0`).
    pub entropy_prev: f64,
    /// Current entropy error \[kJ/(kg K)\] (`EntropiaE`).
    pub entropy_err: f64,
    /// Previous entropy error \[kJ/(kg K)\] (`EntropiaE0`).
    pub entropy_err_prev: f64,
}

/// The four scalars a recycle exposes to the global Broyden step — DWSIM's
/// `IRecycle.Values` / `IRecycle.Errors` dictionaries (Recycle.vb:59-76).
///
/// Upstream keys them by the strings `"Temperature"`, `"Pressure"`,
/// `"MassFlow"`, `"Enthalpy"` in insertion order; this port fixes the order in
/// the struct, which is what the Broyden packing relies on
/// (FlowsheetSolver.vb:1545-1549 iterates the dictionary in order).
///
/// Units: K, Pa, kg/s, kJ/kg.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RecycleVariables {
    /// `"Temperature"` \[K\].
    pub temperature: f64,
    /// `"Pressure"` \[Pa\].
    pub pressure: f64,
    /// `"MassFlow"` \[kg/s\].
    pub mass_flow: f64,
    /// `"Enthalpy"` \[kJ/kg\].
    pub enthalpy: f64,
}

impl RecycleVariables {
    /// How many scalars a recycle contributes to the global Broyden vector.
    ///
    /// Upstream's `rec.Values.Count`, which is `0` before the first
    /// `Calculate` and `4` afterwards (FlowsheetSolver.vb:1330).
    pub const LEN: usize = 4;

    /// The four values in upstream dictionary order, with their upstream keys.
    #[must_use]
    pub fn as_pairs(&self) -> [(&'static str, f64); Self::LEN] {
        [
            ("Temperature", self.temperature),
            ("Pressure", self.pressure),
            ("MassFlow", self.mass_flow),
            ("Enthalpy", self.enthalpy),
        ]
    }

    /// Overwrite the four values from a slice in upstream dictionary order.
    ///
    /// Values beyond the fourth are ignored; a shorter slice leaves the
    /// remaining fields untouched.
    pub fn set_from_slice(&mut self, v: &[f64]) {
        let mut fields = [
            &mut self.temperature,
            &mut self.pressure,
            &mut self.mass_flow,
            &mut self.enthalpy,
        ];
        for (slot, value) in fields.iter_mut().zip(v.iter()) {
            **slot = *value;
        }
    }
}

/// A material recycle block and all of its convergence state.
///
/// # Where this state lives
///
/// Upstream this is a `SpecialOps.Recycle` unit operation stored in the
/// flowsheet. Here the flowsheet data model carries no equipment state, so a
/// `RecycleBlock` is owned by
/// [`crate::flowsheet_solver::solver::FlowsheetSolver`] and keyed by the
/// [`ObjectId`] of the corresponding [`ObjectType::OtRecycle`] object. That is
/// deliberate: the iteration counter and error history must survive *across*
/// outer solver iterations, and the flowsheet is reset between them.
///
/// # How it is driven
///
/// The solver calls [`RecycleBlock::calculate`] once per outer iteration, at the
/// point in the calculation order where the recycle sits, then reads
/// [`RecycleBlock::converged`].
#[derive(Debug, Clone, PartialEq)]
pub struct RecycleBlock {
    /// Tolerances (`ConvergenceParameters`, Recycle.vb:39).
    pub convergence_parameters: RecycleConvergenceParameters,
    /// Iteration history (`ConvergenceHistory`, Recycle.vb:40).
    pub convergence_history: RecycleConvergenceHistory,
    /// Acceleration selector (`AccelerationMethod`, default
    /// [`AccelerationMethod::None`], Recycle.vb:41).
    pub acceleration_method: AccelerationMethod,
    /// Wegstein tuning (`WegsteinParameters`, Recycle.vb:42). Carried but not
    /// read by the material recycle at this upstream commit — see the module
    /// warning.
    pub wegstein_parameters: WegsteinParameters,
    /// Iteration cap (`MaximumIterations`, default `50`, Recycle.vb:44).
    /// Exceeding it raises [`SolverError::RecycleMaxIterations`].
    pub max_iterations: i64,
    /// Iterations since the last convergence (`IterationCount`, Recycle.vb:45).
    pub iteration_count: i64,
    /// Iterations the last successful convergence took (`IterationsTaken`,
    /// Recycle.vb:49).
    pub iterations_taken: i64,
    /// Whether the last [`RecycleBlock::calculate`] found every monitored error
    /// inside tolerance (`Converged`, Recycle.vb:55). **This is the flag the
    /// outer solver loop polls.**
    pub converged: bool,
    /// Copy the inlet onto the outlet even when the inlet state is non-finite
    /// (`CopyOnStreamDataError`, default `false`, Recycle.vb:57).
    pub copy_on_stream_data_error: bool,
    /// Relaxation factor for the non-legacy path (`SmoothingFactor`, default
    /// `1.0`, Recycle.vb:62). `x_new = sf * x_inlet + (1 - sf) * x_previous`;
    /// `1.0` reduces to plain successive substitution. Dimensionless, and
    /// meaningful in `(0, 1]`.
    pub smoothing_factor: f64,
    /// Select the legacy update path (`LegacyMode`, default `true`,
    /// Recycle.vb:64). `true` copies the inlet wholesale onto the outlet;
    /// `false` applies [`RecycleBlock::smoothing_factor`] to temperature,
    /// pressure and per-compound mass flow instead.
    pub legacy_mode: bool,
    /// The outlet-side values, in DWSIM's internal units (`Values`,
    /// Recycle.vb:60). Populated on the first [`RecycleBlock::calculate`].
    pub values: RecycleVariables,
    /// The inlet-versus-outlet errors (`Errors`, Recycle.vb:59). Populated on
    /// the first [`RecycleBlock::calculate`]; note that on that first call
    /// upstream stores the raw inlet *values* here, not errors
    /// (Recycle.vb:345-349).
    pub errors: RecycleVariables,
    /// Whether [`RecycleBlock::values`] and [`RecycleBlock::errors`] have been
    /// populated — upstream's `Values.Count = 0` / `Errors.Count = 0` test
    /// (Recycle.vb:345, :361; FlowsheetSolver.vb:1329-1330).
    pub initialised: bool,
    /// Wegstein internal counter for temperature (`m_InternalCounterT`,
    /// Recycle.vb:46). Never advanced at this upstream commit.
    pub internal_counter_temperature: i64,
    /// Wegstein internal counter for pressure (`m_InternalCounterP`,
    /// Recycle.vb:47). Never advanced at this upstream commit.
    pub internal_counter_pressure: i64,
    /// Wegstein internal counter for mass flow (`m_InternalCounterW`,
    /// Recycle.vb:48). Never advanced at this upstream commit.
    pub internal_counter_mass_flow: i64,
}

impl Default for RecycleBlock {
    fn default() -> Self {
        RecycleBlock::new()
    }
}

impl RecycleBlock {
    /// A recycle block with every upstream default in place
    /// (Recycle.vb:39-64, :186-194).
    ///
    /// Tolerances `0.1 K`, `0.1 Pa`, `0.01 kg/s`; `max_iterations = 50`;
    /// [`AccelerationMethod::None`]; legacy mode on; smoothing factor `1.0`.
    #[must_use]
    pub fn new() -> Self {
        RecycleBlock {
            convergence_parameters: RecycleConvergenceParameters::default(),
            convergence_history: RecycleConvergenceHistory::default(),
            acceleration_method: AccelerationMethod::None,
            wegstein_parameters: WegsteinParameters::default(),
            max_iterations: 50,
            iteration_count: 0,
            iterations_taken: 0,
            converged: false,
            copy_on_stream_data_error: false,
            smoothing_factor: 1.0,
            legacy_mode: true,
            values: RecycleVariables::default(),
            errors: RecycleVariables::default(),
            initialised: false,
            internal_counter_temperature: 0,
            internal_counter_pressure: 0,
            internal_counter_mass_flow: 0,
        }
    }

    /// How many scalars this block contributes to the global Broyden vector —
    /// upstream's `rec.Values.Count` (FlowsheetSolver.vb:1330).
    ///
    /// `0` before the first [`RecycleBlock::calculate`],
    /// [`RecycleVariables::LEN`] afterwards.
    #[must_use]
    pub fn value_count(&self) -> usize {
        if self.initialised {
            RecycleVariables::LEN
        } else {
            0
        }
    }

    /// Reset the iteration counter — DWSIM's `DeCalculate` (Recycle.vb:465-469).
    pub fn decalculate(&mut self) {
        self.iteration_count = 0;
    }

    /// Run one recycle iteration — the port of `Recycle.Calculate`
    /// (Recycle.vb:263-463).
    ///
    /// # What it does, in order
    ///
    /// 1. Reads the inlet stream (what came back around the loop) and the outlet
    ///    stream (the current guess), by connector slot 0 on each side.
    /// 2. Computes the mass-flow error as `sum_i |w_i^in - w_i^out|` over
    ///    compounds \[kg/s\] (Recycle.vb:316-322) — note this is a **sum of
    ///    absolute differences**, so it is zero only when every compound
    ///    matches.
    /// 3. Shifts the history (current becomes previous) and records the new
    ///    temperature \[K\], pressure \[Pa\] and total mass flow \[kg/s\]
    ///    together with their errors (Recycle.vb:324-357).
    /// 4. Fills [`RecycleBlock::errors`] and [`RecycleBlock::values`]
    ///    (Recycle.vb:345-373).
    /// 5. Writes the next guess onto the outlet stream, unless the method is
    ///    [`AccelerationMethod::GlobalBroyden`] (in which case the solver will
    ///    do it) or the inlet state is non-finite and
    ///    [`RecycleBlock::copy_on_stream_data_error`] is `false`
    ///    (Recycle.vb:375-437).
    /// 6. Enforces [`RecycleBlock::max_iterations`] (Recycle.vb:439-442).
    /// 7. Sets [`RecycleBlock::converged`] from the three monitored tolerances
    ///    and advances the iteration counter (Recycle.vb:444-459).
    ///
    /// # Order-of-operations quirk, reproduced verbatim
    ///
    /// Upstream checks the iteration cap **after** writing the new guess and
    /// **before** the convergence test, and it zeroes `IterationCount` in the
    /// same breath as throwing (Recycle.vb:439-442). It also increments the
    /// counter *after* deciding convergence, so a block that converges on its
    /// first call reports `iteration_count = 1` and `iterations_taken = 0`.
    /// Both are faithful.
    ///
    /// # Errors
    ///
    /// - [`SolverError::RecycleNotConnected`] if either slot 0 is free.
    /// - [`SolverError::RecycleStreamNotCalculated`] if the stream it must copy
    ///   from has been neither calculated nor flagged at equilibrium
    ///   (Recycle.vb:396-398, :419-421).
    /// - [`SolverError::RecycleMaxIterations`] on hitting the cap.
    /// - [`SolverError::UnknownObject`] for a dangling connection.
    pub fn calculate(
        &mut self,
        flowsheet: &mut Flowsheet,
        id: &ObjectId,
    ) -> Result<(), SolverError> {
        let (inlet, outlet) = recycle_endpoints(flowsheet, id, ObjectType::MaterialStream)?;

        let inlet_state = material_state(flowsheet, &inlet)?;
        let outlet_state = material_state(flowsheet, &outlet)?;

        // Per-compound mass-flow error (Recycle.vb:308-322).
        let v1 = inlet_state.compound_mass_flows.clone();
        let v2 = outlet_state.compound_mass_flows.clone();
        let w_sum: f64 = v1.iter().sum();
        let w_sum2: f64 = v2.iter().sum();
        let mut w_err = 0.0_f64;
        for (i, a) in v1.iter().enumerate() {
            let b = v2.get(i).copied().unwrap_or(0.0);
            w_err += (a - b).abs();
        }

        // History shift and update, all read off the inlet (Recycle.vb:324-343).
        let h = &mut self.convergence_history;
        h.temperature_err_prev = h.temperature_err;
        h.pressure_err_prev = h.pressure_err;
        h.mass_flow_err_prev = h.mass_flow_err;

        h.temperature_err = inlet_state.temperature - outlet_state.temperature;
        h.pressure_err = inlet_state.pressure - outlet_state.pressure;
        h.mass_flow_err = w_err;

        h.temperature_prev = h.temperature;
        h.pressure_prev = h.pressure;
        h.mass_flow_prev = h.mass_flow;

        h.temperature = inlet_state.temperature;
        h.pressure = inlet_state.pressure;
        h.mass_flow = w_sum;

        h.enthalpy_prev = h.enthalpy;
        h.enthalpy = inlet_state.enthalpy;
        h.entropy_prev = h.entropy;
        h.entropy = inlet_state.entropy;

        // Errors, then values (Recycle.vb:345-373). On the very first call
        // upstream seeds `Errors` with the raw inlet values, not with errors.
        if !self.initialised {
            self.errors = RecycleVariables {
                temperature: inlet_state.temperature,
                pressure: inlet_state.pressure,
                mass_flow: w_sum,
                enthalpy: inlet_state.enthalpy,
            };
            self.values = RecycleVariables {
                temperature: outlet_state.temperature,
                pressure: outlet_state.pressure,
                mass_flow: w_sum2,
                enthalpy: outlet_state.enthalpy,
            };
            self.initialised = true;
        } else {
            self.errors = RecycleVariables {
                temperature: self.values.temperature - inlet_state.temperature,
                pressure: self.values.pressure - inlet_state.pressure,
                mass_flow: w_err,
                enthalpy: self.values.enthalpy - inlet_state.enthalpy,
            };
            self.values = RecycleVariables {
                temperature: outlet_state.temperature,
                pressure: outlet_state.pressure,
                mass_flow: w_sum2,
                enthalpy: outlet_state.enthalpy,
            };
        }

        if self.legacy_mode {
            self.apply_legacy_update(flowsheet, &inlet, &outlet, &inlet_state)?;
        } else {
            self.apply_smoothed_update(flowsheet, &inlet, &outlet, &outlet_state)?;
        }

        // Iteration cap (Recycle.vb:439-442).
        if self.iteration_count >= self.max_iterations {
            self.iteration_count = 0;
            return Err(SolverError::RecycleMaxIterations(id.0.clone()));
        }

        // Convergence test — three monitored errors only (Recycle.vb:444-457).
        let p = self.convergence_parameters;
        let h = self.convergence_history;
        if h.temperature_err.abs() > p.temperature
            || h.pressure_err.abs() > p.pressure
            || h.mass_flow_err.abs() > p.mass_flow
        {
            self.converged = false;
        } else {
            if self.iteration_count != 0 {
                self.iterations_taken = self.iteration_count;
            }
            self.iteration_count = 0;
            self.converged = true;
        }

        self.iteration_count += 1;
        Ok(())
    }

    /// The `LegacyMode = True` branch (Recycle.vb:379-407): copy the inlet
    /// stream wholesale onto the outlet stream — plain successive substitution.
    fn apply_legacy_update(
        &mut self,
        flowsheet: &mut Flowsheet,
        inlet: &ObjectId,
        outlet: &ObjectId,
        inlet_state: &MaterialState,
    ) -> Result<(), SolverError> {
        // `Tnew/Pnew/Wnew` are computed upstream (:381-383) purely to be
        // validity-checked at :388; the copy itself uses the streams.
        let t_new = self.convergence_history.temperature;
        let p_new = self.convergence_history.pressure;
        let w_new = self.convergence_history.mass_flow;

        let copy_data = if self.copy_on_stream_data_error {
            true
        } else {
            t_new.is_finite()
                && p_new.is_finite()
                && w_new.is_finite()
                && inlet_state.mole_fraction_sum.is_finite()
        };

        if self.acceleration_method != AccelerationMethod::GlobalBroyden && copy_data {
            if !inlet_state.calculated && !inlet_state.at_equilibrium {
                return Err(SolverError::RecycleStreamNotCalculated(inlet.0.clone()));
            }
            assign_stream_state(flowsheet, inlet, outlet)?;
        }
        Ok(())
    }

    /// The `LegacyMode = False` branch (Recycle.vb:409-437): relax the outlet
    /// towards the inlet with [`RecycleBlock::smoothing_factor`].
    ///
    /// `T_out <- sf*T_in + (1-sf)*T_out_prev`, likewise pressure, and each
    /// compound's mass flow is relaxed between the inlet and outlet values.
    fn apply_smoothed_update(
        &mut self,
        flowsheet: &mut Flowsheet,
        inlet: &ObjectId,
        outlet: &ObjectId,
        outlet_state: &MaterialState,
    ) -> Result<(), SolverError> {
        let sf = self.smoothing_factor;
        let h = self.convergence_history;
        let t_new = sf * h.temperature + (1.0 - sf) * h.temperature_prev;
        let p_new = sf * h.pressure + (1.0 - sf) * h.pressure_prev;

        if self.acceleration_method == AccelerationMethod::GlobalBroyden {
            return Ok(());
        }
        if !outlet_state.calculated && !outlet_state.at_equilibrium {
            return Err(SolverError::RecycleStreamNotCalculated(outlet.0.clone()));
        }

        let v1 = material_state(flowsheet, inlet)?.compound_mass_flows;
        let v2 = material_state(flowsheet, outlet)?.compound_mass_flows;
        let relaxed: Vec<f64> = v1
            .iter()
            .enumerate()
            .map(|(i, a)| sf * a + (1.0 - sf) * v2.get(i).copied().unwrap_or(0.0))
            .collect();

        let obj = flowsheet
            .object_mut(outlet)
            .ok_or_else(|| SolverError::UnknownObject(outlet.0.clone()))?;
        let ms = obj
            .data
            .as_material_mut()
            .ok_or_else(|| SolverError::Other(format!("'{outlet}' is not a material stream")))?;
        ms.at_equilibrium = false;
        {
            let props = &mut ms.phases[PhaseIndex::Mixture.index()].properties;
            props.temperature = Some(t_new);
            props.pressure = Some(p_new);
        }
        set_overall_compound_mass_flows(ms, &relaxed);
        Ok(())
    }

    /// Push [`RecycleBlock::values`] onto the outlet stream — DWSIM's
    /// `SetOutletStreamProperties` (Recycle.vb:211-243).
    ///
    /// The solver calls this after a global Broyden step has rewritten the
    /// values (FlowsheetSolver.vb:1564). It sets the outlet's temperature \[K\],
    /// pressure \[Pa\], total mass flow \[kg/s\] and mass enthalpy \[kJ/kg\]
    /// from the block's values, copies the **inlet's** mole fractions onto the
    /// outlet, recomputes mass fractions, and clears the equilibrium flag.
    ///
    /// # Errors
    ///
    /// [`SolverError::RecycleNotConnected`] if the outlet slot is free;
    /// [`SolverError::UnknownObject`] for a dangling connection.
    pub fn set_outlet_stream_properties(
        &self,
        flowsheet: &mut Flowsheet,
        id: &ObjectId,
    ) -> Result<(), SolverError> {
        let (inlet, outlet) = recycle_endpoints(flowsheet, id, ObjectType::MaterialStream)?;
        let source_fractions = flowsheet
            .object(&inlet)
            .and_then(|o| o.data.as_material())
            .map(|ms| ms.overall_composition())
            .ok_or_else(|| SolverError::UnknownObject(inlet.0.clone()))?;

        let obj = flowsheet
            .object_mut(&outlet)
            .ok_or_else(|| SolverError::UnknownObject(outlet.0.clone()))?;
        let ms = obj
            .data
            .as_material_mut()
            .ok_or_else(|| SolverError::Other(format!("'{outlet}' is not a material stream")))?;
        {
            let props = &mut ms.phases[PhaseIndex::Mixture.index()].properties;
            props.temperature = Some(self.values.temperature);
            props.pressure = Some(self.values.pressure);
            props.massflow = Some(self.values.mass_flow);
            props.enthalpy = Some(self.values.enthalpy);
        }
        let n = ms.compound_count().min(source_fractions.len());
        for i in 0..n {
            ms.phases[PhaseIndex::Mixture.index()].compounds[i].mole_fraction =
                Some(source_fractions[i]);
        }
        ms.calc_overall_comp_mass_fractions();
        ms.at_equilibrium = false;
        Ok(())
    }

    /// The dynamic-mode path — DWSIM's `RunDynamicModel` (Recycle.vb:245-261).
    ///
    /// In dynamic mode a recycle does no convergence work at all: the transient
    /// integrator carries information around the loop through time, so the
    /// recycle degenerates to a straight copy of its inlet onto its outlet. The
    /// solver short-circuits recycle convergence entirely in dynamic mode
    /// (`If fbag.DynamicMode Then converged = True`, FlowsheetSolver.vb:1493).
    ///
    /// # Errors
    ///
    /// [`SolverError::RecycleStreamNotCalculated`] if the inlet has been neither
    /// calculated nor flagged at equilibrium; [`SolverError::RecycleNotConnected`]
    /// or [`SolverError::UnknownObject`] for connection problems.
    pub fn run_dynamic_model(
        &mut self,
        flowsheet: &mut Flowsheet,
        id: &ObjectId,
    ) -> Result<(), SolverError> {
        let (inlet, outlet) = recycle_endpoints(flowsheet, id, ObjectType::MaterialStream)?;
        let state = material_state(flowsheet, &inlet)?;
        if !state.calculated && !state.at_equilibrium {
            return Err(SolverError::RecycleStreamNotCalculated(inlet.0.clone()));
        }
        assign_stream_state(flowsheet, &inlet, &outlet)
    }
}

// ---------------------------------------------------------------------------
// Energy recycle
// ---------------------------------------------------------------------------

/// Convergence tolerance for an energy recycle — DWSIM's
/// `ConvergenceParametersE` (EnergyRecycle.vb:489-497).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnergyConvergenceParameters {
    /// Power tolerance \[kW\] (`Energy`, default `0.1`), DWSIM's internal
    /// energy-flow unit.
    pub energy: f64,
}

impl Default for EnergyConvergenceParameters {
    fn default() -> Self {
        EnergyConvergenceParameters { energy: 0.1 }
    }
}

impl EnergyConvergenceParameters {
    /// The power tolerance as a `uom` [`Power`] \[W\] (converted from the stored
    /// kW).
    #[must_use]
    pub fn power_tolerance(&self) -> Power {
        Power::new::<kilowatt>(self.energy)
    }
}

/// Iteration history for an energy recycle — DWSIM's `ConvergenceHistoryE`
/// (EnergyRecycle.vb:499-510). All values \[kW\].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EnergyConvergenceHistory {
    /// Current inlet energy flow \[kW\] (`Energy`).
    pub energy: f64,
    /// Previous inlet energy flow \[kW\] (`Energy0`).
    pub energy_prev: f64,
    /// Current error \[kW\] (`EnergyE`) — inlet minus the *previous* inlet,
    /// which is upstream's definition (EnergyRecycle.vb:213).
    pub energy_err: f64,
    /// Previous error \[kW\] (`EnergyE0`).
    pub energy_err_prev: f64,
}

/// What to do when an energy recycle exhausts its iteration budget.
///
/// Upstream pops a modal yes/no `MessageBox` (EnergyRecycle.vb:269-278). GUI is
/// out of scope, so the two answers become an explicit policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EnergyRecycleMaxIterationsPolicy {
    /// Upstream's **"No"** branch (`GoTo final`, :274): stop iterating, record
    /// the iteration count as taken, and accept the current value. This is the
    /// default because it is the non-destructive answer.
    #[default]
    StopAndAccept,
    /// Upstream's **"Yes"** branch (:276): zero the counter and keep going.
    ResetAndContinue,
}

/// An energy recycle block and its convergence state — DWSIM's
/// `SpecialOps.EnergyRecycle` (EnergyRecycle.vb:31-289).
///
/// The energy analogue of [`RecycleBlock`]: it tears a loop carrying a *duty*
/// rather than a material stream, and converges a single scalar, the energy flow
/// \[kW\].
///
/// # It does not gate the outer solver loop
///
/// The master routine collects only [`ObjectType::OtRecycle`] blocks into the
/// list it polls for convergence (FlowsheetSolver.vb:1325-1333), so an energy
/// recycle **never** holds the outer loop open upstream — it simply gets one
/// update per pass. [`EnergyRecycleBlock::converged`] is provided here as
/// honest information for callers, and the solver does not read it. Faithful.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyRecycleBlock {
    /// Tolerance (`ConvergenceParameters`, EnergyRecycle.vb:37).
    pub convergence_parameters: EnergyConvergenceParameters,
    /// History (`ConvergenceHistory`, EnergyRecycle.vb:38).
    pub convergence_history: EnergyConvergenceHistory,
    /// Acceleration selector — default [`AccelerationMethod::Wegstein`]
    /// (EnergyRecycle.vb:39), unlike the material recycle's `None`.
    pub acceleration_method: AccelerationMethod,
    /// Wegstein tuning (`WegsteinParameters`, EnergyRecycle.vb:40). **Read** by
    /// this block, unlike the material recycle's copy.
    pub wegstein_parameters: WegsteinParameters,
    /// Iteration cap (`MaximumIterations`, default `100`, EnergyRecycle.vb:42).
    pub max_iterations: i64,
    /// Iterations since the last convergence (`IterationCount`).
    pub iteration_count: i64,
    /// Iterations the last convergence took (`IterationsTaken`).
    pub iterations_taken: i64,
    /// The Wegstein "iterations since the last accelerated step" counter
    /// (`m_InternalCounterE`, EnergyRecycle.vb:44).
    pub internal_counter_energy: i64,
    /// Whether the last [`EnergyRecycleBlock::calculate`] was inside tolerance.
    /// Informational only — see the type's note.
    pub converged: bool,
    /// What to do on hitting [`EnergyRecycleBlock::max_iterations`].
    pub max_iterations_policy: EnergyRecycleMaxIterationsPolicy,
}

impl Default for EnergyRecycleBlock {
    fn default() -> Self {
        EnergyRecycleBlock::new()
    }
}

impl EnergyRecycleBlock {
    /// An energy recycle with every upstream default in place
    /// (EnergyRecycle.vb:37-45, :171-179): tolerance `0.1 kW`,
    /// `max_iterations = 100`, [`AccelerationMethod::Wegstein`].
    #[must_use]
    pub fn new() -> Self {
        EnergyRecycleBlock {
            convergence_parameters: EnergyConvergenceParameters::default(),
            convergence_history: EnergyConvergenceHistory::default(),
            acceleration_method: AccelerationMethod::Wegstein,
            wegstein_parameters: WegsteinParameters::default(),
            max_iterations: 100,
            iteration_count: 0,
            iterations_taken: 0,
            internal_counter_energy: 0,
            converged: false,
            max_iterations_policy: EnergyRecycleMaxIterationsPolicy::default(),
        }
    }

    /// Reset the iteration counter — `DeCalculate` (EnergyRecycle.vb:291-295).
    pub fn decalculate(&mut self) {
        self.iteration_count = 0;
    }

    /// Run one energy-recycle iteration — the port of `EnergyRecycle.Calculate`
    /// (EnergyRecycle.vb:200-289).
    ///
    /// # The update
    ///
    /// With `E` the inlet energy flow \[kW\] and subscript `0` the previous
    /// iteration, the history is `E_err = E - E_0_recorded`, then:
    ///
    /// - for the first four calls (`iteration_count <= 3`, :223), and for
    ///   [`AccelerationMethod::None`], the new value is plain successive
    ///   substitution, `E_new = E`;
    /// - for [`AccelerationMethod::Wegstein`] past the delay, the secant slope
    ///   is `s = (E_err - E_err_prev) / (E - E_prev)` and the Wegstein factor is
    ///   `q = s / (s - 1)`; the accelerated value
    ///   `E_new = E_err (1 - q) + E q` is taken only when the frequency counter
    ///   has matured, `s` is not NaN, and `qmin < q < qmax`
    ///   (:240-248). Otherwise `E_new = E` and the counter advances.
    ///
    /// # Upstream quirk, reproduced verbatim
    ///
    /// The `Select Case` at `:229-256` has arms for `None` and `Wegstein`
    /// **only**. With [`AccelerationMethod::DominantEigenvalue`] or
    /// [`AccelerationMethod::GlobalBroyden`] selected and
    /// `iteration_count > 3`, control falls straight through and `Enew` keeps
    /// its VB default of `0.0` — so the outlet energy stream is set to **zero
    /// power**. This port does the same, because the maintainer's instruction is
    /// to port what is there. Do not select those two methods on an energy
    /// recycle unless you want that behaviour.
    ///
    /// # Errors
    ///
    /// [`SolverError::RecycleNotConnected`] if either slot 0 is free;
    /// [`SolverError::UnknownObject`] for a dangling connection.
    pub fn calculate(
        &mut self,
        flowsheet: &mut Flowsheet,
        id: &ObjectId,
    ) -> Result<(), SolverError> {
        let (inlet, outlet) = recycle_endpoints(flowsheet, id, ObjectType::EnergyStream)?;

        let inlet_kw = flowsheet
            .object(&inlet)
            .and_then(|o| o.data.as_energy())
            .map(|es| es.power().map_or(0.0, |p| p.get::<kilowatt>()))
            .ok_or_else(|| SolverError::UnknownObject(inlet.0.clone()))?;

        // History (EnergyRecycle.vb:210-221).
        let h = &mut self.convergence_history;
        h.energy_err = inlet_kw - h.energy;
        h.energy_err_prev = h.energy - h.energy_prev;
        h.energy_prev = h.energy;
        h.energy = inlet_kw;

        // The update (EnergyRecycle.vb:223-258).
        let h = self.convergence_history;
        let mut e_new = 0.0_f64;
        if self.iteration_count <= 3 {
            e_new = h.energy;
        } else {
            match self.acceleration_method {
                AccelerationMethod::None => e_new = h.energy,
                AccelerationMethod::Wegstein => {
                    if self.wegstein_parameters.accel_delay <= (self.iteration_count + 3) as f64 {
                        let s_e = (h.energy_err - h.energy_err_prev) / (h.energy - h.energy_prev);
                        let q_e = s_e / (s_e - 1.0);
                        if self.wegstein_parameters.accel_freq <= self.internal_counter_energy
                            && !s_e.is_nan()
                            && q_e > self.wegstein_parameters.qmin
                            && q_e < self.wegstein_parameters.qmax
                        {
                            e_new = h.energy_err * (1.0 - q_e) + h.energy * q_e;
                            self.internal_counter_energy = 0;
                        } else {
                            e_new = h.energy;
                            self.internal_counter_energy += 1;
                        }
                    } else {
                        e_new = h.energy;
                    }
                }
                // No arm upstream — see the quirk note. `e_new` stays 0.0.
                AccelerationMethod::DominantEigenvalue | AccelerationMethod::GlobalBroyden => {}
            }
        }

        // Write the outlet energy flow (EnergyRecycle.vb:260-267), in kW.
        let obj = flowsheet
            .object_mut(&outlet)
            .ok_or_else(|| SolverError::UnknownObject(outlet.0.clone()))?;
        if let Some(es) = obj.data.as_energy_mut() {
            es.set_value_kw(e_new);
        }
        obj.calculated = true;

        // Iteration cap (EnergyRecycle.vb:269-278), then the counter, then the
        // convergence test (:280-287) — upstream's exact order, including the
        // `GoTo final` that jumps *past* the increment.
        let mut goto_final = false;
        if self.iteration_count >= self.max_iterations {
            match self.max_iterations_policy {
                EnergyRecycleMaxIterationsPolicy::StopAndAccept => goto_final = true,
                EnergyRecycleMaxIterationsPolicy::ResetAndContinue => self.iteration_count = 0,
            }
        }
        if !goto_final {
            self.iteration_count += 1;
        }
        let inside_tolerance =
            self.convergence_history.energy_err.abs() <= self.convergence_parameters.energy;
        if goto_final || inside_tolerance {
            self.iterations_taken = self.iteration_count;
            self.iteration_count = 0;
        }
        // Upstream has no `Converged` property on the energy recycle at all;
        // this is the tolerance test itself, exposed for callers.
        self.converged = inside_tolerance;
        Ok(())
    }

    /// The dynamic-mode path — DWSIM's `RunDynamicModel`
    /// (EnergyRecycle.vb:196-198), which is **empty upstream**.
    ///
    /// Kept as an explicit no-op so the call site reads the same as the material
    /// recycle's.
    pub fn run_dynamic_model(&mut self) {}
}

// ---------------------------------------------------------------------------
// Global Broyden
// ---------------------------------------------------------------------------

/// One Broyden quasi-Newton step over the pooled recycle variables — the port of
/// `MathEx.Broyden.broydn` (Broyden.vb:5-82).
///
/// # What it computes
///
/// Given the current variable vector `x`, the residual `f(x)`, and an
/// approximate **inverse** Jacobian `h`, it produces the step `p = -h f` and, on
/// an update call, first refreshes `h` by Broyden's rank-one formula. The caller
/// applies the step; upstream damps it, taking
/// `x_next = 0.3 x + 0.7 (x + p)` (FlowsheetSolver.vb:1560).
///
/// # Arguments
///
/// - `n` — number of equations. Upstream passes `totalv - 1` and loops
///   `For I = 0 To N`, i.e. `N` is the **last index**; this port takes the
///   **count** and loops `0..n`, which is the same set of indices.
/// - `x` — current variables (length `n`). Not modified; upstream says
///   explicitly that the caller must apply the step itself.
/// - `f` — residuals at `x` (length `n`).
/// - `p` — **out**: the predicted step (length `n`).
/// - `xb`, `fb` — **in/out**: the retained previous `x` and `f` (length `n`).
///   Also used as scratch during the update, exactly as upstream does.
/// - `h` — **in/out**: the `n x n` inverse-Jacobian approximation, row-major.
///   Initialise it to the identity on the first call.
/// - `update` — upstream's `IFLAG`. `false` (`0`) is the **initial** call: no
///   update of `h`. `true` (`1`) is an **update** call. The solver passes
///   `false` for the first two outer iterations and `true` afterwards
///   (`If(icount < 2, 0, 1)`, FlowsheetSolver.vb:1553).
///
/// Units are whatever the pooled recycle variables carry (K, Pa, kg/s, kJ/kg
/// interleaved), so this routine is necessarily unit-agnostic `f64`.
///
/// # Numerical note
///
/// Upstream hard-codes `THETA = 1.0` (Broyden.vb:61), which disables the damping
/// the routine's own comments describe, and divides by `DENOM` with no guard. A
/// zero `DENOM` yields non-finite entries in `h`; this port leaves that
/// behaviour alone but returns `false` when the resulting step is non-finite, so
/// the caller can skip it instead of poisoning the flowsheet.
///
/// # Returns
///
/// `true` if every component of `p` is finite, `false` otherwise.
#[allow(clippy::too_many_arguments)]
pub fn broydn(
    n: usize,
    x: &[f64],
    f: &[f64],
    p: &mut [f64],
    xb: &mut [f64],
    fb: &mut [f64],
    h: &mut [Vec<f64>],
    update: bool,
) -> bool {
    if n == 0
        || x.len() < n
        || f.len() < n
        || p.len() < n
        || xb.len() < n
        || fb.len() < n
        || h.len() < n
        || h.iter().take(n).any(|row| row.len() < n)
    {
        return false;
    }

    if update {
        // Broyden.vb:37-71.
        let mut ptp = 0.0_f64;
        for i in 0..n {
            p[i] = x[i] - xb[i];
            ptp += p[i] * p[i];
            let mut hy = 0.0_f64;
            for j in 0..n {
                hy += h[i][j] * (f[j] - fb[j]);
            }
            xb[i] = hy - p[i];
        }
        let mut pthy = 0.0_f64;
        let mut pthf = 0.0_f64;
        for i in 0..n {
            let mut pth = 0.0_f64;
            for j in 0..n {
                pth += p[j] * h[j][i];
            }
            pthy += pth * (f[i] - fb[i]);
            pthf += pth * f[i];
            fb[i] = pth;
        }
        // Upstream hard-codes THETA = 1.0 (:61), so `ptp` and `pthf` are
        // computed and then unused — kept for fidelity to the source.
        let theta = 1.0_f64;
        let denom = (1.0 - theta) * ptp + theta * pthy;
        let _ = pthf;
        for i in 0..n {
            for j in 0..n {
                h[i][j] -= theta * xb[i] * fb[j] / denom;
            }
        }
    }

    // Broyden.vb:72-80: retain x and f, then p = -H f.
    for i in 0..n {
        xb[i] = x[i];
        fb[i] = f[i];
        p[i] = 0.0;
        for j in 0..n {
            p[i] -= h[i][j] * f[j];
        }
    }

    p.iter().take(n).all(|v| v.is_finite())
}

// ---------------------------------------------------------------------------
// Stream helpers
// ---------------------------------------------------------------------------

/// The scalar state a recycle reads off a material stream, in DWSIM's internal
/// units.
#[derive(Debug, Clone, Default, PartialEq)]
struct MaterialState {
    /// Mixture-phase temperature \[K\], `0` if unset (upstream's
    /// `GetValueOrDefault`).
    temperature: f64,
    /// Mixture-phase pressure \[Pa\].
    pressure: f64,
    /// Mixture-phase mass enthalpy \[kJ/kg\].
    enthalpy: f64,
    /// Mixture-phase mass entropy \[kJ/(kg K)\].
    entropy: f64,
    /// Per-compound mass flows \[kg/s\], in slot order.
    compound_mass_flows: Vec<f64>,
    /// Sum of the mixture-phase mole fractions — upstream's
    /// `RET_VMOL(Phase.Mixture).Sum` validity probe (Recycle.vb:388).
    mole_fraction_sum: f64,
    /// The stream's `Calculated` flag.
    calculated: bool,
    /// The stream's `AtEquilibrium` flag.
    at_equilibrium: bool,
}

/// Read a material stream's scalar state.
fn material_state(flowsheet: &Flowsheet, id: &ObjectId) -> Result<MaterialState, SolverError> {
    let obj = flowsheet
        .object(id)
        .ok_or_else(|| SolverError::UnknownObject(id.0.clone()))?;
    let ms = obj
        .data
        .as_material()
        .ok_or_else(|| SolverError::Other(format!("'{id}' is not a material stream")))?;
    let props = &ms.phase(PhaseIndex::Mixture).properties;
    Ok(MaterialState {
        temperature: props.temperature.unwrap_or(0.0),
        pressure: props.pressure.unwrap_or(0.0),
        enthalpy: props.enthalpy.unwrap_or(0.0),
        entropy: props.entropy.unwrap_or(0.0),
        compound_mass_flows: ms
            .phase(PhaseIndex::Mixture)
            .compounds
            .iter()
            .map(|c| c.mass_flow.unwrap_or(0.0))
            .collect(),
        mole_fraction_sum: ms.overall_composition().iter().sum(),
        calculated: obj.calculated,
        at_equilibrium: ms.at_equilibrium,
    })
}

/// The (inlet, outlet) pair of a recycle block, by connector slot 0 on each side.
///
/// `want` is the stream type the block operates on:
/// [`ObjectType::MaterialStream`] for [`RecycleBlock`],
/// [`ObjectType::EnergyStream`] for [`EnergyRecycleBlock`]. It is checked, so a
/// mis-wired flowsheet is reported rather than silently mis-solved.
fn recycle_endpoints(
    flowsheet: &Flowsheet,
    id: &ObjectId,
    want: ObjectType,
) -> Result<(ObjectId, ObjectId), SolverError> {
    let obj = flowsheet
        .object(id)
        .ok_or_else(|| SolverError::UnknownObject(id.0.clone()))?;
    let inlet = obj
        .inputs
        .first()
        .and_then(|c| c.attachment.as_ref())
        .map(|a| a.peer.clone())
        .ok_or_else(|| SolverError::RecycleNotConnected(id.0.clone()))?;
    let outlet = obj
        .outputs
        .first()
        .and_then(|c| c.attachment.as_ref())
        .map(|a| a.peer.clone())
        .ok_or_else(|| SolverError::RecycleNotConnected(id.0.clone()))?;
    for peer in [&inlet, &outlet] {
        let ok = flowsheet.object(peer).is_some_and(|o| o.object_type == want);
        if !ok {
            return Err(SolverError::RecycleNotConnected(id.0.clone()));
        }
    }
    Ok((inlet, outlet))
}

/// Copy a material stream's mixture-phase state onto another — DWSIM's
/// `msto.Assign(msfrom)` + `msto.AssignProps(msfrom)` with the target's
/// `SpecType` preserved (Recycle.vb:400-405).
///
/// The target's `at_equilibrium` is cleared, matching upstream.
fn assign_stream_state(
    flowsheet: &mut Flowsheet,
    from: &ObjectId,
    to: &ObjectId,
) -> Result<(), SolverError> {
    let source = flowsheet
        .object(from)
        .and_then(|o| o.data.as_material())
        .cloned()
        .ok_or_else(|| SolverError::UnknownObject(from.0.clone()))?;
    let obj = flowsheet
        .object_mut(to)
        .ok_or_else(|| SolverError::UnknownObject(to.0.clone()))?;
    let ms = obj
        .data
        .as_material_mut()
        .ok_or_else(|| SolverError::Other(format!("'{to}' is not a material stream")))?;
    // Preserve the target's own specification, as upstream does.
    let previous_spec = ms.spec;
    ms.phases = source.phases.clone();
    ms.spec = previous_spec;
    ms.at_equilibrium = false;
    Ok(())
}

/// Set every compound's mass flow on the mixture phase and rebuild the totals —
/// DWSIM's `SetOverallCompoundMassFlow(i, w)` applied across the slate
/// (Recycle.vb:430-433).
///
/// Recomputes the total mass flow \[kg/s\] as the sum, the mass fractions as
/// `w_i / W`, and the mole fractions from those.
fn set_overall_compound_mass_flows(
    ms: &mut crate::flowsheet::MaterialStreamData,
    mass_flows: &[f64],
) {
    let n = ms.compound_count().min(mass_flows.len());
    for i in 0..n {
        ms.phases[PhaseIndex::Mixture.index()].compounds[i].mass_flow = Some(mass_flows[i]);
    }
    let total: f64 = mass_flows.iter().take(n).sum();
    ms.phases[PhaseIndex::Mixture.index()].properties.massflow = Some(total);
    if total > 0.0 {
        let fractions: Vec<f64> = mass_flows.iter().take(n).map(|w| w / total).collect();
        // `set_overall_mass_composition` normalises and back-fills; a failure
        // here can only mean a length mismatch, which the `min` above prevents.
        let _ = ms.set_overall_mass_composition(&fractions);
        ms.calc_overall_comp_mole_fractions();
    }
}

#[cfg(test)]
mod tests {
    //! # Verification — recycle convergence
    //!
    //! **Methodology.** Two kinds of check.
    //!
    //! 1. **Transcription checks** — upstream defaults, the history shift, the
    //!    convergence test, and the iteration cap, compared against the literal
    //!    values in `Recycle.vb` and `EnergyRecycle.vb`.
    //! 2. **Fixed-point checks** — a *contrived* scalar recycle whose loop map
    //!    is `g(x) = a + b x` with `|b| < 1`, so the fixed point
    //!    `x* = a / (1 - b)` is known analytically. Successive substitution and
    //!    the relaxed (smoothing-factor) update are both driven to that point
    //!    and their iteration counts compared. Pass criterion: convergence to
    //!    within the block's own tolerance, and the accelerated variant taking
    //!    no more iterations than plain successive substitution.
    //!
    //! No property package is involved, so no flash is exercised; these are
    //! verification tests against the transcribed algorithm, not validation
    //! against a physical benchmark.
    //!
    //! **Results (2026-08-11, release build):** recorded in each test's doc
    //! comment.

    use super::*;
    use crate::flowsheet::ObjectType;

    /// Build `[MS in] -> [RY] -> [MS out]` and return the ids.
    fn recycle_rig() -> (Flowsheet, ObjectId, ObjectId, ObjectId) {
        let mut fs = Flowsheet::new();
        let inlet = fs.add_object(ObjectType::MaterialStream, Some("IN"));
        let block = fs.add_object(ObjectType::OtRecycle, Some("RY-1"));
        let outlet = fs.add_object(ObjectType::MaterialStream, Some("OUT"));
        fs.connect(&inlet, &block, None, None).unwrap();
        fs.connect(&block, &outlet, None, None).unwrap();
        for id in [&inlet, &outlet] {
            let obj = fs.object_mut(id).unwrap();
            obj.calculated = true;
            let ms = obj.data.as_material_mut().unwrap();
            ms.add_compound("Water", 18.015);
            ms.equalize_overall_composition();
            ms.at_equilibrium = true;
        }
        (fs, inlet, block, outlet)
    }

    fn set_state(fs: &mut Flowsheet, id: &ObjectId, t: f64, p: f64, w: f64) {
        let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
        let props = &mut ms.phases[PhaseIndex::Mixture.index()].properties;
        props.temperature = Some(t);
        props.pressure = Some(p);
        props.massflow = Some(w);
        ms.phases[PhaseIndex::Mixture.index()].compounds[0].mass_flow = Some(w);
    }

    fn get_mass_flow(fs: &Flowsheet, id: &ObjectId) -> f64 {
        fs.object(id)
            .unwrap()
            .data
            .as_material()
            .unwrap()
            .phase(PhaseIndex::Mixture)
            .properties
            .massflow
            .unwrap_or(0.0)
    }

    /// **Methodology.** Check every upstream default literal:
    /// `Recycle.vb:44` (`MaximumIterations = 50`), `:41` (`AccelMethod.None`),
    /// `:62` (`SmoothingFactor = 1.0`), `:64` (`LegacyMode = True`),
    /// `:706-712` (tolerances), `:798-801` (Wegstein parameters),
    /// `EnergyRecycle.vb:39` (`AccelMethod.Wegstein`), `:42`
    /// (`MaximumIterations = 100`), `:491` (`Energy = 0.1`).
    /// **Result (2026-08-11):** every default matches the cited line exactly;
    /// the `uom` accessors return `0.1 K`, `0.1 Pa`, `0.01 kg/s`, `100 W`.
    #[test]
    fn defaults_match_upstream_literals() {
        let r = RecycleBlock::new();
        assert_eq!(r.max_iterations, 50);
        assert_eq!(r.acceleration_method, AccelerationMethod::None);
        assert!((r.smoothing_factor - 1.0).abs() < 1e-15);
        assert!(r.legacy_mode);
        assert!(!r.copy_on_stream_data_error);
        let p = r.convergence_parameters;
        assert!((p.temperature - 0.1).abs() < 1e-15);
        assert!((p.pressure - 0.1).abs() < 1e-15);
        assert!((p.mass_flow - 0.01).abs() < 1e-15);
        assert!((p.vapor_fraction - 0.01).abs() < 1e-15);
        assert!((p.enthalpy - 1.0).abs() < 1e-15);
        assert!((p.entropy - 0.01).abs() < 1e-15);
        assert!((p.composition - 0.001).abs() < 1e-15);
        assert!((p.temperature_tolerance().get::<kelvin>() - 0.1).abs() < 1e-12);
        assert!((p.pressure_tolerance().get::<pascal>() - 0.1).abs() < 1e-12);
        assert!(
            (p.mass_flow_tolerance().get::<kilogram_per_second>() - 0.01).abs() < 1e-12
        );

        let w = WegsteinParameters::default();
        assert_eq!(w.accel_freq, 4);
        assert!((w.qmax - 0.0).abs() < 1e-15);
        assert!((w.qmin + 20.0).abs() < 1e-15);
        assert!((w.accel_delay - 2.0).abs() < 1e-15);

        let e = EnergyRecycleBlock::new();
        assert_eq!(e.acceleration_method, AccelerationMethod::Wegstein);
        assert_eq!(e.max_iterations, 100);
        assert!((e.convergence_parameters.energy - 0.1).abs() < 1e-15);
        assert!(
            (e.convergence_parameters
                .power_tolerance()
                .get::<uom::si::power::watt>()
                - 100.0)
                .abs()
                < 1e-9
        );
    }

    /// **Methodology.** With inlet and outlet already identical
    /// (`T = 300 K`, `P = 1e5 Pa`, `w = 2 kg/s`), one `calculate` must report
    /// convergence, leave the errors at zero, and increment the counter to 1.
    /// Then perturb the inlet mass flow by `1 kg/s` — far outside the
    /// `0.01 kg/s` tolerance — and check it reports *not* converged and copies
    /// the inlet onto the outlet (successive substitution).
    /// **Result (2026-08-11, measured):** converged on the matched state with
    /// `mass_flow_err = 0.000000 kg/s` and `iteration_count = 1`; after the
    /// perturbation `converged = false`, `mass_flow_err = 1.000000 kg/s`, and
    /// the outlet mass flow became `3.000000 kg/s` (copied from the inlet).
    #[test]
    fn successive_substitution_converges_and_copies_the_inlet() {
        let (mut fs, inlet, block, outlet) = recycle_rig();
        set_state(&mut fs, &inlet, 300.0, 1.0e5, 2.0);
        set_state(&mut fs, &outlet, 300.0, 1.0e5, 2.0);

        let mut ry = RecycleBlock::new();
        ry.calculate(&mut fs, &block).unwrap();
        assert!(ry.converged);
        assert!(ry.convergence_history.mass_flow_err.abs() < 1e-15);
        assert_eq!(ry.iteration_count, 1);
        assert_eq!(ry.value_count(), RecycleVariables::LEN);

        set_state(&mut fs, &inlet, 300.0, 1.0e5, 3.0);
        ry.calculate(&mut fs, &block).unwrap();
        assert!(!ry.converged);
        assert!((ry.convergence_history.mass_flow_err - 1.0).abs() < 1e-12);
        assert!((get_mass_flow(&fs, &outlet) - 3.0).abs() < 1e-12);
    }

    /// **Methodology — the contrived fixed point.** Drive the loop map
    /// `w_next = a + b * w` with `a = 4 kg/s`, `b = 0.6`, whose fixed point is
    /// `w* = 4 / 0.4 = 10 kg/s`. Each outer iteration: read the outlet (the
    /// current guess), apply the map to produce the inlet, then call
    /// `calculate`. Run to `converged`, capped at 200 iterations. Compare plain
    /// successive substitution (`legacy_mode = true`) against the relaxed update
    /// (`legacy_mode = false`, `smoothing_factor = 1.0`, which upstream makes
    /// equivalent to successive substitution on temperature/pressure/mass flow).
    /// Pass criterion: both reach `|w - 10| <= 0.01 kg/s` (the block's own
    /// tolerance), and the relaxed variant takes no **more** iterations than the
    /// plain one.
    /// **Result (2026-08-11, measured):** successive substitution converged in
    /// **13** iterations at `w = 9.988245 kg/s`; the relaxed update with
    /// `sf = 1.0` converged in **13** iterations at the same
    /// `w = 9.988245 kg/s`. `13 <= 13` — the acceleration criterion holds, with
    /// equality, which is the expected outcome at `sf = 1.0` since upstream's
    /// non-legacy path reduces to successive substitution there.
    ///
    /// **Interpretation of the 0.0118 kg/s offset from `w* = 10`:** the block
    /// converges on `|w_in - w_out| <= 0.01 kg/s`, not on distance to the fixed
    /// point. With `b = 0.6` the two are related by
    /// `|w_in - w_out| = 0.4 |w - 10|`, so the tolerance admits
    /// `|w - 10| <= 0.025 kg/s`. The measured `0.0118 kg/s` sits inside that,
    /// so the port is converging exactly where upstream's criterion says it
    /// should — the test bound of `0.03 kg/s` is set from this algebra, not from
    /// the observation.
    #[test]
    fn contrived_fixed_point_converges_and_relaxation_is_no_worse() {
        fn run(legacy: bool, sf: f64) -> (usize, f64) {
            let (mut fs, inlet, block, outlet) = recycle_rig();
            set_state(&mut fs, &outlet, 300.0, 1.0e5, 1.0);
            let mut ry = RecycleBlock::new();
            ry.legacy_mode = legacy;
            ry.smoothing_factor = sf;
            ry.max_iterations = 500;

            for i in 1..=200usize {
                // The loop: whatever the outlet guesses comes back as
                // `a + b * w` at the inlet.
                let guess = get_mass_flow(&fs, &outlet);
                let returned = 4.0 + 0.6 * guess;
                set_state(&mut fs, &inlet, 300.0, 1.0e5, returned);
                ry.calculate(&mut fs, &block).unwrap();
                if ry.converged {
                    return (i, get_mass_flow(&fs, &outlet));
                }
            }
            (usize::MAX, get_mass_flow(&fs, &outlet))
        }

        let (n_ss, w_ss) = run(true, 1.0);
        let (n_relaxed, w_relaxed) = run(false, 1.0);

        assert!(n_ss < 200, "successive substitution did not converge");
        assert!(n_relaxed < 200, "relaxed update did not converge");
        assert!((w_ss - 10.0).abs() <= 0.03, "w_ss = {w_ss}");
        assert!((w_relaxed - 10.0).abs() <= 0.03, "w_relaxed = {w_relaxed}");
        assert!(
            n_relaxed <= n_ss,
            "relaxed update took {n_relaxed} iterations vs {n_ss} for plain SS"
        );
    }

    /// **Methodology.** A recycle that never converges must stop at
    /// `max_iterations` with [`SolverError::RecycleMaxIterations`], and the
    /// counter must be zeroed at the same moment (Recycle.vb:439-442).
    /// **Result (2026-08-11):** with `max_iterations = 3`, the fourth call
    /// returns `Err(RecycleMaxIterations("RY-1"))` and leaves
    /// `iteration_count = 0`.
    #[test]
    fn iteration_cap_is_enforced() {
        let (mut fs, inlet, block, outlet) = recycle_rig();
        set_state(&mut fs, &outlet, 300.0, 1.0e5, 1.0);
        let mut ry = RecycleBlock::new();
        ry.max_iterations = 3;

        let mut last = Ok(());
        for i in 0..4 {
            // Always disagree by 100 kg/s so it can never converge.
            set_state(&mut fs, &inlet, 300.0, 1.0e5, 100.0 + i as f64);
            last = ry.calculate(&mut fs, &block);
            if last.is_err() {
                break;
            }
        }
        assert!(
            matches!(last, Err(SolverError::RecycleMaxIterations(_))),
            "expected RecycleMaxIterations, got {last:?}"
        );
        assert_eq!(ry.iteration_count, 0);
    }

    /// **Methodology.** The Wegstein path on an energy recycle. Drive the scalar
    /// loop map `E_next = 20 + 0.5 * E` (fixed point `40 kW`) through an
    /// [`EnergyRecycleBlock`], and separately confirm the documented upstream
    /// quirk: selecting [`AccelerationMethod::DominantEigenvalue`] past the
    /// fourth iteration drives the outlet to **zero** because the upstream
    /// `Select Case` has no arm for it.
    /// **Result (2026-08-11, measured):** the Wegstein block converged in **9**
    /// iterations at `E = 39.923828 kW`, i.e. `0.076 kW` from the analytic fixed
    /// point `40 kW` and inside the block's own `0.1 kW` tolerance; the
    /// dominant-eigenvalue block left the outlet at `0.000000 kW` after 8 calls
    /// with a moving inlet, reproducing the no-arm quirk.
    #[test]
    fn energy_recycle_wegstein_and_the_no_arm_quirk() {
        fn energy_rig() -> (Flowsheet, ObjectId, ObjectId, ObjectId) {
            let mut fs = Flowsheet::new();
            let inlet = fs.add_object(ObjectType::EnergyStream, Some("EIN"));
            let block = fs.add_object(ObjectType::OtEnergyRecycle, Some("ER-1"));
            let outlet = fs.add_object(ObjectType::EnergyStream, Some("EOUT"));
            fs.connect(&inlet, &block, None, None).unwrap();
            fs.connect(&block, &outlet, None, None).unwrap();
            (fs, inlet, block, outlet)
        }
        fn set_kw(fs: &mut Flowsheet, id: &ObjectId, kw: f64) {
            fs.object_mut(id)
                .unwrap()
                .data
                .as_energy_mut()
                .unwrap()
                .set_value_kw(kw);
        }
        fn get_kw(fs: &Flowsheet, id: &ObjectId) -> f64 {
            fs.object(id)
                .unwrap()
                .data
                .as_energy()
                .unwrap()
                .power()
                .map_or(0.0, |p| p.get::<kilowatt>())
        }

        let (mut fs, inlet, block, outlet) = energy_rig();
        set_kw(&mut fs, &outlet, 1.0);
        let mut er = EnergyRecycleBlock::new();
        let mut iterations = 0usize;
        for i in 1..=100usize {
            let guess = get_kw(&fs, &outlet);
            set_kw(&mut fs, &inlet, 20.0 + 0.5 * guess);
            er.calculate(&mut fs, &block).unwrap();
            if er.converged {
                iterations = i;
                break;
            }
        }
        assert!(iterations > 0, "energy recycle did not converge");
        assert!(
            (get_kw(&fs, &outlet) - 40.0).abs() <= 0.1,
            "E = {}",
            get_kw(&fs, &outlet)
        );

        // The documented no-arm quirk.
        let (mut fs, inlet, block, outlet) = energy_rig();
        let mut er = EnergyRecycleBlock::new();
        er.acceleration_method = AccelerationMethod::DominantEigenvalue;
        // The inlet must keep moving, otherwise the block converges on its
        // second call and resets `iteration_count` before it can pass 3.
        for i in 0..8 {
            set_kw(&mut fs, &inlet, 100.0 + 10.0 * f64::from(i));
            er.calculate(&mut fs, &block).unwrap();
        }
        assert!(
            get_kw(&fs, &outlet).abs() < 1e-15,
            "the no-arm fall-through must leave the outlet at zero, got {}",
            get_kw(&fs, &outlet)
        );
    }

    /// **Methodology.** [`broydn`] on the linear system `f(x) = A x - b` with
    /// `A = [[2, 0], [0, 4]]`, `b = [2, 8]` and exact root `x* = [1, 2]`.
    /// Starting from `x = [0, 0]` with `h` the identity, the first (initial)
    /// call must return the steepest-descent step `p = -f`, and iterating with
    /// `update = true` must approach `x*`. Pass criterion: `|x - x*| < 1e-6`
    /// within 40 iterations.
    /// **Result (2026-08-11, measured):** the initial call returned
    /// `p = [2, 8]` exactly; the iteration reached `x = [1.0, 2.0]` (exact to
    /// the printed precision) after **4** passes of the update loop, with every
    /// residual below `1e-10`.
    #[test]
    fn broydn_finds_the_root_of_a_linear_system() {
        let residual = |x: &[f64]| vec![2.0 * x[0] - 2.0, 4.0 * x[1] - 8.0];

        let n = 2usize;
        let mut x = vec![0.0_f64; n];
        let mut p = vec![0.0_f64; n];
        let mut xb = vec![0.0_f64; n];
        let mut fb = vec![0.0_f64; n];
        let mut h = vec![vec![0.0_f64; n]; n];
        for (i, row) in h.iter_mut().enumerate() {
            row[i] = 1.0;
        }

        let f = residual(&x);
        assert!(broydn(n, &x, &f, &mut p, &mut xb, &mut fb, &mut h, false));
        assert!((p[0] - 2.0).abs() < 1e-12, "{p:?}");
        assert!((p[1] - 8.0).abs() < 1e-12, "{p:?}");

        let mut steps = 0usize;
        for i in 0..n {
            x[i] += p[i];
        }
        for k in 1..=40usize {
            let f = residual(&x);
            if f.iter().all(|v| v.abs() < 1e-10) {
                steps = k;
                break;
            }
            assert!(broydn(n, &x, &f, &mut p, &mut xb, &mut fb, &mut h, true));
            for i in 0..n {
                x[i] += p[i];
            }
        }
        assert!(steps > 0, "broydn did not converge: x = {x:?}");
        assert!((x[0] - 1.0).abs() < 1e-6, "{x:?}");
        assert!((x[1] - 2.0).abs() < 1e-6, "{x:?}");
    }

    /// **Methodology.** `set_outlet_stream_properties` must push the block's
    /// four values onto the outlet and take the inlet's mole fractions
    /// (Recycle.vb:211-243).
    /// **Result (2026-08-11):** outlet `T = 321.000000 K`,
    /// `P = 123456.000000 Pa`, `w = 7.000000 kg/s`, `h = 55.000000 kJ/kg`;
    /// `at_equilibrium` cleared.
    #[test]
    fn set_outlet_stream_properties_pushes_the_values() {
        let (mut fs, inlet, block, outlet) = recycle_rig();
        set_state(&mut fs, &inlet, 300.0, 1.0e5, 2.0);
        set_state(&mut fs, &outlet, 290.0, 0.9e5, 1.0);

        let mut ry = RecycleBlock::new();
        ry.values = RecycleVariables {
            temperature: 321.0,
            pressure: 123_456.0,
            mass_flow: 7.0,
            enthalpy: 55.0,
        };
        ry.set_outlet_stream_properties(&mut fs, &block).unwrap();

        let ms = fs.object(&outlet).unwrap().data.as_material().unwrap();
        let props = &ms.phase(PhaseIndex::Mixture).properties;
        assert!((props.temperature.unwrap() - 321.0).abs() < 1e-12);
        assert!((props.pressure.unwrap() - 123_456.0).abs() < 1e-9);
        assert!((props.massflow.unwrap() - 7.0).abs() < 1e-12);
        assert!((props.enthalpy.unwrap() - 55.0).abs() < 1e-12);
        assert!(!ms.at_equilibrium);
    }
}
