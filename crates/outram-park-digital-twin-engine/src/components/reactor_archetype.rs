//! Reactor archetypes: the kinetics half.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! the vessel art, [`ReactorArchetype`] and [`ReactorArchetypeVisual`], is
//! [`bugis::components::reactor_archetype`], re-exported here. The one item
//! that built physics, `illustrative_kinetics` (a `nee_soon` model), lives
//! here as the [`ReactorArchetypeKinetics`] extension trait, so
//! `archetype.illustrative_kinetics()` keeps working once the trait is in
//! scope (`use outram_park_digital_twin_engine::components::ReactorArchetypeKinetics`).

pub use bugis::components::reactor_archetype::*;
use nee_soon::NordheimFuchsExactTimestepper;
use uom::si::f64::{HeatCapacity, Power, Ratio, TemperatureCoefficient, ThermodynamicTemperature, Time};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// Illustrative point kinetics for a [`ReactorArchetype`] (moved from the
/// bugis enum's inherent method, 2026-11-05).
pub trait ReactorArchetypeKinetics {
    /// A Nordheim-Fuchs prompt-excursion model with kinetics parameters
    /// **illustrative of this reactor type**.
    ///
    /// # These are illustrative, not design data
    ///
    /// **Nothing here represents a specific licensed design, and no value is
    /// sourced from a plant.** They are order-of-magnitude constants chosen so
    /// each reactor type behaves *qualitatively* like its class. Do not quote
    /// them, and do not use them in a V&V case without replacing them with
    /// sourced values — see the reactor's scoping document
    /// ([`ReactorArchetype::scoping_doc`]) for what open data exists.
    ///
    /// # What the differences between them mean
    ///
    /// The *relative ordering* is textbook physics, and is the point of having
    /// one model per reactor rather than one shared model:
    ///
    /// - **Prompt neutron generation time** spans four orders of magnitude.
    ///   Graphite-moderated thermal reactors (HTR-10, MSRE, FHR) sit near a
    ///   millisecond because a neutron rattles around a large moderator before
    ///   being absorbed; light-water reactors (iPWR, BWR) are far shorter
    ///   because water moderates in a much smaller volume; and a sodium **fast**
    ///   reactor (EBR-II) is of order a tenth of a microsecond, since there is
    ///   no thermalisation stage at all. This is why EBR-II responds to a
    ///   reactivity insertion so much faster than the others.
    /// - **Delayed neutron fraction** is near 0.0065 for the U-235-fuelled
    ///   cases. It matters because prompt criticality is reached at a
    ///   reactivity insertion of exactly this size.
    /// - **Fuel heat capacity** scales with core size, so it is derived from
    ///   [`ReactorArchetype::approximate_thermal_power_mw`]. It sets how fast fuel
    ///   temperature — and therefore the feedback — responds.
    /// - **Fuel temperature feedback** is negative for every reactor here, which
    ///   is what makes the excursion self-limiting. Note this lumps Doppler
    ///   together with everything else; for EBR-II in particular that is a real
    ///   simplification, because its behaviour is dominated by **core expansion
    ///   feedbacks that this model does not represent at all** (see
    ///   `docs/reactor-scoping/ebr2.md`, gap 1).
    ///
    /// The model starts at its reference temperature and near-zero power, so a
    /// caller drives it by inserting reactivity.
    fn illustrative_kinetics(self) -> NordheimFuchsExactTimestepper;
}

impl ReactorArchetypeKinetics for ReactorArchetype {
    fn illustrative_kinetics(self) -> NordheimFuchsExactTimestepper {
        // Prompt neutron generation time [s], delayed fraction [-],
        // fuel temperature feedback [1/K], reference temperature [degC].
        let (generation_time_s, beta, alpha_f_per_k, reference_degc) = match self {
            Self::Htr10 => (1.0e-3, 0.0065, -4.0e-5, 600.0),
            Self::Msre => (4.0e-4, 0.0064, -3.0e-5, 650.0),
            Self::IntegralPwr => (2.0e-5, 0.0065, -2.5e-5, 300.0),
            Self::Bwr => (4.0e-5, 0.0065, -3.0e-5, 285.0),
            Self::Fhr => (5.0e-4, 0.0065, -3.5e-5, 650.0),
            Self::EbrII => (1.0e-7, 0.0070, -2.0e-6, 370.0),
        };

        // Lumped whole-core fuel heat capacity, scaled off thermal rating. The
        // coefficient is chosen to give a plausible fuel time constant; it is
        // illustrative like everything else here.
        let heat_capacity_j_per_k = self.approximate_thermal_power_mw() * 4.0e5;

        let reference = ThermodynamicTemperature::new::<degree_celsius>(reference_degc);

        NordheimFuchsExactTimestepper::new(
            Time::new::<second>(generation_time_s),
            Ratio::new::<ratio>(beta),
            HeatCapacity::new::<joule_per_kelvin>(heat_capacity_j_per_k),
            TemperatureCoefficient::new::<per_kelvin>(alpha_f_per_k),
            reference,
            reference,
            Power::new::<watt>(1.0),
        )
        .expect(
            "illustrative kinetics constants are compile-time valid \
             (positive generation time and heat capacity, negative feedback); \
             every archetype is constructed in a unit test",
        )
    }
}

