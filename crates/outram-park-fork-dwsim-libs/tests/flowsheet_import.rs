//! Integration tests for [`outram_park_fork_dwsim_libs::flowsheet::import`] —
//! the read-only importer for DWSIM's saved reference flowsheets.
//!
//! # Methodology
//!
//! Eleven reference files shipped with DWSIM are committed under
//! `tests/fixtures/dwsim_reference/` (see the `References.md` there for their
//! provenance, licence and original paths). For each one the tests check
//! **invariants that were hand-derived from the XML**, not from the importer's
//! own output:
//!
//! - the object count and its split into material streams / energy streams /
//!   unit operations, and the connection count, each cross-checked against an
//!   independent count of `<SimulationObject>` and attached `<Connector>`
//!   elements in the file;
//! - specific tags that must be present;
//! - that every connection endpoint resolves to a real object and joins
//!   exactly one stream to exactly one non-stream, which is the flowsheet
//!   model's structural invariant;
//! - that overall mole fractions sum to 1 and that temperatures and pressures
//!   are physical (`T > 0 K`, `P > 0 Pa`);
//! - for two fixtures, exact stream values read out of the XML by hand.
//!
//! Plus: a `.dwxmz` and its extracted `.xml` must import identically; a
//! truncated document must return `Err`, not panic; and an unknown object type
//! must produce the right [`ImportGap`].
//!
//! **These are verification tests, not validation.** They check that the
//! importer recovers what the file says. No thermodynamics is evaluated and no
//! benchmark is reproduced — that is what these fixtures are *for*, once the
//! solver can run them.
//!
//! # Results (2026-08-11, release build)
//!
//! All 11 fixtures import. 8 import with **zero** gaps. 3 record exactly one
//! gap each:
//!
//! | Fixture | Objects | Connections | Gaps |
//! |---|---|---|---|
//! | `basic_distillation.dwxmz` | 6 | 5 | 1 unsupported property package (MODFAC) |
//! | `cavetts_problem.dwxmz` | 42 | 44 | none |
//! | `compression_and_expansion.dwxmz` | 32 | 24 | none |
//! | `esterification.dwxmz` | 13 | 13 | 1 unsupported property package (NRTL) |
//! | `heating_and_cooling.dwxmz` | 16 | 12 | none |
//! | `humid_air.dwxml` | 4 | 3 | none |
//! | `linde_lng_process.dwxmz` | 25 | 26 | none |
//! | `pump_and_valve.dwxmz` | 28 | 20 | 1 unsupported property package (LKP) |
//! | `three_phase_separator.dwxmz` | 5 | 4 | none |
//! | `wind_turbine.dwxmz` | 2 | 0 | 1 rejected connection (see below) |
//! | `wind_turbine_extracted.dwxml` | 2 | 0 | 1 rejected connection |
//!
//! The wind-turbine gap is a real finding, recorded rather than papered over:
//! this crate's `ENERGY_SOURCE_TYPES` (ported from
//! `DesignSurface.vb:1388-1390`) does not list `WindTurbine`, so the turbine's
//! duty outlet to its energy stream is refused. The fixture is kept precisely
//! because it pins that gap.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use outram_park_fork_dwsim_libs::flowsheet::import::{
    import_flowsheet_bytes, import_flowsheet_dwxml, import_flowsheet_file, GapCategory,
    ImportError, ImportGap, ImportedFlowsheet, SourceKind,
};
use outram_park_fork_dwsim_libs::flowsheet::streams::PhaseIndex;

/// Directory holding the committed reference fixtures.
fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwsim_reference")
}

fn import_fixture(name: &str) -> ImportedFlowsheet {
    let path = fixture_dir().join(name);
    import_flowsheet_file(&path).unwrap_or_else(|e| panic!("{name} failed to import: {e}"))
}

/// What one fixture must yield: file name, total objects, material streams,
/// energy streams, unit operations, connections, compounds, and the tags that
/// must be present.
struct Expected {
    file: &'static str,
    objects: usize,
    material_streams: usize,
    energy_streams: usize,
    unit_operations: usize,
    connections: usize,
    compounds: usize,
    tags: &'static [&'static str],
}

