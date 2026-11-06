"""Drop isolated traced points.

A real curve is sampled in a dense, contiguous run of columns. An isolated
point is a JPEG-blur false positive -- typically a grey-ish edge pixel of
some *other* curve picked up when tracing a grey one. Keep a point only if
it sits in a locally dense neighbourhood.

This removes false positives; it cannot recover a curve the tracer missed.
"""
import csv, sys, os

WIN = float(os.environ.get('WIN', '0.26'))   # x units either side (days for Figs 5.x, hours for Unit 1)
NEED = 4          # neighbours required within the window

def clean(path):
    with open(path) as f:
        rdr = csv.reader(f)
        header = next(rdr)
        rows = [r for r in rdr if len(r) >= 2]
    xs = []
    for r in rows:
        try:
            xs.append(float(r[0]))
        except ValueError:
            xs.append(None)
    keep = []
    for i, x in enumerate(xs):
        if x is None:
            continue
        n = 0
        j = i - 1
        while j >= 0 and xs[j] is not None and x - xs[j] <= WIN:
            n += 1; j -= 1
        j = i + 1
        while j < len(xs) and xs[j] is not None and xs[j] - x <= WIN:
            n += 1; j += 1
        if n >= NEED:
            keep.append(rows[i])
    return header, rows, keep

if __name__ == '__main__':
    tot_in = tot_out = 0
    for path in sorted(sys.argv[1:]):
        header, rows, keep = clean(path)
        dropped = len(rows) - len(keep)
        tot_in += len(rows); tot_out += len(keep)
        out = os.path.join(os.path.dirname(path).replace('raw', 'clean') or 'clean', os.path.basename(path))
        os.makedirs(os.path.dirname(out), exist_ok=True)
        with open(out, 'w', newline='') as f:
            w = csv.writer(f)
            w.writerow(header)
            w.writerows(keep)
        if dropped:
            print(f'  {os.path.basename(path):42s} {len(rows):5d} -> {len(keep):5d}  (-{dropped})')
    print(f'TOTAL {tot_in} -> {tot_out} (-{tot_in - tot_out} isolated points dropped)')
