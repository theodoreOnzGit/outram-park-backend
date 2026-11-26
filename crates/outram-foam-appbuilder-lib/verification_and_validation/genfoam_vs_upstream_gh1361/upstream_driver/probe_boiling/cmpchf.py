import sys, collections
UP = sys.argv[1]; PORT = sys.argv[2]; SHOW = int(sys.argv[3]) if len(sys.argv) > 3 else 12

def parse(line):
    w = line.split()
    d = {}
    for x in w[1:]:
        if '=' in x:
            k, v = x.split('=', 1)
            d[k] = float(v)
    return w[0], d

up = collections.defaultdict(list)
rank = {}
cur = None
for line in open(UP):
    tag, d = parse(line)
    if tag == 'CHF_IN':
        st = int(d['step'])
        rank.setdefault(st, len(rank) + 1)
        cur = dict(d)
        up[(rank[st], int(d['cell']))].append(cur)
    elif tag == 'CHF_ONB':
        cur['TONB'] = d['TONB']
    elif tag == 'CHF_TCHF':
        cur['qCHF'] = d['qCHF']; cur['TCHF'] = d['TCHF']
    elif tag == 'CHF_NB':
        cur['ret'] = d['ret']; cur['dmdtW'] = d['dmdtW']; cur['chfBool'] = 0.0
    elif tag == 'CHF_TB':
        cur['ret'] = d['ret']; cur['dmdtW'] = d['dmdtW']; cur['wfTB'] = d['wfTB']; cur['TLF'] = d['TLF']; cur['chfBool'] = 2.0
        cur['hFBMIN'] = d['hFBMIN']; cur['hGammaMIN'] = d['hGammaMIN']
    elif tag == 'CHF_FB':
        cur['ret'] = d['hFB']; cur['dmdtW'] = d['dmdtW']; cur['TLF'] = d['TLF']; cur['chfBool'] = 1.0
port = collections.defaultdict(list)
times = []
for line in open(PORT):
    tag, d = parse(line)
    if tag != 'PORT_CHF':
        continue
    t = d['t']
    if not times or t != times[-1]:
        times.append(t)
    port[(len(times), int(d['cell']))].append(d)
names = ['Tw', 'Tf', 'Tsat', 'htcFC', 'F', 'qFC', 'Re', 'Pr', 'kappa', 'p', 'L', 'Area', 'TONB', 'qCHF', 'TCHF', 'TLF', 'wfTB', 'ret', 'dmdtW']
first = {}
for key in sorted(up):
    for it, (u, p) in enumerate(zip(up[key], port.get(key, []))):
        for n in names:
            if n in u and n in p:
                a, b = u[n], p[n]
                r = abs(a - b) / abs(a) if a != 0 else abs(b)
                if n == 'L':
                    r = abs(abs(a) - abs(b)) / abs(a)
                if r > 1e-9 and (n, key[0], it + 1) not in first:
                    first[(n, key[0], it + 1)] = (key[1], a, b, r)
rows = sorted(first.items(), key=lambda kv: (kv[0][1], kv[0][2]))
for (n, st, it), (cell, a, b, r) in rows[:SHOW]:
    print('step %d iter %d %-7s cell %d up=%.15g port=%.15g rel=%.2e' % (st, it, n, cell, a, b, r))