/// Hand-derived expectations for every committed fixture.
///
/// Object and connection counts were obtained independently of the importer, by
/// counting `<SimulationObject>` elements and `IsAttached="true"` connectors in
/// each file's `<GraphicObjects>` section. The tags are read off the files'
/// `<Tag>` elements.
const EXPECTED: &[Expected] = &[
    Expected {
        file: "basic_distillation.dwxmz",
        objects: 6,
        material_streams: 3,
        energy_streams: 2,
        unit_operations: 1,
        connections: 5,
        compounds: 7,
        tags: &[
            "DC-01", "MSTR-01", "MSTR-02", "MSTR-03", "ESTR-01", "ESTR-02",
        ],
    },
    Expected {
        file: "cavetts_problem.dwxmz",
        objects: 42,
        material_streams: 20,
        energy_streams: 7,
        unit_operations: 15,
        connections: 44,
        compounds: 15,
        tags: &["COMP-000", "COMP-001", "COMP-002"],
    },
    Expected {
        file: "compression_and_expansion.dwxmz",
        objects: 32,
        material_streams: 16,
        energy_streams: 8,
        unit_operations: 8,
        connections: 24,
        compounds: 3,
        tags: &["COMP-01", "COMP-04", "EXP-01", "EXP-04", "MSTR-01"],
    },
    Expected {
        file: "esterification.dwxmz",
        objects: 13,
        material_streams: 8,
        energy_streams: 1,
        unit_operations: 4,
        connections: 13,
        compounds: 4,
        tags: &["MIX-01", "SPLIT-01", "CONV-01", "REC-01"],
    },
    Expected {
        file: "heating_and_cooling.dwxmz",
        objects: 16,
        material_streams: 8,
        energy_streams: 4,
        unit_operations: 4,
        connections: 12,
        compounds: 3,
        tags: &["HEAT-01", "HEAT-02", "COOL-01", "COOL-02"],
    },
    Expected {
        file: "humid_air.dwxml",
        objects: 4,
        material_streams: 2,
        energy_streams: 1,
        unit_operations: 1,
        connections: 3,
        compounds: 3,
        tags: &["Air", "Dew", "COOL-000", "E"],
    },
    Expected {
        file: "linde_lng_process.dwxmz",
        objects: 25,
        material_streams: 12,
        energy_streams: 4,
        unit_operations: 9,
        connections: 26,
        compounds: 1,
        tags: &[
            "Compressor",
            "HE-01",
            "MIX-01",
            "SEP-01",
            "VALVE-01",
            "REC-01",
        ],
    },
    Expected {
        file: "pump_and_valve.dwxmz",
        objects: 28,
        material_streams: 16,
        energy_streams: 4,
        unit_operations: 8,
        connections: 20,
        compounds: 3,
        tags: &["PUMP-01", "PUMP-04", "MSTR-01", "MSTR-16"],
    },
    Expected {
        file: "three_phase_separator.dwxmz",
        objects: 5,
        material_streams: 4,
        energy_streams: 0,
        unit_operations: 1,
        connections: 4,
        compounds: 3,
        tags: &["feed", "gas", "oil", "water", "separator"],
    },
    Expected {
        file: "wind_turbine.dwxmz",
        objects: 2,
        material_streams: 0,
        energy_streams: 1,
        unit_operations: 1,
        // The turbine -> energy-stream edge is refused; see the module doc.
        connections: 0,
        compounds: 1,
        tags: &["1", "2"],
    },
    Expected {
        file: "wind_turbine_extracted.dwxml",
        objects: 2,
        material_streams: 0,
        energy_streams: 1,
        unit_operations: 1,
        connections: 0,
        compounds: 1,
        tags: &["1", "2"],
    },
];

/// **Methodology.** Import every committed fixture and compare its object
/// split, connection count, compound count and required tags with the
/// hand-derived [`EXPECTED`] table.
/// **Result (2026-08-11):** all 11 fixtures match exactly.
#[test]
fn every_fixture_imports_with_the_expected_shape() {
    for e in EXPECTED {
        let imported = import_fixture(e.file);
        let s = imported.summary();
        assert_eq!(s.objects, e.objects, "{}: object count", e.file);
        assert_eq!(
            s.material_streams, e.material_streams,
            "{}: material streams",
            e.file
        );
        assert_eq!(
            s.energy_streams, e.energy_streams,
            "{}: energy streams",
            e.file
        );
        assert_eq!(
            s.unit_operations, e.unit_operations,
            "{}: unit operations",
            e.file
        );
        assert_eq!(s.connections, e.connections, "{}: connections", e.file);
        assert_eq!(s.compounds, e.compounds, "{}: compounds", e.file);
        for tag in e.tags {
            assert!(
                imported.flowsheet.object_by_tag(tag).is_some(),
                "{}: missing object tagged `{tag}`",
                e.file
            );
        }
    }
}

/// **Methodology.** For every fixture, check the flowsheet model's structural
/// invariants on the imported graph: each connection's two endpoints resolve to
/// registered objects, and exactly one of them is a stream (the model forbids
/// stream-to-stream and unit-op-to-unit-op edges). Also check that each
/// endpoint's connector slot really records the peer, so the edge is stored
/// symmetrically rather than half-written.
/// **Result (2026-08-11):** 151 connections across the 11 fixtures, all
/// well-formed.
#[test]
fn every_connection_resolves_and_joins_a_stream_to_a_unit_operation() {
    let mut total = 0usize;
    for e in EXPECTED {
        let imported = import_fixture(e.file);
        for c in imported.flowsheet.connections() {
            let from = imported
                .flowsheet
                .object(&c.from)
                .unwrap_or_else(|| panic!("{}: dangling source {:?}", e.file, c.from));
            let to = imported
                .flowsheet
                .object(&c.to)
                .unwrap_or_else(|| panic!("{}: dangling destination {:?}", e.file, c.to));
            assert_ne!(
                from.object_type.is_stream(),
                to.object_type.is_stream(),
                "{}: `{}` -> `{}` does not join a stream to a unit operation",
                e.file,
                from.tag,
                to.tag
            );
            total += 1;
        }
    }
    assert_eq!(
        total, 151,
        "expected 151 connections across the fixture set"
    );
}

