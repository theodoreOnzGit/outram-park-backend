//! # Cyclic (periodic) patch support for `snappyHexMesh`
//!
//! A cyclic patch pair is *conformal*: local face `i` of half A couples to local
//! face `i` of half B, and the two halves are related by a rigid **translation**
//! — the separation vector `t`. `FvMeshBuilder` enforces the bookkeeping half of
//! that contract (mutual partners, equal `size`; see
//! [`BoundaryPatch::new_cyclic`]) but performs **no geometric check**, so a mesh
//! whose halves have drifted out of alignment still builds. This module supplies
//! the missing geometric half:
//!
//! - [`check_conformity`] — the **V&V gate**. Asserts that every local face pair
//!   has the same vertex count and that its face centres differ by one common
//!   separation vector, to a tolerance. This is what makes a cyclic BC valid.
//! - [`CyclicPointConstraints`] — the machinery that lets the snapping phase
//!   *move* points that lie on a cyclic plane without breaking the above.
//!
//! ## Why snapping needs this
//!
//! `snappyHexMesh`'s snapping phase projects wall-patch points onto the STL.
//! Before this module, any wall point that also lay on a non-wall boundary face
//! was **frozen** ([`super::snapping`]'s `frozen_patch_points`), cyclic planes
//! included. That is conservative and conformity-safe — a point that never moves
//! cannot break the pairing — but it means the geometry is **never body-fitted
//! where it crosses a periodic plane**. For a fuel-subchannel mesh, whose rods
//! and spacer grid are cut by the periodic planes, the rod walls stay at their
//! staircase (castellated) position exactly on the seams.
//!
//! The fix is the constraint OpenFOAM applies to coupled patches
//! (`syncTools::syncPointList` + the patch's own `pointConstraint`): a point on a
//! cyclic plane may move **within that plane**, and its displacement is
//! **synchronised with its partner point** so both halves move identically.
//! Because the separation vector is then unchanged, conformity is preserved
//! exactly while the in-plane geometry is properly snapped.
//!
//! Formally, for a partner pair `(p, p + t)` with plane normal `n̂ = t̂`:
//!
//! 1. **Project into the plane** — `d ← d − (d · n̂) n̂`, so the periodic plane
//!    stays flat (it is a domain boundary).
//! 2. **Synchronise** — `d_A = d_B = ½(d_A + d_B)`, so `(p_B + d_B) − (p_A + d_A)`
//!    remains exactly `t`.
//!
//! Step 2 alone is sufficient for conformity; step 1 additionally keeps the
//! periodic planes planar, which is what a translationally-periodic domain wants.
//!
//! ## Scope and limits
//!
//! **Translational cyclics only.** Rotational (`cyclicPolyPatch` with a rotation
//! transform) and non-conformal `cyclicAMI` pairs are out of scope here and are
//! rejected rather than silently mishandled — see [`CyclicError`]. Extending to
//! rotational periodicity means replacing the constant `t` with a rotation about
//! an axis in both the pairing and the sync step.
//!
//! All lengths are metres.

use std::collections::HashMap;

use outram_foam_basic_lib::mesh::PatchKind;
use outram_foam_basic_lib::primitives::Vector3;

use super::poly_topology::PolyPatchMesh;

/// Default geometric tolerance `[m]` for matching cyclic faces and points.
///
/// Chosen well below any realistic cell size so a genuine mismatch is caught,
/// while absorbing the round-off of centroid arithmetic.
pub const DEFAULT_CYCLIC_TOL: f64 = 1e-9;

/// Why a cyclic pairing or conformity check failed.
#[derive(Debug, Clone, PartialEq)]
pub enum CyclicError {
    /// A patch marked [`PatchKind::Cyclic`] has no resolved partner.
    UnresolvedPartner {
        /// Index of the offending patch.
        patch: usize,
        /// Its name.
        name: String,
    },
    /// The two halves disagree on who their partner is.
    AsymmetricPartner {
        /// Index of the patch whose partner does not name it back.
        patch: usize,
        /// The partner it names.
        partner: usize,
    },
    /// The two halves have different face counts, so face `i` ↔ face `i` cannot
    /// hold.
    SizeMismatch {
        /// Index of the first half.
        patch: usize,
        /// Its face count.
        size: usize,
        /// Index of the partner half.
        partner: usize,
        /// The partner's face count.
        partner_size: usize,
    },
    /// A local face pair has differing vertex counts.
    FaceShapeMismatch {
        /// Local face index within the patch.
        local: usize,
        /// Vertex count on the first half.
        n_a: usize,
        /// Vertex count on the partner half.
        n_b: usize,
    },
    /// A local face pair's centres are not related by the pair's separation
    /// vector — the halves have drifted out of alignment.
    SeparationMismatch {
        /// Local face index within the patch.
        local: usize,
        /// The separation this pair exhibits `[m]`.
        found: [f64; 3],
        /// The separation the pair is supposed to have `[m]`.
        expected: [f64; 3],
        /// Magnitude of the discrepancy `[m]`.
        error: f64,
    },
    /// A rotational or otherwise non-translational cyclic was encountered.
    NotTranslational {
        /// Index of the offending patch.
        patch: usize,
    },
}

