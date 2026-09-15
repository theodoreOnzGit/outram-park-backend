//! Water electrolyzer: Faraday-law hydrogen production with a cell-voltage
//! and waste-heat energy balance.
//!
//! # Attribution
//!
//! - **Upstream project:** DWSIM — Open Source Process Simulator
//! - **Source file:** `DWSIM.UnitOperations/UnitOperations/CleanEnergies/WaterElectrolyzer.vb`
//! - **Upstream commit:** `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`)
//! - **Upstream copyright:** Daniel Wagner O. de Medeiros and the DWSIM contributors
//! - **Upstream licence:** GPL-3.0
//! - **This port:** GPL-3.0-only (OUTRAM PARK fork; not the official DWSIM software)
//!
//! Everything below translates `Public Overrides Sub Calculate`
//! (`WaterElectrolyzer.vb:394-565`) plus the property declarations at
//! `:25-74`.
//!
//! # What the unit models
//!
//! Liquid water is split electrochemically into hydrogen and oxygen,
//!
//! `H2O(l) -> H2(g) + 0.5 O2(g)`,
//!
//! driven by an electrical power input. Two outlet streams leave: a
//! hydrogen-rich stream saturated with water vapour, and an oxygen-rich
//! stream carrying the unreacted water. DWSIM supports light water
//! (`Water` / `Hydrogen`) and heavy water (`HeavyWater` / `Deuterium`) —
//! see [`ElectrolysisChemistry`].
//!
//! # The voltage chain
//!
//! Three voltages matter, all per **cell** (not per stack):
//!
//! - **Reversible (Nernst) voltage** `V_rev = dG / (n F)` with `n = 2`
//!   electrons per H2 — the thermodynamic minimum. Below it electrolysis
//!   cannot run at all ([`reversible_voltage`], `WaterElectrolyzer.vb:448`).
//! - **Thermoneutral voltage** `V_th = dH / (n F)` — the voltage at which the
//!   electrical input exactly equals the reaction enthalpy, so the cell
//!   neither absorbs nor releases net heat ([`thermoneutral_voltage`],
//!   `:449`). `V_th > V_rev` because `dH > dG` for this reaction.
//! - **Actual cell voltage** `V_cell = V_stack / N`. Everything above `V_th`
//!   becomes waste heat (`:471-473`).
//!
//! # Where the reaction enthalpy comes from (flash boundary)
//!
//! DWSIM obtains `dH` from its property package
//! (`AUX_DELHig_RT` for the ideal-gas reaction enthalpy plus `AUX_HVAPi` for
//! the water heat of vaporization, `:432, :438-440`), and the water vapour
//! pressure from `AUX_PVAPi` (`:523`). Following the crate convention
//! ([`crate::mixer`], [`crate::heater`]), those property-package calls are
//! **not** made here: `dH` and the vapour pressure are inputs. See
//! [`reaction_enthalpy_kj_per_mol`] for how DWSIM assembles `dH`, so a caller
//! can reproduce it from [`crate::thermo`] or any other source.
//!
//! # A note on how DWSIM computes `dG`
//!
//! `WaterElectrolyzer.vb:431` calls `AUX_DELGig_RT` to get an ideal-gas
//! reaction Gibbs energy — and then `:446` **overwrites** that variable with
//!
//! `dG = dH + T * (S_water - (0.5 * S_oxygen + S_hydrogen)) / 1000`
//!
//! built from NIST Shomate entropy correlations (`:443-445`). The
//! `AUX_DELGig_RT` result is therefore dead code upstream, and this port does
//! not reproduce it. Note also the **sign convention**: the bracket is
//! `S_reactant - S_products`, i.e. `-dS_rxn`, so the expression is the usual
//! `dG = dH - T dS` written with the subtraction folded into the bracket.
//! That is reproduced verbatim in [`reaction_gibbs_energy_kj_per_mol`].
//!
//! # Excluded DWSIM behaviour
//!
//! Beyond the module-wide exclusions listed in
//! [`crate::clean_energies`], this file drops:
//!
//! - the compound-presence validation and name lookup by string
//!   (`:406-421`) — replaced by the typed [`ElectrolysisChemistry`] enum and
//!   by the caller owning its own component list;
//! - the property-package calls `AUX_DELGig_RT` / `AUX_DELHig_RT` /
//!   `AUX_HVAPi` / `AUX_PVAPi` (`:423-438, :523`) — pushed to the caller as
//!   described above;
//! - the outlet material-stream mutation and PT/PH flash invocation
//!   (`:530-563`) — the flash boundary. [`hydrogen_outlet_split`] and
//!   [`OutletEnthalpyBump`] return exactly the numbers DWSIM writes onto the
//!   streams before flashing.

use uom::si::catalytic_activity::katal;
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::f64::{
    CatalyticActivity, ElectricCurrent, ElectricPotential, Power, Pressure, Ratio,
    ThermodynamicTemperature,
};
use uom::si::power::{kilowatt, watt};
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use super::{CleanEnergyError, CleanEnergyUnitOp, FARADAY_CONSTANT_C_PER_MOL};

/// Molar flow rate \[mol/s\]. `uom` 0.38 has no dedicated `MolarFlowRate`
/// quantity; mol/s is dimensionally the katal, so this alias matches
/// [`crate::separator::MolarFlowRate`] and [`crate::splitter::MolarFlowRate`].
pub type MolarFlowRate = CatalyticActivity;

/// Electrons transferred per molecule of H2 produced — `n = 2` in
/// `dG / (n F)`, hard-coded as the literal `2.0` at
/// `WaterElectrolyzer.vb:448-449`.
pub const ELECTRONS_PER_HYDROGEN: f64 = 2.0;

/// Electrolyzer cell technology — DWSIM's `EquipmentTypes` list
/// (`WaterElectrolyzer.vb:25-29`: `{"", "PEM", "Alkaline", "Solid Oxide"}`).
///
/// **This selection is descriptive only.** DWSIM's `Calculate` never branches
/// on it — the same Faraday/voltage arithmetic runs for all three — so it
/// changes no number in this port either. It is carried so the flowsheet
/// metadata survives the translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElectrolyzerTechnology {
    /// DWSIM's empty first entry — no technology chosen.
    #[default]
    Unspecified,
    /// Proton-exchange-membrane electrolysis.
    Pem,
    /// Alkaline (KOH/NaOH) electrolysis.
    Alkaline,
    /// Solid-oxide (high-temperature steam) electrolysis.
    SolidOxide,
}

/// Which isotopic chemistry the cell runs — DWSIM's `HeavyWater`/`Deuterium`
/// vs `Water`/`Hydrogen` compound selection
/// (`WaterElectrolyzer.vb:406-421`).
///
/// DWSIM picks by inspecting the inlet compound names: if `HeavyWater` is
/// present it demands `Deuterium` too and runs heavy-water electrolysis;
/// otherwise it demands `Water` and `Hydrogen`. `Oxygen` is required either
/// way. The arithmetic is identical for both — only the compound names the
/// caller must supply differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElectrolysisChemistry {
    /// `H2O -> H2 + 0.5 O2` (DWSIM compounds `Water`, `Hydrogen`, `Oxygen`).
    #[default]
    LightWater,
    /// `D2O -> D2 + 0.5 O2` (DWSIM compounds `HeavyWater`, `Deuterium`,
    /// `Oxygen`).
    HeavyWater,
}

impl ElectrolysisChemistry {
    /// DWSIM's `wid` — the compound name of the water being split
    /// (`WaterElectrolyzer.vb:412, :417`).
    pub fn water_compound(&self) -> &'static str {
        match self {
            Self::LightWater => "Water",
            Self::HeavyWater => "HeavyWater",
        }
    }

    /// DWSIM's `hid` — the compound name of the hydrogen isotopologue
    /// produced (`WaterElectrolyzer.vb:413, :418`).
    pub fn hydrogen_compound(&self) -> &'static str {
        match self {
            Self::LightWater => "Hydrogen",
            Self::HeavyWater => "Deuterium",
        }
    }

    /// The oxidant compound name, `"Oxygen"` for both chemistries
    /// (`WaterElectrolyzer.vb:421`).
    pub fn oxygen_compound(&self) -> &'static str {
        "Oxygen"
    }
}

// ---------------------------------------------------------------------------
// NIST Shomate absolute entropies
// ---------------------------------------------------------------------------

