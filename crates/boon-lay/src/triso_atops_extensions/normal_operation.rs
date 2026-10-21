// SPDX-License-Identifier: GPL-3.0
//
// NOT a port (gh:#583, 2026-10-05). The per-node normal-operation chain of
// `crate::triso_atops_fork::normal_operation::normal_operation_node` (which
// ports `trisoatops.py::normal_operation` and
// `calculation_functions.py::higher_activities`, TRISO-ATOPS commit de374c8),
// with the plate-out / purification routing taken from a `RemovalRates`
// instead of upstream's fixed per-group switches. Every physics step calls the
// ported function unchanged; only the choice of rate constants is new. The
// ported module is not modified.

//! # Normal operation with element-dependent removal rates
//!
//! [`normal_operation_node_with_rates`] runs the same five steps as the ported
//! [`normal_operation_node`](crate::triso_atops_fork::normal_operation::normal_operation_node),
//! each through the same ported function:
//!
//! 1. `diffusion_coefficient` (kernel and graphite);
//! 2. `rb_fail`;
//! 3. `release_rate`;
//! 4. `base_activities` (coolant source `S`, graphite hold-up `G`);
//! 5. the closed-form pools `circulating`, `plate_out`, `clean_up`.
//!
//! The only difference is step 5's rate constants: they come from
//! [`RemovalRates::for_element`] for the nuclide's atomic number.
//!
//! **One rule replaces upstream's group switches:** a pool whose own rate is
//! zero is zero. Upstream forces the plated pool of a noble gas to 0, and the
//! purification pool to 0 for anything but a noble gas or a halogen, and for
//! everything when the purification system is off. Under
//! [`RemovalRates::upstream`] (with [`RemovalRates::without_purification`] for
//! the "off" case) those are exactly the pools whose rate is zero, so the rule
//! reproduces the port **bit for bit**, for all 84 supported nuclides, with and
//! without parent pools (tested below).
//!
//! **Where the rule differs from upstream:** if a caller sets a non-zero rate
//! for a pool upstream forces to zero (purification of caesium, for example),
//! that pool is now computed, including its parent in-growth term. If a caller
//! sets a zero rate where upstream's rate is non-zero, the pool is zero and
//! does **not** inherit the parent's pool. Upstream never meets either case.

use uom::si::f64::{Frequency, Time};
use uom::si::frequency::hertz;

use super::removal_rates::RemovalRates;
use crate::triso_atops_fork::activities::{
    base_activities, circulating, clean_up, plate_out, release_rate, FailureFractions,
};
use crate::triso_atops_fork::diffusion::diffusion_coefficient;
use crate::triso_atops_fork::normal_operation::{NodalActivities, NodeState, ParentPools};
use crate::triso_atops_fork::release_models::rb_fail;
use crate::triso_atops_fork::{Activity, TrisoAtopsNuclide};
use uom::si::f64::Length;

