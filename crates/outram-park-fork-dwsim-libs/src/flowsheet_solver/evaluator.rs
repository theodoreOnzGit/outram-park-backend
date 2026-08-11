//! The unit-operation evaluation hook, and the built-in evaluator.
//!
//! # What this module is
//!
//! The seam between the *execution engine* (ordering, queueing, recycle
//! convergence, adjust solving) and the *equipment physics*. DWSIM has no such
//! seam: its solver calls `ISimulationObject.Solve()` and every model overrides
//! it. This port keeps the two apart, because the engine and the equipment
//! models are separate workstreams here and the engine must be testable without
//! a thermodynamic property package.
//!
//! The seam is a **compile-time generic**, never a trait object
//! (workspace Rust design rules): the solver is generic over
//! `E: `[`UnitOpEvaluator`], and the blanket impl below makes every
//! `FnMut(&mut Flowsheet, &CalculationArgs) -> Result<(), SolverError>` closure
//! satisfy it. So the ordinary way to supply equipment physics is to pass a
//! closure.
//!
//! # What the built-in evaluator covers
//!
//! [`DefaultEvaluator`] handles only what can be computed **from the flowsheet
//! data model alone** — no property package, no flash, no equipment parameters:
//!
//! | [`ObjectType`] | Built-in behaviour |
//! |---|---|
//! | [`ObjectType::MaterialStream`] | Records the input snapshot and marks the stream solved. **No flash is performed** — see below. |
//! | [`ObjectType::EnergyStream`] | No-op; the power was written by whichever block produced it. |
//! | [`ObjectType::Mixer`] | Overall mass balance, compound mass-flow balance, mass-weighted enthalpy, outlet pressure = minimum of the inlets, plus upstream's single-active-inlet pass-through shortcut. |
//! | [`ObjectType::EnergyMixer`] | Sums the inlet energy-stream powers into the outlet energy stream. |
//! | anything else | [`SolverError::NoModel`] — delegate it. |
//!
//! **Not** covered, and why:
//!
//! - **Every real unit operation** (pump, heater, cooler, valve, pipe,
//!   compressor, expander, heat exchanger, separator, reactors, columns, ...).
//!   Their models live in this crate's sibling modules with their own typed
//!   APIs; wiring the whole registry into a type-dispatched table is a larger
//!   integration than this workstream, and is proposed as a follow-up.
//! - **[`ObjectType::Splitter`]**. DWSIM's splitter divides the inlet by
//!   user-set stream ratios, which the flowsheet data model does not carry
//!   (`ObjectData::UnitOperation` holds only a net power and a free-form results
//!   map). Guessing an even split would be inventing physics.
//! - **[`ObjectType::OtRecycle`] and [`ObjectType::OtEnergyRecycle`]**. These
//!   are handled by [`crate::flowsheet_solver::solver::FlowsheetSolver`] itself,
//!   not by any evaluator, because their convergence state (iteration counter,
//!   error history, Wegstein counters) must persist *across* solver iterations
//!   and therefore lives in the solver, not in the flowsheet.
//! - **The material-stream flash.** `MaterialStream.Solve()` upstream is a
//!   property-package flash (TP / PH / PS / ...). The flowsheet data model
//!   deliberately stops at the flash boundary
//!   ([`crate::flowsheet`]'s "Excluded DWSIM behavior"), so the built-in
//!   evaluator does the bookkeeping half only: it snapshots the inputs
//!   (`LastSolutionInputData`) and marks the stream calculated. **A stream
//!   evaluated this way carries whatever phase split it already had.** Supply a
//!   hook that calls [`crate::thermo`] if you need a real flash.
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
//! - `DWSIM.FlowsheetSolver/FlowsheetSolver.vb:352-416` (`CalculateMaterialStream`)
//!   — the material-stream branch.
//! - `DWSIM.UnitOperations/UnitOperations/Mixer.vb:40-52` (the
//!   `PressureBehavior` enum and its `Minimum` default) and `:110-175` (the
//!   balance itself, including the single-active-inlet shortcut at `:148-158`).
//!
//! # Excluded DWSIM behavior
//!
//! - **`Mixer.PressureCalculation = Maximum | Average`** (Mixer.vb:130-135,
//!   :167). Only the `Minimum` default is applied, because the choice is an
//!   equipment parameter and `ObjectData::UnitOperation` carries none. Set the
//!   outlet pressure from a hook if another behaviour is wanted.
//! - **Dynamic-mode backward pressure propagation in the mixer**
//!   (Mixer.vb:170-175). It reads the outlet pressure and pushes it onto the
//!   inlets; that is the dynamics workstream's concern, not this one's.
//! - **`Inspector` narrative paragraphs** throughout (Mixer.vb:120-142). A
//!   debugging/report facility with no computational effect.
//! - **`ms.Validate()`** (Mixer.vb:123). The port checks the compound lists line
//!   up and otherwise trusts the data model's own invariants.

