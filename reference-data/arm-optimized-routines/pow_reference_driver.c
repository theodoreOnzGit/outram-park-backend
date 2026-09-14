/* pow_reference_driver.c -- oracle generator for PETIR's `fast_pow` port.
 *
 * Compiles ARM optimized-routines' own `math/pow.c` + `math/pow_log_data.c`
 * + `math/exp_data.c` (pow reuses the exp table) and dumps `pow` over a fixed
 * probe grid as raw IEEE-754 bit patterns.
 *
 * IMPORTANT -- build WITHOUT FMA (no `-mfma`, no `-march=native`).  See this
 * directory's README: `HAVE_FAST_FMA` selects a different argument reduction
 * in `log_inline` and a different `hi`/`lo` split, on top of the polynomial
 * contraction.  The portable branch is what `src/fast_pow.rs` transcribes.
 *
 * The upstream symbol is `pow`, which collides with <math.h>; objcopy renames
 * it after compilation (see the README's regeneration recipe).
 */
#include <stdio.h>
#include <stdint.h>
#include <string.h>

double arm_pow (double, double);

static uint64_t bits (double x) { uint64_t u; memcpy (&u, &x, 8); return u; }

static void emit (double x, double y)
{
  printf ("%016llx %016llx %016llx\n", (unsigned long long) bits (x),
          (unsigned long long) bits (y),
          (unsigned long long) bits (arm_pow (x, y)));
}

int
main (void)
{
  int i, j, e;

  puts ("# PETIR fast_pow oracle -- ARM optimized-routines math/pow.c, built without FMA.");
  puts ("# Columns: <x bits> <y bits> <pow(x,y) bits>, all hex64.  One probe per line.");

  /* The bulk: positive bases against a spread of exponents, both signs. */
  for (i = 1; i <= 400; i++)
    {
      double x = (double) i * 0.37;
      for (j = -40; j <= 40; j++)
        emit (x, (double) j * 0.83);
    }

  /* Negative bases.  Integer exponents are the interesting case -- they are
     the ones with an answer, routed through `checkint` and `SIGN_BIAS`.  A
     few non-integer exponents are included as well so the invalid-operand
     path is checked rather than assumed. */
  for (i = 1; i <= 120; i++)
    {
      double x = -(double) i * 0.37;
      for (j = -25; j <= 25; j++)
        emit (x, (double) j);
      emit (x, 0.5);
      emit (x, 1.5);
      emit (x, -2.5);
    }

  /* Bases near 1, where `log_inline`'s extra precision is what keeps `pow`
     accurate -- this is the regime that makes pow hard. */
  for (i = -300; i <= 300; i++)
    {
      double x = 1.0 + (double) i * 1.0e-4;
      emit (x, 1000.0);
      emit (x, -1000.0);
      emit (x, 1.0e6);
      emit (x, 0.5);
    }

  /* Integer and half-integer exponents, incl. the odd/even sign path for a
     negative base (`checkint`). */
  for (i = -60; i <= 60; i++)
    {
      double y = (double) i;
      emit (2.0, y);  emit (-2.0, y);
      emit (0.5, y);  emit (-0.5, y);
      emit (10.0, y); emit (-10.0, y);
      emit (3.0, y + 0.5);
    }

  /* Wide exponent range on the base, crossing subnormal normalisation and the
     overflow/underflow edges of the result. */
  for (e = -1070; e <= 1020; e += 7)
    {
      double s = 1.0;
      int t;
      if (e >= 0) for (t = 0; t < e; t++) s *= 2.0;
      else        for (t = 0; t < -e; t++) s *= 0.5;
      if (s == 0.0) continue;
      emit (s, 0.25); emit (s, 1.5); emit (s, -0.75); emit (s, 3.0);
    }

  /* The IEEE special cases the prologue handles before any polynomial runs. */
  {
    static const double sx[] = { 0.0, -0.0, 1.0, -1.0, 2.0, -2.0, 0.5,
                                 1.0 / 0.0, -1.0 / 0.0 };
    static const double sy[] = { 0.0, -0.0, 1.0, -1.0, 2.0, -3.0, 0.5, -0.5,
                                 1.0 / 0.0, -1.0 / 0.0, 1e-20, 1e20 };
    unsigned a, b;
    for (a = 0; a < sizeof sx / sizeof *sx; a++)
      for (b = 0; b < sizeof sy / sizeof *sy; b++)
        emit (sx[a], sy[b]);
  }

  return 0;
}