/// **Methodology.** For every material stream in every fixture that carries an
/// overall composition, check that the mole fractions sum to 1 and that the
/// stored temperature and pressure are physical.
///
/// The tolerance on the sum is `1e-5`, not `1e-6`: the Three Phase Separator's
/// feed composition is entered in the file as three literal `0.333333`, whose
/// sum is `0.999999` — a property of the reference file, not of the importer.
/// Streams whose composition is all zeros (never specified) are skipped, and so
/// are the `T`/`P` checks on streams that carry neither.
/// **Result (2026-08-11):** 89 material streams checked; every composition sums
/// to 1 within 1e-5 and every stated `T`/`P` is strictly positive.
#[test]
fn material_stream_states_are_normalised_and_physical() {
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;

    let mut checked = 0usize;
    for e in EXPECTED {
        let imported = import_fixture(e.file);
        for object in imported.flowsheet.iter() {
            let Some(stream) = object.data.as_material() else {
                continue;
            };
            let x = stream.overall_composition();
            if !x.is_empty() && x.iter().any(|v| *v != 0.0) {
                let sum: f64 = x.iter().sum();
                assert!(
                    (sum - 1.0).abs() < 1e-5,
                    "{}: stream `{}` overall composition sums to {sum}",
                    e.file,
                    object.tag
                );
            }
            if let Some(t) = stream.temperature() {
                assert!(
                    t.get::<kelvin>() > 0.0,
                    "{}: stream `{}` has T = {} K",
                    e.file,
                    object.tag,
                    t.get::<kelvin>()
                );
            }
            if let Some(p) = stream.pressure() {
                assert!(
                    p.get::<pascal>() > 0.0,
                    "{}: stream `{}` has P = {} Pa",
                    e.file,
                    object.tag,
                    p.get::<pascal>()
                );
            }
            checked += 1;
        }
    }
    assert_eq!(
        checked, 89,
        "expected 89 material streams in the fixture set"
    );
}

/// **Methodology.** Read the four streams of `three_phase_separator.dwxmz` back
/// and compare with values transcribed by hand from the file's XML. The feed is
/// an equimolar methane/water/n-decane mixture flashed at 300 K, 101 325 Pa;
/// the three products are the vapour, oil and water phases.
///
/// Hand-transcribed reference values (mixture slot of each stream):
///
/// | Tag | T \[K\] | P \[Pa\] | mass flow \[kg/s\] | molar flow \[mol/s\] | h \[kJ/kg\] |
/// |---|---|---|---|---|---|
/// | `feed` | 300 | 101325 | 32 | 544.394140964461 | -528.233474326972 |
/// | `gas` | 300 | 101325 | 3.06040030894343 | 186.599226130543 | 3.0949143354263 |
/// | `oil` | 300 | 101325 | 25.7717190080739 | 181.957433515791 | -345.439794744781 |
/// | `water` | 300 | 101325 | 3.16771222439068 | 175.837481318127 | -2528.75418076306 |
///
/// The three product mass flows must also close the mass balance against the
/// feed: `3.06040030894343 + 25.7717190080739 + 3.16771222439068 =
/// 31.99983154141801` against a feed of `32` kg/s. That residual,
/// `-1.6846e-4 kg/s` (`-5.3` ppm of the feed), is **the reference file's own
/// flash convergence residual**, not an importer error — the importer performs
/// no arithmetic on these numbers. The tolerance below is set to `1e-3 kg/s` to
/// admit it while still catching a real transcription fault, which would be
/// order-unity.
/// **Result (2026-08-11):** every value matches to 1e-9 relative; the mass
/// balance residual is -1.6846e-4 kg/s as stated; `feed` is a
/// `Temperature_and_Pressure` specification and the three products are
/// `Pressure_and_Enthalpy`.
#[test]
fn three_phase_separator_reproduces_hand_read_values() {
    use outram_park_fork_dwsim_libs::flowsheet::streams::StreamSpec;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;

    let imported = import_fixture("three_phase_separator.dwxmz");

    // (tag, T [K], P [Pa], mass flow [kg/s], molar flow [mol/s], h [kJ/kg])
    const REFERENCE: [(&str, f64, f64, f64, f64, f64); 4] = [
        (
            "feed",
            300.0,
            101_325.0,
            32.0,
            544.394140964461,
            -528.233474326972,
        ),
        (
            "gas",
            300.0,
            101_325.0,
            3.06040030894343,
            186.599226130543,
            3.0949143354263,
        ),
        (
            "oil",
            300.0,
            101_325.0,
            25.7717190080739,
            181.957433515791,
            -345.439794744781,
        ),
        (
            "water",
            300.0,
            101_325.0,
            3.16771222439068,
            175.837481318127,
            -2528.75418076306,
        ),
    ];

    for (tag, t_k, p_pa, w, n, h_kj_per_kg) in REFERENCE {
        let stream = imported
            .flowsheet
            .object_by_tag(tag)
            .unwrap_or_else(|| panic!("missing stream `{tag}`"))
            .data
            .as_material()
            .unwrap_or_else(|| panic!("`{tag}` is not a material stream"));

        assert!(
            (stream.temperature().unwrap().get::<kelvin>() - t_k).abs() < 1e-9,
            "{tag}: T"
        );
        assert!(
            (stream.pressure().unwrap().get::<pascal>() - p_pa).abs() < 1e-6,
            "{tag}: P"
        );
        assert!(
            (stream.mass_flow().unwrap().get::<kilogram_per_second>() - w).abs() < 1e-12,
            "{tag}: mass flow"
        );
        // `MolarFlowRate` is uom's `CatalyticActivity`; its base unit is mol/s.
        assert!(
            (stream.molar_flow().unwrap().value - n).abs() < 1e-9,
            "{tag}: molar flow"
        );
        // The file stores kJ/kg; the uom accessor reports J/kg.
        assert!(
            (stream.mass_enthalpy().unwrap().get::<joule_per_kilogram>() - h_kj_per_kg * 1000.0)
                .abs()
                < 1e-6,
            "{tag}: mass enthalpy"
        );
        assert_eq!(
            stream.compound_names(),
            vec!["Methane", "Water", "N-decane"]
        );
    }

    let mass_of = |tag: &str| {
        imported
            .flowsheet
            .object_by_tag(tag)
            .unwrap()
            .data
            .as_material()
            .unwrap()
            .mass_flow()
            .unwrap()
            .get::<kilogram_per_second>()
    };
    let residual = mass_of("gas") + mass_of("oil") + mass_of("water") - mass_of("feed");
    assert!(
        residual.abs() < 1e-3,
        "the separator's stored mass balance must close to the file's own flash \
         tolerance; residual = {residual} kg/s"
    );
    assert!(
        (residual + 1.6845859198966195e-4).abs() < 1e-12,
        "the residual is a property of the reference file and must be reproduced \
         exactly; got {residual} kg/s"
    );

    let feed = imported
        .flowsheet
        .object_by_tag("feed")
        .unwrap()
        .data
        .as_material()
        .unwrap();
    assert_eq!(feed.spec, StreamSpec::TemperatureAndPressure);
    for tag in ["gas", "oil", "water"] {
        let s = imported
            .flowsheet
            .object_by_tag(tag)
            .unwrap()
            .data
            .as_material()
            .unwrap();
        assert_eq!(s.spec, StreamSpec::PressureAndEnthalpy, "{tag}: spec");
    }

    // Molar masses come from this file's own <Compounds> section, in kg/kmol.
    // Note they are the *file's* values, not a physical constant this crate
    // supplies: this fixture rounds water to 18.015 where other reference files
    // in the same corpus write 18.01528. The importer must reproduce whatever
    // the file says.
    assert!((imported.compound_molar_mass("Methane").unwrap() - 16.043).abs() < 1e-12);
    assert!((imported.compound_molar_mass("Water").unwrap() - 18.015).abs() < 1e-12);
    assert!((imported.compound_molar_mass("N-decane").unwrap() - 142.285).abs() < 1e-12);
    assert_eq!(imported.compound_molar_mass("Unobtainium"), None);
    // ...and they reach the stream, in the stream's own compound order.
    let masses: Vec<f64> = feed
        .phase(PhaseIndex::Mixture)
        .compounds
        .iter()
        .map(|c| c.molar_mass)
        .collect();
    assert_eq!(masses, vec![16.043, 18.015, 142.285]);
}

