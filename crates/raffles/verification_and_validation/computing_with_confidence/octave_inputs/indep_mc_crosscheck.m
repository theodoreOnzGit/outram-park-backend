% Secondary cross-check of the independence results of Case studies 1 and 2
% (Pressure tank, TRIGA) by plain Monte Carlo, after the structure of Adolphus
% Lye's Supplementary Files/*_Indep.m (https://github.com/Adolphus8/
% Computing-with-Confidence).  Run:   octave --no-gui indep_mc_crosscheck.m
%
% Why not run the original .m files: (1) betarnd lives in Octave's `statistics`
% package, which is not installed here (no packages are installed system-wide);
% it is replaced below by the gamma-ratio construction, Beta(a,b) = X/(X+Y),
% X~Gamma(a), Y~Gamma(b), with the pba.r conventions Beta(0,b) = 0 and
% Beta(a,0) = 1; (2) Octave scripts need functions defined before use.
% (3) The original Pressure_vessel file uses KN(2,2000) for the tank where the
% R script uses KN(0,2000); the R parameters are used here so the number
% compared with Rust and with R is the same quantity.
%
% Estimator: for each of NREP independent replicates of Nsamps draws, the
% system c-box edges are the system function of the left-edge and of the
% right-edge samples; the 95 % end points are prctile(left, 2.5) and
% prctile(right, 97.5).  The mean and standard deviation over replicates give
% the Monte Carlo noise, from which the pass band is set (not from Rust).
Nsamps = 100000; NREP = 20;
rand('state', 20261020); randn('state', 20261020);
function x = bet(a, b, n)
  if a == 0, x = zeros(n,1); elseif b == 0, x = ones(n,1);
  else, g1 = randg(a, n, 1); g2 = randg(b, n, 1); x = g1 ./ (g1 + g2); end
end
function c = kn(k, n, N)
  c = [bet(k, n - k + 1, N), bet(k + 1, n - k, N)];
end
orI  = @(x,y) 1 - (1 - x).*(1 - y);
andI = @(x,y) x.*y;

res1 = zeros(NREP,2); res2 = zeros(NREP,2);
for r = 1:NREP
  % Case study 1: E1 = T or K2 or (S and (S1 or K1 or R))
  p = {[0 2000],[3 500],[1 150],[0 460],[7 10000],[0 380]};
  C = cell(1,6); for i=1:6, C{i} = kn(p{i}(1), p{i}(2), Nsamps); end
  top = @(s) orI(orI(s(:,1), s(:,2)), andI(s(:,3), orI(orI(s(:,6), s(:,5)), s(:,4))));
  L = top([C{1}(:,1) C{2}(:,1) C{3}(:,1) C{4}(:,1) C{5}(:,1) C{6}(:,1)]);
  R = top([C{1}(:,2) C{2}(:,2) C{3}(:,2) C{4}(:,2) C{5}(:,2) C{6}(:,2)]);
  res1(r,:) = [prctile(L, 2.5), prctile(R, 97.5)];
  % Case study 2: E1 = p and ((v3 or ov3) or (v4 or ov4))
  q = {[1 42],[8 42],[8 42]};
  D = cell(1,3); for i=1:3, D{i} = kn(q{i}(1), q{i}(2), Nsamps); end
  ov = 3.49e-5 * ones(Nsamps,1);
  top2 = @(a,b,c) andI(a, orI(orI(b, ov), orI(c, ov)));
  L2 = top2(D{1}(:,1), D{2}(:,1), D{3}(:,1));
  R2 = top2(D{1}(:,2), D{2}(:,2), D{3}(:,2));
  res2(r,:) = [prctile(L2, 2.5), prctile(R2, 97.5)];
end
printf('case1_e1i_lower %.10g %.10g\n', mean(res1(:,1)), std(res1(:,1)));
printf('case1_e1i_upper %.10g %.10g\n', mean(res1(:,2)), std(res1(:,2)));
printf('case2_e1i_lower %.10g %.10g\n', mean(res2(:,1)), std(res2(:,1)));
printf('case2_e1i_upper %.10g %.10g\n', mean(res2(:,2)), std(res2(:,2)));
printf('OCTAVE_VERSION %s NREP %d Nsamps %d\n', version(), NREP, Nsamps);
