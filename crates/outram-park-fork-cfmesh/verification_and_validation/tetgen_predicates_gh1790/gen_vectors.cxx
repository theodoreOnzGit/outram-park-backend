// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// gh:#1790: generator of the differential vectors for the Rust port of TetGen
// 1.6.0's `predicates.cxx` (src/tetgen/predicates.rs).
//
// This is OUR harness. It compiles UPSTREAM `predicates.cxx` (libigl/tetgen,
// commit e05aca7df74e, TetGen 1.6.0; Shewchuk's public-domain predicates with
// Hang Si's modifications, Copyright (C) 2002-2020 Hang Si, WIAS Berlin,
// AGPL-3.0-or-later) into the same translation unit with `#include`, so it can
// read the file's `static` constants after `exactinit()`. Upstream is never
// modified; the stage-counting build includes a copy rewritten by
// `instrument.awk` (COUNT_STAGES), which only adds counters.
//
// Usage: gen_vectors <out.bin>   (see run.sh and README.md)
//
// File format (little-endian):
//   "TGPRED01"
//   u32 nconfig; per config: u8 noexact, u8 nofilter, f64 maxx, maxy, maxz,
//       then 19 f64: epsilon, splitter, resulterrbound, ccwerrboundA/B/C,
//       o3derrboundA/B/C, iccerrboundA/B/C, isperrboundA/B/C,
//       o3dstaticfilter, ispstaticfilter, (f64) _use_inexact_arith,
//       (f64) _use_static_filter
//   u32 neval (= 3): configs 0..2 are the evaluation modes
//   u32 n4; per case: u8 category, f64 pa[3] pb[3] pc[3] pd[3],
//       u64 orient3d[mode 0..2], orient3dfast, orient3dexact,
//       orient2dexact(pa, pb, pc)
//   u32 n5; per case: u8 category, f64 pa..pe [15], f64 heights[5],
//       u64 insphere[mode 0..2], insphereexact, orient4d[mode 0..2],
//       orient4dexact

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>
#include <array>

#ifdef COUNT_STAGES
enum { FN_ORIENT3D, FN_ORIENT3DADAPT, FN_INSPHERE, FN_INSPHEREADAPT,
       FN_ORIENT4D, FN_ORIENT4DADAPT, FN_COUNT };
static long long g_hits[FN_COUNT][8][3];
static int g_mode = 0;
#define TG_HIT(fn, k) (g_hits[fn][k][g_mode]++)
#endif

#include "predicates.cxx"

REAL orient3dfast(REAL *pa, REAL *pb, REAL *pc, REAL *pd);
REAL insphereexact(REAL *pa, REAL *pb, REAL *pc, REAL *pd, REAL *pe);

// ---- deterministic generator (splitmix64) --------------------------------
static uint64_t g_state = 0x1790179017901790ULL;
static uint64_t next_u64() {
  uint64_t z = (g_state += 0x9E3779B97F4A7C15ULL);
  z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
  z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
  return z ^ (z >> 31);
}
// Uniform in [-1, 1).
static double uni() { return (double)(next_u64() >> 11) * 0x1.0p-52 - 1.0; }
static int rint_in(int lo, int hi) { return lo + (int)(next_u64() % (uint64_t)(hi - lo + 1)); }
// Random sign, random full 52-bit mantissa, exponent in [elo, ehi].
static double fullrand(int elo, int ehi) {
  uint64_t m = next_u64() & 0x000FFFFFFFFFFFFFULL;
  double x = ldexp(1.0 + (double)m * 0x1.0p-52, rint_in(elo, ehi));
  return (next_u64() & 1) ? -x : x;
}
static double ulp_nudge(double x) {
  return (next_u64() & 1) ? nextafter(x, INFINITY) : nextafter(x, -INFINITY);
}

static uint64_t bits(double x) { uint64_t b; memcpy(&b, &x, 8); return b; }
static void put_u8(FILE *f, uint8_t v) { fwrite(&v, 1, 1, f); }
static void put_u32(FILE *f, uint32_t v) { fwrite(&v, 4, 1, f); }
static void put_f64(FILE *f, double v) { fwrite(&v, 8, 1, f); }
static void put_u64(FILE *f, uint64_t v) { fwrite(&v, 8, 1, f); }

