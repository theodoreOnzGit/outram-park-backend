/* GSL 2.8 specfunc reference driver for PETIR (gh:#1338, case 1).
 *
 * Every ported specfunc routine on a grid built to hit every branch (dense
 * linear grid, log grid, the round numbers GSL's thresholds sit on and their
 * floating-point neighbours, huge/tiny arguments, domain-error arguments).
 * One line per call:
 *
 *     <name> <arg bits>... <gsl status> <gsl err bits> <value bits>...
 *
 * All numbers are the 64-bit IEEE patterns in 16 hex digits; integer
 * arguments are printed as the bits of the double that holds them, and
 * `mode` arguments as a decimal 0 (GSL_PREC_DOUBLE) or 1 (SINGLE; APPROX is identical in ellint)
 * placed before the status.  After the status comes GSL's `result.err` (0 where
 * GSL has none).  GSL's error estimate `result.err` is printed for
 * reference only: PETIR's specfunc returns a bare f64 and has no error
 * estimate to compare it with (see specfunc/mod.rs).
 *
 * Build and run (from the repository root; needs the built GSL, see README.md):
 *   reference-data/gsl/build_driver.sh reference-data/gsl/specfunc_reference_driver.c \
 *       reference-data/gsl/specfunc-gsl-2.8-reference.txt
 */
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <math.h>
#include <gsl/gsl_sf.h>
#include <gsl/gsl_errno.h>

static uint64_t bits(double x) { uint64_t u; memcpy(&u, &x, 8); return u; }
#define B(x) ((unsigned long long)bits(x))

static double G1[4000];
static int n1 = 0;
static void add(double x) { G1[n1++] = x; }
static void build_grid(void) {
  add(0.0); add(-0.0);
  for (int k = -160; k <= 160; k++) if (k) add(k * 0.25);
  for (int e = -80; e <= 12; e++) { double m = pow(10.0, e / 4.0); add(m); add(-m); }
  double big[] = {1e4, 3e4, 1e5, 1e6, 1e8, 1e10, 1e15, 1e20, 1e50, 1e100, 1e154, 1e200, 1e300, 1.7e308,
                  1e-100, 1e-200, 1e-300, 2.3e-308, 1e-310, 4.9e-324};
  for (unsigned i = 0; i < sizeof big / sizeof *big; i++) { add(big[i]); add(-big[i]); }
  double thr[] = {0.5, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 15, 16, 18, 20, 25, 30, 32, 35, 40, 50, 64, 70, 100,
                  1.5, 2.5, 0.25, 0.125, 0.0625, 0.1, 0.2, 0.3, 0.7, 0.9, 0.36787944117144233,
                  3.141592653589793, 6.283185307179586, 1.5707963267948966, 745.0, 709.0, 710.0, 708.0,
                  0.001, 1e-5, 1e-8, 1.4901161193847656e-8, 7.4009597974140505e-4, 2.2204460492503131e-16};
  for (unsigned i = 0; i < sizeof thr / sizeof *thr; i++) {
    double t = thr[i];
    add(t); add(-t); add(nextafter(t, 1e300)); add(nextafter(t, -1e300));
    add(-nextafter(t, 1e300)); add(-nextafter(t, -1e300));
  }
  /* approach to the poles of Gamma / psi and to zeros of sin(pi x): -n +- 10^-j */
  {
    int ns[] = {1, 2, 3, 5, 10, 20, 50};
    for (unsigned a = 0; a < sizeof ns / sizeof *ns; a++)
      for (int j = 1; j <= 15; j++) { double e = pow(10.0, -j); add(-ns[a] + e); add(-ns[a] - e); }
  }
  add(INFINITY); add(-INFINITY); add(NAN);
}

typedef int (*f1)(double, gsl_sf_result *);
#define W1(nm, expr) static int nm(double x, gsl_sf_result *r) { return expr; }
W1(airy_ai, gsl_sf_airy_Ai_e(x, GSL_PREC_DOUBLE, r))
W1(airy_ais, gsl_sf_airy_Ai_scaled_e(x, GSL_PREC_DOUBLE, r))
W1(airy_bi, gsl_sf_airy_Bi_e(x, GSL_PREC_DOUBLE, r))
W1(airy_bis, gsl_sf_airy_Bi_scaled_e(x, GSL_PREC_DOUBLE, r))

static void run1(const char *nm, f1 f) {
  for (int i = 0; i < n1; i++) {
    gsl_sf_result r = {NAN, NAN};
    int st = f(G1[i], &r);
    printf("%s %016llx %d %016llx %016llx\n", nm, B(G1[i]), st, B(r.err), B(r.val));
  }
}

