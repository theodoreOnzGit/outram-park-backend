//! Connector data model: the typed inlet/outlet/energy slots a flowsheet object
//! exposes, what is currently attached to each, and the per-object-type default
//! slot layout.
//!
//! # What this represents physically
//!
//! A *connection point* is one port on a piece of equipment: the suction nozzle
//! of a pump, the shell-side inlet of a heat exchanger, the reboiler-duty
//! terminal of a distillation column. Each port has a **kind**
//! ([`ConType`]) — process inlet, process outlet, or energy terminal — and is
//! either free or attached to exactly one peer port on another object. Nothing
//! here is a physical *quantity*, so no `uom` types appear: this module is pure
//! topology.
//!
//! Slots are **index-addressed** and their order is meaningful — `input(0)` of a
//! heat exchanger is "Inlet Stream 1", `output(1)` of a distillation column is
//! "Bottoms". The connection API takes those indices, exactly as DWSIM's
//! `fidx`/`tidx` arguments do.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2024 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, not
//! the official DWSIM software.
//!
//! Source regions ported here:
//!
//! - `DWSIM.Interfaces/Enums.vb` lines 654-661 — the `ConType` enumeration
//!   (`ConIn = -1`, `ConOut = 1`, `ConEn = 0`, `ConSp = 2`).
//! - `DWSIM.Drawing.SkiaSharp/GraphicObjects/Connector/ConnectorClass.vb`
//!   lines 10-47 — the `ConnectionPoint` class: `Type`, `ConnectorName`,
//!   `IsAttached`, `AttachedConnector`, `Active`.
//! - `DWSIM.Drawing.SkiaSharp/GraphicObjects/Shapes/*.vb`, each shape's
//!   `CreateConnectors(InCount, OutCount)` — the per-object-type slot counts,
//!   slot kinds, and connector names, collected into [`ConnectorLayout::default_for`].
//!   Specific files and their slot counts are cited on that function.
//!
//! # Excluded DWSIM behavior
//!
//! - **All geometry.** `ConnectionPoint.Position`/`X`/`Y`/`Direction`
//!   (ConnectorClass.vb:18, :37-41) place the port on a drawing canvas; they are
//!   not topology and are not ported. Likewise every `CreateConnectors` body's
//!   `New Point(X + 0.5 * Width, ...)` arithmetic is dropped — only the *number*,
//!   *kind*, and *name* of the slots survive.
//! - **`ConnectorGraphic`** (ConnectorClass.vb:49+) — the drawn poly-line
//!   between two ports, its `SKPath`, and its hit-testing. This port stores the
//!   attachment directly on both endpoint slots instead, so no connector object
//!   is needed.
//! - **`ConType::ConSp`** is kept as a variant for round-tripping but is never
//!   produced by any ported layout: DWSIM uses it only for the drawing-surface
//!   "spec" attachment glyph, and the spec/adjust attachment itself is modelled
//!   as [`crate::flowsheet::objects::FlowsheetObject::attached_spec`] instead.
//! - **`ConnectionPoint.Active`** is recorded (as
//!   [`ConnectorLayout::energy_connector_active`]) but, faithfully to upstream,
//!   is **not** consulted when connecting: DWSIM's `ConnectObject`
//!   (DesignSurface.vb:1399) tests only `IsAttached`, never `Active`. See the
//!   note on [`ConnectorLayout::energy_connector_active`].

use crate::flowsheet::objects::{ObjectId, ObjectType};

/// Kind of a connection point — DWSIM's `ConType` (Enums.vb:654-661).
///
/// The kind is a hard constraint on what may attach: a material stream may only
/// land in a [`ConType::In`] slot, an energy stream only in a [`ConType::Energy`]
/// slot (DesignSurface.vb:1324-1361).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConType {
    /// Process inlet (upstream `ConIn = -1`): accepts a material stream.
    In,
    /// Process outlet (upstream `ConOut = 1`): feeds a material stream.
    Out,
    /// Energy terminal (upstream `ConEn = 0`): accepts or supplies an energy
    /// stream (a duty/work link, kW).
    Energy,
    /// Specification attachment glyph (upstream `ConSp = 2`). Kept for
    /// round-tripping; never produced by this port — see the module's
    /// "Excluded DWSIM behavior".
    Spec,
}

