//! Can a solved crude cut be handed to the rest of the crate as a stream?
//!
//! Issue #75 asks whether this crate's public API produces objects the *next*
//! unit operation can consume, rather than display values. Issue #73 asks the
//! same question as "stream identity across units". Until `CutResult` carried
//! a composition, the answer was demonstrably no: a cut was a flow, a
//! temperature and a label, and no amount of API polish turns that into a
//! feed.
//!
//! This test is the executable form of the answer.

use outram_park_fork_dwsim_libs::flowsheet::streams::MaterialStreamData;
use outram_park_fork_dwsim_libs::prelude::*;
use outram_park_fork_dwsim_libs::columns::thermo_bridge::ColumnThermo;
use outram_park_fork_dwsim_libs::flowsheet::streams::{MolarFlowRate, PhaseIndex};
use uom::si::catalytic_activity::katal;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

/// How far a converged cut gets towards being a consumable stream — and the
/// exact point where it stops.
///
/// # Methodology
///
/// Solve the default atmospheric crude column, then for each product cut build
/// a `MaterialStreamData` from *only* what the public API hands back:
///
/// - compounds and molar masses from `CrudeColumnResult::components`
///   (`Component::molar_mass` is kg/mol; `StreamCompound` wants DWSIM's
///   internal kg/kmol, hence the factor 1000);
/// - overall molar composition from `CutResult::composition`;
/// - temperature from `CutResult::temperature_k`, pressure from the column
///   configuration, molar flow from `CutResult::flow_mol_s` (the crate's
///   `MolarFlowRate` is `uom`'s `CatalyticActivity`, whose `katal` is mol/s);
/// - molar enthalpy from `ColumnThermo::liquid_molar_enthalpy` — every product
///   of this column is a liquid (total condenser, liquid side draws, bottoms).
///
/// Then ask `MaterialStreamData::validate` to accept it.
///
/// # Results (measured 2026-09-11, release mode)
///
/// All five products of `BlackOilCrude::light_sweet()` +
/// `CrudeColumnConfig::atmospheric_default()` at `cut_count = 12` build with a
/// 12-compound list, a composition that round-trips to within 1e-12, and a
/// finite molar enthalpy. The five molar flows sum to the 1.0 mol/s feed.
///
/// **`validate()` nonetheless rejects every one of them**, with
/// `InvalidSpecValue { property: "entropy" }`. Before the enthalpy was filled
/// in it rejected them on `"enthalpy"` instead, so the two are a queue, not a
/// single gate: `validate` requires temperature, pressure, enthalpy **and**
/// entropy on the mixture phase (`streams.rs:1221-1240`).
///
/// # Interpretation — this is the F2 gap, reached from the other side
///
/// `docs/upstream-port-coverage.md` records F2 ("MaterialStream property
/// calculation") as `MISSING + REQUIRED`: nothing turns a
/// `MaterialStreamData` + `StreamSpec` + `PropertyPackageModel` into a solved
/// stream. This test shows what that costs in practice at the *crude* end,
/// and adds a detail the audit did not have: **the blocker for a petroleum cut
/// is entropy specifically, and it is a data gap, not only a missing driver.**
/// `PseudoComponent`'s own documentation states that
/// `Component::ig_entropy_formation_25c` is left `f64::NAN` and the ideal-gas
/// `Cp` coefficients at zero, because DWSIM does not estimate either for
/// pseudo-components. So even once F2's flash driver exists, a mixture entropy
/// for a characterised crude has no data to stand on. That is worth knowing
/// before F2 is scoped.
///
/// Asserting the failure rather than papering over it is deliberate: an
/// entropy *could* be produced from `ideal_props::mixture_ideal_gas_entropy`
/// with the zeroed coefficients, and it would be a finite number with no
/// physical content. Writing that into a stream would make this test pass and
/// the crate less honest.
///
/// **When F2 lands this test will fail**, which is the intent — it is the
/// tripwire that says the gap closed, and whoever closes it should rewrite the
/// assertion into the positive form.
///
/// This is a harness check on API reachability, not physics validation.
#[test]
fn a_crude_cut_reaches_a_stream_but_stops_at_entropy() {
    let crude = BlackOilCrude::light_sweet();
    let config = CrudeColumnConfig::atmospheric_default();
    let result = solve_crude_column(&crude, &config, 12).expect("column should solve");

    // Enthalpy needs a thermo bridge over the SAME component basis as the
    // compositions — the whole slate, not the column's light end.
    let all_components: Vec<_> = result
        .components
        .iter()
        .map(|pc| pc.component.clone())
        .collect();
    let thermo = ColumnThermo::new(all_components, config.package);

    let mut total_flow = 0.0;

    for cut in &result.cuts {
        let mut stream = MaterialStreamData::new();
        for pc in &result.components {
            // kg/mol -> kg/kmol, DWSIM's internal unit for StreamCompound.
            stream.add_compound(pc.component.name.clone(), pc.component.molar_mass * 1000.0);
        }
        assert_eq!(
            stream.compound_count(),
            cut.composition.len(),
            "the composition must index the compounds one for one"
        );

        stream
            .set_overall_molar_composition(&cut.composition)
            .unwrap_or_else(|e| panic!("stage {}: composition rejected: {e:?}", cut.stage));
        stream.set_temperature(ThermodynamicTemperature::new::<kelvin>(cut.temperature_k));
        stream.set_pressure(Pressure::new::<pascal>(config.pressure_pa));
        stream.set_molar_flow(MolarFlowRate::new::<katal>(cut.flow_mol_s));

        let h =
            thermo.liquid_molar_enthalpy(&cut.composition, cut.temperature_k, config.pressure_pa);
        assert!(
            h.is_finite(),
            "stage {}: liquid molar enthalpy is not finite ({h})",
            cut.stage
        );
        stream.phase_mut(PhaseIndex::Mixture).properties.enthalpy = Some(h);

        // The honest result: everything above is reachable from the public
        // API, and it is still not enough. See this test's doc comment.
        let verdict = stream.validate(&format!("cut_stage_{}", cut.stage));
        let err = verdict.expect_err(
            "validate() unexpectedly ACCEPTED the stream — if the F2 stream-property \
             gap has been closed, rewrite this test into its positive form rather \
             than deleting the assertion",
        );
        assert!(
            format!("{err:?}").contains("entropy"),
            "stage {}: expected validation to stop at entropy, got {err:?}",
            cut.stage
        );

        // The composition survives the round trip through the stream model.
        let mixture = stream.phase(PhaseIndex::Mixture);
        for (i, compound) in mixture.compounds.iter().enumerate() {
            let stored = compound.mole_fraction.expect("mole fraction was set");
            assert!(
                (stored - cut.composition[i]).abs() < 1e-12,
                "stage {}: compound {i} round-tripped {stored} vs {}",
                cut.stage,
                cut.composition[i]
            );
        }

        total_flow += cut.flow_mol_s;
    }

    assert!(
        (total_flow - config.feed_flow_mol_s).abs() < 1e-9,
        "the streams built from the cuts carry {total_flow} mol/s against a \
         {} mol/s feed",
        config.feed_flow_mol_s
    );
}
