//! Headless execution of the HTGR plant model — no GUI, no threads, no clock.
//!
//! Exists so the plant can be **run and observed without `eframe`**, which the
//! interactive binary cannot do. Two things need that:
//!
//! 1. **Recording a reference baseline** before the execution refactor
//!    (bead `op-fbou`). A baseline captured through the GUI would be a
//!    baseline of the GUI's timing, not of the physics.
//! 2. **Regression testing.** A test can call [`run`] and compare the trace
//!    against a committed fixture.
//!
//! ## Why this is already almost a kernel driver
//!
//! [`crate::physics::HtgrPlant::step`] is already deterministic, bounded and
//! free of any thread of its own — the loop that drives it lives in
//! `app_scaffold::spawn_physics_thread`, not in the physics. This module is
//! therefore a *second* driver for the same step function, and the fact that
//! writing it required no change to `physics/` is the evidence that the kernel
//! extraction (bead `op-37ke`) is an extraction rather than a rewrite.
//!
//! ## Determinism
//!
//! No wall clock, no RNG, no I/O inside the loop. Given the same
//! `HeadlessConfig` this produces byte-identical output on the same build.
//! That is the property the reference baseline depends on, and
//! `determinism_same_config_same_trace` in this module asserts it.

use crate::app::state::HtgrSnapshot;
use crate::physics::PLANT_TIMESTEP_S;
use crate::runtime::{PlantControls, PlantRuntime};
use outram_park_digital_twin_engine::prelude::{HeadlessModel, HeadlessRun};
use uom::si::f64::Time;
use uom::si::time::second;

/// How to run a headless simulation.
#[derive(Clone, Debug)]
pub struct HeadlessConfig {
    /// Number of plant timesteps to advance.
    pub steps: usize,
    /// Emit one trace row every `sample_every` steps. `1` records every step.
    pub sample_every: usize,
    /// Operator controls, held constant for the whole run.
    ///
    /// `PlantCommands::default()` is the published operating point with the
    /// rods at their critical insertion.
    ///
    /// Its docstring in `physics` claims this starts *"near steady state
    /// rather than on a prompt excursion"*. **Measured 2026-09-06, it does
    /// not:** power rises to ~27.8 MW near 100 s — roughly 2.8x nominal —
    /// before settling near 8.1 MW by 1200 s, with bed temperature still
    /// drifting downward at that point. It is a large startup transient.
    ///
    /// That makes it a *better* baseline, not a worse one — a transient
    /// exercises far more of the model than a hold would — but it must not be
    /// described as steady state.
    pub controls: PlantControls,
}

impl Default for HeadlessConfig {
    fn default() -> Self {
        Self {
            steps: 600,
            sample_every: 60,
            controls: PlantControls::default(),
        }
    }
}

/// One sampled row of a headless run.
///
/// A deliberately narrow projection of [`HtgrSnapshot`]'s fields: enough to
/// detect a behavioural change across the whole plant, few enough that the
/// committed fixture stays readable and a diff is interpretable.
///
/// **Widened 2026-09-29 (gh:#394)** from the kinetics and bed temperatures
/// alone to the whole primary circuit and the plant's energy ledger, so a
/// global energy balance can be computed **from the CSV**: the six `e_*_j`
/// columns are cumulative since construction, and
/// `e_source_j + e_circulator_work_j - e_stored_j - e_to_steam_generator_j -
/// e_to_rccs_j = e_residual_j` holds row by row (see
/// `crate::physics::PlantEnergyLedger` for the boundary: the steam
/// generator is outside it, and the boundary on that side is the helium
/// stream).
#[derive(Clone, Debug, PartialEq)]
pub struct TraceRow {
    pub step: usize,
    pub sim_time_s: f64,
    pub reactor_power_mw: f64,
    pub prompt_power_mw: f64,
    pub delayed_power_mw: f64,
    pub fuel_temperature_k: f64,
    pub bed_temperature_k: f64,
    /// Circulator helium mass flow \[kg/s\].
    pub helium_flow_kg_per_s: f64,
    /// Core inlet (RPV-annuli CV; the cold-return CV until 2026-10-01) helium
    /// temperature \[K\].
    pub core_inlet_k: f64,
    /// Core outlet (bed fluid node) helium temperature \[K\].
    pub core_outlet_k: f64,
    /// Hot-duct CV helium temperature \[K\] -- the steam generator's inlet.
    pub hot_duct_k: f64,
    /// Steam-generator helium outlet temperature \[K\].
    pub sg_helium_outlet_k: f64,
    /// Steam-generator duty leaving the helium \[MW\].
    pub sg_helium_duty_mw: f64,
    /// Steam-generator duty entering the water/steam \[MW\].
    pub sg_secondary_duty_mw: f64,
    /// Passive loss from the bed to the reflector \[MW\].
    pub passive_loss_mw: f64,
    /// Circulator work delivered to the helium \[MW\].
    pub circulator_work_mw: f64,
    /// Reflector -> riser helium heat \[MW\] (gh:#397; internal to the
    /// ledger: it leaves the reflector and enters the RPV-annuli CV).
    pub riser_heat_mw: f64,
    /// Lumped reflector temperature \[K\].
    pub reflector_k: f64,
    /// Lumped RPV temperature \[K\].
    pub rpv_k: f64,
    /// Cumulative fission + decay heat deposited in the fuel \[J\].
    pub e_source_j: f64,
    /// Cumulative stored-energy change in every lumped CV \[J\].
    pub e_stored_j: f64,
    /// Cumulative enthalpy handed from the helium to the steam generator \[J\].
    pub e_to_steam_generator_j: f64,
    /// Cumulative heat to the RCCS \[J\].
    pub e_to_rccs_j: f64,
    /// Cumulative circulator work delivered to the helium \[J\].
    pub e_circulator_work_j: f64,
    /// Cumulative ledger residual \[J\].
    pub e_residual_j: f64,
}