/// Which slot on an object a connection refers to.
///
/// Replaces DWSIM's pair of `AttachedFromInput`/`AttachedToOutput` booleans plus
/// an index (`ConnectorGraphic.AttachedFromConnectorIndex`,
/// DesignSurface.vb:1471-1488) with one exhaustive enum, so an illegal
/// combination cannot be represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectorSlot {
    /// The `i`-th inlet slot (`InputConnectors(i)`).
    Input(usize),
    /// The `i`-th outlet slot (`OutputConnectors(i)`).
    Output(usize),
    /// The single dedicated energy connector (`EnergyConnector`).
    Energy,
}

/// What a connection point is attached to: the peer object and the peer's slot.
///
/// DWSIM stores an `AttachedConnector` graphic on both endpoints and reads the
/// peer off it (`AttachedFrom` / `AttachedTo`). This port stores the peer
/// identity directly, by [`ObjectId`] — never by reference, per the workspace
/// no-lifetimes rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// The object on the other end of this connection.
    pub peer: ObjectId,
    /// Which slot on `peer` this connection lands in.
    pub peer_slot: ConnectorSlot,
}

/// One port on a flowsheet object — DWSIM's `ConnectionPoint`
/// (ConnectorClass.vb:10-47), stripped of geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionPoint {
    /// Which kind of stream this port accepts.
    pub connector_type: ConType,
    /// Human-readable port name, e.g. `"Inlet Stream 1"`, `"Reboiler Duty"`.
    /// Taken verbatim from the upstream shape classes.
    pub connector_name: String,
    /// `Some(..)` when attached; `None` when free. Equivalent to DWSIM's
    /// `IsAttached` + `AttachedConnector` pair, with the invalid "attached but
    /// no connector" state made unrepresentable (upstream defends against it at
    /// ConnectorClass.vb:22-28).
    pub attachment: Option<Attachment>,
}

impl ConnectionPoint {
    /// A free port of the given kind and name.
    #[must_use]
    pub fn new(connector_type: ConType, connector_name: impl Into<String>) -> Self {
        ConnectionPoint {
            connector_type,
            connector_name: connector_name.into(),
            attachment: None,
        }
    }

    /// Whether this port currently has a peer (DWSIM's `IsAttached`).
    #[must_use]
    pub fn is_attached(&self) -> bool {
        self.attachment.is_some()
    }

    /// The peer object, if attached.
    #[must_use]
    pub fn peer(&self) -> Option<&ObjectId> {
        self.attachment.as_ref().map(|a| &a.peer)
    }
}

/// The default connector slot layout for an object type.
///
/// Slot order is significant and matches the upstream index order exactly, so a
/// DWSIM connection recorded as `(fidx, tidx)` means the same thing here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorLayout {
    /// Inlet slots in index order.
    pub inputs: Vec<ConnectionPoint>,
    /// Outlet slots in index order.
    pub outputs: Vec<ConnectionPoint>,
    /// Whether the object's dedicated energy connector is marked active
    /// (`EnergyConnector.Active`, default `True` in ConnectorClass.vb:43).
    ///
    /// **Recorded, not enforced.** DWSIM's `ConnectObject` never tests `Active`
    /// — it tests only `IsAttached` (DesignSurface.vb:1399) — so a shape that
    /// sets `Active = False` can still have its energy connector wired. This
    /// port reproduces that behaviour rather than "fixing" it, and exposes the
    /// flag so a caller (or a future GUI) can honour it if it wants to.
    pub energy_connector_active: bool,
}

