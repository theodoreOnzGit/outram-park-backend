// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! The layered (thermally stratified) pool's column operations (gh:#1100):
//! Lagrangian re-layering by split and merge, the convective adjustment
//! of an unstable column, and implicit conduction across a stable one.
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! The pool is `N` horizontal layers stacked bottom first, each with its own
//! mass and internal energy, `(kg, J)`. The layers are Lagrangian (what
//! enters at a port joins the layer at the port's elevation, what leaves
//! comes from the layer there) and are re-layered by splitting and merging
//! ([`split_merge`]) so that none grows thick or thin. Elevations come from the cumulative mass over the pool's mean
//! density through the volume–altitude table (Boussinesq: the layers'
//! density differences set the stability, not the level).
//!
//! **Ported, not written fresh.** The stability rule is `sembawang`'s
//! two-layer torus pool, `sembawang::bwr_mark1::containment::Containment::layer_pool`
//! (crate-private; `Mechanisms::pool_layers`, 2026-10-29, after the PPOOLEX
//! experiments and Li and Kudinov's effective heat and momentum source
//! models of the Mark I torus): the layers mix back to one state
//! whenever the deep one is the lighter, the stratification then being
//! unstable. Here that rule is generalised from two layers to `N` by the
//! standard complete convective adjustment of a column (merge any lighter-
//! below-denser pair, re-test the merged block against its neighbours until
//! the column is stable; e.g. Rahmstorf 1993, "A fast and complete convection
//! scheme for ocean models", Ocean Modelling 101; **not re-checked against
//! the paper**: it is not in the kovan corpus), and "lighter" is decided by
//! IAPWS-IF97 density at a common pressure, not by temperature, so water
//! below 4 °C stratifies the right way round. `sembawang` neglects all other
//! exchange between its layers ("conduction and entrainment neglected");
//! here a stable column exchanges heat by molecular conduction (see
//! [`conduct`]).

/// Re-layer a Lagrangian column (bottom first) by splitting and merging,
/// keeping the count `n`, with `t = M / n` the mean layer mass:
///
/// - a **thin** layer (empty, or under `t / 2`) is merged with whichever
///   neighbour is closer to it in specific energy, and the heaviest layer is
///   then split into two equal halves;
/// - otherwise, while a layer is **thick** (over `2 t`), the adjacent pair
///   whose specific energies differ least (neither of them the thick one,
///   together at most `2 t`) is merged and the heaviest split.
///
/// A split keeps its specific energy, so it smears nothing; a merge mixes
/// two neighbours, the most alike when there is a choice, so a
/// stratification survives water entering at one end and leaving at the
/// other (an equal-mass remap every step diffuses it like a first-order
/// upwind scheme). **After DYRESM's Lagrangian layers**, split when too
/// thick and amalgamated when too thin (Imberger, Patterson, Hebbert and
/// Loh 1978, "Dynamics of reservoir of medium size", J. Hydraulics Division
/// ASCE 104; **not re-checked**: not in the kovan corpus); here the count
/// is held fixed, so each split pays for a merge, and the bounds `t / 2` and
/// `2 t` are the resolution, not a physical coefficient. Mass and energy are
/// conserved exactly (sums, and halving, are exact in binary floating
/// point). A column with every layer in `[t / 2, 2 t]` is left bit for bit.
/// Returns the number of split-merge operations.
pub(crate) fn split_merge(layers: &mut Vec<(f64, f64)>) -> usize {
    let n = layers.len();
    if n < 2 {
        return 0;
    }
    let total: f64 = layers.iter().map(|l| l.0).sum();
    if !(total > 0.0) {
        return 0;
    }
    let mean = total / n as f64;
    let (thin, thick) = (0.5 * mean, 2.0 * mean);
    let u = |l: (f64, f64)| if l.0 > 0.0 { l.1 / l.0 } else { f64::NAN };
    let heaviest = |ls: &[(f64, f64)]| (0..ls.len()).fold(0, |b, i| if ls[i].0 > ls[b].0 { i } else { b });
    let mut ops = 0;
    for _ in 0..4 * n {
        // The thinnest layer, if thin.
        let lightest = (0..n).fold(0, |b, i| if layers[i].0 < layers[b].0 { i } else { b });
        let k = if layers[lightest].0 < thin {
            // Merge it with its closer neighbour (an empty layer, or an
            // empty neighbour, merges with either: the lower when there is
            // a choice).
            let i = lightest;
            let ui = u(layers[i]);
            let du = |j: usize| {
                let d = (u(layers[j]) - ui).abs();
                if d.is_nan() { 0.0 } else { d }
            };
            if i == 0 {
                0
            } else if i == n - 1 || du(i - 1) <= du(i + 1) {
                i - 1
            } else {
                i
            }
        } else {
            let big = heaviest(layers);
            if layers[big].0 <= thick {
                break;
            }
            let mut best: Option<(usize, f64)> = None;
            for k in 0..n - 1 {
                let (a, b) = (layers[k], layers[k + 1]);
                if k == big || k + 1 == big || a.0 + b.0 > thick {
                    continue;
                }
                let key = (u(a) - u(b)).abs();
                if best.is_none_or(|(_, kb)| key < kb) {
                    best = Some((k, key));
                }
            }
            match best {
                Some((k, _)) => k,
                None => break,
            }
        };
        let merged = (layers[k].0 + layers[k + 1].0, layers[k].1 + layers[k + 1].1);
        layers[k] = merged;
        layers.remove(k + 1);
        let big = heaviest(layers);
        let half = (0.5 * layers[big].0, 0.5 * layers[big].1);
        layers[big] = half;
        layers.insert(big + 1, half);
        ops += 1;
    }
    ops
}