struct Config { int noexact, nofilter; double mx, my, mz; };
// 0..2 are the evaluation modes: static filter on (TetGen's default), filter
// off (-X1), inexact (-X). The rest only check exactinit's constants (the
// sort of the extents, and degenerate extents).
static const Config CONFIGS[] = {
  {0, 0, 2.7, 0.9, 1.3}, {0, 1, 2.7, 0.9, 1.3}, {1, 0, 2.7, 0.9, 1.3},
  {0, 0, 0.9, 1.3, 2.7}, {0, 0, 0.9, 2.7, 1.3}, {0, 0, 1.3, 0.9, 2.7},
  {0, 0, 1.3, 2.7, 0.9}, {0, 0, 2.7, 1.3, 0.9}, {0, 0, 4.0, 1.0, 2.0},
  {0, 0, 0.0, 0.0, 0.0}, {1, 1, 1e-300, 3e-7, 1e100}, {0, 0, 0.1, 0.1, 0.1},
};
static const int NCONFIG = sizeof(CONFIGS) / sizeof(CONFIGS[0]);
static const int NEVAL = 3;

static void init(const Config &c) {
  exactinit(0, c.noexact, c.nofilter, c.mx, c.my, c.mz);
}

struct Case4 { uint8_t cat; double p[4][3]; };
struct Case5 { uint8_t cat; double p[5][3]; double h[5]; };

static const int PER_CAT = 600;

// Integer points on spheres x^2 + y^2 + z^2 = r^2 (r = 3, 7, 9, 11, 15).
static std::vector<std::array<int, 3>> sphere_lattice(int r) {
  std::vector<std::array<int, 3>> v;
  for (int x = -r; x <= r; x++)
    for (int y = -r; y <= r; y++)
      for (int z = -r; z <= r; z++)
        if (x * x + y * y + z * z == r * r) v.push_back({x, y, z});
  return v;
}

static void gen4(std::vector<Case4> &out) {
  Case4 c;
  // 0: uniform random in [-1, 1)^3.
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 0;
    for (auto &q : c.p) for (auto &x : q) x = uni();
    out.push_back(c);
  }
  // 1: random full mantissas, exponents in [-10, 10].
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 1;
    for (auto &q : c.p) for (auto &x : q) x = fullrand(-10, 10);
    out.push_back(c);
  }
  // 2: integer lattice points on a plane z = a x + b y + d (exactly coplanar,
  //    exact differences: zero tails).
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 2;
    int a = rint_in(-3, 3), b = rint_in(-3, 3), d = rint_in(-5, 5);
    for (auto &q : c.p) {
      int x = rint_in(-20, 20), y = rint_in(-20, 20);
      q[0] = x; q[1] = y; q[2] = a * x + b * y + d;
    }
    out.push_back(c);
  }
  // 3: exactly coplanar on x = y, random full mantissas (inexact differences:
  //    the adaptive tails are non-zero, the determinant is exactly zero).
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 3;
    for (auto &q : c.p) { q[0] = q[1] = fullrand(-4, 4); q[2] = fullrand(-4, 4); }
    out.push_back(c);
  }
  // 4: nearly coplanar: pd = pa + s (pb - pa) + t (pc - pa) in floating point.
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 4;
    for (int i = 0; i < 3; i++) for (auto &x : c.p[i]) x = fullrand(-3, 3);
    double s = uni(), t = uni();
    for (int k = 0; k < 3; k++)
      c.p[3][k] = c.p[0][k] + s * (c.p[1][k] - c.p[0][k]) + t * (c.p[2][k] - c.p[0][k]);
    out.push_back(c);
  }
  // 5: the degenerate categories 2, 3, 4 with one coordinate moved by 1 ulp.
  {
    size_t base = out.size();
    for (int n = 0; n < PER_CAT; n++) {
      c = out[PER_CAT * (2 + n % 3) + n / 3];
      c.cat = 5;
      int i = rint_in(0, 3), k = rint_in(0, 2);
      c.p[i][k] = ulp_nudge(c.p[i][k]);
      out.push_back(c);
    }
    (void)base;
  }
  // 6: large magnitudes (categories 1, 3, 4 scaled by 2^200, then 1e60).
  for (int n = 0; n < PER_CAT; n++) {
    c = out[PER_CAT * (n % 2 ? 3 : (n % 3 ? 1 : 4)) + n / 3];
    c.cat = 6;
    double s = (n % 4 == 0) ? 1e60 : 0x1.0p200;
    for (auto &q : c.p) for (auto &x : q) x *= s;
    out.push_back(c);
  }
  // 7: tiny magnitudes (scaled by 2^-200, 1e-60, and 2^-340 where products
  //    of three reach the subnormal range).
  for (int n = 0; n < PER_CAT; n++) {
    c = out[PER_CAT * (n % 2 ? 3 : (n % 3 ? 1 : 4)) + n / 3];
    c.cat = 7;
    double s = (n % 3 == 0) ? 1e-60 : (n % 3 == 1 ? 0x1.0p-200 : 0x1.0p-340);
    for (auto &q : c.p) for (auto &x : q) x *= s;
    out.push_back(c);
  }
  // 8: an exactly coplanar lattice translated by a non-dyadic offset (0.1,
  //    1e3 + 0.3, ...): nearly coplanar with inexact differences (stage C).
  for (int n = 0; n < PER_CAT; n++) {
    c = out[PER_CAT * 2 + n];
    c.cat = 8;
    double off[3] = {0.1 * rint_in(1, 9), 1e3 + 0.3, -7.7 * rint_in(1, 3)};
    if (n % 2) { off[0] *= 1e6; off[1] = 0.123456789; }
    for (auto &q : c.p) for (int k = 0; k < 3; k++) q[k] += off[k];
    out.push_back(c);
  }
  // 9: repeated points (pa = pd, or pa = pb), collinear triples.
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 9;
    for (auto &q : c.p) for (auto &x : q) x = fullrand(-2, 2);
    if (n % 3 == 0) for (int k = 0; k < 3; k++) c.p[3][k] = c.p[0][k];
    else if (n % 3 == 1) for (int k = 0; k < 3; k++) c.p[1][k] = c.p[0][k];
    else for (int k = 0; k < 3; k++) c.p[2][k] = 2.0 * c.p[1][k] - c.p[0][k];
    out.push_back(c);
  }
}