/// **Methodology.** `wind_turbine.dwxmz` is the smallest reference file: one
/// wind turbine exporting a duty to one energy stream. Check the values
/// transcribed by hand from its XML — the energy stream's
/// `EnergyFlow = 0.523975236841745 kW`, and the turbine's stored scalars
/// `DiskArea = 10`, `Efficiency = 80`, `NumberOfTurbines = 50`,
/// `AirDensity = 1.17454042760979`, `GeneratedPower = 0.523975236841745`,
/// which the importer keeps verbatim as the fixture's answer key.
///
/// The one gap must be the rejected turbine-to-energy-stream connection, and
/// the turbine's `power` must be `None` (DWSIM computes duty rather than
/// storing it on the object).
/// **Result (2026-08-11):** energy stream reads 523.975236841745 W; all five
/// scalars present verbatim; exactly one gap, of category
/// [`GapCategory::Connection`]; `power` is `None`.
#[test]
fn wind_turbine_reproduces_hand_read_values_and_pins_a_known_gap() {
    use outram_park_fork_dwsim_libs::flowsheet::objects::ObjectData;
    use uom::si::power::watt;

    let imported = import_fixture("wind_turbine.dwxmz");

    let duty = imported
        .flowsheet
        .object_by_tag("2")
        .unwrap()
        .data
        .as_energy()
        .unwrap();
    assert!(
        (duty.power().unwrap().get::<watt>() - 523.975236841745).abs() < 1e-9,
        "the file stores 0.523975236841745 kW; uom must report it in W"
    );

    let turbine_id = imported.flowsheet.id_by_tag("1").unwrap().clone();
    let props = imported.raw_properties(&turbine_id).unwrap();
    for (key, value) in [
        ("DiskArea", "10"),
        ("Efficiency", "80"),
        ("NumberOfTurbines", "50"),
        ("AirDensity", "1.17454042760979"),
        ("GeneratedPower", "0.523975236841745"),
    ] {
        assert_eq!(
            props.get(key).map(String::as_str),
            Some(value),
            "turbine scalar `{key}` must survive verbatim"
        );
    }
    match &imported.flowsheet.object(&turbine_id).unwrap().data {
        ObjectData::UnitOperation { power, .. } => assert_eq!(*power, None),
        other => panic!("the turbine should be a unit operation, got {other:?}"),
    }

    assert_eq!(imported.gaps.len(), 1);
    assert_eq!(imported.gaps[0].category(), GapCategory::Connection);
    assert!(
        matches!(&imported.gaps[0], ImportGap::ConnectionRejected { from_tag, to_tag, .. }
                 if from_tag == "1" && to_tag == "2"),
        "expected the turbine -> energy-stream edge to be the recorded gap, got {:?}",
        imported.gaps[0]
    );
}

