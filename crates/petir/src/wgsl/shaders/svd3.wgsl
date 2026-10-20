// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// TRANSCRIBED to WGSL (f32) from PETIR's port of faer 0.24.4's dense SVD,
// `src/faer_dense/svd.rs` and `src/faer_dense/kernels.rs` (faer: MIT,
// Copyright (c) sarah quiñones and the faer contributors; full notice in
// `src/faer_dense/svd.rs`). Upstream items: `svd_imp`, `bidiag_in_place`,
// `bidiag_fused_op_simd`, `qr_algorithm`, the block-size-1 Householder
// application, `make_householder`, `make_givens`, `hypot`, `norm_l2_*`.
//
// Specialised to n = 3. The CPU mirror is `src/wgsl/mirror_svd3.rs`, line for
// line; read its module docs for the three deliberate differences from the
// f64 port (scalar left-to-right reductions in place of faer's SIMD lanes,
// `1 / tau` stored in place of an infinite `tau`, f32 eps / iteration cap).
//
// LAYOUT. `petir_svd3` and `petir_closest_rotation3` take and return 3 x 3
// matrices ROW-MAJOR in `array<f32, 9>` (`m[3*i + j]` = entry (i, j)), as
// `faer_dense::svd3` takes `m[i][j]`. Internally the working matrix is
// column-major (`a[3*j + i]`), as faer's is.

const PETIR_SVD3_MIN_POSITIVE: f32 = 0x1p-126f;
const PETIR_SVD3_MAX_POSITIVE: f32 = 0x1p126f;
const PETIR_SVD3_SQRT_MIN: f32 = 0x1p-63f;
const PETIR_SVD3_SQRT_MAX: f32 = 0x1p63f;
const PETIR_SVD3_EPS: f32 = 0x1p-23f;

struct PetirSvd3 {
    u: array<f32, 9>,
    s: vec3<f32>,
    v: array<f32, 9>,
    ok: bool,
};

struct PetirSvd3Householder {
    head: f32,
    tau_inv: f32,
    beta_inv: f32,
    infinite: bool,
};

fn petir_svd3_is_finite(x: f32) -> bool {
    return (bitcast<u32>(x) & 0x7f800000u) != 0x7f800000u;
}

fn petir_svd3_hypot(re: f32, im: f32) -> f32 {
    let re_abs = abs(re);
    let im_abs = abs(im);
    if (re_abs > PETIR_SVD3_SQRT_MAX || im_abs > PETIR_SVD3_SQRT_MAX) {
        let a = re * PETIR_SVD3_SQRT_MIN;
        let b = im * PETIR_SVD3_SQRT_MIN;
        return sqrt(a * a + b * b) * PETIR_SVD3_SQRT_MAX;
    } else if (re_abs > 1.0 || im_abs > 1.0) {
        return sqrt(re * re + im * im);
    }
    let a = re * PETIR_SVD3_SQRT_MAX;
    let b = im * PETIR_SVD3_SQRT_MAX;
    return sqrt(a * a + b * b) * PETIR_SVD3_SQRT_MIN;
}

fn petir_svd3_norm_select(acc_sml: f32, acc_med: f32, acc_big: f32) -> f32 {
    if (acc_sml >= 1.0) {
        return sqrt(acc_sml) * PETIR_SVD3_SQRT_MAX;
    } else if (acc_med >= 1.0) {
        return sqrt(acc_med);
    }
    return sqrt(acc_big) * PETIR_SVD3_SQRT_MIN;
}

fn petir_svd3_norm_contiguous(x: array<f32, 3>, len: u32) -> f32 {
    var xs = x;
    var acc_sml = 0.0;
    var acc_med = 0.0;
    var acc_big = 0.0;
    for (var q = 0u; q < len; q++) {
        let v = xs[q];
        let vs = v * PETIR_SVD3_SQRT_MIN;
        acc_sml = vs * vs + acc_sml;
        acc_med = v * v + acc_med;
        let vb = v * PETIR_SVD3_SQRT_MAX;
        acc_big = vb * vb + acc_big;
    }
    return petir_svd3_norm_select(acc_sml, acc_med, acc_big);
}

