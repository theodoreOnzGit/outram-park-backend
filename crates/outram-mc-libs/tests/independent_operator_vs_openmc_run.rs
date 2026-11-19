#![cfg(feature = "std")]
//! **Transport-free depletion against OpenMC run, every integrator**
//! (gh:#1626 arm 1).
//!
//! Reference: OpenMC `e4febe9`'s `IndependentOperator` with each of its eight
//! integrators, on the inputs of its own regression test
//! `tests/regression_tests/deplete_no_transport`: UO2 at 4.25 % (O-18 dropped,
//! see the script), 10.4 g/cc, volume `pi*0.42^2` cm^3,
//! `tests/micro_xs_simple.csv`, `tests/chain_simple.xml`, flux
//! 1164719970082145.0 n/cm^2/s. Three 5-day steps, IPF CRAM-48, one substep,
//! SI methods at 10 iterations; normalisation `source-rate` (1.0) and
//! `fission-q` (174 W). Run 2026-11-17 by
//! `verification_and_validation/spectrum_depletion/openmc_inputs/
//! ref_independent_operator.py` with libopenmc built from `e4febe9`; output
//! `openmc_outputs/independent_operator.tsv` (atoms and per-atom rates of
//! every saved step).
//!
//! The port: OpenMC's initial atoms, the same chain and one-group cross
//! sections, an operator that forms the rates the way
//! `IndependentOperator._IndependentRateHelper.get_material_rates` and
//! `OpenMCOperator._calculate_reaction_rates` do (tally `sigma * (phi * V)`,
//! divided by `1e24 V`, times the helper's factor), this crate's
//! `ChainFissionHelper` and `SourceRateHelper` arithmetic (`depletion::
//! helpers`), the chain's `build_matrix`, and PETIR's integrators with
//! CRAM-48.
//!
//! **Prediction, written before the run.** No statistics are involved, so
//! every difference is code:
//! - beginning-of-step rates of step 0: identical, or a few ulp where numpy's
//!   `dot` sums the fission energy in another order (fission-q only);
//! - atoms, every integrator, every step: within 1e-10 relative on every
//!   entry above 1e-12 of the largest, the level the transfer-rate arm
//!   reached on the same chain (dense LU here, SuperLU there; summation
//!   order), with the bulk near 1e-14.
//!
//! **Results, measured 2026-11-17 (prediction held):**
//! - step-0 rates: **bit-identical** in both modes, all 16 runs (numpy's
//!   `dot` over 9 entries summed in the same order here);
//! - atoms: 480 entries compared, 69 bit-identical, worst **4.7e-15**
//!   relative (SI-CE/LI, source-rate, Xe-135 at step 3); per integrator
//!   1.9e-15 to 4.7e-15. Cause, understood: the CRAM-48 linear solves (dense
//!   elimination in PETIR, SuperLU in scipy) round differently; the
//!   operator side is exact. Below the workspace's 1e-9 code-to-code
//!   allowance, flagged here as the rule asks. The gates stay at the
//!   prediction (rates 1e-15, atoms 1e-10), not moved to the result.

use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;
use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::helpers::{chain_fission_energy, energy_normalization_factor, source_rate_factor};
use outram_mc_libs::depletion::integrators::{CramSolver, DepletionIntegrator, Integrator, Keff, OperatorResult, TransportOperator};
use outram_mc_libs::depletion::matrix::DepletionMatrix;
use outram_mc_libs::depletion::{MicroRate, ReactionRates};
use std::convert::Infallible;
use std::path::PathBuf;

const FLUX: f64 = 1164719970082145.0;

/// Normalisation: upstream's `SourceRateHelper` or `ChainFissionHelper`.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    SourceRate,
    FissionQ,
}

/// The transport-free operator: OpenMC's `IndependentOperator` on one
/// material.
struct Independent {
    chain: DepletionChain,
    names: Vec<String>,
    /// `(gamma, fission)` barns per chain nuclide, 0 where the CSV has none.
    xs: Vec<(f64, f64)>,
    /// Chain fission Q [eV] per chain nuclide (`ChainFissionHelper.prepare`).
    q: Vec<f64>,
    volume: f64,
    mode: Mode,
}

impl TransportOperator for Independent {
    type Rates = ReactionRates;
    type Error = Infallible;

    fn evaluate(&mut self, n: &[Vec<f64>], source_rate: f64) -> Result<OperatorResult<ReactionRates>, Infallible> {
        let k = Keff { value: 1.0, std_dev: 0.0 };
        if source_rate == 0.0 {
            return Ok(OperatorResult { k: Keff { value: 0.0, std_dev: 0.0 }, rates: ReactionRates::zero() });
        }
        let flux = FLUX * self.volume;
        let volume_b_cm = 1e24 * self.volume;
        let number = &n[0];
        // tally_rates = (xs * flux).sum() per [nuclide, reaction]
        let tally: Vec<(f64, f64)> = self.xs.iter().map(|(g, f)| (g * flux, f * flux)).collect();
        let factor = match self.mode {
            Mode::SourceRate => source_rate_factor(source_rate),
            Mode::FissionQ => {
                let fission_rates: Vec<f64> = tally.iter().zip(number).map(|((_, f), num)| f * (num / volume_b_cm)).collect();
                let energy = chain_fission_energy(0.0, &fission_rates, &self.q);
                energy_normalization_factor(source_rate, energy).unwrap()
            }
        };
        let mut rates = ReactionRates::zero();
        for (name, (g, f)) in self.names.iter().zip(&tally) {
            rates.set(name, MicroRate { gamma: g / volume_b_cm * factor, fission: f / volume_b_cm * factor, n2n: 0.0 });
        }
        Ok(OperatorResult { k, rates })
    }

