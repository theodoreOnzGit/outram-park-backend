//! The OPC-UA node map: the single source of truth for what the HTGR
//! demonstration simulator publishes.
//!
//! # UNIT CONVENTION — read this before anything else
//!
//! **Every value on the wire is a bare `f64` in SI base or coherent SI derived
//! units: W, K, Pa, kg/s, s, J/kg, J/(kg K), and dimensionless.** No
//! megawatts, no kilopascals, no degrees Celsius, no pcm — ever, anywhere in
//! this interface.
//!
//! That convention exists because an OPC-UA client reads a `Double` and gets no
//! unit with it: 700 °C, 700 K and 973.15 K are all just `700` or `973.15` on
//! the wire. So the unit is carried redundantly, in four places, and every one
//! of them is checked by a test in this module:
//!
//! 1. the **browse name** ends in the unit — `CoreOutletTemperatureKelvin`,
//!    `ThermalPowerWatt`, `MassFlowKilogramPerSecond`;
//! 2. the **display name** ends in the unit symbol in brackets — `"Core outlet
//!    helium temperature [K]"`;
//! 3. the **description** states it again in words;
//! 4. [`HtgrSignal::unit`] / [`HtgrControl::unit`] expose it programmatically,
//!    so a client-side table can render it without string-scraping.
//!
//! Dimensionless quantities still carry a suffix rather than none, so that a
//! missing suffix always means "someone forgot": `Ratio` for a plain
//! dimensionless ratio (steam quality, `beta_eff`) and `Dollar` for a
//! reactivity normalised by `beta` (`rho/beta`). Reactivity in dollars is
//! dimensionless, so it does not break the SI rule; multiply by
//! `DelayedNeutronFractionRatio`, which is published alongside it, to recover
//! the absolute `rho`.
//!
//! The simulator's own GUI snapshot (`examples/htgr_sim_v1/app/state.rs`) uses
//! display units — MW, kPa, MPa, pcm — and mixes kPa with MPa in the same
//! struct. The conversion to SI happens once, in
//! [`super::state`], not per node. Do **not** add a node that publishes a
//! display unit "for convenience"; that is how a twin ends up serving two
//! numbers for one pressure.
//!
//! # Model fidelity is published too
//!
//! Every read-only node carries a [`ModelFidelity`] and a
//! [`HtgrSignal::varies_during_run`] flag, and both are spelled out in the node
//! description a client sees. Three things a client must be able to tell apart:
//!
//! - values whose numerical content comes from a **real property library or
//!   published nuclear data** (helium `c_p` from the CoolProp-derived Helmholtz
//!   EOS, the IAPWS-IF97 steam/water flashes, the five-group U-235
//!   delayed-neutron data);
//! - values that are dynamically computed but whose magnitude is set by
//!   **illustrative** stand-in plant data (loop geometry, IHX `UA`,
//!   efficiencies, kinetics parameters, controller constants) — the simulator's
//!   own module docs call these illustrative, and so does this node map;
//! - values that are **held constant** and do not vary during a run at all —
//!   the steam pressure and the condenser pressure, and therefore the
//!   condensate and feedwater enthalpies that follow from them.
//!
//! `RESPONSIBLE_USE.md` is the reason: publishing a fixed 10 MPa as though it
//! were a live steam-header measurement is exactly the failure mode this
//! project exists to avoid. Nothing here is a measurement; nothing here is
//! validated.
//!
//! # What is deliberately NOT published
//!
//! - **Control-rod position.** The simulator has no control-rod model. It
//!   takes external reactivity in dollars directly
//!   ([`HtgrControl::ExternalReactivity`]). A "rod position" node would have to
//!   be invented from a worth curve that does not exist in the model, so there
//!   is none.
//! - **Moderator / graphite temperature.** The kinetics carries a *single*
//!   lumped fuel-temperature node with one whole-core heat capacity. There is
//!   no separate moderator temperature to publish, and reporting the fuel
//!   temperature under a second name would be a fabrication.
//! - **Electrical output.** [`HtgrSignal::NetCyclePower`] is *mechanical* shaft
//!   power less feed-pump work. No generator, no house load, no grid.
//!
//! # Node identifiers
//!
//! Nodes live in namespace [`HTGR_NAMESPACE_URI`] and use **string**
//! identifiers, so a client can address them without browsing:
//!
//! ```text
//! ns=<index>;s=HTGR.Kinetics.ReactorThermalPowerWatt
//! ns=<index>;s=HTGR.Primary.CoreOutletTemperatureKelvin
//! ```
//!
//! The namespace index is assigned by the server at start-up (typically `2`).
//! Do not hard-code it in a client — read it from the server, or resolve by
//! browse name.
//!
//! Unlike `ciet_opcua`'s node map, the **Rust variant names carry no unit
//! suffix** (`ReactorThermalPower`, not `ReactorThermalPowerMw`). CIET needs
//! the suffix because it mixes kW with degC with Pa; here the SI convention is
//! global, so the suffix would be noise in Rust while remaining essential on
//! the wire — which is why the browse names keep it.
//!
//! # Scope (`RESPONSIBLE_USE.md`)
//!
//! OPC-UA is a plant-connectivity protocol, so the boundary matters: this
//! interface exists so an **offline demonstration simulator** can be driven by
//! standard OPC-UA tooling on a bench or in a classroom. It must never be
//! connected to live operational systems, plant systems, safety-critical
//! infrastructure, real-time plant monitoring, or institutional production
//! systems, and its outputs are not authoritative for any operational,
//! licensing or safety purpose.

use super::state::HtgrPlantSnapshot;

/// Namespace URI for every variable this simulator publishes.
///
/// Deliberately **not** named for HTR-10. The simulator behind this node map
/// (`examples/htgr_sim_v1`) is a generic helium-cooled, graphite-moderated HTGR
/// at an illustrative ~200 MWth on ~85 kg/s of helium; the real HTR-10 is
/// 10 MWth on 4.3 kg/s (see [`crate::htr10::design`]). Publishing this model
/// under an `htr-10` identity would tell a client it is looking at a specific
/// licensed design. When the HTR-10 rewrite (bead `op-jyyp`) lands, the
/// namespace URI is the thing to change — with the node map's constants and
/// descriptions revisited at the same time.
pub const HTGR_NAMESPACE_URI: &str = "urn:outram-park:htgr-demonstration-simulator-v1";

/// Default OPC-UA TCP port for this simulator.
///
/// **Not** 4840. 4840 is the IANA-registered `opcua-tcp` port and is already
/// taken by `ciet_opcua`'s server; a maintainer running both demonstrators on
/// one machine would otherwise hit a bind failure with no obvious cause.
pub const DEFAULT_OPCUA_PORT: u16 = 4841;

/// Endpoint path appended to the server URL, giving
/// `opc.tcp://<host>:<port>/htgr`.
pub const ENDPOINT_PATH: &str = "/htgr";

/// Browse name of the folder holding the writable controls.
pub const CONTROLS_FOLDER_NAME: &str = "Controls";

/// One-line statement of the unit convention, suitable for the server to hang
/// on the root folder's description so a browsing client meets it immediately.
pub const UNIT_CONVENTION_NOTICE: &str =
    "All values are bare f64 in SI units (W, K, Pa, kg/s, s, J/kg, J/(kg K), \
     dimensionless). The unit is repeated in every browse name, display name \
     and description. Outputs are from an OFFLINE, NOT-VALIDATED demonstration \
     model; they are not measurements.";

/// A subsystem of the plant, used to group nodes into OPC-UA folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subsystem {
    /// Reactor kinetics: power, fuel temperature, reactivity.
    Kinetics,
    /// Helium primary loop: core temperatures, flow, hydraulics, properties.
    PrimaryHeliumLoop,
    /// Intermediate heat exchanger between the helium and steam sides.
    IntermediateHeatExchanger,
    /// Steam (Rankine) secondary cycle: steam generator, turbine, condenser,
    /// feedwater.
    SecondarySteamCycle,
    /// Simulation bookkeeping rather than plant state.
    Diagnostics,
    /// Client-writable set points.
    Controls,
}

