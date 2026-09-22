//! Temporary scan for the #266 convergence study: does Xe-135 enter the
//! asymptotic regime once the step resolves its ~9.14 h half-life?
fn main() {
    use outram_mc_libs::depletion::chain::DepletionChain;
    use outram_mc_libs::depletion::operator::{deplete_predictor, BurnupSettings, OneGroupWeighting};
    let chain = DepletionChain::simple();
    let initial = vec![("U235".to_string(), 7.0e-4), ("U238".to_string(), 2.2e-2)];
    let total = 40.0_f64;
    println!("  step[d]  step[h]   n      k_inf(EOL)            Xe135(EOL)");
    let mut ks = vec![]; let mut xs = vec![];
    for k in 0..8 {
        let h = 5.0 / 2f64.powi(k);
        let n = (total / h).round() as usize;
        let s = BurnupSettings { power_watts: 1.0e6, fuel_volume_cm3: 1.0e3, step_days: h,
            n_steps: n, temperature_k: 293.6, one_group_energy_ev: 0.0253,
            weighting: OneGroupWeighting::SingleEnergy };
        let r = deplete_predictor(&chain, &initial, &s);
        let l = r.steps.last().unwrap();
        let xe = l.densities.iter().find(|(x,_)|x=="Xe135").map(|(_,d)|*d).unwrap();
        println!("{h:9.5} {:8.2} {n:6}   {:.12}   {xe:.9e}", h*24.0, l.k_inf);
        ks.push(l.k_inf); xs.push(xe);
    }
    let ord = |a: f64, b: f64, c: f64| { let d1=a-b; let d2=b-c;
        if d2==0.0 || d1/d2<=0.0 { f64::NAN } else { (d1/d2).log2() } };
    println!("\n  order estimates (Richardson, successive triples)");
    for i in 0..ks.len()-2 {
        println!("   h={:8.5}  k_inf p={:6.3}   Xe135 p={:6.3}",
                 5.0/2f64.powi(i as i32), ord(ks[i],ks[i+1],ks[i+2]), ord(xs[i],xs[i+1],xs[i+2]));
    }
}
