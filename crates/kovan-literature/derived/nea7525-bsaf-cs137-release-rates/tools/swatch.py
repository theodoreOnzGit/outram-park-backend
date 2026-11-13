"""Read each legend entry's swatch colour: the authoritative curve colour."""
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

if __name__ == '__main__':
    path = sys.argv[1]
    ytop, ybot, xleft, xright = map(int, sys.argv[2:6])
    w, h, raw = load(path)
    # swatch lines sit just inside the left border of the legend box
    off0 = int(sys.argv[6]) if len(sys.argv) > 6 else 12
    off1 = int(sys.argv[7]) if len(sys.argv) > 7 else 70
    xs = range(xleft + off0, xleft + off1)
    rows = []
    for y in range(ytop + 3, ybot - 2):
        samples = [px(raw, w, x, y) for x in xs]
        ink = [p for p in samples if max(p) < 235]
        if len(ink) < 0.6 * len(samples):
            continue
        r = sum(p[0] for p in ink) / len(ink)
        g = sum(p[1] for p in ink) / len(ink)
        b = sum(p[2] for p in ink) / len(ink)
        rows.append((y, r, g, b))
    # group contiguous rows into swatches, take the darkest/most saturated row
    groups, cur = [], []
    for rec in rows:
        if cur and rec[0] - cur[-1][0] > 3:
            groups.append(cur); cur = []
        cur.append(rec)
    if cur: groups.append(cur)
    print(f'{path}: {len(groups)} swatches')
    for g in groups:
        best = min(g, key=lambda t: sum(t[1:]))
        y, r, gg, b = best
        print(f'  row {y:5d}  rgb({round(r):3d},{round(gg):3d},{round(b):3d})  '
              f'#{round(r):02X}{round(gg):02X}{round(b):02X}  rows={g[0][0]}-{g[-1][0]}')
