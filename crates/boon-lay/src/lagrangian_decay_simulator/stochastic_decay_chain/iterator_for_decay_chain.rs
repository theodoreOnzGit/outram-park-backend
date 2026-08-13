use fission_yields_data::prelude::Nuclide;

use crate::{lagrangian_decay_simulator::StochasticDecayChain, prelude::HalfLifeAndDecayEnergyInfo};

// this part is vibe coded for Convenience
// Assuming `Nuclide` and `HalfLifeAndDecayEnergyInfo` are defined elsewhere.

// Convenience methods: .iter() and .iter_mut()
impl StochasticDecayChain {
    pub fn iter(&self) -> DecayChainIter<'_> {
        self.into_iter()
    }

    pub fn iter_mut(&mut self) -> DecayChainIterMut<'_> {
        self.into_iter()
    }
}

// Consuming iterator (moves out of DecayChain)
pub struct DecayChainIntoIter {
    inner: std::vec::IntoIter<(Nuclide, HalfLifeAndDecayEnergyInfo)>,
}

impl Iterator for DecayChainIntoIter {
    type Item = (Nuclide, HalfLifeAndDecayEnergyInfo);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for DecayChainIntoIter {}
impl std::iter::FusedIterator for DecayChainIntoIter {}

impl IntoIterator for StochasticDecayChain {
    type Item = (Nuclide, HalfLifeAndDecayEnergyInfo);
    type IntoIter = DecayChainIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        DecayChainIntoIter {
            inner: self.nuclides_and_decay_data_vec.into_iter(),
        }
    }
}

// Shared-reference iterator: &DecayChain
pub struct DecayChainIter<'a> {
    inner: std::slice::Iter<'a, (Nuclide, HalfLifeAndDecayEnergyInfo)>,
}

impl<'a> Iterator for DecayChainIter<'a> {
    type Item = &'a (Nuclide, HalfLifeAndDecayEnergyInfo);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a> ExactSizeIterator for DecayChainIter<'a> {}
impl<'a> std::iter::FusedIterator for DecayChainIter<'a> {}

impl<'a> IntoIterator for &'a StochasticDecayChain {
    type Item = &'a (Nuclide, HalfLifeAndDecayEnergyInfo);
    type IntoIter = DecayChainIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        DecayChainIter {
            inner: self.nuclides_and_decay_data_vec.iter(),
        }
    }
}

// Mutable-reference iterator: &mut DecayChain
pub struct DecayChainIterMut<'a> {
    inner: std::slice::IterMut<'a, (Nuclide, HalfLifeAndDecayEnergyInfo)>,
}

impl<'a> Iterator for DecayChainIterMut<'a> {
    type Item = &'a mut (Nuclide, HalfLifeAndDecayEnergyInfo);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a> ExactSizeIterator for DecayChainIterMut<'a> {}
impl<'a> std::iter::FusedIterator for DecayChainIterMut<'a> {}

impl<'a> IntoIterator for &'a mut StochasticDecayChain {
    type Item = &'a mut (Nuclide, HalfLifeAndDecayEnergyInfo);
    type IntoIter = DecayChainIterMut<'a>;

    fn into_iter(self) -> Self::IntoIter {
        DecayChainIterMut {
            inner: self.nuclides_and_decay_data_vec.iter_mut(),
        }
    }
}

// Example usage:
//
// let chain = DecayChain { nuclides_and_decay_data: vec![ (n1, info1), (n2, info2) ] };
//
// // Borrowed iteration
// for (nuclide, info) in &chain {
//     println!("{nuclide:?} -> {info:?}");
// }
//
// // Mutable iteration
// for (nuclide, info) in &mut chain.clone() {
//     // mutate info if needed
// }
//
// // Consuming iteration (moves data out)
// for (nuclide, info) in chain {
//     // take ownership
// }
