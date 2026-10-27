"""Re-project digitised points onto the source figure so a human can check them.

Draws each traced point as a small marker in a contrasting colour over a faded
copy of the original. If the markers do not sit on the curve, the trace is
wrong -- this is the check, not the point count.
"""
import csv, math, subprocess, sys, os

def marks(csv_path, x0, x1, y0, y1):
    pts = []
    with open(csv_path) as f:
        for row in csv.DictReader(f):
            vals = list(row.values())
            try:
                x, y = float(vals[0]), float(vals[1])
            except (ValueError, TypeError):
                continue
            if y <= 0:
                continue
            xp = x0 + (x / 20.0) * (x1 - x0)
            yp = y0 + (math.log10(1e15 / y) / 8.0) * (y1 - y0)
            pts.append((xp, yp))
    return pts

if __name__ == '__main__':
    src, dst, x0, x1, y0, y1 = sys.argv[1], sys.argv[2], *map(int, sys.argv[3:7])
    specs = sys.argv[7:]          # csv:hexcolour pairs
    cmd = ['convert', src, '-fill', 'white', '-colorize', '55%']
    for spec in specs:
        path, colour = spec.rsplit(':', 1)
        if not os.path.exists(path):
            continue
        pts = marks(path, x0, x1, y0, y1)
        cmd += ['-stroke', 'none', '-fill', '#' + colour]
        draw = ' '.join(f'rectangle {p[0]-1.2:.1f},{p[1]-1.2:.1f} {p[0]+1.2:.1f},{p[1]+1.2:.1f}'
                        for p in pts)
        if draw:
            cmd += ['-draw', draw]
        print(f'  {os.path.basename(path)}: {len(pts)} markers in #{colour}')
    cmd.append(dst)
    subprocess.run(cmd, check=True)
    print('wrote', dst)