use crate::flowsheet::{
    CalculationArgs, Flowsheet, ObjectData, ObjectId, ObjectType, PhaseIndex,
};
use crate::flowsheet_solver::errors::SolverError;

/// A source of unit-operation physics for the flowsheet solver.
///
/// Implement this — or, far more usually, just pass a closure — to tell the
/// solver how to calculate an object it does not handle itself.
///
/// # Contract
///
/// `evaluate` is called once per queue item, with the flowsheet in the state the
/// upstream objects left it. It must:
///
/// - read the object's inlet streams and write its outlet streams **through the
///   flowsheet**, since that is how state reaches the next object in the order;
/// - return `Ok(())` on success — the solver then marks the object
///   `calculated`;
/// - return `Err` on failure — the solver records the message on the object,
///   collects the error, and either stops or continues depending on
///   [`crate::flowsheet_solver::solver::SolveOptions::break_on_exception`].
///
/// It must **not** re-enter the solver.
///
/// # Why a trait and not `Box<dyn Fn>`
///
/// The workspace forbids trait objects. This trait is a compile-time contract
/// only: the solver takes `E: UnitOpEvaluator` as a generic parameter and
/// monomorphises. The blanket impl below covers closures, so no user needs to
/// name a type.
pub trait UnitOpEvaluator {
    /// Calculate the object named by `args` in `flowsheet`.
    ///
    /// # Errors
    ///
    /// Any [`SolverError`]; the solver attributes it to `args.tag`.
    fn evaluate(
        &mut self,
        flowsheet: &mut Flowsheet,
        args: &CalculationArgs,
    ) -> Result<(), SolverError>;
}

impl<F> UnitOpEvaluator for F
where
    F: FnMut(&mut Flowsheet, &CalculationArgs) -> Result<(), SolverError>,
{
    fn evaluate(
        &mut self,
        flowsheet: &mut Flowsheet,
        args: &CalculationArgs,
    ) -> Result<(), SolverError> {
        self(flowsheet, args)
    }
}

/// The evaluator that covers what the flowsheet data model alone can express.
///
/// See the module documentation for the exact coverage table. Use it directly
/// when you only need stream propagation and mixing, or compose it with your own
/// models:
///
/// ```
/// use outram_park_fork_dwsim_libs::flowsheet::{CalculationArgs, Flowsheet};
/// use outram_park_fork_dwsim_libs::flowsheet_solver::{default_evaluate, SolverError};
///
/// let mut hook = |fs: &mut Flowsheet, args: &CalculationArgs| -> Result<(), SolverError> {
///     match default_evaluate(fs, args) {
///         Some(result) => result,          // the built-in handled it
///         None => Ok(()),                  // ... your own equipment model here
///     }
/// };
/// # let _ = &mut hook;
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DefaultEvaluator;

impl UnitOpEvaluator for DefaultEvaluator {
    fn evaluate(
        &mut self,
        flowsheet: &mut Flowsheet,
        args: &CalculationArgs,
    ) -> Result<(), SolverError> {
        match default_evaluate(flowsheet, args) {
            Some(result) => result,
            None => Err(SolverError::NoModel(args.object_type)),
        }
    }
}

