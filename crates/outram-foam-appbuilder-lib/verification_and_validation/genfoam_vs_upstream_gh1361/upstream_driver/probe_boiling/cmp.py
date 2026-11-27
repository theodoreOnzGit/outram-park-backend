import sys, collections
up_f, port_f = sys.argv[1], sys.argv[2]
OFF = int(sys.argv[4]) if len(sys.argv) > 4 else 25
thr = float(sys.argv[3]) if len(sys.argv) > 3 else 1e-12

def parse(line):
    w = line.split()
    d = {}
    for x in w[1:]:
        if '=' in x:
            k, v = x.split('=', 1)
            d[k] = float(v)
    return w[0], d

# upstream: group by (step, cell) -> list of merged dicts by call
up = collections.defaultdict(list)   # (step,cell) -> list of dict
steprank = {}
cur = {}
for line in open(up_f):
    tag, d = parse(line)
    if tag == 'TRACE_IN':
        cur = dict(d)
        st = int(d['step'])
        steprank.setdefault(st, len(steprank) + 1)
        up[(steprank[st], int(d['cell']))].append(cur)
    elif tag in ('TRACE_ONB',):
        cur['TONB'] = d['TONB']
    elif tag == 'BASU':
        for k in ('A', 'rhoOther', 'absL', 'dTONBsat'):
            cur['B_' + k] = d[k]
        cur['B_kappa'] = d['kappa']
    elif tag == 'TRACE_NB':
        for k in ('hPBONB', 'hPB', 'qBI', 'qPB', 'qNB', 'f', 'qSCDmdt', 'dmdtW', 'ret'):
            cur[k] = d[k]
# parse step from tag line manually (step= is parsed as float key 'step')
port = collections.defaultdict(list)
times = []
for line in open(port_f):
    tag, d = parse(line)
    t = d['t']
    if not times or t != times[-1]:
        times.append(t)
    step = len(times)
    port[(step, int(d['cell'])+OFF)].append(d)

names = ['Tw', 'Tf', 'Tsat', 'htcFC', 'F', 'htc2pFC', 'qFC', 'Re', 'Pr', 'kappa', 'Dh', 'p', 'L', 'TONB', 'rhoOther', 'absL', 'hPBONB', 'hPB', 'qNB', 'f', 'dmdtW', 'ret']
alias = {'rhoOther': 'B_rhoOther', 'absL': 'B_absL'}
first = {}
worst = collections.defaultdict(float)
for key in sorted(up):
    if key not in port:
        continue
    for k, (u, p) in enumerate(zip(up[key], port[key])):
        for n in names:
            un = alias.get(n, n)
            if un in u and n in p:
                a, b = u[un], p[n]
                rel = abs(a - b) / max(abs(a), 1e-300) if a != 0 else abs(b)
                if n == 'L':
                    rel = abs(abs(a) - abs(b)) / abs(a)
                if rel > worst[(key[0], n)]:
                    worst[(key[0], n)] = rel
                if rel > thr and n not in first:
                    first[n] = (key[0], k + 1, key[1], a, b, rel)
print('first exceedance per name (step, outer_iter, cell, upstream, port, rel):')
for n in names:
    if n in first:
        print(' ', n, first[n])
print('per step worst rel:')
steps = sorted(set(s for s, _ in worst))
for s in steps:
    print(' step', s, ' '.join('%s=%.1e' % (n, worst[(s, n)]) for n in names if (s, n) in worst))