/// Shomate absolute molar entropy \[J/(mol·K)\] from the coefficient set
/// `(a, b, c, d, e, g)`:
///
/// `S = a ln(t) + b t + c t^2 / 2 + d t^3 / 3 - e / (2 t^2) + g`,  with `t = T / 1000`.
///
/// This is the standard NIST Chemistry WebBook Shomate entropy form, written
/// exactly as DWSIM inlines it three times at `WaterElectrolyzer.vb:443-445`.
/// Factored into one function here so the three species differ only by their
/// coefficients.
///
/// `temperature_k` is in kelvin and must be `> 0` (a logarithm is taken).
/// Shomate fits are valid only over the temperature window quoted for each
/// species — see the per-species wrappers.
fn shomate_entropy_j_per_mol_k(
    temperature_k: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    g: f64,
) -> f64 {
    let t = temperature_k / 1000.0;
    a * t.ln() + b * t + c * t * t / 2.0 + d * t * t * t / 3.0 - e / (2.0 * t * t) + g
}

/// Absolute molar entropy of **liquid water** \[J/(mol·K)\] at
/// `temperature_k` \[K\], from the NIST Shomate coefficients DWSIM inlines at
/// `WaterElectrolyzer.vb:443`.
///
/// Coefficients `(-203.606, 1523.29, -3196.413, 2474.455, 3.855326, -488.7163)`.
/// The NIST fit for liquid water covers roughly **298 K to 500 K**; outside
/// that the polynomial extrapolates and should not be trusted. Note the
/// unusually large coefficients — they are the published liquid-phase set,
/// not the gas-phase one, which is why the heat of vaporization must be added
/// to `dH` separately (see [`reaction_enthalpy_kj_per_mol`]).
pub fn liquid_water_entropy_j_per_mol_k(temperature_k: f64) -> f64 {
    shomate_entropy_j_per_mol_k(
        temperature_k,
        -203.606,
        1523.29,
        -3196.413,
        2474.455,
        3.855326,
        -488.7163,
    )
}

/// Absolute molar entropy of **gaseous hydrogen** \[J/(mol·K)\] at
/// `temperature_k` \[K\], from the NIST Shomate coefficients DWSIM inlines at
/// `WaterElectrolyzer.vb:444`.
///
/// NIST coefficients `A = 33.066178`, `B = -11.363417`, `C = 11.432816`,
/// `D = -2.772874`, **`E = -0.158558`**, `G = 172.707974`.
/// The NIST fit covers **298 K to 1000 K**. At 298.15 K this reproduces the
/// tabulated `S°(H2, g) = 130.68 J/(mol·K)` DWSIM cites in its comment at
/// `:441`.
///
/// Note the **negative `E`**: DWSIM writes this term as
/// `+ 0.158558 / (2 t²)` (`:444`), which is the standard `-E/(2 t²)` form
/// with the sign of the negative NIST coefficient already folded in. Passing
/// `+0.158558` to [`shomate_entropy_j_per_mol_k`] here would flip that term
/// and put the 298.15 K entropy 1.79 J/(mol·K) low.
pub fn hydrogen_gas_entropy_j_per_mol_k(temperature_k: f64) -> f64 {
    shomate_entropy_j_per_mol_k(
        temperature_k,
        33.066178,
        -11.363417,
        11.432816,
        -2.772874,
        -0.158558,
        172.707974,
    )
}

/// Absolute molar entropy of **gaseous oxygen** \[J/(mol·K)\] at
/// `temperature_k` \[K\], from the NIST Shomate coefficients DWSIM inlines at
/// `WaterElectrolyzer.vb:445`.
///
/// NIST coefficients `A = 31.32234`, `B = -20.23531`, `C = 57.86644`,
/// `D = -36.50624`, **`E = -0.007374`**, `G = 246.7945`.
/// The NIST fit covers **100 K to 700 K**. At 298.15 K this reproduces the
/// tabulated `S°(O2, g) = 205.15 J/(mol·K)` DWSIM cites in its comment at
/// `:441`.
///
/// As for hydrogen, the `E` coefficient is **negative** — DWSIM writes
/// `+ 0.007374 / (2 t²)` (`:445`) for the standard `-E/(2 t²)` term. Liquid
/// water is the only one of the three with a positive `E`, which is why
/// `:443` alone shows a minus sign.
pub fn oxygen_gas_entropy_j_per_mol_k(temperature_k: f64) -> f64 {
    shomate_entropy_j_per_mol_k(
        temperature_k,
        31.32234,
        -20.23531,
        57.86644,
        -36.50624,
        -0.007374,
        246.7945,
    )
}

// ---------------------------------------------------------------------------
// Reaction thermodynamics
// ---------------------------------------------------------------------------

/// Assemble the reaction enthalpy `dH` \[kJ/mol H2\] the way DWSIM does
/// (`WaterElectrolyzer.vb:432, :438-440`):
///
/// `dH = dH_ig_rxn + dH_vap(water)`
///
/// where `dH_ig_rxn` is the **ideal-gas** enthalpy of
/// `H2O -> H2 + 0.5 O2` (DWSIM: `AUX_DELHig_RT(298.15, T, [w, h, O2],
/// [-1, 1, 0.5], 0) * 8.314 * T / 1000`, already in kJ/mol) and `dH_vap` is
/// water's heat of vaporization at `T` (DWSIM: `AUX_HVAPi(wid, T) * MW / 1000`,
/// converting a mass-basis kJ/kg to a molar kJ/mol).
///
/// Adding `dH_vap` moves the reference for the water reactant from vapour to
/// **liquid**, which is what makes `dH` the liquid-feed reaction enthalpy —
/// about **286 kJ/mol** at 298 K (the higher heating value of hydrogen).
///
/// Both inputs are in kJ/mol and both must come from the caller's property
/// package: this crate does not evaluate them (see the module "flash
/// boundary" note). The function is a one-line sum, provided so the
/// assembly rule is documented and testable rather than folded silently into
/// a caller.
pub fn reaction_enthalpy_kj_per_mol(
    ideal_gas_reaction_enthalpy_kj_per_mol: f64,
    water_heat_of_vaporization_kj_per_mol: f64,
) -> f64 {
    ideal_gas_reaction_enthalpy_kj_per_mol + water_heat_of_vaporization_kj_per_mol
}

/// Reaction Gibbs energy `dG` \[kJ/mol H2\] at `temperature_k` \[K\], from the
/// reaction enthalpy and the three Shomate entropies
/// (`WaterElectrolyzer.vb:446`):
///
/// `dG = dH + T * (S_water - (0.5 S_oxygen + S_hydrogen)) / 1000`
///
/// The bracket is `S_reactant - S_products = -dS_rxn`, so this is `dG = dH - T dS`
/// with the sign folded in; the `/1000` converts the J/(mol·K) entropies to
/// kJ/(mol·K) so the result stays in kJ/mol. Because `dS_rxn > 0` (a liquid
/// becomes 1.5 mol of gas), `dG < dH` and hence `V_rev < V_th`.
///
/// `delta_h_kj_per_mol` is `dH` from [`reaction_enthalpy_kj_per_mol`].
/// Valid over the intersection of the three Shomate windows, i.e. roughly
/// **298 K to 500 K**.
pub fn reaction_gibbs_energy_kj_per_mol(delta_h_kj_per_mol: f64, temperature_k: f64) -> f64 {
    let s_water = liquid_water_entropy_j_per_mol_k(temperature_k);
    let s_hydrogen = hydrogen_gas_entropy_j_per_mol_k(temperature_k);
    let s_oxygen = oxygen_gas_entropy_j_per_mol_k(temperature_k);
    delta_h_kj_per_mol + temperature_k * (s_water - (0.5 * s_oxygen + s_hydrogen)) / 1000.0
}

/// Reversible (Nernst) cell voltage `V_rev = dG / (n F)` \[V\]
/// (`WaterElectrolyzer.vb:448`, `Vrev = DGf * 1000.0 / (2.0 * 96485.3365)`).
///
/// `delta_g_kj_per_mol` is the reaction Gibbs energy in kJ/mol H2 (the
/// `*1000` converts it to J/mol); `n = 2` electrons per H2
/// ([`ELECTRONS_PER_HYDROGEN`]) and `F` is [`FARADAY_CONSTANT_C_PER_MOL`].
///
/// This is the thermodynamic floor: a cell driven below `V_rev` cannot split
/// water at all. For liquid water at 298 K it is about **1.23 V**.
pub fn reversible_voltage(delta_g_kj_per_mol: f64) -> ElectricPotential {
    ElectricPotential::new::<volt>(
        delta_g_kj_per_mol * 1000.0 / (ELECTRONS_PER_HYDROGEN * FARADAY_CONSTANT_C_PER_MOL),
    )
}