typedef int (*fu)(unsigned, gsl_sf_result *);
static void runu(const char *nm, fu f, unsigned lo, unsigned hi, unsigned step) {
  for (unsigned n = lo; n <= hi; n += step) {
    gsl_sf_result r = {NAN, NAN};
    int st = f(n, &r);
    printf("%s %016llx %d %016llx %016llx\n", nm, B((double)n), st, B(r.err), B(r.val));
  }
}
typedef int (*fi)(int, gsl_sf_result *);
static void runi(const char *nm, fi f, int lo, int hi, int step) {
  for (int n = lo; n <= hi; n += step) {
    gsl_sf_result r = {NAN, NAN};
    int st = f(n, &r);
    printf("%s %016llx %d %016llx %016llx\n", nm, B((double)n), st, B(r.err), B(r.val));
  }
}
typedef int (*f2)(double, double, gsl_sf_result *);
static void run2(const char *nm, f2 f, const double *a, int na, const double *b, int nb) {
  for (int i = 0; i < na; i++) for (int j = 0; j < nb; j++) {
    gsl_sf_result r = {NAN, NAN};
    int st = f(a[i], b[j], &r);
    printf("%s %016llx %016llx %d %016llx %016llx\n", nm, B(a[i]), B(b[j]), st, B(r.err), B(r.val));
  }
}