impl std::fmt::Display for CyclicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnresolvedPartner { patch, name } => write!(
                f,
                "cyclic patch {patch} ('{name}') has no resolved partner (cyclic_partner is None)"
            ),
            Self::AsymmetricPartner { patch, partner } => write!(
                f,
                "cyclic patch {patch} names {partner} as partner, but {partner} does not name it back"
            ),
            Self::SizeMismatch {
                patch,
                size,
                partner,
                partner_size,
            } => write!(
                f,
                "cyclic halves disagree in size: patch {patch} has {size} faces, partner {partner} has {partner_size}"
            ),
            Self::FaceShapeMismatch { local, n_a, n_b } => write!(
                f,
                "cyclic local face {local}: {n_a} vertices on one half, {n_b} on the other"
            ),
            Self::SeparationMismatch {
                local,
                found,
                expected,
                error,
            } => write!(
                f,
                "cyclic local face {local} is not conformal: separation {found:?} m differs from the pair's {expected:?} m by {error:e} m"
            ),
            Self::NotTranslational { patch } => write!(
                f,
                "cyclic patch {patch} is not a pure translation (rotational / AMI cyclics are not supported here)"
            ),
        }
    }
}

impl std::error::Error for CyclicError {}

/// Area-weighted-free centroid of a face — the plain mean of its vertices.
///
/// Sufficient for conformity checking: the two halves of a conformal pair have
/// congruent vertex loops, so their vertex means differ by exactly the
/// separation vector whenever the faces themselves do.
fn face_centroid(points: &[Vector3], face: &[usize]) -> Vector3 {
    if face.is_empty() {
        return Vector3::ZERO;
    }
    let mut acc = Vector3::ZERO;
    for &p in face {
        acc += points[p];
    }
    acc * (1.0 / face.len() as f64)
}

/// One resolved, translational cyclic patch pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CyclicPair {
    /// Patch index of the first half.
    pub patch_a: usize,
    /// Patch index of the partner half.
    pub patch_b: usize,
    /// Separation `t` `[m]` such that `centre_b = centre_a + t` for every local
    /// face pair.
    pub separation: Vector3,
}

impl CyclicPair {
    /// Unit normal of the cyclic planes — the normalised separation direction.
    #[must_use]
    pub fn plane_normal(&self) -> Vector3 {
        self.separation.normalise(1e-300)
    }
}

/// Resolve every cyclic pair in `topo`, verifying the bookkeeping contract.
///
/// Each pair is reported **once** (with `patch_a < patch_b`). The separation is
/// taken from local face `0` and then required of every other local face by
/// [`check_conformity`].
///
/// # Errors
///
/// [`CyclicError::UnresolvedPartner`], [`CyclicError::AsymmetricPartner`] or
/// [`CyclicError::SizeMismatch`] if the patch bookkeeping is inconsistent.
pub fn resolve_pairs(topo: &PolyPatchMesh) -> Result<Vec<CyclicPair>, CyclicError> {
    let mut pairs = Vec::new();
    for (pi, patch) in topo.patches.iter().enumerate() {
        if patch.kind == PatchKind::CyclicAmi {
            return Err(CyclicError::NotTranslational { patch: pi });
        }
        if patch.kind != PatchKind::Cyclic {
            continue;
        }
        let Some(pj) = patch.cyclic_partner else {
            return Err(CyclicError::UnresolvedPartner {
                patch: pi,
                name: patch.name.clone(),
            });
        };
        if pj <= pi {
            // Report each pair once, from the lower index. The reverse direction
            // is validated when the lower half is visited.
            continue;
        }
        let partner = &topo.patches[pj];
        if partner.cyclic_partner != Some(pi) {
            return Err(CyclicError::AsymmetricPartner {
                patch: pi,
                partner: pj,
            });
        }
        if partner.size != patch.size {
            return Err(CyclicError::SizeMismatch {
                patch: pi,
                size: patch.size,
                partner: pj,
                partner_size: partner.size,
            });
        }
        if patch.size == 0 {
            continue;
        }
        let ca = face_centroid(&topo.points, &topo.faces[patch.start]);
        let cb = face_centroid(&topo.points, &topo.faces[partner.start]);
        pairs.push(CyclicPair {
            patch_a: pi,
            patch_b: pj,
            separation: cb - ca,
        });
    }
    Ok(pairs)
}

