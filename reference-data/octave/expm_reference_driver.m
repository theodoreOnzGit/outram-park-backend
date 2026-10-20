## Reference driver: GNU Octave's expm, for petir::linalg::expm (gh:#993).
##
## Run with GNU Octave 11.3.0 from the repository root:
##   octave-cli --no-gui reference-data/octave/expm_reference_driver.m \
##       > reference-data/octave/expm-octave-11.3.0-reference.txt
##
## For each case it prints the input A, stock expm(A) as R, and expm_nobal(A)
## as RNB: a copy of Octave 11.3.0's scripts/linear-algebra/expm.m
## (Copyright (C) 2008-2026 The Octave Project Developers, GPL-3.0-or-later)
## whose ONLY change is that the balancing step is removed (d = ones,
## p = 1:n, aa = A) together with its inverse -- the deviation PETIR's port
## makes, because Octave's balance() is LAPACK dgebal. Comparing PETIR with
## RNB measures the port; comparing RNB with R measures what balancing does.
##
## Output format (%.17e): CASE <name> <n>, A <i> <j> <v>, R <i> <j> <v>,
## RNB <i> <j> <v>, END <name>; indices zero-based.

1;

function r = expm_nobal (A)
  ## expm.m, Octave 11.3.0, with the balance step removed. Everything else
  ## is verbatim.
  if (isempty (A))
    r = A;
    return;
  elseif (isscalar (A))
    r = exp (A);
    return;
  elseif (isdiag (A))
    r = diag (exp (diag (A)));
    return;
  endif

  n = rows (A);
  id = eye (n);
  ## Trace reduction.
  A(A == -Inf) = -realmax ();
  trshift = trace (A) / n;
  if (trshift > 0)
    A -= trshift * id;
  endif
  ## Balancing: REMOVED (d = ones, p = 1:n).
  aa = A;
  [~, e] = log2 (norm (aa, "inf"));
  s = max (0, e);
  s = min (s, 1023);
  aa *= 2^(-s);

  ## Pade approximation for exp(A).
  c = [5.0000000000000000e-1, ...
       1.1666666666666667e-1, ...
       1.6666666666666667e-2, ...
       1.6025641025641026e-3, ...
       1.0683760683760684e-4, ...
       4.8562548562548563e-6, ...
       1.3875013875013875e-7, ...
       1.9270852604185938e-9];

  a2 = aa^2;
  x = (((c(8) * a2 + c(6) * id) * a2 + c(4) * id) * a2 + c(2) * id) * a2 + id;
  y = (((c(7) * a2 + c(5) * id) * a2 + c(3) * id) * a2 + c(1) * id) * aa;

  r = (x - y) \ (x + y);

  ## Undo scaling by repeated squaring.
  for k = 1:s
    r ^= 2;
  endfor

  ## Inverse balancing: REMOVED.
  ## Inverse trace reduction.
  if (trshift > 0)
    r *= exp (trshift);
  endif
endfunction

function emit (name, A)
  n = rows (A);
  printf ("CASE %s %d\n", name, n);
  for i = 1:n
    for j = 1:n
      printf ("A %d %d %.17e\n", i - 1, j - 1, A(i, j));
    endfor
  endfor
  R = expm (A);
  for i = 1:n
    for j = 1:n
      printf ("R %d %d %.17e\n", i - 1, j - 1, R(i, j));
    endfor
  endfor
  R = expm_nobal (A);
  for i = 1:n
    for j = 1:n
      printf ("RNB %d %d %.17e\n", i - 1, j - 1, R(i, j));
    endfor
  endfor
  printf ("END %s\n", name);
endfunction

## expm.m's own test matrices
emit ("ward2", [1 -1; 0 1]);
emit ("ward3", [1 -1 -1; 0 1 -1; 0 0 1]);
## scalar and diagonal special cases
emit ("scalar", 2.5);
emit ("diag3", diag ([1 -2 3]));
## rotation generator (trace 0), and a large one (many squarings)
emit ("rot2", [0 2; -2 0]);
emit ("bigrot2", 40 * [0 1; -1 0]);
## positive trace: the trace-shift branch
emit ("postrace2", [2 1; 0.5 3]);
## small norm: s = 0
emit ("tiny2", 1e-3 * [1 2; 3 -4]);
## strongly non-normal
emit ("nonnormal2", [-1 100; 0 -2]);
## a 4-nuclide decay chain times a step: depletion's use case (cf. CRAM)
l = [1e-1, 3e-2, 5e-3, 0];
D = diag (-l) + diag (l(1:3), -1);
emit ("chain4", D * 10);
## badly scaled: where balancing matters
emit ("scaled3", [1 1e6 0; 1e-6 2 1e3; 0 1e-3 -1]);
## symmetric: Octave's \ tries Cholesky on x - y here
S = [4 1 0 0; 1 3 1 0; 0 1 2 1; 0 0 1 1] / 2;
emit ("symm4", S);
## deterministic dense matrices
B = zeros (6);
for i = 1:6
  for j = 1:6
    B(i, j) = sin (i * j + i);
  endfor
endfor
emit ("dense6", B);
C = zeros (10);
for i = 1:10
  for j = 1:10
    C(i, j) = cos (3 * i - 2 * j) / 2;
  endfor
endfor
emit ("dense10", C);
