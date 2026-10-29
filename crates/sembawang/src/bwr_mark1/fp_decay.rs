//! Fission-product decay heat by element, for decay heat that follows the
//! fission products out of the core (gh:#1041, plan item F10).
//!
//! MELCOR's DCH package gives each radionuclide class its own decay heat,
//! and the package holding a class's mass gets its heat. It weights ORIGEN
//! elemental decay heats into the classes (MELCOR 2.2 Vol. 2, DCH §2,
//! Table 2.1). Daughters are "assumed to remain with the parents", so an
//! element's heat includes its descendants'. MELCOR's elemental tables come
//! from SAND reports that are not held, so they are **computed here**: a
//! summation calculation on ENDF/B-VIII data already in the workspace.
//!
//! - **Decay data**: half-lives, mean decay energies and branches from
//!   `openmc-endf-8-depletion-lib-a`/`-b` (OpenMC's ENDF/B-VIII chain), for
//!   the elements Ti to Er.
//! - **Yields**: ENDF/B-VIII independent fission yields
//!   (`fission-yields-data`): U-235 and Pu-239 thermal (0.0253 eV), U-238
//!   fast (500 keV).
//! - **History**: constant power for `operating_time_s` at the fission
//!   shares of `decay.rs` (MELCOR's BWR defaults), then decay.
//! - **Method**: every nuclide's atoms obey `dN/dt = S + sum_p lambda_p b_p
//!   N_p - lambda N`. The decay graph is acyclic, so one pass in topological
//!   order per time step suffices. Each step uses the exponential
//!   integrator, exact for a constant feed, with the parents' feed averaged
//!   over the step. **Attribution**: each nuclide born in fission is credited
//!   to its own element, and so are all its descendants, which is MELCOR's
//!   daughters-stay-with-parents rule.
//!
//! Not modelled (stated): neutron capture on fission products during
//! irradiation (the ANS standard's `G` factor covers it for the total), and
//! delayed-neutron emission is followed only as far as the decay library's
//! branches give it.
//!
//! The table is costly to compute (about 0.2 s), so it is committed:
//! `verification_and_validation/bwr_mark1/fp_decay_by_element.csv`, with
//! one row per time and each element's share of the fission-product decay
//! heat. A test regenerates it and compares it byte for byte; re-bless with
//! `FUKUSHIMA_BLESS=1`.

use super::params::Params;
use petir::mathf::RealMath;
use std::collections::HashMap;

/// The fission-product elements, by atomic number (Ti to Er).
const ELEMENTS: [(u32, &str); 47] = [
    (22, "Ti"), (23, "V"), (24, "Cr"), (25, "Mn"), (26, "Fe"), (27, "Co"), (28, "Ni"), (29, "Cu"), (30, "Zn"), (31, "Ga"),
    (32, "Ge"), (33, "As"), (34, "Se"), (35, "Br"), (36, "Kr"), (37, "Rb"), (38, "Sr"), (39, "Y"), (40, "Zr"), (41, "Nb"),
    (42, "Mo"), (43, "Tc"), (44, "Ru"), (45, "Rh"), (46, "Pd"), (47, "Ag"), (48, "Cd"), (49, "In"), (50, "Sn"), (51, "Sb"),
    (52, "Te"), (53, "I"), (54, "Xe"), (55, "Cs"), (56, "Ba"), (57, "La"), (58, "Ce"), (59, "Pr"), (60, "Nd"), (61, "Pm"),
    (62, "Sm"), (63, "Eu"), (64, "Gd"), (65, "Tb"), (66, "Dy"), (67, "Ho"), (68, "Er"),
];

/// One nuclide of the decay graph.
struct Nuc {
    name: String,
    element: usize,
    /// Mass number.
    a: u32,
    lambda: f64,
    energy_j: f64,
    /// Decay branches to nuclides in the set: (index, branching ratio).
    branches: Vec<(usize, f64)>,
}

/// Read one element's records into `(name, half-life s, decay energy eV,
/// branches)`.
type Raw = (String, Option<f64>, Option<f64>, Vec<(String, f64)>);

macro_rules! read_lib {
    ($lib:ident, $m:ident, $f:ident) => {{
        $lib::decay_xml_info_serde::$m::$f()
            .nuclides
            .into_iter()
            .map(|n| {
                let br = n.raw_decay_data.iter().filter_map(|d| d.target.clone().map(|t| (t, d.branching_ratio))).collect();
                (n.name, n.half_life_seconds, n.decay_energy_electronvolt, br)
            })
            .collect::<Vec<Raw>>()
    }};
}

