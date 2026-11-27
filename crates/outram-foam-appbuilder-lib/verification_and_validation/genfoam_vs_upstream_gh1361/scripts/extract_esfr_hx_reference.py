# gh:#1903 round 52: extract the small reference text of a run_esfr_hx_tight.sh run.
#   TIMES t1 t2 t3
#   G <t> <field> v...     every 11th cell of the whole mesh (U: 3 values per cell)   [U p_rgh T]
#   Z <t> <field> v...     every 7th cell of hxA followed by hxB (zone order, hxA first)  [T T.HX htc]
# plus the PRGH/H lines of extract_msfr_th_log.sh (appended by the caller). usage: python3 -I extract_esfr_hx_reference.py <run dir> <cellZones.gz> <out>
import gzip, os, re, sys

run, zfile, out = sys.argv[1:4]
zt = gzip.open(zfile, 'rt').read()
zones = {}
for m in re.finditer(r'\n(\w+)\s*\{\s*type cellZone;\s*cellLabels\s+List<label>\s*(\d+)\s*\(([^)]*)\)', zt):
    zones[m.group(1)] = list(map(int, m.group(3).split()))
core = zones['hxA'] + zones['hxB']
times = sorted((d for d in os.listdir(run) if re.fullmatch(r'[0-9.e+-]+', d) and d != '0'), key=float)


def read(path):
    t = open(path).read()
    t = t[t.index('internalField'):]
    m = re.match(r'internalField\s+uniform\s+([^;]+);', t)
    if m:
        return 'UNIFORM', m.group(1).strip()
    m = re.match(r'internalField\s+nonuniform\s+List<\w+>\s*(\d+)\s*\(', t)
    body = t[m.end():]
    body = body[:body.index('\n)')]
    rows = [r.strip().strip('()') for r in body.strip().split('\n')]
    return 'LIST', rows


lines = ['TIMES ' + ' '.join(times)]
for t in times:
    for f in ['U', 'p_rgh', 'T']:
        k, v = read('%s/%s/fluidRegion/%s' % (run, t, f))
        assert k == 'LIST', (t, f, k)
        lines.append('G %s %s %s' % (t, f, ' '.join(v[i] for i in range(0, len(v), 11))))
    for f in ['T', 'T.HX', 'htc']:
        k, v = read('%s/%s/fluidRegion/%s' % (run, t, f))
        if k == 'UNIFORM':
            lines.append('Z %s %s UNIFORM %s' % (t, f, v))
        else:
            lines.append('Z %s %s %s' % (t, f, ' '.join(v[core[i]] for i in range(0, len(core), 7))))
open(out, 'w').write('\n'.join(lines) + '\n')