/// **Methodology.** `wind_turbine_extracted.dwxml` is the byte-for-byte `.xml`
/// member of `wind_turbine.dwxmz`, extracted at fixture-collection time. The
/// two must import to the same flowsheet and the same gaps, differing only in
/// the recorded [`SourceKind`] — which is what proves the ZIP path adds nothing
/// but decompression.
/// **Result (2026-08-11):** identical summaries, identical gaps, identical
/// tag/type/connection sets; sources differ as
/// `ZippedXml { entry: "e0958db8-...-aa92ae1a6c43.xml" }` vs `PlainXml`.
#[test]
fn a_zipped_flowsheet_imports_identically_to_its_extracted_xml() {
    let zipped = import_fixture("wind_turbine.dwxmz");
    let plain = import_fixture("wind_turbine_extracted.dwxml");

    assert_eq!(plain.source, SourceKind::PlainXml);
    assert!(
        matches!(&zipped.source, SourceKind::ZippedXml { entry } if entry.ends_with(".xml")),
        "the zipped import must record which member it read"
    );

    assert_eq!(zipped.summary(), plain.summary());
    assert_eq!(zipped.gaps, plain.gaps);
    assert_eq!(zipped.compounds, plain.compounds);
    assert_eq!(zipped.property_packages, plain.property_packages);
    assert_eq!(zipped.general_info, plain.general_info);

    let describe = |f: &ImportedFlowsheet| {
        let mut objects: Vec<(String, String)> = f
            .flowsheet
            .iter()
            .map(|o| (o.tag.clone(), format!("{:?}", o.object_type)))
            .collect();
        objects.sort();
        let mut edges: Vec<(String, String)> = f
            .flowsheet
            .connections()
            .iter()
            .map(|c| (c.from.0.clone(), c.to.0.clone()))
            .collect();
        edges.sort();
        (objects, edges)
    };
    assert_eq!(describe(&zipped), describe(&plain));

    // The stream states must agree too, not just the topology.
    for object in zipped.flowsheet.iter() {
        let twin = plain.flowsheet.object_by_tag(&object.tag).unwrap();
        assert_eq!(object.data, twin.data, "state of `{}`", object.tag);
    }
}

/// **Methodology.** Three ways of handing the importer something it cannot
/// read: a fixture truncated to its first 200 bytes (well past the XML
/// declaration, so the reader is mid-document when the input ends), a ZIP
/// container truncated the same way, and a well-formed XML document whose root
/// element is not `DWSIM_Simulation_Data`. Each must return `Err` and must not
/// panic.
/// **Result (2026-08-11):** [`ImportError::Xml`], [`ImportError::Archive`],
/// [`ImportError::NotADwsimDocument`] respectively.
#[test]
fn malformed_input_returns_an_error_rather_than_panicking() {
    let plain = std::fs::read(fixture_dir().join("wind_turbine_extracted.dwxml")).unwrap();
    let truncated = &plain[..200];
    assert!(
        matches!(
            import_flowsheet_bytes(truncated, "truncated.dwxml"),
            Err(ImportError::Xml(_))
        ),
        "a truncated document must be an XML error"
    );

    let zipped = std::fs::read(fixture_dir().join("wind_turbine.dwxmz")).unwrap();
    assert!(
        matches!(
            import_flowsheet_bytes(&zipped[..200], "truncated.dwxmz"),
            Err(ImportError::Archive(_))
        ),
        "a truncated archive must be an archive error"
    );

    assert!(matches!(
        import_flowsheet_dwxml("<?xml version=\"1.0\"?><NotDwsim><a /></NotDwsim>"),
        Err(ImportError::NotADwsimDocument { root }) if root == "NotDwsim"
    ));

    assert!(matches!(
        import_flowsheet_file(Path::new("/nonexistent/definitely-not-here.dwxml")),
        Err(ImportError::Io { .. })
    ));
}

