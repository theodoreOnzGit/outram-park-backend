//! Flowsheet data model — the object registry, connection topology, stream
//! state, calculation queue, and results report of a process flowsheet.
//!
//! # What this module is
//!
//! A **flowsheet** is the description of a process: which unit operations exist,
//! which streams join them, and what state each stream carries. This module is
//! the *data model* for that description — the passive structure a solver reads
//! and writes. It computes no thermodynamics and runs no iteration; the
//! execution engine is [`crate::flowsheet_solver`] and the physics is
//! [`crate::thermo`] plus the equipment modules ([`crate::pump`],
//! [`crate::heat_exchanger`], ...).
//!
//! Start at [`graph::Flowsheet`]. It owns everything else:
//!
//! ```text
//!   Flowsheet                          (graph.rs)
//!    +- HashMap<ObjectId, FlowsheetObject>   (objects.rs)
//!    |     +- inputs / outputs / energy_connector: ConnectionPoint  (connectors.rs)
//!    |     +- data: ObjectData
//!    |           +- Material(MaterialStreamData)   (streams.rs)
//!    |           +- Energy(EnergyStreamData)       (streams.rs)
//!    |           +- UnitOperation { power, results }
//!    +- calculation_queue: CalculationQueue   (queue.rs)
//!    +- results: FlowsheetResults             (graph.rs)
//! ```
//!
//! and [`report`] renders a whole flowsheet as a plain-text results table.
//!
//! # The structural invariant
//!
//! Every connection joins **exactly one stream to exactly one unit operation**.
//! Streams are nodes, not edges, so a material stream can never connect straight
//! to another material stream and two unit operations can never connect
//! directly. [`graph::Flowsheet::connect`] enforces this, and it is what lets a
//! solver treat any stream as a state carrier it may read, overwrite, or tear.
//!
//! # Units
//!
//! Public accessors are `uom`-typed and plain SI (K, Pa, kg/s, mol/s, m³/s,
//! J/kg, W). The **stored** `f64` fields keep DWSIM's internal units verbatim so
//! ported algorithms stay comparable to their source, and DWSIM's internal
//! system is SI-with-kilo for energy: **enthalpy kJ/kg, entropy kJ/(kg·K),
//! energy flow kW, molecular weight kg/kmol**. Every field says so in its own
//! doc comment, and [`streams`] explains the boundary in full.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2024 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Primary sources, by submodule:
//!
//! | Submodule | Upstream source |
//! |---|---|
//! | [`objects`] | `DWSIM.Interfaces/Enums.vb` (`ObjectType`, `SimulationObjectClass`); `DWSIM.FlowsheetBase/FlowsheetBase.vb` (`AddObject`, the tag-prefix table, the spec/adjust attachment fields) |
//! | [`connectors`] | `DWSIM.Interfaces/Enums.vb` (`ConType`); `DWSIM.Drawing.SkiaSharp/GraphicObjects/Connector/ConnectorClass.vb`; each shape's `CreateConnectors` |
//! | [`streams`] | `DWSIM.Thermodynamics/MaterialStream/MaterialStream.vb`; `DWSIM.Thermodynamics/BaseClasses/ThermodynamicsBase.vb`; `DWSIM.UnitOperations/EnergyStream/Streams.vb`; `DWSIM.Interfaces/Enums.vb` (`StreamSpec` and friends) |
//! | [`graph`] | `DWSIM.FlowsheetBase/FlowsheetBase.vb`; `DWSIM.Drawing.SkiaSharp/GraphicsSurface/DesignSurface.vb` (`ConnectObject` / `DisconnectObject`) |
//! | [`queue`] | `DWSIM.FlowsheetSolver/ObjectInfo.vb`; `FlowsheetBase.vb` (`CalculationQueue`, `RequestCalculation`) |
//! | [`report`] | `DWSIM.FlowsheetBase/ReportCreator.vb` |
//!
//! Each submodule's header cites the exact line ranges it ports.
//!
//! # Excluded DWSIM behavior
//!
//! Each submodule documents its own exclusions in detail; the workspace-level
//! summary is:
//!
//! - **GUI and drawing.** WinForms/Eto editors, the SkiaSharp drawing surface,
//!   all geometry (`X`, `Y`, `Width`, `Height`, connector positions), icons,
//!   zoom, selection, `Draw`/`HitTest`, and every `MustOverride` UI hook
//!   (`DisplayForm`, `ShowMessage`, `UpdateInterface`, `RunCodeOnUIThread`, ...).
//!   One consequence is called out where it bites: DWSIM's
//!   `AddConnectedObjects` picks streams to auto-attach by *screen distance*
//!   (FlowsheetBase.vb:1848-1867), so only its "create fresh streams" branch
//!   survives as [`graph::Flowsheet::add_connected_streams`].
//! - **XML/JSON serialization** — `SaveToXML`/`LoadFromXML`/`SaveToMXML`/
//!   `LoadFromMXML`/`CloneXML`/`SaveData`/`LoadData`/`GetProcessData`/
//!   `LoadProcessData`/`LoadZippedXML`, plus the snapshot and undo/redo stacks
//!   built on them (`RegisterSnapshot`, `GetSnapshot`, `RestoreSnapshot`,
//!   `ProcessUndo`, `ProcessRedo`).
//! - **Property-grid reflection** — `GetPropertyValue`, `GetProperties`,
//!   `SetPropertyValue`, `GetPropertyUnit`, and the display-unit
//!   (`IUnitsOfMeasure`) conversion layer they feed. This model is SI-internal
//!   only; [`report`] states the property list it reports explicitly instead of
//!   slicing a reflected array at magic indices.
//! - **Scripting** — IronPython / Python.NET integration (`RunScript`,
//!   `ProcessScripts`, `PythonPreprocessor`).
//! - **.NET host plumbing** — assembly reflection in `Initialize()`, the
//!   resource-manager translation layer (`GetTranslatedString`), the weather
//!   provider, the spreadsheet bridge, CAPE-OPEN COM interfaces, extenders, and
//!   `Task.Factory.StartNew` scheduling.
//! - **Compound databases** — ChemSep/CoolProp/ChEDL/Biodiesel/Electrolyte
//!   loading. Compound constants are [`crate::thermo::component::Component`]'s
//!   concern; a stream carries only the molar mass its composition algebra
//!   needs.
//! - **The stream property calculator** — `MaterialStream.Calculate` and its
//!   property-package plumbing. That is the flash boundary: this module stores
//!   the state and the specification, [`crate::thermo`] computes it.
//! - **Solver execution** — `SolveFlowsheet` and the `RequestCalculation*`
//!   wrappers. Only the *queueing* half is here ([`queue`]); draining the queue
//!   is [`crate::flowsheet_solver`]'s job.
//! - **Economics and GHG results** — six of the eight identifiers
//!   [`graph::FlowsheetResults::result_ids`] lists are not computed by this
//!   port and honestly report `f64::NAN` rather than a fabricated value.
//!
//! # Honest scope
//!
//! This is **AI-assisted draft material and has had no human V&V**. The tests in
//! each submodule are *verification* against the transcribed upstream logic and
//! hand-computed arithmetic — they check "did we port it correctly?", not "does
//! it represent physical reality?". No benchmark flowsheet has been run. Per the
//! workspace `RESPONSIBLE_USE.md`, treat it as untrusted until reviewed. Not for
//! nuclear facility operation, reactor control, safety-critical decisions, or
//! licensing.
//!
//! # Known gaps
//!
//! - **`ConnectorLayout::default_for` is complete only for the object types
//!   whose upstream shape classes were transcribed** (the table on that
//!   function). Everything else — drawing annotations, gauges, controllers,
//!   clean-power sources, plug-in wrappers — falls back to a documented
//!   one-inlet/one-outlet default that is a port-side approximation, not an
//!   upstream fact.
//! - **Object identities are a deterministic counter, not GUIDs**, because this
//!   crate takes no UUID dependency (Android/Termux portability). See
//!   [`graph::Flowsheet::add_object`]; use
//!   [`graph::Flowsheet::add_object_with_id`] to preserve a DWSIM GUID.
//! - **`ObjectData::UnitOperation` carries no equipment state** beyond a net
//!   power and a free-form results map. Equipment parameters live in this
//!   crate's own equipment modules; wiring them into the registry is the
//!   solver workstream's decision, not this module's.

