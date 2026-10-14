"""Arma maptiles_hd.png (6 temas x 4 casillas de 160 px en una fila, como maptiles.png) a partir
de una grilla de ChatGPT de 4 columnas x 6 filas separadas por franjas magenta.
Uso: maptiles.py <ia.png> <salida.png> [<mosaico_prev.png>]"""
import sys

import numpy as np
from PIL import Image

ai_path, out_path = sys.argv[1:3]
prev_path = sys.argv[3] if len(sys.argv) > 3 else None
CELL, COLS, ROWS = 160, 4, 6

a = np.array(Image.open(ai_path).convert("RGB")).astype(np.int32)
r, g, b = a[..., 0], a[..., 1], a[..., 2]
magenta = (r > 180) & (b > 180) & (g < 100)


def runs(inside):
    out, start = [], None
    for i, s in enumerate(inside):
        if s and start is None:
            start = i
        if not s and start is not None:
            out.append((start, i))
            start = None
    if start is not None:
        out.append((start, len(inside)))
    return [rn for rn in out if rn[1] - rn[0] > 40]


# Filas y columnas de casillas: donde casi no hay magenta.
row_runs = runs(magenta.mean(1) < 0.5)
col_runs = runs(magenta.mean(0) < 0.5)
assert len(row_runs) == ROWS and len(col_runs) == COLS, (row_runs, col_runs)

tiles = []
for y0, y1 in row_runs:
    for x0, x1 in col_runs:
        # Recorta lo que quede de magenta en el borde de la casilla.
        m = magenta[y0:y1, x0:x1]
        ys = np.nonzero(m.mean(1) < 0.2)[0]
        xs = np.nonzero(m.mean(0) < 0.2)[0]
        c = a[y0 + ys.min():y0 + ys.max() + 1, x0 + xs.min():x0 + xs.max() + 1]
        tiles.append(Image.fromarray(c.astype(np.uint8)).resize((CELL, CELL), Image.LANCZOS))

# Suelos orgánicos que se repiten: se mezclan cerca del borde con una copia desplazada media
# casilla (que empalma consigo misma), así no se nota la grilla. El centro queda intacto. La
# cocina y el metal son baldosas y planchas a propósito: no se tocan.
SEAMLESS = {0, 1, 2, 4}
BAND = 36


def seamless(img):
    t = np.array(img).astype(np.float32)
    rolled = np.roll(np.roll(t, CELL // 2, 0), CELL // 2, 1)
    d = np.minimum(np.arange(CELL), CELL - 1 - np.arange(CELL)).astype(np.float32)
    k = np.clip(d / BAND, 0, 1)
    k = k * k * (3 - 2 * k)
    w = np.minimum.outer(k, k)[..., None]
    return Image.fromarray((w * t + (1 - w) * rolled).round().astype(np.uint8))


for th in SEAMLESS:
    tiles[th * COLS] = seamless(tiles[th * COLS])

out = Image.new("RGB", (ROWS * COLS * CELL, CELL))
for i, t in enumerate(tiles):
    out.paste(t, (i * CELL, 0))
out.save(out_path)
print(out.size)

if prev_path:
    # Mosaico de prueba por tema (a la mitad): suelo repetido con muros y cajas encima.
    layout = [
        "1111111",
        "1000201",
        "1010103",
        "1002001",
        "1111111",
    ]
    h = CELL // 2
    prev = Image.new("RGB", (len(layout[0]) * h * 3 + 20, len(layout) * h * 2 + 10), (0, 0, 0))
    for t in range(ROWS):
        ox, oy = (t % 3) * (len(layout[0]) * h + 10), (t // 3) * (len(layout) * h + 10)
        for j, row in enumerate(layout):
            for i, ch in enumerate(row):
                prev.paste(tiles[t * COLS + int(ch)].resize((h, h), Image.LANCZOS),
                           (ox + i * h, oy + j * h))
    prev.save(prev_path)