/// **Methodology.** Take a real fixture and rewrite the type of both its
/// heaters to name a unit operation that does not exist, in two stages, to
/// exercise both halves of the type-resolution rule:
///
/// 1. Rewrite only `<SimulationObject><Type>`. The importer must **still**
///    import the heaters, by falling back to the graphic object's
///    `<ObjectType>Heater</ObjectType>` — that fallback is what lets the
///    importer read pre-5.x files whose class namespaces have since been
///    renamed, so it is a feature to be pinned, not a leak.
/// 2. Rewrite the graphic `<ObjectType>` as well, leaving nothing that
///    resolves. Now both heaters must be dropped with a precise
///    [`ImportGap::UnsupportedObjectType`] naming each, the rest of the
///    flowsheet must still import, and every edge that pointed at a dropped
///    object must be reported as an [`ImportGap::DanglingConnection`] rather
///    than silently lost.
///
/// **Result (2026-08-11):** stage 1 imports all 16 objects with 0
/// `UnsupportedObjectType` gaps; stage 2 gives 14 objects and 6 connections
/// (down from 16 and 12), with 2 `UnsupportedObjectType` gaps and 4
/// `DanglingConnection` gaps, each naming one of the two dropped heaters.
#[test]
fn an_unknown_object_type_becomes_a_precise_gap() {
    // Read the fixture's XML out of its archive, then patch it as text.
    let bytes = std::fs::read(fixture_dir().join("heating_and_cooling.dwxmz")).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let index = (0..zip.len())
        .find(|i| zip.by_index(*i).unwrap().name().ends_with(".xml"))
        .unwrap();
    let mut text = String::new();
    {
        use std::io::Read;
        zip.by_index(index)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
    }

    // Stage 1 -- the class name alone is unknown; the graphic object still says
    // `Heater`, so nothing is lost.
    let class_only = text.replace(
        "<Type>DWSIM.UnitOperations.UnitOperations.Heater</Type>",
        "<Type>DWSIM.UnitOperations.UnitOperations.Teleporter</Type>",
    );
    assert_ne!(class_only, text, "the patch must actually apply");
    let fallback = import_flowsheet_dwxml(&class_only).unwrap();
    assert_eq!(
        fallback.summary().objects,
        16,
        "the graphic object's ObjectType must rescue an unknown class name"
    );
    assert_eq!(
        fallback
            .gap_counts()
            .get(&GapCategory::UnsupportedObjectType),
        None
    );

    // Stage 2 -- nothing resolves any more.
    let patched = class_only.replace(
        "<ObjectType>Heater</ObjectType>",
        "<ObjectType>Teleporter</ObjectType>",
    );
    assert_ne!(patched, class_only, "the second patch must actually apply");

    let imported = import_flowsheet_dwxml(&patched).unwrap();
    assert_eq!(
        imported.summary().objects,
        14,
        "both heaters must be dropped"
    );

    let unsupported: Vec<&ImportGap> = imported
        .gaps
        .iter()
        .filter(|g| g.category() == GapCategory::UnsupportedObjectType)
        .collect();
    assert_eq!(unsupported.len(), 2);
    let mut dropped: Vec<String> = Vec::new();
    for gap in unsupported {
        match gap {
            ImportGap::UnsupportedObjectType {
                dwsim_type,
                object_name,
            } => {
                assert_eq!(dwsim_type, "DWSIM.UnitOperations.UnitOperations.Teleporter");
                dropped.push(object_name.clone());
            }
            other => panic!("unexpected gap {other:?}"),
        }
    }

    let dangling: Vec<&ImportGap> = imported
        .gaps
        .iter()
        .filter(|g| matches!(g, ImportGap::DanglingConnection { .. }))
        .collect();
    assert_eq!(
        dangling.len(),
        4,
        "each dropped heater had a material and an energy inlet"
    );
    for gap in dangling {
        match gap {
            ImportGap::DanglingConnection { to_object, .. } => {
                assert!(
                    dropped.contains(to_object),
                    "dangling edge names `{to_object}`, not one of the dropped heaters {dropped:?}"
                );
            }
            other => panic!("unexpected gap {other:?}"),
        }
    }
    assert_eq!(
        imported.summary().connections,
        6,
        "the two dropped heaters take 6 of the 12 edges with them"
    );
}

/// **Methodology.** Property packages the crate does not implement must be
/// recorded as gaps without stopping the import, and the material streams must
/// still carry their reference to the package. `pump_and_valve.dwxmz` uses
/// Lee-Kesler-Plocker, which this crate has no
/// `PropertyPackageModel` variant for.
/// **Result (2026-08-11):** exactly one
/// [`ImportGap::UnsupportedPropertyPackage`]; the package's `model` is `None`;
/// all 16 material streams still reference `PP-ce6f5231-...` and all 28 objects
/// import.
#[test]
fn an_unsupported_property_package_is_a_gap_not_a_failure() {
    let imported = import_fixture("pump_and_valve.dwxmz");
    assert_eq!(imported.summary().objects, 28);
    assert_eq!(
        imported
            .gap_counts()
            .get(&GapCategory::UnsupportedPropertyPackage),
        Some(&1)
    );
    let package = &imported.property_packages[0];
    assert!(package.dwsim_type.ends_with("LKPPropertyPackage"));
    assert_eq!(package.model, None);

    let referencing = imported
        .flowsheet
        .iter()
        .filter(|o| o.object_type.is_material_stream())
        .filter(|o| imported.stream_property_package(&o.id) == Some(package.id.as_str()))
        .count();
    assert_eq!(
        referencing, 16,
        "every material stream still names the package it was solved with"
    );
}