fn petir_svd3_norm_strided(x: array<f32, 3>, len: u32) -> f32 {
    if (len == 0u) {
        return petir_svd3_norm_select(0.0, 0.0, 0.0);
    }
    var xs = x;
    var acc = 0.0;
    for (var q = 0u; q < len; q++) {
        acc = petir_svd3_hypot(acc, abs(xs[q]));
    }
    acc = acc * acc;
    return petir_svd3_norm_select(
        PETIR_SVD3_MIN_POSITIVE * acc, acc, PETIR_SVD3_MAX_POSITIVE * acc);
}

// faer's make_householder, minus the tail scaling: the caller multiplies the
// tail by `beta_inv` unless `infinite`.
fn petir_svd3_householder(head_in: f32, tail_norm: f32) -> PetirSvd3Householder {
    var head = head_in;
    var head_norm = abs(head);
    if (head_norm < PETIR_SVD3_MIN_POSITIVE) {
        head = 0.0;
        head_norm = 0.0;
    }
    if (tail_norm < PETIR_SVD3_MIN_POSITIVE) {
        return PetirSvd3Householder(head, 0.0, 0.0, true);
    }
    let norm = petir_svd3_hypot(head_norm, tail_norm);
    var sign = 1.0;
    if (head_norm != 0.0) {
        sign = head * (1.0 / head_norm);
    }
    let signed_norm = sign * norm;
    let head_with_beta = head + signed_norm;
    let head_with_beta_inv = 1.0 / head_with_beta;
    let t = tail_norm * abs(head_with_beta_inv);
    let tau = 0.5 * (1.0 + t * t);
    return PetirSvd3Householder(-signed_norm, 1.0 / tau, head_with_beta_inv, false);
}

// faer's make_givens: (c, s).
fn petir_svd3_givens(p: f32, q: f32) -> vec2<f32> {
    if (q == 0.0) {
        if (p < 0.0) {
            return vec2<f32>(-1.0, 0.0);
        }
        return vec2<f32>(1.0, 0.0);
    } else if (p == 0.0) {
        if (q < 0.0) {
            return vec2<f32>(0.0, 1.0);
        }
        return vec2<f32>(0.0, -1.0);
    } else if (abs(p) > abs(q)) {
        let t = q / p;
        var u = petir_svd3_hypot(t, 1.0);
        if (p < 0.0) {
            u = -u;
        }
        let c = 1.0 / u;
        return vec2<f32>(c, -t * c);
    }
    let t = p / q;
    var u = petir_svd3_hypot(t, 1.0);
    if (q < 0.0) {
        u = -u;
    }
    let s = -(1.0 / u);
    return vec2<f32>(-t * s, s);
}

// faer's rotate_left on columns `ca`, `cb` of a column-major 3 x 3.
fn petir_svd3_rotate_cols(m: array<f32, 9>, ca: u32, cb: u32, c: f32, s: f32) -> array<f32, 9> {
    var r = m;
    if (c == 1.0 && s == 0.0) {
        return r;
    }
    for (var row = 0u; row < 3u; row++) {
        let xx = r[3u * ca + row];
        let yy = r[3u * cb + row];
        r[3u * ca + row] = (-s) * yy + xx * c;
        r[3u * cb + row] = s * xx + yy * c;
    }
    return r;
}