impl TraceRow {
    /// CSV header matching [`Self::to_csv`].
    pub fn csv_header() -> &'static str {
        "step,sim_time_s,reactor_power_mw,prompt_power_mw,delayed_power_mw,fuel_temperature_k,bed_temperature_k,\
helium_flow_kg_per_s,core_inlet_k,core_outlet_k,hot_duct_k,sg_helium_outlet_k,sg_helium_duty_mw,\
sg_secondary_duty_mw,passive_loss_mw,circulator_work_mw,riser_heat_mw,reflector_k,rpv_k,e_source_j,e_stored_j,\
e_to_steam_generator_j,e_to_rccs_j,e_circulator_work_j,e_residual_j"
    }

    /// One CSV row.
    ///
    /// `{:.9}` throughout for the state columns: enough digits that a real
    /// behavioural change is visible, and a fixed width so a committed fixture
    /// diffs cleanly. The cumulative energies are `{:.12e}` -- a fixed
    /// thirteen significant figures, since they grow to 1e10 J and more.
    pub fn to_csv(&self) -> String {
        format!(
            "{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},\
             {:.9},{:.9},{:.9},{:.9},{:.9},{:.12e},{:.12e},{:.12e},{:.12e},{:.12e},{:.12e}",
            self.step,
            self.sim_time_s,
            self.reactor_power_mw,
            self.prompt_power_mw,
            self.delayed_power_mw,
            self.fuel_temperature_k,
            self.bed_temperature_k,
            self.helium_flow_kg_per_s,
            self.core_inlet_k,
            self.core_outlet_k,
            self.hot_duct_k,
            self.sg_helium_outlet_k,
            self.sg_helium_duty_mw,
            self.sg_secondary_duty_mw,
            self.passive_loss_mw,
            self.circulator_work_mw,
            self.riser_heat_mw,
            self.reflector_k,
            self.rpv_k,
            self.e_source_j,
            self.e_stored_j,
            self.e_to_steam_generator_j,
            self.e_to_rccs_j,
            self.e_circulator_work_j,
            self.e_residual_j,
        )
    }
}

/// The plant as a [`HeadlessModel`], so it uses the engine library's shared
/// harness rather than a bespoke loop.
///
/// Owns the snapshot buffer because [`HeadlessModel::sample`] takes `&self`
/// while `PlantRuntime::publish` writes into a caller-supplied snapshot; the
/// buffer is a rendering scratch space, not plant state.
pub struct HtgrHeadless {
    runtime: PlantRuntime,
    scratch: std::cell::RefCell<HtgrSnapshot>,
}

impl HtgrHeadless {
    /// A fresh plant at the default operating point.
    pub fn new() -> Self {
        Self {
            runtime: PlantRuntime::new(),
            scratch: std::cell::RefCell::new(HtgrSnapshot::default()),
        }
    }
}

impl Default for HtgrHeadless {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadlessModel for HtgrHeadless {
    type Controls = PlantControls;
    type Sample = TraceRow;

    fn submit(&mut self, controls: PlantControls) {
        self.runtime.submit(controls);
    }

