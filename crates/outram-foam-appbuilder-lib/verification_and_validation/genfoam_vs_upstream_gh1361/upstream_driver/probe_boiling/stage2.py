import sys, collections
CALLS = [int(x) for x in sys.argv[1].split(',')]
CELLS = [int(x) for x in sys.argv[2].split(',')]

def kv(line):
    return {x.split('=')[0]: float(x.split('=')[1]) for x in line.split()[1:] if '=' in x}

up = collections.defaultdict(dict)
call = 0
for line in open('up_psbt.txt'):
    if line.startswith('STAGE_'):
        st = line.split()[0][6:]
        d = kv(line)
        if st == 'S0' and int(d['cell']) == 20:
            call += 1
        up[(call, st, int(d['cell']))] = d
port = {}
n = 0
for line in open('port_psbt2.txt'):
    tag = line.split()[0]
    if tag.startswith('PORT_S'):
        st = tag[5:]
        d = kv(line)
        if st == 'S0' and int(d['cell']) == 20:
            n += 1
        port[(n, st, int(d['cell']))] = d
for c in CALLS:
    for cell in CELLS:
        for name, src in (('UP  ', up), ('PORT', port)):
            g = lambda st, q: src[(c, st, cell)][q]
            print(name, 'call', c, 'cell', cell,
                  'dS1S0.diag=%.10e' % (g('S1', 'diag') - g('S0', 'diag')),
                  'dS2S1.diag=%.10e' % (g('S2', 'diag') - g('S1', 'diag')),
                  'dS2S1.src=%.10e' % (g('S2', 'source') - g('S1', 'source')),
                  'dS3S2.diag=%.10e' % (g('S3', 'diag') - g('S2', 'diag')),
                  'dS3S2.src=%.10e' % (g('S3', 'source') - g('S2', 'source')))
