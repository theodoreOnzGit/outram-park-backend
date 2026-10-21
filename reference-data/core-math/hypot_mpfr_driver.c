/* Correct-rounding check for PETIR's `hypot` / `hypotf` (gh:#997).
 *
 * Reference: GNU MPFR `mpfr_hypot` at the target precision (53 / 24 bits),
 * round-to-nearest, exponent range set to binary64 / binary32
 * (emin = -1073 / -148, emax = 1024 / 128) and `mpfr_subnormalize`, i.e. the
 * correctly rounded IEEE result, subnormals included. Zeros and NaNs are
 * handled before MPFR exactly as CORE-MATH's own `hypot_mpfr.c` /
 * `hypotf_mpfr.c` do (MPFR has no signed NaN payloads).
 *
 * Against that reference, for EVERY input, it counts mismatches of:
 *   petir  - PETIR's port (crate::hypot, via petir_hypot_shim.rs)
 *   glibc  - the platform's hypot / hypotf (-lm)
 *   musl   - the `libm` crate's hypot / hypotf (what PETIR used before)
 * A match is equal bits, or both NaN.
 *
 * Inputs: deterministic splitmix64 sweeps (regenerated bit for bit by
 * crates/petir/tests/hypot_correct_rounding.rs), an all-pairs table of
 * special values, and CORE-MATH's own worst-case files hypot.wc / hypotf.wc.
 *
 * Output (stdout): the reference file. `H` lines carry an FNV-1a hash of
 * the MPFR results of a whole sweep (NaN canonicalised), `E` lines explicit
 * cases, `#` lines the mismatch counts.
 *
 * Build and run (from the repository root): see README.md.
 */
#include <math.h>
#include <mpfr.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef uint64_t u64;
typedef uint32_t u32;

extern double petir_hypot(double, double);
extern float petir_hypotf(float, float);
extern double musl_hypot(double, double);
extern float musl_hypotf(float, float);

static u64 d2u(double x) { u64 u; memcpy(&u, &x, 8); return u; }
static double u2d(u64 u) { double x; memcpy(&x, &u, 8); return x; }
static u32 f2u(float x) { u32 u; memcpy(&u, &x, 4); return u; }
static float u2f(u32 u) { float x; memcpy(&x, &u, 4); return x; }

/* ---- references, after CORE-MATH hypot_mpfr.c / hypotf_mpfr.c ---- */
static double ref_hypot(double x, double y) {
  u64 xu = d2u(x), yu = d2u(y);
  /* sNaN anywhere: qNaN (IEEE 754; CORE-MATH's hypot_mpfr.c does the same) */
  if ((isnan(x) && !((xu >> 51) & 1)) || (isnan(y) && !((yu >> 51) & 1))) return x + y;
  if (isnan(x) || isnan(y)) {
    if (isinf(x) || isinf(y)) return INFINITY; /* hypot(+-inf, qNaN) = +inf */
    return x + y;
  }
  if ((xu << 1) == 0) return u2d((yu << 1) >> 1);
  if ((yu << 1) == 0) return u2d((xu << 1) >> 1);
  mpfr_t xm, ym, zm;
  mpfr_set_emin(-1073);
  mpfr_set_emax(1024);
  mpfr_init2(xm, 53); mpfr_init2(ym, 53); mpfr_init2(zm, 53);
  mpfr_set_d(xm, x, MPFR_RNDN);
  mpfr_set_d(ym, y, MPFR_RNDN);
  int inex = mpfr_hypot(zm, xm, ym, MPFR_RNDN);
  mpfr_subnormalize(zm, inex, MPFR_RNDN);
  double r = mpfr_get_d(zm, MPFR_RNDN);
  mpfr_clear(xm); mpfr_clear(ym); mpfr_clear(zm);
  return r;
}

static float ref_hypotf(float x, float y) {
  u32 xu = f2u(x), yu = f2u(y);
  if ((isnan(x) && !((xu >> 22) & 1)) || (isnan(y) && !((yu >> 22) & 1))) return x + y;
  if (isnan(x) || isnan(y)) {
    if (isinf(x) || isinf(y)) return INFINITY;
    return x + y;
  }
  if ((xu << 1) == 0) return u2f((yu << 1) >> 1);
  if ((yu << 1) == 0) return u2f((xu << 1) >> 1);
  mpfr_t xm, ym, zm;
  mpfr_set_emin(-148);
  mpfr_set_emax(128);
  mpfr_init2(xm, 24); mpfr_init2(ym, 24); mpfr_init2(zm, 24);
  mpfr_set_flt(xm, x, MPFR_RNDN);
  mpfr_set_flt(ym, y, MPFR_RNDN);
  int inex = mpfr_hypot(zm, xm, ym, MPFR_RNDN);
  mpfr_subnormalize(zm, inex, MPFR_RNDN);
  float r = mpfr_get_flt(zm, MPFR_RNDN);
  mpfr_clear(xm); mpfr_clear(ym); mpfr_clear(zm);
  return r;
}

