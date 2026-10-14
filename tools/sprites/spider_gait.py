"""Rehace la hoja HD de la araña: alinea cada cuadro por la marca roja del abdomen (en vez de
por su caja, que cambia con las patas) y arma el ciclo de caminar alternando las dos poses más
distintas de cada dirección. Uso: spider_gait.py <hoja_hd.png> <salida.png> <gif_prefijo>"""
import sys
from PIL import Image
import numpy as np

src, out_path, gif_prefix = sys.argv[1:4]
C = 160
sheet = np.array(Image.open(src).convert("RGBA"))


def cell(j, i):
    return sheet[j * C:(j + 1) * C, i * C:(i + 1) * C]


def red_centroid(c):
    r, g, b, a = (c[..., k].astype(int) for k in range(4))
    m = (r > 170) & (g < 90) & (b < 90) & (a > 128)
    ys, xs = np.nonzero(m)
    return np.array([xs.mean(), ys.mean()])


def shift(c, d):
    """Mueve la celda d=(dx, dy) píxeles enteros, rellenando con transparente."""
    dx, dy = (int(round(v)) for v in d)
    out = np.zeros_like(c)
    ys = slice(max(0, dy), min(C, C + dy)), slice(max(0, -dy), min(C, C - dy))
    xs = slice(max(0, dx), min(C, C + dx)), slice(max(0, -dx), min(C, C - dx))
    out[ys[0], xs[0]] = c[ys[1], xs[1]]
    return out


out = sheet.copy()
modes = {"actual": [[cell(j, i) for i in range(1, 9)] for j in range(4)], "alineada": [], "pasos": []}
for j in range(4):
    anchor = red_centroid(cell(j, 0))
    aligned = [shift(cell(j, i), anchor - red_centroid(cell(j, i))) for i in range(9)]
    # Las dos poses de caminar (1..8) con las patas más distintas.
    masks = [a[..., 3] > 128 for a in aligned]
    best = max(
        ((p, q) for p in range(1, 9) for q in range(p + 1, 9)),
        key=lambda pq: (masks[pq[0]] ^ masks[pq[1]]).sum(),
    )
    print("dirección", j, "poses", best)
    a, b = (aligned[k] for k in best)
    cycle = [aligned[0], a, a, b, b, a, a, b, b]
    for i, c in enumerate(cycle):
        out[j * C:(j + 1) * C, i * C:(i + 1) * C] = c
    modes["alineada"].append(aligned[1:9])
    modes["pasos"].append(cycle[1:9])

Image.fromarray(out).save(out_path)

# Vista previa: las 4 direcciones caminando a 20 cuadros por segundo, sobre suelo.
for name, rows in modes.items():
    frames = []
    for t in range(16):
        f = Image.new("RGBA", (4 * C, C), (110, 95, 80, 255))
        for j in range(4):
            f.alpha_composite(Image.fromarray(rows[j][t % 8]), (j * C, 0))
        frames.append(f.convert("RGB"))
    frames[0].save(f"{gif_prefix}-{name}.gif", save_all=True, append_images=frames[1:],
                   duration=50, loop=0)