/// Thermoneutral cell voltage `V_th = dH / (n F)` \[V\]
/// (`WaterElectrolyzer.vb:449`, `Vth = DHf * 1000.0 / (2.0 * 96485.3365)`).
///
/// At exactly `V_th` the electrical power input equals the reaction enthalpy,
/// so the cell exchanges no net heat with its surroundings. Driven above it
/// the cell heats up (the excess is [`ElectrolyzerResult::waste_heat`]);
/// between `V_rev` and `V_th` an ideal cell would *absorb* heat.
///
/// `delta_h_kj_per_mol` is `dH` in kJ/mol H2. For liquid water at 298 K
/// `V_th` is about **1.48 V**.
pub fn thermoneutral_voltage(delta_h_kj_per_mol: f64) -> ElectricPotential {
    ElectricPotential::new::<volt>(
        delta_h_kj_per_mol * 1000.0 / (ELECTRONS_PER_HYDROGEN * FARADAY_CONSTANT_C_PER_MOL),
    )
}

// ---------------------------------------------------------------------------
// Specification and result
// ---------------------------------------------------------------------------

/// How the electrolyzer's operating point is specified — DWSIM's two mutually
/// exclusive branches at `WaterElectrolyzer.vb:459` and `:476`, with the
/// `Else` at `:492-495` becoming
/// [`CleanEnergyError::UnderspecifiedElectrolyzer`].
///
/// Modelled as an enum rather than DWSIM's "set these to zero and fill in
/// that one" convention, so an invalid combination cannot be represented.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ElectrolyzerSpecification {
    /// **Voltage branch** (`:459-474`). The user gives the total stack
    /// voltage and the cell count; current, production rates, cell voltage
    /// and waste heat all follow from the power input.
    ///
    /// DWSIM's guard is `Voltage > 0 And NumberOfCells > 0`, so both must be
    /// strictly positive.
    VoltageAndCells {
        /// Total stack voltage `V_stack` \[V\], `> 0`. Cells are in series,
        /// so `V_cell = V_stack / N`.
        stack_voltage: ElectricPotential,
        /// Number of cells in the stack `N`, `>= 1`.
        number_of_cells: u32,
    },
    /// **Efficiency branch** (`:476-490`). The user gives an overall
    /// efficiency instead; DWSIM then splits the power input into a reaction
    /// share and a waste-heat share, back-calculates the cell voltage as
    /// `V_th / eta`, and reports zero current (there is no voltage to divide
    /// the power by).
    ///
    /// DWSIM's guard is `InputEfficiency > 0 And InputEfficiency <= 1.0`, so
    /// this is a fraction in `(0, 1]`, not a percentage.
    Efficiency(Ratio),
}

/// Everything DWSIM's electrolyzer `Calculate` computes, in one owned struct.
///
/// The field names mirror the upstream properties at
/// `WaterElectrolyzer.vb:56-74` so the mapping is checkable line by line.
/// Consumption/production rates are **molar** flows in mol/s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElectrolyzerResult {
    /// Stack current `I` \[A\] (`:461`, `Current = P/V`). **Zero** in the
    /// efficiency branch, which DWSIM sets explicitly at `:490`.
    pub current: ElectricCurrent,
    /// Per-cell voltage `V_cell` \[V\] — `V_stack / N` in the voltage branch
    /// (`:468`), `V_th / eta` in the efficiency branch (`:487`).
    pub cell_voltage: ElectricPotential,
    /// Total electron throughput \[mol e⁻/s\] — `I N / F` in the voltage
    /// branch (`:463`), `2 * n_H2O` in the efficiency branch (`:489`).
    pub electron_transfer: MolarFlowRate,
    /// Water consumed \[mol/s\] (`:465`, `:483`). Subtracted from the inlet
    /// water flow.
    pub water_consumption: MolarFlowRate,
    /// Hydrogen produced \[mol/s\] (`:466`, `:484`). Equals the water
    /// consumption — one H2 per H2O.
    pub hydrogen_production: MolarFlowRate,
    /// Oxygen produced \[mol/s\] (`:467`, `:485`). Half the hydrogen rate.
    pub oxygen_production: MolarFlowRate,
    /// Waste heat `Q_waste` \[W\] released by the cell (`:473`, `:481`).
    /// Positive means heat leaving the electrochemistry into the streams.
    pub waste_heat: Power,
    /// Overall efficiency `(P - Q_waste) / P`, dimensionless (`:498`). In
    /// the efficiency branch this reproduces the user's input exactly.
    pub efficiency: Ratio,
    /// Reversible voltage `V_rev` \[V\] at the operating temperature
    /// (`:453`), carried through for reporting.
    pub reversible_voltage: ElectricPotential,
    /// Thermoneutral voltage `V_th` \[V\] at the operating temperature
    /// (`:451`), carried through for reporting.
    pub thermoneutral_voltage: ElectricPotential,
}