impl Subsystem {
    /// Every subsystem, in the order the address space presents them.
    pub const ALL: &'static [Subsystem] = &[
        Self::Kinetics,
        Self::PrimaryHeliumLoop,
        Self::IntermediateHeatExchanger,
        Self::SecondarySteamCycle,
        Self::Diagnostics,
        Self::Controls,
    ];

    /// Folder browse name, and the middle segment of every node identifier in
    /// this subsystem (`HTGR.<folder>.<node>`).
    pub fn folder_name(&self) -> &'static str {
        match self {
            Self::Kinetics => "Kinetics",
            Self::PrimaryHeliumLoop => "Primary",
            Self::IntermediateHeatExchanger => "Ihx",
            Self::SecondarySteamCycle => "Secondary",
            Self::Diagnostics => "Diagnostics",
            Self::Controls => CONTROLS_FOLDER_NAME,
        }
    }

    /// Human-facing folder label.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Kinetics => "Reactor kinetics",
            Self::PrimaryHeliumLoop => "Primary helium loop",
            Self::IntermediateHeatExchanger => "Intermediate heat exchanger",
            Self::SecondarySteamCycle => "Secondary steam cycle",
            Self::Diagnostics => "Simulation diagnostics",
            Self::Controls => "Controls (writable)",
        }
    }
}

/// How much of a published value rests on real physics, and how much on
/// stand-in numbers.
///
/// This is not a quality score. Every value here comes from an offline,
/// unvalidated demonstration model. The distinction it draws is the one a
/// client actually needs: whether the number's magnitude was set by a published
/// property library or by an illustrative plant constant somebody picked to
/// look HTGR-shaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelFidelity {
    /// The numerical content comes from a real, published property library or
    /// published nuclear data — the CoolProp-derived helium Helmholtz EOS, the
    /// IAPWS-IF97 steam/water tables, the five-group U-235 delayed-neutron
    /// data — evaluated at this model's (illustrative) operating point.
    RealPropertyData,
    /// Dynamically computed, but the plant data that sets its magnitude are
    /// illustrative HTGR-scale stand-ins rather than any design's data.
    IllustrativeParameters,
    /// An exact bookkeeping quantity of the simulation itself, not a modelled
    /// physical measurement.
    ExactDiagnostic,
}

impl ModelFidelity {
    /// The sentence appended to every node description carrying this fidelity.
    ///
    /// Written for the person reading the node in a generic OPC-UA browser, who
    /// has none of this repository's context.
    pub fn note(&self) -> &'static str {
        match self {
            Self::RealPropertyData => {
                "MODEL FIDELITY: the numerical content of this value comes from a real, \
                 published property library or published nuclear data, evaluated at this \
                 model's illustrative operating point. It is a simulation output, not a \
                 measurement, and the surrounding plant model is not validated."
            }
            Self::IllustrativeParameters => {
                "MODEL FIDELITY: ILLUSTRATIVE. This value is dynamically computed, but the \
                 plant data that set its magnitude (loop geometry, heat-exchanger UA, \
                 efficiencies, kinetics parameters, controller constants) are HTGR-scale \
                 stand-ins, not any plant's design data. It is a simulation output, not a \
                 measurement, and the model is not validated."
            }
            Self::ExactDiagnostic => {
                "MODEL FIDELITY: bookkeeping diagnostic of the simulation itself, not a \
                 modelled physical measurement."
            }
        }
    }
}

/// Sentence appended to a node that is not dynamically modelled at all.
const HELD_CONSTANT_NOTE: &str =
    "HELD CONSTANT: this quantity is not dynamically modelled in the current version. \
     It does not vary during a run and must not be read as a live measurement.";

/// Whether a client may write a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeAccess {
    /// `CurrentRead` only — a simulator output.
    ReadOnly,
    /// `CurrentRead | CurrentWrite` — a set point a client may drive.
    ReadWrite,
}

/// A read-only quantity the simulator publishes.
///
/// All are `f64` in the SI unit named by [`unit`](Self::unit). Read one out of
/// a snapshot with [`read`](Self::read); that accessor is the only place the
/// server touches plant state, so the node-to-field mapping exists exactly
/// once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HtgrSignal {
    // ---- Kinetics ----
    /// Total reactor thermal power (prompt + delayed), W.
    ReactorThermalPower,
    /// Prompt-excursion-layer power, W.
    PromptPower,
    /// Delayed-neutron power increment added over the last step, W.
    DelayedPowerIncrement,
    /// Lumped whole-core fuel temperature, K.
    FuelTemperature,
    /// Reactivity margin, dollars.
    ReactivityMargin,
    /// Effective total delayed-neutron fraction, dimensionless.
    DelayedNeutronFraction,

    // ---- Primary helium loop ----
    /// Core inlet helium temperature, K.
    CoreInletTemperature,
    /// Core outlet helium temperature, K.
    CoreOutletTemperature,
    /// Helium mass flow, kg/s.
    HeliumMassFlow,
    /// Helium loop residence time, s.
    HeliumResidenceTime,
    /// Frictional pressure drop around the helium loop, Pa.
    PrimaryPressureDrop,
    /// Circulator hydraulic power, W.
    CirculatorPower,
    /// Helium isobaric specific heat, J/(kg K).
    HeliumSpecificHeat,

    // ---- Intermediate heat exchanger ----
    /// Heat transferred from helium to steam, W.
    IhxDuty,
    /// Helium-side IHX outlet temperature, K.
    IhxHeliumOutletTemperature,

    // ---- Secondary steam cycle ----
    /// Live steam pressure, Pa (held constant in this model version).
    SteamPressure,
    /// Steam-generator steam outlet temperature, K.
    SteamGeneratorOutletTemperature,
    /// Steam-generator outlet specific enthalpy, J/kg.
    SteamSpecificEnthalpy,
    /// Turbine inlet steam temperature, K.
    TurbineInletTemperature,
    /// Turbine mechanical power, W.
    TurbinePower,
    /// Steam quality at the turbine exhaust, dimensionless.
    SteamQualityAfterTurbine,
    /// Condenser back-pressure, Pa (held constant in this model version).
    CondenserPressure,
    /// Secondary (feedwater/steam) mass flow, kg/s.
    FeedwaterMassFlow,
    /// Secondary loop residence time, s.
    SecondaryResidenceTime,
    /// Feedwater specific enthalpy, J/kg.
    FeedwaterSpecificEnthalpy,
    /// Hotwell condensate specific enthalpy, J/kg.
    CondensateSpecificEnthalpy,
    /// Feed-pump power, W.
    FeedPumpPower,
    /// Net cycle mechanical power, W.
    NetCyclePower,
    /// Condenser heat rejection, W.
    CondenserDuty,
    /// Cooling-water outlet temperature, K.
    CoolingWaterOutletTemperature,

    // ---- Diagnostics ----
    /// Accumulated simulated time, s.
    SimulationTime,
}

