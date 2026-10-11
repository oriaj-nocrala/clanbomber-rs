//! Texto con fuentes TrueType en campo de distancias (SDF). Al cargar la fuente cada glifo se
//! rasteriza grande (`BASE_PX`) y se convierte en la distancia con signo a su borde; el shader
//! (`quad.wgsl`, modo TEXT) recorta el borde a la resolución física de la ventana, así que las
//! letras quedan nítidas a cualquier tamaño, con contorno y sombra suave sin texturas extra.

use std::collections::HashMap;

use crate::{Renderer, TextureId, MODE_TEXT};

/// Carácteres que entran en el atlas: ASCII y Latin-1 (acentos y ñ del español).
const CHARS: &[std::ops::RangeInclusive<u32>] = &[0x20..=0x7E, 0xA1..=0xFF];
/// Tamaño al que se rasterizan los glifos del atlas (se escalan al tamaño pedido al dibujar).
const BASE_PX: f32 = 48.0;
/// Alcance del campo de distancias en píxeles del atlas, a cada lado del borde.
const SPREAD: usize = 8;
/// Lado del atlas.
const ATLAS: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Sombra paralela: desplazamiento (px virtuales), difuminado (px) y color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub offset: [f32; 2],
    pub blur: f32,
    pub color: [f32; 4],
}

/// Adornos opcionales del texto. El alfa de la sombra y del contorno se multiplica por el del
/// color del texto, para que se desvanezcan con él.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// Grosor (px virtuales) y color.
    pub outline: Option<(f32, [f32; 4])>,
    pub shadow: Option<Shadow>,
}

impl TextStyle {
    /// Sin adornos.
    pub const PLAIN: TextStyle = TextStyle {
        outline: None,
        shadow: None,
    };
    /// El de [`Renderer::text`]: una sombra suave que despega el texto del fondo.
    pub const DEFAULT: TextStyle = TextStyle {
        outline: None,
        shadow: Some(Shadow {
            offset: [1.5, 2.0],
            blur: 2.0,
            color: [0.0, 0.0, 0.0, 0.7],
        }),
    };
}

#[derive(Clone, Copy)]
struct Glyph {
    /// Región en el atlas (x, y, w, h en píxeles), con el margen del campo de distancias.
    src: [f32; 4],
    /// Desplazamiento del borde izquierdo y de la parte superior de `src` respecto al
    /// origen/línea de arriba, en píxeles del atlas.
    xmin: f32,
    top: f32,
    advance: f32,
}

pub struct Font {
    atlas: TextureId,
    glyphs: HashMap<char, Glyph>,
    face: fontdue::Font,
    /// Píxeles virtuales por píxel del atlas.
    scale: f32,
    line_height: f32,
}

impl Font {
    /// Cada glifo dibujable de `s` con su posición x (px virtuales desde el inicio), y el ancho
    /// total. Aplica el kerning de la fuente entre cada par.
    fn layout(&self, s: &str) -> (Vec<(Glyph, f32)>, f32) {
        let mut out = Vec::with_capacity(s.len());
        let mut cursor = 0.0;
        let mut prev = None;
        for ch in s.chars() {
            let Some(g) = self.glyphs.get(&ch) else {
                continue;
            };
            if let Some(p) = prev {
                cursor += self.face.horizontal_kern(p, ch, BASE_PX).unwrap_or(0.0) * self.scale;
            }
            if g.src[2] > 0.0 {
                out.push((*g, cursor));
            }
            cursor += g.advance * self.scale;
            prev = Some(ch);
        }
        (out, cursor)
    }

    /// Ancho en píxeles de `s` en esta fuente.
    pub fn width(&self, s: &str) -> f32 {
        self.layout(s).1
    }

    pub fn line_height(&self) -> f32 {
        self.line_height
    }
}

