//! 2-D Cartesian domain decomposition and distributed solve (bead op-gj5).
//!
//! Extends the 1-D [`super`] decomposition to a **2-D process grid** built on the
//! MPI Cartesian topology ([`outram_park_mpi::CartesianComm`]): the global
//! `nx × ny` cell grid is tiled into rectangular blocks, one per rank, and each
//! rank finds its four face neighbours with [`cart_shift`](outram_park_mpi::CartesianComm::shift).
//! A 5-point-stencil halo exchange ([`exchange_halo_2d`]) swaps edge rows/columns,
//! and a distributed conjugate gradient ([`distributed_cg_2d`]) — halo-exchanged
//! matvec plus [`all_reduce`](outram_park_mpi::Communicator)-reduced dot products
//! (reusing [`super::krylov::distributed_dot`]) — solves the 2-D shifted-Poisson
//! system to the **same answer as a serial solve at any process-grid shape**
//! (checked in the module tests).
//!
//! # Scope / human-review flags
//!
//! Verification-only, untrusted AI draft. This demonstrates 2-D distributed
//! partitioning + solve on a model SPD operator; 3-D, unstructured meshes,
//! preconditioning, and using it as the linear stage of pflotran's real Newton
//! solve remain op-gj5 follow-ups. Blocks are balanced per axis; no
//! reorder-for-locality.

use outram_park_mpi::{CartesianComm, MpiResult};

use super::krylov::distributed_dot;

/// Balanced contiguous 1-axis split: `(start, len)` for `coord` of `nparts` over
/// `n` cells (the first `n % nparts` parts get one extra).
fn balanced(n: usize, nparts: usize, coord: usize) -> (usize, usize) {
    let base = n / nparts;
    let rem = n % nparts;
    let len = base + if coord < rem { 1 } else { 0 };
    let start = coord * base + coord.min(rem);
    (start, len)
}

/// A 2-D rectangular block decomposition of an `nx_global × ny_global` cell grid
/// over a Cartesian process grid.
///
/// Local fields are stored row-major within the block: local cell `(ix, iy)` is at
/// index `iy * lx + ix`, for `ix in 0..lx`, `iy in 0..ly`.
pub struct Decomposition2D {
    /// Global cell counts.
    pub nx_global: usize,
    /// Global cell counts.
    pub ny_global: usize,
    /// The 2-D Cartesian communicator (`dims = [px, py]`).
    pub cart: CartesianComm,
    /// Global x offset of this block's first column.
    pub x0: usize,
    /// Global y offset of this block's first row.
    pub y0: usize,
    /// Local block width (x) and height (y).
    pub lx: usize,
    /// Local block width (x) and height (y).
    pub ly: usize,
    /// Neighbour ranks (in the Cartesian comm): `-x`, `+x`, `-y`, `+y`; `None` at a
    /// physical domain edge.
    pub left: Option<i32>,
    pub right: Option<i32>,
    pub down: Option<i32>,
    pub up: Option<i32>,
}

impl Decomposition2D {
    /// Build the 2-D block decomposition for this rank from the global grid size
    /// and a 2-D [`CartesianComm`] (its `dims` are the process grid `[px, py]`).
    pub fn new(nx_global: usize, ny_global: usize, cart: CartesianComm) -> Self {
        let dims = cart.dims();
        let (px, py) = (dims[0] as usize, dims[1] as usize);
        let coords = cart.my_coords();
        let (cx, cy) = (coords[0] as usize, coords[1] as usize);
        let (x0, lx) = balanced(nx_global, px, cx);
        let (y0, ly) = balanced(ny_global, py, cy);
        // cart_shift(dim, +1) -> (source = lower neighbour, dest = upper neighbour).
        let (left, right) = cart.shift(0, 1);
        let (down, up) = cart.shift(1, 1);
        Decomposition2D {
            nx_global,
            ny_global,
            cart,
            x0,
            y0,
            lx,
            ly,
            left,
            right,
            down,
            up,
        }
    }

    /// Number of local cells (`lx * ly`).
    pub fn local_len(&self) -> usize {
        self.lx * self.ly
    }
}