/// **Methodology.** Cavett's Problem is the classic recycle-convergence
/// benchmark and the largest committed fixture (3.7 MB of XML). Check the
/// pieces that make it a *benchmark*: three recycle blocks, three compressors,
/// four flash vessels, two mixers, three valves, twenty material streams, and
/// that each recycle block sits on a real cycle (it has both an inbound and an
/// outbound edge).
/// **Result (2026-08-11):** 42 objects, 44 connections, 15 compounds, no gaps;
/// all three recycle blocks are wired on both sides.
#[test]
fn cavetts_problem_carries_its_recycle_structure() {
    use outram_park_fork_dwsim_libs::flowsheet::objects::ObjectType;

    let imported = import_fixture("cavetts_problem.dwxmz");
    assert!(imported.gaps.is_empty(), "gaps: {:?}", imported.gaps);

    for (object_type, count) in [
        (ObjectType::OtRecycle, 3),
        (ObjectType::Compressor, 3),
        (ObjectType::Vessel, 4),
        (ObjectType::NodeIn, 2),
        (ObjectType::Valve, 3),
        (ObjectType::MaterialStream, 20),
        (ObjectType::EnergyStream, 7),
    ] {
        assert_eq!(
            imported.flowsheet.ids_of_type(object_type).len(),
            count,
            "count of {object_type:?}"
        );
    }

    for id in imported.flowsheet.ids_of_type(ObjectType::OtRecycle) {
        let block = imported.flowsheet.object(&id).unwrap();
        assert!(
            block.inputs.iter().any(|c| c.is_attached()),
            "recycle `{}` has no inbound edge",
            block.tag
        );
        assert!(
            block.outputs.iter().any(|c| c.is_attached()),
            "recycle `{}` has no outbound edge",
            block.tag
        );
    }

    assert_eq!(
        imported
            .property_package(
                imported
                    .stream_property_package(imported.flowsheet.id_by_tag("1").unwrap())
                    .unwrap()
            )
            .unwrap()
            .dwsim_type,
        "DWSIM.Thermodynamics.PropertyPackages.PengRobinsonPropertyPackage"
    );
}

// ---------------------------------------------------------------------------
// Corpus census (ignored by default -- needs the upstream clone)
// ---------------------------------------------------------------------------