fn raw_data() -> Vec<Raw> {
    use openmc_endf_8_depletion_lib_a as a;
    use openmc_endf_8_depletion_lib_b as b;
    let mut v = Vec::new();
    v.extend(read_lib!(a, titanium, get_titanium_xml_serde_data));
    v.extend(read_lib!(a, vanadium, get_vanadium_xml_serde_data));
    v.extend(read_lib!(a, chromium, get_chromium_xml_serde_data));
    v.extend(read_lib!(a, manganese, get_manganese_xml_serde_data));
    v.extend(read_lib!(a, iron, get_iron_xml_serde_data));
    v.extend(read_lib!(a, cobalt, get_cobalt_xml_serde_data));
    v.extend(read_lib!(a, nickel, get_nickel_xml_serde_data));
    v.extend(read_lib!(a, copper, get_copper_xml_serde_data));
    v.extend(read_lib!(a, zinc, get_zinc_xml_serde_data));
    v.extend(read_lib!(a, gallium, get_gallium_xml_serde_data));
    v.extend(read_lib!(a, germanium, get_germanium_xml_serde_data));
    v.extend(read_lib!(a, arsenic, get_arsenic_xml_serde_data));
    v.extend(read_lib!(a, selenium, get_selenium_xml_serde_data));
    v.extend(read_lib!(a, bromine, get_bromine_xml_serde_data));
    v.extend(read_lib!(a, krypton, get_krypton_xml_serde_data));
    v.extend(read_lib!(a, rubidium, get_rubidium_xml_serde_data));
    v.extend(read_lib!(a, strontium, get_strontium_xml_serde_data));
    v.extend(read_lib!(a, yttrium, get_yttrium_xml_serde_data));
    v.extend(read_lib!(a, zirconium, get_zirconium_xml_serde_data));
    v.extend(read_lib!(a, niobium, get_niobium_xml_serde_data));
    v.extend(read_lib!(a, molybdenum, get_molybdenum_xml_serde_data));
    v.extend(read_lib!(a, technetium, get_technetium_xml_serde_data));
    v.extend(read_lib!(a, ruthenium, get_ruthenium_xml_serde_data));
    v.extend(read_lib!(a, rhodium, get_rhodium_xml_serde_data));
    v.extend(read_lib!(a, palladium, get_palladium_xml_serde_data));
    v.extend(read_lib!(a, silver, get_silver_xml_serde_data));
    v.extend(read_lib!(b, cadmium, get_cadmium_xml_serde_data));
    v.extend(read_lib!(b, indium, get_indium_xml_serde_data));
    v.extend(read_lib!(b, tin, get_tin_xml_serde_data));
    v.extend(read_lib!(b, antimony, get_antimony_xml_serde_data));
    v.extend(read_lib!(b, tellurium, get_tellurium_xml_serde_data));
    v.extend(read_lib!(b, iodine, get_iodine_xml_serde_data));
    v.extend(read_lib!(b, xenon, get_xenon_xml_serde_data));
    v.extend(read_lib!(b, cesium, get_cesium_xml_serde_data));
    v.extend(read_lib!(b, barium, get_barium_xml_serde_data));
    v.extend(read_lib!(b, lanthanum, get_lanthanum_xml_serde_data));
    v.extend(read_lib!(b, cerium, get_cerium_xml_serde_data));
    v.extend(read_lib!(b, praseodymium, get_praseodymium_xml_serde_data));
    v.extend(read_lib!(b, neodymium, get_neodymium_xml_serde_data));
    v.extend(read_lib!(b, promethium, get_promethium_xml_serde_data));
    v.extend(read_lib!(b, samarium, get_samarium_xml_serde_data));
    v.extend(read_lib!(b, europium, get_europium_xml_serde_data));
    v.extend(read_lib!(b, gadolinium, get_gadolinium_xml_serde_data));
    v.extend(read_lib!(b, terbium, get_terbium_xml_serde_data));
    v.extend(read_lib!(b, dysprosium, get_dysprosium_xml_serde_data));
    v.extend(read_lib!(b, holmium, get_holmium_xml_serde_data));
    v.extend(read_lib!(b, erbium, get_erbium_xml_serde_data));
    v
}