impl HtgrSignal {
    /// Every signal, in the order the address space and UI tables present them.
    pub const ALL: &'static [HtgrSignal] = &[
        Self::ReactorThermalPower,
        Self::PromptPower,
        Self::DelayedPowerIncrement,
        Self::FuelTemperature,
        Self::ReactivityMargin,
        Self::DelayedNeutronFraction,
        Self::CoreInletTemperature,
        Self::CoreOutletTemperature,
        Self::HeliumMassFlow,
        Self::HeliumResidenceTime,
        Self::PrimaryPressureDrop,
        Self::CirculatorPower,
        Self::HeliumSpecificHeat,
        Self::IhxDuty,
        Self::IhxHeliumOutletTemperature,
        Self::SteamPressure,
        Self::SteamGeneratorOutletTemperature,
        Self::SteamSpecificEnthalpy,
        Self::TurbineInletTemperature,
        Self::TurbinePower,
        Self::SteamQualityAfterTurbine,
        Self::CondenserPressure,
        Self::FeedwaterMassFlow,
        Self::SecondaryResidenceTime,
        Self::FeedwaterSpecificEnthalpy,
        Self::CondensateSpecificEnthalpy,
        Self::FeedPumpPower,
        Self::NetCyclePower,
        Self::CondenserDuty,
        Self::CoolingWaterOutletTemperature,
        Self::SimulationTime,
    ];

    /// Which plant subsystem this signal belongs to, and therefore which folder
    /// it appears in.
    pub fn subsystem(&self) -> Subsystem {
        match self {
            Self::ReactorThermalPower
            | Self::PromptPower
            | Self::DelayedPowerIncrement
            | Self::FuelTemperature
            | Self::ReactivityMargin
            | Self::DelayedNeutronFraction => Subsystem::Kinetics,

            Self::CoreInletTemperature
            | Self::CoreOutletTemperature
            | Self::HeliumMassFlow
            | Self::HeliumResidenceTime
            | Self::PrimaryPressureDrop
            | Self::CirculatorPower
            | Self::HeliumSpecificHeat => Subsystem::PrimaryHeliumLoop,

            Self::IhxDuty | Self::IhxHeliumOutletTemperature => {
                Subsystem::IntermediateHeatExchanger
            }

            Self::SteamPressure
            | Self::SteamGeneratorOutletTemperature
            | Self::SteamSpecificEnthalpy
            | Self::TurbineInletTemperature
            | Self::TurbinePower
            | Self::SteamQualityAfterTurbine
            | Self::CondenserPressure
            | Self::FeedwaterMassFlow
            | Self::SecondaryResidenceTime
            | Self::FeedwaterSpecificEnthalpy
            | Self::CondensateSpecificEnthalpy
            | Self::FeedPumpPower
            | Self::NetCyclePower
            | Self::CondenserDuty
            | Self::CoolingWaterOutletTemperature => Subsystem::SecondarySteamCycle,

            Self::SimulationTime => Subsystem::Diagnostics,
        }
    }

    /// The string part of this signal's `NodeId`, e.g.
    /// `"HTGR.Primary.CoreOutletTemperatureKelvin"`.
    ///
    /// Stable: treat these as public API, because client configurations and
    /// saved trend definitions reference them by name. The last segment always
    /// ends in the SI unit.
    pub fn node_identifier(&self) -> &'static str {
        match self {
            Self::ReactorThermalPower => "HTGR.Kinetics.ReactorThermalPowerWatt",
            Self::PromptPower => "HTGR.Kinetics.PromptPowerWatt",
            Self::DelayedPowerIncrement => "HTGR.Kinetics.DelayedPowerIncrementWatt",
            Self::FuelTemperature => "HTGR.Kinetics.FuelTemperatureKelvin",
            Self::ReactivityMargin => "HTGR.Kinetics.ReactivityMarginDollar",
            Self::DelayedNeutronFraction => "HTGR.Kinetics.DelayedNeutronFractionRatio",

            Self::CoreInletTemperature => "HTGR.Primary.CoreInletTemperatureKelvin",
            Self::CoreOutletTemperature => "HTGR.Primary.CoreOutletTemperatureKelvin",
            Self::HeliumMassFlow => "HTGR.Primary.HeliumMassFlowKilogramPerSecond",
            Self::HeliumResidenceTime => "HTGR.Primary.HeliumResidenceTimeSecond",
            Self::PrimaryPressureDrop => "HTGR.Primary.PressureDropPascal",
            Self::CirculatorPower => "HTGR.Primary.CirculatorPowerWatt",
            Self::HeliumSpecificHeat => "HTGR.Primary.HeliumSpecificHeatJoulePerKilogramKelvin",

            Self::IhxDuty => "HTGR.Ihx.DutyWatt",
            Self::IhxHeliumOutletTemperature => "HTGR.Ihx.HeliumOutletTemperatureKelvin",

            Self::SteamPressure => "HTGR.Secondary.SteamPressurePascal",
            Self::SteamGeneratorOutletTemperature => {
                "HTGR.Secondary.SteamGeneratorOutletTemperatureKelvin"
            }
            Self::SteamSpecificEnthalpy => "HTGR.Secondary.SteamSpecificEnthalpyJoulePerKilogram",
            Self::TurbineInletTemperature => "HTGR.Secondary.TurbineInletTemperatureKelvin",
            Self::TurbinePower => "HTGR.Secondary.TurbinePowerWatt",
            Self::SteamQualityAfterTurbine => "HTGR.Secondary.SteamQualityAfterTurbineRatio",
            Self::CondenserPressure => "HTGR.Secondary.CondenserPressurePascal",
            Self::FeedwaterMassFlow => "HTGR.Secondary.FeedwaterMassFlowKilogramPerSecond",
            Self::SecondaryResidenceTime => "HTGR.Secondary.ResidenceTimeSecond",
            Self::FeedwaterSpecificEnthalpy => {
                "HTGR.Secondary.FeedwaterSpecificEnthalpyJoulePerKilogram"
            }
            Self::CondensateSpecificEnthalpy => {
                "HTGR.Secondary.CondensateSpecificEnthalpyJoulePerKilogram"
            }
            Self::FeedPumpPower => "HTGR.Secondary.FeedPumpPowerWatt",
            Self::NetCyclePower => "HTGR.Secondary.NetCyclePowerWatt",
            Self::CondenserDuty => "HTGR.Secondary.CondenserDutyWatt",
            Self::CoolingWaterOutletTemperature => {
                "HTGR.Secondary.CoolingWaterOutletTemperatureKelvin"
            }

            Self::SimulationTime => "HTGR.Diagnostics.SimulationTimeSecond",
        }
    }

    /// Short OPC-UA browse name, e.g. `"CoreOutletTemperatureKelvin"` — the
    /// last dot-separated segment of [`node_identifier`](Self::node_identifier).
    pub fn browse_name(&self) -> &'static str {
        last_segment(self.node_identifier())
    }

    /// Human-facing label, always ending in the SI unit symbol in brackets.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ReactorThermalPower => "Reactor thermal power [W]",
            Self::PromptPower => "Prompt-layer power [W]",
            Self::DelayedPowerIncrement => "Delayed-neutron power increment [W]",
            Self::FuelTemperature => "Lumped fuel temperature [K]",
            Self::ReactivityMargin => "Reactivity margin [$]",
            Self::DelayedNeutronFraction => "Effective delayed-neutron fraction [1]",

            Self::CoreInletTemperature => "Core inlet helium temperature [K]",
            Self::CoreOutletTemperature => "Core outlet helium temperature [K]",
            Self::HeliumMassFlow => "Helium mass flow [kg/s]",
            Self::HeliumResidenceTime => "Helium loop residence time [s]",
            Self::PrimaryPressureDrop => "Primary loop pressure drop [Pa]",
            Self::CirculatorPower => "Circulator hydraulic power [W]",
            Self::HeliumSpecificHeat => "Helium isobaric specific heat [J/(kg K)]",

            Self::IhxDuty => "IHX duty [W]",
            Self::IhxHeliumOutletTemperature => "IHX helium outlet temperature [K]",

            Self::SteamPressure => "Live steam pressure [Pa]",
            Self::SteamGeneratorOutletTemperature => "Steam-generator outlet temperature [K]",
            Self::SteamSpecificEnthalpy => "Steam specific enthalpy [J/kg]",
            Self::TurbineInletTemperature => "Turbine inlet temperature [K]",
            Self::TurbinePower => "Turbine mechanical power [W]",
            Self::SteamQualityAfterTurbine => "Steam quality after turbine [1]",
            Self::CondenserPressure => "Condenser back-pressure [Pa]",
            Self::FeedwaterMassFlow => "Feedwater / steam mass flow [kg/s]",
            Self::SecondaryResidenceTime => "Secondary loop residence time [s]",
            Self::FeedwaterSpecificEnthalpy => "Feedwater specific enthalpy [J/kg]",
            Self::CondensateSpecificEnthalpy => "Condensate specific enthalpy [J/kg]",
            Self::FeedPumpPower => "Feed-pump power [W]",
            Self::NetCyclePower => "Net cycle mechanical power [W]",
            Self::CondenserDuty => "Condenser heat rejection [W]",
            Self::CoolingWaterOutletTemperature => "Cooling-water outlet temperature [K]",

            Self::SimulationTime => "Simulated time [s]",
        }
    }

    /// SI unit symbol, as a short display string. Never empty — dimensionless
    /// quantities report `"1"`, and reactivity in dollars reports `"$"`.
    pub fn unit(&self) -> &'static str {
        match self {
            Self::ReactorThermalPower
            | Self::PromptPower
            | Self::DelayedPowerIncrement
            | Self::CirculatorPower
            | Self::IhxDuty
            | Self::TurbinePower
            | Self::FeedPumpPower
            | Self::NetCyclePower
            | Self::CondenserDuty => "W",

            Self::FuelTemperature
            | Self::CoreInletTemperature
            | Self::CoreOutletTemperature
            | Self::IhxHeliumOutletTemperature
            | Self::SteamGeneratorOutletTemperature
            | Self::TurbineInletTemperature
            | Self::CoolingWaterOutletTemperature => "K",

            Self::PrimaryPressureDrop | Self::SteamPressure | Self::CondenserPressure => "Pa",

            Self::HeliumMassFlow | Self::FeedwaterMassFlow => "kg/s",

            Self::HeliumResidenceTime | Self::SecondaryResidenceTime | Self::SimulationTime => "s",

            Self::SteamSpecificEnthalpy
            | Self::FeedwaterSpecificEnthalpy
            | Self::CondensateSpecificEnthalpy => "J/kg",

            Self::HeliumSpecificHeat => "J/(kg K)",

            Self::ReactivityMargin => "$",

            Self::DelayedNeutronFraction | Self::SteamQualityAfterTurbine => "1",
        }
    }

    /// What the quantity is, physically, and how the model produces it.
    ///
    /// This is the physics sentence only; [`description`](Self::description)
    /// wraps it with the unit statement and the fidelity caveats that actually
    /// go on the wire.
    pub fn summary(&self) -> &'static str {
        match self {
            Self::ReactorThermalPower => {
                "Total reactor thermal power, prompt plus delayed, from a point-kinetics model: \
                 a Nordheim-Fuchs prompt-excursion layer coupled by a Lie split to a five-group \
                 U-235 delayed-neutron precursor bank."
            }
            Self::PromptPower => {
                "Prompt-excursion-layer power P_p, before the delayed-neutron increment is fed \
                 back into the prompt model."
            }
            Self::DelayedPowerIncrement => {
                "Delayed-neutron power increment S*dt added over the most recent timestep. This \
                 is a per-step increment, not the delayed power fraction, so its magnitude \
                 depends on the timestep length."
            }
            Self::FuelTemperature => {
                "Lumped fuel temperature from the prompt layer's adiabatic feedback. The model \
                 carries a SINGLE whole-core fuel node; there is no separate graphite-moderator \
                 temperature, and none is published."
            }
            Self::ReactivityMargin => {
                "Reactivity margin rho_ext - beta + alpha_f (T_fuel - T_ref), expressed in \
                 dollars (rho/beta, dimensionless). Negative means subcritical on prompt \
                 neutrons. Multiply by the effective delayed-neutron fraction to recover the \
                 absolute reactivity."
            }
            Self::DelayedNeutronFraction => {
                "Effective total delayed-neutron fraction beta = sum(beta_i) of the five-group \
                 U-235 precursor bank, as a dimensionless ratio (0.0065 here). Multiply by 1e5 \
                 for pcm. It is a constant of the precursor bank, not a computed plant state."
            }

            Self::CoreInletTemperature => {
                "Helium temperature entering the core. A computed loop variable, not a boundary \
                 condition: it is the IHX helium-side outlet after the loop's return transport \
                 lag, so reducing secondary heat removal raises it."
            }
            Self::CoreOutletTemperature => {
                "Helium temperature leaving the core, from the lumped-node energy balance \
                 T_in + Q/(m_dot c_p) relaxed through a first-order core thermal lag."
            }
            Self::HeliumMassFlow => {
                "Helium mass flow through the core. It follows the commanded circulator setpoint \
                 with a 1 kg/s floor; no circulator dynamics are modelled, so it tracks the \
                 setpoint within one timestep."
            }
            Self::HeliumResidenceTime => {
                "Loop transport residence time m/m_dot, from the helium inventory rho*A*L at the \
                 live density. This is what sets the travel time of the schematic's flow tracers."
            }
            Self::PrimaryPressureDrop => {
                "Frictional pressure drop around the helium loop: Darcy-Weisbach with a Haaland \
                 friction factor, evaluated at the live helium density and bulk velocity."
            }
            Self::CirculatorPower => {
                "Circulator hydraulic power m_dot dp / (rho eta) needed to sustain the loop \
                 pressure drop. Hydraulic power, not electrical input."
            }
            Self::HeliumSpecificHeat => {
                "Helium isobaric specific heat at the loop pressure (7 MPa) and the current bulk \
                 mean loop temperature, from the CoolProp-derived Helmholtz equation of state \
                 (Ortiz-Vega et al.), re-evaluated every timestep rather than frozen."
            }

            Self::IhxDuty => {
                "Heat transferred from the helium loop to the steam side by the intermediate \
                 heat exchanger, from an effectiveness-NTU model with one isothermal (boiling) \
                 side, pinched against the steam saturation temperature. It cannot exceed what \
                 the temperature difference and UA support, and is zero while the helium is \
                 colder than the steam side."
            }
            Self::IhxHeliumOutletTemperature => {
                "Helium temperature leaving the IHX, T_core_out - Q_ihx/(m_dot c_p). The core \
                 inlet relaxes toward this value through the return transport lag."
            }

            Self::SteamPressure => {
                "Live steam pressure at the steam-generator outlet and turbine inlet. The model \
                 carries no steam-generator mass-and-energy inventory, so there is no \
                 sliding-pressure or drum response: this is a fixed 10 MPa."
            }
            Self::SteamGeneratorOutletTemperature => {
                "Steam temperature leaving the steam generator, from an IAPWS-IF97 (p, h) flash \
                 of the secondary-side energy balance h_feed + Q_ihx/m_dot."
            }
            Self::SteamSpecificEnthalpy => {
                "Specific enthalpy of the steam leaving the steam generator, h_feed + \
                 Q_ihx/m_dot. The feedwater controller chases a fixed 3.4e6 J/kg target."
            }
            Self::TurbineInletTemperature => {
                "Steam temperature at the turbine inlet. IDENTICAL to the steam-generator outlet \
                 temperature in this model: no steam-line heat loss or pressure drop is modelled \
                 between them. Both are published because both exist in the simulator's own \
                 state, not because they are independent measurements."
            }
            Self::TurbinePower => {
                "Turbine mechanical power m_dot (h_in - h_out), from an isentropic IAPWS-IF97 \
                 (p, s) expansion to condenser pressure de-rated by an adiabatic efficiency. \
                 Shaft power; no generator is modelled."
            }
            Self::SteamQualityAfterTurbine => {
                "Steam quality at the turbine exhaust from an IAPWS-IF97 (p, h) flash at the \
                 condenser pressure. 0 is saturated liquid, 1 is dry saturated vapour."
            }
            Self::CondenserPressure => {
                "Condenser back-pressure, a fixed 7 kPa consistent with the illustrative \
                 cooling-water inlet temperature. No condenser pressure dynamics are modelled."
            }
            Self::FeedwaterMassFlow => {
                "Secondary water/steam mass flow, moved by a first-order-lagged proportional \
                 feedwater controller toward the flow that would hold the target steam enthalpy \
                 at the current duty, clamped to 5-200 kg/s."
            }
            Self::SecondaryResidenceTime => {
                "Secondary loop transport residence time m/m_dot, from the secondary water/steam \
                 inventory. This is what sets the travel time of the schematic's steam-line \
                 tracers."
            }
            Self::FeedwaterSpecificEnthalpy => {
                "Feedwater specific enthalpy entering the steam generator: hotwell condensate \
                 plus the real feed-pump work v dp / eta. It is computed rather than assumed, \
                 but with both cycle pressures held fixed it evaluates to the same number every \
                 step."
            }
            Self::CondensateSpecificEnthalpy => {
                "Saturated-liquid specific enthalpy in the condenser hotwell, an IAPWS-IF97 \
                 saturation flash at the condenser pressure. With the condenser pressure held \
                 fixed, this evaluates to the same number every step."
            }
            Self::FeedPumpPower => {
                "Feed-pump power m_dot (h_feedwater - h_condensate), the work put into raising \
                 the condensate to steam-generator pressure."
            }
            Self::NetCyclePower => {
                "Net cycle power: turbine MECHANICAL output less feed-pump work. There is no \
                 generator, no house load and no grid in this model, so this is not an \
                 electrical output."
            }
            Self::CondenserDuty => {
                "Heat rejected in the condenser, m_dot (h_turbine_out - h_condensate), carried \
                 away by the cooling-water stream."
            }
            Self::CoolingWaterOutletTemperature => {
                "Cooling-water outlet temperature from the condenser energy balance \
                 Q/(m_cw c_p), above an illustrative 298.15 K inlet at an illustrative \
                 cooling-water flow."
            }

            Self::SimulationTime => {
                "Accumulated SIMULATED time since the model started. This is not wall-clock \
                 time; the two diverge whenever the simulator runs faster or slower than real \
                 time."
            }
        }
    }

    /// How much of this value rests on real physics versus stand-in plant data.
    pub fn fidelity(&self) -> ModelFidelity {
        match self {
            // Direct evaluations of a published property library or published
            // nuclear data.
            Self::DelayedNeutronFraction
            | Self::HeliumSpecificHeat
            | Self::SteamGeneratorOutletTemperature
            | Self::TurbineInletTemperature
            | Self::SteamQualityAfterTurbine
            | Self::FeedwaterSpecificEnthalpy
            | Self::CondensateSpecificEnthalpy => ModelFidelity::RealPropertyData,

            // Simulation bookkeeping.
            Self::SimulationTime => ModelFidelity::ExactDiagnostic,

            // Everything else is dynamically computed on illustrative plant
            // data (geometry, UA, efficiencies, kinetics parameters,
            // controller constants).
            Self::ReactorThermalPower
            | Self::PromptPower
            | Self::DelayedPowerIncrement
            | Self::FuelTemperature
            | Self::ReactivityMargin
            | Self::CoreInletTemperature
            | Self::CoreOutletTemperature
            | Self::HeliumMassFlow
            | Self::HeliumResidenceTime
            | Self::PrimaryPressureDrop
            | Self::CirculatorPower
            | Self::IhxDuty
            | Self::IhxHeliumOutletTemperature
            | Self::SteamPressure
            | Self::SteamSpecificEnthalpy
            | Self::TurbinePower
            | Self::CondenserPressure
            | Self::FeedwaterMassFlow
            | Self::SecondaryResidenceTime
            | Self::FeedPumpPower
            | Self::NetCyclePower
            | Self::CondenserDuty
            | Self::CoolingWaterOutletTemperature => ModelFidelity::IllustrativeParameters,
        }
    }

    /// Whether this quantity actually changes while the simulator runs.
    ///
    /// `false` means the published number is fixed by construction in the
    /// current model version. A client trending such a node gets a flat line
    /// forever, and must not read it as a live measurement — which is why it is
    /// stated per node and repeated in [`description`](Self::description)
    /// rather than left for a reader to discover.
    ///
    /// The four constants are the two cycle pressures (no steam-generator
    /// inventory model, no condenser dynamics) and the two enthalpies that
    /// follow from them, plus the delayed-neutron fraction, which is a property
    /// of the precursor bank rather than a plant state.
    pub fn varies_during_run(&self) -> bool {
        !matches!(
            self,
            Self::SteamPressure
                | Self::CondenserPressure
                | Self::FeedwaterSpecificEnthalpy
                | Self::CondensateSpecificEnthalpy
                | Self::DelayedNeutronFraction
        )
    }

    /// The full OPC-UA `Description` attribute: the physics summary, the unit
    /// restated in words, the held-constant warning where it applies, and the
    /// model-fidelity caveat.
    ///
    /// Composed rather than written out per node so that no variant can be
    /// added without its caveats — there is no code path that publishes a
    /// summary alone.
    pub fn description(&self) -> String {
        let mut text = String::with_capacity(512);
        text.push_str(self.summary());
        text.push_str(" UNIT: this value is a bare f64 in SI units, ");
        text.push_str(self.unit());
        text.push('.');
        if !self.varies_during_run() {
            text.push(' ');
            text.push_str(HELD_CONSTANT_NOTE);
        }
        text.push(' ');
        text.push_str(self.fidelity().note());
        text
    }

    /// Read this signal's current value out of a plant snapshot.
    ///
    /// The only place the OPC-UA read callbacks touch plant state, so the
    /// node-to-field mapping exists exactly once. Pure: no locking, no
    /// side effects.
    pub fn read(&self, snapshot: &HtgrPlantSnapshot) -> f64 {
        match self {
            Self::ReactorThermalPower => snapshot.reactor_power_w,
            Self::PromptPower => snapshot.prompt_power_w,
            Self::DelayedPowerIncrement => snapshot.delayed_power_w,
            Self::FuelTemperature => snapshot.fuel_temperature_k,
            Self::ReactivityMargin => snapshot.reactivity_margin_dollar,
            Self::DelayedNeutronFraction => snapshot.delayed_neutron_fraction_ratio,

            Self::CoreInletTemperature => snapshot.core_inlet_temp_k,
            Self::CoreOutletTemperature => snapshot.core_outlet_temp_k,
            Self::HeliumMassFlow => snapshot.helium_mass_flow_kg_per_s,
            Self::HeliumResidenceTime => snapshot.helium_residence_time_s,
            Self::PrimaryPressureDrop => snapshot.primary_pressure_drop_pa,
            Self::CirculatorPower => snapshot.circulator_power_w,
            Self::HeliumSpecificHeat => snapshot.helium_cp_j_per_kg_k,

            Self::IhxDuty => snapshot.ihx_duty_w,
            Self::IhxHeliumOutletTemperature => snapshot.ihx_outlet_temp_k,

            Self::SteamPressure => snapshot.steam_pressure_pa,
            Self::SteamGeneratorOutletTemperature => snapshot.sg_steam_outlet_temp_k,
            Self::SteamSpecificEnthalpy => snapshot.steam_enthalpy_j_per_kg,
            Self::TurbineInletTemperature => snapshot.turbine_inlet_temp_k,
            Self::TurbinePower => snapshot.turbine_power_w,
            Self::SteamQualityAfterTurbine => snapshot.steam_quality_after_turbine,
            Self::CondenserPressure => snapshot.condenser_pressure_pa,
            Self::FeedwaterMassFlow => snapshot.secondary_mass_flow_kg_per_s,
            Self::SecondaryResidenceTime => snapshot.secondary_residence_time_s,
            Self::FeedwaterSpecificEnthalpy => snapshot.feedwater_enthalpy_j_per_kg,
            Self::CondensateSpecificEnthalpy => snapshot.condensate_enthalpy_j_per_kg,
            Self::FeedPumpPower => snapshot.feed_pump_power_w,
            Self::NetCyclePower => snapshot.net_cycle_power_w,
            Self::CondenserDuty => snapshot.condenser_duty_w,
            Self::CoolingWaterOutletTemperature => snapshot.cooling_water_outlet_temp_k,

            Self::SimulationTime => snapshot.sim_time_s,
        }
    }
}