/// Ghost rows/columns received from the four face neighbours (empty where the
/// block touches a physical domain edge — a zero Dirichlet ghost).
#[derive(Debug, Clone, Default)]
pub struct Halo2D {
    /// Left neighbour's rightmost column (length `ly`), else empty.
    pub left: Vec<f64>,
    /// Right neighbour's leftmost column (length `ly`), else empty.
    pub right: Vec<f64>,
    /// Down neighbour's top row (length `lx`), else empty.
    pub down: Vec<f64>,
    /// Up neighbour's bottom row (length `lx`), else empty.
    pub up: Vec<f64>,
}

const TAG_X_RIGHT: i32 = 201; // sent to the +x neighbour
const TAG_X_LEFT: i32 = 202; // sent to the -x neighbour
const TAG_Y_UP: i32 = 203; // sent to the +y neighbour
const TAG_Y_DOWN: i32 = 204; // sent to the -y neighbour

/// Exchange the four edge rows/columns of `u` with the block's face neighbours for
/// a 5-point stencil.
///
/// # Errors
/// Propagates any transport error.
pub fn exchange_halo_2d(decomp: &Decomposition2D, u: &[f64]) -> MpiResult<Halo2D> {
    let comm = decomp.cart.comm();
    let (lx, ly) = (decomp.lx, decomp.ly);
    let at = |ix: usize, iy: usize| u[iy * lx + ix];

    // Post sends (buffered), then receive.
    if let Some(r) = decomp.right {
        let col: Vec<f64> = (0..ly).map(|iy| at(lx - 1, iy)).collect();
        comm.send(&col, r, TAG_X_RIGHT)?;
    }
    if let Some(l) = decomp.left {
        let col: Vec<f64> = (0..ly).map(|iy| at(0, iy)).collect();
        comm.send(&col, l, TAG_X_LEFT)?;
    }
    if let Some(u_) = decomp.up {
        let row: Vec<f64> = (0..lx).map(|ix| at(ix, ly - 1)).collect();
        comm.send(&row, u_, TAG_Y_UP)?;
    }
    if let Some(d) = decomp.down {
        let row: Vec<f64> = (0..lx).map(|ix| at(ix, 0)).collect();
        comm.send(&row, d, TAG_Y_DOWN)?;
    }

    let mut halo = Halo2D::default();
    // Left ghost = left neighbour's right column (it sent with TAG_X_RIGHT).
    if let Some(l) = decomp.left {
        halo.left = comm.recv::<f64>(l, TAG_X_RIGHT)?.0;
    }
    if let Some(r) = decomp.right {
        halo.right = comm.recv::<f64>(r, TAG_X_LEFT)?.0;
    }
    if let Some(d) = decomp.down {
        halo.down = comm.recv::<f64>(d, TAG_Y_UP)?.0;
    }
    if let Some(u_) = decomp.up {
        halo.up = comm.recv::<f64>(u_, TAG_Y_DOWN)?.0;
    }
    Ok(halo)
}

/// Apply the 2-D shifted-Poisson operator `A = (4+shift) I - L` (5-point stencil)
/// to the distributed field `u`, returning `A u` on the local block. Ghosts at a
/// physical domain edge are `0` (homogeneous Dirichlet); `shift > 0` makes `A` SPD.
///
/// # Errors
/// Propagates any halo-exchange transport error.
pub fn poisson_matvec_2d(
    decomp: &Decomposition2D,
    u: &[f64],
    shift: f64,
) -> MpiResult<Vec<f64>> {
    let halo = exchange_halo_2d(decomp, u)?;
    let (lx, ly) = (decomp.lx, decomp.ly);
    let at = |ix: usize, iy: usize| u[iy * lx + ix];
    let mut out = vec![0.0; lx * ly];
    for iy in 0..ly {
        for ix in 0..lx {
            let west = if ix > 0 {
                at(ix - 1, iy)
            } else {
                halo.left.get(iy).copied().unwrap_or(0.0)
            };
            let east = if ix + 1 < lx {
                at(ix + 1, iy)
            } else {
                halo.right.get(iy).copied().unwrap_or(0.0)
            };
            let south = if iy > 0 {
                at(ix, iy - 1)
            } else {
                halo.down.get(ix).copied().unwrap_or(0.0)
            };
            let north = if iy + 1 < ly {
                at(ix, iy + 1)
            } else {
                halo.up.get(ix).copied().unwrap_or(0.0)
            };
            out[iy * lx + ix] = (4.0 + shift) * at(ix, iy) - west - east - south - north;
        }
    }
    Ok(out)
}

