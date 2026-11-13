% Differential driver: GNU Octave control package vs z_domain (gh:#1433, epic #1336).
%
% Run:  octave-cli --no-gui octave_driver.m > octave_reference.csv
% Octave 8.4.0 + control 4.0.1 (Ubuntu apt octave-control).  NOTE the port cites
% control 4.2.2+ commit f39e2625; the forge package could not be fetched (proxy),
% so the reference is 4.0.1 -- see README.md.
%
% Output is one CSV-ish line per record:  tag|name|key|v1 v2 v3 ...
% every number is printed with %.17g (round-trips an IEEE double).
% No input is a hidden random draw: the inputs are PRINTED (tag `sysd`, `u`, `sysc`)
% and the Rust test reads them back, so the Rust side never re-generates them.
pkg load control
% Optional: OCTAVE_OVERRIDE_DIR holds an @tf/ with __c2d__.m / __d2c__.m taken from
% pkg-control 4.3.0 (7773b7b1); addpath-ed so those two methods shadow the installed 4.0.1.
ovr = getenv('OCTAVE_OVERRIDE_DIR');
if ~isempty(ovr), addpath(ovr); end
rand('twister', 1433);
randn('twister', 1433);

function p(tag, name, key, v)
  printf('%s|%s|%s|', tag, name, key);
  if ~isempty(v), printf('%.17g ', v); end
  printf('\n');
end

% ---- replica of the PORT's recurrence operation order (discrete_tf.rs:197) ----
function y = port_order(b, a, u)
  k = 1 / a(1); b = b * k; a = a * k;       % polynomial::scale(.., 1.0 / a0)
  n = max(numel(b), numel(a)) - 1;
  b(end+1:n+1) = 0; a(end+1:n+1) = 0;
  s = zeros(1, n); y = zeros(size(u));
  for t = 1:numel(u)
    if n >= 1, yy = b(1) * u(t) + s(1); else yy = b(1) * u(t); end
    for i = 1:n
      if i < n, carry = s(i+1); else carry = 0; end
      s(i) = b(i+1) * u(t) + carry - a(i+1) * yy;
    end
    y(t) = yy;
  end
end

% ---- replica of liboctave filter.cc DF2T operation order (psi[j+1] - a*y + b*x) ----
function y = octave_order(b, a, u)
  b = b / a(1); a = a / a(1);                % division, not multiply by reciprocal
  n = max(numel(b), numel(a)) - 1;
  b(end+1:n+1) = 0; a(end+1:n+1) = 0;
  s = zeros(1, n); y = zeros(size(u));
  for t = 1:numel(u)
    if n >= 1, yy = s(1) + b(1) * u(t); else yy = b(1) * u(t); end
    for i = 1:n
      if i < n
        s(i) = s(i+1) - a(i+1) * yy + b(i+1) * u(t);
      else
        s(i) = b(i+1) * u(t) - a(i+1) * yy;
      end
    end
    y(t) = yy;
  end
end

function s = ifelse_str(c)
  if c, s = '_unstable'; else s = ''; end
end

N = 40;
% ------------------------------------------------------------------ discrete set
names = {}; B = {}; A = {}; TS = [];
names{end+1} = 'd01_filt_doc'; B{end+1} = [0 3]; A{end+1} = [1 4 2]; TS(end+1) = 1;
for k = 1:10
  ord = 1 + mod(k, 4);                          % orders 2,3,4,1,2,3,4,1,2,3
  if k <= 8, rmax = 0.9; else rmax = 1.15; end % k = 9,10 are unstable
  r = rmax * (0.2 + 0.8 * rand(1, ord)) .* sign(randn(1, ord));
  a = poly(r);                                  % a0 = 1
  b = round(1000 * randn(1, ord + 1)) / 1000;
  b(1) = b(1) * (rand() > 0.5);                 % sometimes a delayed numerator
  names{end+1} = sprintf('d%02d_rand_ord%d%s', k + 1, ord, ifelse_str(k > 8)); B{end+1} = b; A{end+1} = a; TS(end+1) = [1 0.5](1 + mod(k, 2));
end
names{end+1} = 'd12_a0_3'; B{end+1} = [1 0.5]; A{end+1} = [3 -0.6 0.08]; TS(end+1) = 1;
names{end+1} = 'd13_a0_0p7'; B{end+1} = [0.2 0.1 0.05]; A{end+1} = [0.7 -0.5 0.06]; TS(end+1) = 0.5;

urand = 2 * rand(1, N) - 1;
p('u', 'rand', 'u', urand);
p('u', 'step', 'u', ones(1, N));
imp = zeros(1, N); imp(1) = 1;
p('u', 'impulse', 'u', imp);

for k = 1:numel(names)
  b = B{k}; a = A{k}; T = TS(k);
  p('sysd', names{k}, 'b_zinv', b); p('sysd', names{k}, 'a_zinv', a); p('sysd', names{k}, 'T', T);
  sys = filt(b, a, T);
  t = (0:N-1) * T;
  ins = {'rand', urand; 'step', ones(1, N); 'impulse', imp};
  for j = 1:3
    nm = ins{j, 1}; u = ins{j, 2};
    p('filter',  names{k}, nm, filter(b, a, u));
    p('portrep', names{k}, nm, port_order(b, a, u));
    p('octrep',  names{k}, nm, octave_order(b, a, u));
    p('lsim',    names{k}, nm, lsim(sys, u(:), t(:))');
  end
  p('step_fn',    names{k}, 'step',    step(sys, t(:))');
  p('impulse_fn', names{k}, 'impulse', impulse(sys, t(:))');
end

% ------------------------------------------------------------------ continuous set
cn = {}; CN = {}; CD = {};
cn{end+1} = 'c01_first_K2_tau3';      CN{end+1} = 2;            CD{end+1} = [1 3];
cn{end+1} = 'c02_first_K1_tau0p2';    CN{end+1} = 1;            CD{end+1} = [1 0.2];
cn{end+1} = 'c03_second_zeta0p3';     CN{end+1} = 1.5;          CD{end+1} = [1 2*0.3*2 4];
cn{end+1} = 'c04_second_critical';    CN{end+1} = 1;            CD{end+1} = [1 2*1*2 4];
cn{end+1} = 'c05_second_overdamped';  CN{end+1} = 1;            CD{end+1} = [1 2*2.5*2 4];
cn{end+1} = 'c06_first_with_zero';    CN{end+1} = [1 2];        CD{end+1} = [1 3];
cn{end+1} = 'c07_second_with_zero';   CN{end+1} = [1 0.5];      CD{end+1} = [1 1.2 0.8];
cn{end+1} = 'c08_third_order';        CN{end+1} = 1;            CD{end+1} = conv(conv([1 1], [1 2]), [1 3]);
cn{end+1} = 'c09_integrator_lag';     CN{end+1} = 1;            CD{end+1} = [0 1 2];   % 1/(s(1+2s))
Tc = [0.1 0.5];
methods = {'zoh', 'tustin', 'prewarp', 'matched'};
for k = 1:numel(cn)
  % port convention: ascending powers of s.  Octave: descending.
  p('sysc', cn{k}, 'num_asc', CN{k}); p('sysc', cn{k}, 'den_asc', CD{k});
  G = tf(fliplr(CN{k}), fliplr(CD{k}));
  p('dcgain', cn{k}, 'dcgain', dcgain(G));
  for ti = 1:numel(Tc)
    T = Tc(ti);
    for mi = 1:numel(methods)
      m = methods{mi}; w0 = 1.0;
      key = sprintf('%s|T=%g', m, T);
      try
        if strcmp(m, 'prewarp'), Gd = c2d(G, T, 'prewarp', w0); else Gd = c2d(G, T, m); end
        [n, d] = tfdata(Gd, 'vector');
        p('c2d_num', cn{k}, key, n); p('c2d_den', cn{k}, key, d);
      catch err
        printf('c2d_err|%s|%s|%s\n', cn{k}, key, strrep(err.message, "\n", ' '));
      end
    end
  end
end

% ------------------------------------------------------------------ d2c set
en = {'e1_first', 'e2_second', 'e3_second_b', 'e4_third'};
EB = {[0.3], [0.1 0.05], [0.2 0.1 0.05], [0.1 0.2 0.1]};
EA = {[1 -0.7], [1 -1.2 0.45], [1 -0.5 0.06], [1 -0.9 0.3 -0.05]};
Te = 0.5;
for k = 1:numel(en)
  p('sysd_d2c', en{k}, 'b_zinv', EB{k}); p('sysd_d2c', en{k}, 'a_zinv', EA{k}); p('sysd_d2c', en{k}, 'T', Te);
  Gd = filt(EB{k}, EA{k}, Te);
  for mi = 2:4
    m = methods{mi}; w0 = 1.0;
    key = sprintf('%s|T=%g', m, Te);
    try
      if strcmp(m, 'prewarp'), Gc = d2c(Gd, 'prewarp', w0); else Gc = d2c(Gd, m); end
      [n, d] = tfdata(Gc, 'vector');
      p('d2c_num', en{k}, key, n); p('d2c_den', en{k}, key, d);
    catch err
      printf('d2c_err|%s|%s|%s\n', en{k}, key, strrep(err.message, "\n", ' '));
    end
  end
  % d2c zoh: the port deliberately omits it; the reference is recorded for the gap row.
  try
    Gc = d2c(Gd, 'zoh'); [n, d] = tfdata(Gc, 'vector');
    p('d2c_num', en{k}, sprintf('zoh|T=%g', Te), n); p('d2c_den', en{k}, sprintf('zoh|T=%g', Te), d);
  catch err
    printf('d2c_err|%s|zoh|%s\n', en{k}, strrep(err.message, "\n", ' '));
  end
end
