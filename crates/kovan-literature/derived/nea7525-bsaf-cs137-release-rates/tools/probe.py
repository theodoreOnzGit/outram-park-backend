"""Locate the plot frame and the legend box in a BSAF figure raster.

Reads raw RGB produced by `convert <png> -depth 8 rgb:-`. No PIL/numpy here:
neither imports in this container.
"""
import subprocess, sys

def load(path):
    wh = subprocess.run(['convert', path, '-format', '%w %h', 'info:'],
                        capture_output=True, text=True, check=True).stdout.split()
    w, h = int(wh[0]), int(wh[1])
    raw = subprocess.run(['convert', path, '-depth', '8', 'rgb:-'],
                         capture_output=True, check=True).stdout
    assert len(raw) == w * h * 3, (len(raw), w * h * 3)
    return w, h, raw

def px(raw, w, x, y):
    i = (y * w + x) * 3
    return raw[i], raw[i+1], raw[i+2]

def is_dark(p, t=120):
    return p[0] < t and p[1] < t and p[2] < t

def frame(w, h, raw):
    """Longest dark vertical/horizontal runs = the axis box."""
    best_cols, best_rows = [], []
    for x in range(w):
        n = sum(1 for y in range(h) if is_dark(px(raw, w, x, y)))
        if n > 0.35 * h:
            best_cols.append((x, n))
    for y in range(h):
        n = sum(1 for x in range(w) if is_dark(px(raw, w, x, y)))
        if n > 0.35 * w:
            best_rows.append((y, n))
    return best_cols, best_rows

if __name__ == '__main__':
    path = sys.argv[1]
    w, h, raw = load(path)
    print(f'{path}: {w}x{h}')
    cols, rows = frame(w, h, raw)
    def groups(items):
        out, cur = [], []
        for v, n in items:
            if cur and v - cur[-1][0] > 3:
                out.append(cur); cur = []
            cur.append((v, n))
        if cur: out.append(cur)
        return [(g[0][0], g[-1][0], max(n for _, n in g)) for g in out]
    print('  dark columns (start,end,maxrun):', groups(cols))
    print('  dark rows    (start,end,maxrun):', groups(rows))