/// A continuous set point a client may write.
///
/// The simulator has exactly two operator inputs, matching the two sliders in
/// its GUI. Writes are **clamped** to [`valid_range`](Self::valid_range) rather
/// than rejected, so a client that sends 1000 kg/s gets the 150 kg/s ceiling
/// and a clear read-back, and a NaN write is ignored outright.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HtgrControl {
    /// Externally inserted reactivity, dollars (`rho/beta`).
    ///
    /// This is the reactivity input itself, **not** a control-rod position: the
    /// simulator has no rod model, no rod worth curve, and no rod drive.
    ExternalReactivity,
    /// Helium circulator mass-flow set point, kg/s.
    HeliumFlowSetpoint,
}

impl HtgrControl {
    /// Every control, in address-space and UI order.
    pub const ALL: &'static [HtgrControl] = &[Self::ExternalReactivity, Self::HeliumFlowSetpoint];

    /// Position of this control in [`Self::ALL`].
    ///
    /// Lets a transport layer index fixed-size pending-request arrays without
    /// allocating or hashing. Guaranteed `< HtgrControl::ALL.len()`.
    pub fn index(&self) -> usize {
        Self::ALL
            .iter()
            .position(|candidate| candidate == self)
            .expect("every HtgrControl variant must appear in HtgrControl::ALL")
    }