/// Solve the 2-D shifted-Poisson system `A u = b` by distributed conjugate
/// gradient over the block decomposition, starting from `u = 0`. Returns this
/// rank's solution block and the iteration count.
///
/// # Errors
/// Propagates any transport error from the matvec or reduced dot products.
pub fn distributed_cg_2d(
    decomp: &Decomposition2D,
    b: &[f64],
    shift: f64,
    tol: f64,
    max_iter: usize,
) -> MpiResult<(Vec<f64>, usize)> {
    let comm = decomp.cart.comm();
    let n = b.len();
    let mut x = vec![0.0; n];
    let mut r = b.to_vec();
    let mut p = r.clone();
    let mut rs_old = distributed_dot(comm, &r, &r)?;
    let mut iters = 0;
    if rs_old.sqrt() < tol {
        return Ok((x, 0));
    }
    for k in 0..max_iter {
        iters = k + 1;
        let ap = poisson_matvec_2d(decomp, &p, shift)?;
        let alpha = rs_old / distributed_dot(comm, &p, &ap)?;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rs_new = distributed_dot(comm, &r, &r)?;
        if rs_new.sqrt() < tol {
            break;
        }
        let beta = rs_new / rs_old;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }
        rs_old = rs_new;
    }
    Ok((x, iters))
}