/// The element symbol of a nuclide name (`Xe135_m1` -> `Xe`).
fn symbol(name: &str) -> &str {
    let end = name.find(|c: char| c.is_ascii_digit()).unwrap_or(name.len());
    &name[..end]
}

/// The yield library's spelling of a decay-library name: first isomers
/// `_m1` are `m` there, second isomers `_m2` are `n`.
fn yield_name(name: &str) -> String {
    name.replace("_m1", "m").replace("_m2", "n")
}

/// The decay graph in topological order (parents before daughters).
fn graph() -> Vec<Nuc> {
    let z_of: HashMap<&str, usize> = ELEMENTS.iter().enumerate().map(|(i, (_, s))| (*s, i)).collect();
    let raw: Vec<Raw> = raw_data().into_iter().filter(|r| z_of.contains_key(symbol(&r.0))).collect();
    let idx: HashMap<String, usize> = raw.iter().enumerate().map(|(i, r)| (r.0.clone(), i)).collect();
    let n = raw.len();
    // Kahn's algorithm on parent -> daughter edges.
    let mut indeg = vec![0usize; n];
    for r in &raw {
        for (t, _) in &r.3 {
            if let Some(&j) = idx.get(t) {
                indeg[j] += 1;
            }
        }
    }
    let mut order: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut k = 0;
    while k < order.len() {
        let i = order[k];
        for (t, _) in &raw[i].3 {
            if let Some(&j) = idx.get(t) {
                indeg[j] -= 1;
                if indeg[j] == 0 {
                    order.push(j);
                }
            }
        }
        k += 1;
    }
    assert_eq!(order.len(), n, "the decay graph has a cycle");
    let pos: Vec<usize> = {
        let mut p = vec![0; n];
        for (k, &i) in order.iter().enumerate() {
            p[i] = k;
        }
        p
    };
    order
        .iter()
        .map(|&i| {
            let r = &raw[i];
            let lambda = r.1.filter(|&h| h > 0.0).map(|h| std::f64::consts::LN_2 / h).unwrap_or(0.0);
            Nuc {
                name: r.0.clone(),
                element: z_of[symbol(&r.0)],
                a: r.0[symbol(&r.0).len()..].split('_').next().and_then(|x| x.parse().ok()).unwrap_or(0),
                lambda,
                energy_j: r.2.unwrap_or(0.0) * 1.602176634e-19,
                branches: r.3.iter().filter_map(|(t, b)| idx.get(t).map(|&j| (pos[j], *b))).collect(),
            }
        })
        .collect()
}

/// The fission rate of each parent at full power, 1/s, and its yields for
/// every nuclide of `nucs`.
fn sources(prm: &Params, nucs: &[Nuc]) -> Vec<f64> {
    use fission_yields_data::prelude::{fission_yield_linear_interpolation, Nuclide};
    use std::str::FromStr;
    use uom::si::energy::{electronvolt, kiloelectronvolt};
    use uom::si::f64::Energy;
    use uom::si::ratio::ratio;
    let mev = 1.602176634e-13;
    let p0 = prm.thermal_power_w;
    let parents = [
        (Nuclide::U235, Energy::new::<electronvolt>(0.0253), prm.fission_share_u235, prm.q_u235_mev),
        (Nuclide::Pu239, Energy::new::<electronvolt>(0.0253), prm.fission_share_pu239, prm.q_pu239_mev),
        (Nuclide::U238, Energy::new::<kiloelectronvolt>(500.0), 1.0 - prm.fission_share_u235 - prm.fission_share_pu239, prm.q_u238_mev),
    ];
    nucs.iter()
        .map(|nu| match Nuclide::from_str(&yield_name(&nu.name)) {
            Ok(p) => parents.iter().map(|(f, e, share, q)| share * p0 / (q * mev) * fission_yield_linear_interpolation(*f, p, *e).get::<ratio>()).sum(),
            Err(_) => 0.0,
        })
        .collect()
}

/// The times of the committed table, s after the scram: 10 s to 600 h,
/// 61 points evenly spaced in the logarithm.
pub fn table_times() -> Vec<f64> {
    (0..61).map(|i| 10.0 * (2.16e6f64 / 10.0).r_powf(i as f64 / 60.0)).collect()
}