static double lift(const double *p) { return p[0] * p[0] + p[1] * p[1] + p[2] * p[2]; }

static void gen5(std::vector<Case5> &out) {
  Case5 c;
  auto s3 = sphere_lattice(3), s7 = sphere_lattice(7), s9 = sphere_lattice(9),
       s11 = sphere_lattice(11), s15 = sphere_lattice(15);
  std::vector<std::array<int, 3>> *spheres[] = {&s3, &s7, &s9, &s11, &s15};
  // 0: uniform random, random heights.
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 0;
    for (auto &q : c.p) for (auto &x : q) x = uni();
    for (auto &h : c.h) h = uni();
    out.push_back(c);
  }
  // 1: random full mantissas; heights the lifts |p|^2 in floating point.
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 1;
    for (auto &q : c.p) for (auto &x : q) x = fullrand(-8, 8);
    for (int i = 0; i < 5; i++) c.h[i] = (n % 2) ? lift(c.p[i]) : fullrand(-8, 8);
    out.push_back(c);
  }
  // 2: integer points on a sphere about an integer centre (exactly
  //    cospherical, exact differences); heights = exact integer lifts.
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 2;
    auto &s = *spheres[n % 5];
    int cx = rint_in(-9, 9), cy = rint_in(-9, 9), cz = rint_in(-9, 9);
    for (int i = 0; i < 5; i++) {
      auto &v = s[next_u64() % s.size()];
      c.p[i][0] = v[0] + cx; c.p[i][1] = v[1] + cy; c.p[i][2] = v[2] + cz;
      c.h[i] = lift(c.p[i]);
    }
    out.push_back(c);
  }
  // 3: five points on the plane x = y with full mantissas: insphere and
  //    orient4d determinants exactly zero, non-zero tails.
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 3;
    for (auto &q : c.p) { q[0] = q[1] = fullrand(-4, 4); q[2] = fullrand(-4, 4); }
    for (int i = 0; i < 5; i++) c.h[i] = (n % 2) ? lift(c.p[i]) : fullrand(-4, 4);
    out.push_back(c);
  }
  // 4: category 2 translated by a non-dyadic offset: nearly cospherical with
  //    inexact differences; heights the lifts in floating point.
  for (int n = 0; n < PER_CAT; n++) {
    c = out[PER_CAT * 2 + n];
    c.cat = 4;
    double off[3] = {0.1, 0.2 + rint_in(0, 5), 0.3 * rint_in(1, 7)};
    if (n % 3 == 0) { off[0] = 1e4 + 0.7; }
    for (int i = 0; i < 5; i++) {
      for (int k = 0; k < 3; k++) c.p[i][k] += off[k];
      c.h[i] = lift(c.p[i]);
    }
    out.push_back(c);
  }
  // 5: categories 2, 3, 4 with one coordinate (or height) moved by 1 ulp.
  for (int n = 0; n < PER_CAT; n++) {
    c = out[PER_CAT * (2 + n % 3) + n / 3];
    c.cat = 5;
    int i = rint_in(0, 4), k = rint_in(0, 3);
    if (k == 3) c.h[i] = ulp_nudge(c.h[i]); else c.p[i][k] = ulp_nudge(c.p[i][k]);
    out.push_back(c);
  }
  // 6: large magnitudes (categories 1, 3, 4 scaled; heights scaled by s^2).
  for (int n = 0; n < PER_CAT; n++) {
    c = out[PER_CAT * (n % 2 ? 3 : (n % 3 ? 1 : 4)) + n / 3];
    c.cat = 6;
    double s = (n % 4 == 0) ? 1e40 : 0x1.0p120;
    for (auto &q : c.p) for (auto &x : q) x *= s;
    for (auto &h : c.h) h *= s * s;
    out.push_back(c);
  }
  // 7: tiny magnitudes (2^-120, 1e-40, and 2^-210 where the degree-5 terms
  //    reach the subnormal range).
  for (int n = 0; n < PER_CAT; n++) {
    c = out[PER_CAT * (n % 2 ? 3 : (n % 3 ? 1 : 4)) + n / 3];
    c.cat = 7;
    double s = (n % 3 == 0) ? 1e-40 : (n % 3 == 1 ? 0x1.0p-120 : 0x1.0p-210);
    for (auto &q : c.p) for (auto &x : q) x *= s;
    for (auto &h : c.h) h *= s * s;
    out.push_back(c);
  }
  // 8: points on the unit sphere by normalisation (nearly cospherical,
  //    rounded), about a random centre; heights the floating-point lifts.
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 8;
    double ctr[3] = {uni(), uni(), uni()};
    for (int i = 0; i < 5; i++) {
      double v[3] = {uni(), uni(), uni()};
      double r = sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
      for (int k = 0; k < 3; k++) c.p[i][k] = ctr[k] + v[k] / r;
      c.h[i] = lift(c.p[i]);
    }
    out.push_back(c);
  }
  // 9: heights on a hyperplane h = a x + b y + c z + d (orient4d exactly or
  //    nearly zero), integer or full-mantissa coordinates.
  for (int n = 0; n < PER_CAT; n++) {
    c.cat = 9;
    int a = rint_in(-3, 3), b = rint_in(-3, 3), cc = rint_in(-3, 3), d = rint_in(-5, 5);
    for (int i = 0; i < 5; i++) {
      for (int k = 0; k < 3; k++) c.p[i][k] = (n % 2) ? (double)rint_in(-30, 30) : fullrand(-3, 3);
      c.h[i] = a * c.p[i][0] + b * c.p[i][1] + cc * c.p[i][2] + d;
    }
    out.push_back(c);
  }
}