pub mod connectors;
pub mod graph;
/// Read-only importer for DWSIM's saved `.dwxml` / `.dwxmz` reference files.
pub mod import;
pub mod objects;
pub mod queue;
pub mod report;
pub mod streams;

pub use connectors::{Attachment, ConType, ConnectionPoint, ConnectorLayout, ConnectorSlot};
pub use graph::{Connection, ConnectionError, Flowsheet, FlowsheetError, FlowsheetResults};
pub use objects::{
    available_object_type_names, FlowsheetObject, ObjectData, ObjectId, ObjectType,
    SimulationObjectClass,
};
pub use queue::{CalculationArgs, CalculationQueue, CalculationSender};
pub use report::{build_report_rows, render_text_report, ReportOptions, ReportRow};
pub use streams::{
    CompositionBasis, EnergyStreamData, FlowSpec, ForcedPhase, MaterialStreamData,
    MaterialStreamInputData, MolarFlowRate, PhaseData, PhaseIndex, PhaseLabel, PhaseName,
    PhaseProperties, StreamCompound, StreamSpec, StreamValidationError,
};

#[cfg(test)]
mod tests {
    //! # Integration verification — a small flowsheet end to end
    //!
    //! **Methodology.** Build a three-object flowsheet (feed stream -> heater
    //! -> product stream, plus an energy stream supplying the heater's duty)
    //! through the public API only, then check the topology, the balances
    //! (`UpdateMassAndEnergyBalance`, FlowsheetBase.vb:5412) and the report
    //! against hand-computed values. This exercises the seams between the
    //! submodules, which their own unit tests do not. Verification only — no
    //! thermodynamics is evaluated. Results recorded 2026-08-11, release build.

