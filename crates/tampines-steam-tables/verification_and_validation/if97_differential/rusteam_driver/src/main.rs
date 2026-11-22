//! rust-steam driver for gh:#1346. Reads the CoolProp reference CSV on stdin (plain
//! text) and, for every row it can answer, writes `TAG,<inputs>,<rusteam outputs>`.
//! rust-steam works in kJ/kg (h, u, s, cp, cv); outputs here are multiplied by 1000.0
//! to SI so they compare directly with this crate. That multiplication is the only
//! operation added, and the Rust test applies the same one.
//!
//! Row layouts written (`nan` where rusteam returns Err, `panic` where it panics):
//!   TP   T p  v h s u cp cv w
//!   SATT T psat
//!   SATP p Tsat
//!   PH   p h T        (h in J/kg, divided by 1000.0 for rusteam)
//!   PS   p s T        (s in J/kg/K, divided by 1000.0 for rusteam)
use rust_steam::iapws97::*;
use std::io::{BufRead, Write};

fn f(x: Result<f64, IAPWSError>, scale: f64) -> String {
    match x {
        Ok(v) => format!(",{:.17e}", v * scale),
        Err(_) => ",nan".to_string(),
    }
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let c: Vec<&str> = line.split(',').collect();
        let num = |i: usize| -> f64 { c[i].parse::<f64>().unwrap() };
        match c[0] {
            "TP" => {
                let (t, p) = (num(1), num(2));
                let r = std::panic::catch_unwind(|| {
                    let mut s = format!("TP,{:.17e},{:.17e}", t, p);
                    s += &f(vmass_tp(t, p), 1.0);
                    s += &f(hmass_tp(t, p), 1000.0);
                    s += &f(smass_tp(t, p), 1000.0);
                    s += &f(umass_tp(t, p), 1000.0);
                    s += &f(cpmass_tp(t, p), 1000.0);
                    s += &f(cvmass_tp(t, p), 1000.0);
                    s += &f(speed_sound_tp(t, p), 1.0);
                    s
                });
                match r {
                    Ok(s) => writeln!(out, "{}", s).unwrap(),
                    Err(_) => writeln!(
                        out,
                        "TP,{:.17e},{:.17e},panic,panic,panic,panic,panic,panic,panic",
                        t, p
                    )
                    .unwrap(),
                }
            }
            "SATT" => {
                let t = num(1);
                writeln!(out, "SATT,{:.17e},{:.17e}", t, psat97(&t)).unwrap();
            }
            "SATP" => {
                let p = num(1);
                writeln!(out, "SATP,{:.17e},{:.17e}", p, tsat97(&p)).unwrap();
            }
            "PH" => {
                let (p, h) = (num(1), num(2));
                let r = std::panic::catch_unwind(|| temperature_ph(p, h / 1000.0));
                match r {
                    Ok(x) => writeln!(out, "PH,{:.17e},{:.17e}{}", p, h, f(x, 1.0)).unwrap(),
                    Err(_) => writeln!(out, "PH,{:.17e},{:.17e},panic", p, h).unwrap(),
                }
            }
            "PS" => {
                let (p, s) = (num(1), num(2));
                let r = std::panic::catch_unwind(|| temperature_ps(p, s / 1000.0));
                match r {
                    Ok(x) => writeln!(out, "PS,{:.17e},{:.17e}{}", p, s, f(x, 1.0)).unwrap(),
                    Err(_) => writeln!(out, "PS,{:.17e},{:.17e},panic", p, s).unwrap(),
                }
            }
            _ => {}
        }
    }
}