int main(int argc, char **argv) {
  if (argc < 2) { fprintf(stderr, "usage: %s out.bin\n", argv[0]); return 2; }
  FILE *f = fopen(argv[1], "wb");
  if (!f) { perror(argv[1]); return 1; }
  fwrite("TGPRED01", 1, 8, f);

  put_u32(f, NCONFIG);
  for (int i = 0; i < NCONFIG; i++) {
    const Config &c = CONFIGS[i];
    init(c);
    put_u8(f, c.noexact); put_u8(f, c.nofilter);
    put_f64(f, c.mx); put_f64(f, c.my); put_f64(f, c.mz);
    double k[] = {epsilon, splitter, resulterrbound,
                  ccwerrboundA, ccwerrboundB, ccwerrboundC,
                  o3derrboundA, o3derrboundB, o3derrboundC,
                  iccerrboundA, iccerrboundB, iccerrboundC,
                  isperrboundA, isperrboundB, isperrboundC,
                  o3dstaticfilter, ispstaticfilter,
                  (double)_use_inexact_arith, (double)_use_static_filter};
    for (double x : k) put_f64(f, x);
  }
  put_u32(f, NEVAL);

  std::vector<Case4> c4; gen4(c4);
  std::vector<Case5> c5; gen5(c5);

  // Results per case: mode-dependent ones under each evaluation config, the
  // mode-independent ones once (under config 0).
  std::vector<uint64_t> r4(c4.size() * 6), r5(c5.size() * 8);
  for (int m = 0; m < NEVAL; m++) {
    init(CONFIGS[m]);
#ifdef COUNT_STAGES
    g_mode = m;
#endif
    for (size_t i = 0; i < c4.size(); i++) {
      Case4 c = c4[i];
      r4[i * 6 + m] = bits(orient3d(c.p[0], c.p[1], c.p[2], c.p[3]));
    }
    for (size_t i = 0; i < c5.size(); i++) {
      Case5 c = c5[i];
      r5[i * 8 + m] = bits(insphere(c.p[0], c.p[1], c.p[2], c.p[3], c.p[4]));
      r5[i * 8 + 4 + m] = bits(orient4d(c.p[0], c.p[1], c.p[2], c.p[3], c.p[4],
                                        c.h[0], c.h[1], c.h[2], c.h[3], c.h[4]));
    }
  }
  init(CONFIGS[0]);
  for (size_t i = 0; i < c4.size(); i++) {
    Case4 c = c4[i];
    r4[i * 6 + 3] = bits(orient3dfast(c.p[0], c.p[1], c.p[2], c.p[3]));
    r4[i * 6 + 4] = bits(orient3dexact(c.p[0], c.p[1], c.p[2], c.p[3]));
    r4[i * 6 + 5] = bits(orient2dexact(c.p[0], c.p[1], c.p[2]));
  }
  for (size_t i = 0; i < c5.size(); i++) {
    Case5 c = c5[i];
    r5[i * 8 + 3] = bits(insphereexact(c.p[0], c.p[1], c.p[2], c.p[3], c.p[4]));
    r5[i * 8 + 7] = bits(orient4dexact(c.p[0], c.p[1], c.p[2], c.p[3], c.p[4],
                                       c.h[0], c.h[1], c.h[2], c.h[3], c.h[4]));
  }

  put_u32(f, (uint32_t)c4.size());
  for (size_t i = 0; i < c4.size(); i++) {
    put_u8(f, c4[i].cat);
    for (auto &q : c4[i].p) for (double x : q) put_f64(f, x);
    for (int j = 0; j < 6; j++) put_u64(f, r4[i * 6 + j]);
  }
  put_u32(f, (uint32_t)c5.size());
  for (size_t i = 0; i < c5.size(); i++) {
    put_u8(f, c5[i].cat);
    for (auto &q : c5[i].p) for (double x : q) put_f64(f, x);
    for (double h : c5[i].h) put_f64(f, h);
    for (int j = 0; j < 8; j++) put_u64(f, r5[i * 8 + j]);
  }
  fclose(f);

#ifdef COUNT_STAGES
  static const char *fn_names[FN_COUNT] = {"orient3d", "orient3dadapt", "insphere",
                                           "insphereadapt", "orient4d", "orient4dadapt"};
  static const int nsites[FN_COUNT] = {5, 4, 4, 4, 2, 4};
  static const char *site_names[FN_COUNT][5] = {
    {"inexact (-X)", "static filter, det > f", "static filter, det < -f", "stage A", "-> orient3dadapt"},
    {"stage B", "all tails zero", "stage C", "exact tail"},
    {"inexact (-X)", "static filter, |det| > f", "stage A", "-> insphereadapt"},
    {"stage B", "all tails zero", "stage C", "-> insphereexact"},
    {"stage A", "-> orient4dadapt"},
    {"stage B", "all tails zero", "stage C", "-> orient4dexact"}};
  printf("%-14s %-26s %10s %10s %10s\n", "function", "return site", "filter-on", "filter-off", "inexact");
  for (int fn = 0; fn < FN_COUNT; fn++)
    for (int k = 0; k < nsites[fn]; k++)
      printf("%-14s %-26s %10lld %10lld %10lld\n", fn_names[fn], site_names[fn][k],
             g_hits[fn][k][0], g_hits[fn][k][1], g_hits[fn][k][2]);
  // Sanity: every function's sites numbered as expected (no extra returns).
  for (int fn = 0; fn < FN_COUNT; fn++)
    for (int k = nsites[fn]; k < 8; k++)
      for (int m = 0; m < 3; m++)
        if (g_hits[fn][k][m]) { fprintf(stderr, "unexpected site %s/%d\n", fn_names[fn], k); return 1; }
#endif
  return 0;
}
