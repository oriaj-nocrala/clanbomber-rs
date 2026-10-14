"""Arma las 4 hojas HD de las pelotas (bomber_dull_*) a partir de 5 sprites azules de ChatGPT
(abajo, izquierda, arriba, derecha, muerta): cada dirección se repite en los 9 cuadros (las
originales tampoco se animan) y los otros colores salen girando el tono de lo azul.
Uso: balls.py <ia.png> <dir_pics>"""
import colorsys
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

ai_path, pics = sys.argv[1:3]
C, S = 40, 4
cell = C * S

ai = np.array(Image.open(ai_path).convert("RGB")).astype(np.int32)
r, g, b = ai[..., 0], ai[..., 1], ai[..., 2]
green_px = (g > 180) & (r < 90) & (b < 90)
# Recorta la franja verde (la imagen trae bordes blancos).
rows = np.nonzero(green_px.mean(1) > 0.3)[0]
cols = np.nonzero(green_px.mean(0) > 0.3)[0]
ai = ai[rows.min():rows.max() + 1, cols.min():cols.max() + 1]
r, g, b = ai[..., 0], ai[..., 1], ai[..., 2]
bg = np.median(ai[green_px[rows.min():rows.max() + 1, cols.min():cols.max() + 1]], 0)

dist = np.sqrt(((ai - bg) ** 2).sum(-1))
alpha = np.clip((dist - 35) / 60, 0.0, 1.0)
greenish = (g > np.maximum(r, b) + 25) & ((r < 0.45 * g) | (b > 0.5 * r))
alpha[greenish & (g > 180)] = 0.0
g2 = np.where(greenish, np.minimum(g, np.maximum(r, b)), g)
rgba = np.dstack([r, g2, b, (alpha * 255).round()]).clip(0, 255).astype(np.uint8)

labels, n = ndimage.label(alpha > 0.2, structure=np.ones((3, 3)))
objs = ndimage.find_objects(labels)
areas = ndimage.sum_labels(np.ones_like(labels), labels, range(1, n + 1))
big = sorted((k for k in range(n) if areas[k] > 5000), key=lambda k: objs[k][1].start)
assert len(big) == 5, len(big)
parts = {k: [k] for k in big}
for k in range(n):
    if k in parts:
        continue
    # Trozos sueltos (gotas de sangre), al sprite más cercano en horizontal.
    cx = (objs[k][1].start + objs[k][1].stop) / 2
    near = min(big, key=lambda q: abs((objs[q][1].start + objs[q][1].stop) / 2 - cx))
    parts[near].append(k)


def sprite(k):
    ks = parts[k]
    y0 = min(objs[q][0].start for q in ks)
    y1 = max(objs[q][0].stop for q in ks)
    x0 = min(objs[q][1].start for q in ks)
    x1 = max(objs[q][1].stop for q in ks)
    out = rgba[y0:y1, x0:x1].copy()
    out[..., 3] = np.where(np.isin(labels[y0:y1, x0:x1], [q + 1 for q in ks]), out[..., 3], 0)
    return Image.fromarray(out)


# Escala: la pelota más grande ocupa la celda con 3 px de margen (como la original).
balls = [sprite(k) for k in big]
kk = min((cell - 6) / max(im.width, im.height) for im in balls[:4])
cells = []
for i, im in enumerate(balls):
    k = kk if i < 4 else min(kk, (cell - 6) / im.width, (cell - 6) / im.height)
    im = im.resize((round(im.width * k), round(im.height * k)), Image.LANCZOS)
    c = Image.new("RGBA", (cell, cell), (0, 0, 0, 0))
    # Las pelotas, centradas; la muerta, apoyada abajo.
    y = (cell - im.height) // 2 if i < 4 else cell - im.height - 3
    c.alpha_composite(im, ((cell - im.width) // 2, y))
    cells.append(c)

sheet = Image.new("RGBA", (10 * cell, 4 * cell), (0, 0, 0, 0))
for j in range(4):
    for i in range(9):
        sheet.alpha_composite(cells[j], (i * cell, j * cell))
    sheet.alpha_composite(cells[4], (9 * cell, j * cell))
base = np.array(sheet).astype(np.float32) / 255.0


def recolor(hue, sat=1.0, val=1.0):
    """Gira a `hue` (0..1) el tono de lo azul; la sangre, los ojos y el contorno no cambian."""
    out = base.copy()
    rgb = out[..., :3]
    mx, mn = rgb.max(-1), rgb.min(-1)
    h = np.vectorize(lambda R, G, B: colorsys.rgb_to_hsv(R, G, B)[0])(
        rgb[..., 0], rgb[..., 1], rgb[..., 2]
    )
    s = np.where(mx > 0, (mx - mn) / np.maximum(mx, 1e-6), 0)
    blue = (h > 0.5) & (h < 0.75) & (s > 0.25)
    ys, xs = np.nonzero(blue)
    for y, x in zip(ys, xs):
        _, ss, vv = colorsys.rgb_to_hsv(*rgb[y, x])
        rgb[y, x] = colorsys.hsv_to_rgb(hue, min(ss * sat, 1.0), min(vv * val, 1.0))
    return Image.fromarray((out * 255).round().astype(np.uint8))


for name, args in {
    "blue": None,
    "red": (0.0, 0.95, 1.0),
    "yellow": (0.14, 1.0, 1.5),
    "green": (0.33, 0.9, 1.05),
}.items():
    img = sheet if args is None else recolor(*args)
    img.save(f"{pics}/bomber_dull_{name}_hd.png")
    print(name)
