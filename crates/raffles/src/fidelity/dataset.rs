// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-11-04 for GitHub issue #1194 (epic #1177)
// at the WORKSPACE MAINTAINER's direction; the crate owner's (Adolphus Lye's)
// review is outstanding — see this crate's CLAUDE.md, "Scope boundaries".

//! [`Dataset`]: model runs `(x, y)` with a provenance, a fidelity tag and a
//! training / held-out split fixed at construction; [`MultiFidelityDataset`]
//! orders several of them low → high.

use core::marker::PhantomData;

use super::quantities::{Channel, Quantities};
use crate::{RafflesError, Result};

/// Whether a row may be used to fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Split {
    /// A row the surrogate is fitted to.
    Training,
    /// A check: never read by any fit.
    HeldOut,
}

/// Which model produced a dataset: a rank (higher is higher fidelity) and a
/// label for a human (`"CFD"`, `"TUAS 1-D"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fidelity {
    /// Higher is higher fidelity; [`MultiFidelityDataset`] orders by it.
    pub rank: u32,
    /// What the model is.
    pub label: String,
}

impl Fidelity {
    /// A fidelity tag.
    #[must_use]
    pub fn new(rank: u32, label: &str) -> Self {
        Self {
            rank,
            label: label.to_string(),
        }
    }
}

/// Runs of one model: typed at the API, SI `f64` rows inside.
///
/// **The split is fixed at construction and cannot be changed.** Fitting
/// reads only [`training_si`](Self::training_si), which never contains a
/// held-out row; validation reads [`held_out_si`](Self::held_out_si). The
/// test `poisoned_held_out_outputs_do_not_change_the_fit` (in
/// `fidelity::build`) pins that.
///
/// Values are not checked for finiteness here: a fit refuses non-finite
/// training values itself, and a non-finite held-out value shows up as a
/// non-finite error in the report, which is the honest outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct Dataset<I: Quantities, O: Quantities> {
    inputs: Vec<Vec<f64>>,
    outputs: Vec<Vec<f64>>,
    split: Vec<Split>,
    fidelity: Fidelity,
    provenance: String,
    _types: PhantomData<fn(I) -> O>,
}

/// SI rows of one side of a split: `(inputs, outputs)`.
pub type SiRows = (Vec<Vec<f64>>, Vec<Vec<f64>>);

impl<I: Quantities, O: Quantities> Dataset<I, O> {
    /// A dataset from typed rows, one split per row.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if the three vectors differ in
    /// length; [`RafflesError::InvalidParameter`] if there is no training row.
    pub fn new(
        inputs: &[I],
        outputs: &[O],
        split: &[Split],
        fidelity: Fidelity,
        provenance: &str,
    ) -> Result<Self> {
        let inputs: Vec<Vec<f64>> = inputs.iter().map(Quantities::to_si).collect();
        let outputs: Vec<Vec<f64>> = outputs.iter().map(Quantities::to_si).collect();
        Self::from_si(inputs, outputs, split.to_vec(), fidelity, provenance)
    }

    /// A dataset from SI rows (in [`Quantities::channels`] order).
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new), plus [`RafflesError::DimensionMismatch`] for a
    /// row of the wrong width.
    pub fn from_si(
        inputs: Vec<Vec<f64>>,
        outputs: Vec<Vec<f64>>,
        split: Vec<Split>,
        fidelity: Fidelity,
        provenance: &str,
    ) -> Result<Self> {
        if inputs.len() != outputs.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: inputs.len(),
                found: outputs.len(),
            });
        }
        if inputs.len() != split.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: inputs.len(),
                found: split.len(),
            });
        }
        let (wi, wo) = (I::width(), O::width());
        for (x, y) in inputs.iter().zip(outputs.iter()) {
            if x.len() != wi {
                return Err(RafflesError::DimensionMismatch {
                    expected: wi,
                    found: x.len(),
                });
            }
            if y.len() != wo {
                return Err(RafflesError::DimensionMismatch {
                    expected: wo,
                    found: y.len(),
                });
            }
        }
        if !split.contains(&Split::Training) {
            return Err(RafflesError::InvalidParameter {
                parameter: "split".to_string(),
                value: 0.0,
                reason: "a dataset needs at least one training row".to_string(),
            });
        }
        Ok(Self {
            inputs,
            outputs,
            split,
            fidelity,
            provenance: provenance.to_string(),
            _types: PhantomData,
        })
    }

    /// Number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.split.len()
    }

    /// Whether there are no rows (never, by construction).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.split.is_empty()
    }

    /// Each row's split.
    #[must_use]
    pub fn split(&self) -> &[Split] {
        &self.split
    }

    /// Number of training / held-out rows.
    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        let t = self.split.iter().filter(|s| **s == Split::Training).count();
        (t, self.split.len() - t)
    }

    /// The fidelity tag.
    #[must_use]
    pub fn fidelity(&self) -> &Fidelity {
        &self.fidelity
    }

    /// Where the data came from.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// Input and output channels.
    #[must_use]
    pub fn channels() -> (Vec<Channel>, Vec<Channel>) {
        (I::channels(), O::channels())
    }

    /// Row `i`, typed.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if `i` is out of range.
    pub fn row(&self, i: usize) -> Result<(I, O, Split)> {
        if i >= self.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: self.len(),
                found: i,
            });
        }
        Ok((
            I::from_si(&self.inputs[i])?,
            O::from_si(&self.outputs[i])?,
            self.split[i],
        ))
    }

    /// The training rows, SI. The only rows any fit reads.
    #[must_use]
    pub fn training_si(&self) -> SiRows {
        self.side(Split::Training)
    }

    /// The held-out rows, SI.
    #[must_use]
    pub fn held_out_si(&self) -> SiRows {
        self.side(Split::HeldOut)
    }

    fn side(&self, which: Split) -> SiRows {
        let mut x = Vec::new();
        let mut y = Vec::new();
        for i in 0..self.len() {
            if self.split[i] == which {
                x.push(self.inputs[i].clone());
                y.push(self.outputs[i].clone());
            }
        }
        (x, y)
    }
}

