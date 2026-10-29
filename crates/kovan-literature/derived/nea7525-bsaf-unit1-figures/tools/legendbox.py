"""Refine a legend box from an approximate rectangle read off the figure.

The Unit 1 figures have no full plot frame, so calib.legend_box (which looks
for the longest dark runs inside a frame) picks up flat curves instead of
the legend border. Here the operator gives the approximate box, and the
border is located as the rows/columns within +-20 px whose pixels are
>=70 % grey ink along the box's length.

usage: legendbox.py IMAGE XL XR YT YB
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # python3 -I drops it
from calib import load, px

def greyink(p):
    return max(p) < 170 and max(p) - min(p) < 30

if __name__ == '__main__':
    path, xl, xr, yt, yb = sys.argv[1], *map(int, sys.argv[2:6])
    w, h, raw = load(path)
    def row_frac(y): return sum(greyink(px(raw, w, x, y)) for x in range(xl + 20, xr - 20)) / (xr - xl - 40)
    def col_frac(x): return sum(greyink(px(raw, w, x, y)) for y in range(yt + 20, yb - 20)) / (yb - yt - 40)
    top = [y for y in range(yt - 20, yt + 20) if row_frac(y) > 0.7]
    bot = [y for y in range(yb - 20, yb + 20) if row_frac(y) > 0.7]
    lef = [x for x in range(xl - 20, xl + 20) if col_frac(x) > 0.7]
    rig = [x for x in range(xr - 20, xr + 20) if col_frac(x) > 0.7]
    print(f'{path}: top {top} bottom {bot} left {lef} right {rig}')
    if top and bot and lef and rig:
        print(f'  box (ytop,ybot,xleft,xright) = {min(top)} {max(bot)} {min(lef)} {max(rig)}')