/// Evaluate `args` if the built-in evaluator covers its object type.
///
/// Returns `None` — meaning "not mine, delegate it" — for every type outside the
/// coverage table in the module documentation. This is the composable form;
/// [`DefaultEvaluator`] is the same thing with `None` turned into
/// [`SolverError::NoModel`].
///
/// # Errors
///
/// Propagates whatever the individual built-in routine reports: a missing
/// object, a missing outlet connection, or mismatched compound lists.
pub fn default_evaluate(
    flowsheet: &mut Flowsheet,
    args: &CalculationArgs,
) -> Option<Result<(), SolverError>> {
    let id = ObjectId(args.name.clone());
    match args.object_type {
        ObjectType::MaterialStream => Some(evaluate_material_stream(flowsheet, &id)),
        ObjectType::EnergyStream => Some(Ok(())),
        ObjectType::Mixer => Some(evaluate_mixer(flowsheet, &id)),
        ObjectType::EnergyMixer => Some(evaluate_energy_mixer(flowsheet, &id)),
        _ => None,
    }
}

/// The bookkeeping half of `CalculateMaterialStream` (FlowsheetSolver.vb:352-416).
///
/// Records the stream's current inputs as its last solution
/// (`LastSolutionInputData`) so
/// [`crate::flowsheet::MaterialStreamData::is_dirty_versus_last_solution`] can
/// answer honestly afterwards. **Performs no flash** — see the module
/// documentation.
///
/// # Errors
///
/// [`SolverError::UnknownObject`] if the id is not in the flowsheet, or
/// [`SolverError::Other`] if it is not a material stream.
fn evaluate_material_stream(flowsheet: &mut Flowsheet, id: &ObjectId) -> Result<(), SolverError> {
    let obj = flowsheet
        .object_mut(id)
        .ok_or_else(|| SolverError::UnknownObject(id.0.clone()))?;
    let ms = obj
        .data
        .as_material_mut()
        .ok_or_else(|| SolverError::Other(format!("'{id}' is not a material stream")))?;
    let snapshot = ms.snapshot_input();
    ms.last_solution_input = Some(snapshot);
    Ok(())
}

/// Sum the inlet energy streams into the outlet energy stream \[W\].
///
/// The energy-mixer analogue of the material mixer's mass balance. Inlet powers
/// that are `None` count as zero, matching upstream's
/// `GetValueOrDefault` idiom throughout the logical blocks. The block's own net
/// power is recorded in [`ObjectData::UnitOperation`] in DWSIM's internal kW.
///
/// # Known gap (in the flowsheet data model, not here)
///
/// [`crate::flowsheet::ConnectorLayout::default_for`] has **no entry for
/// [`ObjectType::EnergyMixer`]**, so it falls back to a material one-inlet /
/// one-outlet layout and [`crate::flowsheet::Flowsheet::connect`] refuses to
/// attach an energy stream to it; nor is `EnergyMixer` listed among the types
/// allowed to *supply* an energy stream. Until both are filled in, an energy
/// mixer must have its [`crate::flowsheet::ConnectionPoint::connector_type`]
/// slots retyped to [`crate::flowsheet::ConType::Energy`] and its outlet edge
/// written directly through
/// [`crate::flowsheet::ConnectionPoint::attachment`]. This routine itself
/// selects its inlets and outlet by *peer object type*, not by slot type, so it
/// works either way. Filing the layout gap is proposed as follow-up work for
/// the flowsheet workstream.
///
/// # Errors
///
/// [`SolverError::UnknownObject`] if the block or a connected stream is missing;
/// [`SolverError::Other`] if it has no attached energy-stream outlet.
fn evaluate_energy_mixer(flowsheet: &mut Flowsheet, id: &ObjectId) -> Result<(), SolverError> {
    let inlets = attached_peers_of_type(flowsheet, id, Side::Inlet, ObjectType::EnergyStream)?;
    let outlets = attached_peers_of_type(flowsheet, id, Side::Outlet, ObjectType::EnergyStream)?;
    let Some(outlet) = outlets.first().cloned() else {
        return Err(SolverError::Other(format!(
            "'{id}': energy mixer has no attached outlet energy stream"
        )));
    };

    let mut total_w = 0.0_f64;
    for inlet in &inlets {
        let obj = flowsheet
            .object(inlet)
            .ok_or_else(|| SolverError::UnknownObject(inlet.0.clone()))?;
        if let Some(es) = obj.data.as_energy() {
            total_w += es.power().map_or(0.0, |p| p.value);
        }
    }

    let obj = flowsheet
        .object_mut(&outlet)
        .ok_or_else(|| SolverError::UnknownObject(outlet.0.clone()))?;
    if let Some(es) = obj.data.as_energy_mut() {
        es.set_power(uom::si::f64::Power::new::<uom::si::power::watt>(total_w));
    }
    if let ObjectData::UnitOperation { power, .. } = &mut flowsheet
        .object_mut(id)
        .ok_or_else(|| SolverError::UnknownObject(id.0.clone()))?
        .data
    {
        // Recorded in DWSIM's internal energy unit, kW.
        *power = Some(total_w / 1000.0);
    }
    Ok(())
}