/// Solve the electrolyzer's electrochemical balance —
/// `WaterElectrolyzer.vb:459-498`.
///
/// # Inputs
///
/// - `power_input` — electrical power delivered to the stack \[W\]. DWSIM
///   reads this from the inlet energy stream in kW
///   (`esin.EnergyFlow`, `:461`); the `uom` boundary here handles the
///   scaling. Must be `> 0` for a meaningful result.
/// - `spec` — the operating specification, see [`ElectrolyzerSpecification`].
/// - `reversible_voltage` — `V_rev` \[V\] from [`reversible_voltage`].
/// - `thermoneutral_voltage` — `V_th` \[V\] from [`thermoneutral_voltage`].
/// - `delta_h_kj_per_mol` — reaction enthalpy `dH` \[kJ/mol H2\] from
///   [`reaction_enthalpy_kj_per_mol`]. Used **only** by the efficiency
///   branch, where the production rate is `reaction_heat / dH` (`:483-485`);
///   the voltage branch ignores it.
///
/// # Faraday stoichiometry
///
/// In the voltage branch DWSIM computes the electron throughput as
/// `n_e = I N / F` and then
///
/// - `n_H2O = n_e / 4 * 2 = n_e / 2` (`:465`)
/// - `n_H2  = n_e / 4 * 2 = n_e / 2` (`:466`)
/// - `n_O2  = n_e / 4`               (`:467`)
///
/// i.e. two electrons per H2 and four per O2 — textbook Faraday
/// stoichiometry. The `/4*2` spelling is upstream's; it is kept as-is in the
/// implementation so the correspondence is visible.
///
/// # Waste heat
///
/// The voltage branch's `Q_waste = (V_cell - V_th) * I * N` (`:473`, upstream
/// divides by 1000 to reach kW) is algebraically `P - N V_th I`: the input
/// power minus the part that goes into the reaction. It is therefore
/// negative when `V_cell < V_th`, correctly representing a cell that draws
/// heat from its surroundings — DWSIM does not clamp it, and neither does
/// this port.
///
/// # Errors
///
/// - [`CleanEnergyError::CellVoltageBelowReversible`] if `V_cell < V_rev`
///   (`:469`).
/// - [`CleanEnergyError::UnderspecifiedElectrolyzer`] if the voltage branch
///   is given a non-positive voltage or zero cells, or the efficiency branch
///   an efficiency outside `(0, 1]` (`:494`).
pub fn calculate(
    power_input: Power,
    spec: ElectrolyzerSpecification,
    reversible_voltage: ElectricPotential,
    thermoneutral_voltage: ElectricPotential,
    delta_h_kj_per_mol: f64,
) -> Result<ElectrolyzerResult, CleanEnergyError> {
    // DWSIM's energy stream is in kW throughout this routine.
    let power_kw = power_input.get::<kilowatt>();
    let v_rev = reversible_voltage.get::<volt>();
    let v_th = thermoneutral_voltage.get::<volt>();

    let (current_a, electron_transfer, water_rate, h2_rate, o2_rate, cell_voltage_v, waste_heat_kw) =
        match spec {
            ElectrolyzerSpecification::VoltageAndCells {
                stack_voltage,
                number_of_cells,
            } => {
                let v_stack = stack_voltage.get::<volt>();
                // WaterElectrolyzer.vb:459 -- `Voltage > 0 And NumberOfCells > 0`.
                if !(v_stack > 0.0) || number_of_cells == 0 {
                    return Err(CleanEnergyError::UnderspecifiedElectrolyzer);
                }
                let n_cells = f64::from(number_of_cells);

                // :461 -- Current = EnergyFlow[kW] * 1000 / Voltage  -> amperes.
                let current = power_kw * 1000.0 / v_stack;
                // :463 -- ElectronTransfer = Current / F * N  -> mol e-/s.
                let electron_transfer = current / FARADAY_CONSTANT_C_PER_MOL * n_cells;
                // :465-467 -- Faraday stoichiometry, upstream spelling kept.
                let waterr = electron_transfer / 4.0 * 2.0;
                let h2r = electron_transfer / 4.0 * 2.0;
                let o2r = electron_transfer / 4.0;

                // :468 -- cells are in series, so each sees V_stack / N.
                let cell_voltage = v_stack / n_cells;
                // :469 -- below the reversible voltage nothing can happen.
                if cell_voltage < v_rev {
                    return Err(CleanEnergyError::CellVoltageBelowReversible {
                        cell_voltage_v: cell_voltage,
                        reversible_voltage_v: v_rev,
                    });
                }

                // :471-473 -- overpotential above thermoneutral becomes heat.
                let over_v = cell_voltage - v_th;
                let waste_heat = over_v * current * n_cells / 1000.0; // kW

                (
                    current,
                    electron_transfer,
                    waterr,
                    h2r,
                    o2r,
                    cell_voltage,
                    waste_heat,
                )
            }
            ElectrolyzerSpecification::Efficiency(eff) => {
                let eta = eff.get::<ratio>();
                // WaterElectrolyzer.vb:476 -- `InputEfficiency > 0 And <= 1.0`.
                if !(eta > 0.0) || eta > 1.0 {
                    return Err(CleanEnergyError::UnderspecifiedElectrolyzer);
                }
                // :480-481 -- split the input power into reaction and waste.
                let reaction_heat = eta * power_kw; // kW
                let waste_heat = (1.0 - eta) * power_kw; // kW

                // :483-485 -- kW / (kJ/mol) = mol/s.
                let waterr = reaction_heat / delta_h_kj_per_mol;
                let h2r = reaction_heat / delta_h_kj_per_mol;
                let o2r = 0.5 * reaction_heat / delta_h_kj_per_mol;

                // :487-491 -- cell voltage back-calculated; current is
                // explicitly zeroed upstream because no voltage was given.
                let cell_voltage = v_th / eta;
                let electron_transfer = 2.0 * waterr;

                (
                    0.0,
                    electron_transfer,
                    waterr,
                    h2r,
                    o2r,
                    cell_voltage,
                    waste_heat,
                )
            }
        };

    // :498 -- Efficiency = (EnergyFlow - WasteHeat) / EnergyFlow.
    let efficiency = (power_kw - waste_heat_kw) / power_kw;

    Ok(ElectrolyzerResult {
        current: ElectricCurrent::new::<ampere>(current_a),
        cell_voltage: ElectricPotential::new::<volt>(cell_voltage_v),
        electron_transfer: MolarFlowRate::new::<katal>(electron_transfer),
        water_consumption: MolarFlowRate::new::<katal>(water_rate),
        hydrogen_production: MolarFlowRate::new::<katal>(h2_rate),
        oxygen_production: MolarFlowRate::new::<katal>(o2_rate),
        waste_heat: Power::new::<kilowatt>(waste_heat_kw),
        efficiency: Ratio::new::<ratio>(efficiency),
        reversible_voltage,
        thermoneutral_voltage,
    })
}

// ---------------------------------------------------------------------------
// Outlet composition
// ---------------------------------------------------------------------------

/// Apply the reaction extents to an inlet molar-flow vector —
/// `WaterElectrolyzer.vb:500-518`.
///
/// `inlet_molar_flows[i]` is component `i`'s inlet molar flow \[mol/s\];
/// `water_index`, `hydrogen_index` and `oxygen_index` locate the three
/// participating species in that vector (DWSIM finds them by name at
/// `:506-518`; the caller owns its component ordering here).
///
/// Returns the post-reaction vector: water down by `n_H2O`, hydrogen up by
/// `n_H2`, oxygen up by `n_O2`, everything else (inerts) untouched.
///
/// # Errors
///
/// [`CleanEnergyError::NegativeMolarFlow`] if the water flow goes negative —
/// DWSIM's "Negative {0} molar flow calculated. Increase water rate in inlet
/// stream or reduce power." (`:510`). Physically: the cell is being driven
/// harder than the water feed can sustain.
///
/// # Panics
///
/// Panics if any index is out of bounds for `inlet_molar_flows`.
pub fn apply_reaction_extents(
    inlet_molar_flows: &[f64],
    water_index: usize,
    hydrogen_index: usize,
    oxygen_index: usize,
    result: &ElectrolyzerResult,
    chemistry: ElectrolysisChemistry,
) -> Result<Vec<f64>, CleanEnergyError> {
    let mut flows = inlet_molar_flows.to_vec();
    flows[water_index] -= result.water_consumption.get::<katal>();
    if flows[water_index] < 0.0 {
        return Err(CleanEnergyError::NegativeMolarFlow {
            species: chemistry.water_compound(),
            molar_flow_mol_per_s: flows[water_index],
        });
    }
    flows[hydrogen_index] += result.hydrogen_production.get::<katal>();
    flows[oxygen_index] += result.oxygen_production.get::<katal>();
    Ok(flows)
}

/// Composition of the hydrogen-rich outlet, saturated with water vapour —
/// `WaterElectrolyzer.vb:520-526`.
///
/// DWSIM assumes the H2 stream leaves **saturated** with water at the cell
/// temperature and pressure:
///
/// - `x_H2O,sat = P_vap(water, T) / P`  (`:523`)
/// - `x_H2 = 1 - x_H2O,sat`             (`:524`)
/// - `n_total = n_H2 / x_H2`            (`:525`)
/// - `n_H2O,sat = n_total - n_H2`       (`:526`)
///
/// # Inputs
///
/// - `hydrogen_molar_flow` — the post-reaction hydrogen flow \[mol/s\]
///   (element `hydrogen_index` of [`apply_reaction_extents`]'s output).
/// - `water_vapour_pressure` — `P_vap(water, T)` \[Pa\]. DWSIM gets this from
///   `AUX_PVAPi`; supply it from [`crate::thermo::saturation`] or your own
///   property package (flash boundary — see the module docs).
/// - `pressure` — cell pressure `P` \[Pa\], taken by DWSIM from the inlet
///   stream (`:520`).
///
/// Returns `(n_H2, n_H2O,sat)` \[mol/s\] — precisely the two molar flows
/// DWSIM writes onto outlet 1 before its PT flash (`:532-533`).
///
/// # Errors
///
/// [`CleanEnergyError::OutOfDomain`] if `P_vap >= P`, i.e. the cell is at or
/// below the water's boiling pressure: `x_H2` would then be zero or negative
/// and the total flow would be infinite or negative. DWSIM does not guard
/// this and would silently produce a non-finite stream.
pub fn hydrogen_outlet_split(
    hydrogen_molar_flow: MolarFlowRate,
    water_vapour_pressure: Pressure,
    pressure: Pressure,
) -> Result<(MolarFlowRate, MolarFlowRate), CleanEnergyError> {
    let p = pressure.get::<pascal>();
    let p_vap = water_vapour_pressure.get::<pascal>();
    let x_h2o_sat = p_vap / p;
    let x_h2 = 1.0 - x_h2o_sat;
    if x_h2 <= 0.0 {
        return Err(CleanEnergyError::OutOfDomain {
            parameter: "water vapour pressure / cell pressure",
            value: x_h2o_sat,
            reason: "saturated water mole fraction reaches 1; the cell is at or below the \
                     boiling pressure, leaving no room for hydrogen",
        });
    }
    let n_h2 = hydrogen_molar_flow.get::<katal>();
    let n_total = n_h2 / x_h2;
    let n_h2o_sat = n_total - n_h2;
    Ok((
        MolarFlowRate::new::<katal>(n_h2),
        MolarFlowRate::new::<katal>(n_h2o_sat),
    ))
}

