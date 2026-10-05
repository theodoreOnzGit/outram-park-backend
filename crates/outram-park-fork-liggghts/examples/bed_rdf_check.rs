//! g(r) diagnostics of a bed CSV (`id,x,y,z,...`, metres): the contact peak,
//! and the second shell (sqrt2 d trough vs FCC peak, sqrt3 d and 2 d peaks)
//! that separates a random packing from a crystalline one. Core region z > 0
//! only, as the publications package's g(r) figures use.
//!
//! cargo run --release -p outram-park-fork-liggghts --example bed_rdf_check -- FILE.csv [FILE2.csv ...]
use outram_park_fork_liggghts::compute::{ComputeType, ThreadCount};
use outram_park_fork_liggghts::particle::Vec3;
use outram_park_fork_liggghts::rdf::{radial_distribution, RdfDomain, RdfSettings};

fn main() {
    for f in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&f).expect("read");
        let c: Vec<Vec3> = text
            .lines()
            .skip(1)
            .filter_map(|l| {
                let v: Vec<f64> = l.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (v.len() >= 4 && v[3] > 0.0).then(|| Vec3::new(v[1], v[2], v[3]))
            })
            .collect();
        let d = 0.06;
        let rdf = radial_distribution(&c, RdfSettings::for_diameter(d, RdfDomain::from_positions(&c)), ComputeType::CpuMultiThread(ThreadCount::Fixed(8)));
        let at = |x: f64| {
            let i = rdf.r.iter().position(|r| *r >= x * d).unwrap_or(0);
            rdf.g[i.saturating_sub(1)..=(i + 1).min(rdf.g.len() - 1)].iter().sum::<f64>() / 3.0
        };
        let peak = rdf.g.iter().cloned().fold(0.0, f64::max);
        println!("{f}: n={} contact peak {peak:.2}  g(sqrt2 d) {:.3}  g(sqrt3 d) {:.3}  g(2 d) {:.3}", c.len(), at(2f64.sqrt()), at(3f64.sqrt()), at(2.0));
    }
}