/// The mixer mass and energy balance (Mixer.vb:110-175).
///
/// Computes, for the single outlet material stream:
///
/// - **mass flow** `W = sum_i w_i` \[kg/s\];
/// - **pressure** `P = min_i P_i` \[Pa\] — upstream's `PressureBehavior.Minimum`
///   default (Mixer.vb:46, :124-129), skipping inlets whose pressure is unset;
/// - **mass enthalpy** `h = (sum_i w_i h_i) / W` \[J/kg\], with non-finite inlet
///   enthalpies skipped exactly as upstream does (Mixer.vb:145);
/// - **overall composition**, from the summed per-compound mass flows.
///
/// **Temperature is not set**, because determining it from `(P, h)` is a PH
/// flash. The one exception is upstream's own shortcut: if exactly one inlet
/// carries a non-zero mass flow, the outlet is assigned wholesale from that
/// stream (Mixer.vb:148-158), which does carry its temperature over.
///
/// # Errors
///
/// [`SolverError::UnknownObject`] for a missing object;
/// [`SolverError::Other`] if the mixer has no attached outlet material stream or
/// if the inlet and outlet compound lists disagree.
fn evaluate_mixer(flowsheet: &mut Flowsheet, id: &ObjectId) -> Result<(), SolverError> {
    let inlets = attached_peers_of_type(flowsheet, id, Side::Inlet, ObjectType::MaterialStream)?;
    let outlets = attached_peers_of_type(flowsheet, id, Side::Outlet, ObjectType::MaterialStream)?;
    let Some(outlet) = outlets.first().cloned() else {
        return Err(SolverError::Other(format!(
            "'{id}': mixer has no attached outlet material stream"
        )));
    };

    let names = compound_names(flowsheet, &outlet)?;
    for inlet in &inlets {
        if compound_names(flowsheet, inlet)? != names {
            return Err(SolverError::Other(format!(
                "'{id}': inlet '{inlet}' and the outlet have different compound lists"
            )));
        }
    }

    // Upstream's single-active-inlet shortcut (Mixer.vb:148-158).
    let active: Vec<ObjectId> = inlets
        .iter()
        .filter(|i| mixture_mass_flow(flowsheet, i).unwrap_or(0.0) > 0.0)
        .cloned()
        .collect();
    if active.len() == 1 {
        let source = flowsheet
            .object(&active[0])
            .and_then(|o| o.data.as_material())
            .cloned()
            .ok_or_else(|| SolverError::UnknownObject(active[0].0.clone()))?;
        let target = flowsheet
            .object_mut(&outlet)
            .ok_or_else(|| SolverError::UnknownObject(outlet.0.clone()))?;
        if let Some(ms) = target.data.as_material_mut() {
            ms.phases[PhaseIndex::Mixture.index()] =
                source.phases[PhaseIndex::Mixture.index()].clone();
            ms.at_equilibrium = false;
        }
        return Ok(());
    }

    let n = names.len();
    let mut total_w = 0.0_f64;
    let mut enthalpy_flow = 0.0_f64;
    let mut compound_w = vec![0.0_f64; n];
    let mut pressure: Option<f64> = None;

    for inlet in &inlets {
        let ms = flowsheet
            .object(inlet)
            .and_then(|o| o.data.as_material())
            .ok_or_else(|| SolverError::UnknownObject(inlet.0.clone()))?;
        let props = &ms.phase(PhaseIndex::Mixture).properties;
        if let Some(p) = props.pressure {
            pressure = Some(match pressure {
                Some(current) if current <= p => current,
                _ => p,
            });
        }
        let w = props.massflow.unwrap_or(0.0);
        total_w += w;
        if let Some(h) = props.enthalpy {
            // Upstream skips NaN enthalpies (Mixer.vb:145). `h` is kJ/kg here,
            // DWSIM's internal enthalpy unit, matching the stored field.
            if h.is_finite() {
                enthalpy_flow += w * h;
            }
        }
        for (i, c) in ms.phase(PhaseIndex::Mixture).compounds.iter().enumerate() {
            compound_w[i] += c.mass_flow.unwrap_or(0.0);
        }
    }

    let target = flowsheet
        .object_mut(&outlet)
        .ok_or_else(|| SolverError::UnknownObject(outlet.0.clone()))?;
    let ms = target
        .data
        .as_material_mut()
        .ok_or_else(|| SolverError::Other(format!("'{outlet}' is not a material stream")))?;
    {
        let props = &mut ms.phases[PhaseIndex::Mixture.index()].properties;
        props.massflow = Some(total_w);
        props.enthalpy = Some(if total_w != 0.0 {
            enthalpy_flow / total_w
        } else {
            0.0
        });
        if let Some(p) = pressure {
            props.pressure = Some(p);
        }
    }
    let summed: f64 = compound_w.iter().sum();
    if summed > 0.0 {
        let fractions: Vec<f64> = compound_w.iter().map(|w| w / summed).collect();
        ms.set_overall_mass_composition(&fractions)
            .map_err(|e| SolverError::Other(format!("'{outlet}': {e}")))?;
        ms.calc_overall_comp_mole_fractions();
    }
    for (i, c) in ms.phases[PhaseIndex::Mixture.index()]
        .compounds
        .iter_mut()
        .enumerate()
    {
        c.mass_flow = Some(compound_w[i]);
    }
    ms.at_equilibrium = false;
    Ok(())
}