/// The summation's results: each element's fission-product decay heat, W,
/// at [`table_times`] (descendants credited to the element born in
/// fission), and each element's mass at the scram, kg (by the element the
/// atoms are then, stable isotopes included).
pub struct Summation {
    pub heat_w: Vec<Vec<f64>>,
    pub mass_kg: Vec<f64>,
    /// Each nuclide's mass at the scram, kg, by its ENDF name (`Cs137`).
    pub nuclide_kg: Vec<(String, f64)>,
}

impl Summation {
    /// One nuclide's mass at the scram, kg (0 if absent).
    pub fn nuclide_mass_kg(&self, name: &str) -> f64 {
        self.nuclide_kg.iter().find(|(n, _)| n == name).map_or(0.0, |x| x.1)
    }
    /// One element's mass at the scram, kg (0 if absent).
    pub fn element_mass_kg(&self, symbol: &str) -> f64 {
        ELEMENTS.iter().position(|e| e.1 == symbol).map_or(0.0, |i| self.mass_kg[i])
    }
}

/// The summation (see the module doc).
pub fn compute(prm: &Params) -> Summation {
    let nucs = graph();
    let s = sources(prm, &nucs);
    let ne = ELEMENTS.len();
    let n = nucs.len();
    // Atoms of nuclide i born (or descended) from element e: n x ne.
    let mut atoms = vec![vec![0.0; ne]; n];
    let step = |atoms: &mut Vec<Vec<f64>>, dt: f64, on: bool| {
        // Feed from parents into each daughter, averaged over the step.
        let mut feed_old = vec![vec![0.0; ne]; n];
        for (i, nu) in nucs.iter().enumerate() {
            for &(j, b) in &nu.branches {
                for e in 0..ne {
                    feed_old[j][e] += nu.lambda * b * atoms[i][e];
                }
            }
        }
        let mut feed_new = vec![vec![0.0; ne]; n];
        for (i, nu) in nucs.iter().enumerate() {
            let x = nu.lambda * dt;
            let decay = (-x).r_exp();
            let grow = if nu.lambda > 0.0 { (1.0 - decay) / nu.lambda } else { dt };
            for e in 0..ne {
                let src = if on && nu.element == e { s[i] } else { 0.0 } + 0.5 * (feed_old[i][e] + feed_new[i][e]);
                atoms[i][e] = atoms[i][e] * decay + src * grow;
            }
            for &(j, b) in &nu.branches {
                for e in 0..ne {
                    feed_new[j][e] += nu.lambda * b * atoms[i][e];
                }
            }
        }
    };
    // Irradiation: 600 geometric steps from 0.01 s to the operating time.
    let t_op = prm.operating_time_s;
    let k = 600;
    let r = (t_op / 0.01).r_powf(1.0 / k as f64);
    let mut t_prev = 0.0;
    for i in 1..=k {
        let t = if i == k { t_op } else { 0.01 * r.r_powf(i as f64) };
        step(&mut atoms, t - t_prev, true);
        t_prev = t;
    }
    let mut mass_kg = vec![0.0; ne];
    let mut nuclide_kg = Vec::with_capacity(n);
    for (i, nu) in nucs.iter().enumerate() {
        let kg = atoms[i].iter().sum::<f64>() * nu.a as f64 * 1.66053907e-27;
        mass_kg[nu.element] += kg;
        nuclide_kg.push((nu.name.clone(), kg));
    }
    // Decay: 10 sub-steps between table times, from the scram.
    let heat = |atoms: &Vec<Vec<f64>>| (0..ne).map(|e| nucs.iter().enumerate().map(|(i, nu)| nu.lambda * nu.energy_j * atoms[i][e]).sum()).collect::<Vec<f64>>();
    let mut out = Vec::new();
    let mut t = 0.0;
    for &tt in &table_times() {
        let mut first = if t == 0.0 { 1.0e-3 } else { t };
        if t == 0.0 {
            step(&mut atoms, first, false);
        }
        let sub = 20;
        let rr = (tt / first).r_powf(1.0 / sub as f64);
        for _ in 0..sub {
            let next = first * rr;
            step(&mut atoms, next - first, false);
            first = next;
        }
        t = tt;
        out.push(heat(&atoms));
    }
    Summation { heat_w: out, mass_kg, nuclide_kg }
}