impl ConnectorLayout {
    /// The default slot layout DWSIM's shape class creates for `object_type`.
    ///
    /// Transcribed from each shape's `CreateConnectors(InCount, OutCount)` in
    /// `DWSIM.Drawing.SkiaSharp/GraphicObjects/Shapes/`:
    ///
    /// | Type | Source file | In | Out | Energy connector |
    /// |---|---|---|---|---|
    /// | `MaterialStream` | `MaterialStream.vb:50-84` | 1 | 1 | inactive |
    /// | `EnergyStream` | `EnergyStream.vb` | 1 | 1 | active (unused) |
    /// | `NodeIn` / `Mixer` | `Mixer.vb:49-120` | 6 | 1 | inactive |
    /// | `NodeOut` / `Splitter` | `Splitter.vb` | 1 | 3 | inactive |
    /// | `Pump` | `Pump.vb` | 2 (1 energy) | 1 | inactive |
    /// | `Compressor` | `Compressor.vb` | 2 (1 energy) | 1 | inactive |
    /// | `Expander` | `Expander.vb` | 1 | 1 | active |
    /// | `Heater` | `Heater.vb` | 2 (1 energy) | 1 | active |
    /// | `Cooler` | `Cooler.vb` | 2 (1 energy) | 1 | active |
    /// | `Pipe` | `PipeSegment.vb` | 1 | 1 | active |
    /// | `Valve` / `Tank` / `OrificePlate` / recycles | resp. `.vb` | 1 | 1 | inactive |
    /// | `Vessel` | `SeparatorVessel.vb` | 7 (1 energy) | 4 | inactive |
    /// | `HeatExchanger` | `HeatExchanger.vb:16` `CreateConnectors(2, 2)` | 2 | 2 | inactive |
    /// | `ShortcutColumn` | `ShortcutColumn.vb:15` `CreateConnectors(2, 2)` | 2 (1 energy) | 2 | active |
    /// | `DistillationColumn` | `RigorousColumn.vb:15` `CreateConnectors(11, 11)` | 11 (1 energy) | 11 (1 energy) | inactive |
    /// | `AbsorptionColumn` | `AbsorptionColumn.vb:15` `CreateConnectors(10, 10)` | 10 | 10 | inactive |
    /// | `ComponentSeparator` / `SolidSeparator` / `Filter` | resp. `.vb` | 1 | 2 | see table below |
    /// | `RctConversion` | `ConversionReactor.vb` | 2 (1 energy) | 3 (1 energy) | active |
    /// | `RctEquilibrium` / `RctGibbs` | resp. `.vb` | 2 (1 energy) | 2 | active |
    /// | `RctCstr` | `CSTR.vb` | 2 (1 energy) | 2 | inactive |
    /// | `RctPfr` | `PFR.vb` | 2 (1 energy) | 1 | inactive |
    /// | `OtAdjust` / `OtSpec` | `Adjust.vb` / `Spec.vb` | 0 | 0 | inactive |
    ///
    /// Types not in that table (drawing annotations, gauges, controllers, the
    /// clean-power sources, and the plug-in wrappers) get a **port-side
    /// default** of one inlet plus one outlet with an inactive energy connector,
    /// because their upstream shapes were not transcribed. That is a documented
    /// approximation, not an upstream fact — do not rely on it for those types.
    #[must_use]
    pub fn default_for(object_type: ObjectType) -> ConnectorLayout {
        use ConType::{Energy, In, Out};
        use ObjectType as T;

        // Small helpers keep the table below readable.
        let inlet = |name: &str| ConnectionPoint::new(In, name);
        let outlet = |name: &str| ConnectionPoint::new(Out, name);
        let energy = |name: &str| ConnectionPoint::new(Energy, name);

        let (inputs, outputs, energy_active): (Vec<_>, Vec<_>, bool) = match object_type {
            T::MaterialStream => (vec![inlet("Inlet")], vec![outlet("Outlet")], false),
            T::EnergyStream => (vec![inlet("Inlet")], vec![outlet("Outlet")], true),
            T::NodeIn | T::Mixer => (
                (1..=6)
                    .map(|i| inlet(&format!("Inlet Stream {i}")))
                    .collect(),
                vec![outlet("Outlet")],
                false,
            ),
            T::NodeOut | T::Splitter => (
                vec![inlet("Inlet")],
                (1..=3).map(|i| outlet(&format!("Outlet {i}"))).collect(),
                false,
            ),
            T::Pump | T::Compressor => (
                vec![inlet("Inlet"), energy("Energy Stream")],
                vec![outlet("Outlet")],
                false,
            ),
            T::Expander => (vec![inlet("Inlet")], vec![outlet("Outlet")], true),
            T::Heater | T::Cooler | T::HeaterCooler => (
                vec![inlet("Inlet"), energy("Energy Stream (Secondary)")],
                vec![outlet("Outlet")],
                true,
            ),
            T::CompressorExpander => (
                vec![inlet("Inlet"), energy("Energy Stream")],
                vec![outlet("Outlet")],
                false,
            ),
            T::Pipe => (vec![inlet("Inlet")], vec![outlet("Outlet")], true),
            T::Valve | T::Tank | T::OtRecycle => {
                (vec![inlet("Inlet")], vec![outlet("Outlet")], false)
            }
            T::OtEnergyRecycle => (vec![energy("Inlet")], vec![energy("Outlet")], false),
            T::OrificePlate => (
                vec![inlet("Inlet Stream")],
                vec![outlet("Outlet Stream")],
                false,
            ),
            T::Vessel | T::TpVessel => {
                let mut ins: Vec<_> = (0..6)
                    .map(|i| inlet(&format!("Inlet Stream #{i}")))
                    .collect();
                ins.push(energy("Energy Stream"));
                (
                    ins,
                    vec![
                        outlet("Vapor Outlet"),
                        outlet("Light Liquid Outlet"),
                        outlet("Heavy Liquid Outlet"),
                        outlet("Relief Valve Outlet"),
                    ],
                    false,
                )
            }
            T::HeatExchanger => (
                vec![inlet("Inlet Stream 1"), inlet("Inlet Stream 2")],
                vec![outlet("Outlet Stream 1"), outlet("Outlet Stream 2")],
                false,
            ),
            T::ShortcutColumn => (
                vec![inlet("Inlet"), energy("Reboiler Duty")],
                vec![outlet("Distillate"), outlet("Bottoms")],
                true,
            ),
            T::DistillationColumn | T::RefluxedAbsorber | T::ReboiledAbsorber => {
                let mut ins: Vec<_> = (1..=10)
                    .map(|i| inlet(&format!("Column Feed Port #{i}")))
                    .collect();
                ins.push(energy("Reboiler Duty"));
                let mut outs = vec![outlet("Distillate"), outlet("Bottoms")];
                for i in 1..=7 {
                    outs.push(outlet(&format!("Side Draw #{i}")));
                }
                outs.push(outlet("Overhead Vapor"));
                outs.push(energy("Condenser Duty"));
                (ins, outs, false)
            }
            T::AbsorptionColumn => {
                let ins: Vec<_> = (1..=10)
                    .map(|i| inlet(&format!("Column Feed Port #{i}")))
                    .collect();
                let mut outs = vec![outlet("Top Product"), outlet("Bottoms Product")];
                for i in 1..=8 {
                    outs.push(outlet(&format!("Side Draw #{i}")));
                }
                (ins, outs, false)
            }
            T::ComponentSeparator => (
                vec![inlet("Inlet")],
                vec![outlet("Outlet 1"), outlet("Outlet 2")],
                true,
            ),
            T::SolidSeparator => (
                vec![inlet("Inlet")],
                vec![outlet("Outlet 1"), outlet("Outlet 2")],
                false,
            ),
            T::Filter => (
                vec![inlet("Inlet")],
                vec![outlet("Filtrate"), outlet("Retentate")],
                true,
            ),
            T::RctConversion => (
                vec![inlet("Inlet"), energy("Energy Stream")],
                vec![
                    outlet("Vapor Outlet"),
                    outlet("Liquid Outlet"),
                    energy("Energy Stream"),
                ],
                true,
            ),
            T::RctEquilibrium | T::RctGibbs | T::RctGibbsReaktoro => (
                vec![inlet("Inlet"), energy("Energy Stream")],
                vec![outlet("Vapor Outlet"), outlet("Liquid Outlet")],
                true,
            ),
            T::RctCstr => (
                vec![inlet("Inlet"), energy("Energy Stream")],
                vec![outlet("Liquid Outlet"), outlet("Vapor Outlet (Optional)")],
                false,
            ),
            T::RctPfr => (
                vec![inlet("Inlet"), energy("Energy Stream")],
                vec![outlet("Outlet")],
                false,
            ),
            T::OtAdjust | T::OtSpec => (Vec::new(), Vec::new(), false),
            // Port-side default for the un-transcribed types (see the doc note).
            _ => (vec![inlet("Inlet")], vec![outlet("Outlet")], false),
        };

        ConnectorLayout {
            inputs,
            outputs,
            energy_connector_active: energy_active,
        }
    }
}

