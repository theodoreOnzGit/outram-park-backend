"""Verify log-axis calibration against the drawn gridlines, and locate the
legend box + its swatch colours.

Calibration is CHECKED, not assumed: the y axis is claimed to run 1e15 (top
frame) to 1e7 (bottom frame) over 8 decades, so the drawn gridlines must fall
at evenly spaced pixel rows. If they do not, the frame edges are not the axis
limits and everything downstream would be wrong.
"""
import subprocess, sys

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

def gridlines(w, h, raw, x0, x1, y0, y1):
    """Rows inside the frame that are mostly light-grey ink (the gridlines)."""
    hits = []
    span = x1 - x0
    for y in range(y0 + 2, y1 - 1):
        n = 0
        for x in range(x0 + 5, x1 - 5, 3):
            p = px(raw, w, x, y)
            mx, mn = max(p), min(p)
            if 150 < mx < 240 and mx - mn < 18:
                n += 1
        if n > 0.55 * (span / 3):
            hits.append(y)
    out, cur = [], []
    for y in hits:
        if cur and y - cur[-1] > 3:
            out.append(sum(cur) / len(cur)); cur = []
        cur.append(y)
    if cur: out.append(sum(cur) / len(cur))
    return out

def legend_box(w, h, raw, x0, x1, y0, y1):
    """Longest dark horizontal runs strictly inside the frame = legend border."""
    best = []
    for y in range(y0 + 10, y1 - 2):
        run = x = 0
        bs = be = None
        for x in range(x0 + 5, x1 - 5):
            p = px(raw, w, x, y)
            if max(p) < 140:
                if run == 0: s = x
                run += 1
            else:
                if run > 150 and (bs is None or run > be - bs):
                    bs, be = s, x
                run = 0
        if run > 150 and (bs is None or run > (be - bs if bs else 0)):
            bs, be = s, x1
        if bs is not None:
            best.append((y, bs, be))
    if not best: return None
    # top and bottom borders: the two rows with the widest runs
    best.sort(key=lambda t: t[2] - t[1], reverse=True)
    wide = [b for b in best if b[2] - b[1] > 0.8 * (best[0][2] - best[0][1])]
    ys = [b[0] for b in wide]
    return min(ys), max(ys), min(b[1] for b in wide), max(b[2] for b in wide)

if __name__ == '__main__':
    path, x0, x1, y0, y1 = sys.argv[1], *map(int, sys.argv[2:6])
    w, h, raw = load(path)
    g = gridlines(w, h, raw, x0, x1, y0, y1)
    print(f'{path}  frame x[{x0},{x1}] y[{y0},{y1}]  height={y1-y0}px')
    print(f'  gridlines found at rows: {[round(v,1) for v in g]}')
    if len(g) >= 2:
        d = [round(g[i+1]-g[i], 2) for i in range(len(g)-1)]
        print(f'  spacings: {d}')
        print(f'  expected decade spacing if 8 decades edge-to-edge: {round((y1-y0)/8, 2)}')
    lb = legend_box(w, h, raw, x0, x1, y0, y1)
    print(f'  legend box (ytop,ybot,xleft,xright): {lb}')
