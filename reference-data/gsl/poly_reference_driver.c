/* Reference driver: GSL 2.8 polynomial evaluation and divided differences
 * (poly/gsl_poly.h inline gsl_poly_eval; poly/deriv.c gsl_poly_eval_derivs;
 * poly/dd.c gsl_poly_dd_init, gsl_poly_dd_taylor; gsl_poly_dd_eval inline),
 * for petir::poly::eval, petir::poly::eval_derivs and
 * petir::poly::DividedDifference.
 *
 * Not emitted, because PETIR does not port them: gsl_poly_complex_eval,
 * gsl_complex_poly_complex_eval, and gsl_poly_dd_hermite_init.
 *
 * Coefficients and abscissae are exact rationals (small integers and
 * dyadic fractions, or single correctly-rounded divisions); no libm is
 * called, so the inputs do not depend on the platform libm.
 *
 * Output, per case: CASE <name>, then the inputs (C, XA, YA, and the
 * evaluation points), then the outputs as "<TAG> <index> <hex> <dec>" lines,
 * then END <name>. Every value is written as its IEEE-754 bit pattern.
 */
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <stddef.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_poly.h>

static void hexdec(const char *tag, size_t i, double v)
{
  uint64_t bits;
  memcpy(&bits, &v, sizeof bits);
  printf("%s %zu %016llx %.17e\n", tag, i, (unsigned long long)bits, v);
}

static void emit_vec(const char *tag, const double *v, size_t n)
{
  size_t i;
  for (i = 0; i < n; i++)
    hexdec(tag, i, v[i]);
}

/* ---- gsl_poly_eval and gsl_poly_eval_derivs ------------------------------ */

static void emit_eval(const char *name, const double *c, int len,
                      const double *xs, size_t nx)
{
  size_t k;
  printf("CASE %s\n", name);
  emit_vec("C", c, (size_t)len);
  emit_vec("XS", xs, nx);
  for (k = 0; k < nx; k++)
    hexdec("EVAL", k, gsl_poly_eval(c, len, xs[k]));
  printf("END %s\n", name);
}

static void emit_derivs(const char *name, const double *c, size_t lenc,
                        double x, size_t lenres)
{
  double res[16];
  size_t i;
  printf("CASE %s\n", name);
  emit_vec("C", c, lenc);
  hexdec("XS", 0, x);
  printf("LENRES %zu\n", lenres);
  gsl_poly_eval_derivs(c, lenc, x, res, lenres);
  for (i = 0; i < lenres; i++)
    hexdec("DERIV", i, res[i]);
  printf("END %s\n", name);
}

/* ---- divided differences ------------------------------------------------- */

static void emit_dd(const char *name, const double *xa, const double *ya,
                    size_t n, const double *xs, size_t nx, const double *xps,
                    size_t nxp)
{
  double dd[32], w[32], c[32];
  size_t k, i;

  printf("CASE %s %zu\n", name, n);
  emit_vec("XA", xa, n);
  emit_vec("YA", ya, n);
  emit_vec("XS", xs, nx);
  emit_vec("XP", xps, nxp);

  gsl_poly_dd_init(dd, xa, ya, n);
  emit_vec("DD", dd, n);

  for (k = 0; k < nx; k++)
    hexdec("EVAL", k, gsl_poly_dd_eval(dd, xa, n, xs[k]));

  for (i = 0; i < nxp; i++) {
    gsl_poly_dd_taylor(c, xps[i], dd, xa, n, w);
    emit_vec("TAYLOR", c, n);
    printf("TAYLOR_END %zu\n", i);
  }
  printf("END %s\n", name);
}