#[cfg(test)]
mod kinetics_tests {
    use super::*;

    /// Every archetype must build a valid Nordheim-Fuchs model.
    ///
    /// `illustrative_kinetics` unwraps internally, so this is what stops an
    /// invalid constant (non-positive generation time or heat capacity,
    /// non-negative feedback) reaching a caller as a panic.
    #[test]
    fn every_archetype_builds_a_valid_kinetics_model() {
        for a in ReactorArchetype::ALL {
            let k = a.illustrative_kinetics();
            assert!(
                k.prompt_neutron_generation_time.get::<second>() > 0.0,
                "{a:?} has non-positive generation time"
            );
            assert!(
                k.fuel_feedback_coefficient.get::<per_kelvin>() < 0.0,
                "{a:?} feedback must be negative to be self-limiting"
            );
        }
    }

    /// A fast reactor's prompt neutron generation time is orders of magnitude
    /// shorter than a thermal reactor's — there is no thermalisation stage.
    ///
    /// **Methodology.** Compare EBR-II's generation time against every thermal
    /// archetype's, requiring at least two orders of magnitude of separation.
    /// This is the qualitative difference that justifies one kinetics model per
    /// reactor rather than one shared model, so it is pinned rather than left
    /// to the constants happening to be right.
    ///
    /// **Result (2026-08-06):** EBR-II at 1e-7 s against 2e-5 s (iPWR, the
    /// shortest thermal case) — a factor of 200, and up to 1e4 against the
    /// graphite-moderated cases. Interpretation: the fast case responds to a
    /// reactivity insertion far faster than any thermal case, as it must.
    #[test]
    fn the_fast_reactor_has_a_far_shorter_generation_time() {
        let fast = ReactorArchetype::EbrII
            .illustrative_kinetics()
            .prompt_neutron_generation_time
            .get::<second>();

        for a in ReactorArchetype::ALL {
            if *a == ReactorArchetype::EbrII {
                continue;
            }
            let thermal = a
                .illustrative_kinetics()
                .prompt_neutron_generation_time
                .get::<second>();
            assert!(
                thermal > fast * 100.0,
                "{a:?} generation time {thermal} is not clearly slower than the fast case {fast}"
            );
        }
    }

    /// Graphite-moderated reactors must have longer generation times than
    /// light-water ones — a neutron rattles around a large moderator volume
    /// before absorption, where water thermalises in a much smaller one.
    #[test]
    fn graphite_moderated_cases_are_slower_than_light_water_cases() {
        let gen = |a: ReactorArchetype| {
            a.illustrative_kinetics()
                .prompt_neutron_generation_time
                .get::<second>()
        };

        for graphite in [
            ReactorArchetype::Htr10,
            ReactorArchetype::Msre,
            ReactorArchetype::Fhr,
        ] {
            for water in [ReactorArchetype::IntegralPwr, ReactorArchetype::Bwr] {
                assert!(
                    gen(graphite) > gen(water),
                    "{graphite:?} ({}) should be slower than {water:?} ({})",
                    gen(graphite),
                    gen(water)
                );
            }
        }
    }

    /// Delayed neutron fraction must stay near the U-235 value — prompt
    /// criticality is reached at exactly this insertion, so a wrong value would
    /// silently move the threshold the whole model is about.
    #[test]
    fn delayed_fractions_are_near_the_u235_value() {
        for a in ReactorArchetype::ALL {
            let beta = a
                .illustrative_kinetics()
                .delayed_neutron_fraction
                .get::<ratio>();
            assert!(
                (0.005..=0.008).contains(&beta),
                "{a:?} delayed fraction {beta} is outside the plausible band"
            );
        }
    }

    /// Heat capacity must scale with core size, so a small experimental reactor
    /// heats up faster than a power-reactor-scale core for the same energy.
    #[test]
    fn heat_capacity_scales_with_thermal_power() {
        let small = ReactorArchetype::Msre.illustrative_kinetics();
        let large = ReactorArchetype::Bwr.illustrative_kinetics();
        assert!(
            large.fuel_heat_capacity.get::<joule_per_kelvin>()
                > small.fuel_heat_capacity.get::<joule_per_kelvin>(),
            "a power-reactor core must have more heat capacity than an experimental one"
        );
    }
}