// faer's apply_householder_left: essential part `ess[0..len]`, `1 / tau`,
// head row `r0`, every column of the column-major `t`.
fn petir_svd3_reflect(ess: array<f32, 3>, len: u32, tau_inv: f32, t: array<f32, 9>, r0: u32) -> array<f32, 9> {
    var e = ess;
    var r = t;
    for (var c = 0u; c < 3u; c++) {
        var dot = 0.0;
        for (var q = 0u; q < len; q++) {
            dot = e[q] * r[3u * c + r0 + 1u + q] + dot;
        }
        dot = r[3u * c + r0] + dot;
        let k = -dot * tau_inv;
        r[3u * c + r0] += k;
        for (var q = 0u; q < len; q++) {
            let at = 3u * c + r0 + 1u + q;
            r[at] = e[q] * k + r[at];
        }
    }
    return r;
}

// faer's SVD of the row-major `m`: U diag(s) V^T, s non-increasing.
// `ok` is false where the f64 port errors.
fn petir_svd3(m: array<f32, 9>) -> PetirSvd3 {
    var mm = m;
    var a: array<f32, 9>;
    for (var i = 0u; i < 3u; i++) {
        for (var j = 0u; j < 3u; j++) {
            a[3u * j + i] = mm[3u * i + j];
        }
    }

    // ---- bidiag_in_place (block size 1) ----
    var y = array<f32, 3>(0.0, 0.0, 0.0);
    var z = array<f32, 3>(0.0, 0.0, 0.0);
    var hl_inv = array<f32, 3>(0.0, 0.0, 0.0);
    var hr_inv = array<f32, 2>(0.0, 0.0);
    for (var k = 0u; k < 3u; k++) {
        let y1 = y[k];
        let z1 = z[k];
        if (k >= 1u) {
            let k1 = k - 1u;
            let up0 = a[3u * k1 + k];
            a[3u * k + k] -= up0 * y1 + z1;
            for (var i = k + 1u; i < 3u; i++) {
                a[3u * k + i] -= a[3u * k1 + i] * y1 + z[i];
            }
            for (var j = k + 1u; j < 3u; j++) {
                a[3u * j + k] -= up0 * y[j] + z1 * a[3u * j + k1];
            }
        }

        // Left reflector on column k, tail rows k+1..
        let len = 2u - k;
        var tail = array<f32, 3>(0.0, 0.0, 0.0);
        for (var q = 0u; q < len; q++) {
            tail[q] = a[3u * k + k + 1u + q];
        }
        let tail_norm = petir_svd3_norm_contiguous(tail, len);
        let hl = petir_svd3_householder(a[3u * k + k], tail_norm);
        if (!hl.infinite) {
            for (var q = 0u; q < len; q++) {
                tail[q] *= hl.beta_inv;
            }
        }
        a[3u * k + k] = hl.head;
        for (var q = 0u; q < len; q++) {
            a[3u * k + k + 1u + q] = tail[q];
        }
        hl_inv[k] = hl.tau_inv;
        let tl_inv = hl.tau_inv;

        if (k >= 1u) {
            let k1 = k - 1u;
            for (var j = k + 1u; j < 3u; j++) {
                let yj = -y[j];
                let vj = -a[3u * j + k1];
                var acc = 0.0;
                for (var i = k + 1u; i < 3u; i++) {
                    var v = a[3u * j + i];
                    v = a[3u * k1 + i] * yj + v;
                    v = z[i] * vj + v;
                    a[3u * j + i] = v;
                    acc = a[3u * k + i] * v + acc;
                }
                y[j] = acc;
            }
        } else {
            for (var j = k + 1u; j < 3u; j++) {
                var d = 0.0;
                for (var i = k + 1u; i < 3u; i++) {
                    d = a[3u * j + i] * a[3u * k + i] + d;
                }
                y[j] = 1.0 * d;
            }
        }

        // y2 = (y2 + A12) / tl; A12 -= y2.
        var a12 = array<f32, 3>(0.0, 0.0, 0.0);
        let len12 = 2u - k;
        for (var q = 0u; q < len12; q++) {
            let j = k + 1u + q;
            y[j] = (y[j] + a[3u * j + k]) * tl_inv;
            a12[q] = a[3u * j + k] - y[j];
        }
        let norm = petir_svd3_norm_strided(a12, len12);
        if (norm != 0.0) {
            let norm_inv = 1.0 / norm;
            for (var q = 0u; q < len12; q++) {
                a12[q] *= norm_inv;
            }
        }
        for (var q = 0u; q < len12; q++) {
            a[3u * (k + 1u + q) + k] = a12[q];
        }

        // z2 = A22 A12^H.
        for (var i = k + 1u; i < 3u; i++) {
            z[i] = 0.0;
        }
        for (var q = 0u; q < len12; q++) {
            let r = a12[q] * 1.0;
            for (var i = k + 1u; i < 3u; i++) {
                z[i] = a[3u * (k + 1u + q) + i] * r + z[i];
            }
        }

        if (k + 1u == 3u) {
            break;
        }

        // Right reflector on A12.
        var a12_b = array<f32, 3>(0.0, 0.0, 0.0);
        let lenb = len12 - 1u;
        for (var q = 0u; q < lenb; q++) {
            a12_b[q] = a12[1u + q];
        }
        let tnorm = petir_svd3_norm_strided(a12_b, lenb);
        let hr = petir_svd3_householder(a12[0], tnorm);
        if (!hr.infinite) {
            for (var q = 0u; q < lenb; q++) {
                a12_b[q] *= hr.beta_inv;
            }
        }
        hr_inv[k] = hr.tau_inv;
        let tr_inv = hr.tau_inv;
        let beta = hr.head;
        a[3u * (k + 1u) + k] = hr.head * norm;
        for (var q = 0u; q < lenb; q++) {
            a[3u * (k + 2u + q) + k] = a12_b[q];
        }

        var sb = 0.0;
        for (var q = 0u; q < lenb; q++) {
            sb = y[k + 2u + q] * a12_b[q] + sb;
        }
        let b = y[k + 1u] + sb;

        for (var i = k + 1u; i < 3u; i++) {
            let ui = a[3u * k + i];
            let ai = a[3u * (k + 1u) + i];
            if (!hr.infinite) {
                var w = z[i] - ai * beta;
                w = w * hr.beta_inv;
                w = w - ui * b;
                z[i] = w * tr_inv;
            } else {
                let w = ai - ui * b;
                z[i] = w * tr_inv;
            }
        }
    }

    // ---- svd_imp: read off the bidiagonal ----
    var diag = array<f32, 3>(a[0], a[4], a[8]);
    var sub = array<f32, 3>(a[3], a[7], 0.0);
    var out: PetirSvd3;
    out.ok = false;
    for (var i = 0u; i < 3u; i++) {
        if (!(petir_svd3_is_finite(diag[i]) && petir_svd3_is_finite(sub[i]))) {
            return out;
        }
    }
    var ub = array<f32, 9>(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    var vb = ub;

    // ---- qr_algorithm ----
    var mx = 0.0;
    for (var i = 0u; i < 3u; i++) {
        if (abs(diag[i]) > mx) {
            mx = abs(diag[i]);
        }
    }
    var mx_sub = 0.0;
    for (var i = 0u; i < 3u; i++) {
        if (abs(sub[i]) > mx_sub) {
            mx_sub = abs(sub[i]);
        }
    }
    if (mx_sub > mx) {
        mx = mx_sub;
    }
    if (mx != 0.0) {
        let mx_inv = 1.0 / mx;
        for (var i = 0u; i < 3u; i++) {
            diag[i] *= mx_inv;
            sub[i] *= mx_inv;
        }
        let eps2 = PETIR_SVD3_EPS * PETIR_SVD3_EPS;
        let sml = PETIR_SVD3_MIN_POSITIVE;
        var converged = false;
        for (var iter = 0u; iter < 270u; iter++) {
            for (var i0 = 0u; i0 < 2u; i0++) {
                let e = sub[i0];
                if (e * e <= eps2 * abs(diag[i0] * diag[i0 + 1u]) + sml) {
                    sub[i0] = 0.0;
                }
            }
            var end = 3u;
            while (end >= 2u && sub[end - 2u] * sub[end - 2u] <= sml) {
                end -= 1u;
            }
            if (end == 1u) {
                converged = true;
                break;
            }
            var start = end - 1u;
            while (start >= 1u && sub[start - 1u] != 0.0) {
                start -= 1u;
            }
            var found_zero_diag = false;
            for (var i = start; i < end - 1u; i++) {
                if (abs(diag[i]) <= PETIR_SVD3_EPS) {
                    found_zero_diag = true;
                    var val = sub[i];
                    sub[i] = 0.0;
                    for (var j = i + 1u; j < end; j++) {
                        let g = petir_svd3_givens(diag[j], val);
                        diag[j] = g.x * diag[j] - g.y * val;
                        if (j + 1u < end) {
                            val = g.y * sub[j];
                            sub[j] = g.x * sub[j];
                        }
                        vb = petir_svd3_rotate_cols(vb, j, i, g.x, g.y);
                    }
                }
            }
            if (found_zero_diag) {
                if (iter + 1u == 270u) {
                    return out;
                }
                continue;
            }
            let end2 = end - 2u;
            let end1 = end - 1u;
            let d2 = diag[end2];
            var t00 = d2 * d2;
            if (end - start != 2u) {
                let e3 = sub[end - 3u];
                t00 = d2 * d2 + e3 * e3;
            }
            let d1 = diag[end1];
            let e2 = sub[end2];
            let t11 = d1 * d1 + e2 * e2;
            let t01 = d2 * e2;
            let t01_2 = t01 * t01;
            var mu = t11;
            if (t01_2 > sml) {
                let d = (t00 - t11) * 0.5;
                var delta = sqrt(d * d + t01_2);
                if (d < 0.0) {
                    delta = -delta;
                }
                mu = t11 - t01_2 / (d + delta);
            }
            let ds = diag[start];
            var yy = ds * ds - mu;
            var zz = ds * sub[start];
            for (var k = start; k < end1; k++) {
                let g = petir_svd3_givens(yy, zz);
                if (k > start) {
                    sub[k - 1u] = abs(g.x * yy - g.y * zz);
                }
                let ek = sub[k];
                let diag_k = g.x * diag[k] - g.y * ek;
                sub[k] = g.y * diag[k] + g.x * ek;
                let k1 = k + 1u;
                yy = diag_k;
                let dk1 = diag[k1];
                zz = -g.y * dk1;
                diag[k1] = g.x * dk1;
                ub = petir_svd3_rotate_cols(ub, k1, k, g.x, -g.y);
                let h = petir_svd3_givens(yy, zz);
                diag[k] = h.x * yy - h.y * zz;
                let ek2 = sub[k];
                let dk1b = diag[k1];
                sub[k] = h.x * ek2 - h.y * dk1b;
                diag[k1] = h.y * ek2 + h.x * dk1b;
                if (k < end - 2u) {
                    yy = sub[k];
                    let ek1 = sub[k1];
                    zz = -h.y * ek1;
                    sub[k1] = h.x * ek1;
                }
                vb = petir_svd3_rotate_cols(vb, k1, k, h.x, -h.y);
            }
            if (iter + 1u == 270u) {
                return out;
            }
        }
        if (!converged) {
            return out;
        }
        for (var j = 0u; j < 3u; j++) {
            if (diag[j] < 0.0) {
                diag[j] = -diag[j];
                for (var r = 0u; r < 3u; r++) {
                    vb[3u * j + r] = -vb[3u * j + r];
                }
            }
        }
        for (var k = 0u; k < 3u; k++) {
            var best = 0.0;
            var idx = k;
            for (var kk = k; kk < 3u; kk++) {
                if (diag[kk] > best) {
                    best = diag[kk];
                    idx = kk;
                }
            }
            if (k != idx) {
                let tmp = diag[k];
                diag[k] = diag[idx];
                diag[idx] = tmp;
                for (var r = 0u; r < 3u; r++) {
                    let tu = ub[3u * k + r];
                    ub[3u * k + r] = ub[3u * idx + r];
                    ub[3u * idx + r] = tu;
                    let tv = vb[3u * k + r];
                    vb[3u * k + r] = vb[3u * idx + r];
                    vb[3u * idx + r] = tv;
                }
            }
        }
        for (var i = 0u; i < 3u; i++) {
            diag[i] *= mx;
        }
    }

    // ---- back-transformation ----
    var u = vb;
    for (var jj = 0u; jj < 3u; jj++) {
        let j = 2u - jj;
        let len = 2u - j;
        if (len == 0u) {
            continue;
        }
        var ess = array<f32, 3>(0.0, 0.0, 0.0);
        for (var q = 0u; q < len; q++) {
            ess[q] = a[3u * j + j + 1u + q];
        }
        u = petir_svd3_reflect(ess, len, hl_inv[j], u, j);
    }
    for (var j = 1u; j < 3u; j++) {
        for (var i = 0u; i < j; i++) {
            a[3u * i + j] = a[3u * j + i];
        }
    }
    var v = ub;
    for (var jj = 0u; jj < 2u; jj++) {
        let j = 1u - jj;
        let len = 1u - j;
        if (len == 0u) {
            continue;
        }
        var ess = array<f32, 3>(0.0, 0.0, 0.0);
        for (var q = 0u; q < len; q++) {
            ess[q] = a[3u * j + 1u + j + 1u + q];
        }
        v = petir_svd3_reflect(ess, len, hr_inv[j], v, 1u + j);
    }

    for (var i = 0u; i < 3u; i++) {
        for (var j = 0u; j < 3u; j++) {
            out.u[3u * i + j] = u[3u * j + i];
            out.v[3u * i + j] = v[3u * j + i];
        }
    }
    out.s = vec3<f32>(diag[0], diag[1], diag[2]);
    out.ok = true;
    return out;
}

// R = A B^T, row-major.
fn petir_svd3_mul_abt(a: array<f32, 9>, b: array<f32, 9>) -> array<f32, 9> {
    var aa = a;
    var bb = b;
    var c: array<f32, 9>;
    for (var i = 0u; i < 3u; i++) {
        for (var j = 0u; j < 3u; j++) {
            c[3u * i + j] = aa[3u * i] * bb[3u * j] + aa[3u * i + 1u] * bb[3u * j + 1u]
                + aa[3u * i + 2u] * bb[3u * j + 2u];
        }
    }
    return c;
}

// The closest proper rotation to the row-major `m` (Kabsch / Procrustes):
// R = U V^T, negating U's last column when det(U V^T) < 0; the identity when
// the SVD fails (outram-blender's `arap::closest_rotation` contract).
fn petir_closest_rotation3(m: array<f32, 9>) -> array<f32, 9> {
    var r = petir_svd3(m);
    if (!r.ok) {
        return array<f32, 9>(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    }
    var q = petir_svd3_mul_abt(r.u, r.v);
    let det = q[0] * (q[4] * q[8] - q[5] * q[7])
        - q[1] * (q[3] * q[8] - q[5] * q[6])
        + q[2] * (q[3] * q[7] - q[4] * q[6]);
    if (det < 0.0) {
        r.u[2] = -r.u[2];
        r.u[5] = -r.u[5];
        r.u[8] = -r.u[8];
    }
    return petir_svd3_mul_abt(r.u, r.v);
}

// Entry `e` (row-major) of the closest rotation of the row-major matrix at
// `src[off .. off + 9]`. Reads the `src` binding, as `matrix.wgsl` does.
fn petir_closest_rotation3_src(off: u32, e: u32) -> f32 {
    var m: array<f32, 9>;
    for (var q = 0u; q < 9u; q++) {
        m[q] = src[off + q];
    }
    var r = petir_closest_rotation3(m);
    return r[e];
}