/// **Methodology.** Run the importer over *every* `.dwxml` / `.dwxmz` in a
/// directory tree and print a coverage census: how many parse, the aggregate
/// gap counts by category, and the most frequent unsupported object types and
/// property packages. This is roadmap data, not a pass/fail gate, so it is
/// `#[ignore]`d and reads its input directory from the `DWSIM_REFERENCE_DIR`
/// environment variable — the reference corpus is not redistributed here beyond
/// the eleven committed fixtures.
///
/// Run it with:
///
/// ```text
/// DWSIM_REFERENCE_DIR=/path/to/dwsim/PlatformFiles/Common \
///   cargo test --release -p outram-park-fork-dwsim-libs \
///   --test flowsheet_import -- --ignored --nocapture corpus_census
/// ```
///
/// The only assertion is that at least one file was found, so a mistyped path
/// fails loudly instead of reporting a vacuously perfect census.
///
/// # Result (2026-08-11, release build)
///
/// Run separately over `samples/` (43 files) and `tests/` (132 files) of
/// upstream commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`, i.e. the whole
/// 175-file reference corpus:
///
/// | Metric | samples | tests | total |
/// |---|---:|---:|---:|
/// | files found | 43 | 132 | **175** |
/// | parsed | 43 | 132 | **175 (100%)** |
/// | hard failures | 0 | 0 | **0** |
/// | objects imported | 644 | 4227 | 4871 |
/// | connections imported | 569 | 4038 | 4607 |
///
/// Gaps, aggregated:
///
/// | Category | Count | What it means |
/// |---|---:|---|
/// | unsupported object type | **0** | every distinct `<Type>` in the corpus maps |
/// | unsupported property package | 171 | this crate implements 3 of DWSIM's ~20 packages |
/// | object problem | 62 | all tag collisions (two objects sharing a `<Tag>`); topology unaffected |
/// | connection | 319 | see the breakdown below |
///
/// Connection gaps break down as: 197 "no free outlet slot", 70 "no free
/// material inlet slot", 29 slot fallbacks, 17 "no free energy inlet slot", 6
/// "source type is not in `ENERGY_SOURCE_TYPES`". Almost all of these are one
/// root cause — `ConnectorLayout::default_for` gives the plug-in wrappers
/// (`CapeOpenUO`, `CustomUO`, `ExcelUO`, `FlowsheetUO`, `External`) and the
/// clean-power sources a documented port-side default of one inlet and one
/// outlet, because their upstream shape classes were never transcribed. Real
/// DWSIM files wire several streams into those objects.
///
/// Top unsupported property packages: NRTL 45, UNIFAC 27, UNIQUAC 26,
/// Chao-Seader 11, MODFAC 9, NIST-MFAC 8, Grayson-Streed 7, PRSV2 6,
/// Peng-Robinson/Lee-Kesler 6, Steam Tables 6, UNIFAC-LL 5.
#[test]
#[ignore = "requires a DWSIM checkout; set DWSIM_REFERENCE_DIR"]
fn corpus_census() {
    let Ok(dir) = std::env::var("DWSIM_REFERENCE_DIR") else {
        panic!("set DWSIM_REFERENCE_DIR to a directory of DWSIM reference flowsheets");
    };
    let mut files = Vec::new();
    collect_flowsheet_files(Path::new(&dir), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .dwxml/.dwxmz files under {dir}");

    let mut parsed = 0usize;
    let mut failed: Vec<(String, String)> = Vec::new();
    let mut objects = 0usize;
    let mut connections = 0usize;
    let mut categories: BTreeMap<GapCategory, usize> = BTreeMap::new();
    let mut missing_types: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing_packages: BTreeMap<String, usize> = BTreeMap::new();
    let mut connection_reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut object_problems: BTreeMap<String, usize> = BTreeMap::new();

    for path in &files {
        match import_flowsheet_file(path) {
            Ok(imported) => {
                parsed += 1;
                let s = imported.summary();
                objects += s.objects;
                connections += s.connections;
                for (category, n) in imported.gap_counts() {
                    *categories.entry(category).or_insert(0) += n;
                }
                for gap in &imported.gaps {
                    match gap {
                        ImportGap::UnsupportedObjectType { dwsim_type, .. } => {
                            *missing_types.entry(dwsim_type.clone()).or_insert(0) += 1;
                        }
                        ImportGap::UnsupportedPropertyPackage { dwsim_type, .. } => {
                            *missing_packages.entry(dwsim_type.clone()).or_insert(0) += 1;
                        }
                        ImportGap::ConnectionRejected { reason, .. } => {
                            *connection_reasons
                                .entry(format!("rejected -- {}", classify(reason)))
                                .or_insert(0) += 1;
                        }
                        ImportGap::ConnectionSlotFallback { reason, .. } => {
                            *connection_reasons
                                .entry(format!("slot fallback -- {}", classify(reason)))
                                .or_insert(0) += 1;
                        }
                        ImportGap::DanglingConnection { .. } => {
                            *connection_reasons
                                .entry("dangling -- endpoint not imported".to_string())
                                .or_insert(0) += 1;
                        }
                        ImportGap::TagRenamed { .. } => {
                            *object_problems
                                .entry("tag collision -- object relabelled".to_string())
                                .or_insert(0) += 1;
                        }
                        ImportGap::ObjectRejected { reason, .. } => {
                            *object_problems
                                .entry(format!("rejected -- {reason}"))
                                .or_insert(0) += 1;
                        }
                        ImportGap::UnidentifiedObject { .. } => {
                            *object_problems
                                .entry("no identity element".to_string())
                                .or_insert(0) += 1;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => failed.push((path.display().to_string(), e.to_string())),
        }
    }

    println!("\n=== DWSIM reference corpus census ===");
    println!("files found   : {}", files.len());
    println!("parsed        : {parsed}");
    println!("failed        : {}", failed.len());
    println!("objects       : {objects}");
    println!("connections   : {connections}");
    println!("\ngaps by category:");
    for (category, n) in &categories {
        println!("  {n:6}  {category}");
    }
    println!("\nunsupported object types:");
    let mut types: Vec<_> = missing_types.iter().collect();
    types.sort_by_key(|(name, n)| (std::cmp::Reverse(**n), (*name).clone()));
    if types.is_empty() {
        println!("  (none -- every object type in the corpus maps)");
    }
    for (name, n) in types {
        println!("  {n:6}  {name}");
    }
    println!("\nunsupported property packages:");
    let mut packages: Vec<_> = missing_packages.iter().collect();
    packages.sort_by_key(|(name, n)| (std::cmp::Reverse(**n), (*name).clone()));
    for (name, n) in packages {
        println!("  {n:6}  {name}");
    }
    println!("\nobject problems, by cause:");
    let mut problems: Vec<_> = object_problems.iter().collect();
    problems.sort_by_key(|(name, n)| (std::cmp::Reverse(**n), (*name).clone()));
    if problems.is_empty() {
        println!("  (none)");
    }
    for (name, n) in problems {
        println!("  {n:6}  {name}");
    }
    println!("\nconnection gaps, by cause:");
    let mut reasons: Vec<_> = connection_reasons.iter().collect();
    reasons.sort_by_key(|(name, n)| (std::cmp::Reverse(**n), (*name).clone()));
    for (name, n) in reasons {
        println!("  {n:6}  {name}");
    }
    for (path, message) in &failed {
        println!("FAILED {path}: {message}");
    }
}

/// Collapse a connection-error message into a bucket, so the census reports
/// *kinds* of failure rather than one line per object GUID.
fn classify(reason: &str) -> &'static str {
    if reason.contains("cannot supply an energy stream") {
        "source type is not in ENERGY_SOURCE_TYPES"
    } else if reason.contains("no free `Energy` inlet slot") {
        "no free energy inlet slot in this crate's connector layout"
    } else if reason.contains("no free `In` inlet slot") {
        "no free material inlet slot in this crate's connector layout"
    } else if reason.contains("no free outlet slot") {
        "no free outlet slot in this crate's connector layout"
    } else if reason.contains("has no Input(") || reason.contains("has no Output(") {
        "requested slot index beyond this crate's connector layout"
    } else if reason.contains("does not join a stream") || reason.contains("cannot be connected") {
        "pairing rejected by the stream/unit-operation alternation rule"
    } else {
        "other"
    }
}

/// Recursively collect every `.dwxml` / `.dwxmz` under `dir`.
///
/// Unreadable directories are skipped silently: the census is a survey, and one
/// permission error should not abort it.
fn collect_flowsheet_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_flowsheet_files(&path, out);
        } else {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.ends_with(".dwxml") || name.ends_with(".dwxmz") {
                out.push(path);
            }
        }
    }
}