/// Complete convective adjustment of a column (bottom first): wherever a
/// block of layers is lighter than the block above it, the two mix to one
/// specific energy (mass-weighted, energy conserved), and the merged block
/// is tested again against its neighbours until the column is stable.
/// `density(u)` gives the density of water of specific internal energy `u`
/// at the common reference pressure. Empty layers take no part. Layers in a
/// block of one are left bit for bit; in a merged block the top layer takes
/// the remainder of the block's energy. Returns the number of merges.
pub(crate) fn convective_adjustment<D: Fn(f64) -> f64>(layers: &mut [(f64, f64)], density: D) -> usize {
    // Blocks: (first layer, last layer, mass, energy, density).
    let mut blocks: Vec<(usize, usize, f64, f64, f64)> = Vec::with_capacity(layers.len());
    let mut merges = 0;
    for (i, l) in layers.iter().enumerate() {
        if !(l.0 > 0.0) {
            continue;
        }
        blocks.push((i, i, l.0, l.1, density(l.1 / l.0)));
        // The new block is the upper one: unstable when it is denser than
        // the block below it.
        while blocks.len() >= 2 {
            let up = blocks[blocks.len() - 1];
            let dn = blocks[blocks.len() - 2];
            if up.4 > dn.4 {
                blocks.pop();
                blocks.pop();
                let (m, e) = (dn.2 + up.2, dn.3 + up.3);
                blocks.push((dn.0, up.1, m, e, density(e / m)));
                merges += 1;
            } else {
                break;
            }
        }
    }
    for b in blocks {
        if b.0 == b.1 {
            continue;
        }
        let u = b.3 / b.2;
        let mut used = 0.0;
        let mut top = b.0;
        for (i, l) in layers.iter_mut().enumerate().take(b.1 + 1).skip(b.0) {
            if l.0 > 0.0 {
                l.1 = l.0 * u;
                used += l.1;
                top = i;
            }
        }
        layers[top].1 += b.3 - used;
    }
    merges
}