/// Composition of the oxygen-rich outlet — `WaterElectrolyzer.vb:545-553`.
///
/// After outlet 1 is drawn off, DWSIM zeroes the hydrogen entry and removes
/// the saturating water from the remaining vector; whatever is left (oxygen,
/// unreacted water, inerts) becomes outlet 2.
///
/// `post_reaction_flows` is [`apply_reaction_extents`]'s output;
/// `water_vapour_flow` is `n_H2O,sat` from [`hydrogen_outlet_split`].
/// Returns the outlet-2 molar-flow vector \[mol/s\]; the caller normalises it
/// to a composition and sums it for the total molar flow, as DWSIM does at
/// `:552-553`.
///
/// # Errors
///
/// [`CleanEnergyError::NegativeMolarFlow`] if removing the saturating water
/// drives the water flow negative — DWSIM's "Negative Water molar flow
/// calculated." (`:548`).
///
/// # Panics
///
/// Panics if either index is out of bounds.
pub fn oxygen_outlet_flows(
    post_reaction_flows: &[f64],
    water_index: usize,
    hydrogen_index: usize,
    water_vapour_flow: MolarFlowRate,
    chemistry: ElectrolysisChemistry,
) -> Result<Vec<f64>, CleanEnergyError> {
    let mut flows = post_reaction_flows.to_vec();
    // :545 -- all hydrogen leaves in outlet 1.
    flows[hydrogen_index] = 0.0;
    // :546 -- the saturating water left with outlet 1 too.
    flows[water_index] -= water_vapour_flow.get::<katal>();
    if flows[water_index] < 0.0 {
        return Err(CleanEnergyError::NegativeMolarFlow {
            species: chemistry.water_compound(),
            molar_flow_mol_per_s: flows[water_index],
        });
    }
    Ok(flows)
}

/// The waste-heat term DWSIM adds to each outlet's mass enthalpy before its
/// PH flash — `WaterElectrolyzer.vb:539-541` and `:559-561`.
///
/// Upstream computes a mass-flow ratio `wh = w_outlet / w_inlet` and then
/// writes `h_outlet := h_outlet + WasteHeat * wh`, splitting the waste heat
/// between the two product streams in proportion to their mass flows.
///
/// > **Dimensional warning — reproduced faithfully, not corrected.**
/// > `WasteHeat` is a *power* (kW) and `wh` is dimensionless, so
/// > `WasteHeat * wh` has units of kW while `h` is a specific enthalpy in
/// > kJ/kg. To be dimensionally consistent the term would have to be
/// > `WasteHeat * wh / w_outlet`, or equivalently `WasteHeat / w_inlet`.
/// > DWSIM's sibling PEM fuel cell does write the dimensionally correct form
/// > (`PEMFC_Amphlett.vb:266`, `+ WasteHeat / (w1 + w2)`), which makes the
/// > electrolyzer's version look like an upstream slip. It is **kept
/// > verbatim** here because this is a port, and silently "fixing" it would
/// > make the two codes disagree without any reference to say which is right.
/// > Callers who want the consistent form should use
/// > [`OutletEnthalpyBump::dimensionally_consistent`].
///
/// Both variants are returned so the choice is explicit and documented at the
/// call site rather than hidden.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutletEnthalpyBump {
    /// `WasteHeat[kW] * (w_outlet / w_inlet)`, interpreted as kJ/kg — DWSIM's
    /// literal expression (`:541`, `:561`). Dimensionally inconsistent; see
    /// the struct docs.
    pub dwsim_verbatim: f64,
    /// `WasteHeat[kW] / w_inlet[kg/s]` = kJ/kg — the dimensionally consistent
    /// reading of the same intent, matching the fuel cell's own formula.
    pub dimensionally_consistent: f64,
}

/// Compute both readings of the outlet enthalpy bump described in
/// [`OutletEnthalpyBump`] — `WaterElectrolyzer.vb:539-541, :559-561`.
///
/// - `waste_heat` — `Q_waste` \[W\] from [`ElectrolyzerResult::waste_heat`].
/// - `outlet_mass_flow`, `inlet_mass_flow` — \[kg/s\]; the ratio is DWSIM's
///   `wh1` / `wh2`.
///
/// Both returned values are in **kJ/kg**, matching DWSIM's mass-enthalpy
/// convention. Returns non-finite values if `inlet_mass_flow` is zero, which
/// is degenerate input.
pub fn outlet_enthalpy_bump(
    waste_heat: Power,
    outlet_mass_flow: f64,
    inlet_mass_flow: f64,
) -> OutletEnthalpyBump {
    let q_kw = waste_heat.get::<kilowatt>();
    let wh = outlet_mass_flow / inlet_mass_flow;
    OutletEnthalpyBump {
        dwsim_verbatim: q_kw * wh,
        dimensionally_consistent: q_kw / inlet_mass_flow,
    }
}

// ---------------------------------------------------------------------------
// Unit-operation struct
// ---------------------------------------------------------------------------

/// A configured water-electrolyzer unit operation — the ported subset of
/// DWSIM's `WaterElectrolyzer` class state (`WaterElectrolyzer.vb:25-74`).
///
/// The struct holds configuration plus the most recent solved result, exactly
/// as the .NET object holds its `Public Property` fields between solver
/// passes. It owns everything by value: no `Box`, no `Arc`, no lifetimes.
///
/// A default-constructed electrolyzer is unsolved — [`Self::last_result`] is
/// `None` and [`CleanEnergyUnitOp::generated_power`] reports 0 W.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WaterElectrolyzer {
    /// Cell technology (descriptive only — see [`ElectrolyzerTechnology`]).
    pub technology: ElectrolyzerTechnology,
    /// Light- or heavy-water chemistry (`:406-419`).
    pub chemistry: ElectrolysisChemistry,
    /// Electrical power drawn from the inlet energy stream \[W\]
    /// (`esin.EnergyFlow`, `:461`).
    pub power_input: Power,
    /// The most recent [`calculate`] result, or `None` if never solved.
    pub last_result: Option<ElectrolyzerResult>,
}

impl WaterElectrolyzer {
    /// Solve this electrolyzer and store the result in
    /// [`Self::last_result`].
    ///
    /// A thin wrapper over the free function [`calculate`] — see it for the
    /// full argument description, the Faraday stoichiometry, and the error
    /// conditions. Provided so the struct can be driven statefully the way
    /// DWSIM's `Calculate` drives the .NET object, while the free function
    /// stays available for callers who want no state at all.
    ///
    /// # Errors
    ///
    /// Propagates every error from [`calculate`]; on error
    /// [`Self::last_result`] is left untouched.
    pub fn solve(
        &mut self,
        spec: ElectrolyzerSpecification,
        reversible_voltage: ElectricPotential,
        thermoneutral_voltage: ElectricPotential,
        delta_h_kj_per_mol: f64,
    ) -> Result<ElectrolyzerResult, CleanEnergyError> {
        let result = calculate(
            self.power_input,
            spec,
            reversible_voltage,
            thermoneutral_voltage,
            delta_h_kj_per_mol,
        )?;
        self.last_result = Some(result);
        Ok(result)
    }
}

impl CleanEnergyUnitOp for WaterElectrolyzer {
    /// `"Water Electrolyzer"` — `WaterElectrolyzer.vb:46-48`.
    fn display_name(&self) -> &'static str {
        "Water Electrolyzer"
    }

    /// `"WE-"` — `WaterElectrolyzer.vb:54`.
    fn prefix(&self) -> &'static str {
        "WE-"
    }

    /// The electrolyzer is a power **consumer**, so this returns the negative
    /// of [`Self::power_input`] (watts) once the unit has been solved, and
    /// 0 W before that. DWSIM has no equivalent signed accessor — it simply
    /// draws from an inlet energy stream — so the sign convention here is
    /// this port's, chosen to keep [`CleanEnergyUnitOp::generated_power`]
    /// meaningful across generators and consumers alike.
    fn generated_power(&self) -> Power {
        match self.last_result {
            Some(_) => -self.power_input,
            None => Power::new::<watt>(0.0),
        }
    }
}

