import re, sys
f = sys.argv[1]
lo = int(sys.argv[2]); hi = int(sys.argv[3])
worst = {}
for line in open(f):
    m = re.match(r'\s*step\s+(\d+)', line)
    if not m:
        continue
    n = int(m.group(1))
    if n < lo or n > hi:
        continue
    for k, v in re.findall(r'(\S+)=([0-9.e+-]+)', line):
        worst[k] = max(worst.get(k, 0), float(v))
print('steps', lo, '-', hi, ' '.join('%s=%.1e' % (k, v) for k, v in worst.items()))