/// The ported normal-operation chain for one nuclide at one node, with the
/// removal rates from `rates` (see the module doc).
///
/// # Arguments
/// - `nuclide`, `short_lived`, `inventory`, `fractions`, `node`, `parent`: as
///   for the ported `normal_operation_node`.
/// - `rates`: the plate-out and purification rates by element
///   ([`RemovalRates::upstream`] for upstream's model).
/// - `graphite_thickness`, `grain_size`, `sic_thickness`, `kernel_radius`,
///   `run_time`, `irradiation_time`: the fields of the ported
///   `PlantConstants` other than its two rates, which `rates` replaces.
///
/// # Returns
/// The same [`NodalActivities`] as the port (effective units; convert with
/// [`NodalActivities::to_curies`]).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn normal_operation_node_with_rates(
    nuclide: &TrisoAtopsNuclide,
    short_lived: bool,
    inventory: Activity,
    fractions: FailureFractions,
    rates: &RemovalRates,
    graphite_thickness: Length,
    grain_size: Length,
    sic_thickness: Length,
    kernel_radius: Length,
    run_time: Time,
    irradiation_time: Time,
    node: NodeState,
    parent: ParentPools,
) -> NodalActivities {
    let z = nuclide.z;
    let lam = nuclide.decay_constant();
    let group = nuclide.element_group();

    // Steps 1-4: the ported functions, called as the port calls them.
    let diff = diffusion_coefficient(z, node.core_temperature, node.graphite_temperature);
    let rb = rb_fail(
        z,
        short_lived,
        lam,
        node.core_temperature,
        irradiation_time,
        grain_size,
        sic_thickness,
        kernel_radius,
        diff.kernel,
    );
    let r = release_rate(rb, group, fractions, inventory, short_lived, irradiation_time, lam);
    let sg = base_activities(group, lam, irradiation_time, graphite_thickness, diff.graphite, r);

    // Step 5: this element's rates; a pool whose own rate is zero is zero.
    let own = rates.for_element(z);
    let (k_plate, k_clean): (Frequency, Frequency) = (own.k_plate, own.k_clean);
    let c = circulating(sg.source_rate, k_plate, lam, run_time, k_clean, parent.circulating);
    let p = if k_plate.get::<hertz>() > 0.0 {
        plate_out(k_plate, sg.source_rate, lam, run_time, c, k_clean, parent.plate_out)
    } else {
        0.0
    };
    let hps = if k_clean.get::<hertz>() > 0.0 {
        clean_up(k_plate, sg.source_rate, lam, run_time, c, k_clean, parent.clean_up)
    } else {
        0.0
    };

    NodalActivities {
        release_rate: r,
        source_rate: sg.source_rate,
        graphite_activity: sg.graphite_activity,
        circulating_activity: c,
        plate_out_activity: p,
        clean_up_activity: hps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triso_atops_extensions::removal_rates::GroupRates;
    use crate::triso_atops_fork::activities::becquerels_from_curies;
    use crate::triso_atops_fork::nuclide_model::{supported_nuclides, ElementGroup};
    use crate::triso_atops_fork::normal_operation::{normal_operation_node, PlantConstants};
    use std::collections::HashMap;
    use uom::si::f64::ThermodynamicTemperature;
    use uom::si::length::meter;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::second;

    const FRACTIONS: FailureFractions = FailureFractions {
        heavy_metal: 1e-4,
        sic: 1e-4,
        incremental: 2.3e-5,
        incremental_sic: 3.6e-5,
    };

    /// The ported module's own test plant (`normal_operation::tests::plant`).
    fn plant() -> PlantConstants {
        let yr = 3.155_76e7;
        PlantConstants {
            k_plate: Frequency::new::<hertz>(7.5e-4),
            k_clean: Frequency::new::<hertz>(8.77e-5),
            graphite_thickness: Length::new::<meter>(0.0045),
            grain_size: Length::new::<meter>(1e-5),
            sic_thickness: Length::new::<meter>(3.5e-5),
            kernel_radius: Length::new::<meter>(0.000213),
            run_time: Time::new::<second>(40.0 * yr),
            irradiation_time: Time::new::<second>(3.0 * yr),
        }
    }

    fn node(c: f64) -> NodeState {
        let t = ThermodynamicTemperature::new::<degree_celsius>(c);
        NodeState { core_temperature: t, graphite_temperature: t }
    }

    /// The six outputs' bit patterns: "identical" including NaN and inf
    /// (Se-82 is effectively stable, and the port itself returns inf / NaN for
    /// it; `NaN != NaN`, so `==` on the floats cannot say "same").
    fn bits(a: &NodalActivities) -> [u64; 6] {
        [
            a.release_rate,
            a.source_rate,
            a.graphite_activity,
            a.circulating_activity,
            a.plate_out_activity,
            a.clean_up_activity,
        ]
        .map(f64::to_bits)
    }

    fn with_rates(
        n: &TrisoAtopsNuclide,
        sl: bool,
        rates: &RemovalRates,
        at: NodeState,
        parent: ParentPools,
    ) -> NodalActivities {
        let p = plant();
        normal_operation_node_with_rates(
            n,
            sl,
            becquerels_from_curies(44.5),
            FRACTIONS,
            rates,
            p.graphite_thickness,
            p.grain_size,
            p.sic_thickness,
            p.kernel_radius,
            p.run_time,
            p.irradiation_time,
            at,
            parent,
        )
    }

    /// **Reduces to the port, bit for bit.**
    ///
    /// Methodology: every one of the 84 supported nuclides, in table order,
    /// at 700 C and 1100 C, with the purification system on and off. A
    /// nuclide whose parent comes earlier in the table gets that parent's
    /// ported pools, so the parent in-growth terms are exercised too. The
    /// extension with `RemovalRates::upstream(k_plate, k_clean)` (plus
    /// `without_purification` when off) is compared with the ported
    /// `normal_operation_node` on the same inputs. Pass: every one of the six
    /// outputs identical, bit for bit (not a tolerance; NaN and inf included:
    /// the port returns them for the effectively stable Se-82, and so must
    /// the extension). **Result (2026-10-05):** 84 x 2 x 2 = 336 cases, all
    /// identical.
    #[test]
    fn upstream_rates_reproduce_the_port_bit_for_bit() {
        let p = plant();
        let t_irr = p.irradiation_time.get::<second>();
        let mut cases = 0;
        for hps in [true, false] {
            let mut rates = RemovalRates::upstream(p.k_plate, p.k_clean);
            if !hps {
                rates = rates.without_purification();
            }
            for c in [700.0, 1100.0] {
                let mut done: HashMap<&str, NodalActivities> = HashMap::new();
                for n in supported_nuclides() {
                    let sl = n.half_life.get::<second>() / t_irr < 0.2;
                    let parent = n
                        .parents
                        .first()
                        .and_then(|name| done.get(name))
                        .map_or(ParentPools::none(), NodalActivities::parent_pools);
                    let ported = normal_operation_node(
                        &n,
                        sl,
                        becquerels_from_curies(44.5),
                        FRACTIONS,
                        p,
                        node(c),
                        hps,
                        parent,
                    );
                    let ours = with_rates(&n, sl, &rates, node(c), parent);
                    assert_eq!(bits(&ours), bits(&ported), "{} at {c} C, HPS {hps}", n.name);
                    done.insert(n.name, ported);
                    cases += 1;
                }
            }
        }
        println!("{cases} cases identical to the port");
        assert_eq!(cases, 336);
    }

    /// **Changing one group changes only that group.**
    ///
    /// Methodology: from upstream's rates, give the special metals their own
    /// plate-out (ten times upstream's) and give caesium purification (which
    /// upstream never applies). Expected: Cs-137 circulates less (with
    /// `k_plate` far above lambda, C ~ S / (k_plate + k_clean)) and gains a
    /// purification pool; Sr-90 (same group, no override) circulates less but
    /// has no purification pool; I-131 and Kr-85 are unchanged, bit for bit.
    /// (The plated pool of a long-lived metal hardly moves: over 40 years
    /// nearly every atom plates out at either rate.)
    /// **Result (2026-10-05):** passes.
    #[test]
    fn one_group_or_element_changes_only_itself() {
        let p = plant();
        let base = RemovalRates::upstream(p.k_plate, p.k_clean);
        let changed = base
            .clone()
            .with_group(
                ElementGroup::SpecialMetal,
                GroupRates { k_plate: p.k_plate * 10.0, k_clean: Frequency::new::<hertz>(0.0) },
            )
            .with_element(55, GroupRates { k_plate: p.k_plate * 10.0, k_clean: p.k_clean });
        let get = |name: &str, r: &RemovalRates| {
            let n = supported_nuclides().into_iter().find(|n| n.name == name).unwrap();
            with_rates(&n, false, r, node(900.0), ParentPools::none())
        };
        let (cs0, cs1) = (get("Cs-137", &base), get("Cs-137", &changed));
        assert!(cs1.circulating_activity < 0.2 * cs0.circulating_activity);
        assert_eq!(cs0.clean_up_activity, 0.0);
        assert!(cs1.clean_up_activity > 0.0);
        let (sr0, sr1) = (get("Sr-90", &base), get("Sr-90", &changed));
        assert!(sr1.circulating_activity < 0.2 * sr0.circulating_activity);
        assert_eq!(sr1.clean_up_activity, 0.0);
        for name in ["I-131", "Kr-85"] {
            assert_eq!(bits(&get(name, &base)), bits(&get(name, &changed)), "{name}");
        }
    }
}