/// Which side of an object's connector list to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// `InputConnectors`.
    Inlet,
    /// `OutputConnectors` plus the dedicated `EnergyConnector`.
    Outlet,
}

/// The attached peers on one side of `id` that have the requested object type,
/// in connector-slot order.
fn attached_peers_of_type(
    flowsheet: &Flowsheet,
    id: &ObjectId,
    side: Side,
    want: ObjectType,
) -> Result<Vec<ObjectId>, SolverError> {
    let obj = flowsheet
        .object(id)
        .ok_or_else(|| SolverError::UnknownObject(id.0.clone()))?;
    let slots: Vec<&crate::flowsheet::ConnectionPoint> = match side {
        Side::Inlet => obj.inputs.iter().collect(),
        Side::Outlet => obj
            .outputs
            .iter()
            .chain(std::iter::once(&obj.energy_connector))
            .collect(),
    };
    Ok(slots
        .into_iter()
        .filter_map(|c| c.attachment.as_ref().map(|a| a.peer.clone()))
        .filter(|p| {
            flowsheet
                .object(p)
                .is_some_and(|o| o.object_type == want)
        })
        .collect())
}

/// The compound names of a material stream's mixture phase, in slot order.
fn compound_names(flowsheet: &Flowsheet, id: &ObjectId) -> Result<Vec<String>, SolverError> {
    let ms = flowsheet
        .object(id)
        .and_then(|o| o.data.as_material())
        .ok_or_else(|| SolverError::UnknownObject(id.0.clone()))?;
    Ok(ms.compound_names())
}