#[derive(Debug)]
pub enum FontError {
    Io(std::io::Error),
    Parse(&'static str),
}

/// Transformada de distancia euclídea al cuadrado en 1D (Felzenszwalb y Huttenlocher), sobre
/// `n` valores de `grid` separados por `stride`. `f`, `v` y `z` son memoria de trabajo.
fn edt_1d(
    grid: &mut [f64],
    offset: usize,
    stride: usize,
    n: usize,
    f: &mut [f64],
    v: &mut [usize],
    z: &mut [f64],
) {
    for q in 0..n {
        f[q] = grid[offset + q * stride];
    }
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    let mut k = 0;
    for q in 1..n {
        let fq = f[q] + (q * q) as f64;
        let mut s;
        loop {
            let r = v[k];
            s = (fq - f[r] - (r * r) as f64) / (2 * (q - r)) as f64;
            // z[0] es -inf: nunca baja de 0.
            if s > z[k] {
                break;
            }
            k -= 1;
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for q in 0..n {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let r = v[k];
        let d = q as f64 - r as f64;
        grid[offset + q * stride] = d * d + f[r];
    }
}

fn edt_2d(grid: &mut [f64], w: usize, h: usize) {
    let n = w.max(h);
    let (mut f, mut v, mut z) = (vec![0.0; n], vec![0; n], vec![0.0; n + 1]);
    for x in 0..w {
        edt_1d(grid, x, w, h, &mut f, &mut v, &mut z);
    }
    for y in 0..h {
        edt_1d(grid, y * w, 1, w, &mut f, &mut v, &mut z);
    }
}

/// Campo de distancias de una cobertura (0..1, `w`x`h`): 0.5 en el borde, más dentro de la
/// letra y menos fuera; una unidad son `2 * SPREAD` píxeles. Los píxeles a medio cubrir sitúan el
/// borde dentro del píxel (como TinySDF), así que no hace falta rasterizar a más resolución.
fn sdf(coverage: &[f32], w: usize, h: usize) -> Vec<u8> {
    const FAR: f64 = 1e20;
    let mut outer = vec![0.0; w * h];
    let mut inner = vec![0.0; w * h];
    for (i, &a) in coverage.iter().enumerate() {
        let a = a as f64;
        (outer[i], inner[i]) = if a >= 1.0 {
            (0.0, FAR)
        } else if a <= 0.0 {
            (FAR, 0.0)
        } else {
            ((0.5 - a).max(0.0).powi(2), (a - 0.5).max(0.0).powi(2))
        };
    }
    edt_2d(&mut outer, w, h);
    edt_2d(&mut inner, w, h);
    outer
        .iter()
        .zip(&inner)
        .map(|(o, i)| {
            let d = o.sqrt() - i.sqrt();
            let v = 0.5 - d / (2.0 * SPREAD as f64);
            (v.clamp(0.0, 1.0) * 255.0).round() as u8
        })
        .collect()
}

/// Lleva un ancho en "unidades de distancia" (0..0.5) a los 8 bits que espera el shader.
fn distance_bits(units: f32) -> u32 {
    ((units / 0.5).clamp(0.0, 1.0) * 255.0).round() as u32
}

impl Renderer {
    /// Carga un `.ttf` para dibujarlo a `px` píxeles de alto.
    pub fn load_font(
        &mut self,
        path: impl AsRef<std::path::Path>,
        px: f32,
    ) -> Result<Font, FontError> {
        let data = std::fs::read(path).map_err(FontError::Io)?;
        let face = fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
            .map_err(FontError::Parse)?;
        let lm = face
            .horizontal_line_metrics(BASE_PX)
            .ok_or(FontError::Parse("sin métricas de línea"))?;

        // Empaquetado por filas en un atlas cuadrado; cada glifo con `SPREAD` de margen.
        let mut pixels = vec![0u8; ATLAS * ATLAS * 4];
        let mut glyphs = HashMap::new();
        let (mut cx, mut cy, mut row_h) = (0usize, 0usize, 0usize);
        for ch in CHARS
            .iter()
            .flat_map(|r| r.clone())
            .filter_map(char::from_u32)
        {
            let (m, bitmap) = face.rasterize(ch, BASE_PX);
            let empty = m.width == 0 || m.height == 0;
            let (w, h) = if empty {
                (0, 0)
            } else {
                (m.width + 2 * SPREAD, m.height + 2 * SPREAD)
            };
            if cx + w > ATLAS {
                cx = 0;
                cy += row_h;
                row_h = 0;
            }
            if cy + h > ATLAS {
                break;
            }
            if !empty {
                let mut coverage = vec![0.0; w * h];
                for y in 0..m.height {
                    for x in 0..m.width {
                        coverage[(y + SPREAD) * w + x + SPREAD] =
                            bitmap[y * m.width + x] as f32 / 255.0;
                    }
                }
                let field = sdf(&coverage, w, h);
                for y in 0..h {
                    for x in 0..w {
                        let i = ((cy + y) * ATLAS + cx + x) * 4;
                        pixels[i..i + 4].copy_from_slice(&[255, 255, 255, field[y * w + x]]);
                    }
                }
            }
            glyphs.insert(
                ch,
                Glyph {
                    src: [cx as f32, cy as f32, w as f32, h as f32],
                    xmin: m.xmin as f32 - SPREAD as f32,
                    top: lm.ascent - (m.ymin as f32 + m.height as f32) - SPREAD as f32,
                    advance: m.advance_width,
                },
            );
            cx += w;
            row_h = row_h.max(h);
        }
        let atlas = self.load_rgba_smooth(&pixels, ATLAS as u32, ATLAS as u32);
        let scale = px / BASE_PX;
        Ok(Font {
            atlas,
            glyphs,
            face,
            scale,
            line_height: lm.new_line_size * scale,
        })
    }

    /// Dibuja `s` con la esquina superior de la línea en `(x, y)`; `align` dice si `x` es el
    /// borde izquierdo, el centro o el borde derecho. Lleva la sombra de [`TextStyle::DEFAULT`].
    pub fn text(&mut self, font: &Font, s: &str, x: f32, y: f32, color: [f32; 4], align: Align) {
        self.text_styled(font, s, x, y, color, align, TextStyle::DEFAULT);
    }

    /// Como [`text`](Self::text) pero con los adornos de `style`.
    #[allow(clippy::too_many_arguments)]
    pub fn text_styled(
        &mut self,
        font: &Font,
        s: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        align: Align,
        style: TextStyle,
    ) {
        let (glyphs, w) = font.layout(s);
        let x0 = match align {
            Align::Left => x,
            Align::Center => x - w / 2.0,
            Align::Right => x - w,
        };
        // Px virtuales -> unidades del campo de distancias.
        let units = |px: f32| px / font.scale / (2.0 * SPREAD as f32);
        let outline = style.outline.map_or(0.0, |(t, _)| units(t));
        let mut layer = |offset: [f32; 2], tint: [f32; 4], dilate: f32, soft: f32| {
            let mode = MODE_TEXT | distance_bits(dilate) << 16 | distance_bits(soft) << 24;
            for (g, gx) in &glyphs {
                let dst = [
                    x0 + gx + g.xmin * font.scale + offset[0],
                    y + g.top * font.scale + offset[1],
                    g.src[2] * font.scale,
                    g.src[3] * font.scale,
                ];
                self.push_quad(font.atlas, dst, g.src, tint, mode);
            }
        };
        let fade = |c: [f32; 4]| [c[0], c[1], c[2], c[3] * color[3]];
        // Cada capa entera antes de la siguiente: la sombra de una letra no tapa a la anterior.
        if let Some(sh) = style.shadow {
            layer(sh.offset, fade(sh.color), outline, units(sh.blur));
        }
        if let Some((_, c)) = style.outline {
            layer([0.0; 2], fade(c), outline, 0.0);
        }
        layer([0.0; 2], color, 0.0, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_campo_vale_medio_en_el_borde_y_crece_hacia_dentro() {
        // Un cuadrado lleno de 10x10 en una rejilla de 30x30.
        let (w, h) = (30, 30);
        let cov: Vec<f32> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                if (10..20).contains(&x) && (10..20).contains(&y) {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        let f = sdf(&cov, w, h);
        let at = |x: usize, y: usize| f[y * w + x] as f32 / 255.0;
        // Primer píxel dentro y primero fuera: a un paso de distancia (entre centros de
        // píxel), simétricos respecto al 0.5, así que el borde cae justo entre los dos.
        let step = 1.0 / (2.0 * SPREAD as f32);
        assert!((at(10, 15) - (0.5 + step)).abs() < 0.01, "{}", at(10, 15));
        assert!((at(9, 15) - (0.5 - step)).abs() < 0.01, "{}", at(9, 15));
        // Más adentro y más afuera, monótono.
        assert!(at(12, 15) > at(11, 15) && at(11, 15) > at(10, 15));
        assert!(at(7, 15) < at(8, 15) && at(8, 15) < at(9, 15));
        // Lejos del borde se satura.
        assert_eq!(f[0], 0);
    }

    #[test]
    fn la_cobertura_parcial_mueve_el_borde_dentro_del_pixel() {
        // Media columna cubierta al 50 %: ese píxel es el borde.
        let (w, h) = (20, 5);
        let cov: Vec<f32> = (0..w * h)
            .map(|i| match i % w {
                x if x < 10 => 1.0,
                10 => 0.5,
                _ => 0.0,
            })
            .collect();
        let f = sdf(&cov, w, h);
        assert!((f[2 * w + 10] as f32 / 255.0 - 0.5).abs() < 0.01);
    }
}
