"""Find the axis lines and tick marks of a MATLAB-style 'box off' plot, and
check the linear calibration they imply.

The Unit 1 figures (NEA No. 7525, Ch. 2) draw only a left and a bottom axis,
with short inward tick marks and (on some panels) light-grey gridlines. The
calibration is CHECKED, not assumed: the tick labels are evenly spaced
numbers, so the detected ticks must be evenly spaced pixels. The script
prints each tick's measured pixel against the pixel predicted by a
least-squares line through all of them, and the worst residual.

usage: axes.py IMAGE XVALS YVALS [x0] [y0]
  x0 / y0: the first label sits ON the other axis line (no separate tick
  mark), so the axis-line centre is used as that tick.
  XVALS / YVALS: comma lists of the printed tick labels matched to the
  detected marks, left->right and bottom->top; '-' for a detected mark that
  is a curve, not a tick (e.g. 0,5,10,15,20  and 0,1,2,3,4,5,6,7,8).
No PIL/numpy: reads raw RGB from ImageMagick.
"""
import subprocess, sys

def load(path):
    wh = subprocess.run(['convert', path, '-format', '%w %h', 'info:'],
                        capture_output=True, text=True, check=True).stdout.split()
    w, h = int(wh[0]), int(wh[1])
    raw = subprocess.run(['convert', path, '-depth', '8', 'rgb:-'],
                         capture_output=True, check=True).stdout
    return w, h, raw

def dark(raw, w, x, y, t=150):
    i = (y * w + x) * 3
    r, g, b = raw[i], raw[i+1], raw[i+2]
    return max(r, g, b) < t and max(r, g, b) - min(r, g, b) < 40

def longest_run(seq):
    best = (0, 0, 0); s = None
    for i, v in enumerate(seq + [False]):
        if v and s is None: s = i
        if not v and s is not None:
            if i - s > best[0]: best = (i - s, s, i - 1)
            s = None
    return best

def groups(idx, gap=2):
    out, cur = [], []
    for v in idx:
        if cur and v - cur[-1] > gap:
            out.append(cur); cur = []
        cur.append(v)
    if cur: out.append(cur)
    return [sum(g) / len(g) for g in out]

def fit(px, vals):
    n = len(px); mx = sum(vals) / n; mp = sum(px) / n
    sxx = sum((v - mx) ** 2 for v in vals)
    a = sum((v - mx) * (p - mp) for v, p in zip(vals, px)) / sxx
    b = mp - a * mx
    return a, b

def report(name, px, vals):
    # a label of None marks a detected mark that is NOT a tick (a curve
    # running perpendicular to the axis); it is dropped, and the drop is
    # printed so it can be audited.
    if len(px) == len(vals):
        for p, v in zip(px, vals):
            if v is None: print(f'    {name}: mark at px {p:.1f} assigned to no label (curve, not tick)')
        keep = [(p, v) for p, v in zip(px, vals) if v is not None]
        px = [k[0] for k in keep]; vals = [k[1] for k in keep]
    if len(px) != len(vals):
        print(f'  {name}: found {len(px)} ticks {[round(p,1) for p in px]} '
              f'but {len(vals)} labels -- CALIBRATION FAILS, inspect by hand')
        return None
    a, b = fit(px, vals)
    res = [p - (a * v + b) for p, v in zip(px, vals)]
    sp = [round(px[i+1] - px[i], 1) for i in range(len(px) - 1)]
    print(f'  {name}: ticks {[round(p,1) for p in px]}')
    print(f'    spacings {sp} (between consecutive found ticks); fitted {abs(a):.4f} px per unit')
    print(f'    residuals vs LSQ line {[round(r,2) for r in res]}  worst {max(abs(r) for r in res):.2f} px')
    print(f'    {vals[0]} at px {b + a*vals[0]:.2f}, {vals[-1]} at px {b + a*vals[-1]:.2f}')
    return a, b

if __name__ == '__main__':
    path = sys.argv[1]
    xv = [None if v == '-' else float(v) for v in sys.argv[2].split(',')]
    yv = [None if v == '-' else float(v) for v in sys.argv[3].split(',')]
    w, h, raw = load(path)
    # y axis: column with the longest dark vertical run
    best = (0, 0, 0, 0)
    for x in range(w // 3):
        r = longest_run([dark(raw, w, x, y) for y in range(h)])
        if r[0] > best[0]: best = (r[0], x, r[1], r[2])
    _, xa, ytop, ybot_a = best
    # x axis: row with the longest dark horizontal run, lower half
    bestx = (0, 0, 0, 0)
    for y in range(h // 2, h):
        r = longest_run([dark(raw, w, x, y) for x in range(w)])
        if r[0] > bestx[0]: bestx = (r[0], y, r[1], r[2])
    _, ya, xl, xr = bestx
    # axis lines are 2-3 px thick: find the extent
    xcols = [x for x in range(xa - 4, xa + 5) if sum(dark(raw, w, x, y) for y in range(ytop, ybot_a)) > 0.8 * (ybot_a - ytop)]
    yrows = [y for y in range(ya - 4, ya + 5) if sum(dark(raw, w, x, y) for x in range(xl, xr)) > 0.8 * (xr - xl)]
    print(f'{path}: {w}x{h}')
    print(f'  y-axis line cols {xcols} rows {ytop}-{ybot_a}; x-axis line rows {yrows} cols {xl}-{xr}')
    xin = max(xcols) + 1; yin = min(yrows) - 1
    # ticks: thin (1-2 px) grey marks pointing inward from the axis. Score
    # each column (row) by how dark the 6 px band inside the axis is, using a
    # light threshold because JPEG greys the 1-px ticks; a tick is a column
    # whose band is fully inked while both neighbours 3 px away are not.
    def ink(x, y):
        i = (y * w + x) * 3
        r, g, b = raw[i], raw[i+1], raw[i+2]
        return max(r, g, b) < 215 and max(r, g, b) - min(r, g, b) < 40
    def band_x(x): return sum(ink(x, y) for y in range(yin - 6, yin + 1))
    def band_y(y): return sum(ink(x, y) for x in range(xin, xin + 7))
    xt = [x for x in range(xin + 4, min(w - 4, xr + 3))
          if band_x(x) >= 5 and band_x(x - 3) <= 2 and band_x(x + 3) <= 2]
    yt = [y for y in range(4, yin - 4)
          if band_y(y) >= 5 and band_y(y - 3) <= 2 and band_y(y + 3) <= 2]
    xt = groups(xt); yt = groups(yt)
    axis_x = (min(xcols) + max(xcols)) / 2; axis_y = (min(yrows) + max(yrows)) / 2
    print(f'  axis-line centres: x-axis row {axis_y}, y-axis col {axis_x}')
    if len(sys.argv) > 4 and 'x0' in sys.argv[4:]: xt = [axis_x] + xt
    if len(sys.argv) > 4 and 'y0' in sys.argv[4:]: yt = yt + [axis_y]
    report('x', xt, xv)
    report('y', sorted(yt, reverse=True), yv)