/// Convenience: the cell temperature at which DWSIM's Shomate correlations
/// are anchored, 298.15 K, used as the reference in `AUX_DELGig_RT` /
/// `AUX_DELHig_RT` (`WaterElectrolyzer.vb:431-432`).
pub fn reference_temperature() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(298.15)
}

#[cfg(test)]
mod tests {
    //! # Verification tests (methodology + measured results)
    //!
    //! **Verification, not validation.** These check that the port reproduces
    //! DWSIM's arithmetic and that the electrochemistry closes; they do not
    //! compare against experimental electrolyzer data, and no such comparison
    //! has been run. All numbers below were measured by running this suite on
    //! **2026-08-11** with `cargo test --release`.
    use super::*;
    use approx::assert_relative_eq;

    /// Reaction enthalpy of liquid-water electrolysis at 298.15 K, kJ/mol —
    /// hydrogen's higher heating value. Used as a physically sane `dH` for
    /// the tests below; sourced from the NIST Chemistry WebBook values DWSIM
    /// itself cites at `WaterElectrolyzer.vb:441-442` (public literature
    /// data, per the workspace data policy).
    const DH_298_KJ_PER_MOL: f64 = 285.83;

    /// Methodology: evaluate the three Shomate entropy correlations
    /// (`WaterElectrolyzer.vb:443-445`) at 298.15 K and compare against the
    /// tabulated NIST standard entropies DWSIM quotes in its own comment at
    /// `:441` — `S(H2, g) = 130.68` and `S(O2, g) = 205.15 J/(mol·K)`. This
    /// is a self-consistency check of the transcribed coefficients: a typo in
    /// any coefficient moves the 298.15 K value off the tabulated one.
    ///
    /// Results (2026-08-11): `S(H2) = 130.68015 J/(mol·K)` (tabulated
    /// 130.68, +0.0002 %), `S(O2) = 205.14728 J/(mol·K)` (tabulated 205.15,
    /// -0.001 %), `S(H2O, l) = 69.95364 J/(mol·K)` (literature 69.95,
    /// +0.005 %). All three coefficient sets are transcribed correctly.
    ///
    /// This test caught a real transcription error during the port: the `E`
    /// coefficients of hydrogen and oxygen are **negative** in NIST's tables,
    /// which DWSIM encodes by writing `+E/(2 t²)` where liquid water gets
    /// `-E/(2 t²)` (`:443-445`). Passing the unsigned magnitudes put
    /// `S(H2)` at 128.896 J/(mol·K), 1.78 low. See
    /// [`hydrogen_gas_entropy_j_per_mol_k`].
    #[test]
    fn shomate_entropies_match_tabulated_values_at_298k() {
        let t = 298.15;
        assert_relative_eq!(hydrogen_gas_entropy_j_per_mol_k(t), 130.68, epsilon = 0.01);
        assert_relative_eq!(oxygen_gas_entropy_j_per_mol_k(t), 205.15, epsilon = 0.01);
        assert_relative_eq!(liquid_water_entropy_j_per_mol_k(t), 69.95, epsilon = 0.1);
    }

    /// Methodology: with `dH = 285.83 kJ/mol` at 298.15 K, compute `dG` via
    /// [`reaction_gibbs_energy_kj_per_mol`] (`:446`) and then the two
    /// voltages (`:448-449`). The textbook values for liquid-water
    /// electrolysis at standard conditions are `V_rev ≈ 1.23 V` and
    /// `V_th ≈ 1.48 V`; recovering them from the transcribed Shomate
    /// coefficients + Faraday constant checks the whole chain end to end.
    ///
    /// Results (2026-08-11): `dG = 237.14206 kJ/mol`,
    /// `V_rev = 1.228902 V` (textbook 1.229 V, -0.008 %),
    /// `V_th = 1.481210 V` (textbook 1.481 V, +0.014 %), and
    /// `V_th > V_rev` as thermodynamics requires. Recovering both textbook
    /// voltages to better than 0.02 % from nothing but the transcribed
    /// Shomate coefficients and Faraday's constant is the strongest evidence
    /// available here that the reaction-thermodynamics chain is correct.
    #[test]
    fn reversible_and_thermoneutral_voltages_match_textbook_values() {
        let dh = DH_298_KJ_PER_MOL;
        let dg = reaction_gibbs_energy_kj_per_mol(dh, 298.15);
        assert_relative_eq!(dg, 237.14, epsilon = 0.5);

        let v_rev = reversible_voltage(dg).get::<volt>();
        let v_th = thermoneutral_voltage(dh).get::<volt>();
        assert_relative_eq!(v_rev, 1.229, epsilon = 0.005);
        assert_relative_eq!(v_th, 1.481, epsilon = 0.005);
        assert!(
            v_th > v_rev,
            "thermoneutral voltage must exceed reversible voltage (dH > dG)"
        );
    }

    /// Methodology — **Faraday stoichiometry, the core check**
    /// (`WaterElectrolyzer.vb:461-467`). A 100-cell stack at 200 V drawing
    /// 100 kW. Hand calculation:
    ///
    /// - `I = 100 kW * 1000 / 200 V = 500 A`
    /// - `n_e = I N / F = 500 * 100 / 96485.3365 = 0.5182135 mol e-/s`
    /// - `n_H2 = n_e / 2 = 0.2591067 mol/s`
    /// - `n_O2 = n_e / 4 = 0.1295534 mol/s`
    /// - `n_H2O = n_H2 = 0.2591067 mol/s`
    ///
    /// The two invariants that must hold for *any* input are `n_H2 = 2 n_O2`
    /// (the 2:1 electrolysis ratio) and `n_H2O = n_H2` (one water per
    /// hydrogen).
    ///
    /// Results (2026-08-11): `I = 500.000000 A`,
    /// `n_e = 0.5182134593 mol e-/s`, `n_H2 = 0.2591067297 mol/s`,
    /// `n_O2 = 0.1295533648 mol/s`, `n_H2O = 0.2591067297 mol/s`. Ratios
    /// `n_H2/n_O2 = 2.000000` and `n_H2O/n_H2 = 1.000000`, both exact to
    /// 1e-12.
    #[test]
    fn faraday_stoichiometry_hydrogen_oxygen_and_water_rates() {
        let result = calculate(
            Power::new::<kilowatt>(100.0),
            ElectrolyzerSpecification::VoltageAndCells {
                stack_voltage: ElectricPotential::new::<volt>(200.0),
                number_of_cells: 100,
            },
            ElectricPotential::new::<volt>(1.229),
            ElectricPotential::new::<volt>(1.481),
            DH_298_KJ_PER_MOL,
        )
        .unwrap();

        assert_relative_eq!(result.current.get::<ampere>(), 500.0, epsilon = 1e-9);
        assert_relative_eq!(
            result.electron_transfer.get::<katal>(),
            500.0 / FARADAY_CONSTANT_C_PER_MOL * 100.0,
            epsilon = 1e-12
        );
        let n_h2 = result.hydrogen_production.get::<katal>();
        let n_o2 = result.oxygen_production.get::<katal>();
        let n_h2o = result.water_consumption.get::<katal>();
        assert_relative_eq!(n_h2, 0.2591067297, epsilon = 1e-9);
        assert_relative_eq!(n_o2, 0.1295533648, epsilon = 1e-9);
        // The 2:1 electrolysis ratio, and one water consumed per hydrogen.
        assert_relative_eq!(n_h2 / n_o2, 2.0, epsilon = 1e-12);
        assert_relative_eq!(n_h2o / n_h2, 1.0, epsilon = 1e-12);
    }