    fn form_matrix(&self, rates: &ReactionRates, _material: usize) -> Result<DepletionMatrix, Infallible> {
        Ok(self.chain.build_matrix(rates))
    }
}

#[test]
fn every_integrator_matches_openmc_independent_operator() {
    let v = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/spectrum_depletion");
    let xml = DepletionChainXml::parse(&std::fs::read_to_string(v.join("openmc_inputs/chain_simple_tests.xml")).unwrap()).unwrap();
    let chain = DepletionChain::from_chain_xml(&xml, 0.0253).unwrap();
    let names: Vec<String> = chain.nuclide_names().iter().map(|s| (*s).to_owned()).collect();

    let csv = std::fs::read_to_string(v.join("openmc_inputs/micro_xs_simple.csv")).unwrap();
    let mut xs = vec![(0.0, 0.0); names.len()];
    for line in csv.lines().skip(1) {
        // nuclides,"reactions",groups,xs
        let (nuc, rest) = line.split_once(',').unwrap();
        let (rx, rest) = rest.rsplit_once(',').map(|(a, x)| (a, x)).unwrap();
        let value: f64 = rest.parse().unwrap();
        let rx = rx.rsplit_once(',').unwrap().0.trim_matches('"');
        if let Some(i) = names.iter().position(|n| n == nuc) {
            match rx {
                "(n,gamma)" => xs[i].0 = value,
                "fission" => xs[i].1 = value,
                other => panic!("unexpected reaction {other}"),
            }
        }
    }
    let q: Vec<f64> = names
        .iter()
        .map(|n| xml.nuclides.iter().find(|x| &x.name == n).and_then(|x| x.reactions.iter().find(|r| r.kind == "fission")).map_or(0.0, |r| r.q_ev))
        .collect();

    let text = std::fs::read_to_string(v.join("openmc_outputs/independent_operator.tsv")).unwrap();
    let mut volume = 0.0;
    let mut initial = vec![0.0; names.len()];
    let mut atoms: Vec<(String, String, usize, String, f64)> = Vec::new();
    let mut ref_rates: Vec<(String, String, usize, String, String, f64)> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["volume", x] => volume = x.parse().unwrap(),
            ["initial", nuc, x] => initial[names.iter().position(|n| n == nuc).unwrap()] = x.parse().unwrap(),
            ["rate", m, i, s, nuc, rx, x] => ref_rates.push(((*m).into(), (*i).into(), s.parse().unwrap(), (*nuc).into(), (*rx).into(), x.parse().unwrap())),
            [m, i, s, nuc, x] => atoms.push(((*m).into(), (*i).into(), s.parse().unwrap(), (*nuc).into(), x.parse().unwrap())),
            _ => panic!("bad line {line}"),
        }
    }

    let dt = vec![5.0 * 86_400.0; 3];
    let mut worst_rate = 0.0f64;
    let mut worst_atoms = 0.0f64;
    let mut bit_identical = 0usize;
    let mut compared = 0usize;
    for (mode, mode_name, source) in [(Mode::SourceRate, "source-rate", 1.0), (Mode::FissionQ, "fission-q", 174.0)] {
        for method in Integrator::ALL {
            let mut op = Independent { chain: chain.clone(), names: names.clone(), xs: xs.clone(), q: q.clone(), volume, mode };
            let mut integ = DepletionIntegrator::new(method, CramSolver::Cram48, dt.clone(), vec![source; 3]).unwrap();
            let records = integ.integrate(&mut op, vec![initial.clone()], false).unwrap();
            assert_eq!(records.len(), 4);
            let mut worst = 0.0f64;
            let mut at = String::new();
            for rec in &records {
                let big = rec.n[0].iter().cloned().fold(0.0, f64::max);
                for (j, nuc) in names.iter().enumerate() {
                    let r = atoms.iter().find(|a| a.0 == mode_name && a.1 == method.name() && a.2 == rec.index && &a.3 == nuc).unwrap().4;
                    let ours = rec.n[0][j];
                    if r.abs() < 1e-12 * big {
                        continue;
                    }
                    compared += 1;
                    if ours.to_bits() == r.to_bits() {
                        bit_identical += 1;
                    }
                    let rel = ((ours - r) / r).abs();
                    if rel > worst {
                        worst = rel;
                        at = format!("step {} {nuc}: ours {ours:e}, OpenMC {r:e}", rec.index);
                    }
                }
            }
            // Step-0 beginning-of-step rates.
            let mut wr = 0.0f64;
            for (nuc, rx, r) in ref_rates.iter().filter(|x| x.0 == mode_name && x.1 == method.name() && x.2 == 0).map(|x| (&x.3, &x.4, x.5)) {
                let m = records[0].rates.rate_for(nuc);
                let ours = if rx == "fission" { m.fission } else { m.gamma };
                if r != 0.0 {
                    wr = wr.max(((ours - r) / r).abs());
                } else {
                    assert_eq!(ours, 0.0);
                }
            }
            println!("{mode_name:>11} {:>8}: atoms worst {worst:.2e} ({at}); step-0 rates worst {wr:.2e}", method.name());
            worst_atoms = worst_atoms.max(worst);
            worst_rate = worst_rate.max(wr);
        }
    }
    println!("{compared} atom entries compared, {bit_identical} bit-identical; worst atoms {worst_atoms:.2e}, worst rate {worst_rate:.2e}");
    assert!(worst_rate <= 1e-15, "{worst_rate}");
    assert!(worst_atoms <= 1e-10, "{worst_atoms}");
}
