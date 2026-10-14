"""Convierte una hoja redibujada por IA (fondo verde, rejilla irregular) en una hoja 4x alineada
con la original. Cada sprite se separa por componentes conectados (los trozos sueltos, como la
punta de un tridente, van al cuerpo más cercano), se escala con una sola escala para toda la hoja
y se apoya en la base del sprite original.
Uso: fit_sheet.py <original.png> <ia.png> <salida.png> [celda_w celda_h] [--head]"""
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

orig_path, ai_path, out_path = sys.argv[1:4]
CW, CH = (int(v) for v in (sys.argv[4:6] if len(sys.argv) > 5 else (40, 60)))
ALIGN_HEAD = "--head" in sys.argv
S = 4
COLS, ROWS = 10, 4

orig = np.array(Image.open(orig_path).convert("RGBA"))
ai = np.array(Image.open(ai_path).convert("RGB")).astype(np.int32)
r, g, b = ai[..., 0], ai[..., 1], ai[..., 2]
# Color de fondo: la mediana del borde de la imagen.
edge = np.concatenate([ai[0], ai[-1], ai[:, 0], ai[:, -1]])
dist = np.sqrt(((ai - np.median(edge, 0)) ** 2).sum(-1))
alpha = np.clip((dist - 35) / 60, 0.0, 1.0)
# Verdoso: domina el verde y no es amarillo (el amarillo casi no tiene azul).
greenish = (g > np.maximum(r, b) + 25) & ((r < 0.45 * g) | (b > 0.5 * r))
# El verde claro es un halo que la IA pinta alrededor (entre los dientes de un tridente): fondo.
alpha[greenish & (g > 180)] = 0.0
fg = alpha > 0.5
g2 = np.where(greenish, np.minimum(g, np.maximum(r, b)), g)
rgba = np.dstack([r, g2, b, (alpha * 255).round()]).clip(0, 255).astype(np.uint8)

# Componentes conectados (8 vecinos) sobre lo que no es fondo, con algo de alfa para no perder
# los bordes finos.
labels, n = ndimage.label(alpha > 0.2, structure=np.ones((3, 3)))
objs = ndimage.find_objects(labels)
areas = ndimage.sum_labels(np.ones_like(labels), labels, range(1, n + 1))
big = [k for k in range(n) if areas[k] > 1500]


def center(k):
    sl = objs[k]
    return ((sl[0].start + sl[0].stop) / 2, (sl[1].start + sl[1].stop) / 2)


# Los cuerpos grandes se reparten en filas y columnas por su centro.
cy = np.array([center(k)[0] for k in big])
cx = np.array([center(k)[1] for k in big])
H, W = labels.shape
sprites = [[[] for _ in range(COLS)] for _ in range(ROWS)]
for k, y, x in zip(big, cy, cx):
    sprites[min(int(y / H * ROWS), ROWS - 1)][min(int(x / W * COLS), COLS - 1)].append(k)
for j in range(ROWS):
    for i in range(COLS):
        assert sprites[j][i], f"celda vacía: fila {j} columna {i}"


def gap(sa, sb):
    """Distancia entre dos rectángulos (0 si se tocan)."""
    dy = max(0, sa[0].start - sb[0].stop, sb[0].start - sa[0].stop)
    dx = max(0, sa[1].start - sb[1].stop, sb[1].start - sa[1].stop)
    return np.hypot(dx, dy)


# Cada trozo pequeño, al cuerpo más cercano.
owner = {}
for j in range(ROWS):
    for i in range(COLS):
        for k in sprites[j][i]:
            owner[k] = (j, i)
for k in range(n):
    if k in owner:
        continue
    best = min(big, key=lambda q: gap(objs[k], objs[q]))
    owner[k] = owner[best]
    sprites[owner[k][0]][owner[k][1]].append(k)


def sprite_box(j, i):
    ks = sprites[j][i]
    y0 = min(objs[k][0].start for k in ks)
    y1 = max(objs[k][0].stop for k in ks)
    x0 = min(objs[k][1].start for k in ks)
    x1 = max(objs[k][1].stop for k in ks)
    return y0, y1, x0, x1


def sprite_rgba(j, i):
    y0, y1, x0, x1 = sprite_box(j, i)
    mask = np.isin(labels[y0:y1, x0:x1], [k + 1 for k in sprites[j][i]])
    out = rgba[y0:y1, x0:x1].copy()
    out[..., 3] = np.where(mask, out[..., 3], 0)
    return out


def orig_box(j, i):
    oc = orig[j * CH:(j + 1) * CH, i * CW:(i + 1) * CW, 3] > 0
    ys, xs = np.nonzero(oc)
    return ys.min(), ys.max() + 1, xs.min(), xs.max() + 1


def fit_scale(box, k):
    y0, y1, x0, x1 = box
    return min(k, (CW * S - 4) / (x1 - x0), (CH * S - 4) / (y1 - y0))


# Escala: la altura de las poses de caminar del original, y una sola para toda la hoja (si no,
# la dirección más ancha sale más chica y el personaje cambia de tamaño al girar).
ratios = []
for j in range(ROWS):
    for i in range(COLS - 1):
        a, o = sprite_box(j, i), orig_box(j, i)
        ratios.append((o[1] - o[0]) * S / (a[1] - a[0]))
k = float(np.median(ratios))
walk_k = min(fit_scale(sprite_box(j, i), k) for j in range(ROWS) for i in range(COLS - 1))
print("escala", k, "final", walk_k)

def head_offset(ref, cur, top, r=16):
    """Desplazamiento (dx, dy) que mejor calza la cabeza (las filas [top]) de cur sobre ref."""
    def prem(a):
        a = a.astype(np.float32)
        return a[..., :3] * a[..., 3:] / 255.0

    pr, pc = prem(ref), prem(cur)
    h = top.stop
    best, arg = None, (0, 0)
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            shifted = np.roll(np.roll(pc, dy, 0), dx, 1)
            err = np.abs(shifted[top.start:h] - pr[top.start:h]).mean()
            if best is None or err < best:
                best, arg = err, (dx, dy)
    return arg


out = Image.new("RGBA", (orig.shape[1] * S, orig.shape[0] * S), (0, 0, 0, 0))
cw, ch = CW * S, CH * S
for j in range(ROWS):
    ref = None
    for i in range(COLS):
        box = sprite_box(j, i)
        kk = walk_k if i < COLS - 1 else fit_scale(box, k)
        img = Image.fromarray(sprite_rgba(j, i))
        w, h = round(img.width * kk), round(img.height * kk)
        img = img.resize((w, h), Image.LANCZOS)
        # Apoyado en la base del original y centrado en su caja, sin salirse de la celda.
        oy0, oy1, ox0, ox1 = orig_box(j, i)
        x = int(np.clip(round((ox0 + ox1) / 2 * S - w / 2), 2, cw - w - 2))
        y = int(np.clip(oy1 * S - h, 2, ch - h - 2))
        if ALIGN_HEAD and i < COLS - 1:
            # Los cuadros de caminar se alinean por la cabeza con el cuadro quieto: la caja cambia
            # con la cola, el tridente o las piernas y la cabeza temblaría.
            cell = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
            cell.alpha_composite(img, (x, y))
            cur = np.array(cell)
            if ref is None:
                ref, ref_top = cur, y
            else:
                dx, dy = head_offset(ref, cur, slice(ref_top, ref_top + int(0.4 * h)))
                x = int(np.clip(x + dx, 2, cw - w - 2))
                y = int(np.clip(y + dy, 2, ch - h - 2))
        out.alpha_composite(img, (i * cw + x, j * ch + y))
out.save(out_path)
print(out.size)