/// The committed table as CSV text: a header of element symbols, then per
/// time its value and each element's share of the total.
pub fn table_csv(sum: &Summation) -> String {
    let heat = &sum.heat_w;
    let mut s = String::from("t_s,total_w");
    for (_, sym) in ELEMENTS {
        s.push(',');
        s.push_str(sym);
    }
    s.push('\n');
    for (t, h) in table_times().iter().zip(heat) {
        let total: f64 = h.iter().sum();
        s.push_str(&format!("{t:.6e},{total:.6e}"));
        for x in h {
            s.push_str(&format!(",{:.6e}", x / total));
        }
        s.push('\n');
    }
    s
}

/// The element masses at the scram as CSV text: symbol, kg.
pub fn inventory_csv(sum: &Summation) -> String {
    let mut s = String::from("element,kg\n");
    for ((_, sym), m) in ELEMENTS.iter().zip(&sum.mass_kg) {
        s.push_str(&format!("{sym},{m:.6e}\n"));
    }
    s
}

/// The committed inventory (`inventory_csv` at the default inputs).
const COMMITTED_INVENTORY: &str = include_str!("../../verification_and_validation/bwr_mark1/fp_inventory_by_element.csv");

/// An element's mass at the scram from the committed inventory, kg (0 if
/// absent).
pub fn inventory_kg(symbol: &str) -> f64 {
    COMMITTED_INVENTORY.lines().skip(1).find_map(|l| {
        let mut it = l.split(',');
        (it.next() == Some(symbol)).then(|| it.next().and_then(|x| x.parse().ok())).flatten()
    }).unwrap_or(0.0)
}

/// The committed table (`table_csv` at the default inputs).
const COMMITTED: &str = include_str!("../../verification_and_validation/bwr_mark1/fp_decay_by_element.csv");

/// The committed table, read: times, s, and each element's share of the
/// fission-product decay heat at each.
#[derive(Clone, Debug, PartialEq)]
pub struct ElementShares {
    pub times_s: Vec<f64>,
    pub symbols: Vec<String>,
    pub shares: Vec<Vec<f64>>,
}

