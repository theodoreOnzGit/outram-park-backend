//! # Simple Rankine Steam Cycle Tutorial
//!
//! This tutorial walks through a **steady-state Rankine steam cycle**, the most
//! common thermodynamic system in power generation. It demonstrates:
//!
//! 1. **Feedwater pump**: Compress saturated liquid from 10 kPa to 8 MPa (State 1→2s)
//! 2. **Boiler**: Heat pressurized liquid to superheated steam at 480 °C (State 2s→3)
//! 3. **Turbine**: Expand steam isentropically back to 10 kPa (State 3→4s)
//! 4. **Condenser**: Condense wet steam back to saturated liquid (State 4s→1)
//!
//! We compute the specific enthalpy `h` [kJ/kg] and entropy `s` [J/(kg·K)] at each
//! state point, then calculate the **thermal efficiency** = (turbine work - pump work) / heat input.
//!
//! **Key units to remember:**
//! - Pressure in Pa (use `::megapascal`, `::kilopascal` from uom)
//! - Temperature in K (Kelvin), NOT Celsius
//! - Specific enthalpy in J/kg (NOT kJ/kg — the crate uses SI units)
//! - Specific entropy in J/(kg·K)
//! - Work and heat in J/kg after accounting for mass

use uom::si::f64::*;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::pressure::{kilopascal, megapascal};
use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;

