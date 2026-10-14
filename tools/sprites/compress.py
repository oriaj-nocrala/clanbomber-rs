"""Reduce PNGs a 256 colores con libimagequant (Pillow), con difuminado.
Uso: compress.py <png>..."""
import os
import sys

from PIL import Image

for p in sys.argv[1:]:
    before = os.path.getsize(p)
    im = Image.open(p).convert("RGBA")
    im.quantize(256, method=Image.Quantize.LIBIMAGEQUANT, dither=Image.Dither.FLOYDSTEINBERG).save(
        p, optimize=True
    )
    print(f"{p}: {before // 1024} KB -> {os.path.getsize(p) // 1024} KB")