#[cfg(test)]
mod tests {
    //! # Verification tests — connector layouts
    //!
    //! **Methodology.** Verification against the transcribed upstream shape
    //! classes (does the Rust table reproduce `CreateConnectors`?), not
    //! validation against a physical benchmark — connector topology carries no
    //! physics. Results recorded 2026-08-11.

    use super::*;

    /// **Methodology.** Check the slot counts and kinds for the layouts whose
    /// upstream `CreateConnectors` bodies were read line by line.
    /// **Result (2026-08-11):** material stream 1/1; mixer 6/1; splitter 1/3;
    /// pump 2/1 with `input(1)` an energy terminal; heat exchanger 2/2;
    /// rigorous distillation column 11/11 with `input(10)` and `output(10)`
    /// energy terminals and `output(9)` = "Overhead Vapor".
    #[test]
    fn transcribed_layouts_match_upstream_counts_and_kinds() {
        let ms = ConnectorLayout::default_for(ObjectType::MaterialStream);
        assert_eq!(ms.inputs.len(), 1);
        assert_eq!(ms.outputs.len(), 1);
        assert_eq!(ms.inputs[0].connector_type, ConType::In);
        assert_eq!(ms.outputs[0].connector_type, ConType::Out);
        assert!(!ms.energy_connector_active);

        let mix = ConnectorLayout::default_for(ObjectType::NodeIn);
        assert_eq!(mix.inputs.len(), 6);
        assert_eq!(mix.outputs.len(), 1);
        assert_eq!(mix.inputs[5].connector_name, "Inlet Stream 6");

        let spl = ConnectorLayout::default_for(ObjectType::NodeOut);
        assert_eq!(spl.inputs.len(), 1);
        assert_eq!(spl.outputs.len(), 3);

        let pump = ConnectorLayout::default_for(ObjectType::Pump);
        assert_eq!(pump.inputs.len(), 2);
        assert_eq!(pump.inputs[0].connector_type, ConType::In);
        assert_eq!(pump.inputs[1].connector_type, ConType::Energy);
        assert_eq!(pump.outputs.len(), 1);

        let hx = ConnectorLayout::default_for(ObjectType::HeatExchanger);
        assert_eq!(hx.inputs.len(), 2);
        assert_eq!(hx.outputs.len(), 2);
        assert_eq!(hx.outputs[1].connector_name, "Outlet Stream 2");

        let col = ConnectorLayout::default_for(ObjectType::DistillationColumn);
        assert_eq!(
            col.inputs.len(),
            11,
            "RigorousColumn.vb:15 CreateConnectors(11, 11)"
        );
        assert_eq!(col.outputs.len(), 11);
        assert_eq!(col.inputs[10].connector_type, ConType::Energy);
        assert_eq!(col.inputs[10].connector_name, "Reboiler Duty");
        assert_eq!(col.outputs[9].connector_name, "Overhead Vapor");
        assert_eq!(col.outputs[10].connector_type, ConType::Energy);
        assert_eq!(col.outputs[10].connector_name, "Condenser Duty");

        let abs = ConnectorLayout::default_for(ObjectType::AbsorptionColumn);
        assert_eq!(
            abs.inputs.len(),
            10,
            "AbsorptionColumn.vb:15 CreateConnectors(10, 10)"
        );
        assert_eq!(abs.outputs.len(), 10);
        assert!(abs.outputs.iter().all(|c| c.connector_type == ConType::Out));

        let vessel = ConnectorLayout::default_for(ObjectType::Vessel);
        assert_eq!(vessel.inputs.len(), 7);
        assert_eq!(vessel.inputs[6].connector_type, ConType::Energy);
        assert_eq!(vessel.outputs.len(), 4);
        assert_eq!(vessel.outputs[0].connector_name, "Vapor Outlet");
    }