use tampines_steam_tables::prelude::*;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║        RANKINE STEAM CYCLE: TUTORIAL CALCULATION            ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // =========================================================================
    // CYCLE PARAMETERS (typical textbook values)
    // =========================================================================
    // Condenser pressure: 10 kPa (where steam condenses back to liquid)
    let p_low: Pressure = Pressure::new::<kilopascal>(10.0);

    // Boiler pressure: 8 MPa (high pressure steam generation)
    let p_high: Pressure = Pressure::new::<megapascal>(8.0);

    // Boiler outlet temperature: 480 °C (superheated steam)
    // IMPORTANT: uom uses Kelvin, so convert 480 °C = 753.15 K
    let t_boiler_outlet: ThermodynamicTemperature =
        ThermodynamicTemperature::new::<kelvin>(480.0 + 273.15);

    println!("Cycle Parameters:");
    println!(
        "  Condenser pressure (P_low):      {:.1} kPa",
        p_low.get::<kilopascal>()
    );
    println!(
        "  Boiler pressure (P_high):        {:.1} MPa",
        p_high.get::<megapascal>()
    );
    println!(
        "  Boiler outlet temperature:       480 °C = {:.2} K\n",
        t_boiler_outlet.get::<kelvin>()
    );

    // =========================================================================
    // STATE 1: SATURATED LIQUID AT CONDENSER (x = 0)
    // Pressure: 10 kPa, Quality: 0 (100% liquid)
    // =========================================================================
    println!("STATE 1: Saturated liquid at condenser");
    println!("  Inputs: P = 10 kPa, x = 0.0 (saturated liquid)");

    // Create a dummy control volume (1 m³) to evaluate properties
    let dummy_volume: Volume = Volume::new::<uom::si::volume::cubic_meter>(1.0);

    // Use the saturation relationship: at p_low, find T_sat
    // Then create state 1 with x=0 (saturated liquid)
    let state_1 = TampinesSteamTableCV::new_from_sat_pressure_quality(
        p_low,
        0.0, // x = 0: saturated liquid
        dummy_volume,
    );

    let p_1 = state_1.get_pressure();
    let t_1 = state_1.get_temperature();
    let h_1 = state_1.get_specific_enthalpy();
    let s_1 = state_1.get_specific_entropy();
    let v_1 = state_1.get_specific_volume();

    println!("  Output:");
    println!("    P₁ = {:.2} kPa", p_1.get::<kilopascal>());
    println!(
        "    T₁ = {:.2} K ({:.2} °C)",
        t_1.get::<kelvin>(),
        t_1.get::<kelvin>() - 273.15
    );
    println!("    h₁ = {:.2} kJ/kg", h_1.get::<kilojoule_per_kilogram>());
    println!(
        "    s₁ = {:.4} J/(kg·K)",
        s_1.get::<joule_per_kilogram_kelvin>()
    );
    println!(
        "    v₁ = {:.6} m³/kg\n",
        v_1.get::<uom::si::specific_volume::cubic_meter_per_kilogram>()
    );

    // =========================================================================
    // STATE 2s: AFTER ISENTROPIC PUMP (s₂ = s₁, P₂ = P_high)
    // The pump is a steady-flow device; for an incompressible liquid in an
    // isentropic pump, entropy is constant (reversible, adiabatic process).
    // =========================================================================
    println!("STATE 2s: After ISENTROPIC pump (compressed liquid)");
    println!("  Inputs: P = 8 MPa, s = s₁ (isentropic compression)");
    println!("  Process: Constant entropy (reversible, adiabatic)");

    // Create state 2s: use (p, s) flash at high pressure with entropy from state 1
    let s_isentropic = s_1; // Isentropic process: s₂ = s₁

    let state_2s = TampinesSteamTableCV::new_from_ps(p_high, s_isentropic, dummy_volume);

    let p_2s = state_2s.get_pressure();
    let t_2s = state_2s.get_temperature();
    let h_2s = state_2s.get_specific_enthalpy();
    let s_2s = state_2s.get_specific_entropy();
    let v_2s = state_2s.get_specific_volume();

    println!("  Output:");
    println!("    P₂ₛ = {:.2} MPa", p_2s.get::<megapascal>());
    println!(
        "    T₂ₛ = {:.2} K ({:.2} °C)",
        t_2s.get::<kelvin>(),
        t_2s.get::<kelvin>() - 273.15
    );
    println!(
        "    h₂ₛ = {:.2} kJ/kg",
        h_2s.get::<kilojoule_per_kilogram>()
    );
    println!(
        "    s₂ₛ = {:.4} J/(kg·K)",
        s_2s.get::<joule_per_kilogram_kelvin>()
    );
    println!(
        "    v₂ₛ = {:.6} m³/kg\n",
        v_2s.get::<uom::si::specific_volume::cubic_meter_per_kilogram>()
    );

    // Pump work (per unit mass): w_pump = h₂ - h₁
    // Negative because work is done ON the system (compression)
    let h_diff_pump = h_2s - h_1;
    println!(
        "  Pump specific work: w_pump = h₂ₛ - h₁ = {:.2} kJ/kg (input required)\n",
        h_diff_pump.get::<kilojoule_per_kilogram>()
    );

    // =========================================================================
    // STATE 3: SUPERHEATED STEAM FROM BOILER (T = 480 °C, P = 8 MPa)
    // The boiler heats the compressed liquid to a superheated state.
    // We specify both pressure and temperature.
    // =========================================================================
    println!("STATE 3: Superheated steam from BOILER");
    println!("  Inputs: P = 8 MPa, T = 480 °C = 753.15 K");
    println!("  Process: Isobaric heating (constant pressure, 0 kPa to 8 MPa)");

    // Create state 3: use (T, P) flash for superheated steam
    // The second parameter in new_from_tp_quality is a dummy x value; it's ignored for
    // superheated states (x only affects saturation-line calculations).
    let state_3 = TampinesSteamTableCV::new_from_tp_quality(
        t_boiler_outlet,
        p_high,
        dummy_volume,
        1.0, // x=1.0 (dummy value, ignored for superheated region)
    );

    let p_3 = state_3.get_pressure();
    let t_3 = state_3.get_temperature();
    let h_3 = state_3.get_specific_enthalpy();
    let s_3 = state_3.get_specific_entropy();
    let v_3 = state_3.get_specific_volume();

    println!("  Output:");
    println!("    P₃ = {:.2} MPa", p_3.get::<megapascal>());
    println!(
        "    T₃ = {:.2} K ({:.2} °C)",
        t_3.get::<kelvin>(),
        t_3.get::<kelvin>() - 273.15
    );
    println!("    h₃ = {:.2} kJ/kg", h_3.get::<kilojoule_per_kilogram>());
    println!(
        "    s₃ = {:.4} J/(kg·K)",
        s_3.get::<joule_per_kilogram_kelvin>()
    );
    println!(
        "    v₃ = {:.6} m³/kg\n",
        v_3.get::<uom::si::specific_volume::cubic_meter_per_kilogram>()
    );

    // Boiler heat input (per unit mass): q_in = h₃ - h₂s
    // Positive because heat flows INTO the system
    let h_diff_boiler = h_3 - h_2s;
    println!(
        "  Boiler heat input: q_in = h₃ - h₂ₛ = {:.2} kJ/kg\n",
        h_diff_boiler.get::<kilojoule_per_kilogram>()
    );

    // =========================================================================
    // STATE 4s: AFTER ISENTROPIC TURBINE EXPANSION (s₄ = s₃, P₄ = P_low)
    // The turbine expands steam isentropically (reversible, adiabatic).
    // We expect a two-phase mixture (wet steam) at the outlet.
    // =========================================================================
    println!("STATE 4s: After ISENTROPIC turbine expansion (wet steam)");
    println!("  Inputs: P = 10 kPa, s = s₃ (isentropic expansion)");
    println!("  Process: Constant entropy (reversible, adiabatic)");

    // Create state 4s: use (p, s) flash at low pressure with entropy from state 3
    let s_turbine = s_3; // Isentropic process: s₄ = s₃

    let state_4s = TampinesSteamTableCV::new_from_ps(p_low, s_turbine, dummy_volume);

    let p_4s = state_4s.get_pressure();
    let t_4s = state_4s.get_temperature();
    let h_4s = state_4s.get_specific_enthalpy();
    let s_4s = state_4s.get_specific_entropy();
    let v_4s = state_4s.get_specific_volume();

    println!("  Output:");
    println!("    P₄ₛ = {:.2} kPa", p_4s.get::<kilopascal>());
    println!(
        "    T₄ₛ = {:.2} K ({:.2} °C)",
        t_4s.get::<kelvin>(),
        t_4s.get::<kelvin>() - 273.15
    );
    println!(
        "    h₄ₛ = {:.2} kJ/kg",
        h_4s.get::<kilojoule_per_kilogram>()
    );
    println!(
        "    s₄ₛ = {:.4} J/(kg·K)",
        s_4s.get::<joule_per_kilogram_kelvin>()
    );
    println!(
        "    v₄ₛ = {:.6} m³/kg\n",
        v_4s.get::<uom::si::specific_volume::cubic_meter_per_kilogram>()
    );

    // Turbine work (per unit mass): w_out = h₃ - h₄s
    // Positive because work is done BY the system (expansion)
    let h_diff_turbine = h_3 - h_4s;
    println!(
        "  Turbine specific work: w_turbine = h₃ - h₄ₛ = {:.2} kJ/kg (output produced)\n",
        h_diff_turbine.get::<kilojoule_per_kilogram>()
    );

    // Condenser heat rejection (per unit mass): q_out = h₄ - h₁
    // This is negative because heat flows OUT of the system
    let h_diff_condenser = h_4s - h_1;
    println!(
        "  Condenser heat rejection: q_out = h₄ₛ - h₁ = {:.2} kJ/kg (heat rejected)\n",
        h_diff_condenser.get::<kilojoule_per_kilogram>()
    );

    // =========================================================================
    // CYCLE ANALYSIS: EFFICIENCY AND NET WORK
    // =========================================================================
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              CYCLE PERFORMANCE SUMMARY                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Net work output (per unit mass): W_net = W_turbine - W_pump = (h₃-h₄) - (h₂-h₁)
    // Units: J/kg (or kJ/kg if converted)
    let w_net = h_diff_turbine - h_diff_pump;

    // Thermal efficiency: η = W_net / Q_in = (W_turbine - W_pump) / (h₃ - h₂)
    let efficiency = w_net / h_diff_boiler;

    println!("Specific work and heat per kilogram:");
    println!(
        "  Turbine work output (W_turbine):  {:.2} kJ/kg",
        h_diff_turbine.get::<kilojoule_per_kilogram>()
    );
    println!(
        "  Pump work input (W_pump):         {:.2} kJ/kg",
        h_diff_pump.get::<kilojoule_per_kilogram>()
    );
    println!(
        "  Net work (W_net):                 {:.2} kJ/kg",
        w_net.get::<kilojoule_per_kilogram>()
    );
    println!(
        "  Heat input (Q_in):                {:.2} kJ/kg\n",
        h_diff_boiler.get::<kilojoule_per_kilogram>()
    );

    println!("THERMAL EFFICIENCY (ideal Rankine cycle):");
    println!(
        "  η = W_net / Q_in = {:.4} = {:.2}%\n",
        efficiency.get::<uom::si::ratio::ratio>(),
        efficiency.get::<uom::si::ratio::ratio>() * 100.0
    );

    // =========================================================================
    // STATE-POINT TABLE: Formatted summary
    // =========================================================================
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              STATE-POINT TABLE (ALL STATES)                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!(
        "{:<8} {:<12} {:<12} {:<12} {:<12} {:<12}",
        "State", "P [kPa]", "T [°C]", "h [kJ/kg]", "s [J/(kg·K)]", "v [m³/kg]"
    );
    println!(
        "{:-<8} {:-<12} {:-<12} {:-<12} {:-<12} {:-<12}",
        "", "", "", "", "", ""
    );

    let state_1_p_kpa = p_1.get::<kilopascal>();
    let state_1_t_c = t_1.get::<kelvin>() - 273.15;
    let state_1_h_kj = h_1.get::<kilojoule_per_kilogram>();
    let state_1_s_j = s_1.get::<joule_per_kilogram_kelvin>();
    let state_1_v_m3 = v_1.get::<uom::si::specific_volume::cubic_meter_per_kilogram>();

    println!(
        "{:<8} {:<12.2} {:<12.2} {:<12.2} {:<12.4} {:<12.6}",
        "1 (sat liq)", state_1_p_kpa, state_1_t_c, state_1_h_kj, state_1_s_j, state_1_v_m3
    );

    let state_2s_p_kpa = p_2s.get::<megapascal>() * 1000.0;
    let state_2s_t_c = t_2s.get::<kelvin>() - 273.15;
    let state_2s_h_kj = h_2s.get::<kilojoule_per_kilogram>();
    let state_2s_s_j = s_2s.get::<joule_per_kilogram_kelvin>();
    let state_2s_v_m3 = v_2s.get::<uom::si::specific_volume::cubic_meter_per_kilogram>();

    println!(
        "{:<8} {:<12.2} {:<12.2} {:<12.2} {:<12.4} {:<12.6}",
        "2s (compr)", state_2s_p_kpa, state_2s_t_c, state_2s_h_kj, state_2s_s_j, state_2s_v_m3
    );

    let state_3_p_kpa = p_3.get::<megapascal>() * 1000.0;
    let state_3_t_c = t_3.get::<kelvin>() - 273.15;
    let state_3_h_kj = h_3.get::<kilojoule_per_kilogram>();
    let state_3_s_j = s_3.get::<joule_per_kilogram_kelvin>();
    let state_3_v_m3 = v_3.get::<uom::si::specific_volume::cubic_meter_per_kilogram>();

    println!(
        "{:<8} {:<12.2} {:<12.2} {:<12.2} {:<12.4} {:<12.6}",
        "3 (superh)", state_3_p_kpa, state_3_t_c, state_3_h_kj, state_3_s_j, state_3_v_m3
    );

    let state_4s_p_kpa = p_4s.get::<kilopascal>();
    let state_4s_t_c = t_4s.get::<kelvin>() - 273.15;
    let state_4s_h_kj = h_4s.get::<kilojoule_per_kilogram>();
    let state_4s_s_j = s_4s.get::<joule_per_kilogram_kelvin>();
    let state_4s_v_m3 = v_4s.get::<uom::si::specific_volume::cubic_meter_per_kilogram>();

    println!(
        "{:<8} {:<12.2} {:<12.2} {:<12.2} {:<12.4} {:<12.6}",
        "4s (wet)", state_4s_p_kpa, state_4s_t_c, state_4s_h_kj, state_4s_s_j, state_4s_v_m3
    );

    println!("\nLegend:");
    println!("  State 1: Saturated liquid (condenser outlet)");
    println!("  State 2s: Compressed liquid (pump outlet, isentropic)");
    println!("  State 3: Superheated steam (boiler outlet)");
    println!("  State 4s: Wet steam (turbine outlet, isentropic)");
    println!("\nNote: 's' suffix denotes isentropic (ideal) process");
}
