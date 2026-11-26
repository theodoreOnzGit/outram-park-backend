import sys, collections
up_f, port_f, S = sys.argv[1], sys.argv[2], int(sys.argv[3])

def parse(line):
    w = line.split()
    d = {}
    for x in w[1:]:
        if '=' in x:
            k, v = x.split('=', 1)
            d[k] = float(v)
    return w[0], d

up = collections.defaultdict(list)
steprank = {}
cur = {}
for line in open(up_f):
    tag, d = parse(line)
    if tag == 'TRACE_IN':
        st = int(d['step'])
        steprank.setdefault(st, len(steprank) + 1)
        cur = dict(d)
        up[(steprank[st], int(d['cell']))].append(cur)
    elif tag == 'TRACE_ONB':
        cur['TONB'] = d['TONB']
    elif tag == 'BASU':
        cur['B_rhoOther'] = d['rhoOther']
        cur['B_absL'] = d['absL']
    elif tag == 'TRACE_NB':
        for k in ('hPBONB', 'hPB', 'qNB', 'f', 'dmdtW', 'ret'):
            cur[k] = d[k]
port = collections.defaultdict(list)
times = []
for line in open(port_f):
    tag, d = parse(line)
    if tag != 'PORT':
        continue
    t = d['t']
    if not times or t != times[-1]:
        times.append(t)
    port[(len(times), int(d['cell']))].append(d)

names = ['Tw','Tf','Tsat','htcFC','F','Re','Pr','p','L','TONB','rhoOther','hPB','f','dmdtW','ret']
alias = {'rhoOther': 'B_rhoOther'}
for it in range(1, 13):
    row = []
    for n in names:
        w = 0.0
        for (s, cell), lst in up.items():
            if s != S or (s, cell) not in port:
                continue
            if len(lst) < it or len(port[(s, cell)]) < it:
                continue
            u = lst[it - 1]
            p = port[(s, cell)][it - 1]
            un = alias.get(n, n)
            if un in u and n in p and u[un] != 0:
                w = max(w, abs(abs(u[un]) - abs(p[n])) / abs(u[un]))
        row.append('%s=%.1e' % (n, w))
    print('step', S, 'iter', it, ' '.join(row))
