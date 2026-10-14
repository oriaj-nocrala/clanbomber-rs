"""Caras de los cubos del tablero 3D del menú (`maptiles_cubes.png`: 6 temas x 5 casillas de
160 px en una fila: arriba del muro, lado del muro, arriba de la caja, lado de la caja y tierra
bajo la isla). Los muros y cajas de `maptiles.png` son a veces un objeto sobre el suelo (roca,
bola de nieve, balde); como cubos hacen falta bloques que llenen la casilla.

Uso:
  cubes.py ref <maptiles.png> <referencia.png>
      Grilla de 3 columnas x 6 filas sobre magenta con muro, caja y suelo de cada tema tal como
      se ven en la partida, para pasarle a ChatGPT.
  cubes.py build <ia.png> <salida.png> [<prueba.png>] [--fix]
      Corta la grilla de 5 columnas x 6 filas que devuelva ChatGPT (separada por franjas
      magenta, como en maptiles.py) y arma la hoja. Con <prueba.png>, cada tema como un cubo de
      muro y uno de caja en falsa perspectiva sobre la tierra, para mirarlos sin abrir el juego.
      --fix aplica `FIX` (los arreglos de la generación usada)."""
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


# ChatGPT dibujó varias caras como objetos en perspectiva (la bola de nieve, el barril y el balde,
# el "hexágono" de oro) o apoyados en el suelo. Por celda (fila, columna): ("crop", recorte en
# fracciones de la celda, bisel) toma solo la parte plana del material y, con bisel, le dibuja un
# borde para que se lea el canto del cubo; ("copy", fila, columna) usa otra celda.
FIX = {
    (0, 1): ("crop", (0.0, 0.0, 1.0, 0.84), False),  # lado del muro de hielo, sin la nieve del suelo
    (0, 2): ("crop", (0.22, 0.14, 0.78, 0.86), True),  # dentro de la bola de nieve
    (0, 3): ("crop", (0.0, 0.0, 1.0, 0.87), False),  # cajón con nieve, sin la del suelo
    (1, 0): ("copy", 1, 1),  # roca entre hojas → la pared de piedras con hojas
    (3, 2): ("crop", (0.2, 0.12, 0.78, 0.5), True),  # tapa del barril
    (3, 3): ("crop", (0.22, 0.32, 0.78, 0.92), True),  # cuerpo del balde
    (5, 2): ("crop", (0.25, 0.12, 0.75, 0.55), True),  # cara de arriba del lingote
    (5, 3): ("crop", (0.03, 0.1, 0.97, 0.99), False),  # caja dorada sin el canto de arriba
}


def bevel(img, width=6, strength=0.35):
    """Aclara los bordes de arriba e izquierda y oscurece los de abajo y derecha."""
    t = np.array(img).astype(np.float32)
    n = t.shape[0]
    d = np.arange(n, dtype=np.float32)
    ramp = np.clip(1 - d / width, 0, 1)
    light = np.maximum.outer(ramp, np.zeros(n)) + np.maximum.outer(np.zeros(n), ramp)
    dark = np.maximum.outer(ramp[::-1], np.zeros(n)) + np.maximum.outer(np.zeros(n), ramp[::-1])
    k = 1 + strength * np.clip(light, 0, 1) - strength * 1.3 * np.clip(dark, 0, 1)
    return Image.fromarray(np.clip(t * k[..., None], 0, 255).round().astype(np.uint8))


def build(ai_path, out_path, *rest):
    fix = "--fix" in rest
    rest = [x for x in rest if x != "--fix"]
    test_path = rest[0] if rest else None
    a = np.array(Image.open(ai_path).convert("RGB")).astype(np.int32)
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    magenta = (r > 180) & (b > 180) & (g < 100)
    row_runs = runs(magenta.mean(1) < 0.5)
    col_runs = runs(magenta.mean(0) < 0.5)
    assert len(row_runs) == ROWS and len(col_runs) == COLS, (row_runs, col_runs)
    cells = {}
    for i, (y0, y1) in enumerate(row_runs):
        for j, (x0, x1) in enumerate(col_runs):
            m = magenta[y0:y1, x0:x1]
            ys = np.nonzero(m.mean(1) < 0.2)[0]
            xs = np.nonzero(m.mean(0) < 0.2)[0]
            # Unos texels más hacia adentro: el antialias deja un hilo magenta en el borde.
            k = 3
            cells[i, j] = a[
                y0 + ys.min() + k:y0 + ys.max() + 1 - k, x0 + xs.min() + k:x0 + xs.max() + 1 - k
            ]
    tiles = []
    for i in range(ROWS):
        for j in range(COLS):
            c, edge = cells[i, j], False
            f = FIX.get((i, j)) if fix else None
            if f and f[0] == "copy":
                c = cells[f[1], f[2]]
            elif f:
                (fx0, fy0, fx1, fy1), edge = f[1], f[2]
                h, w = c.shape[:2]
                c = c[round(fy0 * h):round(fy1 * h), round(fx0 * w):round(fx1 * w)]
            t = Image.fromarray(c.astype(np.uint8)).resize((CELL, CELL), Image.LANCZOS)
            tiles.append(bevel(t) if edge else t)
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