/// One backward-Euler step of conduction between neighbouring layers:
/// `C_i (T_i' − T_i) = Δt Σ G (T_j' − T_i')`, `C_i` each layer's heat
/// capacity, J/K, and `g[k]` the conductance between layers `k` and `k + 1`,
/// W/K, solved by the tridiagonal (Thomas) algorithm. The energy moved
/// across each interface (what the layers below it gave up, `−Σ C_i (T_i' −
/// T_i)`, equal to `Δt G_k (T_k' − T_{k+1}')` in exact arithmetic but
/// well conditioned when a layer is thin) is then taken from one layer and
/// given to the other, so the column's energy is conserved to round-off
/// whatever the heat capacities' temperature dependence. A layer
/// with no heat capacity (empty) conducts nothing. Returns the energy moved
/// across each interface, J (positive upward).
///
/// **The physics, and why no eddy diffusivity.** A stably stratified,
/// otherwise quiescent pool damps turbulence (buoyancy suppresses vertical
/// mixing; e.g. Turner, *Buoyancy Effects in Fluids*, CUP 1973, **not
/// re-checked**: not in the kovan corpus), so the heat transport across it
/// falls to molecular conduction, `k` from MELCOR's liquid-water
/// conductivity table (NCG/H2O RM §4.1.1, as `pool_surface`). This is the
/// lower bound of the exchange: stirring by vents, spargers and plumes
/// (Li and Kudinov's effective momentum sources) would add an eddy
/// diffusivity that needs a model of the stirring, which is not here
/// (stated); so the stratification this computes is the strongest the
/// layering allows, and the surface layer the hottest.
pub(crate) fn conduct(layers: &mut [(f64, f64)], t: &[f64], cap: &[f64], g: &[f64], dt: f64) -> Vec<f64> {
    let n = layers.len();
    let mut q = vec![0.0; n.saturating_sub(1)];
    if n < 2 {
        return q;
    }
    // `G Δt` is bounded by 1e6 times the smaller heat capacity: beyond that
    // the pair relaxes within `e^-1e6` of a step either way, and an
    // unbounded ratio makes the system singular in floating point (a 1e-9 kg
    // film's layers, `C/(G Δt)` below 1e-16, gave garbage temperatures:
    // found 2026-11-01). A conditioning bound, not physics.
    let gk: Vec<f64> = (0..n - 1)
        .map(|k| if cap[k] > 0.0 && cap[k + 1] > 0.0 && g[k] > 0.0 { (g[k] * dt).min(1e6 * cap[k].min(cap[k + 1])) } else { 0.0 })
        .collect();
    // Thomas algorithm on rows with C_i > 0 (an empty layer's row is the
    // identity, its conductances zero).
    let mut c_star = vec![0.0; n];
    let mut d_star = vec![0.0; n];
    for i in 0..n {
        let below = if i > 0 { gk[i - 1] } else { 0.0 };
        let above = if i + 1 < n { gk[i] } else { 0.0 };
        let (a, b, c, d) = if cap[i] > 0.0 { (-below, cap[i] + below + above, -above, cap[i] * t[i]) } else { (0.0, 1.0, 0.0, t[i]) };
        let denom = if i > 0 { b - a * c_star[i - 1] } else { b };
        c_star[i] = c / denom;
        d_star[i] = if i > 0 { (d - a * d_star[i - 1]) / denom } else { d / denom };
    }
    let mut tn = vec![0.0; n];
    tn[n - 1] = d_star[n - 1];
    for i in (0..n - 1).rev() {
        tn[i] = d_star[i] - c_star[i] * tn[i + 1];
    }
    // The energy across interface k is what layers 0..=k gave up,
    // `−Σ C_i (T_i' − T_i)`, rather than `Δt G_k (T_k' − T_{k+1}')`: the two
    // agree in exact arithmetic, but the second multiplies a huge
    // conductance by a round-off temperature difference when layers are
    // thin (found 2026-11-01: a 1e-9 kg pool in three layers gained 7e5 J).
    // A zero conductance (an empty layer) separates independent blocks.
    let mut acc = 0.0;
    for k in 0..n - 1 {
        acc -= cap[k] * (tn[k] - t[k]);
        if gk[k] > 0.0 {
            layers[k].1 -= acc;
            layers[k + 1].1 += acc;
            q[k] = acc;
        } else {
            acc = 0.0;
        }
    }
    q
}
