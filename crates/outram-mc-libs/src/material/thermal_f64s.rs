//! **A [`ThermalScattering`] law as plain `f64`s** (gh:#786), so a law built
//! once (by THERMR from a tape, or by LEAPR) can cross a boundary that only
//! moves numbers: the HTR-10 browser demo's Web Workers, where one worker
//! builds each law and every other receives it.
//!
//! **Exact.** Every `f64` is stored as itself; counts, flags and the name's
//! bytes as `f64`s, exact far beyond their range. Pinned bit for bit, in
//! cross sections and in sampled `(E', μ)`, by `f64s_round_trip_exactly`
//! below and by `tests/processed_evaluation_round_trip.rs` on real laws.

use super::{
    CoherentElasticTable, ContinuousEmission, EmissionTable, IncoherentElasticTable,
    ThermalElastic, ThermalScattering,
};
use njoy_outram_park_fork::NjoyError;

/// First word of the encoding.
const MAGIC: f64 = 7860.0;
/// Encoding version, bumped on any layout change.
const VERSION: f64 = 1.0;

/// Appends a length-prefixed vector.
fn put(v: &mut Vec<f64>, x: &[f64]) {
    v.push(x.len() as f64);
    v.extend_from_slice(x);
}

/// Reads the encoding front to back: the words and the position reached
/// (owned, not borrowed: the workspace keeps lifetimes off structs).
struct Reader {
    v: Vec<f64>,
    at: usize,
}

impl Reader {
    fn bad(what: &str) -> NjoyError {
        NjoyError::EndfParse(format!("thermal law from f64s: {what}"))
    }
    fn take(&mut self, n: usize) -> Result<Vec<f64>, NjoyError> {
        let s = self
            .v
            .get(self.at..self.at + n)
            .ok_or_else(|| Self::bad("too short"))?
            .to_vec();
        self.at += n;
        Ok(s)
    }
    fn one(&mut self) -> Result<f64, NjoyError> {
        Ok(self.take(1)?[0])
    }
    fn count(&mut self) -> Result<usize, NjoyError> {
        let n = self.one()?;
        if !(n >= 0.0 && n.fract() == 0.0) {
            return Err(Self::bad("a count is not a whole number"));
        }
        Ok(n as usize)
    }
    fn vec(&mut self) -> Result<Vec<f64>, NjoyError> {
        let n = self.count()?;
        self.take(n)
    }
}

impl ThermalScattering {
    /// Every field as `f64`s; read back with [`Self::from_f64s`].
    ///
    /// Layout: `[MAGIC, VERSION]`, the name (byte count, bytes), `cutoff_ev`,
    /// `selected_temperature_k`, `skewed`, `legacy_equiprobable`, the σ_inel
    /// grid and values, the emission grid, the binned tables (count, then per
    /// table `n_mu`, `e_out`, `cosines`), the continuous laws (count, then per
    /// law `n_mu`, `e_out`, `pdf`, `cdf`, `cosines`), and the elastic channel
    /// (kind `0` none, `1` coherent, `2` incoherent, `3` mixed, then its
    /// tables). Every vector is length-prefixed.
    pub fn to_f64s(&self) -> Vec<f64> {
        let mut v = vec![MAGIC, VERSION];
        let name: Vec<f64> = self.name.bytes().map(f64::from).collect();
        put(&mut v, &name);
        v.extend([
            self.cutoff_ev,
            self.selected_temperature_k,
            f64::from(u8::from(self.skewed)),
            f64::from(u8::from(self.legacy_equiprobable)),
        ]);
        put(&mut v, &self.xs_e);
        put(&mut v, &self.xs_sigma);
        put(&mut v, &self.emit_e);
        v.push(self.emit_tables.len() as f64);
        for t in &self.emit_tables {
            v.push(t.n_mu as f64);
            put(&mut v, &t.e_out);
            put(&mut v, &t.cosines);
        }
        v.push(self.continuous.len() as f64);
        for c in &self.continuous {
            v.push(c.n_mu as f64);
            put(&mut v, &c.e_out);
            put(&mut v, &c.pdf);
            put(&mut v, &c.cdf);
            put(&mut v, &c.cosines);
        }
        let coh = |v: &mut Vec<f64>, c: &CoherentElasticTable| {
            put(v, &c.edges_ev);
            put(v, &c.s_cum);
        };
        let inc = |v: &mut Vec<f64>, i: &IncoherentElasticTable| {
            v.push(i.n_mu as f64);
            put(v, &i.e_grid);
            put(v, &i.sigma);
            put(v, &i.cosines);
        };
        match &self.elastic {
            ThermalElastic::None => v.push(0.0),
            ThermalElastic::Coherent(c) => {
                v.push(1.0);
                coh(&mut v, c);
            }
            ThermalElastic::Incoherent(i) => {
                v.push(2.0);
                inc(&mut v, i);
            }
            ThermalElastic::Mixed(c, i) => {
                v.push(3.0);
                coh(&mut v, c);
                inc(&mut v, i);
            }
        }
        v
    }