int main(void) {
  gsl_set_error_handler_off();
  build_grid();

  /* airy */
  run1("airy_ai", airy_ai); run1("airy_ai_scaled", airy_ais);
  run1("airy_bi", airy_bi); run1("airy_bi_scaled", airy_bis);
  /* atanint, clausen, dawson, dilog, expint3, sinint, shint, synchrotron, lambert */
  run1("atanint", gsl_sf_atanint_e); run1("clausen", gsl_sf_clausen_e); run1("dawson", gsl_sf_dawson_e);
  run1("dilog", gsl_sf_dilog_e); run1("expint_3", gsl_sf_expint_3_e);
  run1("si", gsl_sf_Si_e); run1("ci", gsl_sf_Ci_e); run1("shi", gsl_sf_Shi_e); run1("chi", gsl_sf_Chi_e);
  run1("synchrotron_1", gsl_sf_synchrotron_1_e); run1("synchrotron_2", gsl_sf_synchrotron_2_e);
  run1("lambert_w0", gsl_sf_lambert_W0_e); run1("lambert_wm1", gsl_sf_lambert_Wm1_e);
  /* bessel */
  run1("bessel_j0", gsl_sf_bessel_J0_e); run1("bessel_j1", gsl_sf_bessel_J1_e);
  run1("bessel_y0", gsl_sf_bessel_Y0_e); run1("bessel_y1", gsl_sf_bessel_Y1_e);
  run1("bessel_i0", gsl_sf_bessel_I0_e); run1("bessel_i1", gsl_sf_bessel_I1_e);
  run1("bessel_i0_scaled", gsl_sf_bessel_I0_scaled_e); run1("bessel_i1_scaled", gsl_sf_bessel_I1_scaled_e);
  run1("bessel_k0", gsl_sf_bessel_K0_e); run1("bessel_k1", gsl_sf_bessel_K1_e);
  run1("bessel_k0_scaled", gsl_sf_bessel_K0_scaled_e); run1("bessel_k1_scaled", gsl_sf_bessel_K1_scaled_e);
  /* debye, transport */
  run1("debye_1", gsl_sf_debye_1_e); run1("debye_2", gsl_sf_debye_2_e); run1("debye_3", gsl_sf_debye_3_e);
  run1("debye_4", gsl_sf_debye_4_e); run1("debye_5", gsl_sf_debye_5_e); run1("debye_6", gsl_sf_debye_6_e);
  run1("transport_2", gsl_sf_transport_2_e); run1("transport_3", gsl_sf_transport_3_e);
  run1("transport_4", gsl_sf_transport_4_e); run1("transport_5", gsl_sf_transport_5_e);
  /* fermi-dirac */
  run1("fd_m1", gsl_sf_fermi_dirac_m1_e); run1("fd_0", gsl_sf_fermi_dirac_0_e);
  run1("fd_1", gsl_sf_fermi_dirac_1_e); run1("fd_2", gsl_sf_fermi_dirac_2_e);
  run1("fd_mhalf", gsl_sf_fermi_dirac_mhalf_e); run1("fd_half", gsl_sf_fermi_dirac_half_e);
  run1("fd_3half", gsl_sf_fermi_dirac_3half_e);
  /* erf (PETIR substitutes libm: compared for information), gamma, psi, zeta */
  run1("erf", gsl_sf_erf_e); run1("erfc", gsl_sf_erfc_e);
  run1("ln_gamma", gsl_sf_lngamma_e); run1("gamma", gsl_sf_gamma_e);
  run1("psi", gsl_sf_psi_e); run1("psi_1", gsl_sf_psi_1_e); run1("psi_1piy", gsl_sf_psi_1piy_e);
  run1("zeta", gsl_sf_zeta_e); run1("zetam1", gsl_sf_zetam1_e); run1("eta", gsl_sf_eta_e);
  for (int i = 0; i < n1; i++) {
    gsl_sf_result r = {NAN, NAN}; double sgn = NAN;
    int st = gsl_sf_lngamma_sgn_e(G1[i], &r, &sgn);
    printf("ln_gamma_sgn %016llx %d %016llx %016llx %016llx\n", B(G1[i]), st, B(r.err), B(r.val), B(sgn));
  }
  /* integer argument */
  runu("factorial", gsl_sf_fact_e, 0, 175, 1);
  runu("ln_factorial", gsl_sf_lnfact_e, 0, 400, 1);
  runu("ln_factorial", gsl_sf_lnfact_e, 1000, 100000, 9973);
  runi("psi_int", gsl_sf_psi_int_e, 0, 130, 1);
  runi("psi_int", gsl_sf_psi_int_e, 1000, 100000, 9973);
  runi("psi_1_int", gsl_sf_psi_1_int_e, 0, 130, 1);
  runi("psi_1_int", gsl_sf_psi_1_int_e, 1000, 100000, 9973);
  runi("zeta_int", gsl_sf_zeta_int_e, -60, 120, 1);
  runi("zetam1_int", gsl_sf_zetam1_int_e, -60, 120, 1);
  runi("eta_int", gsl_sf_eta_int_e, -60, 120, 1);
  /* psi_n(n, x) */
  {
    int ns[] = {0, 1, 2, 3, 4, 5, 7, 10, 20, 50};
    double xs[] = {-2.5, -0.5, 0.01, 0.1, 0.5, 1, 1.5, 2, 3, 5, 10, 30, 100, 1e3, 1e5, 1e10};
    for (unsigned a = 0; a < sizeof ns / sizeof *ns; a++)
      for (unsigned b = 0; b < sizeof xs / sizeof *xs; b++) {
        gsl_sf_result r = {NAN, NAN};
        int st = gsl_sf_psi_n_e(ns[a], xs[b], &r);
        printf("psi_n %016llx %016llx %d %016llx %016llx\n", B((double)ns[a]), B(xs[b]), st, B(r.err), B(r.val));
      }
  }
  /* hzeta(s, q) */
  {
    double s[] = {1.0000001, 1.001, 1.1, 1.5, 2, 3, 5, 10, 30, 100, 0.5, -1, -0.5, 1e-3, 1e3};
    double q[] = {-0.5, 0, 1e-3, 0.01, 0.5, 1, 1.5, 3, 10, 30, 100, 1e4, 1e8};
    run2("hzeta", gsl_sf_hzeta_e, s, 15, q, 13);
  }
  /* ln_beta, beta */
  {
    double a[] = {-2.5, -0.5, 1e-8, 0.01, 0.1, 0.5, 1, 1.5, 2, 3.7, 10, 50, 170, 300, 1e4, 1e10};
    run2("ln_beta", gsl_sf_lnbeta_e, a, 16, a, 16);
    run2("beta", gsl_sf_beta_e, a, 16, a, 16);
  }
  /* choose(n, k) */
  for (unsigned n = 0; n <= 60; n++) for (unsigned k = 0; k <= n + 2; k++) {
    gsl_sf_result r = {NAN, NAN};
    int st = gsl_sf_choose_e(n, k, &r);
    printf("choose %016llx %016llx %d %016llx %016llx\n", B((double)n), B((double)k), st, B(r.err), B(r.val));
  }
  {
    unsigned ns[] = {100, 150, 200, 500, 1000, 100000};
    unsigned ks[] = {0, 1, 2, 5, 20, 50, 100, 500};
    for (int a = 0; a < 6; a++) for (int b = 0; b < 8; b++) {
      gsl_sf_result r = {NAN, NAN};
      int st = gsl_sf_choose_e(ns[a], ks[b], &r);
      printf("choose %016llx %016llx %d %016llx %016llx\n", B((double)ns[a]), B((double)ks[b]), st, B(r.err), B(r.val));
    }
  }
  /* gegenbauer */
  {
    double lam[] = {-0.9, -0.5, -0.1, 0, 0.2, 0.5, 1, 2.5, 10, 100};
    double x[] = {-1.5, -1, -0.7, -0.1, 0, 0.3, 0.9, 1, 1.5, 10};
    run2("gegenpoly_1", gsl_sf_gegenpoly_1_e, lam, 10, x, 10);
    run2("gegenpoly_2", gsl_sf_gegenpoly_2_e, lam, 10, x, 10);
    run2("gegenpoly_3", gsl_sf_gegenpoly_3_e, lam, 10, x, 10);
    int ns[] = {-1, 0, 1, 2, 3, 4, 5, 10, 30, 100};
    for (int a = 0; a < 10; a++) for (int b = 0; b < 10; b++) for (int c = 0; c < 10; c++) {
      gsl_sf_result r = {NAN, NAN};
      int st = gsl_sf_gegenpoly_n_e(ns[a], lam[b], x[c], &r);
      printf("gegenpoly_n %016llx %016llx %016llx %d %016llx %016llx\n", B((double)ns[a]), B(lam[b]), B(x[c]), st, B(r.err), B(r.val));
    }
  }
  /* legendre Plm(l, m, x) */
  {
    double x[] = {-1.5, -1, -0.99999, -0.9, -0.5, -1e-3, 0, 1e-3, 0.3, 0.7, 0.99, 0.999999, 1, 1.5};
    int ls[] = {0, 1, 2, 3, 4, 5, 8, 12, 20, 30, 50, 100, 200};
    for (int a = 0; a < 13; a++)
      for (int m = 0; m <= ls[a] + 1; m += (ls[a] > 20 ? ls[a] / 7 : 1))
        for (int c = 0; c < 14; c++) {
          gsl_sf_result r = {NAN, NAN};
          int st = gsl_sf_legendre_Plm_e(ls[a], m, x[c], &r);
          printf("legendre_plm %016llx %016llx %016llx %d %016llx %016llx\n", B((double)ls[a]), B((double)m), B(x[c]), st, B(r.err), B(r.val));
        }
  }
  /* elliptic integrals; mode 0 = DOUBLE, 1 = SINGLE (APPROX is read identically to SINGLE by ellint.c) */
  {
    double v[] = {0, 1e-12, 1e-5, 0.01, 0.5, 1, 2, 3.5, 7, 25, 1e3, 1e8, 1e20};
    double w[] = {-1, -0.5, 0, 1e-12, 0.1, 1, 4, 60, 1e5};
    double phi[] = {-6, -3.2, -1.7, -1.5707963267948966, -0.7, 0, 1e-9, 0.3, 0.7853981633974483, 1.2,
                    1.5707963267948966, 1.6, 2.5, 3.1415926535897931, 4.7, 6.3, 100};
    double k[] = {-1.2, -0.9999999, -0.99, -0.7, -0.3, 0, 1e-9, 0.1, 0.5, 0.8, 0.95, 0.999, 0.9999999999, 1, 1.01};
    double nn[] = {-5, -1.5, -0.5, 0, 0.3, 0.9, 0.9999, 1, 1.5, 4};
    for (int mo = 0; mo < 2; mo++) {
      for (int i = 0; i < 13; i++) for (int j = 0; j < 9; j++) {
        gsl_sf_result r = {NAN, NAN};
        int st = gsl_sf_ellint_RC_e(v[i], w[j], mo, &r);
        printf("rc %016llx %016llx %d %d %016llx %016llx\n", B(v[i]), B(w[j]), mo, st, B(r.err), B(r.val));
      }
      for (int i = 0; i < 13; i++) for (int j = 0; j < 13; j++) for (int l = 0; l < 13; l++) {
        gsl_sf_result r = {NAN, NAN};
        int st = gsl_sf_ellint_RF_e(v[i], v[j], v[l], mo, &r);
        printf("rf %016llx %016llx %016llx %d %d %016llx %016llx\n", B(v[i]), B(v[j]), B(v[l]), mo, st, B(r.err), B(r.val));
        r.val = NAN;
        st = gsl_sf_ellint_RD_e(v[i], v[j], v[l], mo, &r);
        printf("rd %016llx %016llx %016llx %d %d %016llx %016llx\n", B(v[i]), B(v[j]), B(v[l]), mo, st, B(r.err), B(r.val));
      }
      for (int i = 0; i < 13; i += 2) for (int j = 0; j < 13; j += 2) for (int l = 0; l < 13; l += 2) for (int p = 0; p < 13; p++) {
        double pp = (p % 2 ? 1 : -1) * v[p];
        gsl_sf_result r = {NAN, NAN};
        int st = gsl_sf_ellint_RJ_e(v[i], v[j], v[l], pp, mo, &r);
        printf("rj %016llx %016llx %016llx %016llx %d %d %016llx %016llx\n", B(v[i]), B(v[j]), B(v[l]), B(pp), mo, st, B(r.err), B(r.val));
      }
      for (int i = 0; i < 17; i++) for (int j = 0; j < 15; j++) {
        gsl_sf_result r = {NAN, NAN};
        int st = gsl_sf_ellint_F_e(phi[i], k[j], mo, &r);
        printf("ellint_f %016llx %016llx %d %d %016llx %016llx\n", B(phi[i]), B(k[j]), mo, st, B(r.err), B(r.val));
        r.val = NAN;
        st = gsl_sf_ellint_E_e(phi[i], k[j], mo, &r);
        printf("ellint_e %016llx %016llx %d %d %016llx %016llx\n", B(phi[i]), B(k[j]), mo, st, B(r.err), B(r.val));
        r.val = NAN;
        st = gsl_sf_ellint_D_e(phi[i], k[j], mo, &r);
        printf("ellint_d %016llx %016llx %d %d %016llx %016llx\n", B(phi[i]), B(k[j]), mo, st, B(r.err), B(r.val));
        for (int p = 0; p < 10; p++) {
          r.val = NAN;
          st = gsl_sf_ellint_P_e(phi[i], k[j], nn[p], mo, &r);
          printf("ellint_p %016llx %016llx %016llx %d %d %016llx %016llx\n", B(phi[i]), B(k[j]), B(nn[p]), mo, st, B(r.err), B(r.val));
        }
      }
      for (int j = -1100; j <= 1100; j++) {
        double kk = j * 0.001;
        if (j > 1000) kk = 1 - pow(10, -(j - 1000) * 0.03);
        if (j < -1000) kk = -(1 - pow(10, -(-j - 1000) * 0.03));
        gsl_sf_result r = {NAN, NAN};
        int st = gsl_sf_ellint_Kcomp_e(kk, mo, &r);
        printf("ellint_kcomp %016llx %d %d %016llx %016llx\n", B(kk), mo, st, B(r.err), B(r.val));
        r.val = NAN;
        st = gsl_sf_ellint_Ecomp_e(kk, mo, &r);
        printf("ellint_ecomp %016llx %d %d %016llx %016llx\n", B(kk), mo, st, B(r.err), B(r.val));
        r.val = NAN;
        st = gsl_sf_ellint_Dcomp_e(kk, mo, &r);
        printf("ellint_dcomp %016llx %d %d %016llx %016llx\n", B(kk), mo, st, B(r.err), B(r.val));
        if (j % 20 == 0) for (int p = 0; p < 10; p++) {
          r.val = NAN;
          st = gsl_sf_ellint_Pcomp_e(kk, nn[p], mo, &r);
          printf("ellint_pcomp %016llx %016llx %d %d %016llx %016llx\n", B(kk), B(nn[p]), mo, st, B(r.err), B(r.val));
        }
      }
    }
    (void)w;
  }
  /* elljac(u, m) */
  {
    double u[] = {-50, -5, -2.5, -1, -0.3, 0, 1e-8, 0.1, 0.5, 1, 1.5, 2, 3, 5, 7.7, 12, 100, 1e5};
    double m[] = {-1.5, -1, -0.7, -1e-9, 0, 1e-10, 1e-6, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99, 0.999999, 1 - 1e-12, 1, 1.01};
    for (int i = 0; i < 18; i++) for (int j = 0; j < 17; j++) {
      double sn = NAN, cn = NAN, dn = NAN;
      int st = gsl_sf_elljac_e(u[i], m[j], &sn, &cn, &dn);
      printf("elljac %016llx %016llx %d 0000000000000000 %016llx %016llx %016llx\n", B(u[i]), B(m[j]), st, B(sn), B(cn), B(dn));
    }
  }
  /* trig: in-place angle reduction */
  for (int i = 0; i < n1; i++) {
    double t = G1[i];
    int st = gsl_sf_angle_restrict_pos_e(&t);
    printf("angle_restrict_pos %016llx %d 0000000000000000 %016llx\n", B(G1[i]), st, B(t));
    t = G1[i];
    st = gsl_sf_angle_restrict_symm_e(&t);
    printf("angle_restrict_symm %016llx %d 0000000000000000 %016llx\n", B(G1[i]), st, B(t));
  }
  return 0;
}
