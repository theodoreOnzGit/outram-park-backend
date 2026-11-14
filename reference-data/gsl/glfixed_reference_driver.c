/* glfixed_reference_driver.c -- compiled-GSL reference for
 * petir::integration::{glfixed_table_alloc, glfixed, glfixed_point}
 * (gh:#1340).
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Output, one record per line, doubles in %.17e:
 *   "TBL <n> <precomputed> <m> x[0..m) w[0..m)"       the stored table
 *   "INT <n> <name> <a> <b> <value>"                  gsl_integration_glfixed
 *   "PT <n> <a> <b> <i> <status> <xi> <wi>"           glfixed_point, all i
 *   "PTERR <n> <status>"                              glfixed_point, i = n
 * Table sizes: every precomputed n (2..20, 32, 64, 96, 100, 128, 256, 512,
 * 1024) and generated ones (1, 21, 22, 25, 33, 50, 63, 101, 200, 513, 1023,
 * 1025, 2000). The on-the-fly Newton start uses cos(), so a generated table
 * can differ from PETIR's by round-off; the record counts it.
 *
 * Build: reference-data/gsl/build_gsl_family.sh glfixed_reference_driver.c out.txt
 */
#include <math.h>
#include <stdio.h>
#include <gsl/gsl_errno.h>
#include <gsl/gsl_integration.h>

static double f_poly(double x, void *p) { (void) p; return x * x * x * x * x - 2.0 * x + 1.0; }
static double f_exp(double x, void *p) { (void) p; return exp(x); }
static double f_cos(double x, void *p) { (void) p; return cos(7.0 * x); }
static double f_rat(double x, void *p) { (void) p; return 1.0 / (1.0 + 25.0 * x * x); }

int main(void)
{
  size_t sizes[] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
                    21, 22, 25, 32, 33, 50, 63, 64, 96, 100, 101, 128, 200, 256, 512, 513,
                    1023, 1024, 1025, 2000};
  size_t ns = sizeof(sizes) / sizeof(sizes[0]);
  gsl_function fs[4];
  const char *names[4] = {"poly", "exp", "cos7", "rational"};
  size_t s, k, i;
  fs[0].function = f_poly; fs[1].function = f_exp; fs[2].function = f_cos; fs[3].function = f_rat;
  for (k = 0; k < 4; k++) fs[k].params = NULL;
  gsl_set_error_handler_off();

  for (s = 0; s < ns; s++)
    {
      size_t n = sizes[s];
      gsl_integration_glfixed_table *t = gsl_integration_glfixed_table_alloc(n);
      size_t m = (n + 1) >> 1;
      printf("TBL %zu %d %zu", n, (int) t->precomputed, m);
      for (i = 0; i < m; i++) printf(" %.17e", t->x[i]);
      for (i = 0; i < m; i++) printf(" %.17e", t->w[i]);
      printf("\n");
      for (k = 0; k < 4; k++)
        {
          printf("INT %zu %s -1 1 %.17e\n", n, names[k], gsl_integration_glfixed(&fs[k], -1.0, 1.0, t));
          printf("INT %zu %s 0.5 3.25 %.17e\n", n, names[k], gsl_integration_glfixed(&fs[k], 0.5, 3.25, t));
        }
      if (n <= 25 || n == 101 || n == 1025)
        {
          for (i = 0; i < n; i++)
            {
              double xi = 0, wi = 0;
              int st = gsl_integration_glfixed_point(-0.5, 2.0, i, &xi, &wi, t);
              printf("PT %zu -0.5 2 %zu %d %.17e %.17e\n", n, i, st, xi, wi);
            }
        }
      {
        double xi = 0, wi = 0;
        int st = gsl_integration_glfixed_point(0.0, 1.0, n, &xi, &wi, t);
        printf("PTERR %zu %d\n", n, st);
      }
      gsl_integration_glfixed_table_free(t);
    }
  return 0;
}