/// Serial reference: the 2-D shifted-Poisson CG on one rank over the whole
/// `nx × ny` grid (row-major `iy*nx + ix`), the oracle for [`distributed_cg_2d`].
pub fn serial_poisson_2d_cg(
    nx: usize,
    ny: usize,
    b: &[f64],
    shift: f64,
    tol: f64,
    max_iter: usize,
) -> (Vec<f64>, usize) {
    let n = nx * ny;
    let at = |u: &[f64], ix: usize, iy: usize| u[iy * nx + ix];
    let matvec = |u: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; n];
        for iy in 0..ny {
            for ix in 0..nx {
                let west = if ix > 0 { at(u, ix - 1, iy) } else { 0.0 };
                let east = if ix + 1 < nx { at(u, ix + 1, iy) } else { 0.0 };
                let south = if iy > 0 { at(u, ix, iy - 1) } else { 0.0 };
                let north = if iy + 1 < ny { at(u, ix, iy + 1) } else { 0.0 };
                out[iy * nx + ix] = (4.0 + shift) * at(u, ix, iy) - west - east - south - north;
            }
        }
        out
    };
    let dot = |a: &[f64], c: &[f64]| -> f64 { a.iter().zip(c).map(|(x, y)| x * y).sum() };

    let mut x = vec![0.0; n];
    let mut r = b.to_vec();
    let mut p = r.clone();
    let mut rs_old = dot(&r, &r);
    let mut iters = 0;
    if rs_old.sqrt() < tol {
        return (x, 0);
    }
    for k in 0..max_iter {
        iters = k + 1;
        let ap = matvec(&p);
        let alpha = rs_old / dot(&p, &ap);
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rs_new = dot(&r, &r);
        if rs_new.sqrt() < tol {
            break;
        }
        let beta = rs_new / rs_old;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }
        rs_old = rs_new;
    }
    (x, iters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_mpi::run;

    /// Gather a distributed 2-D solution's per-rank block back into a check by
    /// comparing each rank's block to the serial reference slice.
    fn check_matches_serial(nx: usize, ny: usize, px: i32, py: i32, shift: f64) {
        let tol = 1e-10;
        // Global RHS (row-major), some smooth-ish pattern.
        let b_global: Vec<f64> = (0..nx * ny)
            .map(|k| {
                let ix = k % nx;
                let iy = k / nx;
                ((ix as f64) * 0.2).sin() + ((iy as f64) * 0.3).cos() + 1.0
            })
            .collect();
        let (reference, _) = serial_poisson_2d_cg(nx, ny, &b_global, shift, tol, 5000);

        let nprocs = px * py;
        let ok = run(nprocs, move |c| {
            let cart = c.cart_create(&[px, py], &[false, false]).unwrap().unwrap();
            let d = Decomposition2D::new(nx, ny, cart);
            // Extract this block's RHS from the global vector.
            let mut b_local = vec![0.0; d.local_len()];
            for iy in 0..d.ly {
                for ix in 0..d.lx {
                    let gx = d.x0 + ix;
                    let gy = d.y0 + iy;
                    b_local[iy * d.lx + ix] = b_global[gy * nx + gx];
                }
            }
            let (x_local, _) = distributed_cg_2d(&d, &b_local, shift, tol, 5000).unwrap();
            // Compare block to the serial reference.
            let mut good = true;
            for iy in 0..d.ly {
                for ix in 0..d.lx {
                    let gx = d.x0 + ix;
                    let gy = d.y0 + iy;
                    if (x_local[iy * d.lx + ix] - reference[gy * nx + gx]).abs() > 1e-7 {
                        good = false;
                    }
                }
            }
            good
        })
        .unwrap();
        assert!(ok.iter().all(|&b| b), "2-D distributed != serial for {px}x{py}");
    }

    #[test]
    fn distributed_2d_cg_matches_serial_on_various_process_grids() {
        // Same 12x8 global grid, different process-grid shapes.
        check_matches_serial(12, 8, 1, 1, 0.2);
        check_matches_serial(12, 8, 2, 2, 0.2);
        check_matches_serial(12, 8, 4, 2, 0.2);
        check_matches_serial(12, 8, 3, 1, 0.2);
        check_matches_serial(12, 8, 1, 4, 0.2);
    }

    #[test]
    fn block_partition_tiles_the_grid_without_gaps() {
        // 2x2 process grid over 5x3 cells: blocks must cover every global cell once.
        let cover = run(4, |c| {
            let cart = c.cart_create(&[2, 2], &[false, false]).unwrap().unwrap();
            let d = Decomposition2D::new(5, 3, cart);
            (d.x0, d.y0, d.lx, d.ly, d.cart.my_coords())
        })
        .unwrap();
        // Total cells across blocks == 15.
        let total: usize = cover.iter().map(|(_, _, lx, ly, _)| lx * ly).sum();
        assert_eq!(total, 15);
        // x split of 5 over 2 -> 3,2 ; y split of 3 over 2 -> 2,1.
        for (x0, y0, lx, ly, coords) in &cover {
            let (cx, cy) = (coords[0], coords[1]);
            assert_eq!(*lx, if cx == 0 { 3 } else { 2 });
            assert_eq!(*ly, if cy == 0 { 2 } else { 1 });
            assert_eq!(*x0, if cx == 0 { 0 } else { 3 });
            assert_eq!(*y0, if cy == 0 { 0 } else { 2 });
        }
    }

    #[test]
    fn distributed_2d_residual_is_near_zero() {
        let (nx, ny, shift) = (10, 6, 0.5);
        let b: Vec<f64> = (0..nx * ny).map(|k| (k % 7) as f64).collect();
        let res = run(4, move |c| {
            let cart = c.cart_create(&[2, 2], &[false, false]).unwrap().unwrap();
            let d = Decomposition2D::new(nx, ny, cart);
            let mut b_local = vec![0.0; d.local_len()];
            for iy in 0..d.ly {
                for ix in 0..d.lx {
                    b_local[iy * d.lx + ix] = b[(d.y0 + iy) * nx + (d.x0 + ix)];
                }
            }
            let (x, _) = distributed_cg_2d(&d, &b_local, shift, 1e-12, 5000).unwrap();
            let ax = poisson_matvec_2d(&d, &x, shift).unwrap();
            b_local
                .iter()
                .zip(&ax)
                .map(|(bi, axi)| (bi - axi).abs())
                .fold(0.0_f64, f64::max)
        })
        .unwrap();
        for r in res {
            assert!(r < 1e-8, "2-D residual too large: {r:e}");
        }
    }
}
