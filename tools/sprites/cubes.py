"""Caras de los cubos del tablero 3D del menú (`maptiles_cubes.png`: 6 temas x 5 casillas de
160 px en una fila: arriba del muro, lado del muro, arriba de la caja, lado de la caja y tierra
bajo la isla). Los muros y cajas de `maptiles.png` son a veces un objeto sobre el suelo (roca,
bola de nieve, balde); como cubos hacen falta bloques que llenen la casilla.

Uso:
  cubes.py ref <maptiles.png> <referencia.png>
      Grilla de 3 columnas x 6 filas sobre magenta con muro, caja y suelo de cada tema tal como
      se ven en la partida, para pasarle a ChatGPT.
  cubes.py build <ia.png> <salida.png> [<prueba.png>]
      Corta la grilla de 5 columnas x 6 filas que devuelva ChatGPT (separada por franjas
      magenta, como en maptiles.py) y arma la hoja. Con <prueba.png>, cada tema como un cubo de
      muro y uno de caja en falsa perspectiva sobre la tierra, para mirarlos sin abrir el juego."""
import sys

import numpy as np
from PIL import Image

CELL, COLS, ROWS = 160, 5, 6
MAGENTA = (255, 0, 255)


def ref(maptiles_path, out_path):
    tiles = Image.open(maptiles_path).convert("RGB")
    big, gap = 320, 24
    cols = 3
    out = Image.new("RGB", (cols * big + (cols + 1) * gap, ROWS * big + (ROWS + 1) * gap), MAGENTA)
    for t in range(ROWS):
        # Muro, caja y suelo de maptiles.png (celdas 1, 2 y 0 del tema).
        for c, cell in enumerate((1, 2, 0)):
            x = (t * 4 + cell) * CELL
            img = tiles.crop((x, 0, x + CELL, CELL)).resize((big, big), Image.NEAREST)
            out.paste(img, (gap + c * (big + gap), gap + t * (big + gap)))
    out.save(out_path)
    print(out.size)


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


def build(ai_path, out_path, test_path=None):
    a = np.array(Image.open(ai_path).convert("RGB")).astype(np.int32)
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    magenta = (r > 180) & (b > 180) & (g < 100)
    row_runs = runs(magenta.mean(1) < 0.5)
    col_runs = runs(magenta.mean(0) < 0.5)
    assert len(row_runs) == ROWS and len(col_runs) == COLS, (row_runs, col_runs)
    tiles = []
    for y0, y1 in row_runs:
        for x0, x1 in col_runs:
            m = magenta[y0:y1, x0:x1]
            ys = np.nonzero(m.mean(1) < 0.2)[0]
            xs = np.nonzero(m.mean(0) < 0.2)[0]
            c = a[y0 + ys.min():y0 + ys.max() + 1, x0 + xs.min():x0 + xs.max() + 1]
            tiles.append(Image.fromarray(c.astype(np.uint8)).resize((CELL, CELL), Image.LANCZOS))
    out = Image.new("RGB", (ROWS * COLS * CELL, CELL))
    for i, t in enumerate(tiles):
        out.paste(t, (i * CELL, 0))
    out.save(out_path)
    print(out.size)
    if test_path:
        preview(tiles, test_path)


def preview(tiles, path):
    """Para cada tema: muro y caja con la cara de arriba aplastada encima de la del lado, sobre
    una franja de tierra."""
    s, depth = 100, 50
    img = Image.new("RGB", (ROWS * (2 * s + 40), depth + s + 2 * s // 3 + 20), (10, 10, 14))
    for t in range(ROWS):
        ox = t * (2 * s + 40) + 20
        for k in range(2):
            top, side = tiles[t * COLS + 2 * k], tiles[t * COLS + 2 * k + 1]
            img.paste(top.resize((s, depth), Image.LANCZOS), (ox + k * s, 10))
            img.paste(side.resize((s, s), Image.LANCZOS), (ox + k * s, 10 + depth))
        earth = tiles[t * COLS + 4].resize((s, 2 * s // 3), Image.LANCZOS)
        for k in range(2):
            img.paste(earth, (ox + k * s, 10 + depth + s))
    img.save(path)


if __name__ == "__main__":
    mode, *args = sys.argv[1:]
    {"ref": ref, "build": build}[mode](*args)