    fn tick(&mut self) {
        self.runtime.tick(Time::new::<second>(PLANT_TIMESTEP_S));
    }

    fn sample(&self, step: usize) -> TraceRow {
        let mut snap = self.scratch.borrow_mut();
        self.runtime.publish(&mut snap);
        TraceRow {
            step,
            sim_time_s: snap.sim_time_s,
            reactor_power_mw: snap.reactor_power_mw,
            prompt_power_mw: snap.prompt_power_mw,
            delayed_power_mw: snap.delayed_power_mw,
            fuel_temperature_k: snap.fuel_temperature_k,
            bed_temperature_k: snap.bed_temperature_k,
            helium_flow_kg_per_s: snap.helium_mass_flow_kg_per_s,
            core_inlet_k: snap.core_inlet_temp_k,
            core_outlet_k: snap.core_outlet_temp_k,
            hot_duct_k: snap.hot_duct_temp_k,
            sg_helium_outlet_k: snap.ihx_outlet_temp_k,
            sg_helium_duty_mw: snap.ihx_duty_mw,
            sg_secondary_duty_mw: snap.sg_secondary_duty_mw,
            passive_loss_mw: snap.passive_heat_loss_mw,
            circulator_work_mw: snap.circulator_power_mw,
            riser_heat_mw: snap.riser_heat_mw,
            reflector_k: snap.reflector_temp_k,
            rpv_k: snap.rpv_temp_k,
            e_source_j: snap.energy_source_j,
            e_stored_j: snap.energy_stored_j,
            e_to_steam_generator_j: snap.energy_to_steam_generator_j,
            e_to_rccs_j: snap.energy_to_rccs_j,
            e_circulator_work_j: snap.energy_circulator_work_j,
            e_residual_j: snap.energy_residual_j,
        }
    }

    fn csv_header() -> &'static str {
        TraceRow::csv_header()
    }

    fn sample_csv(s: &TraceRow) -> String {
        s.to_csv()
    }
}

/// Advance a fresh plant for `cfg.steps` and return the sampled trace.
///
/// Single-threaded, no clock, no I/O. See the module docs on determinism.
pub fn run(cfg: &HeadlessConfig) -> Vec<TraceRow> {
    let mut model = HtgrHeadless::new();
    outram_park_digital_twin_engine::prelude::run(
        &mut model,
        cfg.controls.clone(),
        HeadlessRun {
            steps: cfg.steps,
            sample_every: cfg.sample_every,
        },
    )
}

/// Run and print the trace as CSV on stdout. The `--headless` entry point.
pub fn run_and_print(cfg: &HeadlessConfig) {
    println!("{}", TraceRow::csv_header());
    for row in run(cfg) {
        println!("{}", row.to_csv());
    }
}

