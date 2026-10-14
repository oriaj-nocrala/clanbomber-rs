"""Arma not_available.png (la X de "mapa no disponible", 256 px a 4x) a partir de una X roja
de ChatGPT sobre negro: la transparencia sale del brillo.
Uso: not_available.py <ia.png> <salida.png>"""
import sys

import numpy as np
from PIL import Image

src, dst = sys.argv[1:3]
SIZE, FILL = 256 * 4, 0.94

a = np.array(Image.open(src).convert("RGB")).astype(np.float32)
alpha = np.clip((a.max(-1) - 15) / 110, 0, 1)
rgb = a / np.maximum(alpha[..., None], 1e-3)
img = Image.fromarray(np.dstack([np.clip(rgb, 0, 255), alpha * 255]).round().astype(np.uint8))
img = img.crop(img.getbbox())
k = SIZE * FILL / max(img.size)
img = img.resize((round(img.width * k), round(img.height * k)), Image.LANCZOS)
out = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
out.alpha_composite(img, ((SIZE - img.width) // 2, (SIZE - img.height) // 2))
out.save(dst)
print(out.size)
