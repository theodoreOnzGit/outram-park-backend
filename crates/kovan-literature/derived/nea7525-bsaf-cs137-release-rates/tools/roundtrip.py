"""Round-trip check: does each digitised point land on its own curve?

Take every (x, y) in the cleaned CSV, map it back to a pixel with the same
calibration used to produce it, and sample the ORIGINAL (unmasked) figure
there. The pixel should be the curve's legend colour. Reports the share
within tolerance and the median RGB distance.

This catches a trace that locked onto the wrong curve or onto the axis --
which a point count cannot.
"""
import csv, math, subprocess, sys

def load(path):
    wh = subprocess.run(['convert', path, '-format', '%w %h', 'info:'],
                        capture_output=True, text=True, check=True).stdout.split()
    w, h = int(wh[0]), int(wh[1])
    raw = subprocess.run(['convert', path, '-depth', '8', 'rgb:-'],
                         capture_output=True, check=True).stdout
    return w, h, raw

def px(raw, w, x, y):
    i = (y * w + x) * 3
    return raw[i], raw[i+1], raw[i+2]

def dist(a, b):
    return math.sqrt(sum((a[i]-b[i])**2 for i in range(3)))

if __name__ == '__main__':
    img, csvp = sys.argv[1], sys.argv[2]
    x0, x1, y0, y1 = map(int, sys.argv[3:7])
    r, g, b = map(int, sys.argv[7:10])
    tol = float(sys.argv[10]) if len(sys.argv) > 10 else 30.0
    w, h, raw = load(img)
    want = (r, g, b)
    ds = []
    with open(csvp) as f:
        for row in csv.reader(f):
            try:
                x, y = float(row[0]), float(row[1])
            except (ValueError, IndexError):
                continue
            if y <= 0: continue
            xp = int(round(x0 + (x / 20.0) * (x1 - x0)))
            yp = int(round(y0 + (math.log10(1e15 / y) / 8.0) * (y1 - y0)))
            if not (0 <= xp < w and 0 <= yp < h): continue
            # best match in a 1-px neighbourhood (anti-aliasing / rounding)
            best = min((dist(px(raw, w, xp+dx, yp+dy), want)
                        for dx in (-1, 0, 1) for dy in (-1, 0, 1)
                        if 0 <= xp+dx < w and 0 <= yp+dy < h))
            ds.append(best)
    if not ds:
        print('  NO POINTS'); sys.exit()
    ds.sort()
    ok = sum(1 for d in ds if d <= tol)
    print(f'  n={len(ds):5d}  on-curve={100.0*ok/len(ds):5.1f}%  '
          f'median_d={ds[len(ds)//2]:5.1f}  p90_d={ds[int(0.9*len(ds))]:5.1f}  max_d={ds[-1]:5.1f}')
