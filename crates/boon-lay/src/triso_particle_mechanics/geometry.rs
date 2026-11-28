// SPDX-License-Identifier: AGPL-3.0-only
//! Layers and the radial finite-element mesh of a spherically symmetric
//! particle.
//!
//! A particle is a list of [`LayerSpec`]s, innermost first, each a spherical
//! shell `inner_radius <= r <= outer_radius` with its own material and its own
//! number of equal-length two-node elements. Adjacent layers must touch: the
//! outer radius of one is the inner radius of the next (relative tolerance
//! 1e-12). The first inner radius may be zero (a solid kernel at the centre),
//! in which case [`super::mechanics`] fixes `u(0) = 0`, the symmetry condition.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use uom::si::f64::Length;
use uom::si::length::meter;

use super::material::LayerMaterial;
use super::ParticleMechanicsError;

/// One spherical shell of the particle.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerSpec {
    /// Name for reports ("IPyC", "SiC", ...). Not used by the physics.
    pub label: String,
    /// Inner radius of the shell, metres internally.
    pub inner_radius_m: f64,
    /// Outer radius of the shell, metres internally.
    pub outer_radius_m: f64,
    /// Number of equal-length linear elements across the shell (count, >= 1).
    pub n_elements: usize,
    /// The layer's material laws.
    pub material: LayerMaterial,
}

impl LayerSpec {
    /// A shell from `inner` to `outer` (lengths), meshed with `n_elements`
    /// equal elements.
    ///
    /// # Errors
    ///
    /// [`ParticleMechanicsError::InvalidGeometry`] if `inner < 0`,
    /// `outer <= inner`, either is non-finite, or `n_elements == 0`.
    pub fn new(
        label: &str,
        inner: Length,
        outer: Length,
        n_elements: usize,
        material: LayerMaterial,
    ) -> Result<Self, ParticleMechanicsError> {
        let a = inner.get::<meter>();
        let b = outer.get::<meter>();
        if !(a.is_finite() && b.is_finite()) || a < 0.0 || b <= a {
            return Err(ParticleMechanicsError::InvalidGeometry(format!(
                "layer {label}: need 0 <= inner < outer, got inner = {a} m, outer = {b} m"
            )));
        }
        if n_elements == 0 {
            return Err(ParticleMechanicsError::InvalidGeometry(format!(
                "layer {label}: needs at least one element"
            )));
        }
        Ok(Self {
            label: String::from(label),
            inner_radius_m: a,
            outer_radius_m: b,
            n_elements,
            material,
        })
    }
}

/// The assembled radial mesh: node radii and the layer each element is in.
///
/// Node `i` sits at `node_radius_m(i)`; element `e` joins nodes `e` and
/// `e + 1`. One displacement (and one temperature) per node.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleMesh {
    layers: Vec<LayerSpec>,
    nodes_m: Vec<f64>,
    element_layer: Vec<usize>,
}

impl ParticleMesh {
    /// Mesh a contiguous stack of layers, innermost first.
    ///
    /// # Errors
    ///
    /// [`ParticleMechanicsError::InvalidGeometry`] if the list is empty or two
    /// neighbours do not touch (relative gap above 1e-12 of the outer one).
    pub fn new(layers: Vec<LayerSpec>) -> Result<Self, ParticleMechanicsError> {
        if layers.is_empty() {
            return Err(ParticleMechanicsError::InvalidGeometry(String::from(
                "a particle needs at least one layer",
            )));
        }
        for w in layers.windows(2) {
            let gap = (w[1].inner_radius_m - w[0].outer_radius_m).abs();
            if gap > 1e-12 * w[1].inner_radius_m.max(w[0].outer_radius_m) {
                return Err(ParticleMechanicsError::InvalidGeometry(format!(
                    "layers {} and {} do not touch: {} m vs {} m",
                    w[0].label, w[1].label, w[0].outer_radius_m, w[1].inner_radius_m
                )));
            }
        }
        let mut nodes_m = vec![layers[0].inner_radius_m];
        let mut element_layer = Vec::new();
        for (il, layer) in layers.iter().enumerate() {
            let h = (layer.outer_radius_m - layer.inner_radius_m) / layer.n_elements as f64;
            for k in 1..=layer.n_elements {
                // The last node of a layer is its outer radius exactly, so the
                // next layer starts on it with no accumulated rounding.
                let r = if k == layer.n_elements {
                    layer.outer_radius_m
                } else {
                    layer.inner_radius_m + h * k as f64
                };
                nodes_m.push(r);
                element_layer.push(il);
            }
        }
        Ok(Self {
            layers,
            nodes_m,
            element_layer,
        })
    }

    /// Number of nodes (count).
    #[must_use]
    pub fn n_nodes(&self) -> usize {
        self.nodes_m.len()
    }

    /// Number of elements (count).
    #[must_use]
    pub fn n_elements(&self) -> usize {
        self.element_layer.len()
    }

    /// Radius of node `i`, metres.
    #[must_use]
    pub fn node_radius_m(&self, i: usize) -> f64 {
        self.nodes_m[i]
    }

    /// All node radii, metres, innermost first.
    #[must_use]
    pub fn node_radii_m(&self) -> &[f64] {
        &self.nodes_m
    }

    /// Index of the layer element `e` belongs to.
    #[must_use]
    pub fn element_layer(&self, e: usize) -> usize {
        self.element_layer[e]
    }

    /// The layers, innermost first.
    #[must_use]
    pub fn layers(&self) -> &[LayerSpec] {
        &self.layers
    }

    /// Innermost radius of the mesh, metres (0 for a solid centre).
    #[must_use]
    pub fn inner_radius_m(&self) -> f64 {
        self.nodes_m[0]
    }

    /// Outermost radius of the mesh, metres.
    #[must_use]
    pub fn outer_radius_m(&self) -> f64 {
        self.nodes_m[self.nodes_m.len() - 1]
    }
}
