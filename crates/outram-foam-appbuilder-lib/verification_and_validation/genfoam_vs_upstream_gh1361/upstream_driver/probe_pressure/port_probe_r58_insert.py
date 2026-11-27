import sys
# usage: insert_probe.py <mod.rs> new|old   (TEMPORARY gh:#1909 probe dump in correct_pressure; never committed)
p = sys.argv[1]
mode = sys.argv[2]
s = open(p).read()
a_expr = "u_eqns[k].a(&g)" if mode == "new" else "u_eqns[k].eqn.a(&g)"
ds = '''
                    let mut ds = |name: &str, v: &[f64]| {
                        let mut line = format!("S {corr} {name}");
                        for x in v {
                            line.push_str(&format!(" {x:.17e}"));
                        }
                        writeln!(fh, "{line}").expect("w");
                    };'''
blk1 = '''
            // TEMP gh:#1909 probe dump (never committed)
            if let Ok(path) = std::env::var("PROBE1833_FILE") {
                let mx: usize = std::env::var("PROBE1833_MAXCORR").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
                if corr <= mx && self.time <= 1.0001 * self.dt {
                    use std::io::Write;
                    let mut fh = std::fs::OpenOptions::new().create(true).append(true).open(path).expect("probe");''' + ds + '''
                    for k in 0..2 {
                        let kk = k + 1;
                        ds(&format!("alpha{kk}"), &self.fluids[k].alpha.c);
                        ds(&format!("rho{kk}"), &self.fluids[k].rho.c);
                        ds(&format!("A{kk}cell"), &''' + a_expr + ''');
                        let h = u_eqns[k].h(&g, &self.fluids[k].u);
                        let hx: Vec<f64> = h.iter().flat_map(|v| [v.x, v.y, v.z]).collect();
                        ds(&format!("H{kk}cell"), &hx);
                        let ux: Vec<f64> = self.fluids[k].u.c.iter().flat_map(|v| [v.x, v.y, v.z]).collect();
                        ds(&format!("U{kk}"), &ux);
                        ds(&format!("alphaf{kk}"), &af[k][..ni]);
                        ds(&format!("Af{kk}"), &afk[k][..ni]);
                        ds(&format!("alphaRhof{kk}byDt"), &arf[k][..ni]);
                        ds(&format!("alphaRhof{kk}0byDt"), &arf0[k][..ni]);
                        ds(&format!("rAUf{kk}"), &rauf[k][..ni]);
                        ds(&format!("alpharAUf{kk}Star"), &star[k][..ni]);
                        ds(&format!("phiS{kk}"), &phi_s[k][..ni]);
                        ds(&format!("phiH{kk}"), &phi_h[k][..ni]);
                        ds(&format!("phiHbyA{kk}"), &phi_hbya[k][..ni]);
                    }
                    ds("Kd12", &self.kd_ff);
                    ds("Kdf12", &kdf[..ni]);
                    ds("phiHbyA", &phi_hbya_mix[..ni]);
                    ds("rAUf", &rau_mix[..ni]);
                    ds("p_rgh_pre", &self.p_rgh.c);
                }
            }
            // calcComp_2p.H
'''
assert "            // calcComp_2p.H\n" in s
s = s.replace("            // calcComp_2p.H\n", blk1, 1)
blk2 = '''
            // TEMP gh:#1909 probe dump (never committed)
            if let Ok(path) = std::env::var("PROBE1833_FILE") {
                if corr <= 2 && self.time <= 1.0001 * self.dt {
                    use std::io::Write;
                    let mut fh = std::fs::OpenOptions::new().create(true).append(true).open(path).expect("probe");''' + ds + '''
                    ds("pdiag", &peqn.ldu.diag);
                    ds("pupper", &peqn.ldu.upper);
                    ds("psource", &peqn.source);
                }
            }
'''
if mode == "new":
    key = "            let folded = peqn.folded(&g, &pic, &pbc);\n"
    assert key in s
    s = s.replace(key, blk2 + key, 1)
else:
    key = "            let mut x = self.p_rgh.c.clone();\n            let res = if let Some(pc) = self.controls.p_rgh_pcg {"
    assert key in s
    s = s.replace(key, blk2 + key, 1)
blk3 = '''
            // TEMP gh:#1909 probe dump (never committed)
            if let Ok(path) = std::env::var("PROBE1833_FILE") {
                let mx: usize = std::env::var("PROBE1833_MAXCORR").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
                if corr <= mx && self.time <= 1.0001 * self.dt {
                    use std::io::Write;
                    let mut fh = std::fs::OpenOptions::new().create(true).append(true).open(path).expect("probe");''' + ds + '''
                    ds("pEqnFlux", &p_flux[..ni]);
                    ds("p_rgh_post", &self.p_rgh.c);
                }
            }
'''
key = "            for f in 0..nf {\n                if !g.active[f] {\n                    continue;\n                }\n                self.phi[f] = phi_hbya_mix[f] + p_flux[f];"
assert key in s
s = s.replace(key, blk3 + key, 1)
open(p, 'w').write(s)