/// **The V&V gate.** Verify every cyclic pair is geometrically conformal.
///
/// For each pair and each local face `i`, requires that half A's face `i` and
/// half B's face `i` have the same vertex count and that their centres differ by
/// the pair's separation vector to within `tol` metres.
///
/// Passing this is what makes a `cyclic` boundary condition valid on the mesh:
/// the solver couples face `i` to face `i` and assumes they are the same patch
/// of geometry displaced by `t`.
///
/// # Errors
///
/// Any [`CyclicError`]; in particular [`CyclicError::SeparationMismatch`] naming
/// the first local face that has drifted, and by how much.
pub fn check_conformity(topo: &PolyPatchMesh, tol: f64) -> Result<(), CyclicError> {
    let pairs = resolve_pairs(topo)?;
    for pair in &pairs {
        let a = &topo.patches[pair.patch_a];
        let b = &topo.patches[pair.patch_b];
        for local in 0..a.size {
            let fa = &topo.faces[a.start + local];
            let fb = &topo.faces[b.start + local];
            if fa.len() != fb.len() {
                return Err(CyclicError::FaceShapeMismatch {
                    local,
                    n_a: fa.len(),
                    n_b: fb.len(),
                });
            }
            let ca = face_centroid(&topo.points, fa);
            let cb = face_centroid(&topo.points, fb);
            let found = cb - ca;
            let delta = found - pair.separation;
            let err = delta.dot(delta).sqrt();
            if err > tol {
                return Err(CyclicError::SeparationMismatch {
                    local,
                    found: [found.x, found.y, found.z],
                    expected: [
                        pair.separation.x,
                        pair.separation.y,
                        pair.separation.z,
                    ],
                    error: err,
                });
            }
        }
    }
    Ok(())
}

/// Per-point cyclic constraints for the snapping phase.
///
/// Built once from the castellated topology; consumed every snapping iteration
/// by [`Self::constrain_and_sync`].
#[derive(Debug, Clone, Default)]
pub struct CyclicPointConstraints {
    /// Global point id → unit normal of the cyclic plane it lies on. A point on
    /// two cyclic planes at once (a periodic *edge*, e.g. the corner column of a
    /// doubly-periodic subchannel) appears here once per plane via
    /// [`Self::extra_normals`].
    normal_of: HashMap<usize, Vector3>,
    /// Second plane normal for points lying on two cyclic planes.
    extra_normals: HashMap<usize, Vector3>,
    /// Global point id → its partner point across the seam, for each pairing the
    /// point participates in.
    partners: Vec<(usize, usize)>,
}

impl CyclicPointConstraints {
    /// Is this point constrained by any cyclic plane?
    #[must_use]
    pub fn is_constrained(&self, point: usize) -> bool {
        self.normal_of.contains_key(&point)
    }

    /// Number of partner point pairings resolved.
    #[must_use]
    pub fn n_pairings(&self) -> usize {
        self.partners.len()
    }