    /// **Methodology.** The logical adjust/spec blocks carry no stream ports at
    /// all upstream (`Adjust.vb`, `Spec.vb`: no `InputConnectors.Add`); they
    /// attach to their target objects logically instead.
    /// **Result (2026-08-11):** both layouts are empty.
    #[test]
    fn logical_blocks_have_no_stream_ports() {
        for t in [ObjectType::OtAdjust, ObjectType::OtSpec] {
            let l = ConnectorLayout::default_for(t);
            assert!(l.inputs.is_empty(), "{t:?} should have no inlets");
            assert!(l.outputs.is_empty(), "{t:?} should have no outlets");
        }
    }

    /// **Methodology.** A fresh [`ConnectionPoint`] must be free, and attaching
    /// must make `is_attached` true and expose the peer.
    /// **Result (2026-08-11):** as expected.
    #[test]
    fn connection_point_attachment_state() {
        let mut c = ConnectionPoint::new(ConType::In, "Inlet");
        assert!(!c.is_attached());
        assert_eq!(c.peer(), None);
        c.attachment = Some(Attachment {
            peer: ObjectId::from("peer-1"),
            peer_slot: ConnectorSlot::Output(0),
        });
        assert!(c.is_attached());
        assert_eq!(c.peer().unwrap().as_str(), "peer-1");
    }
}