    use super::*;
    use approx::assert_relative_eq;
    use uom::si::f64::{MassRate, Power};
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::power::watt;

    /// **Methodology.** Wire feed(2 kg/s) -> heater -> product(2 kg/s) with an
    /// energy stream on the heater's energy inlet, give the heater
    /// `power = +150 kW`, and check: 3 connections; the energy stream lands on
    /// the heater's `Input(1)` (its `ConEn` slot); one feed and one product
    /// stream; residual mass balance `2 - 2 = 0 kg/s`; total energy balance
    /// `150 kW`; and the report contains all four objects.
    /// **Result (2026-08-11):** 3 connections; energy stream on `Input(1)`;
    /// residual 0.000000 kg/s (0.000000 kg/s via `uom`); energy 150.000000 kW
    /// (150000.000000 W via `uom`); report lists FEED, HT-1, E1 and PROD.
    #[test]
    fn small_flowsheet_end_to_end() {
        let mut fs = Flowsheet::new();

        let feed = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let heater = fs.add_object(ObjectType::Heater, None);
        let product = fs.add_object(ObjectType::MaterialStream, Some("PROD"));
        let duty = fs.add_object(ObjectType::EnergyStream, None);

        fs.connect(&feed, &heater, None, None).unwrap();
        fs.connect(&heater, &product, None, None).unwrap();
        let c = fs.connect(&duty, &heater, None, None).unwrap();
        assert_eq!(
            c.to_slot,
            ConnectorSlot::Input(1),
            "energy stream must take the heater's ConEn slot"
        );
        assert_eq!(fs.connections().len(), 3);

        for (id, w) in [(&feed, 2.0), (&product, 2.0)] {
            let ms = fs.object_mut(id).unwrap().data.as_material_mut().unwrap();
            ms.add_compound("Water", 18.015);
            ms.equalize_overall_composition();
            ms.set_mass_flow(MassRate::new::<kilogram_per_second>(w));
        }
        fs.object_mut(&duty)
            .unwrap()
            .data
            .as_energy_mut()
            .unwrap()
            .set_power(Power::new::<watt>(150_000.0));
        if let ObjectData::UnitOperation { power, .. } = &mut fs.object_mut(&heater).unwrap().data {
            *power = Some(150.0);
        }

        assert_eq!(fs.feed_streams().len(), 1);
        assert_eq!(fs.product_streams().len(), 1);

        let r = fs.update_mass_and_energy_balance();
        assert_relative_eq!(r.residual_mass_balance, 0.0, epsilon = 1e-12);
        assert_relative_eq!(r.total_energy_balance, 150.0, epsilon = 1e-12);
        assert_relative_eq!(
            r.total_energy_balance_power().get::<watt>(),
            150_000.0,
            epsilon = 1e-9
        );

        let text = render_text_report(&fs, &ReportOptions::default());
        for tag in ["FEED", "HT-1", "E1", "PROD"] {
            assert!(
                text.contains(&format!("Object: {tag}")),
                "report is missing {tag}\n{text}"
            );
        }
        assert!(text.contains("Energy Flow"));
        assert!(text.contains("Power Generated/Consumed"));
    }

    /// **Methodology.** Removing the heater from that flowsheet must leave every
    /// stream isolated and the graph edge-free (`DeleteSelectedObject`,
    /// FlowsheetBase.vb:175-290), and the calculation queue must survive as an
    /// independent structure.
    /// **Result (2026-08-11):** 0 connections after removal; the 2 streams
    /// remain; the queue still reports its single pending request.
    #[test]
    fn removing_a_unit_operation_leaves_the_streams_isolated() {
        let mut fs = Flowsheet::new();
        let feed = fs.add_object(ObjectType::MaterialStream, None);
        let heater = fs.add_object(ObjectType::Heater, None);
        let product = fs.add_object(ObjectType::MaterialStream, None);
        fs.connect(&feed, &heater, None, None).unwrap();
        fs.connect(&heater, &product, None, None).unwrap();

        let obj = fs.object(&heater).unwrap().clone();
        fs.calculation_queue
            .request_calculation(&obj, CalculationSender::PropertyGrid);

        fs.remove_object(&heater).unwrap();
        assert_eq!(fs.len(), 2, "the two streams survive the heater's removal");
        assert!(fs.connections().is_empty());
        assert!(fs.object(&feed).unwrap().is_isolated());
        assert!(fs.object(&product).unwrap().is_isolated());
        assert_eq!(fs.calculation_queue.len(), 1);
    }
}