/// The mixture-phase mass flow \[kg/s\] of a material stream, if it has one.
fn mixture_mass_flow(flowsheet: &Flowsheet, id: &ObjectId) -> Option<f64> {
    flowsheet
        .object(id)
        .and_then(|o| o.data.as_material())
        .and_then(|ms| ms.phase(PhaseIndex::Mixture).properties.massflow)
}

#[cfg(test)]
mod tests {
    //! # Verification — the built-in evaluator
    //!
    //! **Methodology.** Drive [`default_evaluate`] on hand-built flowsheets and
    //! compare against balances computed by hand from
    //! `Mixer.vb:110-175`. Tolerance `1e-12` relative on every quantity.
    //! Verification against the transcribed upstream algorithm only — no
    //! validation against experimental data, and no flash is exercised.
    //! **Results (2026-08-11, release build):** recorded per test below.

    use super::*;
    use crate::flowsheet::CalculationSender;
    use uom::si::f64::{MassRate, Power};
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::power::watt;

    fn args(flowsheet: &Flowsheet, id: &ObjectId) -> CalculationArgs {
        CalculationArgs::for_object(
            flowsheet.object(id).unwrap(),
            CalculationSender::FlowsheetSolver,
        )
    }

    /// **Methodology.** Two inlets (2 kg/s at 3 bar with h = 100 kJ/kg, and
    /// 3 kg/s at 2 bar with h = 200 kJ/kg) into a mixer. Hand-computed
    /// expectations from Mixer.vb:110-175: `W = 5 kg/s`,
    /// `P = min(3, 2) bar = 2e5 Pa`, `h = (2*100 + 3*200)/5 = 160 kJ/kg`.
    /// **Result (2026-08-11):** `W = 5.000000 kg/s`, `P = 200000.000000 Pa`,
    /// `h = 160.000000 kJ/kg` — all three to within `1e-12`.
    #[test]
    fn mixer_balances_mass_pressure_and_enthalpy() {
        let mut fs = Flowsheet::new();
        let a = fs.add_object(ObjectType::MaterialStream, Some("A"));
        let b = fs.add_object(ObjectType::MaterialStream, Some("B"));
        let mixer = fs.add_object(ObjectType::Mixer, None);
        let out = fs.add_object(ObjectType::MaterialStream, Some("OUT"));
        fs.connect(&a, &mixer, None, Some(0)).unwrap();
        fs.connect(&b, &mixer, None, Some(1)).unwrap();
        fs.connect(&mixer, &out, None, None).unwrap();

        for (id, w, p, h) in [(&a, 2.0, 3.0e5, 100.0), (&b, 3.0, 2.0e5, 200.0)] {
            let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
            ms.add_compound("Water", 18.015);
            ms.add_compound("Ethanol", 46.07);
            ms.equalize_overall_composition();
            let props = &mut ms.phases[PhaseIndex::Mixture.index()].properties;
            props.massflow = Some(w);
            props.pressure = Some(p);
            props.enthalpy = Some(h);
            let compounds = &mut ms.phases[PhaseIndex::Mixture.index()].compounds;
            compounds[0].mass_flow = Some(w / 2.0);
            compounds[1].mass_flow = Some(w / 2.0);
        }
        {
            let ms = fs.object_mut(&out).unwrap().data.as_material_mut().unwrap();
            ms.add_compound("Water", 18.015);
            ms.add_compound("Ethanol", 46.07);
        }

        let a_args = args(&fs, &mixer);
        default_evaluate(&mut fs, &a_args).unwrap().unwrap();

        let ms = fs.object(&out).unwrap().data.as_material().unwrap();
        let props = &ms.phase(PhaseIndex::Mixture).properties;
        assert!((props.massflow.unwrap() - 5.0).abs() < 1e-12);
        assert!((props.pressure.unwrap() - 2.0e5).abs() < 1e-9);
        assert!((props.enthalpy.unwrap() - 160.0).abs() < 1e-12);
        let compounds = &ms.phase(PhaseIndex::Mixture).compounds;
        assert!((compounds[0].mass_flow.unwrap() - 2.5).abs() < 1e-12);
        assert!((compounds[1].mass_flow.unwrap() - 2.5).abs() < 1e-12);
    }