    /// The inverse of [`Self::to_f64s`].
    ///
    /// # Errors
    ///
    /// [`NjoyError::EndfParse`] for a vector that is not this encoding, too
    /// short for what it declares, or with words left over.
    pub fn from_f64s(v: &[f64]) -> Result<Self, NjoyError> {
        let mut r = Reader {
            v: v.to_vec(),
            at: 0,
        };
        let h = r.take(2)?;
        if h[0] != MAGIC || h[1] != VERSION {
            return Err(Reader::bad("not a thermal law of this version"));
        }
        let name_bytes: Vec<u8> = r.vec()?.into_iter().map(|b| b as u8).collect();
        let name = String::from_utf8(name_bytes).map_err(|_| Reader::bad("name is not UTF-8"))?;
        let f = r.take(4)?;
        let (cutoff_ev, selected_temperature_k, skewed, legacy_equiprobable) =
            (f[0], f[1], f[2] != 0.0, f[3] != 0.0);
        let xs_e = r.vec()?;
        let xs_sigma = r.vec()?;
        let emit_e = r.vec()?;
        let n_tables = r.count()?;
        let mut emit_tables = Vec::with_capacity(n_tables);
        for _ in 0..n_tables {
            let n_mu = r.count()?;
            emit_tables.push(EmissionTable {
                n_mu,
                e_out: r.vec()?,
                cosines: r.vec()?,
            });
        }
        let n_cont = r.count()?;
        let mut continuous = Vec::with_capacity(n_cont);
        for _ in 0..n_cont {
            let n_mu = r.count()?;
            continuous.push(ContinuousEmission {
                n_mu,
                e_out: r.vec()?,
                pdf: r.vec()?,
                cdf: r.vec()?,
                cosines: r.vec()?,
            });
        }
        let coh = |r: &mut Reader| -> Result<CoherentElasticTable, NjoyError> {
            Ok(CoherentElasticTable {
                edges_ev: r.vec()?,
                s_cum: r.vec()?,
            })
        };
        let inc = |r: &mut Reader| -> Result<IncoherentElasticTable, NjoyError> {
            let n_mu = r.count()?;
            Ok(IncoherentElasticTable {
                n_mu,
                e_grid: r.vec()?,
                sigma: r.vec()?,
                cosines: r.vec()?,
            })
        };
        let elastic = match r.count()? {
            0 => ThermalElastic::None,
            1 => ThermalElastic::Coherent(coh(&mut r)?),
            2 => ThermalElastic::Incoherent(inc(&mut r)?),
            3 => {
                let c = coh(&mut r)?;
                ThermalElastic::Mixed(c, inc(&mut r)?)
            }
            _ => return Err(Reader::bad("unknown elastic kind")),
        };
        if r.at != v.len() {
            return Err(Reader::bad("words left over"));
        }
        Ok(Self {
            name,
            cutoff_ev,
            selected_temperature_k,
            xs_e,
            xs_sigma,
            emit_e,
            emit_tables,
            continuous,
            skewed,
            legacy_equiprobable,
            elastic,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A law with every channel kind crosses unchanged: every field, and
    /// cross sections and sampled `(E', μ)` on a grid of energies and seeds,
    /// bit for bit. Truncated or padded vectors are refused.
    #[test]
    fn f64s_round_trip_exactly() {
        let law = ThermalScattering {
            name: "c_Graphite".into(),
            cutoff_ev: 4.95,
            selected_temperature_k: 300.15,
            xs_e: vec![1.0e-5, 1.0e-3, 0.1, 1.0, 4.95],
            xs_sigma: vec![9.0, 6.0, 4.0, 4.5, 4.7],
            emit_e: vec![1.0e-5, 0.1, 4.95],
            emit_tables: (0..3)
                .map(|k| EmissionTable {
                    n_mu: 2,
                    e_out: vec![0.01 * (k + 1) as f64, 0.05, 0.2],
                    cosines: vec![-0.5, 0.5, -0.2, 0.3, 0.1, 0.9],
                })
                .collect(),
            // One continuous law per emission energy, as a real IFENG = 2
            // table has (the binned tables then go unused in sampling, but
            // are still carried, so both are exercised by the encoding).
            continuous: (0..3)
                .map(|k| ContinuousEmission {
                    n_mu: 1,
                    e_out: vec![0.0, 0.1 + 0.01 * k as f64],
                    pdf: vec![1.0, 9.0],
                    cdf: vec![0.0, 1.0],
                    cosines: vec![0.2, -0.4],
                })
                .collect(),
            skewed: true,
            legacy_equiprobable: false,
            elastic: ThermalElastic::Mixed(
                CoherentElasticTable {
                    edges_ev: vec![1.8e-3, 3.0e-3, 0.01],
                    s_cum: vec![0.0, 0.02, 0.05],
                },
                IncoherentElasticTable {
                    n_mu: 2,
                    e_grid: vec![1.0e-5, 1.0],
                    sigma: vec![2.0, 1.0],
                    cosines: vec![-0.3, 0.6, -0.1, 0.8],
                },
            ),
        };
        let v = law.to_f64s();
        let back = ThermalScattering::from_f64s(&v).expect("decode");
        assert_eq!(
            back.to_f64s()
                .iter()
                .map(|x| x.to_bits())
                .collect::<Vec<_>>(),
            v.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(back.name, law.name);
        for i in 0..300 {
            let e = 1.0e-5 * 10f64.powf(5.6 * f64::from(i) / 300.0);
            assert_eq!(law.total_xs(e).to_bits(), back.total_xs(e).to_bits(), "{e}");
            let (mut s1, mut s2) = (u64::from(i as u32) + 7, u64::from(i as u32) + 7);
            assert_eq!(
                format!("{:?}", law.sample(e, &mut s1)),
                format!("{:?}", back.sample(e, &mut s2)),
                "{e}"
            );
        }
        assert!(ThermalScattering::from_f64s(&v[..v.len() - 1]).is_err());
        let mut long = v.clone();
        long.push(0.0);
        assert!(ThermalScattering::from_f64s(&long).is_err());
    }
}