    /// Build the constraints from a topology's cyclic patches.
    ///
    /// Points are paired by matching `p_a + t` against the partner half's points
    /// within `tol` metres. A point whose partner cannot be located is simply not
    /// paired — the caller must keep such a point frozen, which
    /// [`super::snapping`] does.
    ///
    /// # Errors
    ///
    /// Propagates [`resolve_pairs`]'s bookkeeping errors.
    pub fn build(topo: &PolyPatchMesh, tol: f64) -> Result<Self, CyclicError> {
        let pairs = resolve_pairs(topo)?;
        let mut out = Self::default();

        for pair in &pairs {
            let n = pair.plane_normal();
            let a = &topo.patches[pair.patch_a];
            let b = &topo.patches[pair.patch_b];

            // Collect the point sets of both halves.
            let mut pts_a: Vec<usize> = Vec::new();
            for f in a.start..a.end() {
                pts_a.extend_from_slice(&topo.faces[f]);
            }
            pts_a.sort_unstable();
            pts_a.dedup();
            let mut pts_b: Vec<usize> = Vec::new();
            for f in b.start..b.end() {
                pts_b.extend_from_slice(&topo.faces[f]);
            }
            pts_b.sort_unstable();
            pts_b.dedup();

            // Spatial lookup for half B, keyed on a quantised position so the
            // match is O(1) rather than O(n²).
            let key = |v: Vector3| -> (i64, i64, i64) {
                let q = 1.0 / tol.max(1e-300);
                (
                    (v.x * q).round() as i64,
                    (v.y * q).round() as i64,
                    (v.z * q).round() as i64,
                )
            };
            let mut index_b: HashMap<(i64, i64, i64), usize> = HashMap::with_capacity(pts_b.len());
            for &pb in &pts_b {
                index_b.insert(key(topo.points[pb]), pb);
            }

            for &pa in &pts_a {
                let want = topo.points[pa] + pair.separation;
                // Probe the quantisation cell and its immediate neighbours so a
                // coordinate sitting exactly on a bucket edge still matches.
                let (kx, ky, kz) = key(want);
                let mut found = None;
                'probe: for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            if let Some(&pb) = index_b.get(&(kx + dx, ky + dy, kz + dz)) {
                                let d = topo.points[pb] - want;
                                if d.dot(d).sqrt() <= tol {
                                    found = Some(pb);
                                    break 'probe;
                                }
                            }
                        }
                    }
                }
                let Some(pb) = found else { continue };

                for p in [pa, pb] {
                    if let Some(existing) = out.normal_of.get(&p).copied() {
                        // Already on one cyclic plane: record the second normal
                        // unless it is the same plane again.
                        if existing.cross(n).dot(existing.cross(n)).sqrt() > 1e-12 {
                            out.extra_normals.insert(p, n);
                        }
                    } else {
                        out.normal_of.insert(p, n);
                    }
                }
                out.partners.push((pa, pb));
            }
        }
        Ok(out)
    }

    /// Project a displacement into the cyclic plane(s) constraining `point`.
    fn project(&self, point: usize, d: Vector3) -> Vector3 {
        let mut d = d;
        if let Some(&n) = self.normal_of.get(&point) {
            d = d - n * d.dot(n);
        }
        if let Some(&n2) = self.extra_normals.get(&point) {
            d = d - n2 * d.dot(n2);
        }
        d
    }

    /// Apply the cyclic constraint to a displacement field, in place.
    ///
    /// `disp` is indexed by *local* patch-point index; `local_of` maps a global
    /// point id to that index. Points not present in `local_of` are skipped
    /// (they are not being snapped).
    ///
    /// Two passes, matching the module docs:
    ///
    /// 1. every constrained point's displacement is projected into its cyclic
    ///    plane(s), so the periodic planes stay flat;
    /// 2. every partner pair is averaged, so both halves move identically and
    ///    the separation vector — and therefore conformity — is preserved
    ///    exactly.
    pub fn constrain_and_sync(&self, disp: &mut [Vector3], local_of: &HashMap<usize, usize>) {
        // 1. In-plane projection.
        for (&gp, _) in self.normal_of.iter() {
            if let Some(&l) = local_of.get(&gp) {
                if l < disp.len() {
                    disp[l] = self.project(gp, disp[l]);
                }
            }
        }
        // 2. Partner synchronisation.
        for &(pa, pb) in &self.partners {
            match (local_of.get(&pa), local_of.get(&pb)) {
                (Some(&la), Some(&lb)) if la < disp.len() && lb < disp.len() => {
                    let mean = (disp[la] + disp[lb]) * 0.5;
                    disp[la] = mean;
                    disp[lb] = mean;
                }
                // Only one side is being snapped: moving it alone would break
                // the pairing, so neither moves.
                (Some(&la), None) if la < disp.len() => disp[la] = Vector3::ZERO,
                (None, Some(&lb)) if lb < disp.len() => disp[lb] = Vector3::ZERO,
                _ => {}
            }
        }
    }
}