    /// Methodology — **energy-balance closure** for the voltage branch. The
    /// waste heat `Q = (V_cell - V_th) I N` (`:473`) must satisfy
    /// `P = N V_th I + Q`, i.e. the input power splits exactly into the
    /// enthalpy carried by the reaction and the heat released. Equivalently,
    /// `n_H2 * dH` (the chemical power stored in the hydrogen) must equal
    /// `P - Q`. Same 100 kW / 200 V / 100-cell case as above, with
    /// `V_th = 1.481 V`.
    ///
    /// Hand calculation: `V_cell = 2.0 V`, `Q = (2.0 - 1.481) * 500 * 100 /
    /// 1000 = 25.95 kW`; `N V_th I = 100 * 1.481 * 500 / 1000 = 74.05 kW`;
    /// sum = 100 kW. Chemical power `n_H2 dH = 0.2591067 * 285.83 =
    /// 74.06 kW`, which should match `P - Q = 74.05 kW` to within the
    /// rounding of the 1.481 V `V_th` used as input versus the exact
    /// `dH / 2F = 1.481210 V`.
    ///
    /// Results (2026-08-11): `V_cell = 2.000000 V`, `Q = 25.950000 kW`,
    /// `N V_th I + Q = 100.000000 kW` (closes to 1e-9),
    /// `n_H2 dH = 74.060477 kW` vs `P - Q = 74.050000 kW` — 0.0105 kW
    /// apart, i.e. **0.014 %**, exactly the fractional difference between
    /// the rounded 1.481 V input and the exact 1.481210 V. The residual is
    /// the input rounding, not a balance error; the exact closure is the
    /// `N V_th I + Q = P` identity above. `efficiency = 0.740500`.
    #[test]
    fn voltage_branch_energy_balance_closes() {
        let power_kw = 100.0;
        let v_th = 1.481;
        let n_cells = 100.0;
        let result = calculate(
            Power::new::<kilowatt>(power_kw),
            ElectrolyzerSpecification::VoltageAndCells {
                stack_voltage: ElectricPotential::new::<volt>(200.0),
                number_of_cells: 100,
            },
            ElectricPotential::new::<volt>(1.229),
            ElectricPotential::new::<volt>(v_th),
            DH_298_KJ_PER_MOL,
        )
        .unwrap();

        assert_relative_eq!(result.cell_voltage.get::<volt>(), 2.0, epsilon = 1e-12);
        let q_kw = result.waste_heat.get::<kilowatt>();
        assert_relative_eq!(q_kw, 25.95, epsilon = 1e-9);

        // P = N * V_th * I + Q  -- exact closure of DWSIM's own algebra.
        let i = result.current.get::<ampere>();
        assert_relative_eq!(n_cells * v_th * i / 1000.0 + q_kw, power_kw, epsilon = 1e-9);

        // Chemical power stored in the hydrogen matches P - Q.
        let chemical_kw = result.hydrogen_production.get::<katal>() * DH_298_KJ_PER_MOL;
        // Tolerance 0.02 kW: the 1.481 V input rounds the exact 1.481210 V,
        // which alone accounts for a 0.0105 kW offset -- see the doc comment.
        assert_relative_eq!(chemical_kw, power_kw - q_kw, epsilon = 0.02);

        // :498 -- Efficiency = (P - Q) / P.
        assert_relative_eq!(
            result.efficiency.get::<ratio>(),
            (power_kw - q_kw) / power_kw,
            epsilon = 1e-12
        );
    }

    /// Methodology — **efficiency branch** (`:476-490`). With `eta = 0.7` and
    /// 100 kW in: `reaction_heat = 70 kW`, `Q = 30 kW`,
    /// `n_H2 = 70 / 285.83 = 0.244903 mol/s`, `n_O2 = half that`,
    /// `V_cell = V_th / eta = 1.481 / 0.7 = 2.115714 V`, `I = 0` (DWSIM
    /// zeroes it at `:491`), `n_e = 2 n_H2O`. The reported efficiency
    /// `(P - Q)/P` must return the user's own `eta`.
    ///
    /// Results (2026-08-11): `n_H2 = 0.2449008152 mol/s`,
    /// `n_O2 = 0.1224504076 mol/s`, `Q = 30.000000 kW`,
    /// `V_cell = 2.1157142857 V`, `I = 0.000000 A`,
    /// `n_e = 0.4898016303 mol e-/s`, `efficiency = 0.700000` (round-trips
    /// exactly). Note the hydrogen rate is 5.5 % *below* the voltage
    /// branch's, because 70 % efficiency delivers less reaction heat than
    /// the 74.05 % the voltage branch achieved at 2.0 V/cell.
    #[test]
    fn efficiency_branch_matches_hand_calculation() {
        let result = calculate(
            Power::new::<kilowatt>(100.0),
            ElectrolyzerSpecification::Efficiency(Ratio::new::<ratio>(0.7)),
            ElectricPotential::new::<volt>(1.229),
            ElectricPotential::new::<volt>(1.481),
            DH_298_KJ_PER_MOL,
        )
        .unwrap();

        assert_relative_eq!(
            result.hydrogen_production.get::<katal>(),
            70.0 / DH_298_KJ_PER_MOL,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            result.oxygen_production.get::<katal>(),
            35.0 / DH_298_KJ_PER_MOL,
            epsilon = 1e-12
        );
        assert_relative_eq!(result.waste_heat.get::<kilowatt>(), 30.0, epsilon = 1e-12);
        assert_relative_eq!(
            result.cell_voltage.get::<volt>(),
            1.481 / 0.7,
            epsilon = 1e-12
        );
        assert_relative_eq!(result.current.get::<ampere>(), 0.0, epsilon = 1e-12);
        assert_relative_eq!(
            result.electron_transfer.get::<katal>(),
            2.0 * result.water_consumption.get::<katal>(),
            epsilon = 1e-12
        );
        // The reported efficiency must round-trip the user's input.
        assert_relative_eq!(result.efficiency.get::<ratio>(), 0.7, epsilon = 1e-12);
    }

    /// Methodology — error paths. (a) A 100-cell stack at 100 V gives
    /// `V_cell = 1.0 V < V_rev = 1.229 V`, which DWSIM rejects at `:469`.
    /// (b) Zero cells and (c) an efficiency of 1.5 both fall through to
    /// DWSIM's `Else` at `:492-495`.
    ///
    /// Results (2026-08-11): (a) `CellVoltageBelowReversible { cell_voltage_v:
    /// 1.0, reversible_voltage_v: 1.229 }`; (b) and (c) both
    /// `UnderspecifiedElectrolyzer`.
    #[test]
    fn error_paths_reject_low_voltage_and_bad_specifications() {
        let v_rev = ElectricPotential::new::<volt>(1.229);
        let v_th = ElectricPotential::new::<volt>(1.481);

        let too_low = calculate(
            Power::new::<kilowatt>(100.0),
            ElectrolyzerSpecification::VoltageAndCells {
                stack_voltage: ElectricPotential::new::<volt>(100.0),
                number_of_cells: 100,
            },
            v_rev,
            v_th,
            DH_298_KJ_PER_MOL,
        );
        assert!(matches!(
            too_low,
            Err(CleanEnergyError::CellVoltageBelowReversible { .. })
        ));

        let zero_cells = calculate(
            Power::new::<kilowatt>(100.0),
            ElectrolyzerSpecification::VoltageAndCells {
                stack_voltage: ElectricPotential::new::<volt>(200.0),
                number_of_cells: 0,
            },
            v_rev,
            v_th,
            DH_298_KJ_PER_MOL,
        );
        assert_eq!(
            zero_cells,
            Err(CleanEnergyError::UnderspecifiedElectrolyzer)
        );

        let bad_eff = calculate(
            Power::new::<kilowatt>(100.0),
            ElectrolyzerSpecification::Efficiency(Ratio::new::<ratio>(1.5)),
            v_rev,
            v_th,
            DH_298_KJ_PER_MOL,
        );
        assert_eq!(bad_eff, Err(CleanEnergyError::UnderspecifiedElectrolyzer));
    }

