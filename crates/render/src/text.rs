//! Texto con fuentes TrueType: se rasteriza un atlas de glifos al cargar la fuente y cada
//! cadena se dibuja como una fila de quads.

use std::collections::HashMap;

use crate::{Renderer, TextureId};

/// Carácteres que entran en el atlas: ASCII y Latin-1 (acentos y ñ del español).
const CHARS: &[std::ops::RangeInclusive<u32>] = &[0x20..=0x7E, 0xA1..=0xFF];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy)]
struct Glyph {
    /// Región en el atlas (x, y, w, h en píxeles).
    src: [f32; 4],
    /// Desplazamiento del borde izquierdo y de la parte superior respecto al origen/línea base.
    xmin: f32,
    top: f32,
    advance: f32,
}

pub struct Font {
    atlas: TextureId,
    glyphs: HashMap<char, Glyph>,
    line_height: f32,
}

impl Font {
    /// Ancho en píxeles de `s` en esta fuente.
    pub fn width(&self, s: &str) -> f32 {
        s.chars()
            .filter_map(|c| self.glyphs.get(&c))
            .map(|g| g.advance)
            .sum()
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

impl Renderer {
    /// Carga un `.ttf` y rasteriza sus glifos a `px` píxeles de alto.
    pub fn load_font(
        &mut self,
        path: impl AsRef<std::path::Path>,
        px: f32,
    ) -> Result<Font, FontError> {
        let data = std::fs::read(path).map_err(FontError::Io)?;
        let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
            .map_err(FontError::Parse)?;
        let lm = font
            .horizontal_line_metrics(px)
            .ok_or(FontError::Parse("sin métricas de línea"))?;

        // Empaquetado por filas en un atlas cuadrado.
        const SIZE: usize = 1024;
        let mut pixels = vec![0u8; SIZE * SIZE * 4];
        let mut glyphs = HashMap::new();
        let (mut cx, mut cy, mut row_h) = (1usize, 1usize, 0usize);
        for ch in CHARS
            .iter()
            .flat_map(|r| r.clone())
            .filter_map(char::from_u32)
        {
            let (m, bitmap) = font.rasterize(ch, px);
            if cx + m.width + 1 > SIZE {
                cx = 1;
                cy += row_h + 1;
                row_h = 0;
            }
            if cy + m.height + 1 > SIZE {
                break;
            }
            for y in 0..m.height {
                for x in 0..m.width {
                    let i = ((cy + y) * SIZE + cx + x) * 4;
                    pixels[i..i + 4].copy_from_slice(&[255, 255, 255, bitmap[y * m.width + x]]);
                }
            }
            glyphs.insert(
                ch,
                Glyph {
                    src: [cx as f32, cy as f32, m.width as f32, m.height as f32],
                    xmin: m.xmin as f32,
                    top: lm.ascent - (m.ymin as f32 + m.height as f32),
                    advance: m.advance_width,
                },
            );
            cx += m.width + 1;
            row_h = row_h.max(m.height);
        }
        let atlas = self.load_rgba(&pixels, SIZE as u32, SIZE as u32);
        Ok(Font {
            atlas,
            glyphs,
            line_height: lm.new_line_size,
        })
    }

    /// Dibuja `s` con la esquina superior de la línea en `(x, y)`; `align` dice si `x` es el
    /// borde izquierdo, el centro o el borde derecho.
    pub fn text(&mut self, font: &Font, s: &str, x: f32, y: f32, color: [f32; 4], align: Align) {
        let w = font.width(s);
        let mut cursor = match align {
            Align::Left => x,
            Align::Center => x - w / 2.0,
            Align::Right => x - w,
        };
        for ch in s.chars() {
            let Some(g) = font.glyphs.get(&ch) else {
                continue;
            };
            if g.src[2] > 0.0 {
                let dst = [
                    (cursor + g.xmin).round(),
                    (y + g.top).round(),
                    g.src[2],
                    g.src[3],
                ];
                self.sprite_tinted(font.atlas, g.src, dst, color);
            }
            cursor += g.advance;
        }
    }
}