    /// Which folder this control appears in — always [`Subsystem::Controls`].
    pub fn subsystem(&self) -> Subsystem {
        Subsystem::Controls
    }

    /// The string part of this control's `NodeId`.
    pub fn node_identifier(&self) -> &'static str {
        match self {
            Self::ExternalReactivity => "HTGR.Controls.ExternalReactivityDollar",
            Self::HeliumFlowSetpoint => "HTGR.Controls.HeliumFlowSetpointKilogramPerSecond",
        }
    }

    /// Short OPC-UA browse name — the last dot-separated segment of
    /// [`node_identifier`](Self::node_identifier).
    pub fn browse_name(&self) -> &'static str {
        last_segment(self.node_identifier())
    }

    /// Human-facing label, always ending in the SI unit symbol in brackets.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ExternalReactivity => "External reactivity set point [$]",
            Self::HeliumFlowSetpoint => "Helium circulator flow set point [kg/s]",
        }
    }

    /// SI unit symbol. Never empty; reactivity in dollars reports `"$"`.
    pub fn unit(&self) -> &'static str {
        match self {
            Self::ExternalReactivity => "$",
            Self::HeliumFlowSetpoint => "kg/s",
        }
    }

    /// What this set point does, physically.
    pub fn summary(&self) -> &'static str {
        match self {
            Self::ExternalReactivity => {
                "Externally inserted reactivity, in dollars (rho/beta, dimensionless), held \
                 constant over each timestep and converted to the prompt layer's rho_ext = \
                 dollars * beta. NOT a control-rod position: this simulator models no rods, no \
                 rod worth curve and no rod drive, so the reactivity is inserted directly."
            }
            Self::HeliumFlowSetpoint => {
                "Commanded helium circulator mass flow. The loop follows it within one timestep \
                 (no circulator dynamics are modelled) and floors it at 1 kg/s internally so the \
                 core energy balance stays finite."
            }
        }
    }

    /// Inclusive `(minimum, maximum)` this control accepts, matching the
    /// envelope the simulator's own GUI sliders allow.
    ///
    /// Reactivity is capped at +1 $, which is exactly prompt critical: the
    /// Nordheim-Fuchs prompt-excursion layer above that point runs away until
    /// the fuel-temperature feedback catches it, and letting a client drive
    /// further up is outside what this demonstration is for. The -2 $ floor and
    /// the 10-150 kg/s flow window are likewise the GUI's envelope, so a client
    /// and an operator can reach exactly the same states.
    pub fn valid_range(&self) -> (f64, f64) {
        match self {
            Self::ExternalReactivity => (-2.0, 1.0),
            Self::HeliumFlowSetpoint => (10.0, 150.0),
        }
    }

    /// The full OPC-UA `Description` attribute: what the set point does, its
    /// unit, its clamped envelope, and the scope caveat.
    pub fn description(&self) -> String {
        let (min, max) = self.valid_range();
        let mut text = String::with_capacity(512);
        text.push_str(self.summary());
        text.push_str(" UNIT: this value is a bare f64 in SI units, ");
        text.push_str(self.unit());
        text.push_str(". Writes are CLAMPED to the inclusive range [");
        text.push_str(&format_range_bound(min));
        text.push_str(", ");
        text.push_str(&format_range_bound(max));
        text.push_str(
            "] rather than rejected, and a NaN write is ignored. This drives an \
                       OFFLINE, NOT-VALIDATED demonstration simulator and nothing else.",
        );
        text
    }

    /// Read this control's current set point out of a snapshot, so a client can
    /// see what the value actually is after clamping.
    pub fn read(&self, snapshot: &HtgrPlantSnapshot) -> f64 {
        match self {
            Self::ExternalReactivity => snapshot.external_reactivity_dollar,
            Self::HeliumFlowSetpoint => snapshot.helium_flow_setpoint_kg_per_s,
        }
    }

    /// Apply a client's write to the snapshot, clamped to
    /// [`valid_range`](Self::valid_range).
    ///
    /// A NaN write is ignored entirely: a NaN set point would propagate into
    /// the kinetics and destroy the run. Returns the value actually stored,
    /// which a caller may log or feed back as the read-back.
    pub fn write(&self, snapshot: &mut HtgrPlantSnapshot, value: f64) -> f64 {
        if value.is_nan() {
            return self.read(snapshot);
        }
        let (min, max) = self.valid_range();
        let clamped = value.clamp(min, max);
        match self {
            Self::ExternalReactivity => snapshot.external_reactivity_dollar = clamped,
            Self::HeliumFlowSetpoint => snapshot.helium_flow_setpoint_kg_per_s = clamped,
        }
        self.read(snapshot)
    }
}