int main(void)
{
  gsl_set_error_handler_off();

  /* --- gsl_poly_eval --- */
  {
    static const double c[] = {1.0, -2.0, 3.0, -4.0, 5.0};
    static const double xs[] = {0.0, 1.0, -1.0, 0.5, -0.375, 2.75, 1e-3, -1e3};
    emit_eval("eval_quartic", c, 5, xs, sizeof xs / sizeof xs[0]);
  }
  {
    static const double c[] = {2.5};
    static const double xs[] = {0.0, -7.0, 1e300};
    emit_eval("eval_constant", c, 1, xs, sizeof xs / sizeof xs[0]);
  }
  {
    /* Deterministic dyadic coefficients, degree 19 (20 terms). */
    double c[20];
    double xs[7] = {0.0, 0.25, -0.75, 1.0, -1.0, 0.9, -0.99};
    int i;
    for (i = 0; i < 20; i++)
      c[i] = (double)((i * 37) % 17 - 8) / (double)(1 << (i % 5 + 1));
    emit_eval("eval_deg19", c, 20, xs, 7);
  }

  /* --- gsl_poly_eval_derivs --- */
  {
    static const double c[] = {1.0, -2.0, 3.0, -4.0, 5.0};
    double x;
    for (x = -1.5; x <= 1.5 + 1e-12; x += 0.5) {
      char name[32];
      snprintf(name, sizeof name, "derivs_quartic_x%+.1f", x);
      emit_derivs(name, c, 5, x, 5);
    }
    /* lenres larger than lenc: the derivatives past the degree are zero. */
    emit_derivs("derivs_quartic_long", c, 5, 0.75, 8);
    /* lenres smaller than lenc. */
    emit_derivs("derivs_quartic_short", c, 5, -0.25, 3);
  }
  {
    double c[12];
    int i;
    for (i = 0; i < 12; i++)
      c[i] = (double)((i * 11) % 7 - 3) / (double)(i + 2);
    emit_derivs("derivs_deg11_x0.3", c, 12, 0.3, 12);
    emit_derivs("derivs_deg11_xm0.9", c, 12, -0.9, 12);
  }

  /* --- divided differences --- */
  {
    static const double xa[] = {0.0, 1.0, 2.0};
    static const double ya[] = {1.0, 3.0, 7.0};
    static const double xs[] = {-1.0, 0.5, 3.0, 10.0};
    static const double xps[] = {0.0, 1.5};
    emit_dd("dd_quadratic", xa, ya, 3, xs, 4, xps, 2);
  }
  {
    /* Non-uniform nodes, degree 5. */
    static const double xa[] = {-2.0, -0.5, 0.0, 0.75, 1.25, 3.0};
    static const double ya[] = {4.0, -1.0, 0.25, 2.0, -3.5, 0.125};
    static const double xs[] = {-2.5, -1.0, 0.1, 1.0, 2.0, 4.0};
    static const double xps[] = {0.0, -1.0, 0.5};
    emit_dd("dd_nonuniform6", xa, ya, 6, xs, 6, xps, 3);
  }
  {
    /* Runge's function 1/(1+25x^2) on 11 equispaced nodes in [-1, 1]:
     * every value is one correctly rounded division. */
    double xa[11], ya[11], xs[8], xps[2];
    int i;
    for (i = 0; i < 11; i++) {
      xa[i] = -1.0 + 0.2 * (double)i;
      ya[i] = 1.0 / (1.0 + 25.0 * xa[i] * xa[i]);
    }
    for (i = 0; i < 8; i++)
      xs[i] = -0.95 + 0.27 * (double)i;
    xps[0] = 0.0;
    xps[1] = 0.6;
    emit_dd("dd_runge11", xa, ya, 11, xs, 8, xps, 2);
  }
  {
    /* A pseudo-random set of 10 nodes, in a shuffled order, so the Newton
     * table does not depend on sorted abscissae. */
    double xa[10], ya[10], xs[5], xps[1];
    int i;
    static const int perm[10] = {7, 2, 9, 0, 5, 3, 8, 1, 6, 4};
    for (i = 0; i < 10; i++) {
      int k = perm[i];
      xa[i] = (double)(k * 13 % 17) / 8.0 - 1.0;
      ya[i] = (double)((k * 29 + 5) % 23) / 7.0 - 1.5;
    }
    for (i = 0; i < 5; i++)
      xs[i] = -0.8 + 0.4 * (double)i;
    xps[0] = 0.125;
    emit_dd("dd_shuffled10", xa, ya, 10, xs, 5, xps, 1);
  }

  return 0;
}