impl ElementShares {
    pub fn committed() -> Self {
        let mut lines = COMMITTED.lines();
        let symbols: Vec<String> = lines.next().unwrap_or("").split(',').skip(2).map(String::from).collect();
        let (mut times_s, mut shares) = (Vec::new(), Vec::new());
        for l in lines.filter(|l| !l.is_empty()) {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap_or(0.0)).collect();
            times_s.push(v[0]);
            shares.push(v[2..].to_vec());
        }
        Self { times_s, symbols, shares }
    }

    /// The share of the fission-product decay heat carried by the elements
    /// `symbols` at `t_s` after the scram, interpolated linearly in the
    /// logarithm of time (held at the ends).
    pub fn share(&self, symbols: &[&str], t_s: f64) -> f64 {
        let cols: Vec<usize> = symbols.iter().filter_map(|s| self.symbols.iter().position(|x| x == s)).collect();
        let at = |k: usize| cols.iter().map(|&c| self.shares[k][c]).sum::<f64>();
        let n = self.times_s.len();
        if t_s <= self.times_s[0] {
            return at(0);
        }
        if t_s >= self.times_s[n - 1] {
            return at(n - 1);
        }
        let k = self.times_s.partition_point(|&x| x <= t_s) - 1;
        let w = (t_s / self.times_s[k]).r_ln() / (self.times_s[k + 1] / self.times_s[k]).r_ln();
        at(k) + w * (at(k + 1) - at(k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The graph holds every element, and its topological order puts
    /// parents first: I-135 before Xe-135, Te-132 before I-132.
    #[test]
    fn the_decay_graph_is_ordered() {
        let g = graph();
        let at = |name: &str| g.iter().position(|n| n.name == name).unwrap_or_else(|| panic!("{name}"));
        assert!(at("I135") < at("Xe135") && at("Te132") < at("I132"));
        for (i, nu) in g.iter().enumerate() {
            assert!(nu.branches.iter().all(|&(j, _)| j > i), "{}", nu.name);
        }
        assert!(g.len() > 1000, "{}", g.len());
    }

    /// Cesium-137's share of the cesium mass at the scram, from the
    /// summation (for the Cs-137 release against BSAF Fig 5.1, gh:#1040).
    /// Results (2026-10-29): Cs-137 30.09 kg of 95.01 kg, a share of
    /// 0.3167; Cs-133 31.82 kg, Cs-135 33.09 kg, Cs-134 0.0008 kg.
    /// Predicted 0.35-0.45: **missed**, low. With no neutron capture in
    /// the summation (stated in the module doc) Xe-135 is not burned out,
    /// so Cs-135 outweighs Cs-133, and Cs-134 (made by capture on Cs-133)
    /// is nearly absent; the share is biased low by that.
    #[test]
    fn the_cesium_137_share_is_the_summations() {
        let sum = compute(&Params::default());
        let (cs137, cs) = (sum.nuclide_mass_kg("Cs137"), sum.element_mass_kg("Cs"));
        assert!((cs137 - 30.085).abs() < 0.01 && (cs - 95.008).abs() < 0.01, "{cs137} {cs}");
        assert!((cs137 / cs - 0.316_659).abs() < 1e-5 && (sum.nuclide_mass_kg("Cs135") - 33.095).abs() < 0.01);
        assert_eq!(sum.nuclide_mass_kg("NotANuclide"), 0.0);
    }

    /// The committed table is what the summation computes (regenerated
    /// byte for byte; `FUKUSHIMA_BLESS=1` rewrites it), and its total is
    /// checked against the ANS-1979 fission-product curve of `decay.rs`
    /// (capture and actinides off), an independent evaluation of the same
    /// quantity.
    ///
    /// Results (2026-10-29): the summation is 0.963 of ANS at 15 s, 0.973
    /// at 95 s, 0.994 at 1 h, 0.992 at 6.6 h, 0.991 at 23 h and 0.984 at
    /// 20 days. Predicted: within 15 %. Held, within 4 % everywhere.
    /// The volatile classes (noble gases Xe/Kr, alkali Cs/Rb, halogens
    /// I/Br, chalcogens Te/Se) carry 0.50 at 15 s, 0.58 at 1 h, 0.53 at
    /// 3.6 h, 0.45 at 23 h. Predicted 25-40 %: MISSED. Crediting
    /// descendants to the element born in fission (MELCOR's rule) gives the
    /// gases their alkali daughters, e.g. Kr-88 to Rb-88 and Xe-138 to
    /// Cs-138, and Te its iodine.
    #[test]
    fn the_element_table_is_the_summation_and_matches_ans() {
        use super::super::decay::{DecayParts, PlantDecay};
        let prm = Params::default();
        let sum = compute(&prm);
        let csv = table_csv(&sum);
        let inv = inventory_csv(&sum);
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/verification_and_validation/bwr_mark1/");
        if std::env::var("FUKUSHIMA_BLESS").is_ok() {
            std::fs::write(format!("{dir}fp_decay_by_element.csv"), &csv).unwrap();
            std::fs::write(format!("{dir}fp_inventory_by_element.csv"), &inv).unwrap();
        }
        assert_eq!(csv, COMMITTED, "regenerate with FUKUSHIMA_BLESS=1");
        assert_eq!(inv, COMMITTED_INVENTORY, "regenerate with FUKUSHIMA_BLESS=1");
        eprintln!("INVENTORY Cs {} I {} Te {} Xe {} Rb {} Br {} Kr {}", inventory_kg("Cs"), inventory_kg("I"), inventory_kg("Te"), inventory_kg("Xe"), inventory_kg("Rb"), inventory_kg("Br"), inventory_kg("Kr"));
        let table = ElementShares::committed();
        let mut d = PlantDecay::new(&prm, DecayParts { mixed_finite: true, capture: false, actinides: false });
        let mut t = 0.0;
        for (k, line) in csv.lines().skip(1).enumerate() {
            let tt = table.times_s[k];
            d.advance(tt - t);
            t = tt;
            let total: f64 = line.split(',').nth(1).unwrap().parse().unwrap();
            let r = total / d.power_w();
            assert!((0.95..1.0).contains(&r), "{tt} {r}");
            let sum: f64 = table.shares[k].iter().sum();
            assert!((sum - 1.0).abs() < 1e-5, "{sum}");
        }
        let vol = ["Xe", "Kr", "Cs", "Rb", "I", "Br", "Te", "Se"];
        assert!((table.share(&vol, 3787.0) - 0.580).abs() < 2e-3, "{}", table.share(&vol, 3787.0));
        assert!((table.share(&vol, 8.165e4) - 0.453).abs() < 2e-3, "{}", table.share(&vol, 8.165e4));
        assert_eq!(table.share(&vol, 1.0), table.share(&vol, 10.0));
    }
}