/// Where a node's value comes from, so a transport layer can hold one value per
/// node without a trait object.
///
/// Enum dispatch rather than `Box<dyn Fn>` per the workspace design rules:
/// adding a variant forces every match site to handle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeSource {
    /// A read-only simulator output.
    Signal(HtgrSignal),
    /// A client-writable set point.
    Control(HtgrControl),
}

impl NodeSource {
    /// Read this node's value out of a snapshot.
    pub fn read(&self, snapshot: &HtgrPlantSnapshot) -> f64 {
        match self {
            Self::Signal(signal) => signal.read(snapshot),
            Self::Control(control) => control.read(snapshot),
        }
    }

    /// Apply a client write, returning the stored value, or `None` if the node
    /// is read-only.
    pub fn write(&self, snapshot: &mut HtgrPlantSnapshot, value: f64) -> Option<f64> {
        match self {
            Self::Signal(_) => None,
            Self::Control(control) => Some(control.write(snapshot, value)),
        }
    }
}

/// One OPC-UA variable, as plain data.
///
/// This is the transport-independent description of a node: everything a server
/// needs to build an address space, and everything a client-side table needs to
/// render it, with no dependency on any particular OPC-UA crate, server config
/// or runtime. Build the whole list with [`all_nodes`].
#[derive(Debug, Clone, PartialEq)]
pub struct NodeDescriptor {
    /// String part of the `NodeId`, e.g. `"HTGR.Ihx.DutyWatt"`.
    pub node_identifier: &'static str,
    /// Single-segment browse name, e.g. `"DutyWatt"`.
    pub browse_name: &'static str,
    /// Human-facing label, ending in the SI unit in brackets.
    pub display_name: &'static str,
    /// SI unit symbol, e.g. `"W"`. Never empty.
    pub unit: &'static str,
    /// Full description attribute, including the unit and fidelity caveats.
    pub description: String,
    /// Folder this node belongs in.
    pub subsystem: Subsystem,
    /// Whether a client may write it.
    pub access: NodeAccess,
    /// How much of the value rests on real physics. `None` for controls, which
    /// are operator inputs rather than modelled quantities.
    pub fidelity: Option<ModelFidelity>,
    /// Whether the value changes while the simulator runs. Controls are `true`
    /// because a client can change them.
    pub varies_during_run: bool,
    /// Inclusive write envelope, `None` for read-only nodes.
    pub valid_range: Option<(f64, f64)>,
    /// How to read (and, for controls, write) the value.
    pub source: NodeSource,
}

/// Every OPC-UA variable this simulator publishes, signals first then controls.
///
/// Allocates; call it once at address-space build time, not per read.
pub fn all_nodes() -> Vec<NodeDescriptor> {
    let mut nodes = Vec::with_capacity(total_node_count());
    for signal in HtgrSignal::ALL {
        nodes.push(NodeDescriptor {
            node_identifier: signal.node_identifier(),
            browse_name: signal.browse_name(),
            display_name: signal.display_name(),
            unit: signal.unit(),
            description: signal.description(),
            subsystem: signal.subsystem(),
            access: NodeAccess::ReadOnly,
            fidelity: Some(signal.fidelity()),
            varies_during_run: signal.varies_during_run(),
            valid_range: None,
            source: NodeSource::Signal(*signal),
        });
    }
    for control in HtgrControl::ALL {
        nodes.push(NodeDescriptor {
            node_identifier: control.node_identifier(),
            browse_name: control.browse_name(),
            display_name: control.display_name(),
            unit: control.unit(),
            description: control.description(),
            subsystem: control.subsystem(),
            access: NodeAccess::ReadWrite,
            fidelity: None,
            varies_during_run: true,
            valid_range: Some(control.valid_range()),
            source: NodeSource::Control(*control),
        });
    }
    nodes
}

/// Total number of OPC-UA variables this simulator publishes.
pub fn total_node_count() -> usize {
    HtgrSignal::ALL.len() + HtgrControl::ALL.len()
}

