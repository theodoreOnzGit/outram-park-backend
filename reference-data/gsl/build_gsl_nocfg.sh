#!/bin/bash
# Configure-free build of the GSL 2.8 pieces the integration drivers need
# (gh:#1340 round 3). Equivalent to build_gsl_family.sh's autoconf recipe for
# x86-64 glibc, but runs no script from the cloned tree: it writes a small
# config.h, links the public headers into an include tree and compiles the
# sources of integration/, err/, sys/ and specfunc/ with `gcc -O2`.
#
# usage (from the repository root):
#   reference-data/gsl/build_gsl_nocfg.sh <build-dir>          # once
#   reference-data/gsl/build_gsl_nocfg.sh <build-dir> <driver.c> <out.txt> [gcc args]
set -e
G=$(pwd)/crates/petir/upstream_source/GSL
B=$1; shift
if [ ! -f "$B/libgslnocfg.a" ]; then
  mkdir -p "$B/inc/gsl" "$B/obj"
  for h in "$G"/*/gsl_*.h "$G"/gsl_*.h "$G"/templates_*.h "$G"/build.h; do ln -sf "$h" "$B/inc/gsl/$(basename "$h")"; done
  ln -sf "$G/gsl_inline.h" "$B/inc/gsl/gsl_inline.h"
  ln -sf "$G/build.h" "$B/inc/gsl/build.h"
  cat > "$B/inc/config.h" <<'CFG'
#define HAVE_DECL_ACOSH 1
#define HAVE_DECL_ASINH 1
#define HAVE_DECL_ATANH 1
#define HAVE_DECL_EXPM1 1
#define HAVE_DECL_FINITE 1
#define HAVE_DECL_FREXP 1
#define HAVE_DECL_HYPOT 1
#define HAVE_DECL_ISFINITE 1
#define HAVE_DECL_ISINF 1
#define HAVE_DECL_ISNAN 1
#define HAVE_DECL_LDEXP 1
#define HAVE_DECL_LOG1P 1
#define HAVE_EXIT_SUCCESS_AND_FAILURE 1
#define HAVE_IEEE_COMPARISONS 1
#define HAVE_IEEE_DENORMALS 1
#define HAVE_MEMCPY 1
#define HAVE_MEMMOVE 1
#define HAVE_STRDUP 1
#define HAVE_VPRINTF 1
#define STDC_HEADERS 1
#define RETURN_IF_NULL(x) if (!x) { return ; }
CFG
  for d in integration err sys specfunc; do
    # the library's own source list from Makefile.am (some .c files are
    # #included by others and do not compile alone)
    srcs=$(sed -e ':a' -e '/\\$/N; s/\\\n//; ta' "$G/$d/Makefile.am" | grep -E '^libgsl[a-z]*_la_SOURCES' | sed 's/^[^=]*=//' | tr ' \t' '\n\n' | grep '\.c$')
    for n in $srcs; do
      gcc -O2 -w -I"$B/inc" -I"$G" -I"$G/$d" -c "$G/$d/$n" -o "$B/obj/${d}_${n%.c}.o"
    done
  done
  ar rcs "$B/libgslnocfg.a" "$B"/obj/*.o
fi
if [ $# -ge 2 ]; then
  driver=$1; out=$2; shift 2
  exe=$(mktemp)
  gcc -O2 -I"$B/inc" -I"$G" -o "$exe" "$driver" "$B/libgslnocfg.a" "$@" -lm
  "$exe" > "$out"
  rm -f "$exe"
fi
