"""Genera explosion2_hd.png (7 piezas x 3 cuadros, celdas de 160 px) por código: punta izquierda,
horizontal, punta derecha, punta arriba, vertical, punta abajo y centro (Explosion.h), del
cuadro más grueso al más fino. Solo se ve sin compute shaders (si no, las dibuja fire.wgsl).
Cada pieza es una banda de fuego con el borde roto por ruido; el ruido depende de la posición a
lo largo de la banda con período de una celda, así que las piezas vecinas empalman exacto.
Uso: explosion.py <salida.png> [prev.png]"""
import sys

import numpy as np
from PIL import Image

out_path = sys.argv[1]
prev_path = sys.argv[2] if len(sys.argv) > 2 else None
C = 160
# Media anchura de la banda por cuadro (en px de la celda HD) y punta: dónde acaba el brazo.
HALF = [34, 24, 13]
TIP = 22
# Pixel art: se calcula a la mitad y se agranda sin filtrar.
P = 2
N = C // P


def noise_1d(u, seed, octaves=((2, 0.5), (5, 0.3), (11, 0.2))):
    """Suma de senos con frecuencias enteras en una celda: periódica, empalma entre piezas."""
    rng = np.random.default_rng(seed)
    out = np.zeros_like(u, dtype=np.float64)
    for f, amp in octaves:
        out += amp * np.sin(2 * np.pi * f * u / C + rng.uniform(0, 2 * np.pi))
    return out


def grain(u, v, seed):
    """Grano fijo por píxel (de `u` módulo la celda y `v`), para la textura del fuego."""
    h = (np.mod(u, C).astype(np.int64) * 73856093) ^ (v.astype(np.int64) * 19349663) ^ seed
    h = (h * 2654435761) & 0xFFFFFFFF
    return (h / 0xFFFFFFFF) - 0.5


def band(u, v, half, lo, hi, seed):
    """Intensidad 0..1 de una banda a lo largo de `u` (de `lo` a `hi`, con punta redonda donde
    no llega al borde) y ancho `half` alrededor de v = 0."""
    du = np.maximum(np.maximum(lo - u, u - hi), 0)
    d = np.hypot(du, v)
    edge = half * (1 + 0.22 * noise_1d(u, seed) + 0.12 * noise_1d(v * 3, seed + 1))
    t = 1 - d / np.maximum(edge, 1)
    return np.clip(t + 0.18 * grain(u, v, seed), 0, 1)


def ramp(t):
    """Del borde al centro: rojo oscuro, naranja, amarillo y blanco; alfa en el borde."""
    stops = [
        (0.00, (150, 20, 0, 0)),
        (0.10, (200, 40, 5, 220)),
        (0.30, (245, 110, 15, 255)),
        (0.55, (255, 200, 40, 255)),
        (0.80, (255, 245, 170, 255)),
        (1.00, (255, 255, 240, 255)),
    ]
    xs = [s[0] for s in stops]
    return np.stack([np.interp(t, xs, [s[1][c] for s in stops]) for c in range(4)], -1)


y, x = (np.mgrid[0:N, 0:N] * P + P / 2).astype(np.float64)
cx = cy = C / 2
INF = 1e9
# (horizontal: lo, hi) o (vertical: lo, hi) por pieza; None = sin ese brazo.
PIECES = [
    ((TIP, INF), None),  # punta izquierda
    ((-INF, INF), None),  # horizontal
    ((-INF, C - TIP), None),  # punta derecha
    (None, (TIP, INF)),  # punta arriba
    (None, (-INF, INF)),  # vertical
    (None, (-INF, C - TIP)),  # punta abajo
    ((-INF, INF), (-INF, INF)),  # centro
]

sheet = Image.new("RGBA", (7 * C, 3 * C), (0, 0, 0, 0))
for f, half in enumerate(HALF):
    for i, (h, v) in enumerate(PIECES):
        t = np.zeros((N, N))
        if h:
            t = np.maximum(t, band(x, y - cy, half, h[0], h[1], 7))
        if v:
            t = np.maximum(t, band(y, x - cx, half, v[0], v[1], 7))
        if i == 6:
            # El centro, un poco más gordo y caliente.
            t = np.maximum(t, np.clip(1 - np.hypot(x - cx, y - cy) / (half * 1.5), 0, 1))
        img = Image.fromarray(ramp(t).round().astype(np.uint8)).resize((C, C), Image.NEAREST)
        sheet.alpha_composite(img, (i * C, f * C))
sheet.save(out_path)
print(sheet.size)

if prev_path:
    # Una cruz de potencia 2 armada con las piezas, por cuadro, para ver que empalman.
    layout = {(2, 0): 3, (2, 1): 4, (0, 2): 0, (1, 2): 1, (2, 2): 6, (3, 2): 1, (4, 2): 2,
              (2, 3): 4, (2, 4): 5}
    prev = Image.new("RGBA", (3 * 5 * C // 2 + 20, 5 * C // 2), (40, 34, 30, 255))
    for f in range(3):
        for (gx, gy), i in layout.items():
            cell = sheet.crop((i * C, f * C, (i + 1) * C, (f + 1) * C)).resize((C // 2, C // 2))
            prev.alpha_composite(cell, (f * (5 * C // 2 + 10) + gx * C // 2, gy * C // 2))
    prev.save(prev_path)