/// The last dot-separated segment of a node identifier — its browse name.
fn last_segment(identifier: &'static str) -> &'static str {
    match identifier.rfind('.') {
        Some(dot) => &identifier[dot + 1..],
        None => identifier,
    }
}

/// Format a range bound for a description without a trailing `.0` on integers.
fn format_range_bound(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Build a snapshot in which every field holds a distinct value
    /// `1.0, 2.0, ...` in declaration order, and report how many fields there
    /// were.
    ///
    /// The struct literal names every field explicitly, so adding a field to
    /// [`HtgrPlantSnapshot`] breaks this helper's *compilation* — which is the
    /// point. A node map that silently stops covering a quantity is the defect
    /// these tests exist to catch, and a compile error is a louder failure than
    /// an assertion.
    fn sequentially_numbered_snapshot() -> (HtgrPlantSnapshot, usize) {
        let mut counter = 0.0_f64;
        let snapshot = {
            let mut next = || {
                counter += 1.0;
                counter
            };
            HtgrPlantSnapshot {
                external_reactivity_dollar: next(),
                helium_flow_setpoint_kg_per_s: next(),
                reactor_power_w: next(),
                prompt_power_w: next(),
                delayed_power_w: next(),
                fuel_temperature_k: next(),
                reactivity_margin_dollar: next(),
                delayed_neutron_fraction_ratio: next(),
                core_inlet_temp_k: next(),
                core_outlet_temp_k: next(),
                helium_mass_flow_kg_per_s: next(),
                helium_residence_time_s: next(),
                primary_pressure_drop_pa: next(),
                circulator_power_w: next(),
                helium_cp_j_per_kg_k: next(),
                ihx_duty_w: next(),
                ihx_outlet_temp_k: next(),
                steam_pressure_pa: next(),
                sg_steam_outlet_temp_k: next(),
                steam_enthalpy_j_per_kg: next(),
                turbine_inlet_temp_k: next(),
                turbine_power_w: next(),
                steam_quality_after_turbine: next(),
                condenser_pressure_pa: next(),
                secondary_mass_flow_kg_per_s: next(),
                secondary_residence_time_s: next(),
                feedwater_enthalpy_j_per_kg: next(),
                condensate_enthalpy_j_per_kg: next(),
                feed_pump_power_w: next(),
                net_cycle_power_w: next(),
                condenser_duty_w: next(),
                cooling_water_outlet_temp_k: next(),
                sim_time_s: next(),
            }
        };
        (snapshot, counter as usize)
    }

    /// The node map must publish **every** field of the snapshot, each through
    /// exactly one node. A field with no node is a quantity the twin silently
    /// drops; two nodes on one field is a quantity it silently duplicates under
    /// two names.
    ///
    /// **Methodology.** Fill the snapshot with the distinct sentinel values
    /// `1.0 ..= N` (one per field, in declaration order), read every signal and
    /// every control through its accessor, and compare the set of values
    /// returned with the set assigned. Pass criterion: exact set equality, and
    /// no two accessors returning the same sentinel.
    ///
    /// **Results (2026-08-12).** 33 snapshot fields, 33 nodes (31 signals +
    /// 2 controls), 33 distinct sentinels read back — full coverage, no
    /// duplicates, no omissions.
    #[test]
    fn signal_and_control_accessors_cover_every_snapshot_field() {
        let (snapshot, field_count) = sequentially_numbered_snapshot();

        let mut seen: HashSet<u64> = HashSet::new();
        for signal in HtgrSignal::ALL {
            let value = signal.read(&snapshot);
            assert!(
                seen.insert(value.to_bits()),
                "{signal:?} reads a snapshot field another node already publishes"
            );
        }
        for control in HtgrControl::ALL {
            let value = control.read(&snapshot);
            assert!(
                seen.insert(value.to_bits()),
                "{control:?} reads a snapshot field another node already publishes"
            );
        }

        assert_eq!(
            seen.len(),
            field_count,
            "the node map publishes {} of the snapshot's {field_count} fields",
            seen.len()
        );
        assert_eq!(total_node_count(), field_count);

        let expected: HashSet<u64> = (1..=field_count).map(|i| (i as f64).to_bits()).collect();
        assert_eq!(
            seen, expected,
            "some snapshot field is never published by any node"
        );
    }

    /// Every node identifier must be unique, or the address space would
    /// silently collapse two variables into one and a client would read the
    /// wrong value.
    ///
    /// **Methodology.** Collect every `node_identifier()` into a set and
    /// compare its size with the enumerated node count.
    ///
    /// **Results (2026-08-12).** 33 variables enumerated (31 signals,
    /// 2 controls), 33 distinct identifiers — no collisions.
    #[test]
    fn node_identifiers_are_unique() {
        let mut seen: HashSet<&str> = HashSet::new();
        for signal in HtgrSignal::ALL {
            assert!(
                seen.insert(signal.node_identifier()),
                "duplicate identifier: {signal:?}"
            );
        }
        for control in HtgrControl::ALL {
            assert!(
                seen.insert(control.node_identifier()),
                "duplicate identifier: {control:?}"
            );
        }
        assert_eq!(seen.len(), total_node_count());
    }

    /// Browse names must be unique single path segments — OPC-UA browse names
    /// cannot contain the path separator, and two siblings cannot share one.
    ///
    /// **Methodology.** Check each derived browse name is non-empty, free of
    /// `.`, and distinct across the whole map (not merely within a folder, so
    /// the flat check is stricter than OPC-UA requires).
    ///
    /// **Results (2026-08-12).** All 33 browse names valid and distinct.
    #[test]
    fn browse_names_are_unique_single_segments() {
        let mut seen: HashSet<&str> = HashSet::new();
        for node in all_nodes() {
            assert!(!node.browse_name.is_empty(), "empty browse name");
            assert!(
                !node.browse_name.contains('.'),
                "browse name is not a single segment: {}",
                node.browse_name
            );
            assert!(
                seen.insert(node.browse_name),
                "duplicate browse name: {}",
                node.browse_name
            );
        }
    }

    /// Node identifiers must follow `HTGR.<folder>.<browse name>`, so a client
    /// can predict an address from the folder it browsed.
    ///
    /// **Methodology.** For every node, check the identifier equals
    /// `format!("HTGR.{}.{}", subsystem.folder_name(), browse_name)`.
    ///
    /// **Results (2026-08-12).** All 33 identifiers matched the scheme across
    /// all six folders.
    #[test]
    fn node_identifiers_follow_the_folder_scheme() {
        for node in all_nodes() {
            let expected = format!("HTGR.{}.{}", node.subsystem.folder_name(), node.browse_name);
            assert_eq!(node.node_identifier, expected);
        }
    }

    /// THE unit rule: no node may reach the wire without naming its unit, in
    /// the browse name, the display name and the description alike. A client
    /// sees a bare `Double`; if the map is silent, 700 K and 700 degC are the
    /// same number to it.
    ///
    /// **Methodology.** For every node assert: `unit` is non-empty; the display
    /// name ends in `[unit]`; the description contains the unit and the literal
    /// word `UNIT`; and the browse name ends in a recognised unit-word suffix
    /// (`Watt`, `Kelvin`, `Pascal`, `KilogramPerSecond`, `Second`,
    /// `JoulePerKilogram`, `JoulePerKilogramKelvin`, `Ratio`, `Dollar`).
    ///
    /// **Results (2026-08-12).** All 33 nodes carried their unit in all four
    /// places.
    #[test]
    fn every_node_names_its_unit_everywhere() {
        const UNIT_SUFFIXES: &[&str] = &[
            "Watt",
            "Kelvin",
            "Pascal",
            "KilogramPerSecond",
            "Second",
            "JoulePerKilogram",
            "JoulePerKilogramKelvin",
            "Ratio",
            "Dollar",
        ];

        for node in all_nodes() {
            assert!(
                !node.unit.is_empty(),
                "{} has no unit",
                node.node_identifier
            );
            assert!(
                node.display_name.ends_with(&format!("[{}]", node.unit)),
                "display name {:?} does not end in its unit [{}]",
                node.display_name,
                node.unit
            );
            assert!(
                node.description.contains(node.unit),
                "description of {} never states its unit",
                node.node_identifier
            );
            assert!(
                node.description.contains("UNIT"),
                "description of {} has no UNIT clause",
                node.node_identifier
            );
            assert!(
                UNIT_SUFFIXES
                    .iter()
                    .any(|suffix| node.browse_name.ends_with(suffix)),
                "browse name {} does not end in a unit word",
                node.browse_name
            );
        }
    }

    /// Descriptions must be substantive, not a restated name — the description
    /// is where a browsing client learns what the quantity is and how far to
    /// trust it.
    ///
    /// **Methodology.** Assert every description is at least 120 characters and
    /// longer than its own display name.
    ///
    /// **Results (2026-08-12).** All 33 passed. Shortest description was 306
    /// characters (`HTGR.Diagnostics.SimulationTimeSecond`, the only node whose
    /// fidelity clause is the short `ExactDiagnostic` one), longest 742. The
    /// 120-character floor is therefore slack by design — it is there to catch
    /// a future one-line placeholder, not to police prose length.
    #[test]
    fn every_node_has_a_substantive_description() {
        for node in all_nodes() {
            assert!(
                node.description.len() >= 120,
                "description of {} is too thin: {:?}",
                node.node_identifier,
                node.description
            );
            assert!(node.description.len() > node.display_name.len());
        }
    }

    /// A quantity the simulator's own docs call illustrative, or that does not
    /// move at all, must say so where a client will read it. Publishing a
    /// placeholder as though it were measured is the precise failure mode
    /// `RESPONSIBLE_USE.md` exists to prevent.
    ///
    /// **Methodology.** For every signal: if its fidelity is
    /// `IllustrativeParameters` the description must contain `ILLUSTRATIVE`; if
    /// `varies_during_run()` is false it must contain `HELD CONSTANT`; and
    /// every description, whatever its fidelity, must contain `MODEL FIDELITY`.
    ///
    /// **Results (2026-08-12).** 23 of 31 signals are flagged ILLUSTRATIVE,
    /// 5 are flagged HELD CONSTANT (steam pressure, condenser pressure,
    /// feedwater and condensate enthalpy, delayed-neutron fraction), and all 31
    /// carry a MODEL FIDELITY clause.
    #[test]
    fn illustrative_and_constant_quantities_are_flagged_in_their_descriptions() {
        let mut illustrative = 0;
        let mut held_constant = 0;
        for signal in HtgrSignal::ALL {
            let description = signal.description();
            assert!(
                description.contains("MODEL FIDELITY"),
                "{signal:?} has no fidelity clause"
            );
            if signal.fidelity() == ModelFidelity::IllustrativeParameters {
                illustrative += 1;
                assert!(
                    description.contains("ILLUSTRATIVE"),
                    "{signal:?} is illustrative but does not say so"
                );
            }
            if !signal.varies_during_run() {
                held_constant += 1;
                assert!(
                    description.contains("HELD CONSTANT"),
                    "{signal:?} never varies but does not say so"
                );
            }
        }
        assert_eq!(illustrative, 23, "illustrative signal count changed");
        assert_eq!(held_constant, 5, "held-constant signal count changed");
    }

    /// A write outside a control's documented envelope must be clamped to the
    /// envelope, not stored raw — the envelope is what keeps a client from
    /// driving the solver somewhere it cannot follow.
    ///
    /// **Methodology.** For each control, write `min - 1e6` and `max + 1e6`,
    /// then read back. Pass criterion: exact equality with the documented
    /// bounds (both fields are `f64`, so no `f32` read-back slack is needed).
    ///
    /// **Results (2026-08-12).** Both controls clamped exactly at both ends:
    /// reactivity to [-2, +1] $, helium flow to [10, 150] kg/s.
    #[test]
    fn controls_clamp_out_of_range_writes() {
        for control in HtgrControl::ALL {
            let (min, max) = control.valid_range();
            let mut snapshot = HtgrPlantSnapshot::default();

            let low = control.write(&mut snapshot, min - 1.0e6);
            assert_eq!(low, min, "{control:?} did not clamp at its minimum");

            let high = control.write(&mut snapshot, max + 1.0e6);
            assert_eq!(high, max, "{control:?} did not clamp at its maximum");
        }
    }

    /// A NaN write must leave the set point untouched. A NaN reactivity would
    /// propagate into the kinetics and destroy the whole run, and OPC-UA
    /// happily carries a NaN `Double`.
    ///
    /// **Methodology.** Write a mid-range value, then `f64::NAN`, then read
    /// back. Pass criterion: the read-back is the mid-range value and is
    /// finite.
    ///
    /// **Results (2026-08-12).** Both controls retained their prior value and
    /// stayed finite after a NaN write.
    #[test]
    fn nan_writes_are_ignored() {
        for control in HtgrControl::ALL {
            let (min, max) = control.valid_range();
            let midpoint = 0.5 * (min + max);
            let mut snapshot = HtgrPlantSnapshot::default();

            control.write(&mut snapshot, midpoint);
            let after_nan = control.write(&mut snapshot, f64::NAN);

            assert!(after_nan.is_finite(), "{control:?} became non-finite");
            assert_eq!(after_nan, midpoint, "{control:?} moved on a NaN write");
        }
    }

    /// A read-only signal must stay read-only however it is reached — including
    /// through the type-erased [`NodeSource`] a transport layer iterates.
    ///
    /// **Methodology.** For every node descriptor, attempt a write through
    /// `source.write`. Pass criterion: `None` for every `ReadOnly` node and
    /// `Some` for every `ReadWrite` node, and no `ReadOnly` node's value
    /// changes.
    ///
    /// **Results (2026-08-12).** 31 read-only nodes refused the write and kept
    /// their value; both controls accepted it.
    #[test]
    fn read_only_nodes_reject_writes_through_the_descriptor() {
        for node in all_nodes() {
            let mut snapshot = HtgrPlantSnapshot::default();
            let before = node.source.read(&snapshot);
            let outcome = node.source.write(&mut snapshot, 42.0);
            match node.access {
                NodeAccess::ReadOnly => {
                    assert!(
                        outcome.is_none(),
                        "{} accepted a write",
                        node.node_identifier
                    );
                    assert_eq!(node.source.read(&snapshot), before);
                }
                NodeAccess::ReadWrite => {
                    assert!(
                        outcome.is_some(),
                        "{} refused a write",
                        node.node_identifier
                    );
                }
            }
        }
    }

    /// The descriptor list must mirror the enums exactly — it is what a server
    /// iterates to build the address space, so anything missing from it is
    /// missing from the twin.
    ///
    /// **Methodology.** Compare `all_nodes()` against the enum tables by
    /// length, access split, and range presence.
    ///
    /// **Results (2026-08-12).** 33 descriptors: 31 read-only with no write
    /// range, 2 read/write with one.
    #[test]
    fn descriptor_list_mirrors_the_enums() {
        let nodes = all_nodes();
        assert_eq!(nodes.len(), total_node_count());
        assert_eq!(nodes.len(), 33);

        let read_only = nodes
            .iter()
            .filter(|node| node.access == NodeAccess::ReadOnly)
            .count();
        assert_eq!(read_only, HtgrSignal::ALL.len());

        for node in &nodes {
            match node.access {
                NodeAccess::ReadOnly => {
                    assert!(node.valid_range.is_none());
                    assert!(node.fidelity.is_some());
                }
                NodeAccess::ReadWrite => {
                    assert!(node.valid_range.is_some());
                    assert!(node.fidelity.is_none());
                }
            }
        }
    }

    /// `HtgrControl::index` must be a dense index into `ALL`, since a transport
    /// layer uses it to address fixed-size pending-write arrays.
    ///
    /// **Methodology.** Check `ALL[control.index()] == control` for every
    /// control.
    ///
    /// **Results (2026-08-12).** Both controls round-tripped.
    #[test]
    fn control_indices_address_their_own_slot() {
        for control in HtgrControl::ALL {
            assert!(control.index() < HtgrControl::ALL.len());
            assert_eq!(&HtgrControl::ALL[control.index()], control);
        }
    }

    /// Every subsystem folder must actually hold nodes, and every node's folder
    /// must be a declared subsystem — an empty folder in a browse tree reads as
    /// a broken server.
    ///
    /// **Methodology.** Count nodes per subsystem over `all_nodes()`.
    ///
    /// **Results (2026-08-12).** Kinetics 6, Primary 7, Ihx 2, Secondary 15,
    /// Diagnostics 1, Controls 2 — 33 in total, no empty folder.
    #[test]
    fn every_subsystem_folder_is_populated() {
        let nodes = all_nodes();
        let mut total = 0;
        for subsystem in Subsystem::ALL {
            let count = nodes
                .iter()
                .filter(|node| node.subsystem == *subsystem)
                .count();
            assert!(count > 0, "{subsystem:?} folder would be empty");
            total += count;
        }
        assert_eq!(total, nodes.len(), "a node sits outside every folder");
    }
}