/// Datasets of the same `(I, O)` at increasing fidelity, low → high (the
/// input to a co-kriging route, gh:#1190).
#[derive(Debug, Clone, PartialEq)]
pub struct MultiFidelityDataset<I: Quantities, O: Quantities> {
    levels: Vec<Dataset<I, O>>,
}

impl<I: Quantities, O: Quantities> MultiFidelityDataset<I, O> {
    /// Levels in order of strictly increasing [`Fidelity::rank`].
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `levels` is empty or the ranks
    /// do not strictly increase.
    pub fn new(levels: Vec<Dataset<I, O>>) -> Result<Self> {
        if levels.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "levels".into(),
                value: 0.0,
                reason: "at least one level".into(),
            });
        }
        for w in levels.windows(2) {
            if w[1].fidelity.rank <= w[0].fidelity.rank {
                return Err(RafflesError::InvalidParameter {
                    parameter: "levels".into(),
                    value: f64::from(w[1].fidelity.rank),
                    reason: "fidelity ranks must strictly increase, low to high".into(),
                });
            }
        }
        Ok(Self { levels })
    }

    /// The levels, low → high.
    #[must_use]
    pub fn levels(&self) -> &[Dataset<I, O>] {
        &self.levels
    }

    /// The highest-fidelity level.
    #[must_use]
    pub fn highest(&self) -> &Dataset<I, O> {
        &self.levels[self.levels.len() - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{Length, Ratio};
    use uom::si::length::meter;
    use uom::si::ratio::ratio;

    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct X { l: Length }
    }
    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct Y { r: Ratio, s: Ratio }
    }

    fn data(rank: u32) -> Dataset<X, Y> {
        let x: Vec<X> = (0..4)
            .map(|i| X {
                l: Length::new::<meter>(f64::from(i)),
            })
            .collect();
        let y: Vec<Y> = (0..4)
            .map(|i| Y {
                r: Ratio::new::<ratio>(f64::from(i) * 2.0),
                s: Ratio::new::<ratio>(-1.0),
            })
            .collect();
        let split = [
            Split::Training,
            Split::HeldOut,
            Split::Training,
            Split::HeldOut,
        ];
        Dataset::new(&x, &y, &split, Fidelity::new(rank, "toy"), "unit test").unwrap()
    }

    /// The split stays where construction put it; the two sides partition
    /// the rows; typed rows read back exactly.
    #[test]
    fn the_split_is_fixed_at_construction() {
        let d = data(1);
        assert_eq!(d.len(), 4);
        assert!(!d.is_empty());
        assert_eq!(d.counts(), (2, 2));
        let (tx, ty) = d.training_si();
        assert_eq!(tx, vec![vec![0.0], vec![2.0]]);
        assert_eq!(ty, vec![vec![0.0, -1.0], vec![4.0, -1.0]]);
        let (hx, _) = d.held_out_si();
        assert_eq!(hx, vec![vec![1.0], vec![3.0]]);
        let (x, y, s) = d.row(3).unwrap();
        assert_eq!((x.l.value, y.r.value, s), (3.0, 6.0, Split::HeldOut));
        assert!(d.row(4).is_err());
        assert_eq!(d.split()[1], Split::HeldOut);
        assert_eq!(d.fidelity().label, "toy");
        assert_eq!(d.provenance(), "unit test");
        let (ci, co) = Dataset::<X, Y>::channels();
        assert_eq!((ci.len(), co.len()), (1, 2));
    }

    /// Malformed datasets are refused: ragged, mismatched, no training row.
    #[test]
    fn malformed_datasets_are_refused() {
        let f = || Fidelity::new(0, "toy");
        assert!(
            Dataset::<X, Y>::from_si(vec![vec![0.0]], vec![], vec![Split::Training], f(), "")
                .is_err()
        );
        assert!(
            Dataset::<X, Y>::from_si(vec![vec![0.0]], vec![vec![0.0, 0.0]], vec![], f(), "")
                .is_err()
        );
        assert!(Dataset::<X, Y>::from_si(
            vec![vec![0.0, 1.0]],
            vec![vec![0.0, 0.0]],
            vec![Split::Training],
            f(),
            ""
        )
        .is_err());
        assert!(Dataset::<X, Y>::from_si(
            vec![vec![0.0]],
            vec![vec![0.0]],
            vec![Split::Training],
            f(),
            ""
        )
        .is_err());
        assert!(Dataset::<X, Y>::from_si(
            vec![vec![0.0]],
            vec![vec![0.0, 0.0]],
            vec![Split::HeldOut],
            f(),
            ""
        )
        .is_err());
    }

    /// Levels must be ordered low → high.
    #[test]
    fn multi_fidelity_levels_are_ordered() {
        let m = MultiFidelityDataset::new(vec![data(0), data(1)]).unwrap();
        assert_eq!(m.levels().len(), 2);
        assert_eq!(m.highest().fidelity().rank, 1);
        assert!(MultiFidelityDataset::new(vec![data(1), data(1)]).is_err());
        assert!(MultiFidelityDataset::<X, Y>::new(vec![]).is_err());
    }
}
