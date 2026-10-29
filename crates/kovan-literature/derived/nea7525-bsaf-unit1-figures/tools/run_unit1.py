"""Drive the Unit 1 pipeline (NEA No. 7525 Figs 2.1, 2.2, 2.5) end to end.

For each figure in curves-unit1.tsv:
  1. paint the legend box white (its swatches and labels are not data);
  2. nearest-colour classify every pixel against all the figure's legend
     colours plus the non-curve references in BG (isolate_all.py, MAXD cap);
  3. on each isolated image, white out the inward tick marks at the
     calibrated tick positions (the SNL curve is black, so ticks classify
     as SNL ink) and draw a black frame over the axis lines, because these
     'box off' figures have no frame for `kovan-cli digitise` to detect;
  4. trace with `kovan-cli digitise`, explicit --x-ref/--y-ref from the
     tick calibration (axes.py), continuity strategy.
Raw traces land in WORK/raw/<slug>.csv in data units.

usage: run_unit1.py TSV IMGDIR WORK KOVAN_CLI
"""
import csv, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
MAXD = os.environ.get('MAXD', '30')
# paper, gridline greys, anti-aliased black edge grey, and the two
# measurement-marker families (red-orange squares; teal triangles with a
# light-green rim) so marker pixels are not absorbed by the nearest curve
BG = '255,255,255;247,247,247;230,230,230;210,210,210;128,128,128;255,65,7;33,178,171;102,230,5;200,250,140'
# The left and bottom frame edges sit ON the axis lines (hiding them); the
# top and right edges are PAD px outside the axis box, because several
# curves run exactly along the top (release fraction = 1) or to the right
# end of the axis, and the tracer's inset would otherwise eat them.
PAD = 9
TICKLEN = 22   # px; the inward ticks are up to ~20 px long across these rasters
TICKS = {   # printed tick labels, per figure (x list, y list)
    '2.1a': ([0, 5, 10, 15, 20], [0, 1, 2, 3, 4, 5, 6, 7, 8]),
    '2.1b': ([0, 5, 10, 15, 20], [0, 0.2, 0.4, 0.6, 0.8, 1.0]),
    '2.2':  ([0, 5, 10, 15, 20, 25], [0, 200, 400, 600, 800, 1000, 1200, 1400]),
    '2.5a': ([0, 5, 10, 15], [0, 0.2, 0.4, 0.6, 0.8, 1.0]),
    '2.5b': ([0, 5, 10, 15], [0, 0.2, 0.4, 0.6, 0.8, 1.0]),
    '2.3b': ([50, 100, 150, 200, 250, 300, 350, 400, 450, 500], [0, 5, 10, 15, 20]),
}
# Fig 2.3b: most codes inject 0 kg/s for most of the window, drawn ON the
# x axis. The frame bottom goes PAD px below the axis so that zero is
# traceable; the black axis line then classifies as SNL ink, so SNL (black,
# and itself at zero) cannot be separated from the axis on this panel.
BOTTOM_PAD = {'2.3b': PAD}

def slug(fig, series):
    return f'fig{fig}-{series}'.lower().replace(' ', '-')

def main():
    tsv, imgdir, work, kovan = sys.argv[1:5]
    rows = list(csv.DictReader(open(tsv), delimiter='\t'))
    figs = {}
    for r in rows: figs.setdefault(r['fig'], []).append(r)
    os.makedirs(f'{work}/iso', exist_ok=True); os.makedirs(f'{work}/raw', exist_ok=True)
    for fig, rs in figs.items():
        r0 = rs[0]
        xl, xr, yt, yb = map(int, r0['legend'].split(','))
        masked = f'{work}/masked-{fig}.png'
        subprocess.run(['convert', os.path.join(imgdir, r0['image']), '-fill', 'white', '-stroke', 'white',
                        '-draw', f'rectangle {xl},{yt} {xr},{yb}', masked], check=True)
        cols = [f"{r['r']},{r['g']},{r['b']}" for r in rs]
        subprocess.run(['python3', '-I', os.path.join(HERE, 'isolate_all.py'), masked, f'{work}/iso', f'f{fig}', *cols],
                       check=True, env=dict(os.environ, BG=BG, MAXD=MAXD))
        ac, ar, at, aw = (int(r0[k]) for k in ('ax_col', 'ax_right', 'ax_top', 'ax_row'))
        x0p, x0v, x1p, x1v = float(r0['xpx0']), float(r0['xval0']), float(r0['xpx1']), float(r0['xval1'])
        y0p, y0v, y1p, y1v = float(r0['ypx0']), float(r0['yval0']), float(r0['ypx1']), float(r0['yval1'])
        xpx = lambda v: x0p + (v - x0v) * (x1p - x0p) / (x1v - x0v)
        ypx = lambda v: y0p + (v - y0v) * (y1p - y0p) / (y1v - y0v)
        draw = []
        for v in TICKS[fig][0]:
            if fig in BOTTOM_PAD: continue   # keep zero-level curve ink on the axis
            p = xpx(v); draw += ['-draw', f'rectangle {p-3:.0f},{aw-TICKLEN} {p+3:.0f},{aw}']
        for v in TICKS[fig][1]:
            p = ypx(v); draw += ['-draw', f'rectangle {ac},{p-3:.0f} {ac+TICKLEN},{p+3:.0f}']
        for j, r in enumerate(rs):
            iso = f'{work}/iso/f{fig}-{j}.png'
            framed = f'{work}/iso/f{fig}-{j}-framed.png'
            subprocess.run(['convert', iso, '-fill', 'white', '-stroke', 'none', *draw,
                            '-fill', 'none', '-stroke', 'black', '-strokewidth', '3',
                            '-draw', f'rectangle {ac},{at - PAD} {ar + PAD},{aw + BOTTOM_PAD.get(fig, 0)}', framed], check=True)
            out = f'{work}/raw/{slug(fig, r["series"])}.csv'
            cmd = [kovan, 'digitise', '--image', framed, '--figure', f'Fig. {fig}',
                   '--document-title', 'NEA No. 7525 (BSAF Phase II summary report)',
                   '--notes', f'{r["panel"]}, {r["series"]}, nearest-colour isolated MAXD={MAXD}',
                   '--x-ref', f'{x0p}={x0v}', '--x-ref', f'{x1p}={x1v}',
                   '--y-ref', f'{y0p}={y0v}', '--y-ref', f'{y1p}={y1v}',
                   '--curve-rgb', f"{r['r']},{r['g']},{r['b']}", '--curve-tolerance', MAXD,
                   '--inset', '5', '--step', '2', '--min-line-fraction', '0.7',
                   '--timestamp', '2026-10-29T00:00:00Z', '--csv', out, '--verbose']
            res = subprocess.run(cmd, capture_output=True, text=True)
            print(f'  {fig:5s} {r["series"]:7s} {res.stderr.strip()[:150]}')

main()