/// Print the Map tab's **Bounding air ingress** table (#453) as CSV: the
/// `--bounding-air-ingress` entry point. It is a bounding case, not a
/// transient (#420), so it runs no plant; the numbers are
/// `sembawang::lwr_comparison::bounding_comparison`'s, cached by
/// [`crate::physics::bounding_air_ingress::comparison`].
pub fn print_bounding_air_ingress() {
    use crate::physics::bounding_air_ingress as b;
    use sembawang::lwr_comparison::{Tier, ARM_COLUMNS};
    println!("# {}", b::CASE_LABEL);
    println!("# basis: {}", b::BASIS_LABEL);
    println!("# tier 1, {}", Tier::DesignBasis.label());
    println!("#   {}", b::DBA_LABEL);
    println!("#   {}", b::LWR_LABEL);
    println!("# tier 2, {}", Tier::BeyondDesignBasis.label());
    println!("#   HTR-10 KORA bound ({})", b::CASE_LABEL);
    println!("#   {}", b::SEVERE_LABEL);
    println!("#   assumption: {}", b::CONTAINED_LABEL);
    println!("# {}", b::WASH_LABEL);
    println!(
        "# natural deposition (both LWR LOCA arms): {}",
        b::DEPOSITION_LABEL
    );
    println!("# {}", b::f_ox_provenance());
    match b::comparison() {
        Err(e) => println!("# unavailable: {e}"),
        Ok(c) => {
            let i = c.incomplete;
            println!(
                "# FGR-incomplete share of released Bq (dose is a LOWER BOUND where > 0, #456): \
                 htr10_dba {:.4}, lwr_dba {:.4}, htr10_bound {:.4}, lwr_severe_loca {:.4}, \
                 wash1400 {:.4}",
                i.htr10_dba, i.lwr_dba, i.htr10_bound, i.lwr_severe_loca, i.wash1400
            );
            let keys: Vec<&str> = ARM_COLUMNS.iter().map(|k| k.csv_key).collect();
            println!("distance_m,class,{}", keys.join(","));
            for r in &c.rows {
                let cells: Vec<String> = r
                    .arm_doses_sv()
                    .iter()
                    .map(|v| v.map_or("pending".to_string(), |s| format!("{:.6e}", 1e3 * s)))
                    .collect();
                println!("{:.0},{:?},{}", r.distance_m, r.class, cells.join(","));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property the reference baseline rests on. If this fails, no
    /// committed fixture means anything.
    #[test]
    fn determinism_same_config_same_trace() {
        let cfg = HeadlessConfig {
            steps: 120,
            sample_every: 20,
            ..Default::default()
        };
        assert_eq!(run(&cfg), run(&cfg), "headless run is not deterministic");
    }

    /// **The reference baseline for the execution refactor (bead `op-fbou`).**
    ///
    /// Compares a headless run against `reference/baseline_default_commands.csv`,
    /// captured 2026-09-06 **before** any execution change.
    ///
    /// Its job is to prove that extracting kernels and changing who schedules
    /// them **changed nothing about the physics**. Serial extraction should keep
    /// this bit-exact; that is why the comparison is exact rather than
    /// tolerance-based. When parallel execution lands and reduction order
    /// legitimately changes, add a *separate* tolerance-based comparison rather
    /// than loosening this one.
    ///
    /// **This is a baseline of the current PEBBLE-BED model, and of what it does
    /// rather than what it should do.** It is not an HTR-10 reference and must
    /// never be cited as one — see `op-jyyp.11` for that. It becomes
    /// intentionally obsolete when `op-jyyp` rewrites the physics.
    ///
    /// ## Why the comparison is numeric rather than byte-exact (2026-09-10)
    ///
    /// It was byte-exact as originally written. That held only for as long as
    /// the compiler did not change. Upgrading the toolchain to satisfy
    /// `egui 0.36`'s MSRV (rustc 1.94.1 -> 1.98.1) moved **3 of the 121 rows by
    /// exactly one unit in the ninth printed decimal**:
    ///
    /// | row | field | reference | rustc 1.98.1 |
    /// |---|---|---|---|
    /// | 49 | `fuel_temperature_k` | 958.886919432 | 958.886919433 |
    /// | 57 | `reactor_power_mw` | 13.606869492 | 13.606869491 |
    /// | 119 | `fuel_temperature_k` | 952.091743384 | 952.091743385 |
    ///
    /// That is a relative difference of ~1e-12 — a handful of f64 ULP,
    /// consistent with a codegen change (instruction selection / FMA
    /// contraction) and inconsistent with any change in the physics, which
    /// would move these values in their leading digits, not their last. The
    /// two toolchains could not be A/B'd directly: rustc 1.94.1 can no longer
    /// build this crate at all, which is why it was replaced.
    ///
    /// So the comparison is now **field-by-field to 2e-9 absolute**, one unit
    /// in the last printed place. `step` still matches exactly, and so does
    /// the header. This keeps every bit of the test's original power — the
    /// refactor it guards would have to change a value by less than a
    /// part in 1e12 to slip through, which is not a thing an execution change
    /// does — while not making a compiler upgrade look like a physics
    /// regression.
    ///
    /// ## Regenerated 2026-09-29 (gh:#388, #391-#394) -- an intended physics change
    ///
    /// Regenerated with the command in the assertion message after the
    /// primary helium circuit became enthalpy-balance CVs (core-outlet lag and
    /// clamp deleted, cold-return CV with circulator work, bed fluid row on
    /// enthalpy over the 197 cm bed) and the trace gained 17 columns. The
    /// kinetics/bed columns moved (before -> after, same build otherwise):
    ///
    /// | t | power | fuel node | bed |
    /// |---|---|---|---|
    /// | 0.1 s | 134.836987 -> 134.832263 MW | 972.4238 -> 972.4267 K | 950.0836 -> 950.0941 K |
    /// | 2.6 s | 334.397946 -> 334.476671 MW | 1396.5857 -> 1396.6020 K | 1050.6255 -> 1050.5726 K |
    /// | 100.1 s | 12.515127 -> 12.465065 MW | 1329.6669 -> 1329.6947 K | 1313.6666 -> 1313.7537 K |
    /// | 297.6 s | 16.162937 -> **16.115180 MW (-0.30 %)** | 1323.4121 -> 1323.3959 K | 1303.4258 -> **1303.4671 K** |
    ///
    /// The late-time power falls by ~48 kW: the 52 kW of circulator work now
    /// reaches the helium (it was computed and discarded), so the core needs
    /// that much less fission power at the same feedback balance. At 300 s the
    /// new columns read core inlet 522.67 K, core outlet 1206.47 K, SG helium
    /// duty 15.3168 MW, passive loss 0.7345 MW, circulator work 0.0519 MW, and
    /// a cumulative ledger residual of -4.0e-4 J on 8.16e9 J of source.
    ///
    /// ## Regenerated again 2026-09-29 (gh:#395, #396) -- the passive path in the bed's solve
    ///
    /// | t | power | bed | passive loss | reflector |
    /// |---|---|---|---|---|
    /// | 0.1 s | 134.832263 -> 134.831961 MW | 950.0941 -> 950.0948 K | 0.2541 -> 0.2859 MW | 684.74 -> 736.19 K |
    /// | 100.1 s | 12.465065 -> 12.478451 MW | 1313.7537 -> 1313.7282 K | 0.7505 -> 0.8027 MW | 685.00 -> 736.58 K |
    /// | 297.6 s | 16.115180 -> **16.125588 MW (+0.06 %)** | 1303.4671 -> 1303.4583 K | 0.7345 -> **0.7848 MW (+6.8 %)** | 685.53 -> 737.38 K |
    ///
    /// The passive loss at power rises by the Achenbach flow-dispersion and
    /// wall-film legs (the bed -> reflector leg is more conductive under
    /// forced flow), and the reflector opens in equilibrium with that, 51 K
    /// hotter. Ledger residual at 300 s: +3.1e-4 J on 8.16e9 J.
    ///
    /// ## Regenerated again 2026-09-29 (gh:#397) -- the riser leg
    ///
    /// | t | power | core inlet | passive loss / to risers | reflector / RPV |
    /// |---|---|---|---|---|
    /// | 0.1 s | 134.831961 -> 134.832817 MW | 515.98 -> 516.79 K | 0.2859 -> 0.5559 / 0.4451 MW | 736.2 -> 569.0 / 500.6 -> 426.1 K |
    /// | 297.6 s | 16.125588 -> **16.114366 MW** | 522.24 -> **537.40 K** | 0.7848 -> 1.0936 / 0.3334 MW | 737.4 -> 571.0 / 500.6 -> 426.1 K |
    ///
    /// The risers hold the reflector ~166 K cooler, so more heat leaves the
    /// bed sideways; about a third of it returns to the core inlet through the
    /// cold return. Ledger residual at 300 s: -1.4e-4 J on 8.16e9 J.
    ///
    /// ## Not regenerated since the gh:#403 inventory change, the SG merge
    /// (gh:#319), the single beta (gh:#387) and the uncredited building
    /// (gh:#409)
    ///
    /// Also not re-measured since af7991ca2a (cold return split into cold-duct + RPV-annuli CVs); pending validation work.
    ///
    /// The fixture is stale against all four; per the maintainer's
    /// 2026-09-29 direction it is **not re-measured; pending validation
    /// work**. Expect this ignored test to fail until then.
    ///
    /// **This is not the loosening the note below warns against.** When
    /// parallel execution lands and reduction order legitimately changes,
    /// *that* still wants its own separate, deliberately-tolerant comparison;
    /// this tolerance is far too tight to absorb it.
    #[test]
    #[ignore = "over the 1-minute headless budget (maintainer direction, 2026-09-27): settles or sweeps the WHOLE plant, which runs at ~4.5x real time, so this is minutes to tens of minutes. Run explicitly with --ignored when the transient itself is the subject."]
    fn matches_the_recorded_reference_baseline() {
        let fixture = include_str!("reference/baseline_default_commands.csv");
        let cfg = HeadlessConfig {
            steps: 3000,
            sample_every: 25,
            ..Default::default()
        };

        let produced: Vec<String> = std::iter::once(TraceRow::csv_header().to_string())
            .chain(run(&cfg).iter().map(|r| r.to_csv()))
            .collect();
        let expected: Vec<&str> = fixture.lines().filter(|l| !l.is_empty()).collect();

        assert_eq!(
            produced.len(),
            expected.len(),
            "row count changed: produced {} vs reference {}",
            produced.len(),
            expected.len()
        );

        /// One unit in the last place printed by [`TraceRow::to_csv`]'s
        /// `{:.9}`. Two rows that agree to this have the same physics; see the
        /// toolchain note on the enclosing test for why exact string equality
        /// is not the right bar.
        const LAST_PRINTED_PLACE: f64 = 2e-9;

        let regenerate = "If this is an intended physics change, regenerate with:\n  \
             cargo run --release --example htgr_sim_v1 -- --headless 3000 25 \
             > examples/htgr_sim_v1/reference/baseline_default_commands.csv\n  \
             If it is NOT intended, the execution refactor changed the physics.";

        // Row 0 is the header: no numbers in it, so it must match verbatim.
        assert_eq!(
            produced[0], expected[0],
            "trace header changed\n  produced: {}\n  reference: {}\n{regenerate}",
            produced[0], expected[0]
        );

        for (i, (got, want)) in produced.iter().zip(expected.iter()).enumerate().skip(1) {
            let got_fields: Vec<&str> = got.split(',').collect();
            let want_fields: Vec<&str> = want.split(',').collect();
            assert_eq!(
                got_fields.len(),
                want_fields.len(),
                "field count changed at row {i}\n  produced: {got}\n  reference: {want}"
            );

            // `step` is an integer index, not a measurement -- exact or bust.
            assert_eq!(
                got_fields[0], want_fields[0],
                "step index diverged at row {i}\n  produced: {got}\n  reference: {want}"
            );

            for (column, (g, w)) in got_fields.iter().zip(want_fields.iter()).enumerate().skip(1) {
                let g: f64 = g.parse().expect("produced field is not a number");
                let w: f64 = w.parse().expect("reference field is not a number");
                assert!(
                    (g - w).abs() <= LAST_PRINTED_PLACE,
                    "reference baseline diverged at row {i}, column {column} \
                     ({g} vs {w}, delta {:.3e} > {LAST_PRINTED_PLACE:.0e})\n  \
                     produced: {got}\n  reference: {want}\n{regenerate}",
                    g - w
                );
            }
        }
    }

    /// **The global energy balance closes from the CSV itself** (gh:#394).
    ///
    /// Methodology: 60 s headless run from the default commands, sampled every
    /// 10 steps; re-parse each printed row and require `e_source +
    /// e_circulator_work - e_stored - e_to_steam_generator - e_to_rccs` to
    /// equal `e_residual` to the printing precision (13 significant figures
    /// of the largest term), and `|e_residual|` below 1e-9 of `e_source`. This
    /// checks that the columns are the ledger's, not merely that the plant
    /// conserves energy (that is `physics::tests::the_whole_plant_conserves_energy_from_fission_to_the_steam_generator`).
    #[test]
    fn the_trace_carries_a_closing_energy_balance() {
        let rows = run(&HeadlessConfig {
            steps: 600,
            sample_every: 10,
            ..Default::default()
        });
        for row in &rows {
            let fields: Vec<f64> = row
                .to_csv()
                .split(',')
                .map(|f| f.parse().expect("numeric field"))
                .collect();
            let n = fields.len();
            let (src, stored, sg, rccs, work, res) = (
                fields[n - 6],
                fields[n - 5],
                fields[n - 4],
                fields[n - 3],
                fields[n - 2],
                fields[n - 1],
            );
            let scale = src.abs().max(stored.abs()).max(sg.abs()).max(1.0);
            assert!(
                (src + work - stored - sg - rccs - res).abs() <= 1e-11 * scale,
                "row {}: the printed ledger does not close",
                row.step
            );
            assert!(
                res.abs() <= 1e-9 * src.abs().max(1.0),
                "row {}: residual {res} J",
                row.step
            );
        }
    }

    /// A plant stepped from `PlantCommands::default()` should hold near its
    /// operating point, not run away. This is a sanity check on the harness,
    /// deliberately loose -- it is NOT a physics validation.
    #[test]
    fn default_commands_hold_the_plant_bounded() {
        let trace = run(&HeadlessConfig {
            steps: 600,
            sample_every: 100,
            ..Default::default()
        });
        assert!(!trace.is_empty());
        for row in &trace {
            assert!(
                row.reactor_power_mw.is_finite() && row.fuel_temperature_k.is_finite(),
                "non-finite state at step {}: {:?}",
                row.step,
                row
            );
            assert!(
                row.fuel_temperature_k > 250.0 && row.fuel_temperature_k < 3000.0,
                "fuel temperature left any physical range at step {}: {} K",
                row.step,
                row.fuel_temperature_k
            );
        }
    }
}