/* ---- generators (mirrored in the Rust test; keep in step) ---- */
static u64 sm_state;
static u64 sm(void) {
  u64 z = (sm_state += 0x9e3779b97f4a7c15ull);
  z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ull;
  z = (z ^ (z >> 27)) * 0x94d049bb133111ebull;
  return z ^ (z >> 31);
}
static u64 fin64(u64 b) { return (((b >> 52) & 0x7ff) == 0x7ff) ? b ^ (1ull << 62) : b; }
static u32 fin32(u32 b) { return (((b >> 23) & 0xff) == 0xff) ? b ^ (1u << 30) : b; }

/* set 1 uniform bits, 2 close exponents, 3 subnormal/tiny, 4 Pythagorean +-1ulp */
static void gen64(int set, double *x, double *y) {
  u64 a, b;
  if (set == 1) { a = fin64(sm()); b = fin64(sm()); }
  else if (set == 2) {
    a = fin64(sm());
    long e = (long)((a >> 52) & 0x7ff) + (long)(sm() % 121) - 60;
    if (e < 0) e = 0;
    if (e > 0x7fe) e = 0x7fe;
    b = (sm() & 0x800fffffffffffffull) | ((u64)e << 52);
  } else if (set == 3) {
    /* one sm() per statement: C leaves the order within an expression unspecified */
    u64 ma = sm(); u64 ea = sm() % 64; u64 mb = sm(); u64 eb = sm() % 64;
    a = (ma & 0x800fffffffffffffull) | (ea << 52);
    b = (mb & 0x800fffffffffffffull) | (eb << 52);
  } else {
    u64 m = 1 + sm() % 8191, n = 1 + sm() % 8191;
    if (m == n) m++;
    double p = (double)(m > n ? m * m - n * n : n * n - m * m), q = (double)(2 * m * n);
    int s = (int)(sm() % 2001) - 1000;
    p = ldexp(p, s); q = ldexp(q, s);
    b = d2u(q);
    u64 k = sm() % 3;
    if (k == 1) b += 1;
    if (k == 2) b -= 1;
    a = d2u(p);
  }
  *x = u2d(a); *y = u2d(b);
}

static void gen32(int set, float *x, float *y) {
  u32 a, b;
  if (set == 1) { a = fin32((u32)(sm() >> 32)); b = fin32((u32)(sm() >> 32)); }
  else if (set == 2) {
    a = fin32((u32)(sm() >> 32));
    long e = (long)((a >> 23) & 0xff) + (long)(sm() % 61) - 30;
    if (e < 0) e = 0;
    if (e > 0xfe) e = 0xfe;
    b = ((u32)(sm() >> 32) & 0x807fffffu) | ((u32)e << 23);
  } else if (set == 3) {
    u64 ma = sm(); u64 ea = sm() % 32; u64 mb = sm(); u64 eb = sm() % 32;
    a = ((u32)(ma >> 32) & 0x807fffffu) | ((u32)ea << 23);
    b = ((u32)(mb >> 32) & 0x807fffffu) | ((u32)eb << 23);
  } else {
    u64 m = 1 + sm() % 2047, n = 1 + sm() % 2047;
    if (m == n) m++;
    float p = (float)(m > n ? m * m - n * n : n * n - m * m), q = (float)(2 * m * n);
    int s = (int)(sm() % 241) - 120;
    p = ldexpf(p, s); q = ldexpf(q, s);
    b = f2u(q);
    u64 k = sm() % 3;
    if (k == 1) b += 1;
    if (k == 2) b -= 1;
    a = f2u(p);
  }
  *x = u2f(a); *y = u2f(b);
}

static u64 fnv(u64 h, u64 v, int bytes) {
  for (int i = 0; i < bytes; i++) { h ^= (v >> (8 * i)) & 0xff; h *= 0x100000001b3ull; }
  return h;
}