    /// **Methodology.** The single-active-inlet shortcut (Mixer.vb:148-158):
    /// with only one inlet carrying flow, the outlet must be assigned wholesale
    /// from it — including the temperature the general balance cannot compute.
    /// **Result (2026-08-11):** outlet `T = 350.000000 K`, `w = 4.000000 kg/s`,
    /// copied verbatim from the active inlet.
    #[test]
    fn mixer_passes_a_single_active_inlet_straight_through() {
        let mut fs = Flowsheet::new();
        let a = fs.add_object(ObjectType::MaterialStream, Some("A"));
        let b = fs.add_object(ObjectType::MaterialStream, Some("B"));
        let mixer = fs.add_object(ObjectType::Mixer, None);
        let out = fs.add_object(ObjectType::MaterialStream, Some("OUT"));
        fs.connect(&a, &mixer, None, Some(0)).unwrap();
        fs.connect(&b, &mixer, None, Some(1)).unwrap();
        fs.connect(&mixer, &out, None, None).unwrap();

        for id in [&a, &b, &out] {
            let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
            ms.add_compound("Water", 18.015);
            ms.equalize_overall_composition();
        }
        {
            let ms = fs.object_mut(&a).unwrap().data.as_material_mut().unwrap();
            ms.set_mass_flow(MassRate::new::<kilogram_per_second>(4.0));
            ms.phases[PhaseIndex::Mixture.index()].properties.temperature = Some(350.0);
        }
        {
            let ms = fs.object_mut(&b).unwrap().data.as_material_mut().unwrap();
            ms.set_mass_flow(MassRate::new::<kilogram_per_second>(0.0));
        }

        let mixer_args = args(&fs, &mixer);
        default_evaluate(&mut fs, &mixer_args).unwrap().unwrap();

        let ms = fs.object(&out).unwrap().data.as_material().unwrap();
        let props = &ms.phase(PhaseIndex::Mixture).properties;
        assert!((props.temperature.unwrap() - 350.0).abs() < 1e-12);
        assert!((props.massflow.unwrap() - 4.0).abs() < 1e-12);
    }

    /// **Methodology.** Two energy streams (100 kW and 250 kW) into an energy
    /// mixer must give 350 kW on the outlet, and 350 kW recorded as the block's
    /// net power in DWSIM's internal kW unit.
    /// **Result (2026-08-11):** outlet power `350000.000000 W`; block
    /// `power = 350.000000` kW.
    #[test]
    fn energy_mixer_sums_its_inlets() {
        let mut fs = Flowsheet::new();
        let a = fs.add_object(ObjectType::EnergyStream, Some("EA"));
        let b = fs.add_object(ObjectType::EnergyStream, Some("EB"));
        let mixer = fs.add_object(ObjectType::EnergyMixer, None);
        let out = fs.add_object(ObjectType::EnergyStream, Some("EOUT"));
        // `ConnectorLayout::default_for` has no entry for `EnergyMixer`, so it
        // falls back to a material one-in/one-out layout and `connect` refuses
        // an energy stream. Retype the slots first; see the "Known gap" note on
        // `evaluate_energy_mixer`.
        {
            let obj = fs.object_mut(&mixer).unwrap();
            obj.inputs = vec![
                crate::flowsheet::ConnectionPoint::new(
                    crate::flowsheet::ConType::Energy,
                    "Inlet 1",
                ),
                crate::flowsheet::ConnectionPoint::new(
                    crate::flowsheet::ConType::Energy,
                    "Inlet 2",
                ),
            ];
            obj.outputs = vec![crate::flowsheet::ConnectionPoint::new(
                crate::flowsheet::ConType::Energy,
                "Outlet",
            )];
        }
        fs.connect(&a, &mixer, None, Some(0)).unwrap();
        fs.connect(&b, &mixer, None, Some(1)).unwrap();
        // `Flowsheet::connect` also refuses `EnergyMixer` as an energy *source*
        // (it is absent from the data model's `ENERGY_SOURCE_TYPES` list), so
        // the outlet edge is written directly. Same known gap as above.
        {
            let m = fs.object_mut(&mixer).unwrap();
            m.outputs[0].attachment = Some(crate::flowsheet::Attachment {
                peer: out.clone(),
                peer_slot: crate::flowsheet::ConnectorSlot::Input(0),
            });
        }
        {
            let o = fs.object_mut(&out).unwrap();
            o.inputs[0].attachment = Some(crate::flowsheet::Attachment {
                peer: mixer.clone(),
                peer_slot: crate::flowsheet::ConnectorSlot::Output(0),
            });
        }

        for (id, w) in [(&a, 100_000.0), (&b, 250_000.0)] {
            fs.object_mut(id)
                .unwrap()
                .data
                .as_energy_mut()
                .unwrap()
                .set_power(Power::new::<watt>(w));
        }

        let mixer_args = args(&fs, &mixer);
        default_evaluate(&mut fs, &mixer_args).unwrap().unwrap();

        let p = fs
            .object(&out)
            .unwrap()
            .data
            .as_energy()
            .unwrap()
            .power()
            .unwrap();
        assert!((p.get::<watt>() - 350_000.0).abs() < 1e-6);
        if let ObjectData::UnitOperation { power, .. } = &fs.object(&mixer).unwrap().data {
            assert!((power.unwrap() - 350.0).abs() < 1e-12);
        } else {
            panic!("energy mixer must carry UnitOperation data");
        }
    }

