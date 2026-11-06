"""Find the light-grey gridlines of a MATLAB 'grid on' plot and check the
linear calibration they imply (added 2026-10-29 for Fig 2.6).

Fig 2.6's four panels draw a full set of very light gridlines (~RGB 239,
after JPEG) at every labelled tick. Curves at zero lie on the x axis and
ticks are faint, so axes.py's tick finder fails there; the gridlines are
the better instrument. As in axes.py the calibration is CHECKED, not
assumed: gridlines at evenly spaced labels must be evenly spaced pixels,
and a least-squares line through them gives the residuals.

A column (row) is a gridline candidate when >= FRAC of the plot box's rows
(columns) in it are faint grey (225..252, chroma < 10) AND it is greyer
than the columns NB px either side (a gridline is 1-5 px wide). Runs of
adjacent candidates are merged to their centre.

usage: grid_lin.py IMAGE XL XR YT YB XVALS YVALS [x0] [y0]
  XL..YB: a box inside the plot area (exclude the axis lines and the legend
  if it crosses gridlines, by choosing the box).
  XVALS / YVALS: labels of the found gridlines, left->right, bottom->top,
  '-' for a candidate that is not a gridline (printed for audit).
  x0 / y0: prepend the y-axis column / append the x-axis row given by env
  AXCOL / AXROW as the first-label line (the axis line itself, where the
  first label sits on it and there is no gridline).
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # python3 -I drops it
from calib import load
from axes import groups, report

FRAC = float(os.environ.get('FRAC', '0.35'))
# neighbour offset: panels A and B are softer JPEGs whose gridlines are 4-5 px
# wide, so the 'greyer than its neighbours' test looks NB px either side
NB = int(os.environ.get('NB', '5'))

def faint(raw, w, x, y):
    i = (y * w + x) * 3
    r, g, b = raw[i], raw[i+1], raw[i+2]
    return 225 <= max(r, g, b) <= 252 and max(r, g, b) - min(r, g, b) < 10

if __name__ == '__main__':
    path = sys.argv[1]
    xl, xr, yt, yb = map(int, sys.argv[2:6])
    xv = [None if v == '-' else float(v) for v in sys.argv[6].split(',')]
    yv = [None if v == '-' else float(v) for v in sys.argv[7].split(',')]
    w, h, raw = load(path)
    colf = {x: sum(faint(raw, w, x, y) for y in range(yt, yb)) / (yb - yt) for x in range(xl - NB, xr + NB + 1)}
    rowf = {y: sum(faint(raw, w, x, y) for x in range(xl, xr)) / (xr - xl) for y in range(yt - NB, yb + NB + 1)}
    xc = [x for x in range(xl, xr + 1) if colf[x] >= FRAC and colf[x] > colf[x - NB] + 0.15 and colf[x] > colf[x + NB] + 0.15]
    yc = [y for y in range(yt, yb + 1) if rowf[y] >= FRAC and rowf[y] > rowf[y - NB] + 0.15 and rowf[y] > rowf[y + NB] + 0.15]
    xg = groups(xc); yg = groups(yc)
    print(f'{path}: {w}x{h}  box x[{xl},{xr}] y[{yt},{yb}]')
    print(f'  gridline columns {[round(v, 1) for v in xg]}')
    print(f'  gridline rows    {[round(v, 1) for v in yg]}')
    if 'x0' in sys.argv[8:]: xg = [float(os.environ['AXCOL'])] + xg
    if 'y0' in sys.argv[8:]: yg = yg + [float(os.environ['AXROW'])]
    report('x', xg, xv)
    report('y', sorted(yg, reverse=True), yv)