typedef struct { long n, petir, glibc, musl; } tally;

static int same64(double a, double b) { return (isnan(a) && isnan(b)) || d2u(a) == d2u(b); }
static int same32(float a, float b) { return (isnan(a) && isnan(b)) || f2u(a) == f2u(b); }

static double check64(double x, double y, tally *t) {
  double r = ref_hypot(x, y);
  t->n++;
  if (!same64(petir_hypot(x, y), r)) {
    t->petir++;
    fprintf(stderr, "PETIR MISMATCH f64 %016llx %016llx\n", (unsigned long long)d2u(x), (unsigned long long)d2u(y));
  }
  if (!same64(hypot(x, y), r)) t->glibc++;
  if (!same64(musl_hypot(x, y), r)) t->musl++;
  return r;
}

static float check32(float x, float y, tally *t) {
  float r = ref_hypotf(x, y);
  t->n++;
  if (!same32(petir_hypotf(x, y), r)) {
    t->petir++;
    fprintf(stderr, "PETIR MISMATCH f32 %08x %08x\n", f2u(x), f2u(y));
  }
  if (!same32(hypotf(x, y), r)) t->glibc++;
  if (!same32(musl_hypotf(x, y), r)) t->musl++;
  return r;
}

static void summary(const char *ty, const char *set, tally *t) {
  printf("# SUMMARY %s %s n=%ld petir_mismatch=%ld glibc_mismatch=%ld musl_mismatch=%ld\n",
         ty, set, t->n, t->petir, t->glibc, t->musl);
}

/* parse one CORE-MATH .wc token: hex/decimal float, [+-]nan, [+-]snan, [+-]inf */
static int parse_tok(const char *s, double *v) {
  int neg = (*s == '-');
  const char *p = (*s == '+' || *s == '-') ? s + 1 : s;
  if (!strncmp(p, "snan", 4)) { *v = u2d((neg ? 0xfff4000000000000ull : 0x7ff4000000000000ull)); return 1; }
  if (!strncmp(p, "nan", 3)) { *v = neg ? -NAN : NAN; return 1; }
  if (!strncmp(p, "inf", 3)) { *v = neg ? -INFINITY : INFINITY; return 1; }
  char *end; *v = strtod(s, &end);
  return end != s;
}

static void wc64(const char *path, int stride) {
  FILE *f = fopen(path, "r");
  if (!f) { perror(path); exit(1); }
  char line[512]; tally t = {0}; long i = 0;
  while (fgets(line, sizeof line, f)) {
    if (line[0] == '#' || line[0] == '\n') continue;
    char *c = strchr(line, ',');
    if (!c) continue;
    *c = 0;
    double x, y;
    if (!parse_tok(line, &x) || !parse_tok(c + 1, &y)) continue;
    double r = check64(x, y, &t);
    if (i++ % stride == 0)
      printf("E f64 %016llx %016llx %016llx\n", (unsigned long long)d2u(x), (unsigned long long)d2u(y), (unsigned long long)d2u(r));
  }
  fclose(f);
  summary("f64", "core_math_worst_cases", &t);
}

static void wc32(const char *path, int stride) {
  FILE *f = fopen(path, "r");
  if (!f) { perror(path); exit(1); }
  char line[512]; tally t = {0}; long i = 0;
  while (fgets(line, sizeof line, f)) {
    if (line[0] == '#' || line[0] == '\n') continue;
    char *c = strchr(line, ',');
    if (!c) continue;
    *c = 0;
    double xd, yd;
    if (!parse_tok(line, &xd) || !parse_tok(c + 1, &yd)) continue;
    /* tokens are binary32 values (snan mapped separately) */
    float x = (float)xd, y = (float)yd;
    if (d2u(xd) == 0x7ff4000000000000ull) x = u2f(0x7fa00000u);
    if (d2u(xd) == 0xfff4000000000000ull) x = u2f(0xffa00000u);
    if (d2u(yd) == 0x7ff4000000000000ull) y = u2f(0x7fa00000u);
    if (d2u(yd) == 0xfff4000000000000ull) y = u2f(0xffa00000u);
    float r = check32(x, y, &t);
    if (i++ % stride == 0)
      printf("E f32 %08x %08x %08x\n", f2u(x), f2u(y), f2u(r));
  }
  fclose(f);
  summary("f32", "core_math_worst_cases", &t);
}

