"""Nearest-colour classify a figure once, emitting one isolated image per curve."""
import subprocess, sys, os

import os as _os
# Non-curve references: paper, gridline grey, axis black/grey. A pixel nearest
# one of these belongs to no curve. Override with BG="r,g,b;r,g,b;..." when a
# figure has a BLACK curve (Unit 1, SNL): black must then be a curve reference,
# not background, and the measurement-marker colours are added instead so
# marker pixels are not absorbed by the nearest curve.
BACKGROUND = [(255,255,255), (247,247,247), (210,210,210), (0,0,0), (60,60,60)]
if _os.environ.get('BG'):
    BACKGROUND = [tuple(map(int, c.split(','))) for c in _os.environ['BG'].split(';')]
MAXD = int(__import__('os').environ.get('MAXD', '110'))   # beyond this, a pixel belongs to no curve

def load(path):
    wh = subprocess.run(['convert', path, '-format', '%w %h', 'info:'],
                        capture_output=True, text=True, check=True).stdout.split()
    w, h = int(wh[0]), int(wh[1])
    raw = subprocess.run(['convert', path, '-depth', '8', 'rgb:-'],
                         capture_output=True, check=True).stdout
    return w, h, raw

def main():
    src, outdir, prefix = sys.argv[1], sys.argv[2], sys.argv[3]
    refs = [tuple(map(int, a.split(','))) for a in sys.argv[4:]]
    palette = refs + BACKGROUND
    w, h, raw = load(src)
    n = len(refs)
    outs = [bytearray(b'\xff' * (w*h*3)) for _ in range(n)]
    kept = [0]*n
    cache = {}
    for i in range(0, len(raw), 3):
        key = (raw[i], raw[i+1], raw[i+2])
        hit = cache.get(key, -2)
        if hit == -2:
            bd, bj = 1 << 30, -1
            for j, c in enumerate(palette):
                d = (key[0]-c[0])**2 + (key[1]-c[1])**2 + (key[2]-c[2])**2
                if d < bd:
                    bd, bj = d, j
            hit = bj if (bj < n and bd <= MAXD*MAXD) else -1
            cache[key] = hit
        if hit >= 0:
            o = outs[hit]
            o[i], o[i+1], o[i+2] = key
            kept[hit] += 1
    os.makedirs(outdir, exist_ok=True)
    for j in range(n):
        dst = os.path.join(outdir, f'{prefix}-{j}.png')
        p = subprocess.Popen(['convert', '-size', f'{w}x{h}', '-depth', '8', 'rgb:-', dst],
                             stdin=subprocess.PIPE)
        p.communicate(bytes(outs[j]))
        print(f'  {os.path.basename(dst)}  rgb{refs[j]}  {kept[j]} px')

main()