    /// Methodology — outlet composition bookkeeping
    /// (`WaterElectrolyzer.vb:500-553`). Inlet vector
    /// `[Water 10, Hydrogen 0, Oxygen 0, Nitrogen 1] mol/s` with the 100 kW
    /// case above (`n_H2O = n_H2 = 0.2591067`, `n_O2 = 0.1295534`). Then
    /// outlet 1 is split at `P = 101325 Pa` with `P_vap = 3169 Pa`
    /// (water at 298 K): `x_H2O,sat = 0.0312744`, `x_H2 = 0.9687256`.
    ///
    /// Two conservation checks must hold: the inert nitrogen is untouched,
    /// and the water splits across the two outlets without loss —
    /// concretely `water_out1 + water_out2 = water_after_rxn`.
    ///
    /// Results (2026-08-11): post-reaction water `9.7408932703 mol/s`,
    /// hydrogen `0.2591067297`, oxygen `0.1295533648`, nitrogen `1.000000`
    /// (unchanged). Outlet 1 `(n_H2 0.2591067297, n_H2O 0.0083653493)`;
    /// outlet 2 water `9.7325279210`, hydrogen `0.000000`, oxygen
    /// `0.1295533648`, nitrogen `1.000000`. Water closure
    /// `9.7325279210 + 0.0083653493 = 9.7408932703`, exact to 1e-12.
    #[test]
    fn outlet_composition_conserves_water_and_inerts() {
        let result = calculate(
            Power::new::<kilowatt>(100.0),
            ElectrolyzerSpecification::VoltageAndCells {
                stack_voltage: ElectricPotential::new::<volt>(200.0),
                number_of_cells: 100,
            },
            ElectricPotential::new::<volt>(1.229),
            ElectricPotential::new::<volt>(1.481),
            DH_298_KJ_PER_MOL,
        )
        .unwrap();

        let inlet = [10.0, 0.0, 0.0, 1.0]; // Water, Hydrogen, Oxygen, Nitrogen
        let after =
            apply_reaction_extents(&inlet, 0, 1, 2, &result, ElectrolysisChemistry::LightWater)
                .unwrap();
        assert_relative_eq!(after[3], 1.0, epsilon = 1e-12); // inert untouched
        assert_relative_eq!(
            after[1],
            result.hydrogen_production.get::<katal>(),
            epsilon = 1e-12
        );

        let (n_h2, n_h2o_sat) = hydrogen_outlet_split(
            MolarFlowRate::new::<katal>(after[1]),
            Pressure::new::<pascal>(3169.0),
            Pressure::new::<pascal>(101_325.0),
        )
        .unwrap();
        assert_relative_eq!(n_h2.get::<katal>(), after[1], epsilon = 1e-12);
        assert_relative_eq!(n_h2o_sat.get::<katal>(), 0.0083653493, epsilon = 1e-9);

        let outlet2 =
            oxygen_outlet_flows(&after, 0, 1, n_h2o_sat, ElectrolysisChemistry::LightWater)
                .unwrap();
        assert_relative_eq!(outlet2[1], 0.0, epsilon = 1e-15);
        // Water closure across the two outlets.
        assert_relative_eq!(
            outlet2[0] + n_h2o_sat.get::<katal>(),
            after[0],
            epsilon = 1e-12
        );
        assert_relative_eq!(outlet2[3], 1.0, epsilon = 1e-12); // inert still untouched
    }

    /// Methodology — the negative-flow guards (`:510`, `:548`). Feeding only
    /// 0.1 mol/s of water while producing 0.259 mol/s of hydrogen must trip
    /// [`CleanEnergyError::NegativeMolarFlow`], DWSIM's "Increase water rate
    /// in inlet stream or reduce power."
    ///
    /// Result (2026-08-11): `NegativeMolarFlow { species: "Water",
    /// molar_flow_mol_per_s: -0.1591067297 }`.
    #[test]
    fn insufficient_water_feed_is_rejected() {
        let result = calculate(
            Power::new::<kilowatt>(100.0),
            ElectrolyzerSpecification::VoltageAndCells {
                stack_voltage: ElectricPotential::new::<volt>(200.0),
                number_of_cells: 100,
            },
            ElectricPotential::new::<volt>(1.229),
            ElectricPotential::new::<volt>(1.481),
            DH_298_KJ_PER_MOL,
        )
        .unwrap();
        let starved = apply_reaction_extents(
            &[0.1, 0.0, 0.0],
            0,
            1,
            2,
            &result,
            ElectrolysisChemistry::LightWater,
        );
        assert!(matches!(
            starved,
            Err(CleanEnergyError::NegativeMolarFlow {
                species: "Water",
                ..
            })
        ));
    }

    /// Methodology — heavy-water chemistry naming (`:410-419`). The
    /// arithmetic is identical; only the compound names change.
    /// Result (2026-08-11): light water reports
    /// `("Water", "Hydrogen", "Oxygen")`; heavy water reports
    /// `("HeavyWater", "Deuterium", "Oxygen")`.
    #[test]
    fn heavy_water_chemistry_names_match_upstream() {
        let light = ElectrolysisChemistry::LightWater;
        assert_eq!(light.water_compound(), "Water");
        assert_eq!(light.hydrogen_compound(), "Hydrogen");
        assert_eq!(light.oxygen_compound(), "Oxygen");

        let heavy = ElectrolysisChemistry::HeavyWater;
        assert_eq!(heavy.water_compound(), "HeavyWater");
        assert_eq!(heavy.hydrogen_compound(), "Deuterium");
        assert_eq!(heavy.oxygen_compound(), "Oxygen");
    }

    /// Methodology — the outlet enthalpy bump (`:539-541`) and its
    /// dimensional caveat. `Q = 25.95 kW`, `w_outlet = 0.6 kg/s`,
    /// `w_inlet = 3.0 kg/s`. DWSIM's literal expression gives
    /// `25.95 * 0.2 = 5.19` (units kW, used as kJ/kg); the dimensionally
    /// consistent reading gives `25.95 / 3.0 = 8.65 kJ/kg`.
    ///
    /// Result (2026-08-11): `dwsim_verbatim = 5.190000`,
    /// `dimensionally_consistent = 8.650000`. Both reported so the caller
    /// chooses knowingly.
    #[test]
    fn outlet_enthalpy_bump_reports_both_readings() {
        let bump = outlet_enthalpy_bump(Power::new::<kilowatt>(25.95), 0.6, 3.0);
        assert_relative_eq!(bump.dwsim_verbatim, 5.19, epsilon = 1e-12);
        assert_relative_eq!(bump.dimensionally_consistent, 8.65, epsilon = 1e-12);
    }

    /// Methodology — the stateful wrapper. Solving through
    /// [`WaterElectrolyzer::solve`] must store the result and make
    /// [`CleanEnergyUnitOp::generated_power`] report the power draw as a
    /// negative number (this port's sign convention for consumers).
    ///
    /// Result (2026-08-11): before solving, 0 W and `last_result == None`;
    /// after solving 100 kW in, `generated_power = -100000 W` and
    /// `last_result` is populated.
    #[test]
    fn stateful_unit_reports_negative_power_once_solved() {
        let mut unit = WaterElectrolyzer {
            power_input: Power::new::<kilowatt>(100.0),
            ..Default::default()
        };
        assert_eq!(unit.generated_power().get::<watt>(), 0.0);
        assert!(unit.last_result.is_none());

        unit.solve(
            ElectrolyzerSpecification::VoltageAndCells {
                stack_voltage: ElectricPotential::new::<volt>(200.0),
                number_of_cells: 100,
            },
            ElectricPotential::new::<volt>(1.229),
            ElectricPotential::new::<volt>(1.481),
            DH_298_KJ_PER_MOL,
        )
        .unwrap();
        assert!(unit.last_result.is_some());
        assert_relative_eq!(
            unit.generated_power().get::<watt>(),
            -100_000.0,
            epsilon = 1e-6
        );
    }

    /// Methodology — reaction-enthalpy assembly (`:432, :438-440`) and the
    /// reference temperature (`:431`). `dH_ig = 241.83 kJ/mol` (gas-phase
    /// water) plus `dH_vap = 44.0 kJ/mol` must give the liquid-feed
    /// `285.83 kJ/mol`.
    /// Result (2026-08-11): `285.830000 kJ/mol`; reference temperature
    /// `298.150000 K`.
    #[test]
    fn reaction_enthalpy_assembly_recovers_liquid_feed_value() {
        assert_relative_eq!(
            reaction_enthalpy_kj_per_mol(241.83, 44.0),
            285.83,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            reference_temperature().get::<kelvin>(),
            298.15,
            epsilon = 1e-12
        );
    }

    /// Methodology — the saturated-outlet guard. At `P = 3000 Pa` with
    /// `P_vap = 3169 Pa` the water is above its boiling pressure, so
    /// `x_H2 <= 0` and the split is undefined. DWSIM does not guard this.
    /// Result (2026-08-11): `OutOfDomain { .. }`.
    #[test]
    fn hydrogen_split_rejects_pressure_below_water_vapour_pressure() {
        let out = hydrogen_outlet_split(
            MolarFlowRate::new::<katal>(0.25),
            Pressure::new::<pascal>(3169.0),
            Pressure::new::<pascal>(3000.0),
        );
        assert!(matches!(out, Err(CleanEnergyError::OutOfDomain { .. })));
    }
}