int main(int argc, char **argv) {
  if (argc < 4) { fprintf(stderr, "usage: %s hypot.wc hypotf.wc N\n", argv[0]); return 1; }
  long n = atol(argv[3]);
  static const char *names[] = {"", "uniform_bits", "close_exponents", "subnormal", "pythagorean"};
  printf("# PETIR hypot correct-rounding reference (gh:#997). Made by hypot_mpfr_driver.c\n");
  printf("# MPFR %s, glibc %s. Format: H <type> <set> <seed> <n> <fnv1a of MPFR results>;\n",
         mpfr_get_version(), "(-lm)");
  printf("# E <type> <x bits> <y bits> <MPFR result bits>. NaN results canonicalised in hashes.\n");

  /* special values, all ordered pairs, both signs */
  static const u64 sp64[] = {
    0, 1, 2, 3, 0x000fffffffffffffull, 0x0008000000000000ull, 0x0010000000000000ull,
    0x0010000000000001ull, 0x3ca0000000000000ull, 0x3e40000000000000ull, 0x3e50000000000000ull,
    0x3ff0000000000000ull, 0x3ff0000000000001ull, 0x3ffbb67ae8584caaull /* sqrt 3 */,
    0x4008000000000000ull, 0x4010000000000000ull, 0x5fe0000000000000ull, 0x7fe0000000000000ull,
    0x7fefffffffffffffull, 0x7ff0000000000000ull, 0x7ff8000000000000ull, 0x2000000000000000ull,
    0x1ff0000000000000ull, 0x6000000000000000ull};
  tally ts = {0};
  int ns = sizeof sp64 / sizeof sp64[0];
  for (int i = 0; i < ns; i++)
    for (int j = 0; j < ns; j++)
      for (int s = 0; s < 4; s++) {
        double x = u2d(sp64[i] | ((u64)(s & 1) << 63)), y = u2d(sp64[j] | ((u64)(s >> 1) << 63));
        double r = check64(x, y, &ts);
        printf("E f64 %016llx %016llx %016llx\n", (unsigned long long)d2u(x), (unsigned long long)d2u(y), (unsigned long long)d2u(r));
      }
  summary("f64", "special_pairs", &ts);
  static const u32 sp32[] = {
    0, 1, 2, 3, 0x007fffffu, 0x00400000u, 0x00800000u, 0x00800001u, 0x33800000u, 0x39000000u,
    0x39800000u, 0x3f800000u, 0x3f800001u, 0x3fddb3d7u /* sqrt 3 */, 0x40400000u, 0x40800000u,
    0x5f000000u, 0x7f000000u, 0x7f7fffffu, 0x7f800000u, 0x7fc00000u, 0x20000000u, 0x1f800000u};
  tally ts32 = {0};
  int ns32 = sizeof sp32 / sizeof sp32[0];
  for (int i = 0; i < ns32; i++)
    for (int j = 0; j < ns32; j++)
      for (int s = 0; s < 4; s++) {
        float x = u2f(sp32[i] | ((u32)(s & 1) << 31)), y = u2f(sp32[j] | ((u32)(s >> 1) << 31));
        float r = check32(x, y, &ts32);
        printf("E f32 %08x %08x %08x\n", f2u(x), f2u(y), f2u(r));
      }
  summary("f32", "special_pairs", &ts32);

  for (int set = 1; set <= 4; set++) {
    tally t = {0};
    u64 h = 0xcbf29ce484222325ull;
    sm_state = (u64)set;
    for (long i = 0; i < n; i++) {
      double x, y; gen64(set, &x, &y);
      double r = check64(x, y, &t);
      h = fnv(h, isnan(r) ? 0x7ff8000000000000ull : d2u(r), 8);
    }
    printf("H f64 %s %d %ld %016llx\n", names[set], set, n, (unsigned long long)h);
    summary("f64", names[set], &t);
  }
  for (int set = 1; set <= 4; set++) {
    tally t = {0};
    u64 h = 0xcbf29ce484222325ull;
    sm_state = (u64)(100 + set);
    for (long i = 0; i < n; i++) {
      float x, y; gen32(set, &x, &y);
      float r = check32(x, y, &t);
      h = fnv(h, isnan(r) ? 0x7fc00000u : f2u(r), 4);
    }
    printf("H f32 %s %d %ld %016llx\n", names[set], 100 + set, n, (unsigned long long)h);
    summary("f32", names[set], &t);
  }
  wc64(argv[1], 16);
  wc32(argv[2], 16);
  return 0;
}
