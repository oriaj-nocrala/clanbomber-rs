"""Arma bombs.png (8 bombas x 4 cuadros, celdas de 160 px, índice = aspecto * 4 + cuadro)
a partir de una tira de 8 bombas de ChatGPT. Los 4 cuadros hacen titilar la chispa de la mecha.
Uso: bombs.py <ia.png> <salida.png> <gif_prev>"""
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

ai_path, out_path, gif_path = sys.argv[1:4]
CELL = 160
N = 8
SPARK = [1.0, 0.7, 1.25, 0.85]

ai = np.array(Image.open(ai_path).convert("RGB")).astype(np.int32)
r, g, b = ai[..., 0], ai[..., 1], ai[..., 2]
green_px = (g > 180) & (r < 90) & (b < 90)
bg = np.median(ai[green_px], 0)
dist = np.sqrt(((ai - bg) ** 2).sum(-1))
alpha = np.clip((dist - 35) / 60, 0.0, 1.0)
# Solo es fondo lo parecido al verde que está conectado con el borde de la imagen: el brillo
# verde claro de la bomba verde queda dentro de ella y es opaco.
bgish, _ = ndimage.label(alpha < 0.99)
edge_ids = np.unique(np.concatenate([bgish[0], bgish[-1], bgish[:, 0], bgish[:, -1]]))
inside = (alpha < 0.99) & ~np.isin(bgish, edge_ids[edge_ids > 0])
alpha[inside] = 1.0
# Solo se quita el tinte verde en los bordes (semitransparentes): la bomba verde es verde.
greenish = (g > np.maximum(r, b) + 25) & (alpha < 0.99)
g2 = np.where(greenish, np.minimum(g, np.maximum(r, b)), g)
rgba = np.dstack([r, g2, b, (alpha * 255).round()]).clip(0, 255).astype(np.uint8)

labels, n = ndimage.label(alpha > 0.2, structure=np.ones((3, 3)))
objs = ndimage.find_objects(labels)
areas = ndimage.sum_labels(np.ones_like(labels), labels, range(1, n + 1))
big = sorted((k for k in range(n) if areas[k] > 8000), key=lambda k: objs[k][1].start)
assert len(big) == N, len(big)
parts = {k: [k] for k in big}
for k in range(n):
    if k not in parts:
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
    return out


def spark_mask(s):
    """La chispa: amarillo, naranja o blanco en el cuarto de arriba del sprite."""
    sr, sg, sb, sa = (s[..., i].astype(int) for i in range(4))
    m = (sa > 0) & (((sr > 200) & (sg > 100) & (sb < 170)) | ((sr > 230) & (sg > 230)))
    m[int(s.shape[0] * 0.28):] = False
    return ndimage.binary_dilation(m, iterations=2) & (sa > 0)


sprites = [sprite(k) for k in big]
# Una sola escala: la bomba más grande cabe en la celda con 4 px de margen.
k = min((CELL - 8) / max(s.shape[0], s.shape[1]) for s in sprites)
print("escala", k)

sheet = Image.new("RGBA", (8 * CELL, 4 * CELL), (0, 0, 0, 0))
frames_prev = [Image.new("RGBA", (N * CELL, CELL), (110, 95, 80, 255)) for _ in SPARK]
for i, s in enumerate(sprites):
    m = spark_mask(s)
    ys, xs = np.nonzero(m)
    body = s.copy()
    body[..., 3] = np.where(m, 0, body[..., 3])
    spark = s.copy()
    spark[..., 3] = np.where(m, spark[..., 3], 0)
    sy0, sy1, sx0, sx1 = ys.min(), ys.max() + 1, xs.min(), xs.max() + 1
    spark_img = Image.fromarray(spark[sy0:sy1, sx0:sx1])
    scy, scx = (sy0 + sy1) / 2, (sx0 + sx1) / 2
    for f, z in enumerate(SPARK):
        canvas = Image.fromarray(body)
        sw, sh = max(1, round(spark_img.width * z)), max(1, round(spark_img.height * z))
        # Si la chispa grande se sale por arriba, se apoya en el borde.
        pos = (int(round(scx - sw / 2)), max(0, int(round(scy - sh / 2))))
        canvas.alpha_composite(spark_img.resize((sw, sh), Image.LANCZOS), pos)
        w, h = round(canvas.width * k), round(canvas.height * k)
        img = canvas.resize((w, h), Image.LANCZOS)
        cell = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
        # Centrada en horizontal y apoyada abajo, para que todas pisen el mismo suelo.
        cell.alpha_composite(img, ((CELL - w) // 2, CELL - h - 4))
        idx = i * 4 + f
        sheet.alpha_composite(cell, ((idx % 8) * CELL, (idx // 8) * CELL))
        frames_prev[f].alpha_composite(cell, (i * CELL, 0))
sheet.save(out_path)
frames_prev[0].save(gif_path, save_all=True, append_images=frames_prev[1:], duration=80, loop=0)
print(sheet.size)