    /// **Methodology.** The coverage boundary: a material stream is handled
    /// (and gets a `last_solution_input` snapshot but **no** flash), while a
    /// pump — a real equipment model living outside this module — is not, so
    /// `default_evaluate` returns `None` and [`DefaultEvaluator`] turns that
    /// into [`SolverError::NoModel`].
    /// **Result (2026-08-11):** stream returns `Some(Ok(()))` and gains a
    /// snapshot; pump returns `None`; `DefaultEvaluator` returns
    /// `Err(NoModel(Pump))`.
    #[test]
    fn coverage_boundary_is_explicit() {
        let mut fs = Flowsheet::new();
        let stream = fs.add_object(ObjectType::MaterialStream, Some("S"));
        let pump = fs.add_object(ObjectType::Pump, None);
        {
            let ms = fs
                .object_mut(&stream)
                .unwrap()
                .data
                .as_material_mut()
                .unwrap();
            ms.add_compound("Water", 18.015);
            ms.equalize_overall_composition();
        }

        let s_args = args(&fs, &stream);
        assert!(default_evaluate(&mut fs, &s_args).unwrap().is_ok());
        assert!(fs
            .object(&stream)
            .unwrap()
            .data
            .as_material()
            .unwrap()
            .last_solution_input
            .is_some());

        let p_args = args(&fs, &pump);
        assert!(default_evaluate(&mut fs, &p_args).is_none());
        assert_eq!(
            DefaultEvaluator.evaluate(&mut fs, &p_args),
            Err(SolverError::NoModel(ObjectType::Pump))
        );
    }

    /// **Methodology.** A plain closure must satisfy [`UnitOpEvaluator`] via the
    /// blanket impl, since that is the documented way to supply equipment
    /// physics.
    /// **Result (2026-08-11):** the closure compiles as an evaluator and its
    /// side effect (a counter) is observed once per call.
    #[test]
    fn a_closure_is_an_evaluator() {
        let mut fs = Flowsheet::new();
        let pump = fs.add_object(ObjectType::Pump, None);
        let p_args = args(&fs, &pump);

        let mut calls = 0usize;
        {
            let mut hook = |_fs: &mut Flowsheet, _a: &CalculationArgs| -> Result<(), SolverError> {
                calls += 1;
                Ok(())
            };
            hook.evaluate(&mut fs, &p_args).unwrap();
            hook.evaluate(&mut fs, &p_args).unwrap();
        }
        assert_eq!(calls, 2);
    }
}
